use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::time::SystemTime;

use avian3d::prelude::*;
use bevy::math::ops;
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy_replicon::prelude::*;
use bevy_replicon::shared::backend::connected_client::NetworkId;
use bevy_replicon_renet2::{
    netcode::{NativeSocket, NetcodeServerTransport, ServerAuthentication, ServerSetupConfig},
    renet2::{ConnectionConfig, RenetServer},
    RenetChannelsExt, RepliconRenetPlugins,
};
use big_brain::prelude::*;
use shared::{
    zombie_hit_damage_geometric, Chase, ClientInput, DebugBot, DespawnDebugBot, FireShot, GameOver,
    GameStateEvent, Health, KnockbackVelocity, LastProcessedInput, MoveDebugBot, NetPlayer, NetTransform,
    Player, PlayerDied, PlayerStance, RequestRestart, ShotFx, SpawnDebugBot, SpawnMoreZombies, Zombie,
    ZombieAttackTimer, ZombieMotion, ARENA_HALF_SIZE, ARENA_WALL_HEIGHT, ARENA_WALL_THICKNESS, GRAVITY,
    GROUND_NORMAL_MIN_Y, KNOCKBACK_DECAY_RATE, KNOCKBACK_FORCE, MAX_SHOT_DISTANCE, MOVE_SPEED,
    NAVMESH_BOUNDS, NAVMESH_COLUMN_OBSTACLES, NAVMESH_HEIGHT, PLAYER_SPAWN, ZOMBIE_AI_ENABLED,
    ZOMBIE_AVOID_ANGLES_DEG, ZOMBIE_AVOID_PROBE_DISTANCE, ZOMBIE_CHASE_RANGE, ZOMBIE_LENGTH,
    ZOMBIE_LOD_DISTANCE, ZOMBIE_LOD_REST_HEIGHT, ZOMBIE_MAX_HEALTH, ZOMBIE_PATH_RECALC_INTERVAL,
    DIRECTOR_CALM_DURATION_SECS, DIRECTOR_CALM_PRESSURE_THRESHOLD, DIRECTOR_HORDE_SIZE,
    DIRECTOR_HORDE_SPAWN_RADIUS, DIRECTOR_MELEE_RANGE, DIRECTOR_PRESSURE_SMOOTHING, PLAYER_THREAT_DECAY_RATE,
    PLAYER_THREAT_PER_SHOT, ZOMBIE_LOAD_BALANCE_PENALTY, ZOMBIE_RADIUS, ZOMBIE_RESPAWN_DELAY,
    ZOMBIE_RETARGET_INTERVAL, ZOMBIE_SEPARATION_RADIUS, ZOMBIE_SEPARATION_STRENGTH, ZOMBIE_SPAWNS,
    ZOMBIE_SPEED, ZOMBIE_STOP_DISTANCE, ZOMBIE_THREAT_DISTANCE_BONUS,
};
use shared::sim_latency::{sim_latency_from_env, LaggedSocket};
use vleue_navigator::prelude::*;

// Fixed debug port — the client connects here by default (see SERVER_ADDR
// in crates/client). No auth/matchmaking: ServerAuthentication::Unsecure
// and a large max_clients stand in for "no player limit" per the plan.
const PORT: u16 = 5031;
const PROTOCOL_ID: u64 = 0;
const MAX_CLIENTS: usize = 256;

fn main() {
    App::new()
        .add_plugins((
            MinimalPlugins,
            bevy::log::LogPlugin::default(),
            bevy::asset::AssetPlugin::default(),
            bevy::mesh::MeshPlugin,
            bevy::scene::ScenePlugin,
            StatesPlugin,
            PhysicsPlugins::default(),
            BigBrainPlugin::new(PreUpdate),
            VleueNavigatorPlugin,
            RepliconPlugins,
            RepliconRenetPlugins,
        ))
        .replicate::<NetworkId>()
        .replicate::<NetPlayer>()
        .replicate::<NetTransform>()
        .replicate::<Zombie>()
        .replicate::<Health>()
        .replicate::<LastProcessedInput>()
        .replicate::<DebugBot>()
        .add_client_event::<ClientInput>(Channel::Unreliable)
        .add_client_event::<SpawnMoreZombies>(Channel::Unordered)
        .add_client_event::<FireShot>(Channel::Ordered)
        .add_server_event::<ShotFx>(Channel::Ordered)
        .add_client_event::<PlayerDied>(Channel::Ordered)
        .add_client_event::<RequestRestart>(Channel::Ordered)
        .add_server_event::<GameStateEvent>(Channel::Ordered)
        .add_client_event::<SpawnDebugBot>(Channel::Ordered)
        .add_client_event::<MoveDebugBot>(Channel::Ordered)
        .add_client_event::<DespawnDebugBot>(Channel::Ordered)
        .init_resource::<PendingRespawns>()
        .init_resource::<GameOver>()
        .init_resource::<DebugBotIdCounter>()
        .init_resource::<Director>()
        .add_systems(Startup, (start_server, spawn_arena_colliders, spawn_zombie_navmesh, spawn_initial_zombies))
        .add_systems(
            PreUpdate,
            (
                chase_action_system.in_set(BigBrainSet::Actions),
                near_player_scorer.in_set(BigBrainSet::Scorers),
            ),
        )
        .add_systems(
            Update,
            (move_players, move_zombies, move_debug_bots, tick_respawns, decay_player_threat, run_ai_director),
        )
        .add_observer(spawn_player)
        .add_observer(log_client_disconnect)
        .add_observer(apply_client_input)
        .add_observer(spawn_more_zombies)
        .add_observer(on_player_died)
        .add_observer(on_request_restart)
        .add_observer(handle_fire_shot)
        .add_observer(spawn_debug_bot)
        .add_observer(move_debug_bot_command)
        .add_observer(despawn_debug_bot)
        .run();
}

fn start_server(mut commands: Commands, channels: Res<RepliconChannels>) {
    let server = RenetServer::new(ConnectionConfig::from_channels(
        channels.server_configs(),
        channels.client_configs(),
    ));

    let current_time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("system clock should be after the unix epoch");
    let public_addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), PORT);
    let socket = UdpSocket::bind(public_addr).expect("failed to bind server UDP socket");
    let server_config = ServerSetupConfig {
        current_time,
        max_clients: MAX_CLIENTS,
        protocol_id: PROTOCOL_ID,
        authentication: ServerAuthentication::Unsecure,
        socket_addresses: vec![vec![public_addr]],
    };
    let socket = LaggedSocket::new(NativeSocket::new(socket).unwrap(), sim_latency_from_env());
    let transport = NetcodeServerTransport::new(server_config, socket).expect("failed to set up netcode server transport");

    commands.insert_resource(server);
    commands.insert_resource(transport);
    info!("dedicated server listening on {public_addr}");
}

// Same flat room as crates/client's USE_TEST_ARENA path (floor + 4 walls),
// minus the Mesh3d/MeshMaterial3d/lights — this process never renders
// anything, it only needs the avian3d colliders so move_players has
// something to collide against.
fn spawn_arena_colliders(mut commands: Commands) {
    let floor_size = ARENA_HALF_SIZE * 2.0;
    commands.spawn((
        Transform::from_xyz(0.0, -ARENA_WALL_THICKNESS / 2.0, 0.0),
        RigidBody::Static,
        Collider::cuboid(floor_size, ARENA_WALL_THICKNESS, floor_size),
    ));

    let wall_span = floor_size + ARENA_WALL_THICKNESS;
    let walls = [
        (Vec3::new(0.0, ARENA_WALL_HEIGHT / 2.0, -ARENA_HALF_SIZE), wall_span, ARENA_WALL_THICKNESS),
        (Vec3::new(0.0, ARENA_WALL_HEIGHT / 2.0, ARENA_HALF_SIZE), wall_span, ARENA_WALL_THICKNESS),
        (Vec3::new(-ARENA_HALF_SIZE, ARENA_WALL_HEIGHT / 2.0, 0.0), ARENA_WALL_THICKNESS, wall_span),
        (Vec3::new(ARENA_HALF_SIZE, ARENA_WALL_HEIGHT / 2.0, 0.0), ARENA_WALL_THICKNESS, wall_span),
    ];
    for (center, size_x, size_z) in walls {
        commands.spawn((
            Transform::from_translation(center),
            RigidBody::Static,
            Collider::cuboid(size_x, ARENA_WALL_HEIGHT, size_z),
        ));
    }
}

// Spawns the player's gameplay components directly onto its connection
// entity (same pattern bevy_replicon_renet2's tic_tac_toe example uses) —
// no separate lookup/owner component needed, since ClientId::Client wraps
// this exact Entity.
fn spawn_player(trigger: On<Add, ConnectedClient>, mut commands: Commands) {
    let entity = trigger.event().entity;
    info!("client connected: {entity}");
    commands.entity(entity).insert((
        Transform::from_translation(PLAYER_SPAWN),
        Player {
            vertical_velocity: 0.0,
            grounded: false,
        },
        PlayerStance::default(),
        ClientInput::default(),
        NetPlayer,
        NetTransform {
            translation: PLAYER_SPAWN,
            yaw: 0.0,
            moving: false,
        },
        LastProcessedInput::default(),
        PlayerThreat::default(),
        Replicated,
    ));
}

// Server-only (not replicated) aggro state — see ZOMBIE_THREAT_DISTANCE_BONUS
// in crates/shared for the full explanation. Every NetPlayer-carrying entity
// needs one (including debug bots, which just never fire and so stay at 0),
// since move_zombies' target query requires it alongside NetPlayer.
#[derive(Component, Default)]
struct PlayerThreat(f32);

fn decay_player_threat(time: Res<Time>, mut threats: Query<&mut PlayerThreat>) {
    let decay = (1.0 - PLAYER_THREAT_DECAY_RATE * time.delta_secs()).clamp(0.0, 1.0);
    for mut threat in &mut threats {
        threat.0 *= decay;
    }
}

// Stress-based horde pacing: smooths "how many zombies are currently close
// enough to bite someone" into `pressure`, and once that's stayed calm for
// DIRECTOR_CALM_DURATION_SECS straight, spawns a DIRECTOR_HORDE_SIZE burst
// near a connected player — the L4D-style "quiet, then a scare" beat. See
// DIRECTOR_MELEE_RANGE's own doc comment for why pressure is measured this
// way instead of from player Health (which the server can't see).
#[derive(Resource, Default)]
struct Director {
    pressure: f32,
    calm_timer: f32,
}

fn run_ai_director(
    time: Res<Time>,
    mut director: ResMut<Director>,
    mut commands: Commands,
    zombies: Query<(&Transform, &ChaseTarget), With<Zombie>>,
    players: Query<(Entity, &Transform), (With<NetPlayer>, Without<Zombie>)>,
) {
    let dt = time.delta_secs();
    let player_positions: Vec<(Entity, Vec3)> = players.iter().map(|(e, t)| (e, t.translation)).collect();
    if player_positions.is_empty() {
        return;
    }

    let in_melee_range = zombies
        .iter()
        .filter(|(transform, chase_target)| {
            chase_target
                .entity
                .and_then(|target| player_positions.iter().find(|&&(e, _)| e == target))
                .is_some_and(|&(_, pos)| transform.translation.distance(pos) <= DIRECTOR_MELEE_RANGE)
        })
        .count() as f32;

    director.pressure += (in_melee_range - director.pressure) * (DIRECTOR_PRESSURE_SMOOTHING * dt).min(1.0);

    if director.pressure <= DIRECTOR_CALM_PRESSURE_THRESHOLD {
        director.calm_timer += dt;
    } else {
        director.calm_timer = 0.0;
    }

    if director.calm_timer < DIRECTOR_CALM_DURATION_SECS {
        return;
    }
    director.calm_timer = 0.0;

    let center = player_positions[(time.elapsed_secs() as usize) % player_positions.len()].1;
    let spawns: Vec<Vec3> = (0..DIRECTOR_HORDE_SIZE)
        .map(|i| {
            let angle = i as f32 / DIRECTOR_HORDE_SIZE as f32 * std::f32::consts::TAU;
            center + Vec3::new(angle.cos(), 0.0, angle.sin()) * DIRECTOR_HORDE_SPAWN_RADIUS
        })
        .collect();
    info!("AI Director: sustained calm, spawning a {}-zombie horde near {center}", DIRECTOR_HORDE_SIZE);
    spawn_zombies(&mut commands, &spawns, true);
}

fn log_client_disconnect(trigger: On<Remove, ConnectedClient>) {
    info!("client disconnected: {}", trigger.event().entity);
}

// ClientId::Client(Entity) IS the connection entity spawn_player already
// attached ClientInput to — no separate client-id-to-entity map needed.
fn apply_client_input(
    trigger: On<FromClient<ClientInput>>,
    mut players: Query<(&mut ClientInput, &mut LastProcessedInput)>,
) {
    let Some(entity) = trigger.client_id.entity() else {
        return;
    };
    if let Ok((mut input, mut last_processed)) = players.get_mut(entity) {
        *input = trigger.message;
        last_processed.0 = trigger.message.sequence;
    }
}

// Same MoveAndSlide-based movement the client predicts locally (see
// shared::movement::simulate_player_step's own doc comment) — driven here
// by the latest received ClientInput instead of a locally-pressed key,
// since this process is what actually decides where a player capsule
// ends up.
fn move_players(
    time: Res<Time>,
    mut players: Query<(&mut Transform, &mut Player, &PlayerStance, &ClientInput, &mut NetTransform)>,
    mover: MoveAndSlide,
) {
    for (mut transform, mut player, stance, input, mut net_transform) in &mut players {
        shared::movement::simulate_player_step(&mover, &mut transform, &mut player, *stance, input, time.delta_secs());
        net_transform.translation = transform.translation;
        net_transform.yaw = input.yaw;
    }
}

// --- Debug bots ---
// Fake NetPlayer stand-ins spawned/moved from the client's debug panel (see
// crates/client) so multi-player zombie targeting can be tested without a
// second real client. No Player/ClientInput/physics — move_debug_bots just
// walks them in a straight line toward DebugBotTarget, kinematically.

#[derive(Resource, Default)]
struct DebugBotIdCounter(u32);

#[derive(Component)]
struct DebugBotTarget(Vec3);

fn spawn_debug_bot(trigger: On<FromClient<SpawnDebugBot>>, mut commands: Commands, mut counter: ResMut<DebugBotIdCounter>) {
    let id = counter.0;
    counter.0 += 1;
    let at = trigger.message.at;
    commands.spawn((
        Transform::from_translation(at),
        NetPlayer,
        NetTransform {
            translation: at,
            yaw: 0.0,
            moving: false,
        },
        DebugBot(id),
        DebugBotTarget(at),
        PlayerThreat::default(),
        Replicated,
    ));
    info!("debug bot {id} spawned at {at}");
}

fn move_debug_bot_command(
    trigger: On<FromClient<MoveDebugBot>>,
    mut bots: Query<(&DebugBot, &mut DebugBotTarget)>,
) {
    for (bot, mut target) in &mut bots {
        if bot.0 == trigger.message.id {
            target.0 = trigger.message.target;
            break;
        }
    }
}

fn despawn_debug_bot(trigger: On<FromClient<DespawnDebugBot>>, mut commands: Commands, bots: Query<(Entity, &DebugBot)>) {
    for (entity, bot) in &bots {
        if bot.0 == trigger.message.id {
            commands.entity(entity).despawn();
            break;
        }
    }
}

fn move_debug_bots(time: Res<Time>, mut bots: Query<(&mut Transform, &mut NetTransform, &DebugBotTarget), With<DebugBot>>) {
    for (mut transform, mut net_transform, target) in &mut bots {
        let to_target = target.0 - transform.translation;
        let horizontal = Vec3::new(to_target.x, 0.0, to_target.z);
        let distance = horizontal.length();
        if distance > 0.05 {
            let direction = horizontal / distance;
            let step = (MOVE_SPEED * time.delta_secs()).min(distance);
            transform.translation += direction * step;
            transform.look_to(-direction, Vec3::Y);
            net_transform.yaw = transform.rotation.to_euler(EulerRot::YXZ).0;
        }
        net_transform.translation = transform.translation;
    }
}

fn spawn_zombie_navmesh(mut commands: Commands, mut navmeshes: ResMut<Assets<NavMesh>>) {
    let mut navmesh = NavMesh::from_edge_and_obstacles(
        NAVMESH_BOUNDS.to_vec(),
        NAVMESH_COLUMN_OBSTACLES.iter().map(|&(x, z)| column_footprint(x, z)).collect(),
    );
    navmesh.set_transform(
        Transform::from_xyz(0.0, NAVMESH_HEIGHT, 0.0)
            .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
    );
    navmeshes
        .insert(ManagedNavMesh::get_single().id(), navmesh)
        .expect("inserting under a Uuid asset id never fails");
    commands.spawn(ManagedNavMesh::single());
}

fn column_footprint(x: f32, z: f32) -> Vec<Vec2> {
    const COLUMN_RADIUS: f32 = 0.5;
    vec![
        Vec2::new(x - COLUMN_RADIUS, z - COLUMN_RADIUS),
        Vec2::new(x - COLUMN_RADIUS, z + COLUMN_RADIUS),
        Vec2::new(x + COLUMN_RADIUS, z + COLUMN_RADIUS),
        Vec2::new(x + COLUMN_RADIUS, z - COLUMN_RADIUS),
    ]
}

fn spawn_initial_zombies(mut commands: Commands) {
    spawn_zombies(&mut commands, &ZOMBIE_SPAWNS, false);
}

fn spawn_more_zombies(_trigger: On<FromClient<SpawnMoreZombies>>, mut commands: Commands) {
    spawn_zombies(&mut commands, &ZOMBIE_SPAWNS, false);
}

// Marks a zombie spawned by run_ai_director as a bonus horde rather than
// baseline population — handle_fire_shot checks this to skip scheduling a
// 1:1 respawn on kill, so a calm-triggered horde actually thins out once
// dealt with instead of permanently growing the total zombie count.
#[derive(Component)]
struct DirectorSpawned;

fn spawn_zombies(commands: &mut Commands, spawns: &[Vec3], director_spawned: bool) {
    for &spawn_pos in spawns {
        let mut zombie = commands.spawn((
            Transform::from_translation(spawn_pos),
            Zombie,
            ZombieMotion {
                // Jittered from spawn_pos (deterministic, cheap) so 100
                // zombies spawned at once don't all recompute their path on
                // the exact same frame forever after.
                path_recalc_timer: ops::sin(spawn_pos.x * 12.9898 + spawn_pos.z * 78.233).abs()
                    * ZOMBIE_PATH_RECALC_INTERVAL,
                ..default()
            },
            KnockbackVelocity::default(),
            ZombieAttackTimer::default(),
            NetTransform {
                translation: spawn_pos,
                yaw: 0.0,
                moving: false,
            },
            Health(ZOMBIE_MAX_HEALTH),
            Replicated,
            // Raycast-only: no RigidBody, matching how this worked
            // client-side before zombies moved server-side — avian3d's
            // spatial queries pick up a bare Collider fine.
            Collider::capsule(ZOMBIE_RADIUS, ZOMBIE_LENGTH),
        ));
        if ZOMBIE_AI_ENABLED {
            zombie.insert(Thinker::build().picker(FirstToScore { threshold: 0.8 }).when(NearPlayer, ChaseAction));
        }
        if director_spawned {
            zombie.insert(DirectorSpawned);
        }
    }
}

// Scorer: 1.0 when within ZOMBIE_CHASE_RANGE of the current TargetPlayer, 0
// if there's no target player connected yet.
#[derive(Clone, Component, Debug, ScorerBuilder)]
struct NearPlayer;

// True as soon as ANY connected player is within range — which specific
// player doesn't matter here, only whether there's someone worth chasing
// at all. move_zombies picks whichever connected player is actually
// nearest fresh every frame once Chase is present (see Chase's own doc
// comment on why there's no fixed target to track).
fn near_player_scorer(
    transforms: Query<&Transform>,
    players: Query<&Transform, With<NetPlayer>>,
    mut query: Query<(&Actor, &mut Score), With<NearPlayer>>,
) {
    for (Actor(actor), mut score) in &mut query {
        let Ok(actor_transform) = transforms.get(*actor) else {
            score.set(0.0);
            continue;
        };
        let in_range = players
            .iter()
            .any(|player_transform| actor_transform.translation.distance(player_transform.translation) <= ZOMBIE_CHASE_RANGE);
        score.set(if in_range { 1.0 } else { 0.0 });
    }
}

// Action: inserts Chase on Requested (entering the picked state), removes it
// on Cancelled. Stays Executing indefinitely otherwise — chasing has no
// "done" state, only "no longer in range".
#[derive(Clone, Component, Debug, ActionBuilder)]
struct ChaseAction;

// Server-only (not replicated): which player entity a chasing zombie is
// currently committed to, plus a countdown to the next time it's allowed to
// re-pick — see ZOMBIE_RETARGET_INTERVAL. `entity: None` forces an
// immediate pick on the first move_zombies tick after Chase is inserted.
#[derive(Component, Default)]
struct ChaseTarget {
    entity: Option<Entity>,
    reevaluate_timer: f32,
}

fn chase_action_system(mut commands: Commands, mut query: Query<(&Actor, &mut ActionState, &ActionSpan)>) {
    for (Actor(actor), mut state, span) in &mut query {
        let _guard = span.span().enter();
        match *state {
            // try_insert/try_remove: the actor can already be despawned by
            // the time this command actually applies — same benign race the
            // single-player version worked around.
            ActionState::Requested => {
                commands.entity(*actor).try_insert((Chase { speed: ZOMBIE_SPEED }, ChaseTarget::default()));
                *state = ActionState::Executing;
            }
            ActionState::Cancelled => {
                commands.entity(*actor).try_remove::<(Chase, ChaseTarget)>();
                *state = ActionState::Success;
            }
            _ => {}
        }
    }
}

fn steer_around_obstacles(
    spatial_query: &SpatialQuery,
    origin: Vec3,
    desired_direction: Vec3,
    distance_to_target: f32,
    filter: &SpatialQueryFilter,
) -> Vec3 {
    let probe_distance = ZOMBIE_AVOID_PROBE_DISTANCE.min(distance_to_target);
    let is_clear = |direction: Vec3| {
        Dir3::new(direction).is_ok_and(|dir| spatial_query.cast_ray(origin, dir, probe_distance, false, filter).is_none())
    };

    if is_clear(desired_direction) {
        return desired_direction;
    }
    for angle_deg in ZOMBIE_AVOID_ANGLES_DEG {
        let rotated = Quat::from_rotation_y(angle_deg.to_radians()) * desired_direction;
        if is_clear(rotated) {
            return rotated;
        }
    }
    desired_direction
}

fn move_zombies(
    time: Res<Time>,
    mut zombies: Query<
        (Entity, &mut Transform, &mut ZombieMotion, &Chase, &mut ChaseTarget, &mut KnockbackVelocity, &mut NetTransform),
        With<Zombie>,
    >,
    players: Query<(Entity, &Transform, &PlayerThreat), (With<NetPlayer>, Without<Zombie>)>,
    navmesh: Query<&ManagedNavMesh>,
    navmeshes: Res<Assets<NavMesh>>,
    mover: MoveAndSlide,
) {
    let navmesh = navmesh.single().ok().and_then(|handle| navmeshes.get(handle));

    let positions: Vec<(Entity, Vec3)> = zombies.iter().map(|(e, t, ..)| (e, t.translation)).collect();
    // Snapshot once per frame rather than re-querying per zombie — with
    // more than one player connected, whichever scores best can be a
    // different one for each zombie (see Chase's own doc comment).
    let candidates: Vec<(Entity, Vec3, f32)> = players.iter().map(|(e, t, threat)| (e, t.translation, threat.0)).collect();
    // How many zombies are currently committed to each player, snapshotted
    // before this frame's re-evaluations — see ZOMBIE_LOAD_BALANCE_PENALTY.
    let mut chase_load: std::collections::HashMap<Entity, u32> = std::collections::HashMap::new();
    for (.., chase_target, _, _) in &zombies {
        if let Some(target) = chase_target.entity {
            *chase_load.entry(target).or_insert(0) += 1;
        }
    }

    for (entity, mut transform, mut motion, chase, mut chase_target, mut knockback, mut net_transform) in &mut zombies {
        // Commit to the current target until it disconnects or the
        // re-evaluate timer runs out — see ZOMBIE_RETARGET_INTERVAL's doc
        // comment on why "nearest every frame" flickers with close players.
        chase_target.reevaluate_timer -= time.delta_secs();
        let target_still_connected = chase_target.entity.is_some_and(|target| candidates.iter().any(|&(e, ..)| e == target));
        if !target_still_connected || chase_target.reevaluate_timer <= 0.0 {
            // Lower score wins: plain distance minus a threat-driven bonus
            // (see ZOMBIE_THREAT_DISTANCE_BONUS) — a player who's been
            // shooting can pull attention off someone standing closer but
            // quiet.
            chase_target.entity = candidates
                .iter()
                .min_by(|(a_entity, a_pos, a_threat), (b_entity, b_pos, b_threat)| {
                    let a_load = *chase_load.get(a_entity).unwrap_or(&0) as f32;
                    let b_load = *chase_load.get(b_entity).unwrap_or(&0) as f32;
                    let a_score = transform.translation.distance(*a_pos) - a_threat * ZOMBIE_THREAT_DISTANCE_BONUS
                        + a_load * ZOMBIE_LOAD_BALANCE_PENALTY;
                    let b_score = transform.translation.distance(*b_pos) - b_threat * ZOMBIE_THREAT_DISTANCE_BONUS
                        + b_load * ZOMBIE_LOAD_BALANCE_PENALTY;
                    a_score.total_cmp(&b_score)
                })
                .map(|&(e, ..)| e);
            chase_target.reevaluate_timer = ZOMBIE_RETARGET_INTERVAL;
        }
        let Some(target_translation) =
            chase_target.entity.and_then(|target| candidates.iter().find(|&&(e, ..)| e == target)).map(|&(_, p, _)| p)
        else {
            continue;
        };

        let to_target = target_translation - transform.translation;
        let horizontal = Vec3::new(to_target.x, 0.0, to_target.z);
        let distance = horizontal.length();

        if distance > ZOMBIE_LOD_DISTANCE {
            if distance > ZOMBIE_STOP_DISTANCE {
                let direction = horizontal / distance;
                transform.translation += direction * chase.speed * time.delta_secs();
                transform.look_to(-direction, Vec3::Y);
            }
            transform.translation.y = ZOMBIE_LOD_REST_HEIGHT;
            motion.grounded = true;
            motion.vertical_velocity = 0.0;
            motion.far = true;
            net_transform.translation = transform.translation;
            net_transform.yaw = transform.rotation.to_euler(EulerRot::YXZ).0;
            net_transform.moving = distance > ZOMBIE_STOP_DISTANCE;
            continue;
        }
        motion.far = false;

        let filter = SpatialQueryFilter::default();
        let horizontal_velocity = if distance > ZOMBIE_STOP_DISTANCE {
            motion.path_recalc_timer -= time.delta_secs();
            if motion.path_recalc_timer <= 0.0 {
                motion.path_recalc_timer = ZOMBIE_PATH_RECALC_INTERVAL;
                let desired_direction = navmesh
                    .and_then(|navmesh| navmesh.transformed_path(transform.translation, target_translation))
                    .and_then(|path| path.path.first().copied())
                    .map(|waypoint| {
                        Vec3::new(waypoint.x - transform.translation.x, 0.0, waypoint.z - transform.translation.z)
                            .normalize_or_zero()
                    })
                    .filter(|direction| *direction != Vec3::ZERO)
                    .unwrap_or(horizontal / distance);

                let mut separation = Vec3::ZERO;
                for (other_entity, other_pos) in &positions {
                    if *other_entity == entity {
                        continue;
                    }
                    let away = transform.translation - *other_pos;
                    let away = Vec3::new(away.x, 0.0, away.z);
                    let dist_sq = away.length_squared();
                    if dist_sq > 1e-6 && dist_sq < ZOMBIE_SEPARATION_RADIUS * ZOMBIE_SEPARATION_RADIUS {
                        let dist = dist_sq.sqrt();
                        separation += (away / dist) * (1.0 - dist / ZOMBIE_SEPARATION_RADIUS);
                    }
                }
                let desired_direction = (desired_direction + separation * ZOMBIE_SEPARATION_STRENGTH).normalize_or_zero();
                let desired_direction = if desired_direction == Vec3::ZERO { horizontal / distance } else { desired_direction };

                motion.cached_path_dir = steer_around_obstacles(&mover.spatial_query, transform.translation, desired_direction, distance, &filter);
            }
            motion.cached_path_dir * chase.speed
        } else {
            Vec3::ZERO
        };

        if motion.grounded && motion.vertical_velocity < 0.0 {
            motion.vertical_velocity = -1.0;
        }
        motion.vertical_velocity += GRAVITY * time.delta_secs();

        let decay = (KNOCKBACK_DECAY_RATE * time.delta_secs()).min(1.0);
        knockback.0 *= 1.0 - decay;

        let velocity = horizontal_velocity + knockback.0 + Vec3::Y * motion.vertical_velocity;
        let shape = Collider::capsule(ZOMBIE_RADIUS, ZOMBIE_LENGTH);
        let config = MoveAndSlideConfig::default();

        let mut grounded_this_frame = false;
        let output = mover.move_and_slide(
            &shape,
            transform.translation,
            transform.rotation,
            velocity,
            time.delta(),
            &config,
            &filter,
            |hit| {
                if hit.normal.y > GROUND_NORMAL_MIN_Y {
                    grounded_this_frame = true;
                }
                MoveAndSlideHitResponse::Accept
            },
        );

        transform.translation = output.position;
        motion.grounded = grounded_this_frame;
        motion.vertical_velocity = if grounded_this_frame { motion.vertical_velocity.min(0.0) } else { motion.vertical_velocity };

        if horizontal_velocity != Vec3::ZERO {
            transform.look_to(-horizontal_velocity, Vec3::Y);
        }

        net_transform.translation = transform.translation;
        net_transform.yaw = transform.rotation.to_euler(EulerRot::YXZ).0;
        net_transform.moving = horizontal_velocity != Vec3::ZERO;
    }
}

// Countdown timers for zombies killed by handle_fire_shot; tick_respawns
// spawns a replacement once each hits zero, keeping the population topped
// back up instead of thinning out permanently — the server-authoritative
// counterpart of what used to be the client's own PendingZombieRespawns.
#[derive(Resource, Default)]
struct PendingRespawns(Vec<f32>);

fn tick_respawns(
    time: Res<Time>,
    mut commands: Commands,
    mut respawns: ResMut<PendingRespawns>,
    mut next_spawn_index: Local<usize>,
) {
    let dt = time.delta_secs();
    let mut i = 0;
    let mut to_spawn = Vec::new();
    while i < respawns.0.len() {
        respawns.0[i] -= dt;
        if respawns.0[i] <= 0.0 {
            respawns.0.swap_remove(i);
            to_spawn.push(ZOMBIE_SPAWNS[*next_spawn_index % ZOMBIE_SPAWNS.len()]);
            *next_spawn_index += 1;
        } else {
            i += 1;
        }
    }
    if !to_spawn.is_empty() {
        spawn_zombies(&mut commands, &to_spawn, false);
    }
}

// Resolves a client's shot: raycasts against zombie capsules + arena
// geometry, applies damage/knockback/kill+respawn, and broadcasts the
// result (ShotFx) so every client plays the same hitmarker/blood/spark VFX
// at the same place — not just the shooter, and not based on the shooter's
// own (unauthoritative) guess of what happened.
fn handle_fire_shot(
    trigger: On<FromClient<FireShot>>,
    mut commands: Commands,
    spatial_query: SpatialQuery,
    mut zombies: Query<(&Transform, &mut Health, &mut KnockbackVelocity), With<Zombie>>,
    mut respawns: ResMut<PendingRespawns>,
    mut threats: Query<&mut PlayerThreat>,
    director_spawned: Query<(), With<DirectorSpawned>>,
) {
    // Gunfire is noise — it draws zombie attention even on a miss, so this
    // counts before the raycast below decides hit/miss.
    if let Some(shooter) = trigger.client_id.entity() {
        if let Ok(mut threat) = threats.get_mut(shooter) {
            threat.0 += PLAYER_THREAT_PER_SHOT;
        }
    }

    let shot = trigger.message;
    let Ok(direction) = Dir3::new(shot.direction) else {
        return;
    };
    let filter = SpatialQueryFilter::default();
    let Some(hit) = spatial_query.cast_ray(shot.origin, direction, MAX_SHOT_DISTANCE, true, &filter) else {
        return;
    };
    let hit_point = shot.origin + *direction * hit.distance;

    if let Ok((transform, mut health, mut knockback)) = zombies.get_mut(hit.entity) {
        let damage = zombie_hit_damage_geometric(hit_point, transform);
        health.0 -= damage;
        let killed = health.0 <= 0.0;
        if killed {
            let was_director_spawn = director_spawned.contains(hit.entity);
            commands.entity(hit.entity).despawn();
            if !was_director_spawn {
                respawns.0.push(ZOMBIE_RESPAWN_DELAY);
            }
        } else {
            let push = Vec3::new(shot.direction.x, 0.0, shot.direction.z).normalize_or_zero() * KNOCKBACK_FORCE;
            knockback.0 += push;
        }
        commands.server_trigger(ToClients {
            mode: SendMode::Broadcast,
            message: ShotFx::ZombieHit { point: hit_point, normal: hit.normal, damage, killed },
        });
    } else {
        commands.server_trigger(ToClients {
            mode: SendMode::Broadcast,
            message: ShotFx::SurfaceHit { point: hit_point, normal: hit.normal },
        });
    }
}

// Server-only bookkeeping — not replicated, clients only ever learn about
// game-over/restart via the broadcast GameStateEvent.
#[derive(Component)]
struct Dead;

// A client reports its own local Health hit 0 once (see the client's
// check_game_over) — this just marks that connection's player Dead and
// declares GameOver once every currently-connected player is.
fn on_player_died(
    trigger: On<FromClient<PlayerDied>>,
    mut commands: Commands,
    mut game_over: ResMut<GameOver>,
    connected: Query<Entity, With<NetPlayer>>,
    dead: Query<(), With<Dead>>,
) {
    let Some(entity) = trigger.client_id.entity() else {
        return;
    };
    commands.entity(entity).insert(Dead);

    if game_over.0 {
        return;
    }
    let all_dead = connected.iter().all(|e| e == entity || dead.contains(e));
    if all_dead {
        game_over.0 = true;
        commands.server_trigger(ToClients {
            mode: SendMode::Broadcast,
            message: GameStateEvent::GameOver,
        });
    }
}

// Any client can trigger this once the server has declared GameOver — despawns
// and respawns the zombie population, revives every connected player back at
// PLAYER_SPAWN, and broadcasts the reset so all clients (not just whoever
// pressed restart) clear their own GAME OVER state together.
fn on_request_restart(
    _trigger: On<FromClient<RequestRestart>>,
    mut commands: Commands,
    mut game_over: ResMut<GameOver>,
    mut respawns: ResMut<PendingRespawns>,
    mut director: ResMut<Director>,
    zombies: Query<Entity, With<Zombie>>,
    mut players: Query<(Entity, &mut Transform, &mut Player, &mut NetTransform), With<NetPlayer>>,
) {
    if !game_over.0 {
        return;
    }
    game_over.0 = false;
    respawns.0.clear();
    *director = Director::default();

    for entity in &zombies {
        commands.entity(entity).despawn();
    }
    spawn_zombies(&mut commands, &ZOMBIE_SPAWNS, false);

    for (entity, mut transform, mut player, mut net_transform) in &mut players {
        commands.entity(entity).remove::<Dead>();
        transform.translation = PLAYER_SPAWN;
        player.vertical_velocity = 0.0;
        player.grounded = false;
        net_transform.translation = PLAYER_SPAWN;
        net_transform.yaw = 0.0;
    }

    commands.server_trigger(ToClients {
        mode: SendMode::Broadcast,
        message: GameStateEvent::Restarted,
    });
}

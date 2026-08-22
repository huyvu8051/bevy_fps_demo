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
    zombie_hit_damage_geometric, Chase, ClientInput, FireShot, GameOver, GameStateEvent, Health,
    KnockbackVelocity, NetPlayer, NetTransform, Player, PlayerDied, PlayerStance, RequestRestart,
    ShotFx, SpawnMoreZombies, Zombie, ZombieAttackTimer, ZombieMotion, ARENA_HALF_SIZE,
    ARENA_WALL_HEIGHT, ARENA_WALL_THICKNESS, CROUCH_LENGTH, CROUCH_SPEED_MULTIPLIER, GRAVITY,
    GROUND_NORMAL_MIN_Y, JUMP_SPEED, KNOCKBACK_DECAY_RATE, KNOCKBACK_FORCE, MAX_SHOT_DISTANCE,
    MOVE_SPEED, NAVMESH_BOUNDS, NAVMESH_COLUMN_OBSTACLES, NAVMESH_HEIGHT, PLAYER_LENGTH,
    PLAYER_RADIUS, PLAYER_SPAWN, PRONE_LENGTH, PRONE_SPEED_MULTIPLIER, SPRINT_MULTIPLIER,
    ZOMBIE_AI_ENABLED, ZOMBIE_AVOID_ANGLES_DEG, ZOMBIE_AVOID_PROBE_DISTANCE, ZOMBIE_CHASE_RANGE,
    ZOMBIE_LENGTH, ZOMBIE_LOD_DISTANCE, ZOMBIE_LOD_REST_HEIGHT, ZOMBIE_MAX_HEALTH,
    ZOMBIE_PATH_RECALC_INTERVAL, ZOMBIE_RADIUS, ZOMBIE_RESPAWN_DELAY, ZOMBIE_SEPARATION_RADIUS,
    ZOMBIE_SEPARATION_STRENGTH, ZOMBIE_SPAWNS, ZOMBIE_SPEED, ZOMBIE_STOP_DISTANCE,
};
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
        .add_client_event::<ClientInput>(Channel::Unreliable)
        .add_client_event::<SpawnMoreZombies>(Channel::Unordered)
        .add_client_event::<FireShot>(Channel::Ordered)
        .add_server_event::<ShotFx>(Channel::Ordered)
        .add_client_event::<PlayerDied>(Channel::Ordered)
        .add_client_event::<RequestRestart>(Channel::Ordered)
        .add_server_event::<GameStateEvent>(Channel::Ordered)
        .init_resource::<TargetPlayer>()
        .init_resource::<PendingRespawns>()
        .init_resource::<GameOver>()
        .add_systems(Startup, (start_server, spawn_arena_colliders, spawn_zombie_navmesh, spawn_initial_zombies))
        .add_systems(
            PreUpdate,
            (
                chase_action_system.in_set(BigBrainSet::Actions),
                near_player_scorer.in_set(BigBrainSet::Scorers),
            ),
        )
        .add_systems(Update, (move_players, move_zombies, tick_respawns))
        .add_observer(spawn_player)
        .add_observer(log_client_disconnect)
        .add_observer(apply_client_input)
        .add_observer(spawn_more_zombies)
        .add_observer(on_player_died)
        .add_observer(on_request_restart)
        .add_observer(handle_fire_shot)
        .run();
}

// The single entity every zombie's Chase targets — currently "whichever
// player connected first" (matches the game's original single-player-era
// design, just extended to survive that player disconnecting). Picking the
// *nearest* player per-zombie instead is real future work for proper co-op,
// not needed to get zombie replication itself working.
#[derive(Resource, Default)]
struct TargetPlayer(Option<Entity>);

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
    let transport = NetcodeServerTransport::new(server_config, NativeSocket::new(socket).unwrap())
        .expect("failed to set up netcode server transport");

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
fn spawn_player(trigger: On<Add, ConnectedClient>, mut commands: Commands, mut target: ResMut<TargetPlayer>) {
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
        Replicated,
    ));
    if target.0.is_none() {
        target.0 = Some(entity);
    }
}

fn log_client_disconnect(trigger: On<Remove, ConnectedClient>, mut target: ResMut<TargetPlayer>) {
    let entity = trigger.event().entity;
    info!("client disconnected: {entity}");
    if target.0 == Some(entity) {
        target.0 = None;
    }
}

// ClientId::Client(Entity) IS the connection entity spawn_player already
// attached ClientInput to — no separate client-id-to-entity map needed.
fn apply_client_input(trigger: On<FromClient<ClientInput>>, mut inputs: Query<&mut ClientInput>) {
    let Some(entity) = trigger.client_id.entity() else {
        return;
    };
    if let Ok(mut input) = inputs.get_mut(entity) {
        *input = trigger.message;
    }
}

// Same MoveAndSlide-based movement as the client used to run locally (see
// git history), just driven by the latest received ClientInput instead of
// ButtonInput<KeyCode>/mouse/camera — this process is now the only one that
// actually decides where a player capsule ends up.
fn move_players(
    time: Res<Time>,
    mut players: Query<(&mut Transform, &mut Player, &PlayerStance, &ClientInput, &mut NetTransform)>,
    mover: MoveAndSlide,
) {
    for (mut transform, mut player, stance, input, mut net_transform) in &mut players {
        let move_speed = if input.aiming {
            MOVE_SPEED * shared::AIM_MOVE_SPEED_MULTIPLIER
        } else {
            match stance {
                PlayerStance::Standing if input.sprint => MOVE_SPEED * SPRINT_MULTIPLIER,
                PlayerStance::Standing => MOVE_SPEED,
                PlayerStance::Crouching => MOVE_SPEED * CROUCH_SPEED_MULTIPLIER,
                PlayerStance::Prone => MOVE_SPEED * PRONE_SPEED_MULTIPLIER,
            }
        };
        let capsule_length = match stance {
            PlayerStance::Standing => PLAYER_LENGTH,
            PlayerStance::Crouching => CROUCH_LENGTH,
            PlayerStance::Prone => PRONE_LENGTH,
        };

        let yaw_rotation = Quat::from_rotation_y(input.yaw);
        let forward = yaw_rotation * Vec3::NEG_Z;
        let right = yaw_rotation * Vec3::X;
        let mut horizontal = forward * input.move_axis.y + right * input.move_axis.x;
        if horizontal != Vec3::ZERO {
            horizontal = horizontal.normalize() * move_speed;
        }

        let mut vertical_velocity = player.vertical_velocity;
        if player.grounded {
            if input.jump {
                vertical_velocity = JUMP_SPEED;
            } else if vertical_velocity < 0.0 {
                vertical_velocity = -1.0;
            }
        }
        vertical_velocity += GRAVITY * time.delta_secs();

        let velocity = horizontal + Vec3::Y * vertical_velocity;
        let shape = Collider::capsule(PLAYER_RADIUS, capsule_length);
        let config = MoveAndSlideConfig::default();
        let filter = SpatialQueryFilter::default();

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
        player.grounded = grounded_this_frame;
        player.vertical_velocity = if grounded_this_frame {
            vertical_velocity.min(0.0)
        } else {
            vertical_velocity
        };

        net_transform.translation = transform.translation;
        net_transform.yaw = input.yaw;
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
    spawn_zombies(&mut commands, &ZOMBIE_SPAWNS);
}

fn spawn_more_zombies(_trigger: On<FromClient<SpawnMoreZombies>>, mut commands: Commands) {
    spawn_zombies(&mut commands, &ZOMBIE_SPAWNS);
}

fn spawn_zombies(commands: &mut Commands, spawns: &[Vec3]) {
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
    }
}

// Scorer: 1.0 when within ZOMBIE_CHASE_RANGE of the current TargetPlayer, 0
// if there's no target player connected yet.
#[derive(Clone, Component, Debug, ScorerBuilder)]
struct NearPlayer;

fn near_player_scorer(
    target: Res<TargetPlayer>,
    transforms: Query<&Transform>,
    mut query: Query<(&Actor, &mut Score), With<NearPlayer>>,
) {
    for (Actor(actor), mut score) in &mut query {
        let scored = target.0.and_then(|player| {
            let actor_transform = transforms.get(*actor).ok()?;
            let player_transform = transforms.get(player).ok()?;
            Some(actor_transform.translation.distance(player_transform.translation) <= ZOMBIE_CHASE_RANGE)
        });
        score.set(if scored == Some(true) { 1.0 } else { 0.0 });
    }
}

// Action: inserts Chase on Requested (entering the picked state), removes it
// on Cancelled. Stays Executing indefinitely otherwise — chasing has no
// "done" state, only "no longer in range".
#[derive(Clone, Component, Debug, ActionBuilder)]
struct ChaseAction;

fn chase_action_system(
    target: Res<TargetPlayer>,
    mut commands: Commands,
    mut query: Query<(&Actor, &mut ActionState, &ActionSpan)>,
) {
    for (Actor(actor), mut state, span) in &mut query {
        let _guard = span.span().enter();
        match *state {
            // try_insert/try_remove: the actor can already be despawned by
            // the time this command actually applies — same benign race the
            // single-player version worked around.
            ActionState::Requested => {
                let Some(player) = target.0 else {
                    continue;
                };
                commands.entity(*actor).try_insert(Chase {
                    target: player,
                    speed: ZOMBIE_SPEED,
                });
                *state = ActionState::Executing;
            }
            ActionState::Cancelled => {
                commands.entity(*actor).try_remove::<Chase>();
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
    mut zombies: Query<(Entity, &mut Transform, &mut ZombieMotion, &Chase, &mut KnockbackVelocity, &mut NetTransform), With<Zombie>>,
    targets: Query<&Transform, Without<Zombie>>,
    navmesh: Query<&ManagedNavMesh>,
    navmeshes: Res<Assets<NavMesh>>,
    mover: MoveAndSlide,
) {
    let navmesh = navmesh.single().ok().and_then(|handle| navmeshes.get(handle));

    let positions: Vec<(Entity, Vec3)> = zombies.iter().map(|(e, t, ..)| (e, t.translation)).collect();

    for (entity, mut transform, mut motion, chase, mut knockback, mut net_transform) in &mut zombies {
        let Ok(target_transform) = targets.get(chase.target) else {
            continue;
        };

        let to_target = target_transform.translation - transform.translation;
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
                    .and_then(|navmesh| navmesh.transformed_path(transform.translation, target_transform.translation))
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
        spawn_zombies(&mut commands, &to_spawn);
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
) {
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
            commands.entity(hit.entity).despawn();
            respawns.0.push(ZOMBIE_RESPAWN_DELAY);
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
    zombies: Query<Entity, With<Zombie>>,
    mut players: Query<(Entity, &mut Transform, &mut Player, &mut NetTransform), With<NetPlayer>>,
) {
    if !game_over.0 {
        return;
    }
    game_over.0 = false;
    respawns.0.clear();

    for entity in &zombies {
        commands.entity(entity).despawn();
    }
    spawn_zombies(&mut commands, &ZOMBIE_SPAWNS);

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

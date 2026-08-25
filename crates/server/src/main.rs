use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::time::SystemTime;

use avian3d::prelude::*;
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
    zombie_hit_damage_geometric, Chase, ClearAllZombies, ClientInput, DebugBot, DespawnDebugBot, FireShot,
    GameOver, GameStateEvent, Health, KnockbackVelocity, LastProcessedInput, MoveDebugBot, NetPlayer,
    NetTransform, Player, PlayerDied, PlayerDiedFx, PlayerFiredFx, PlayerStance, RequestRestart,
    SetAutoMaintainZombies, SetZombieCount,
    ShotFx, SpawnDebugBot, SpawnMoreZombies, Zombie,
    ZombieAttackTimer, ZombieMotion, ARENA_HALF_SIZE, ARENA_WALL_HEIGHT, ARENA_WALL_THICKNESS,
    DEBUG_BOT_DETECT_RANGE, DEBUG_BOT_ENGAGE_RANGE, DEBUG_BOT_EYE_HEIGHT, DEBUG_BOT_FIRE_INTERVAL_SECS,
    DEBUG_BOT_RETARGET_INTERVAL, GRAVITY, GROUND_NORMAL_MIN_Y, KNOCKBACK_DECAY_RATE, KNOCKBACK_FORCE,
    MAX_SHOT_DISTANCE, MOVE_SPEED, NAVMESH_BOUNDS, NAVMESH_COLUMN_OBSTACLES, NAVMESH_HEIGHT, PLAYER_LENGTH,
    PLAYER_RADIUS, PLAYER_SPAWN,
    ZOMBIE_AI_ENABLED, ZOMBIE_AVOID_ANGLES_DEG, ZOMBIE_AVOID_PROBE_DISTANCE, ZOMBIE_CHASE_RANGE,
    ZOMBIE_LENGTH, ZOMBIE_LOD_DISTANCE, ZOMBIE_LOD_REST_HEIGHT, ZOMBIE_MAX_HEALTH, ZOMBIE_PATH_RECALC_INTERVAL,
    DIRECTOR_CALM_DURATION_SECS, DIRECTOR_CALM_PRESSURE_THRESHOLD, DIRECTOR_HORDE_SIZE,
    DIRECTOR_HORDE_SPAWN_RADIUS, DIRECTOR_MELEE_RANGE, DIRECTOR_PRESSURE_SMOOTHING, INTERP_DELAY_SECS,
    LAG_COMPENSATION_MAX_SECS, PLAYER_THREAT_DECAY_RATE, PLAYER_THREAT_PER_SHOT, ZOMBIE_LOAD_BALANCE_PENALTY,
    ZOMBIE_RADIUS, ZOMBIE_RESPAWN_DELAY, ZOMBIE_RETARGET_INTERVAL, ZOMBIE_SEPARATION_RADIUS,
    ZOMBIE_SEPARATION_STRENGTH, ZOMBIE_SPAWNS, ZOMBIE_SPEED, ZOMBIE_STOP_DISTANCE, ZOMBIE_THREAT_DISTANCE_BONUS,
    ZOMBIE_TURN_RATE_DEG,
};
use shared::sim_latency::{sim_latency_from_env, LaggedSocket};
use vleue_navigator::prelude::*;

// Fixed debug port — the client connects here by default (see SERVER_ADDR
// in crates/client). No auth/matchmaking: ServerAuthentication::Unsecure
// and a large max_clients stand in for "no player limit" per the plan.
const PORT: u16 = 5031;
const PROTOCOL_ID: u64 = 0;
const MAX_CLIENTS: usize = 256;

// MinimalPlugins' default ScheduleRunnerPlugin loops as fast as the CPU
// allows (RunMode::Loop { wait: None }) — meaning move_players/move_zombies
// ran with a tiny, unstable dt every iteration, different from whatever dt
// the client happened to predict with for the "same" input. That dt
// mismatch was a real source of the client's jump-reconciliation drift
// (see predict_player_movement's own doc comment for the other half of
// that fix). Capping the server to a fixed tick gives both sides a
// consistent, comparable dt. 30Hz: plenty for a hitscan zombie co-op (no
// twitch-competitive aiming needs), and it's not render-rate-bound like the
// client, so a lower tick than 60 is fine and cheaper.
const SERVER_TICK_HZ: f64 = 30.0;

fn main() {
    App::new()
        .add_plugins((
            MinimalPlugins.set(bevy::app::ScheduleRunnerPlugin::run_loop(std::time::Duration::from_secs_f64(
                1.0 / SERVER_TICK_HZ,
            ))),
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
        .add_server_event::<PlayerFiredFx>(Channel::Unordered)
        .add_server_event::<PlayerDiedFx>(Channel::Ordered)
        .add_client_event::<PlayerDied>(Channel::Ordered)
        .add_client_event::<RequestRestart>(Channel::Ordered)
        .add_server_event::<GameStateEvent>(Channel::Ordered)
        .add_client_event::<SpawnDebugBot>(Channel::Ordered)
        .add_client_event::<MoveDebugBot>(Channel::Ordered)
        .add_client_event::<DespawnDebugBot>(Channel::Ordered)
        .add_client_event::<SetZombieCount>(Channel::Ordered)
        .add_client_event::<ClearAllZombies>(Channel::Ordered)
        .add_client_event::<SetAutoMaintainZombies>(Channel::Ordered)
        .init_resource::<PendingRespawns>()
        .init_resource::<GameOver>()
        .init_resource::<DebugBotIdCounter>()
        .init_resource::<Director>()
        .init_resource::<ZombieTargetCount>()
        .init_resource::<AutoMaintainZombies>()
        .init_resource::<NextZombieSpawnIndex>()
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
            (
                move_players,
                (move_zombies, record_zombie_history).chain(),
                (bot_pick_target, move_debug_bots, debug_bot_fire).chain(),
                tick_respawns,
                decay_player_threat,
                // TEMPORARILY disabled — was stacking more hordes on top of
                // the cheat panel's zombie count target every ~20s of calm,
                // fighting with it. Re-add once ZombieTargetCount/director
                // hordes are reconciled to coexist properly.
                // run_ai_director,
            ),
        )
        .add_observer(spawn_player)
        .add_observer(log_client_disconnect)
        .add_observer(apply_client_input)
        .add_observer(spawn_more_zombies)
        .add_observer(on_set_zombie_count)
        .add_observer(on_clear_all_zombies)
        .add_observer(on_set_auto_maintain_zombies)
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
            sprinting: false,
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
    // Y is the zombie's own resting height (capsule center when grounded),
    // not the player's — center.y is the PLAYER's capsule height, which is
    // much lower than the zombie's now that ZOMBIE_RADIUS/ZOMBIE_LENGTH
    // scale with ZOMBIE_SCALE. Reusing it here buried every director-spawned
    // horde well below ground on spawn (same bug ZOMBIE_SPAWNS/
    // ZOMBIE_LOD_REST_HEIGHT already hit — see their own comments).
    let spawn_y = ZOMBIE_RADIUS + ZOMBIE_LENGTH / 2.0;
    let spawns: Vec<Vec3> = (0..DIRECTOR_HORDE_SIZE)
        .map(|i| {
            let angle = i as f32 / DIRECTOR_HORDE_SIZE as f32 * std::f32::consts::TAU;
            Vec3::new(center.x + angle.cos() * DIRECTOR_HORDE_SPAWN_RADIUS, spawn_y, center.z + angle.sin() * DIRECTOR_HORDE_SPAWN_RADIUS)
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
        // Never set before — the client's own walk/idle animation for OTHER
        // players (and bots) reads this exactly like it already does for
        // zombies, but for players it just stayed false forever, so remote
        // player models never animated.
        net_transform.moving = input.move_axis != Vec2::ZERO;
        net_transform.sprinting = input.sprint;
    }
}

// --- Debug bots ---
// Fake NetPlayer stand-ins spawned from the client's debug panel (see
// crates/client) so multi-player zombie targeting can be tested without a
// second real client. No Player/ClientInput/physics — they're kinematic,
// driven entirely by these three chained systems (bot_pick_target ->
// move_debug_bots -> debug_bot_fire) instead of real input. They now hunt on
// their own: whenever a zombie is within DEBUG_BOT_DETECT_RANGE, a bot walks
// into DEBUG_BOT_ENGAGE_RANGE and fires on it, reusing the exact same
// damage/knockback path handle_fire_shot uses for real players (just without
// lag compensation — a bot has no network latency to compensate for).
// MoveDebugBot (the "9" debug-panel key) still writes DebugBotTarget
// directly, but bot_pick_target overwrites it again next tick as soon as a
// zombie is in range — so manual placement is really only useful for
// positioning a bot before/between fights, not for pulling one out of one.

#[derive(Resource, Default)]
struct DebugBotIdCounter(u32);

#[derive(Component)]
struct DebugBotTarget(Vec3);

// Server-only, mirrors ZombieMotion — bots previously had no gravity or
// ground collision at all (move_debug_bots just teleported translation.x/z
// toward DebugBotTarget, never touching .y), so a bot spawned off the
// ground (e.g. via noclip) hung in the air forever instead of falling, and
// since bot_pick_target's distance check is full 3D, hanging far above the
// arena could also put every zombie out of DEBUG_BOT_DETECT_RANGE, making
// the bot look completely inert rather than just floating.
#[derive(Component, Default)]
struct DebugBotMotion {
    vertical_velocity: f32,
    grounded: bool,
}

// Server-only (not replicated) — which zombie a bot is currently hunting,
// plus its own retarget/fire cooldowns. Mirrors ChaseTarget's
// commit-and-periodically-reevaluate pattern for the same reason (flicker
// avoidance when two zombies are similarly close).
#[derive(Component, Default)]
struct DebugBotAi {
    target_zombie: Option<Entity>,
    retarget_timer: f32,
    fire_timer: f32,
}

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
            sprinting: false,
        },
        DebugBot(id),
        DebugBotTarget(at),
        DebugBotAi::default(),
        DebugBotMotion::default(),
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

// Nearest-zombie-in-range targeting, committed to like ChaseTarget rather
// than re-picked every frame (see DebugBotAi's own doc comment). Writes the
// chosen zombie's live position into DebugBotTarget so move_debug_bots'
// walk-toward-target logic below doesn't need to know about zombies at all.
fn bot_pick_target(
    time: Res<Time>,
    mut bots: Query<(Entity, &Transform, &mut DebugBotAi, &mut DebugBotTarget), With<DebugBot>>,
    zombies: Query<(Entity, &Transform), (With<Zombie>, Without<DebugBot>)>,
) {
    for (entity, transform, mut ai, mut manual_target) in &mut bots {
        ai.retarget_timer -= time.delta_secs();
        let target_alive = ai.target_zombie.is_some_and(|target| zombies.contains(target));
        if !target_alive || ai.retarget_timer <= 0.0 {
            ai.target_zombie = zombies
                .iter()
                .map(|(zombie, zombie_transform)| (zombie, transform.translation.distance(zombie_transform.translation)))
                .filter(|&(_, distance)| distance <= DEBUG_BOT_DETECT_RANGE)
                .min_by(|(_, a), (_, b)| a.total_cmp(b))
                .map(|(zombie, _)| zombie);
            // Staggered like ZOMBIE_RETARGET_INTERVAL — see entity_phase's
            // own doc comment for why a flat reset would let bots' retarget
            // checks drift back into lockstep.
            ai.retarget_timer = DEBUG_BOT_RETARGET_INTERVAL * (0.75 + entity_phase(entity) * 0.5);
        }
        if let Some(target) = ai.target_zombie {
            if let Ok((_, zombie_transform)) = zombies.get(target) {
                manual_target.0 = zombie_transform.translation;
            }
        }
    }
}

fn move_debug_bots(
    time: Res<Time>,
    mut bots: Query<
        (&mut Transform, &mut NetTransform, &DebugBotTarget, &DebugBotAi, &mut DebugBotMotion),
        With<DebugBot>,
    >,
    mover: MoveAndSlide,
) {
    let filter = SpatialQueryFilter::default();
    for (mut transform, mut net_transform, target, ai, mut motion) in &mut bots {
        // Stop at engage range when hunting, so debug_bot_fire has room to
        // shoot instead of walking the bot into melee — otherwise stop
        // right on top of a manually-placed DebugBotTarget as before.
        let stop_distance = if ai.target_zombie.is_some() { DEBUG_BOT_ENGAGE_RANGE } else { 0.05 };
        let to_target = target.0 - transform.translation;
        let horizontal = Vec3::new(to_target.x, 0.0, to_target.z);
        let distance = horizontal.length();
        let moving = distance > stop_distance;
        net_transform.moving = moving;
        let horizontal_velocity = if moving { (horizontal / distance) * MOVE_SPEED } else { Vec3::ZERO };

        // Gravity + ground collision, same shape/response as move_zombies —
        // without this a bot spawned off the ground (or one whose target
        // happens to be at a different height) never comes back down, it
        // just teleports its x/z toward the target while y sits wherever it
        // was last placed.
        if motion.grounded && motion.vertical_velocity < 0.0 {
            motion.vertical_velocity = -1.0;
        }
        motion.vertical_velocity += GRAVITY * time.delta_secs();

        let velocity = horizontal_velocity + Vec3::Y * motion.vertical_velocity;
        let shape = Collider::capsule(PLAYER_RADIUS, PLAYER_LENGTH);
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
            let target_rotation = Transform::default().looking_to(-horizontal_velocity, Vec3::Y).rotation;
            turn_towards(&mut transform.rotation, target_rotation, ZOMBIE_TURN_RATE_DEG.to_radians() * time.delta_secs());
        } else if ai.target_zombie.is_some() && distance > 0.05 {
            // Already in engage range but not facing the target (e.g. it
            // just moved) — keep aiming at it even while standing still, so
            // debug_bot_fire's raycast actually points at the zombie.
            let direction = horizontal / distance;
            let target_rotation = Transform::default().looking_to(-direction, Vec3::Y).rotation;
            turn_towards(&mut transform.rotation, target_rotation, ZOMBIE_TURN_RATE_DEG.to_radians() * time.delta_secs());
        }

        net_transform.translation = transform.translation;
        net_transform.yaw = transform.rotation.to_euler(EulerRot::YXZ).0;
    }
}

// Fires at whatever bot_pick_target committed to, once in range and off
// cooldown. No lag compensation and no ZombieHistory rewind (unlike
// handle_fire_shot) — a bot fires against the zombie's live position since
// it has no client round-trip to compensate for.
fn debug_bot_fire(
    mut commands: Commands,
    time: Res<Time>,
    mut bots: Query<(&Transform, &mut DebugBotAi), With<DebugBot>>,
    mut zombies: Query<(Entity, &Transform, &mut Health, &mut KnockbackVelocity), (With<Zombie>, Without<DebugBot>)>,
    mut respawns: ResMut<PendingRespawns>,
    director_spawned: Query<(), With<DirectorSpawned>>,
) {
    for (transform, mut ai) in &mut bots {
        ai.fire_timer -= time.delta_secs();
        let Some(target) = ai.target_zombie else { continue };
        let Ok((entity, zombie_transform, mut health, mut knockback)) = zombies.get_mut(target) else {
            continue;
        };

        let origin = transform.translation + Vec3::Y * DEBUG_BOT_EYE_HEIGHT;
        // Small slack over DEBUG_BOT_ENGAGE_RANGE: move_debug_bots stops the
        // bot at that range measured to the zombie's translation, but the
        // shot origin is offset up by DEBUG_BOT_EYE_HEIGHT, so the straight-
        // line distance to the zombie's capsule center is very slightly
        // larger — without slack the bot would arrive and then wait one
        // extra retarget cycle before its first shot ever counts as "in range".
        if origin.distance(zombie_transform.translation) > DEBUG_BOT_ENGAGE_RANGE + 1.0 || ai.fire_timer > 0.0 {
            continue;
        }
        ai.fire_timer = DEBUG_BOT_FIRE_INTERVAL_SECS;

        let Ok(direction) = Dir3::new(zombie_transform.translation - origin) else {
            continue;
        };
        let capsule = Collider::capsule(ZOMBIE_RADIUS, ZOMBIE_LENGTH);
        let Some((hit_distance, normal)) = capsule.cast_ray(
            zombie_transform.translation,
            zombie_transform.rotation,
            origin,
            *direction,
            MAX_SHOT_DISTANCE,
            true,
        ) else {
            continue;
        };

        let hit_point = origin + *direction * hit_distance;
        let damage = zombie_hit_damage_geometric(hit_point, zombie_transform);
        health.0 -= damage;
        let killed = health.0 <= 0.0;
        if killed {
            let was_director_spawn = director_spawned.contains(entity);
            commands.entity(entity).despawn();
            if !was_director_spawn {
                respawns.0.push(ZOMBIE_RESPAWN_DELAY);
            }
        } else {
            let push = Vec3::new(zombie_transform.translation.x - origin.x, 0.0, zombie_transform.translation.z - origin.z)
                .normalize_or_zero()
                * KNOCKBACK_FORCE;
            knockback.0 += push;
        }
        // No shooter to exclude (a bot has no client of its own predicting
        // this locally) — full Broadcast, unlike handle_fire_shot's
        // BroadcastExcept.
        commands.server_trigger(ToClients {
            mode: SendMode::Broadcast,
            message: ShotFx::ZombieHit { point: hit_point, normal, damage, killed },
        });
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

// Mobile cheat panel's zombie population target (crates/client's
// ZombieCountUi mirrors this locally so the UI has something to show/adjust
// without waiting on a round-trip). 10, not ZOMBIE_SPAWNS.len() (100) — set
// low for on-device perf testing; bump via the cheat panel's +/- buttons.
#[derive(Resource)]
struct ZombieTargetCount(u32);

impl Default for ZombieTargetCount {
    fn default() -> Self {
        Self(10)
    }
}

// Gates whether tick_respawns/maintain_zombie_population actually replace
// killed baseline zombies — off lets the population drain down through
// normal play instead of staying pinned at ZombieTargetCount.
#[derive(Resource)]
struct AutoMaintainZombies(bool);

impl Default for AutoMaintainZombies {
    fn default() -> Self {
        Self(true)
    }
}

// Shared with tick_respawns (was a Local there before this needed to be
// reused by maintain_zombie_population too) — which ZOMBIE_SPAWNS slot the
// next new baseline zombie gets, cycling regardless of which system
// actually does the spawning.
#[derive(Resource, Default)]
struct NextZombieSpawnIndex(usize);

fn spawn_initial_zombies(mut commands: Commands, target: Res<ZombieTargetCount>, mut next_spawn_index: ResMut<NextZombieSpawnIndex>) {
    let count = (target.0 as usize).min(ZOMBIE_SPAWNS.len());
    spawn_zombies(&mut commands, &ZOMBIE_SPAWNS[..count], false);
    next_spawn_index.0 = count;
}

// Reconciles the live baseline (non-director) zombie count to
// ZombieTargetCount whenever it changes (see on_set_zombie_count) — spawns
// more at the next cycling ZOMBIE_SPAWNS slots if under target, despawns
// arbitrary baseline zombies (whichever the query happens to iterate first,
// no particular selection logic — this is a debug tool, not gameplay) if
// over.
fn on_set_zombie_count(
    trigger: On<FromClient<SetZombieCount>>,
    mut commands: Commands,
    mut target: ResMut<ZombieTargetCount>,
    mut next_spawn_index: ResMut<NextZombieSpawnIndex>,
    baseline_zombies: Query<Entity, (With<Zombie>, Without<DirectorSpawned>)>,
) {
    target.0 = trigger.message.0.min(ZOMBIE_SPAWNS.len() as u32);
    let current = baseline_zombies.iter().count() as u32;
    if current < target.0 {
        let to_spawn: Vec<Vec3> = (0..target.0 - current)
            .map(|_| {
                let pos = ZOMBIE_SPAWNS[next_spawn_index.0 % ZOMBIE_SPAWNS.len()];
                next_spawn_index.0 += 1;
                pos
            })
            .collect();
        spawn_zombies(&mut commands, &to_spawn, false);
    } else if current > target.0 {
        for entity in baseline_zombies.iter().take((current - target.0) as usize) {
            commands.entity(entity).try_despawn();
        }
    }
}

// Authoritative clear — despawns every zombie (baseline AND
// director-spawned) and drops pending respawns, unlike the old desktop-only
// debug panel's Digit2 action (which only ever despawned the CLIENT's own
// local replicated copy, so it silently came back on the next replication
// update).
fn on_clear_all_zombies(
    _trigger: On<FromClient<ClearAllZombies>>,
    mut commands: Commands,
    all_zombies: Query<Entity, With<Zombie>>,
    mut respawns: ResMut<PendingRespawns>,
) {
    for entity in &all_zombies {
        commands.entity(entity).try_despawn();
    }
    respawns.0.clear();
}

fn on_set_auto_maintain_zombies(trigger: On<FromClient<SetAutoMaintainZombies>>, mut auto_maintain: ResMut<AutoMaintainZombies>) {
    auto_maintain.0 = trigger.message.0;
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

// Recent (translation, rotation) snapshots for lag-compensated hit
// detection — see handle_fire_shot and record_zombie_history. Not
// replicated (server-only bookkeeping, like ChaseTarget/PlayerThreat).
#[derive(Component, Default)]
struct ZombieHistory(std::collections::VecDeque<(f32, Vec3, Quat)>);

// Runs after move_zombies each tick: records where every zombie is right
// now, and prunes anything older than LAG_COMPENSATION_MAX_SECS needs.
fn record_zombie_history(time: Res<Time>, mut zombies: Query<(&Transform, &mut ZombieHistory), With<Zombie>>) {
    let now = time.elapsed_secs();
    for (transform, mut history) in &mut zombies {
        history.0.push_back((now, transform.translation, transform.rotation));
        while history.0.front().is_some_and(|&(t, ..)| now - t > LAG_COMPENSATION_MAX_SECS) {
            history.0.pop_front();
        }
    }
}

// Nearest recorded snapshot to `target_time` — history is only sampled once
// per server tick (30Hz), so this is a "closest sample" lookup rather than
// interpolating between two, which is plenty precise for what's ultimately
// a cosmetic-accuracy-tolerant debug shooter.
fn zombie_history_at(history: &ZombieHistory, target_time: f32) -> Option<(Vec3, Quat)> {
    history
        .0
        .iter()
        .min_by(|(a, ..), (b, ..)| (a - target_time).abs().total_cmp(&(b - target_time).abs()))
        .map(|&(_, translation, rotation)| (translation, rotation))
}

// Fractional part of entity_index * the golden ratio — a cheap
// low-discrepancy sequence that spreads out well across however many
// zombies exist, regardless of spawn order or position. Used to stagger
// per-zombie timers (path_recalc_timer here, ChaseTarget's
// reevaluate_timer in move_zombies) so 100 zombies spawned in the same
// frame don't all recompute on the exact same later frame forever after.
// The previous approach hashed spawn_pos itself (sin(x*12.9898+z*78.233)),
// but ZOMBIE_SPAWNS is a regular 10x10 grid — only 10 distinct x/z values —
// and that specific hash constant is known to alias badly on grid-aligned
// input, so many zombies ended up with near-identical phase anyway
// (visible as the whole horde recalculating, and re-steering, in
// lockstep). Keying off entity index instead has no such structure to
// alias against.
fn entity_phase(entity: Entity) -> f32 {
    (entity.index().index() as f32 * 0.618_034).fract()
}

fn spawn_zombies(commands: &mut Commands, spawns: &[Vec3], director_spawned: bool) {
    for &spawn_pos in spawns {
        let mut zombie = commands.spawn_empty();
        let phase = entity_phase(zombie.id());
        zombie.insert((
            Transform::from_translation(spawn_pos),
            Zombie,
            ZombieMotion {
                path_recalc_timer: phase * ZOMBIE_PATH_RECALC_INTERVAL,
                ..default()
            },
            KnockbackVelocity::default(),
            ZombieAttackTimer::default(),
            NetTransform {
                translation: spawn_pos,
                yaw: 0.0,
                moving: false,
                sprinting: false,
            },
            Health(ZOMBIE_MAX_HEALTH),
            Replicated,
            // Raycast-only: no RigidBody, matching how this worked
            // client-side before zombies moved server-side — avian3d's
            // spatial queries pick up a bare Collider fine.
            Collider::capsule(ZOMBIE_RADIUS, ZOMBIE_LENGTH),
            ZombieHistory::default(),
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

// Rotates `rotation` toward `target` by at most `max_angle` radians this
// call, instead of snapping straight to it — see ZOMBIE_TURN_RATE_DEG.
// slerp's `t` fraction is derived from the actual angle between the two
// (not a flat lerp factor), so the per-frame angular step stays constant
// regardless of how far off `target` currently is.
fn turn_towards(rotation: &mut Quat, target: Quat, max_angle: f32) {
    let angle = rotation.angle_between(target);
    if angle <= max_angle || angle <= f32::EPSILON {
        *rotation = target;
    } else {
        *rotation = rotation.slerp(target, max_angle / angle);
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
            // Staggered per-zombie (0.75x..1.25x) instead of a flat
            // interval — see entity_phase's own doc comment for why a flat
            // reset here would let every zombie's retarget checks drift
            // back into lockstep even if they started out spread apart.
            chase_target.reevaluate_timer = ZOMBIE_RETARGET_INTERVAL * (0.75 + entity_phase(entity) * 0.5);
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
                let target_rotation = Transform::default().looking_to(-direction, Vec3::Y).rotation;
                turn_towards(&mut transform.rotation, target_rotation, ZOMBIE_TURN_RATE_DEG.to_radians() * time.delta_secs());
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
            let target_rotation = Transform::default().looking_to(-horizontal_velocity, Vec3::Y).rotation;
            turn_towards(&mut transform.rotation, target_rotation, ZOMBIE_TURN_RATE_DEG.to_radians() * time.delta_secs());
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
    mut next_spawn_index: ResMut<NextZombieSpawnIndex>,
    auto_maintain: Res<AutoMaintainZombies>,
) {
    let dt = time.delta_secs();
    let mut i = 0;
    let mut to_spawn = Vec::new();
    while i < respawns.0.len() {
        respawns.0[i] -= dt;
        if respawns.0[i] <= 0.0 {
            respawns.0.swap_remove(i);
            // Still consumes the pending-respawn slot either way — only
            // whether it actually results in a new zombie depends on the
            // toggle, so flipping auto-maintain back on later doesn't
            // suddenly spawn a backlog of everything killed while it was off.
            if auto_maintain.0 {
                to_spawn.push(ZOMBIE_SPAWNS[next_spawn_index.0 % ZOMBIE_SPAWNS.len()]);
                next_spawn_index.0 += 1;
            }
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
    time: Res<Time>,
    renet_server: Res<RenetServer>,
    network_ids: Query<&NetworkId>,
    spatial_query: SpatialQuery,
    mut zombies: Query<(Entity, &mut Health, &mut KnockbackVelocity, &ZombieHistory), With<Zombie>>,
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
        // Broadcast regardless of hit/miss (unlike ShotFx below, which only
        // fires once the raycast/lag-compensation result is known) — every
        // OTHER client plays a brief Gun_Shoot pose on this shooter's own
        // remote-player model (see crate::client's
        // on_player_fired_fx/update_remote_player_model_animation). The
        // shooter's own client already knows it fired locally, so this is
        // purely for everyone else.
        if let Ok(network_id) = network_ids.get(shooter) {
            commands.server_trigger(ToClients {
                mode: SendMode::Broadcast,
                message: PlayerFiredFx { shooter_network_id: network_id.get() },
            });
        }
    }

    let shot = trigger.message;
    let Ok(direction) = Dir3::new(shot.direction) else {
        return;
    };

    let filter = SpatialQueryFilter::default();
    // Live world raycast — only used as the SurfaceHit fallback if no
    // zombie ends up qualifying below. NOT used to bound the rewound
    // search distance (see the bug this fixed, below).
    let live_hit = spatial_query.cast_ray(shot.origin, direction, MAX_SHOT_DISTANCE, true, &filter);

    // Lag compensation: rewind to roughly what the shooter's own screen
    // showed when they fired (their network latency + the client's own
    // render-side interpolation delay — see INTERP_DELAY_SECS) and test
    // the ray against each zombie's capsule at ITS position back then,
    // instead of live. Without this, a shot that looked like a clean hit
    // on a laggy client's screen could whiff here just because the zombie
    // has since moved on — same idea Source engine's server-side lag
    // compensation uses for L4D.
    let rtt = trigger
        .client_id
        .entity()
        .and_then(|entity| network_ids.get(entity).ok())
        .map_or(0.0, |network_id| renet_server.rtt(network_id.get()));
    let rewind_secs = (rtt as f32 / 2.0 + INTERP_DELAY_SECS).min(LAG_COMPENSATION_MAX_SECS);
    let rewind_time = time.elapsed_secs() - rewind_secs;

    // Search bound is MAX_SHOT_DISTANCE, not the live raycast's hit
    // distance — zombies are always chasing (moving toward the shooter),
    // so their rewound (past) position is essentially always *farther*
    // away than their live position. Bounding the search by the live hit
    // distance meant the rewound test could never actually reach the
    // zombie's rewound position — it was always just out of range,
    // silently failing almost every shot except when the player was
    // closing distance fast enough to offset it. That's what was showing
    // up as "damage applies rarely, knockback only when walking into the
    // zombie while firing".
    let capsule = Collider::capsule(ZOMBIE_RADIUS, ZOMBIE_LENGTH);
    let rewound_hit = zombies
        .iter()
        .filter_map(|(entity, _, _, history)| {
            let (position, rotation) = zombie_history_at(history, rewind_time)?;
            let (distance, normal) = capsule.cast_ray(position, rotation, shot.origin, *direction, MAX_SHOT_DISTANCE, true)?;
            Some((distance, entity, position, rotation, normal))
        })
        .min_by(|(a, ..), (b, ..)| a.total_cmp(b));

    if let Some((distance, entity, position, rotation, normal)) = rewound_hit {
        let hit_point = shot.origin + *direction * distance;
        if let Ok((_, mut health, mut knockback, _)) = zombies.get_mut(entity) {
            let rewound_transform = Transform {
                translation: position,
                rotation,
                scale: Vec3::ONE,
            };
            let damage = zombie_hit_damage_geometric(hit_point, &rewound_transform);
            health.0 -= damage;
            let killed = health.0 <= 0.0;
            if killed {
                let was_director_spawn = director_spawned.contains(entity);
                commands.entity(entity).despawn();
                if !was_director_spawn {
                    respawns.0.push(ZOMBIE_RESPAWN_DELAY);
                }
            } else {
                let push = Vec3::new(shot.direction.x, 0.0, shot.direction.z).normalize_or_zero() * KNOCKBACK_FORCE;
                knockback.0 += push;
            }
            // Excludes the shooter, not a plain Broadcast: their own
            // client already predicted and showed this same blood/
            // hitmarker/damage-number the instant they fired (see the
            // client's shoot()), so echoing it back would just double it
            // up. Same "weapon prediction" approach Source engine uses for
            // L4D. SurfaceHit isn't predicted client-side, so that one
            // stays a full Broadcast.
            commands.server_trigger(ToClients {
                mode: SendMode::BroadcastExcept(trigger.client_id),
                message: ShotFx::ZombieHit { point: hit_point, normal, damage, killed },
            });
        }
        return;
    }

    let Some(hit) = live_hit else {
        return;
    };
    let hit_point = shot.origin + *direction * hit.distance;

    // Fallback: the rewound test above found nothing (e.g. a zombie with
    // no history yet, or a genuine miss), but the live world raycast still
    // landed directly on a zombie right now — treat that as an ordinary,
    // un-compensated hit instead of silently treating a zombie as a wall.
    if let Ok((entity, mut health, mut knockback, history)) = zombies.get_mut(hit.entity) {
        let (position, rotation) = zombie_history_at(history, time.elapsed_secs()).unwrap_or((hit_point, Quat::IDENTITY));
        let live_transform = Transform {
            translation: position,
            rotation,
            scale: Vec3::ONE,
        };
        let damage = zombie_hit_damage_geometric(hit_point, &live_transform);
        health.0 -= damage;
        let killed = health.0 <= 0.0;
        if killed {
            let was_director_spawn = director_spawned.contains(entity);
            commands.entity(entity).despawn();
            if !was_director_spawn {
                respawns.0.push(ZOMBIE_RESPAWN_DELAY);
            }
        } else {
            let push = Vec3::new(shot.direction.x, 0.0, shot.direction.z).normalize_or_zero() * KNOCKBACK_FORCE;
            knockback.0 += push;
        }
        commands.server_trigger(ToClients {
            mode: SendMode::BroadcastExcept(trigger.client_id),
            message: ShotFx::ZombieHit { point: hit_point, normal: hit.normal, damage, killed },
        });
        return;
    }

    commands.server_trigger(ToClients {
        mode: SendMode::Broadcast,
        message: ShotFx::SurfaceHit { point: hit_point, normal: hit.normal },
    });
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
    // Without<DebugBot>: bots are fake NetPlayer stand-ins for testing (see
    // spawn_debug_bot) that never receive Dead — counting them here would
    // permanently block all_dead (and therefore Restart, gated on
    // game_over.0 below) for the lifetime of the server if even one bot was
    // ever spawned and left connected.
    connected: Query<Entity, (With<NetPlayer>, Without<DebugBot>)>,
    dead: Query<(), With<Dead>>,
    network_ids: Query<&NetworkId>,
) {
    let Some(entity) = trigger.client_id.entity() else {
        return;
    };
    commands.entity(entity).insert(Dead);

    // Previously the only broadcast on death was the global GameOver (only
    // sent once *every* connected player is dead, below) — other clients
    // had no per-entity signal at all, so a player who died early into a
    // multi-player match just looked frozen mid-animation to everyone else
    // until the whole match ended. This fires immediately, per player.
    if let Ok(network_id) = network_ids.get(entity) {
        commands.server_trigger(ToClients {
            mode: SendMode::Broadcast,
            message: PlayerDiedFx { who_network_id: network_id.get() },
        });
    }

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

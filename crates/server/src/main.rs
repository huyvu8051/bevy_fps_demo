use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::time::SystemTime;

use avian3d::prelude::*;
use bevy_landmass::prelude::*;
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy_replicon::prelude::*;
use bevy_replicon::shared::backend::connected_client::NetworkId;
use bevy_replicon_renet2::{
    netcode::{
        BoxedSocket, NativeSocket, NetcodeServerTransport, ServerAuthentication, ServerSetupConfig,
    },
    renet2::{ConnectionConfig, RenetServer},
    RenetChannelsExt, RepliconRenetPlugins,
};
use big_brain::prelude::*;
use shared::{
    entity_phase, resolve_bone_world_positions, zombie_hit_damage_bones, zombie_hit_damage_ray, zombie_hit_nearest_region, BoneWorldPositions, Chase, ClearAllZombies, ClientInput, DebugBot, DespawnDebugBot, FireShot,
    GameOver, GameStateEvent, Health, KnockbackVelocity, LastProcessedInput, MoveDebugBot, NetPlayer,
    NetTransform, Player, PlayerDied, PlayerDiedFx, PlayerFiredFx, PlayerStance, RequestRestart,
    SetAutoMaintainZombies, SetDamageMultiplier, SetFreezeZombies, SetZombieCount,
    ShotConfirmed, ShotFx, SpawnDebugBot, SpawnMoreZombies, Zombie,
    ZombieAnimationPlayer, ZombieAttackTimer, ZombieHitboxBones, ZombieMotion, ZombiePose, find_animation_root, wire_zombie_hitbox_bones, zombie_pose_for, zombie_speed_for, random_zombie_spawn_pos, 
    MapKind, scenario_player_spawn,
    DEBUG_BOT_DETECT_RANGE, DEBUG_BOT_ENGAGE_RANGE, DEBUG_BOT_EYE_HEIGHT, DEBUG_BOT_FIRE_INTERVAL_SECS,
    DEBUG_BOT_RETARGET_INTERVAL, GRAVITY, GROUND_NORMAL_MIN_Y, KNOCKBACK_DECAY_RATE, KNOCKBACK_FORCE,
    MAX_SHOT_DISTANCE, MOVE_SPEED, PILLAR_RADIUS,
    PLAYER_LENGTH, PLAYER_RADIUS, Scenario,
    STRESS_TEST_ZOMBIE_COUNT,
    ZOMBIE_AI_ENABLED, ZOMBIE_AVOID_ANGLES_DEG, ZOMBIE_AVOID_PROBE_DISTANCE, ZOMBIE_CHASE_RANGE,
    ZOMBIE_LENGTH, ZOMBIE_LOD_DISTANCE, ZOMBIE_LOD_REST_HEIGHT, ZOMBIE_MAX_HEALTH, ZOMBIE_PATH_RECALC_INTERVAL,
    DIRECTOR_CALM_DURATION_SECS, DIRECTOR_CALM_PRESSURE_THRESHOLD, DIRECTOR_HORDE_SIZE,
    DIRECTOR_HORDE_SPAWN_RADIUS, DIRECTOR_MELEE_RANGE, DIRECTOR_PRESSURE_SMOOTHING, INTERP_DELAY_SECS,
    LAG_COMPENSATION_MAX_SECS, PLAYER_THREAT_DECAY_RATE, PLAYER_THREAT_PER_SHOT, ZOMBIE_LOAD_BALANCE_PENALTY,
    PROTOCOL_ID,
    ZombieKind, zombie_kind_for_phase, ZOMBIE_KIND_WEIGHTS, zombie_kind_mesh_path, zombie_kind_anim_path, zombie_kind_pose_clips,
    ZOMBIE_MOVE_RADIUS, ZOMBIE_OUTER_HEIGHT, ZOMBIE_OUTER_RADIUS, ZOMBIE_RADIUS, ZOMBIE_RESPAWN_DELAY, ZOMBIE_RETARGET_INTERVAL, ZOMBIE_SEPARATION_RADIUS,
    ZOMBIE_SEPARATION_STRENGTH, ZOMBIE_SPAWNS, ZOMBIE_STOP_DISTANCE, ZOMBIE_THREAT_DISTANCE_BONUS,
    ZOMBIE_TURN_RATE_DEG,
};
use shared::sim_latency::{sim_latency_from_env, LaggedSocket};

use bevy::animation::graph::{AnimationGraph, AnimationGraphHandle, AnimationNodeIndex};
use bevy::animation::AnimationPlayer;
use bevy::gltf::GltfAssetLabel;
use bevy::pbr::StandardMaterial;

const MAX_CLIENTS: usize = 256;

fn scenario_from_env() -> Scenario {
    std::env::var("MAP_ID")
        .ok()
        .and_then(|s| s.parse::<u8>().ok())
        .map(Scenario::from_map_id)
        .unwrap_or_default()
}

const SERVER_TICK_HZ: f64 = 32.0;

fn main() {
    let scenario = scenario_from_env();

    eprintln!("[server] scenario: {scenario:?} (port {})", scenario.port());
    App::new()
        .insert_resource(scenario)
        .add_plugins((
            MinimalPlugins.set(bevy::app::ScheduleRunnerPlugin::run_loop(std::time::Duration::from_secs_f64(
                1.0 / SERVER_TICK_HZ,
            ))),
            bevy::log::LogPlugin::default(),

            bevy::asset::AssetPlugin {
                file_path: "../client/assets".to_string(),
                ..default()
            },
            bevy::mesh::MeshPlugin,
            bevy::scene::ScenePlugin,
            bevy::world_serialization::WorldSerializationPlugin,

            bevy::image::ImagePlugin::default(),

            bevy::animation::AnimationPlugin,
            bevy::gltf::GltfPlugin::default(),
            StatesPlugin,
            PhysicsPlugins::default(),
            BigBrainPlugin::new(PreUpdate),
            RepliconPlugins,
            RepliconRenetPlugins,
            server::navmesh::ServerNavmeshPlugin,
        ))
        .init_asset::<StandardMaterial>()

        .register_type::<Transform>()
        .register_type::<GlobalTransform>()
        .register_type::<bevy::transform::components::TransformTreeChanged>()
        .register_type::<Visibility>()
        .register_type::<InheritedVisibility>()
        .register_type::<ViewVisibility>()
        .register_type::<Name>()
        .register_type::<Mesh3d>()
        .register_type::<MeshMaterial3d<StandardMaterial>>()
        .register_type::<bevy::ecs::hierarchy::Children>()
        .register_type::<bevy::ecs::hierarchy::ChildOf>()
        .register_type::<bevy::animation::AnimatedBy>()
        .register_type::<AnimationPlayer>()
        .register_type::<bevy::camera::primitives::Aabb>()
        .register_type::<bevy::gltf::GltfExtras>()
        .register_type::<bevy::gltf::GltfSceneExtras>()
        .register_type::<bevy::gltf::GltfMeshExtras>()
        .register_type::<bevy::gltf::GltfMaterialExtras>()
        .register_type::<bevy::gltf::GltfMeshName>()
        .register_type::<bevy::gltf::GltfMaterialName>()
        .register_type::<bevy::mesh::skinning::SkinnedMesh>()
        .replicate::<NetworkId>()
        .replicate::<NetPlayer>()
        .replicate::<NetTransform>()
        .replicate::<Zombie>()
        .replicate::<ZombieKind>()
        .replicate::<Health>()
        .replicate::<LastProcessedInput>()
        .replicate::<DebugBot>()

        .add_client_event::<ClientInput>(Channel::Unreliable)
        .add_client_event::<SpawnMoreZombies>(Channel::Unordered)
        .add_client_event::<FireShot>(Channel::Ordered)
        .add_server_event::<ShotFx>(Channel::Ordered)
        .add_server_event::<ShotConfirmed>(Channel::Ordered)
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
        .add_client_event::<SetFreezeZombies>(Channel::Ordered)
        .add_client_event::<SetDamageMultiplier>(Channel::Ordered)
        .init_resource::<FreezeZombiesServer>()
        .init_resource::<DamageMultiplierServer>()
        .init_resource::<PendingRespawns>()
        .init_resource::<GameOver>()
        .init_resource::<DebugBotIdCounter>()
        .init_resource::<Director>()
        .init_resource::<ZombieTargetCount>()
        .init_resource::<AutoMaintainZombies>()
        .init_resource::<NextZombieSpawnIndex>()
        .init_resource::<DesertSpawnPoints>()
        .add_systems(
            Startup,
            (
                start_server,
                spawn_arena_colliders,

                spawn_stress_test_zombies.run_if(resource_equals(Scenario::StressTest)),
                (collect_desert_spawn_points, spawn_desert_zombies)
                    .chain()
                    .run_if(resource_equals(Scenario::Desert)),
                setup_server_zombie_anim,
            ),
        )
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
                (move_zombies, update_server_zombie_pose, record_zombie_history)
                    .chain()
                    .after(LandmassSystems::Output),
                (bot_pick_target, move_debug_bots, debug_bot_fire).chain(),
                tick_respawns,
                decay_player_threat,

                wire_server_zombie_animations,
                wire_zombie_hitbox_bones,
            ),
        )
        .add_observer(spawn_player)
        .add_observer(log_client_disconnect)
        .add_observer(apply_client_input)
        .add_observer(on_zombie_spawned)
        .add_observer(spawn_more_zombies)
        .add_observer(on_set_zombie_count)
        .add_observer(on_clear_all_zombies)
        .add_observer(on_set_auto_maintain_zombies)
        .add_observer(on_set_freeze_zombies)
.add_observer(on_set_damage_multiplier)
        .add_observer(on_player_died)
        .add_observer(on_request_restart)
        .add_observer(handle_fire_shot)
        .add_observer(spawn_debug_bot)
        .add_observer(move_debug_bot_command)
        .add_observer(despawn_debug_bot)
        .run();
}

fn start_server(mut commands: Commands, channels: Res<RepliconChannels>, scenario: Res<Scenario>) {
    let server = RenetServer::new(ConnectionConfig::from_channels(
        channels.server_configs(),
        channels.client_configs(),
    ));

    let current_time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("system clock should be after the unix epoch");

    let public_addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), scenario.port());
    let udp_socket = UdpSocket::bind(public_addr).expect("failed to bind server UDP socket");
    let native_socket = LaggedSocket::new(NativeSocket::new(udp_socket).unwrap(), sim_latency_from_env());

    let server_config = ServerSetupConfig {
        current_time,
        max_clients: MAX_CLIENTS,
        protocol_id: PROTOCOL_ID,
        authentication: ServerAuthentication::Unsecure,

        socket_addresses: vec![vec![public_addr]],
    };
    let transport = NetcodeServerTransport::new_with_sockets(server_config, vec![BoxedSocket::new(native_socket)])
        .expect("failed to set up netcode server transport");

    commands.insert_resource(server);
    commands.insert_resource(transport);
    info!("dedicated server listening on {public_addr}");
}

fn spawn_arena_colliders(mut commands: Commands, scenario: Res<Scenario>, asset_server: Res<AssetServer>) {
    server::map_colliders::spawn_map_colliders(&mut commands, *scenario, &asset_server);
}

fn spawn_player(trigger: On<Add, ConnectedClient>, mut commands: Commands, scenario: Res<Scenario>) {
    let entity = trigger.event().entity;
    info!("client connected: {entity}");
    let spawn_pos = scenario_player_spawn(*scenario);
    commands.entity(entity).insert((
        Transform::from_translation(spawn_pos),
        Player {
            vertical_velocity: 0.0,
            grounded: false,
        },
        PlayerStance::default(),
        ClientInput::default(),
        NetPlayer,
        NetTransform {
            translation: spawn_pos,
            yaw: 0.0,
            moving: false,
            sprinting: false,
        },
        LastProcessedInput::default(),
        PlayerThreat::default(),
        Replicated,
    ));
}

#[derive(Resource, Default)]
struct FreezeZombiesServer(bool);

fn on_set_freeze_zombies(trigger: On<FromClient<SetFreezeZombies>>, mut freeze: ResMut<FreezeZombiesServer>) {
    freeze.0 = trigger.message.0;
}

#[derive(Resource)]
struct DamageMultiplierServer(f32);
impl Default for DamageMultiplierServer {
    fn default() -> Self {
        Self(1.0)
    }
}

fn on_set_damage_multiplier(trigger: On<FromClient<SetDamageMultiplier>>, mut multiplier: ResMut<DamageMultiplierServer>) {
    multiplier.0 = trigger.message.0.max(0.0);
}

#[derive(Component, Default)]
struct PlayerThreat(f32);

fn decay_player_threat(time: Res<Time>, mut threats: Query<&mut PlayerThreat>) {
    let decay = (1.0 - PLAYER_THREAT_DECAY_RATE * time.delta_secs()).clamp(0.0, 1.0);
    for mut threat in &mut threats {
        threat.0 *= decay;
    }
}

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

    let spawn_y = ZOMBIE_RADIUS + ZOMBIE_LENGTH / 2.0;
    let spawns: Vec<(Vec3, f32)> = (0..DIRECTOR_HORDE_SIZE)
        .map(|i| {
            let angle = i as f32 / DIRECTOR_HORDE_SIZE as f32 * std::f32::consts::TAU;
            let pos = Vec3::new(center.x + angle.cos() * DIRECTOR_HORDE_SPAWN_RADIUS, spawn_y, center.z + angle.sin() * DIRECTOR_HORDE_SPAWN_RADIUS);
            (pos, 0.0)
        })
        .collect();
    info!("AI Director: sustained calm, spawning a {}-zombie horde near {center}", DIRECTOR_HORDE_SIZE);
    spawn_zombies(&mut commands, &spawns, ZOMBIE_AI_ENABLED, true);
}

fn log_client_disconnect(trigger: On<Remove, ConnectedClient>) {
    info!("client disconnected: {}", trigger.event().entity);
}

fn apply_client_input(
    trigger: On<FromClient<ClientInput>>,
    mut players: Query<(&mut ClientInput, &mut LastProcessedInput, &mut PlayerStance)>,
) {
    let Some(entity) = trigger.client_id.entity() else {
        return;
    };
    if let Ok((mut input, mut last_processed, mut stance)) = players.get_mut(entity) {

        *stance = trigger.message.stance;
        *input = trigger.message;
        last_processed.0 = trigger.message.sequence;
    }
}

fn move_players(
    time: Res<Time>,
    mut players: Query<(&mut Transform, &mut Player, &PlayerStance, &ClientInput, &mut NetTransform)>,
    mover: MoveAndSlide,
) {
    for (mut transform, mut player, stance, input, mut net_transform) in &mut players {
        shared::movement::simulate_player_step(&mover, &mut transform, &mut player, *stance, input, time.delta_secs());
        net_transform.translation = transform.translation;
        net_transform.yaw = input.yaw;

        net_transform.moving = input.move_axis != Vec2::ZERO;
        net_transform.sprinting = input.sprint;
    }
}

#[derive(Resource, Default)]
struct DebugBotIdCounter(u32);

#[derive(Component)]
struct DebugBotTarget(Vec3);

#[derive(Component, Default)]
struct DebugBotMotion {
    vertical_velocity: f32,
    grounded: bool,
}

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

        let stop_distance = if ai.target_zombie.is_some() { DEBUG_BOT_ENGAGE_RANGE } else { 0.05 };
        let to_target = target.0 - transform.translation;
        let horizontal = Vec3::new(to_target.x, 0.0, to_target.z);
        let distance = horizontal.length();
        let moving = distance > stop_distance;
        net_transform.moving = moving;
        let horizontal_velocity = if moving { (horizontal / distance) * MOVE_SPEED } else { Vec3::ZERO };

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

            let direction = horizontal / distance;
            let target_rotation = Transform::default().looking_to(-direction, Vec3::Y).rotation;
            turn_towards(&mut transform.rotation, target_rotation, ZOMBIE_TURN_RATE_DEG.to_radians() * time.delta_secs());
        }

        net_transform.translation = transform.translation;
        net_transform.yaw = transform.rotation.to_euler(EulerRot::YXZ).0;
    }
}

fn debug_bot_fire(
    mut commands: Commands,
    time: Res<Time>,
    mut bots: Query<(&Transform, &mut DebugBotAi), With<DebugBot>>,
    mut zombies: Query<(Entity, &Transform, &mut Health, &mut KnockbackVelocity, &ZombieHitboxBones), (With<Zombie>, Without<DebugBot>)>,
    bone_transforms: Query<&GlobalTransform>,
    mut respawns: ResMut<PendingRespawns>,
    director_spawned: Query<(), With<DirectorSpawned>>,
    damage_multiplier: Res<DamageMultiplierServer>,
) {
    for (transform, mut ai) in &mut bots {
        ai.fire_timer -= time.delta_secs();
        let Some(target) = ai.target_zombie else { continue };
        let Ok((entity, zombie_transform, mut health, mut knockback, bones)) = zombies.get_mut(target) else {
            continue;
        };

        let origin = transform.translation + Vec3::Y * DEBUG_BOT_EYE_HEIGHT;

        if origin.distance(zombie_transform.translation) > DEBUG_BOT_ENGAGE_RANGE + 1.0 || ai.fire_timer > 0.0 {
            continue;
        }
        ai.fire_timer = DEBUG_BOT_FIRE_INTERVAL_SECS;

        let Ok(direction) = Dir3::new(zombie_transform.translation - origin) else {
            continue;
        };
        let cylinder = Collider::cylinder(ZOMBIE_OUTER_RADIUS, ZOMBIE_OUTER_HEIGHT);
        let Some((hit_distance, normal)) = cylinder.cast_ray(
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

        let resolved_bones = resolve_bone_world_positions(bones, |bone| bone_transforms.get(bone).ok().map(|g| g.translation()));
        let damage = zombie_hit_damage_bones(hit_point, &resolved_bones, *zombie_transform.forward()) * damage_multiplier.0;
        health.0 -= damage;
        let killed = health.0 <= 0.0;
        if killed {
            let was_director_spawn = director_spawned.contains(entity);
            commands.entity(entity).despawn();
            if !was_director_spawn {
                respawns.0.push(PendingRespawn { timer: ZOMBIE_RESPAWN_DELAY });
            }
        } else {
            let push = Vec3::new(zombie_transform.translation.x - origin.x, 0.0, zombie_transform.translation.z - origin.z)
                .normalize_or_zero()
                * KNOCKBACK_FORCE;
            knockback.0 += push;
        }

        commands.server_trigger(ToClients {
            targets: SendTargets::All,
            message: ShotFx::ZombieHit { origin, point: hit_point, normal, damage, killed },
        });
    }
}

fn column_footprint(x: f32, z: f32) -> Vec<Vec2> {

    vec![
        Vec2::new(x - PILLAR_RADIUS, z - PILLAR_RADIUS),
        Vec2::new(x - PILLAR_RADIUS, z + PILLAR_RADIUS),
        Vec2::new(x + PILLAR_RADIUS, z + PILLAR_RADIUS),
        Vec2::new(x + PILLAR_RADIUS, z - PILLAR_RADIUS),
    ]
}

#[derive(Resource)]
struct ZombieTargetCount(u32);

impl Default for ZombieTargetCount {
    fn default() -> Self {
        let count = std::env::var("ZOMBIE_COUNT").ok().and_then(|s| s.parse().ok()).unwrap_or(10);
        Self(count)
    }
}

#[derive(Resource)]
struct AutoMaintainZombies(bool);

impl Default for AutoMaintainZombies {
    fn default() -> Self {
        Self(true)
    }
}

#[derive(Resource, Default)]
struct NextZombieSpawnIndex(usize);

fn zombie_spawn_pos_for(kind: MapKind, seed: usize, desert: &[Vec3]) -> (Vec3, f32) {
    match kind {
        // Desert reinforcements come from the same navmesh points the initial
        // wave used; `desert` is empty until the navmesh asset lands.
        MapKind::Desert if !desert.is_empty() => {
            (desert[seed % desert.len()] + Vec3::Y * ZOMBIE_LOD_REST_HEIGHT, 0.0)
        }
        MapKind::Desert => (shared::DESERT_PLAYER_SPAWN, 0.0),
        MapKind::Arena => (random_zombie_spawn_pos(seed), 0.0),
    }
}

fn on_set_zombie_count(
    trigger: On<FromClient<SetZombieCount>>,
    mut commands: Commands,
    scenario: Res<Scenario>,
    mut target: ResMut<ZombieTargetCount>,
    mut next_spawn_index: ResMut<NextZombieSpawnIndex>,
    desert_points: Res<DesertSpawnPoints>,
    baseline_zombies: Query<Entity, (With<Zombie>, Without<DirectorSpawned>)>,
) {
    let kind = scenario.map_kind();
    target.0 = trigger.message.0.min(ZOMBIE_SPAWNS.len() as u32);
    let current = baseline_zombies.iter().count() as u32;
    if current < target.0 {
        let to_spawn: Vec<(Vec3, f32)> = (0..target.0 - current)
            .map(|_| {
                let spawn = zombie_spawn_pos_for(kind, next_spawn_index.0, &desert_points.0);
                next_spawn_index.0 += 1;
                spawn
            })
            .collect();
        spawn_zombies(&mut commands, &to_spawn, ZOMBIE_AI_ENABLED, false);
    } else if current > target.0 {
        for entity in baseline_zombies.iter().take((current - target.0) as usize) {
            commands.entity(entity).try_despawn();
        }
    }
}

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
    spawn_zombies(&mut commands, &with_zero_yaw(&ZOMBIE_SPAWNS), ZOMBIE_AI_ENABLED, false);
}

#[derive(Component)]
struct DirectorSpawned;

#[derive(Clone, Default)]
struct BoneLocalOffsets {
    head: Option<Vec3>,
    belly: Option<Vec3>,
    chest: Option<Vec3>,
    left_arm: [Option<Vec3>; 3],
    right_arm: [Option<Vec3>; 3],
    left_leg: [Option<Vec3>; 3],
    right_leg: [Option<Vec3>; 3],
}

#[derive(Component, Default)]
struct ZombieHistory(std::collections::VecDeque<(f32, Vec3, Quat, BoneLocalOffsets)>);

fn record_zombie_history(
    time: Res<Time>,
    mut zombies: Query<(&Transform, &mut ZombieHistory, &ZombieHitboxBones), With<Zombie>>,
    bone_transforms: Query<&GlobalTransform>,
) {
    let now = time.elapsed_secs();
    for (transform, mut history, bones) in &mut zombies {
        let inv_rotation = transform.rotation.inverse();
        let local_offset = |bone: Entity| bone_transforms.get(bone).ok().map(|g| inv_rotation * (g.translation() - transform.translation));
        let offset_chain = |chain: &[Option<Entity>; 3]| chain.map(|joint| joint.and_then(local_offset));
        let offsets = BoneLocalOffsets {
            head: bones.head.and_then(local_offset),
            belly: bones.belly.and_then(local_offset),
            chest: bones.chest.and_then(local_offset),
            left_arm: offset_chain(&bones.left_arm),
            right_arm: offset_chain(&bones.right_arm),
            left_leg: offset_chain(&bones.left_leg),
            right_leg: offset_chain(&bones.right_leg),
        };
        history.0.push_back((now, transform.translation, transform.rotation, offsets));
        while history.0.front().is_some_and(|&(t, ..)| now - t > LAG_COMPENSATION_MAX_SECS) {
            history.0.pop_front();
        }
    }
}

fn zombie_history_at(history: &ZombieHistory, target_time: f32) -> Option<(Vec3, Quat, BoneWorldPositions)> {
    let &(_, translation, rotation, ref offsets) =
        history.0.iter().min_by(|(a, ..), (b, ..)| (a - target_time).abs().total_cmp(&(b - target_time).abs()))?;
    let to_world = |local: Vec3| translation + rotation * local;
    let to_world_chain = |chain: &[Option<Vec3>; 3]| chain.map(|joint| joint.map(to_world));
    Some((
        translation,
        rotation,
        BoneWorldPositions {
            head: offsets.head.map(to_world),
            belly: offsets.belly.map(to_world),
            chest: offsets.chest.map(to_world),
            left_arm: to_world_chain(&offsets.left_arm),
            right_arm: to_world_chain(&offsets.right_arm),
            left_leg: to_world_chain(&offsets.left_leg),
            right_leg: to_world_chain(&offsets.right_leg),
        },
    ))
}

#[cfg(test)]
mod hitbox_phase3_tests {
    use super::*;

    #[test]
    fn rewind_uses_the_rotation_recorded_at_that_instant_not_the_latest_one() {
        let mut history = ZombieHistory::default();

        history.0.push_back((
            0.0,
            Vec3::ZERO,
            Quat::IDENTITY,
            BoneLocalOffsets {
                head: Some(Vec3::Z),
                belly: None,
                chest: None,
                left_arm: [None; 3],
                right_arm: [None; 3],
                left_leg: [None; 3],
                right_leg: [None; 3],
            },
        ));

        let turned = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        history.0.push_back((
            1.0,
            Vec3::ZERO,
            turned,
            BoneLocalOffsets {
                head: Some(Vec3::Z),
                belly: None,
                chest: None,
                left_arm: [None; 3],
                right_arm: [None; 3],
                left_leg: [None; 3],
                right_leg: [None; 3],
            },
        ));

        let (_, _, bones_at_0) = zombie_history_at(&history, 0.0).unwrap();
        assert!(bones_at_0.head.unwrap().distance(Vec3::Z) < 1e-5, "rewound head position: {:?}", bones_at_0.head);

    }
}

fn spawn_zombie(commands: &mut Commands, position: Vec3, yaw: f32, ai_enabled: bool, director_spawned: bool) {
    let mut zombie = commands.spawn_empty();
    let phase = entity_phase(zombie.id());
    zombie.insert((
        Transform::from_translation(position).with_rotation(Quat::from_rotation_y(yaw)),
        Zombie,

        zombie_kind_for_phase(phase, ZOMBIE_KIND_WEIGHTS),
        ZombieMotion {
            path_recalc_timer: phase * ZOMBIE_PATH_RECALC_INTERVAL,
            ..default()
        },
        KnockbackVelocity::default(),
        ZombieAttackTimer::default(),
        NetTransform {
            translation: position,
            yaw,
            moving: false,
            sprinting: false,
        },
        Health(ZOMBIE_MAX_HEALTH),
        Replicated,

        Collider::cylinder(ZOMBIE_OUTER_RADIUS, ZOMBIE_OUTER_HEIGHT),
        ZombieHistory::default(),

        ZombieHitboxBones::default(),

        ZombiePose::default(),

        Visibility::default(),
    ));
    if ai_enabled {
        zombie.insert(Thinker::build().picker(FirstToScore { threshold: 0.8 }).when(NearPlayer, ChaseAction));
    }
    if director_spawned {
        zombie.insert(DirectorSpawned);
    }
}

fn spawn_zombies(commands: &mut Commands, spawns: &[(Vec3, f32)], ai_enabled: bool, director_spawned: bool) {
    for &(position, yaw) in spawns {
        spawn_zombie(commands, position, yaw, ai_enabled, director_spawned);
    }
}

fn with_zero_yaw(positions: &[Vec3]) -> Vec<(Vec3, f32)> {
    positions.iter().map(|&p| (p, 0.0)).collect()
}

fn spawn_stress_test_zombies(mut commands: Commands) {
    spawn_zombies(&mut commands, &with_zero_yaw(&ZOMBIE_SPAWNS[..STRESS_TEST_ZOMBIE_COUNT]), true, false);
}

/// Spawn points taken from the baked navmesh, so zombies always start on a
/// surface they can actually path from. A ring of fixed offsets drops most of
/// them inside buildings or onto disconnected rooftops.
#[derive(Resource, Default)]
struct DesertSpawnPoints(Vec<Vec3>);

fn collect_desert_spawn_points(scenario: Res<Scenario>, mut commands: Commands) {
    let Some(navmesh) = server::navmesh::read_baked_navmesh(*scenario) else {
        return;
    };

    let origin = shared::DESERT_PLAYER_SPAWN;
    let mut points: Vec<Vec3> = navmesh
        .detail
        .vertices
        .iter()
        .copied()
        .filter(|v| {
            let flat = Vec3::new(v.x - origin.x, 0.0, v.z - origin.z).length();
            (shared::DESERT_SPAWN_MIN_RADIUS..shared::DESERT_SPAWN_MAX_RADIUS).contains(&flat)
                && (v.y - origin.y).abs() < shared::DESERT_SPAWN_MAX_RISE
        })
        .collect();
    points.sort_by(|a, b| a.distance(origin).total_cmp(&b.distance(origin)));
    points.dedup_by(|a, b| a.distance(*b) < 5.0);

    info!("desert: {} navmesh spawn points near the player", points.len());
    commands.insert_resource(DesertSpawnPoints(points));
}

fn spawn_desert_zombies(
    points: Res<DesertSpawnPoints>,
    target: Res<ZombieTargetCount>,
    mut commands: Commands,
) {
    if points.0.is_empty() {
        return;
    }
    // Nearest-first: the town's navmesh is fragmented, and points further out
    // are far more likely to sit on an island with no route to the player.
    let count = (target.0 as usize).min(points.0.len());
    let spawns: Vec<(Vec3, f32)> = points.0[..count]
        .iter()
        .map(|p| (*p + Vec3::Y * ZOMBIE_LOD_REST_HEIGHT, 0.0))
        .collect();
    spawn_zombies(&mut commands, &spawns, true, false);
}

struct ServerZombieKindAnim {
    graph: Handle<AnimationGraph>,

    poses: [AnimationNodeIndex; 5],
}

impl ServerZombieKindAnim {
    fn node(&self, pose: ZombiePose) -> AnimationNodeIndex {
        self.poses[match pose {
            ZombiePose::Idle => 0,
            ZombiePose::Walk => 1,
            ZombiePose::Run => 2,
            ZombiePose::Crawl => 3,
            ZombiePose::Bite => 4,
        }]
    }
    fn idle(&self) -> AnimationNodeIndex {
        self.poses[0]
    }
}

#[derive(Resource)]
struct ServerZombieAnim {
    classic: ServerZombieKindAnim,
    meshy: ServerZombieKindAnim,
    soldier: ServerZombieKindAnim,
}

impl ServerZombieAnim {
    fn slot(&self, kind: ZombieKind) -> &ServerZombieKindAnim {
        match kind {
            ZombieKind::Classic => &self.classic,
            ZombieKind::Meshy => &self.meshy,
            ZombieKind::Soldier => &self.soldier,
        }
    }
}

fn setup_server_zombie_anim(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
) {
    let mut build = |kind: ZombieKind| {

        let path = zombie_kind_anim_path(kind);
        let clips: Vec<Handle<AnimationClip>> = zombie_kind_pose_clips(kind)
            .into_iter()
            .map(|index| asset_server.load(GltfAssetLabel::Animation(index).from_asset(path)))
            .collect();
        let (graph, nodes) = AnimationGraph::from_clips(clips);
        ServerZombieKindAnim {
            graph: graphs.add(graph),
            poses: [nodes[0], nodes[1], nodes[2], nodes[3], nodes[4]],
        }
    };
    commands.insert_resource(ServerZombieAnim {
        classic: build(ZombieKind::Classic),
        meshy: build(ZombieKind::Meshy),
        soldier: build(ZombieKind::Soldier),
    });
}

fn on_zombie_spawned(
    trigger: On<Add, Zombie>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    kinds: Query<&ZombieKind>,
) {
    let entity = trigger.event().entity;

    let kind = kinds.get(entity).copied().unwrap_or_default();
    let scene = asset_server.load(GltfAssetLabel::Scene(0).from_asset(zombie_kind_mesh_path(kind)));
    commands.entity(entity).with_child((

        Transform::IDENTITY,
        Visibility::default(),
        WorldAssetRoot(scene),
    ));
}

fn wire_server_zombie_animations(
    mut commands: Commands,
    anim: Res<ServerZombieAnim>,
    child_of: Query<&ChildOf>,
    zombies: Query<Option<&ZombieKind>, With<Zombie>>,
    mut new_players: Query<(Entity, &mut AnimationPlayer), Added<AnimationPlayer>>,
) {
    for (player_entity, mut player) in &mut new_players {
        let Some(root) = find_animation_root(player_entity, &child_of, |e| zombies.get(e).is_ok()) else {
            continue;
        };
        let kind = zombies.get(root).ok().flatten().copied().unwrap_or_default();
        let slot = anim.slot(kind);
        commands.entity(player_entity).try_insert(AnimationGraphHandle(slot.graph.clone()));
        player.play(slot.idle()).repeat();

        commands.entity(root).try_insert(ZombieAnimationPlayer(player_entity));
    }
}

fn update_server_zombie_pose(
    anim: Res<ServerZombieAnim>,
    players: Query<&Transform, (With<NetPlayer>, Without<Zombie>)>,
    mut zombies: Query<
        (&Transform, &NetTransform, &Health, &ChaseTarget, &ZombieAnimationPlayer, Option<&ZombieKind>, &mut ZombiePose),
        With<Zombie>,
    >,
    mut anim_players: Query<&mut AnimationPlayer>,
) {
    for (transform, net_transform, health, chase_target, link, kind, mut pose) in &mut zombies {
        let distance = chase_target
            .entity
            .and_then(|target| players.get(target).ok())
            .map_or(f32::MAX, |target_transform| transform.translation.distance(target_transform.translation));
        let new_pose = zombie_pose_for(distance, health.0 / ZOMBIE_MAX_HEALTH, net_transform.moving);
        if new_pose == *pose {
            continue;
        }
        *pose = new_pose;
        if let Ok(mut player) = anim_players.get_mut(link.0) {
            player.stop_all();
            player.play(anim.slot(kind.copied().unwrap_or_default()).node(new_pose)).repeat();
        }
    }
}

#[derive(Clone, Component, Debug, ScorerBuilder)]
struct NearPlayer;

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

#[derive(Clone, Component, Debug, ActionBuilder)]
struct ChaseAction;

#[derive(Component, Default)]
struct ChaseTarget {
    entity: Option<Entity>,
    reevaluate_timer: f32,
}

fn chase_action_system(mut commands: Commands, mut query: Query<(&Actor, &mut ActionState, &ActionSpan)>) {
    for (Actor(actor), mut state, span) in &mut query {
        let _guard = span.span().enter();
        match *state {

            ActionState::Requested => {
                commands.entity(*actor).try_insert((Chase, ChaseTarget::default()));
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
    freeze: Res<FreezeZombiesServer>,
    mut zombies: Query<
        (
            Entity,
            &mut Transform,
            &mut ZombieMotion,
            &mut ChaseTarget,
            &mut KnockbackVelocity,
            &mut NetTransform,
            Option<&mut AgentTarget3d>,
            Option<&AgentDesiredVelocity3d>,
        ),
        (With<Zombie>, With<Chase>),
    >,
    players: Query<(Entity, &Transform, &PlayerThreat), (With<NetPlayer>, Without<Zombie>)>,
    mover: MoveAndSlide,
) {

    if freeze.0 {
        for (.., mut net_transform, _, _) in &mut zombies {
            net_transform.moving = false;
        }
        return;
    }

    let positions: Vec<(Entity, Vec3)> = zombies.iter().map(|(e, t, ..)| (e, t.translation)).collect();

    let candidates: Vec<(Entity, Vec3, f32)> = players.iter().map(|(e, t, threat)| (e, t.translation, threat.0)).collect();

    let mut chase_load: std::collections::HashMap<Entity, u32> = std::collections::HashMap::new();
    for (.., chase_target, _, _, _, _) in &zombies {
        if let Some(target) = chase_target.entity {
            *chase_load.entry(target).or_insert(0) += 1;
        }
    }

    for (entity, mut transform, mut motion, mut chase_target, mut knockback, mut net_transform, mut agent_target, desired_velocity) in
        &mut zombies
    {

        chase_target.reevaluate_timer -= time.delta_secs();
        let target_still_connected = chase_target.entity.is_some_and(|target| candidates.iter().any(|&(e, ..)| e == target));
        if !target_still_connected || chase_target.reevaluate_timer <= 0.0 {

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
            if let Some(agent_target) = agent_target.as_deref_mut() {
                *agent_target = AgentTarget3d::None;
            }
            if distance > ZOMBIE_STOP_DISTANCE {
                let direction = horizontal / distance;
                transform.translation += direction * zombie_speed_for(distance) * time.delta_secs();
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
        if let Some(agent_target) = agent_target.as_deref_mut() {
            *agent_target = AgentTarget3d::Point(target_translation);
        }

        let filter = SpatialQueryFilter::default();
        let horizontal_velocity = if distance > ZOMBIE_STOP_DISTANCE {
            motion.path_recalc_timer -= time.delta_secs();
            if motion.path_recalc_timer <= 0.0 {
                motion.path_recalc_timer = ZOMBIE_PATH_RECALC_INTERVAL;

                let path_direction = desired_velocity
                    .map(|v| v.velocity())
                    .map(|v| Vec3::new(v.x, 0.0, v.z).normalize_or_zero())
                    .filter(|direction| *direction != Vec3::ZERO);

                let desired_direction = path_direction.or_else(|| {
                    Some(
                        Vec3::new(
                            target_translation.x - transform.translation.x,
                            0.0,
                            target_translation.z - transform.translation.z,
                        )
                        .normalize_or_zero(),
                    )
                    .filter(|direction| *direction != Vec3::ZERO)
                });

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
                let desired_direction = desired_direction.map(|d| (d + separation * ZOMBIE_SEPARATION_STRENGTH).normalize_or_zero());

                motion.cached_path_dir = desired_direction
                    .map(|d| steer_around_obstacles(&mover.spatial_query, transform.translation, d, distance, &filter))
                    .unwrap_or(Vec3::ZERO);
            }
            motion.cached_path_dir * zombie_speed_for(distance)
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

        let shape = Collider::cylinder(ZOMBIE_MOVE_RADIUS, ZOMBIE_OUTER_HEIGHT);
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

#[derive(Resource, Default)]
struct PendingRespawns(Vec<PendingRespawn>);

struct PendingRespawn {
    timer: f32,
}

fn tick_respawns(
    time: Res<Time>,
    mut commands: Commands,
    scenario: Res<Scenario>,
    mut respawns: ResMut<PendingRespawns>,
    mut next_spawn_index: ResMut<NextZombieSpawnIndex>,
    auto_maintain: Res<AutoMaintainZombies>,
    desert_points: Res<DesertSpawnPoints>,
) {
    let kind = scenario.map_kind();
    let dt = time.delta_secs();
    let mut i = 0;
    let mut to_spawn = Vec::new();
    while i < respawns.0.len() {
        respawns.0[i].timer -= dt;
        if respawns.0[i].timer <= 0.0 {
            respawns.0.swap_remove(i);
            if auto_maintain.0 {

                to_spawn.push(zombie_spawn_pos_for(kind, next_spawn_index.0, &desert_points.0));
                next_spawn_index.0 += 1;
            }
        } else {
            i += 1;
        }
    }
    if !to_spawn.is_empty() {
        spawn_zombies(&mut commands, &to_spawn, ZOMBIE_AI_ENABLED, false);
    }
}

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
    damage_multiplier: Res<DamageMultiplierServer>,
) {

    if let Some(shooter) = trigger.client_id.entity() {
        if let Ok(mut threat) = threats.get_mut(shooter) {
            threat.0 += PLAYER_THREAT_PER_SHOT;
        }

        if let Ok(network_id) = network_ids.get(shooter) {
            commands.server_trigger(ToClients {
                targets: SendTargets::All,
                message: PlayerFiredFx { shooter_network_id: network_id.get() },
            });
        }
    }

    let shot = trigger.message;
    let Ok(direction) = Dir3::new(shot.direction) else {
        return;
    };
    info!("[shot] fired: origin={:.2?} direction={:.2?}", shot.origin, *direction);

    let filter = SpatialQueryFilter::default();

    let live_hit = spatial_query.cast_ray(shot.origin, direction, MAX_SHOT_DISTANCE, true, &filter);

    let wall_filter = SpatialQueryFilter::from_excluded_entities(zombies.iter().map(|(e, ..)| e));
    let wall_distance = spatial_query
        .cast_ray(shot.origin, direction, MAX_SHOT_DISTANCE, true, &wall_filter)
        .map(|hit| hit.distance);

    let rtt = trigger
        .client_id
        .entity()
        .and_then(|entity| network_ids.get(entity).ok())
        .map_or(0.0, |network_id| renet_server.rtt(network_id.get()));
    let rewind_secs = (rtt as f32 / 2.0 + INTERP_DELAY_SECS).min(LAG_COMPENSATION_MAX_SECS);
    let rewind_time = time.elapsed_secs() - rewind_secs;

    let cylinder = Collider::cylinder(ZOMBIE_OUTER_RADIUS, ZOMBIE_OUTER_HEIGHT);

    let mut rewound_candidates: Vec<(f32, Entity, BoneWorldPositions, Vec3)> = zombies
        .iter()
        .filter_map(|(entity, _, _, history)| {
            let (position, rotation, bones) = zombie_history_at(history, rewind_time)?;
            let (distance, _outer_normal) = cylinder.cast_ray(position, rotation, shot.origin, *direction, MAX_SHOT_DISTANCE, true)?;

            Some((distance, entity, bones, rotation * Vec3::NEG_Z))
        })
        .collect();
    rewound_candidates.sort_by(|(a, ..), (b, ..)| a.total_cmp(b));

    for (distance, entity, bones, forward) in &rewound_candidates {
        let (entity, forward) = (*entity, *forward);
        info!("[shot] outer capsule hit (rewound): zombie={entity} distance={distance:.2}");

        let Some((hit_point, normal, damage)) = zombie_hit_damage_ray(shot.origin, direction, MAX_SHOT_DISTANCE, bones, forward) else {

            let outer_hit_point = shot.origin + *direction * distance;
            if let Some((region, nearest_distance, radius)) = zombie_hit_nearest_region(outer_hit_point, bones, forward) {
                info!(
                    "[shot] outer capsule hit but missed every inner bone capsule: zombie={entity} — nearest was {region}, \
                     distance={nearest_distance:.3} radius={radius:.3} (missed by {:.3}) — trying the next zombie behind it, if any",
                    nearest_distance - radius
                );
            } else {
                info!("[shot] outer capsule hit but missed every inner bone capsule: zombie={entity} (no bones resolved yet) — trying the next zombie behind it, if any");
            }
            continue;
        };
        let damage = damage * damage_multiplier.0;

        let hit_distance = shot.origin.distance(hit_point);
        if wall_distance.is_some_and(|w| w < hit_distance - 0.1) {
            info!(
                "[shot] blocked by level geometry before zombie={entity} (wall at {:.2} < zombie at {:.2})",
                wall_distance.unwrap(),
                hit_distance
            );
            continue;
        }
        info!("[shot] inner bone capsule hit: zombie={entity} damage={damage:.1}");
        if let Ok((_, mut health, mut knockback, _)) = zombies.get_mut(entity) {
            health.0 -= damage;
            let killed = health.0 <= 0.0;
            if killed {
                let was_director_spawn = director_spawned.contains(entity);
                commands.entity(entity).despawn();
                if !was_director_spawn {
                    respawns.0.push(PendingRespawn { timer: ZOMBIE_RESPAWN_DELAY });
                }
            } else {
                let push = Vec3::new(shot.direction.x, 0.0, shot.direction.z).normalize_or_zero() * KNOCKBACK_FORCE;
                knockback.0 += push;
            }

            commands.server_trigger(ToClients {
                targets: SendTargets::AllExcept(trigger.client_id),
                message: ShotFx::ZombieHit { origin: shot.origin, point: hit_point, normal, damage, killed },
            });

            commands.server_trigger(ToClients {
                targets: SendTargets::Single(trigger.client_id),
                message: ShotConfirmed { point: hit_point, normal, damage, killed },
            });
        }
        return;
    }

    let Some(hit) = live_hit else {
        info!("[shot] missed everything (no zombie, no surface)");
        return;
    };
    let outer_hit_point = shot.origin + *direction * hit.distance;

    if let Ok((entity, mut health, mut knockback, history)) = zombies.get_mut(hit.entity) {
        info!("[shot] outer capsule hit (live, no rewind match): zombie={entity} distance={:.2}", hit.distance);
        let (_, rotation, bones) = zombie_history_at(history, time.elapsed_secs())
            .unwrap_or((outer_hit_point, Quat::IDENTITY, BoneWorldPositions::default()));

        let forward = rotation * Vec3::NEG_Z;

        let Some((hit_point, normal, damage)) = zombie_hit_damage_ray(shot.origin, direction, MAX_SHOT_DISTANCE, &bones, forward) else {
            if let Some((region, nearest_distance, radius)) = zombie_hit_nearest_region(outer_hit_point, &bones, forward) {
                info!(
                    "[shot] outer capsule hit but missed every inner bone capsule: zombie={entity} — nearest was {region}, \
                     distance={nearest_distance:.3} radius={radius:.3} (missed by {:.3}) — treating as a genuine miss, no VFX",
                    nearest_distance - radius
                );
            } else {
                info!("[shot] outer capsule hit but missed every inner bone capsule: zombie={entity} (no bones resolved yet) — treating as a genuine miss, no VFX");
            }
            return;
        };
        let damage = damage * damage_multiplier.0;
        info!("[shot] inner bone capsule hit: zombie={entity} damage={damage:.1}");
        health.0 -= damage;
        let killed = health.0 <= 0.0;
        if killed {
            let was_director_spawn = director_spawned.contains(entity);
            commands.entity(entity).despawn();
            if !was_director_spawn {
                respawns.0.push(PendingRespawn { timer: ZOMBIE_RESPAWN_DELAY });
            }
        } else {
            let push = Vec3::new(shot.direction.x, 0.0, shot.direction.z).normalize_or_zero() * KNOCKBACK_FORCE;
            knockback.0 += push;
        }
        commands.server_trigger(ToClients {
            targets: SendTargets::AllExcept(trigger.client_id),
            message: ShotFx::ZombieHit { origin: shot.origin, point: hit_point, normal, damage, killed },
        });
        commands.server_trigger(ToClients {
            targets: SendTargets::Single(trigger.client_id),
            message: ShotConfirmed { point: hit_point, normal, damage, killed },
        });
        return;
    }

    info!("[shot] hit a surface, no zombie: point={:.2?}", outer_hit_point);
    commands.server_trigger(ToClients {
        targets: SendTargets::All,
        message: ShotFx::SurfaceHit { origin: shot.origin, point: outer_hit_point, normal: hit.normal },
    });
}

#[derive(Component)]
struct Dead;

fn on_player_died(
    trigger: On<FromClient<PlayerDied>>,
    mut commands: Commands,
    mut game_over: ResMut<GameOver>,

    connected: Query<Entity, (With<NetPlayer>, Without<DebugBot>)>,
    dead: Query<(), With<Dead>>,
    network_ids: Query<&NetworkId>,
) {
    let Some(entity) = trigger.client_id.entity() else {
        return;
    };
    commands.entity(entity).insert(Dead);

    if let Ok(network_id) = network_ids.get(entity) {
        commands.server_trigger(ToClients {
            targets: SendTargets::All,
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
            targets: SendTargets::All,
            message: GameStateEvent::GameOver,
        });
    }
}

fn on_request_restart(
    _trigger: On<FromClient<RequestRestart>>,
    mut commands: Commands,
    mut game_over: ResMut<GameOver>,
    mut respawns: ResMut<PendingRespawns>,
    mut director: ResMut<Director>,
    scenario: Res<Scenario>,
    desert_points: Res<DesertSpawnPoints>,
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

    match *scenario {
        Scenario::Desert => {
            let spawns: Vec<(Vec3, f32)> = desert_points
                .0
                .iter()
                .take(STRESS_TEST_ZOMBIE_COUNT)
                .map(|p| (*p + Vec3::Y * ZOMBIE_LOD_REST_HEIGHT, 0.0))
                .collect();
            spawn_zombies(&mut commands, &spawns, true, false);
        }
        Scenario::StressTest => {
            spawn_zombies(&mut commands, &with_zero_yaw(&ZOMBIE_SPAWNS[..STRESS_TEST_ZOMBIE_COUNT]), true, false);
        }
    }
    let player_spawn = scenario_player_spawn(*scenario);

    for (entity, mut transform, mut player, mut net_transform) in &mut players {
        commands.entity(entity).remove::<Dead>();
        transform.translation = player_spawn;
        player.vertical_velocity = 0.0;
        player.grounded = false;
        net_transform.translation = player_spawn;
        net_transform.yaw = 0.0;
    }

    commands.server_trigger(ToClients {
        targets: SendTargets::All,
        message: GameStateEvent::Restarted,
    });
}

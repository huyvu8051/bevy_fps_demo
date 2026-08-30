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
    entity_phase, resolve_bone_world_positions, zombie_hit_damage_bones, zombie_hit_damage_ray, zombie_hit_nearest_region, BoneWorldPositions, Chase, ClearAllZombies, ClientInput, DebugBot, DespawnDebugBot, FireShot,
    GameOver, GameStateEvent, Health, KnockbackVelocity, LastProcessedInput, MoveDebugBot, NetPlayer,
    NetTransform, Player, PlayerDied, PlayerDiedFx, PlayerFiredFx, PlayerStance, RequestRestart,
    SetAutoMaintainZombies, SetFreezeZombies, SetZombieCount,
    ShotConfirmed, ShotFx, SpawnDebugBot, SpawnMoreZombies, Zombie,
    ZombieAnimationPlayer, ZombieAttackTimer, ZombieHitboxBones, ZombieMotion, ZombiePose, find_animation_root, wire_zombie_hitbox_bones, zombie_pose_for, zombie_speed_for, random_zombie_spawn_pos, random_map1_zombie_spawn_pos,
    MapKind, scenario_player_spawn,
    DEBUG_BOT_DETECT_RANGE, DEBUG_BOT_ENGAGE_RANGE, DEBUG_BOT_EYE_HEIGHT, DEBUG_BOT_FIRE_INTERVAL_SECS,
    DEBUG_BOT_RETARGET_INTERVAL, GRAVITY, GROUND_NORMAL_MIN_Y, KNOCKBACK_DECAY_RATE, KNOCKBACK_FORCE,
    HitboxTestSlot, HITBOX_TEST_RESPAWN_DELAY, HITBOX_TEST_ZOMBIE_POSITIONS,
    MAX_SHOT_DISTANCE, MOVE_SPEED, NAVMESH_BOUNDS, NAVMESH_COLUMN_OBSTACLES, NAVMESH_HEIGHT, PILLAR_RADIUS,
    PLAYER_LENGTH, PLAYER_RADIUS, Scenario,
    RENDER_COMPARE_ZOMBIE_POSITIONS,
    STRESS_TEST_ZOMBIE_COUNT,
    ZOMBIE_AI_ENABLED, ZOMBIE_AVOID_ANGLES_DEG, ZOMBIE_AVOID_PROBE_DISTANCE, ZOMBIE_CHASE_RANGE,
    ZOMBIE_CRAWL_HEALTH_FRACTION,
    ZOMBIE_LENGTH, ZOMBIE_LOD_DISTANCE, ZOMBIE_LOD_REST_HEIGHT, ZOMBIE_MAX_HEALTH, ZOMBIE_PATH_RECALC_INTERVAL,
    DIRECTOR_CALM_DURATION_SECS, DIRECTOR_CALM_PRESSURE_THRESHOLD, DIRECTOR_HORDE_SIZE,
    DIRECTOR_HORDE_SPAWN_RADIUS, DIRECTOR_MELEE_RANGE, DIRECTOR_PRESSURE_SMOOTHING, INTERP_DELAY_SECS,
    LAG_COMPENSATION_MAX_SECS, PLAYER_THREAT_DECAY_RATE, PLAYER_THREAT_PER_SHOT, ZOMBIE_LOAD_BALANCE_PENALTY,
    PROTOCOL_ID,
    ZombieKind, zombie_kind_for_phase, zombie_kind_mesh_path, zombie_kind_gameplay_mesh_transform, zombie_kind_pose_clips,
    ZOMBIE_MOVE_RADIUS, ZOMBIE_OUTER_HEIGHT, ZOMBIE_OUTER_RADIUS, ZOMBIE_RADIUS, ZOMBIE_RESPAWN_DELAY, ZOMBIE_RETARGET_INTERVAL, ZOMBIE_SEPARATION_RADIUS,
    ZOMBIE_SEPARATION_STRENGTH, ZOMBIE_SPAWNS, ZOMBIE_STOP_DISTANCE, ZOMBIE_THREAT_DISTANCE_BONUS,
    ZOMBIE_TURN_RATE_DEG,
};
use shared::sim_latency::{sim_latency_from_env, LaggedSocket};
use vleue_navigator::prelude::*;

// AnimationPlayer/StandardMaterial: needed by the register_type calls below
// (SceneSpawner's reflection-based entity cloning requires every component
// type present in a spawned glTF scene to be registered — see the doc
// comment on that block). AnimationGraph/AnimationGraphHandle/
// AnimationNodeIndex/GltfAssetLabel were only needed by Phase 1's spawn
// spike (removed once its exit criterion — head bone Y visibly changing,
// confirmed headless — passed); Phase 2 will need them again once real
// zombies get a live skeleton.
use bevy::animation::graph::{AnimationGraph, AnimationGraphHandle, AnimationNodeIndex};
use bevy::animation::AnimationPlayer;
use bevy::gltf::GltfAssetLabel;
use bevy::pbr::StandardMaterial;

// PORT is now Scenario::port() (SERVER_PORT_BASE + map id) — each map-
// select scenario is its own dedicated-server process on its own port,
// since bevy_replicon has no "rooms" concept (see Scenario's own doc
// comment, crates/shared/src/lib.rs). No auth/matchmaking:
// ServerAuthentication::Unsecure and a large max_clients stand in for "no
// player limit" per the plan.
// PROTOCOL_ID now lives in crates/shared — see its own doc comment.
const MAX_CLIENTS: usize = 256;

// MAP_ID (0/1/2 — matches Scenario::from_map_id) selects which scenario
// this server process runs, read once at startup. Mirrors the client's own
// MAP_ID handling (crates/client/src/lib.rs) — both sides independently
// derive the same port from the same env var/default, no handshake needed.
fn scenario_from_env() -> Scenario {
    let map_id: u8 = std::env::var("MAP_ID").ok().and_then(|s| s.parse().ok()).unwrap_or(0);
    Scenario::from_map_id(map_id)
}

// MinimalPlugins' default ScheduleRunnerPlugin loops as fast as the CPU
// allows (RunMode::Loop { wait: None }) — meaning move_players/move_zombies
// ran with a tiny, unstable dt every iteration, different from whatever dt
// the client happened to predict with for the "same" input. That dt
// mismatch was a real source of the client's jump-reconciliation drift
// (see predict_player_movement's own doc comment for the other half of
// that fix). Capping the server to a fixed tick gives both sides a
// consistent, comparable dt. 32Hz — deliberately low (a hitscan zombie
// co-op has no twitch-competitive aiming needs) and cheap; MUST equal the
// client's INPUT_SEND_HZ.
const SERVER_TICK_HZ: f64 = 32.0;

fn main() {
    let scenario = scenario_from_env();
    // eprintln, not info! — this runs before LogPlugin (added below) has
    // installed the global tracing subscriber, so info! would silently
    // vanish here.
    eprintln!("[server] scenario: {scenario:?} (port {})", scenario.port());
    App::new()
        .insert_resource(scenario)
        .add_plugins((
            MinimalPlugins.set(bevy::app::ScheduleRunnerPlugin::run_loop(std::time::Duration::from_secs_f64(
                1.0 / SERVER_TICK_HZ,
            ))),
            bevy::log::LogPlugin::default(),
            // Phase 1 spike (HITBOX_PLAN.md): the server has no assets/
            // dir of its own (it never needed one — pure geometry/physics
            // so far). Bevy's AssetPlugin resolves `file_path` relative to
            // this crate's own CARGO_MANIFEST_DIR in dev builds, NOT the
            // process's actual CWD (confirmed the hard way: "crates/client/
            // assets" resolved to .../crates/server/crates/client/assets)
            // — so this is relative to crates/server/, one level up then
            // into the client crate's existing assets dir, same file/
            // source of truth zombie.glb already has, no duplication.
            bevy::asset::AssetPlugin {
                file_path: "../client/assets".to_string(),
                ..default()
            },
            bevy::mesh::MeshPlugin,
            bevy::scene::ScenePlugin,
            // Registers Assets<Image> — normally pulled in transitively by
            // RenderPlugin (which this headless server never adds), but
            // bevy_gltf's loader unconditionally tries to allocate an
            // Image handle for every embedded texture regardless of
            // whether anything ever renders it, and panics without this
            // ("Cannot allocate an Asset Handle of type ... Image because
            // the asset type has not been initialized").
            bevy::image::ImagePlugin::default(),
            // Same "asset type never got initialized" story as
            // ImagePlugin above, one step further into the glTF material
            // chain: bevy_gltf allocates a StandardMaterial handle per
            // material regardless of rendering. Registering the asset
            // type directly (init_asset, below) instead of the full
            // PbrPlugin avoids pulling in PbrPlugin's actual render-world
            // extraction/lighting systems, which this headless server has
            // no use for and which likely assume a RenderPlugin exists.
            // Phase 1 of the real-hitbox spike (see HITBOX_PLAN.md): loads
            // the zombie's skeleton/animation curves so bone
            // GlobalTransforms are available server-side, with zero
            // rendering — no RenderPlugin/WinitPlugin/wgpu anywhere in this
            // plugin list, confirming AnimationPlugin's only cost here is
            // the data (scene graph + animation sampling), not a GPU.
            // GltfPlugin is what actually registers the AssetLoader for
            // .glb/.gltf paths — enabling the "bevy_gltf" cargo feature
            // alone only compiles the crate in, it doesn't register
            // anything with AssetServer (first attempt without this hit
            // "Could not find an asset loader matching ... zombie.glb").
            bevy::animation::AnimationPlugin,
            bevy::gltf::GltfPlugin::default(),
            StatesPlugin,
            PhysicsPlugins::default(),
            BigBrainPlugin::new(PreUpdate),
            VleueNavigatorPlugin,
            // Arena maps: the zombie navmesh is triangulated automatically
            // from every ArenaObstacle-tagged collider (the pillar rows in
            // arena_layout_map<ID>.ron), so there's no hand-kept obstacle
            // list to drift. Non-Arena maps still use the manual
            // NavMesh::from_edge_and_obstacles path in spawn_zombie_navmesh.
            NavmeshUpdaterPlugin::<Collider, ArenaObstacle>::default(),
            RepliconPlugins,
            RepliconRenetPlugins,
        ))
        .init_asset::<StandardMaterial>()
        // Phase 1 spike (HITBOX_PLAN.md): SceneSpawner clones entities from
        // a loaded glTF Scene using reflection-based component cloning for
        // any type without a registered ComponentCloneBehavior, so every
        // component type a character-rig scene actually contains has to be
        // registered even though nothing here ever serializes/reflects
        // them for any other reason. TransformPlugin normally does the
        // Transform/GlobalTransform half of this (not present — this
        // server relies on avian3d's own propagation instead of the
        // official plugin), the rest are glTF-scene-specific.
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
        // LevelBox/LevelPanel/LevelTransform used to be registered here —
        // no longer replicated at all (see level_piece.rs's own doc
        // comment): both client and server build the exact same map
        // geometry locally from shared::map1_level_pieces/
        // item_gallery_level_pieces, so there's nothing for the network
        // to carry for it anymore.
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
        .init_resource::<FreezeZombiesServer>()
        .init_resource::<PendingRespawns>()
        .init_resource::<GameOver>()
        .init_resource::<DebugBotIdCounter>()
        .init_resource::<Director>()
        .init_resource::<ZombieTargetCount>()
        .init_resource::<AutoMaintainZombies>()
        .init_resource::<NextZombieSpawnIndex>()
        .add_systems(
            Startup,
            (
                start_server,
                spawn_arena_colliders,
                spawn_zombie_navmesh,
                // Mutually exclusive — exactly one runs, gated on the
                // Scenario resource `insert_resource`d above (fixed for
                // this process's whole lifetime, no runtime switching).
                spawn_initial_zombies.run_if(|s: Res<Scenario>| matches!(*s, Scenario::Normal | Scenario::LegacyArena)),
                spawn_hitbox_test_zombies.run_if(resource_equals(Scenario::HitboxTest)),
                spawn_stress_test_zombies.run_if(resource_equals(Scenario::StressTest)),
                spawn_render_compare_zombies_system.run_if(resource_equals(Scenario::RenderCompare)),
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
                (assign_render_compare_bite_target, move_zombies, update_server_zombie_pose, record_zombie_history).chain(),
                (bot_pick_target, move_debug_bots, debug_bot_fire).chain(),
                tick_respawns,
                decay_player_threat,
                // TEMPORARILY disabled — was stacking more hordes on top of
                // the cheat panel's zombie count target every ~20s of calm,
                // fighting with it. Re-add once ZombieTargetCount/director
                // hordes are reconciled to coexist properly.
                // run_ai_director,
                // Phase 2 (HITBOX_PLAN.md): real skeleton wiring for every
                // server zombie — see on_zombie_spawned (the observer that
                // spawns the child scene) for the other half.
                wire_server_zombie_animations,
                wire_zombie_hitbox_bones,
                spawn_car_nav_obstacle_boxes,
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


// The one geometry-per-scenario decision on the server — mirrors the
// client's own `match current_scenario().map_kind()` in setup(). This
// process never renders, it only needs the avian3d colliders so
// move_players has something to collide against.
fn spawn_arena_colliders(mut commands: Commands, scenario: Res<Scenario>, asset_server: Res<AssetServer>) {
    match scenario.map_kind() {
        MapKind::ItemGallery => spawn_item_gallery_level_pieces(&mut commands, &asset_server),
        MapKind::Building => spawn_map1_level_pieces(&mut commands),
        MapKind::Arena => {
            spawn_flat_arena_colliders(&mut commands, scenario.map_id());
            spawn_item_gallery_car_colliders(&mut commands, &asset_server);
        }
    }
}

// Tags an arena collider the auto-navmesh (NavmeshUpdaterPlugin above)
// should carve a hole around — only the pillars (Cylinder rows). The
// floor, perimeter walls and the ramp are deliberately left untagged: the
// floor/walls are the navmesh boundary, and the ramp has to stay
// traversable (zombies climb it, MoveAndSlide handles the slope).
#[derive(Component)]
struct ArenaObstacle;

// Flat room: floor + 4 perimeter walls + free-standing pillars + centre
// test ramp, read from crates/client/assets/data/arena_layout_map<ID>.ron
// (shared::arena_layout) — one file per MapKind::Arena map id. Colliders
// only, no Mesh3d/lights; the client builds its own visuals from the same
// list in spawn_test_arena.
fn spawn_flat_arena_colliders(commands: &mut Commands, map_id: u8) {
    for b in shared::arena_layout(map_id) {
        let (collider, is_obstacle) = match b.shape {
            shared::ArenaShape::Cuboid => (Collider::cuboid(b.size.x, b.size.y, b.size.z), false),
            // size.x is the full diameter, size.y the height.
            shared::ArenaShape::Cylinder => (Collider::cylinder(b.size.x * 0.5, b.size.y), true),
        };
        let mut e = commands.spawn((
            Transform::from_translation(b.translation).with_rotation(b.rotation),
            RigidBody::Static,
            collider,
        ));
        if is_obstacle {
            e.insert(ArenaObstacle);
        }
    }
}

// Builds every Map 1 floor (garage/lobby/F2/F3) as (translation, rotation,
// full-size, color, roughness) tuples using the exact same
// shared::map1 geometry functions the old 4 collider-only + 4 mesh-only
// functions each used to read independently, then spawns ONE entity per
// piece: a real RigidBody::Static + Collider (this process's own
// authoritative physics, unchanged from before) PLUS LevelBox +
// LevelTransform + Replicated, so a connected client can build its own
// mesh + local collider purely from what shows up over the network — see
// LevelBox's own doc comment (crates/shared/src/level_piece.rs) for why
// this replaces both the old collider-only AND mesh-only functions.
// (translation, rotation, width, height, color, roughness, collider_thickness, collider_offset)
// Both these now just call the shared piece-building functions (see
// level_piece.rs's module doc comment — same function client also
// calls) and spawn COLLIDER-ONLY entities from the result: this process
// runs the real authoritative physics, but no longer decides/replicates
// what a level LOOKS like — the client builds that independently from
// the exact same shared data.
fn spawn_map1_level_pieces(commands: &mut Commands) {
    let (pieces, panels) = shared::map1_level_pieces();
    spawn_collider_pieces(commands, pieces, panels);
}

fn spawn_item_gallery_level_pieces(commands: &mut Commands, asset_server: &AssetServer) {
    let (pieces, panels) = shared::item_gallery_level_pieces();
    spawn_collider_pieces(commands, pieces, panels);
    spawn_item_gallery_car_colliders(commands, asset_server);
}

// Collider proxy for the client's meshy.ai car test props (see crate
// client's spawn_item_gallery_test_props). Loads the SAME glTF meshes the
// client draws and lets avian build the collider off them
// (ColliderConstructor::ConvexHullFromMesh — one convex hull per mesh
// primitive, attached once the asset loads), so hit registration
// (handle_fire_shot's SpatialQuery) and movement blocking (move_players'
// MoveAndSlide) match the real car shape instead of a hand-sized box that
// kept drifting off the mesh. Headless is fine: this process already
// loads zombie.glb the same way (MeshPlugin + GltfPlugin, no RenderPlugin).
//
// RigidBody::Static here (not Dynamic like the client's) on purpose: the
// gravity-drop-then-shake-on-hit behavior is a purely cosmetic client-side
// effect nobody watches this process's own physics world for — this
// process only needs a fixed collider at each car's REST position (no drop
// margin), which Static gives for free without an unnecessary
// physics-settle each Startup.
fn spawn_item_gallery_car_colliders(commands: &mut Commands, asset_server: &AssetServer) {
    for (path, physics) in shared::ITEM_GALLERY_CAR_PATHS.iter().zip(shared::item_gallery_car_physics()) {
        commands.spawn((
            SceneRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(*path))),
            Transform::from_xyz(physics.x, physics.y, physics.z)
                .with_rotation(physics.rotation)
                .with_scale(Vec3::splat(physics.scale)),
            RigidBody::Static,
            ColliderConstructorHierarchy::new(ColliderConstructor::ConvexHullFromMesh),
            // spawn_car_nav_obstacle_boxes turns this into ONE navmesh
            // obstacle box per car once the hull colliders exist.
            CarNavObstacle,
        ));
    }
}

/// Car SceneRoot that still needs its navmesh obstacle box (removed once
/// CarNavBoxDone is added).
#[derive(Component)]
struct CarNavObstacle;
/// Car SceneRoot whose navmesh obstacle box has been spawned.
#[derive(Component)]
struct CarNavBoxDone;

// ONE coarse box per car for the navmesh, from the car's combined collider
// AABB (XZ only; the box is pinned to the navmesh sampling plane so a
// mid-drop snapshot still carves). NOT the raw per-primitive hulls —
// tagging all ~15 of those per car fed hundreds of overlapping constraint
// polygons into polyanya and blew RAM into the tens of GB. Sensor so it
// carves the navmesh without also being an invisible physical wall (the
// real hull colliders already block movement here). Idle once every car
// has its box.
fn spawn_car_nav_obstacle_boxes(
    mut commands: Commands,
    cars: Query<(Entity, &RigidBodyColliders), (With<CarNavObstacle>, Without<CarNavBoxDone>)>,
    aabbs: Query<&ColliderAabb>,
) {
    for (car, colliders) in &cars {
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        let mut ready = !colliders.is_empty();
        for collider in colliders.iter() {
            match aabbs.get(collider) {
                Ok(aabb) if aabb.min.is_finite() && aabb.max.is_finite() => {
                    min = min.min(aabb.min);
                    max = max.max(aabb.max);
                }
                _ => ready = false,
            }
        }
        if !ready {
            continue;
        }
        let center = Vec3::new((min.x + max.x) * 0.5, NAVMESH_HEIGHT, (min.z + max.z) * 0.5);
        let size = Vec3::new((max.x - min.x).max(0.1), 2.0, (max.z - min.z).max(0.1));
        commands.spawn((
            Transform::from_translation(center),
            RigidBody::Static,
            Collider::cuboid(size.x, size.y, size.z),
            Sensor,
            ArenaObstacle,
        ));
        commands.entity(car).insert(CarNavBoxDone);
    }
}

/// Spawns ONLY the avian3d colliders for a set of pieces — no LevelBox/
/// LevelPanel/Transform-for-rendering, since this process never renders
/// anything (see spawn_arena_colliders's own doc comment). A LevelBox
/// piece is just 1 static cuboid collider; a LevelPanel piece is a thin
/// cuboid collider only if it has collider_thickness (decorative panels
/// like the garage's yellow floor stripe get none — nothing to block).
fn spawn_collider_pieces(commands: &mut Commands, pieces: Vec<shared::BoxPiece>, panels: Vec<shared::PanelPiece>) {
    for (translation, rotation, size, _color, _roughness) in pieces {
        commands.spawn((
            Transform::from_translation(translation).with_rotation(rotation),
            RigidBody::Static,
            Collider::cuboid(size.x, size.y, size.z),
        ));
    }
    for (translation, rotation, width, height, _color, _roughness, collider_thickness, collider_offset) in panels {
        let Some(thickness) = collider_thickness else { continue };
        let offset = if collider_offset { -thickness / 2.0 } else { 0.0 };
        let rotation_applied = Transform::from_translation(translation).with_rotation(rotation);
        let local_offset = rotation_applied.rotation * Vec3::new(0.0, 0.0, offset);
        commands.spawn((
            Transform::from_translation(translation + local_offset).with_rotation(rotation),
            RigidBody::Static,
            Collider::cuboid(width, height, thickness),
        ));
    }
}

// Spawns the player's gameplay components directly onto its connection
// entity (same pattern bevy_replicon_renet2's tic_tac_toe example uses) —
// no separate lookup/owner component needed, since ClientId::Client wraps
// this exact Entity.
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

// See SetFreezeZombies' own doc comment for why this is one global
// server-wide flag (read by move_zombies) rather than per-player state.
#[derive(Resource, Default)]
struct FreezeZombiesServer(bool);

fn on_set_freeze_zombies(trigger: On<FromClient<SetFreezeZombies>>, mut freeze: ResMut<FreezeZombiesServer>) {
    freeze.0 = trigger.message.0;
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
    spawn_zombies(&mut commands, &spawns, ZOMBIE_AI_ENABLED, true, false);
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
    mut zombies: Query<(Entity, &Transform, &mut Health, &mut KnockbackVelocity, &ZombieHitboxBones), (With<Zombie>, Without<DebugBot>)>,
    bone_transforms: Query<&GlobalTransform>,
    mut respawns: ResMut<PendingRespawns>,
    director_spawned: Query<(), With<DirectorSpawned>>,
) {
    for (transform, mut ai) in &mut bots {
        ai.fire_timer -= time.delta_secs();
        let Some(target) = ai.target_zombie else { continue };
        let Ok((entity, zombie_transform, mut health, mut knockback, bones)) = zombies.get_mut(target) else {
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
        // Debug bots don't have client round-trips to rewind, so this
        // stays on live (not history-rewound) bone positions — see this
        // function's own doc comment above.
        let resolved_bones = resolve_bone_world_positions(bones, |bone| bone_transforms.get(bone).ok().map(|g| g.translation()));
        let damage = zombie_hit_damage_bones(hit_point, &resolved_bones, *zombie_transform.forward());
        health.0 -= damage;
        let killed = health.0 <= 0.0;
        if killed {
            let was_director_spawn = director_spawned.contains(entity);
            commands.entity(entity).despawn();
            if !was_director_spawn {
                respawns.0.push(PendingRespawn { timer: ZOMBIE_RESPAWN_DELAY, fixed_position: None });
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
            message: ShotFx::ZombieHit { origin, point: hit_point, normal, damage, killed },
        });
    }
}

fn spawn_zombie_navmesh(mut commands: Commands, mut navmeshes: ResMut<Assets<NavMesh>>, scenario: Res<Scenario>) {
    let plane = Transform::from_xyz(0.0, NAVMESH_HEIGHT, 0.0)
        .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2));

    if scenario.map_kind() == MapKind::Arena {
        // Auto-generated: NavmeshUpdaterPlugin::<Collider, ArenaObstacle>
        // triangulates the outer boundary minus every ArenaObstacle
        // collider's footprint — the 8 pillars, plus one coarse box per
        // car (spawn_car_nav_obstacle_boxes) — and rebuilds if one moves.
        // Blocking so the mesh exists by the end of the first update
        // (zombies spawn at Startup too); the car boxes appearing a few
        // frames later just trigger a cheap re-triangulation.
        commands.spawn((
            ManagedNavMesh::single(),
            NavMeshSettings {
                fixed: Triangulation::from_outer_edges(&shared::arena_navmesh_outer_edges(scenario.map_id())),
                // These three are shared with the client's nav_debug copy —
                // see their doc comment in crates/shared/src/arena.rs.
                simplify: shared::ARENA_NAVMESH_SIMPLIFY,
                build_timeout: Some(shared::ARENA_NAVMESH_BUILD_TIMEOUT_SECS),
                agent_radius: shared::ARENA_NAVMESH_AGENT_RADIUS,
                ..default()
            },
            NavMeshUpdateMode::Direct,
            NavMeshUpdateModeBlocking,
            plane,
        ));
        return;
    }

    // Non-Arena (Building / ItemGallery): no per-map RON, so keep the
    // hand-authored boundary + pillar footprints. A bare ManagedNavMesh
    // (no NavMeshSettings) is ignored by the updater, so this manual
    // insert stands.
    let mut navmesh = NavMesh::from_edge_and_obstacles(
        NAVMESH_BOUNDS.to_vec(),
        NAVMESH_COLUMN_OBSTACLES.iter().map(|&(x, z)| column_footprint(x, z)).collect(),
    );
    navmesh.set_transform(plane);
    navmeshes
        .insert(ManagedNavMesh::get_single().id(), navmesh)
        .expect("inserting under a Uuid asset id never fails");
    commands.spawn(ManagedNavMesh::single());
}

fn column_footprint(x: f32, z: f32) -> Vec<Vec2> {
    // Square footprint (not a circle) is a deliberately cheap approximation
    // for the navmesh cutout — PILLAR_RADIUS is small enough relative to
    // ZOMBIE_AVOID_PROBE_DISTANCE that the corner rounding error doesn't
    // matter for pathfinding, and Collider::cylinder below is what actually
    // determines the pillar's real collision shape.
    vec![
        Vec2::new(x - PILLAR_RADIUS, z - PILLAR_RADIUS),
        Vec2::new(x - PILLAR_RADIUS, z + PILLAR_RADIUS),
        Vec2::new(x + PILLAR_RADIUS, z + PILLAR_RADIUS),
        Vec2::new(x + PILLAR_RADIUS, z - PILLAR_RADIUS),
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

// Picks the right spawn scatter for the map: Map 1's own 4 real floors on
// MapKind::Building, the flat arena's generic grid otherwise. Every
// baseline zombie spawn call site (initial spawn, tick-based respawn,
// manual SetZombieCount — all Normal/LegacyArena only) goes through this
// instead of calling the raw fns directly, so none can drift back to
// spawning zombies in the void between Map 1's floors.
fn zombie_spawn_pos_for(kind: MapKind, seed: usize) -> Vec3 {
    match kind {
        MapKind::Building => random_map1_zombie_spawn_pos(seed),
        MapKind::Arena | MapKind::ItemGallery => random_zombie_spawn_pos(seed),
    }
}

fn spawn_initial_zombies(
    mut commands: Commands,
    scenario: Res<Scenario>,
    target: Res<ZombieTargetCount>,
    mut next_spawn_index: ResMut<NextZombieSpawnIndex>,
) {
    let kind = scenario.map_kind();
    let count = (target.0 as usize).min(ZOMBIE_SPAWNS.len());
    let positions: Vec<Vec3> = (0..count).map(|seed| zombie_spawn_pos_for(kind, seed)).collect();
    spawn_zombies(&mut commands, &positions, ZOMBIE_AI_ENABLED, false, false);
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
    scenario: Res<Scenario>,
    mut target: ResMut<ZombieTargetCount>,
    mut next_spawn_index: ResMut<NextZombieSpawnIndex>,
    baseline_zombies: Query<Entity, (With<Zombie>, Without<DirectorSpawned>)>,
) {
    let kind = scenario.map_kind();
    target.0 = trigger.message.0.min(ZOMBIE_SPAWNS.len() as u32);
    let current = baseline_zombies.iter().count() as u32;
    if current < target.0 {
        let to_spawn: Vec<Vec3> = (0..target.0 - current)
            .map(|_| {
                let pos = zombie_spawn_pos_for(kind, next_spawn_index.0);
                next_spawn_index.0 += 1;
                pos
            })
            .collect();
        spawn_zombies(&mut commands, &to_spawn, ZOMBIE_AI_ENABLED, false, false);
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
    spawn_zombies(&mut commands, &ZOMBIE_SPAWNS, ZOMBIE_AI_ENABLED, false, false);
}

// Marks a zombie spawned by run_ai_director as a bonus horde rather than
// baseline population — handle_fire_shot checks this to skip scheduling a
// 1:1 respawn on kill, so a calm-triggered horde actually thins out once
// dealt with instead of permanently growing the total zombie count.
#[derive(Component)]
struct DirectorSpawned;

// Per-bone offsets in the zombie ROOT's own local frame (i.e. already
// rotation.inverse()-applied) at the moment a ZombieHistory entry was
// recorded — HITBOX_PLAN.md Phase 3. Storing local offsets rather than raw
// world positions is what makes rewinding correct even while the zombie
// has since turned: reapplying a *past* rotation to a *local* offset gives
// the bone's world position as it truly was then, whereas a stored world
// position would silently assume the zombie's orientation never changed.
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

// Recent (translation, rotation, bone offsets) snapshots for lag-compensated
// hit detection — see handle_fire_shot and record_zombie_history. Not
// replicated (server-only bookkeeping, like ChaseTarget/PlayerThreat).
#[derive(Component, Default)]
struct ZombieHistory(std::collections::VecDeque<(f32, Vec3, Quat, BoneLocalOffsets)>);

// Runs after move_zombies each tick: records where every zombie (root AND
// each resolved hitbox bone, relative to the root) is right now, and
// prunes anything older than LAG_COMPENSATION_MAX_SECS needs. Bones that
// haven't resolved yet (scene still loading) just record None/empty for
// this tick — zombie_history_at's caller already falls back to the
// geometric guess for anything that never resolves.
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

// Nearest recorded snapshot to `target_time` — history is only sampled once
// per server tick (30Hz), so this is a "closest sample" lookup rather than
// interpolating between two, which is plenty precise for what's ultimately
// a cosmetic-accuracy-tolerant debug shooter. Reconstructs bone world
// positions from that same instant's root pose + locally-stored offsets
// (see BoneLocalOffsets) rather than returning raw offsets, so callers
// don't need to know this rewinding trick at all — just "where was
// everything, back then".
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

// Regression test for the core claim HITBOX_PLAN.md Phase 3 rests on:
// reapplying a PAST rotation to a locally-stored offset reconstructs the
// bone's true past world position, even after the zombie has since
// turned to face a different way. Kept permanently (unlike Phase 1/2's
// throwaway println-based manual checks) since this is exactly the kind
// of subtle math bug a future refactor could silently reintroduce.
#[cfg(test)]
mod hitbox_phase3_tests {
    use super::*;

    #[test]
    fn rewind_uses_the_rotation_recorded_at_that_instant_not_the_latest_one() {
        let mut history = ZombieHistory::default();
        // t=0: zombie faces +Z (no rotation), head bone 1.0 further along
        // its own forward axis (local +Z) from the root.
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
        // t=1: zombie has since turned 90° to face +X, root hasn't moved.
        // Same local offset recorded (rig didn't change shape, just
        // orientation) — this is what record_zombie_history would store
        // every tick regardless of whether the zombie turned in between.
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

        // Rewinding to t=0 must place the head where it was WHEN FACING
        // +Z (world +Z), not where the local offset would land if
        // rotated by the zombie's current (t=1) orientation.
        let (_, _, bones_at_0) = zombie_history_at(&history, 0.0).unwrap();
        assert!(bones_at_0.head.unwrap().distance(Vec3::Z) < 1e-5, "rewound head position: {:?}", bones_at_0.head);

        // Sanity check the other direction too: "rewinding" to t=1 should
        // place the head where +X-facing rotation puts it (world +X), not
        // world +Z — confirms the test itself isn't just trivially always
        // returning the same offset regardless of which entry matched.
        let (_, _, bones_at_1) = zombie_history_at(&history, 1.0).unwrap();
        assert!(bones_at_1.head.unwrap().distance(Vec3::X) < 1e-5, "rewound head position: {:?}", bones_at_1.head);
    }
}

// entity_phase now lives in crates/shared — random_aim_point (server_
// autopilot_think) needs the exact same per-entity hash, not a second copy.

// `ai_enabled` used to be the global ZOMBIE_AI_ENABLED const unconditionally
// — now a per-call flag so Scenario::HitboxTest's row can spawn with no
// Thinker at all (stationary, per its own doc comment) while normal/
// stress-test zombies keep the usual global toggle. `hitbox_test` tags the
// spawned entities with HitboxTestSlot (their own fixed respawn position)
// — see PendingRespawn's own doc comment for why.
fn spawn_zombies(commands: &mut Commands, spawns: &[Vec3], ai_enabled: bool, director_spawned: bool, hitbox_test: bool) {
    for &spawn_pos in spawns {
        let mut zombie = commands.spawn_empty();
        let phase = entity_phase(zombie.id());
        zombie.insert((
            Transform::from_translation(spawn_pos),
            Zombie,
            // ~50/50 Classic/Meshy from the same per-entity hash the
            // path-recalc stagger uses — replicated, so the client builds
            // the matching mesh (on_zombie_replicated / on_zombie_kind).
            zombie_kind_for_phase(phase),
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
            Collider::cylinder(ZOMBIE_OUTER_RADIUS, ZOMBIE_OUTER_HEIGHT),
            ZombieHistory::default(),
            // Phase 2 (HITBOX_PLAN.md): populated by wire_zombie_hitbox_bones
            // (shared, same system the client already ran) once
            // on_zombie_spawned's child scene finishes loading — real bone
            // entities for Phase 4's per-bone hit detection, not the
            // Y-threshold guess zombie_hit_damage_geometric uses today.
            ZombieHitboxBones::default(),
            // See ZombiePose's own doc comment (crates/shared) — server now
            // drives its own real pose transitions instead of sitting
            // frozen on Idle.
            ZombiePose::default(),
            // Needed on the root too, not just the child scene entity — an
            // InheritedVisibility child under a parent with no Visibility
            // at all trips Bevy's own B0004 warning every tick.
            Visibility::default(),
        ));
        if ai_enabled {
            zombie.insert(Thinker::build().picker(FirstToScore { threshold: 0.8 }).when(NearPlayer, ChaseAction));
        }
        if director_spawned {
            zombie.insert(DirectorSpawned);
        }
        if hitbox_test {
            zombie.insert(HitboxTestSlot(spawn_pos));
        }
    }
}

// Scenario::HitboxTest's Startup spawn — see HITBOX_TEST_ZOMBIE_POSITIONS
// and HitboxTestSlot's own doc comments.
fn spawn_hitbox_test_zombies(mut commands: Commands) {
    spawn_zombies(&mut commands, &HITBOX_TEST_ZOMBIE_POSITIONS, false, false, true);
}


// Scenario::StressTest's Startup spawn — STRESS_TEST_ZOMBIE_COUNT zombies
// with normal chase AI, taken from the front of the same ZOMBIE_SPAWNS
// grid the normal game uses.
fn spawn_stress_test_zombies(mut commands: Commands) {
    spawn_zombies(&mut commands, &ZOMBIE_SPAWNS[..STRESS_TEST_ZOMBIE_COUNT], true, false, false);
}

// Scenario::RenderCompare's Startup spawn — 3 stationary zombies, one per
// RENDER_COMPARE_ZOMBIE_POSITIONS entry, each hand-built (not via
// spawn_zombies — no Thinker/Chase on any of them, so move_zombies never
// touches them, and each needs different bonus components) to land on a
// different ZombiePose without ever moving:
//   - Idle: no ChaseTarget at all, so update_server_zombie_pose's query
//     (which requires &ChaseTarget) skips it entirely — it just sits on
//     ZombiePose's own spawn-time default forever.
//   - Crawl: a bare ChaseTarget (entity: None — distance then reads as
//     f32::MAX, well past ZOMBIE_STOP_DISTANCE, so it can't accidentally
//     read as Bite) plus low Health, since zombie_pose_for's Crawl branch
//     only looks at health_fraction.
//   - Bite: a bare ChaseTarget too, but assign_render_compare_bite_target
//     (Update) points its `entity` at the first connected player once one
//     exists (impossible to do here — no player has connected yet at
//     Startup) — RENDER_COMPARE_ZOMBIE_POSITIONS' Bite entry is already
//     placed within ZOMBIE_STOP_DISTANCE of RENDER_COMPARE_PLAYER_SPAWN,
//     so once ChaseTarget.entity is set the distance check passes too.
fn spawn_render_compare_zombies_system(mut commands: Commands) {
    spawn_render_compare_zombies(&mut commands);
}

fn spawn_render_compare_zombies(commands: &mut Commands) {
    for (i, &spawn_pos) in RENDER_COMPARE_ZOMBIE_POSITIONS.iter().enumerate() {
        let mut zombie = commands.spawn_empty();
        zombie.insert((
            Transform::from_translation(spawn_pos),
            Zombie,
            // RenderCompare is a classic-rig visual test — pin it, don't
            // randomise. Present so the client's on_zombie_kind_replicated
            // still fires and builds the mesh.
            ZombieKind::Classic,
            ZombieMotion::default(),
            KnockbackVelocity::default(),
            ZombieAttackTimer::default(),
            NetTransform { translation: spawn_pos, yaw: 0.0, moving: false, sprinting: false },
            Replicated,
            Collider::cylinder(ZOMBIE_OUTER_RADIUS, ZOMBIE_OUTER_HEIGHT),
            ZombieHistory::default(),
            ZombieHitboxBones::default(),
            ZombiePose::default(),
            Visibility::default(),
        ));
        match i {
            // Crawl: low health, bare ChaseTarget.
            2 => {
                zombie.insert((Health(ZOMBIE_MAX_HEALTH * ZOMBIE_CRAWL_HEALTH_FRACTION * 0.5), ChaseTarget::default()));
            }
            // Bite: full health, bare ChaseTarget — entity filled in once a
            // player connects, see assign_render_compare_bite_target.
            1 => {
                zombie.insert((Health(ZOMBIE_MAX_HEALTH), ChaseTarget::default(), RenderCompareBiteZombie));
            }
            // Idle: full health, no ChaseTarget at all.
            _ => {
                zombie.insert(Health(ZOMBIE_MAX_HEALTH));
            }
        }
    }
}

// Marker for RenderCompare's one Bite-pose zombie — lets
// assign_render_compare_bite_target find it without a magic index into
// RENDER_COMPARE_ZOMBIE_POSITIONS.
#[derive(Component)]
struct RenderCompareBiteZombie;

// Startup spawns this zombie before any player has connected, so its
// ChaseTarget.entity starts None — this fills it in with the first
// connected player as soon as one exists, which is what actually makes
// update_server_zombie_pose's distance check (RENDER_COMPARE_ZOMBIE_
// POSITIONS' Bite entry is already within ZOMBIE_STOP_DISTANCE of
// RENDER_COMPARE_PLAYER_SPAWN) start passing. Runs every frame but is a
// no-op once assigned or once there's simply no player yet.
fn assign_render_compare_bite_target(
    players: Query<Entity, With<NetPlayer>>,
    mut zombie: Query<&mut ChaseTarget, With<RenderCompareBiteZombie>>,
) {
    let Ok(mut chase_target) = zombie.single_mut() else {
        return;
    };
    if chase_target.entity.is_some() {
        return;
    }
    if let Some(player) = players.iter().next() {
        chase_target.entity = Some(player);
    }
}

// --- Phase 2 (HITBOX_PLAN.md): real skeleton on every server zombie -------
// Same asset the client renders — ZOMBIE_PATH/ZOMBIE_ANIM_* now live in
// crates/shared (see ZOMBIE_PATH's own doc comment) so this can't drift
// from what crates/client loads. Loaded here purely for its scene graph +
// animation curves (no RenderPlugin ever touches it, confirmed in Phase 1's
// spike). All 5 clips now loaded (not just Idle) — see
// update_server_zombie_pose/ZombiePose's own doc comment for why the
// server needs to actually cycle through real poses now, not sit frozen
// on Idle forever.

// Built once at Startup — one AnimationGraph asset per ZombieKind (shared
// by every zombie of that kind's AnimationPlayer, same one-graph-many-
// players pattern as the client's own ZombieKindGameplayAnim), since a
// zombie plays from its kind's own clip set.
struct ServerZombieKindAnim {
    graph: Handle<AnimationGraph>,
    // [idle, walk, run, crawl, bite] — same order as zombie_kind_pose_clips.
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
}

impl ServerZombieAnim {
    fn slot(&self, kind: ZombieKind) -> &ServerZombieKindAnim {
        match kind {
            ZombieKind::Classic => &self.classic,
            ZombieKind::Meshy => &self.meshy,
        }
    }
}

fn setup_server_zombie_anim(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
) {
    let mut build = |kind: ZombieKind| {
        let path = zombie_kind_mesh_path(kind);
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
    });
}

// Every zombie gets the real skeleton as a child scene the instant it's
// spawned — mirrors crates/client's on_zombie_replicated (Add<Zombie>
// observer spawning the exact same SceneRoot as a cosmetic child), just
// server-side and for hit detection instead of rendering.
fn on_zombie_spawned(
    trigger: On<Add, Zombie>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    kinds: Query<&ZombieKind>,
) {
    let entity = trigger.event().entity;
    // Inserted in the same spawn bundle, so it's here already; default
    // Classic for any spawn path that skips it (render-compare rig).
    let kind = kinds.get(entity).copied().unwrap_or_default();
    let scene = asset_server.load(GltfAssetLabel::Scene(0).from_asset(zombie_kind_mesh_path(kind)));
    commands.entity(entity).with_child((
        // Per-kind child transform — MUST match crates/client's own
        // zombie_mesh_bundle so the server's hit-detection skeleton lines
        // up with what the client draws. See ZOMBIE_MODEL_Y_OFFSET /
        // zombie_kind_gameplay_mesh_transform.
        zombie_kind_gameplay_mesh_transform(kind),
        Visibility::default(),
        SceneRoot(scene),
    ));
}

// Same "wait for the async scene load, then attach the graph and start
// playing" shape as crates/client's wire_zombie_animations, minus that
// version's PlayerModel-vs-Zombie ambiguity check — this server never
// spawns any other kind of animated character, so every AnimationPlayer
// that shows up belongs to a zombie by construction.
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
        // Recorded on the zombie ROOT (not this AnimationPlayer entity
        // itself) so update_server_zombie_pose can look it up the same way
        // crates/client's update_zombie_animation does — see
        // ZombieAnimationPlayer's own doc comment.
        commands.entity(root).try_insert(ZombieAnimationPlayer(player_entity));
    }
}

// Server-side mirror of crates/client's update_zombie_animation — see
// ZombiePose's own doc comment (crates/shared) for why the server needs to
// actually cycle its own AnimationPlayer through real poses now instead of
// sitting frozen on Idle. `distance` is measured to whichever player this
// zombie is actually chasing (ChaseTarget, the same target move_zombies
// itself just picked/used this tick) rather than "nearest any player" —
// keeps this consistent with what's actually driving net_transform.moving,
// instead of a second, possibly-different notion of "the" player.
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
    freeze: Res<FreezeZombiesServer>,
    mut zombies: Query<
        (Entity, &mut Transform, &mut ZombieMotion, &mut ChaseTarget, &mut KnockbackVelocity, &mut NetTransform),
        (With<Zombie>, With<Chase>),
    >,
    players: Query<(Entity, &Transform, &PlayerThreat), (With<NetPlayer>, Without<Zombie>)>,
    navmesh: Query<&ManagedNavMesh>,
    navmeshes: Res<Assets<NavMesh>>,
    mover: MoveAndSlide,
) {
    // See SetFreezeZombies' own doc comment — real position is
    // authoritative here, so this is the one place that actually needs to
    // stop for a debug "freeze" to do anything real (as opposed to the
    // client's own AnimationPlayer pause, which was fighting a still-moving
    // replicated position every frame). net_transform.moving is still set
    // to false so update_zombie_animation settles on Idle and stops
    // thrashing between poses while frozen, instead of being left however
    // it happened to be the instant this engaged.
    if freeze.0 {
        for (_, _, _, _, _, mut net_transform) in &mut zombies {
            net_transform.moving = false;
        }
        return;
    }

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

    for (entity, mut transform, mut motion, mut chase_target, mut knockback, mut net_transform) in &mut zombies {
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
        // Movement footprint, NOT the shot broad-phase — see ZOMBIE_MOVE_RADIUS.
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

// Countdown timers for zombies killed by handle_fire_shot; tick_respawns
// spawns a replacement once each hits zero, keeping the population topped
// back up instead of thinning out permanently — the server-authoritative
// counterpart of what used to be the client's own PendingZombieRespawns.
#[derive(Resource, Default)]
struct PendingRespawns(Vec<PendingRespawn>);

// `fixed_position: Some(pos)` — Scenario::HitboxTest — respawns a fresh
// zombie back at its own row slot (no AI, tagged HitboxTestSlot again)
// regardless of AutoMaintainZombies, which only governs the normal
// cycling-ZOMBIE_SPAWNS-grid population. `None` is the original/normal
// behavior. Only handle_fire_shot's two hit-resolution branches (the real
// player-shot path — what actually matters for hitbox testing) populate
// `fixed_position`; debug_bot_fire's own fire branch always pushes `None`,
// since bots aren't expected to be active during a HitboxTest session.
struct PendingRespawn {
    timer: f32,
    fixed_position: Option<Vec3>,
}

fn tick_respawns(
    time: Res<Time>,
    mut commands: Commands,
    scenario: Res<Scenario>,
    mut respawns: ResMut<PendingRespawns>,
    mut next_spawn_index: ResMut<NextZombieSpawnIndex>,
    auto_maintain: Res<AutoMaintainZombies>,
) {
    let kind = scenario.map_kind();
    let dt = time.delta_secs();
    let mut i = 0;
    let mut to_spawn = Vec::new();
    let mut to_spawn_fixed = Vec::new();
    while i < respawns.0.len() {
        respawns.0[i].timer -= dt;
        if respawns.0[i].timer <= 0.0 {
            let entry = respawns.0.swap_remove(i);
            if let Some(pos) = entry.fixed_position {
                to_spawn_fixed.push(pos);
            } else if auto_maintain.0 {
                // Still consumes the pending-respawn slot either way — only
                // whether it actually results in a new zombie depends on the
                // toggle, so flipping auto-maintain back on later doesn't
                // suddenly spawn a backlog of everything killed while it was off.
                to_spawn.push(zombie_spawn_pos_for(kind, next_spawn_index.0));
                next_spawn_index.0 += 1;
            }
        } else {
            i += 1;
        }
    }
    if !to_spawn.is_empty() {
        spawn_zombies(&mut commands, &to_spawn, ZOMBIE_AI_ENABLED, false, false);
    }
    if !to_spawn_fixed.is_empty() {
        spawn_zombies(&mut commands, &to_spawn_fixed, false, false, true);
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
    mut zombies: Query<(Entity, &mut Health, &mut KnockbackVelocity, &ZombieHistory, Option<&HitboxTestSlot>), With<Zombie>>,
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
    info!("[shot] fired: origin={:.2?} direction={:.2?}", shot.origin, *direction);

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
    let cylinder = Collider::cylinder(ZOMBIE_OUTER_RADIUS, ZOMBIE_OUTER_HEIGHT);
    // EVERY zombie whose (rewound) outer capsule the ray touches, nearest
    // first — not just the single nearest one. The outer capsule is a
    // broad-phase filter, not a solid wall: two zombies standing in a row
    // means the ray's own outer-capsule test can find both, and the front
    // one's inner bones all missing (a clean graze past its broad-phase
    // shell) must not stop the same ray from still being tested against the
    // one behind it. Collecting the whole list is what makes "keep trying
    // the next candidate on an inner-bone miss" possible below — the
    // earlier one-shot `.min_by` (just the nearest) had no next candidate
    // to fall back to, so a miss on the front zombie always ate the whole
    // shot right there, indistinguishable from actually blocking it.
    let mut rewound_candidates: Vec<(f32, Entity, BoneWorldPositions, Vec3)> = zombies
        .iter()
        .filter_map(|(entity, _, _, history, _)| {
            let (position, rotation, bones) = zombie_history_at(history, rewind_time)?;
            let (distance, _outer_normal) = cylinder.cast_ray(position, rotation, shot.origin, *direction, MAX_SHOT_DISTANCE, true)?;
            // See head_and_neck_positions' own doc comment — needs the
            // zombie's own (rewound) forward direction, not any bone's.
            Some((distance, entity, bones, rotation * Vec3::NEG_Z))
        })
        .collect();
    rewound_candidates.sort_by(|(a, ..), (b, ..)| a.total_cmp(b));

    for (distance, entity, bones, forward) in &rewound_candidates {
        let (entity, forward) = (*entity, *forward);
        info!("[shot] outer capsule hit (rewound): zombie={entity} distance={distance:.2}");
        // Bones ARE rewound to rewind_time here too now (Phase 3) —
        // zombie_history_at reconstructs their world position from that
        // same past instant's root pose + the offsets recorded at that
        // tick, not their live position.
        //
        // zombie_hit_damage_ray, not zombie_hit_damage_bones(hit_point,
        // ...) on a point sampled off the outer capsule's own surface —
        // that surface point can sit well outside every real bone region
        // once ZOMBIE_OUTER_RADIUS is wider than the body (see
        // zombie_hit_damage_ray's own doc comment), so this casts the real
        // ray into each region directly instead.
        let Some((hit_point, normal, damage)) = zombie_hit_damage_ray(shot.origin, direction, MAX_SHOT_DISTANCE, bones, forward) else {
            // A genuine miss of this zombie's real body — no fallback to
            // the outer capsule's surface point, no graze/spark VFX, no
            // damage/knockback for THIS zombie. Continues the loop (next
            // candidate, if any) instead of returning, so a shot that
            // grazed past this one's broad-phase shell can still reach
            // whichever real zombie/wall is behind it.
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
        info!("[shot] inner bone capsule hit: zombie={entity} damage={damage:.1}");
        if let Ok((_, mut health, mut knockback, _, hitbox_slot)) = zombies.get_mut(entity) {
            health.0 -= damage;
            let killed = health.0 <= 0.0;
            if killed {
                let was_director_spawn = director_spawned.contains(entity);
                commands.entity(entity).despawn();
                if !was_director_spawn {
                    // HitboxTestSlot present -> Scenario::HitboxTest zombie:
                    // respawn at its own row slot after
                    // HITBOX_TEST_RESPAWN_DELAY instead of the normal
                    // cycling-grid behavior — see PendingRespawn's own doc
                    // comment.
                    let (timer, fixed_position) = match hitbox_slot {
                        Some(slot) => (HITBOX_TEST_RESPAWN_DELAY, Some(slot.0)),
                        None => (ZOMBIE_RESPAWN_DELAY, None),
                    };
                    respawns.0.push(PendingRespawn { timer, fixed_position });
                }
            } else {
                let push = Vec3::new(shot.direction.x, 0.0, shot.direction.z).normalize_or_zero() * KNOCKBACK_FORCE;
                knockback.0 += push;
            }
            // Excludes the shooter, not a plain Broadcast: their own client
            // already predicted and showed this same hitmarker the instant
            // they fired (see the client's shoot()), so echoing it back
            // would just double it up. Same "weapon prediction" approach
            // Source engine uses for L4D. SurfaceHit isn't predicted
            // client-side, so that one stays a full Broadcast.
            commands.server_trigger(ToClients {
                mode: SendMode::BroadcastExcept(trigger.client_id),
                message: ShotFx::ZombieHit { origin: shot.origin, point: hit_point, normal, damage, killed },
            });
            // The shooter's own floating-damage-number/blood-or-spark choice
            // deliberately does NOT come from their own local prediction
            // (which could guess wrong — see ShotConfirmed's own doc
            // comment) — sent Direct, not part of the BroadcastExcept
            // above, since no other client needs this.
            commands.server_trigger(ToClients {
                mode: SendMode::Direct(trigger.client_id),
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

    // Fallback: the rewound test above found nothing (e.g. a zombie with
    // no history yet, or a genuine miss), but the live world raycast still
    // landed directly on a zombie right now — treat that as an ordinary,
    // un-compensated hit instead of silently treating a zombie as a wall.
    //
    // TODO: unlike the rewound branch above (which tries every zombie
    // candidate in order, nearest first), this only ever considers
    // `hit.entity` — the ONE zombie spatial_query.cast_ray happened to
    // return. If that zombie's outer capsule is hit but its inner bones
    // miss, this branch just falls through to the plain SurfaceHit check
    // below, same "front zombie blocks the shot" bug the rewound branch
    // was just fixed for — a real avian3d physics raycast genuinely stops
    // at the first solid collider it touches, so there's no zombie behind
    // it to fall through to here the way there was in the rewound branch's
    // own (non-physics, manually-iterated) candidate list. Only matters in
    // the brief window before a zombie has ZombieHistory yet (right after
    // spawn) — low priority, fix later if it turns out to matter: re-cast
    // spatial_query with a growing SpatialQueryFilter exclusion set (one
    // more excluded entity per miss) instead of a single cast_ray call.
    if let Ok((entity, mut health, mut knockback, history, hitbox_slot)) = zombies.get_mut(hit.entity) {
        info!("[shot] outer capsule hit (live, no rewind match): zombie={entity} distance={:.2}", hit.distance);
        let (_, rotation, bones) = zombie_history_at(history, time.elapsed_secs())
            .unwrap_or((outer_hit_point, Quat::IDENTITY, BoneWorldPositions::default()));
        // See head_and_neck_positions' own doc comment.
        let forward = rotation * Vec3::NEG_Z;
        // See the rewound branch above's own comment on why this is
        // zombie_hit_damage_ray, not zombie_hit_damage_bones(outer_hit_point,
        // ...), and on why a None here is a genuine miss — no fallback VFX,
        // no damage/knockback. This is the last fallback (unlike the
        // rewound branch, nothing meaningful to fall through to further),
        // so this just returns with no effect at all.
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
        info!("[shot] inner bone capsule hit: zombie={entity} damage={damage:.1}");
        health.0 -= damage;
        let killed = health.0 <= 0.0;
        if killed {
            let was_director_spawn = director_spawned.contains(entity);
            commands.entity(entity).despawn();
            if !was_director_spawn {
                let (timer, fixed_position) = match hitbox_slot {
                    Some(slot) => (HITBOX_TEST_RESPAWN_DELAY, Some(slot.0)),
                    None => (ZOMBIE_RESPAWN_DELAY, None),
                };
                respawns.0.push(PendingRespawn { timer, fixed_position });
            }
        } else {
            let push = Vec3::new(shot.direction.x, 0.0, shot.direction.z).normalize_or_zero() * KNOCKBACK_FORCE;
            knockback.0 += push;
        }
        commands.server_trigger(ToClients {
            mode: SendMode::BroadcastExcept(trigger.client_id),
            message: ShotFx::ZombieHit { origin: shot.origin, point: hit_point, normal, damage, killed },
        });
        commands.server_trigger(ToClients {
            mode: SendMode::Direct(trigger.client_id),
            message: ShotConfirmed { point: hit_point, normal, damage, killed },
        });
        return;
    }

    info!("[shot] hit a surface, no zombie: point={:.2?}", outer_hit_point);
    commands.server_trigger(ToClients {
        mode: SendMode::Broadcast,
        message: ShotFx::SurfaceHit { origin: shot.origin, point: outer_hit_point, normal: hit.normal },
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
    scenario: Res<Scenario>,
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
    // Re-spawn the same population Startup would have for this scenario —
    // see spawn_hitbox_test_zombies/spawn_stress_test_zombies/
    // spawn_initial_zombies' own doc comments.
    match *scenario {
        Scenario::Normal | Scenario::LegacyArena => {
            spawn_zombies(&mut commands, &ZOMBIE_SPAWNS, ZOMBIE_AI_ENABLED, false, false);
        }
        Scenario::HitboxTest => {
            spawn_zombies(&mut commands, &HITBOX_TEST_ZOMBIE_POSITIONS, false, false, true);
        }
        Scenario::StressTest => {
            spawn_zombies(&mut commands, &ZOMBIE_SPAWNS[..STRESS_TEST_ZOMBIE_COUNT], true, false, false);
        }
        Scenario::RenderCompare => spawn_render_compare_zombies(&mut commands),
        // No zombies/gameplay — pure walk-around scale-reference rig.
        Scenario::ItemGallery => {}
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
        mode: SendMode::Broadcast,
        message: GameStateEvent::Restarted,
    });
}

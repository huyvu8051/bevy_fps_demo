//! Bakes a map's navmesh offline and writes it next to the map's assets.
//!
//! `MAP_ID=1 cargo run -p server --bin navmesh_baker`
//!
//! Baking is never done at server startup: voxelizing a town-sized map takes
//! far too long to sit in the boot path, so the `.nav` files are built here and
//! shipped as assets.

use avian3d::prelude::*;
use avian_rerecast::prelude::*;
use bevy::ecs::system::SystemState;
use bevy::math::bounding::Aabb3d;
use bevy::prelude::*;
use bevy_rerecast::prelude::*;
use bevy_rerecast::NavmeshBackend;
use server::map_colliders::spawn_map_colliders;
use server::navmesh::bake_settings;
use shared::Scenario;

/// Colliders trickle in as their glTF scenes load, so bake once the count has
/// held steady for this long. This is wall-clock rather than a frame count
/// because the baker runs its schedule as fast as it can, which would burn
/// through any frame budget long before the first asset lands.
const STABLE_SECS: f32 = 2.0;

/// The desert model carries shells and stray geometry hundreds of units above
/// and below the town, which would bake into navmesh layers no zombie can
/// reach. The geometry's own bounds are intersected with these limits rather
/// than replaced by them, so a small map still bakes at full resolution.
const MAX_HALF_EXTENT_XZ: f32 = 200.0;
const MIN_Y: f32 = -2.0;
const MAX_Y: f32 = 40.0;

#[derive(Resource, Default)]
struct BakeProgress {
    last_count: usize,
    stable_secs: f32,
    started: bool,
}

#[derive(Resource)]
struct Baking(Handle<Navmesh>);

fn main() {
    let scenario = std::env::var("MAP_ID")
        .ok()
        .and_then(|s| s.parse::<u8>().ok())
        .map(Scenario::from_map_id)
        .unwrap_or_default();

    eprintln!("[baker] {scenario:?} -> {}", scenario.baked_navmesh_path());

    App::new()
        .insert_resource(scenario)
        .add_plugins((
            MinimalPlugins,
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
            PhysicsPlugins::default(),
            NavmeshPlugins::default(),
            AvianBackendPlugin::default(),
        ))
        .init_asset::<StandardMaterial>()
        .init_resource::<BakeProgress>()
        .add_systems(Startup, spawn_colliders)
        .add_systems(Update, start_bake)
        .add_observer(write_navmesh)
        .run();
}

fn spawn_colliders(mut commands: Commands, scenario: Res<Scenario>, asset_server: Res<AssetServer>) {
    spawn_map_colliders(&mut commands, *scenario, &asset_server);
}

fn start_bake(world: &mut World) {
    if !colliders_settled(world) {
        return;
    }

    let mut settings = bake_settings();
    settings.aabb = Some(clamped_aabb(world, &settings));

    let mut state: SystemState<NavmeshGenerator> = SystemState::new(world);
    let handle = {
        let mut generator = state.get_mut(world).expect("failed to validate NavmeshGenerator");
        generator.generate(settings)
    };
    state.apply(world);
    world.insert_resource(Baking(handle));
}

fn colliders_settled(world: &mut World) -> bool {
    let delta = world.resource::<Time>().delta_secs();
    let count = world.query_filtered::<(), With<Collider>>().iter(world).count();

    let mut progress = world.resource_mut::<BakeProgress>();
    if progress.started {
        return false;
    }
    if count == progress.last_count {
        progress.stable_secs += delta;
    } else {
        progress.last_count = count;
        progress.stable_secs = 0.0;
    }
    // Zero means the scene has not loaded yet, not that the level has settled.
    if count == 0 || progress.stable_secs < STABLE_SECS {
        return false;
    }
    progress.started = true;
    info!("baking from {count} colliders, this can take a while...");
    true
}

fn clamped_aabb(world: &mut World, settings: &bevy_rerecast::NavmeshSettings) -> Aabb3d {
    let backend = world.resource::<NavmeshBackend>().0;
    let trimesh = world
        .run_system_with(backend, settings.clone())
        .expect("navmesh backend system failed");
    let full = trimesh.compute_aabb().expect("no geometry to bake");
    let min = Vec3::new(
        full.min.x.max(-MAX_HALF_EXTENT_XZ),
        full.min.y.max(MIN_Y),
        full.min.z.max(-MAX_HALF_EXTENT_XZ),
    );
    let max = Vec3::new(
        full.max.x.min(MAX_HALF_EXTENT_XZ),
        full.max.y.min(MAX_Y),
        full.max.z.min(MAX_HALF_EXTENT_XZ),
    );
    info!("geometry {:.1?}..{:.1?} clamped to {min:.1?}..{max:.1?}", full.min, full.max);
    Aabb3d { min: min.into(), max: max.into() }
}

fn write_navmesh(
    trigger: On<NavmeshReady>,
    baking: Res<Baking>,
    navmeshes: Res<Assets<Navmesh>>,
    scenario: Res<Scenario>,
) {
    if trigger.event().0 != baking.0.id() {
        return;
    }
    let navmesh = navmeshes.get(baking.0.id()).expect("navmesh ready but asset missing");
    let polygons = navmesh.polygon.polygon_count();
    if polygons == 0 {
        error!("bake produced an empty navmesh; leaving the existing file alone");
        std::process::exit(1);
    }

    let bytes = bincode::serde::encode_to_vec(navmesh, bincode::config::standard())
        .expect("failed to encode navmesh");
    let path = format!(
        "{}/../client/assets/{}",
        env!("CARGO_MANIFEST_DIR"),
        scenario.baked_navmesh_path()
    );
    if let Some(parent) = std::path::Path::new(&path).parent() {
        std::fs::create_dir_all(parent).expect("failed to create navmesh directory");
    }
    std::fs::write(&path, bytes).expect("failed to write navmesh");
    info!("wrote {path}: {polygons} polygons, bounds {:.1?}", navmesh.polygon.aabb);
    std::process::exit(0);
}

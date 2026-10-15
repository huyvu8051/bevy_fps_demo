use bevy::prelude::*;

use crate::*;
use shared::zombie::ZOMBIE_REST_HEIGHT;

#[derive(Resource, Default)]
pub(crate) struct NextZombieSpawnIndex(pub(crate) usize);

pub(crate) fn zombie_spawn_pos_for(scenario: Scenario, seed: usize, points: &[Vec3]) -> (Vec3, f32) {
    if points.is_empty() {
        return (scenario.player_spawn(), 0.0);
    }
    (points[seed % points.len()] + Vec3::Y * ZOMBIE_REST_HEIGHT, 0.0)
}

#[derive(Resource, Default)]
pub(crate) struct ZombieSpawnPoints(pub(crate) Vec<Vec3>);

pub(crate) fn load_zombie_spawn_points(scenario: Res<Scenario>, mut commands: Commands) {
    let points = shared::zombie_spawn_points(*scenario);
    info!("{}: {} authored zombie spawn point(s)", scenario.title(), points.len());
    commands.insert_resource(ZombieSpawnPoints(points));
}

#[derive(Component)]
pub(crate) struct MapRoot;

#[derive(Resource, Default)]
pub(crate) struct WorldReady(pub(crate) bool);

pub(crate) fn world_ready(ready: Res<WorldReady>) -> bool {
    ready.0
}

pub(crate) fn track_world_ready(
    asset_server: Res<AssetServer>,
    map: Query<&WorldAssetRoot, With<MapRoot>>,
    pending_colliders: Query<(), With<ColliderConstructorHierarchy>>,
    mut ready: ResMut<WorldReady>,
) {
    if ready.0 {
        return;
    }
    let Ok(root) = map.single() else {
        return;
    };
    if !asset_server.is_loaded_with_dependencies(&root.0) {
        return;
    }
    if !pending_colliders.is_empty() {
        return;
    }
    ready.0 = true;
    info!("[world] map loaded and colliders built — starting simulation");
}


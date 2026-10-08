use avian3d::prelude::*;
use bevy::prelude::*;
use shared::Scenario;

/// The physics bodies the server simulates against and the navmesh baker
/// voxelizes. Both must see the same geometry, so both call this.
pub fn spawn_map_colliders(commands: &mut Commands, scenario: Scenario, asset_server: &AssetServer) {
    match scenario {
        Scenario::Desert => spawn_desert_colliders(commands, asset_server, scenario),
        Scenario::StressTest => {
            spawn_flat_arena_colliders(commands, scenario.map_id());
            spawn_arena_car_colliders(commands, asset_server);
        }
    }
}

fn spawn_desert_colliders(commands: &mut Commands, asset_server: &AssetServer, scenario: Scenario) {
    commands.spawn((
        Name::new("DesertCity"),
        WorldAssetRoot(asset_server.load(
            GltfAssetLabel::Scene(0)
                .from_asset(scenario.scene_path().expect("the desert is a glTF map")),
        )),
        Transform::IDENTITY,
        shared::scene_colliders(),
    ));
}

fn spawn_flat_arena_colliders(commands: &mut Commands, map_id: u8) {
    for b in shared::arena_layout(map_id) {
        let collider = match b.shape {
            shared::ArenaShape::Cuboid => Collider::cuboid(b.size.x, b.size.y, b.size.z),
            shared::ArenaShape::Cylinder => Collider::cylinder(b.size.x * 0.5, b.size.y),
        };
        commands.spawn((
            Transform::from_translation(b.translation).with_rotation(b.rotation),
            RigidBody::Static,
            collider,
        ));
    }
}

fn spawn_arena_car_colliders(commands: &mut Commands, asset_server: &AssetServer) {
    for (path, physics) in shared::ARENA_CAR_PATHS.iter().zip(shared::arena_car_physics()) {
        commands.spawn((
            WorldAssetRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(*path))),
            Transform::from_xyz(physics.x, physics.y, physics.z)
                .with_rotation(physics.rotation)
                .with_scale(Vec3::splat(physics.scale)),
            RigidBody::Static,
            ColliderConstructorHierarchy::new(ColliderConstructor::ConvexHullFromMesh),
        ));
    }
}

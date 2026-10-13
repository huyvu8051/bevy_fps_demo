use avian3d::prelude::*;
use bevy::prelude::*;
use shared::Scenario;

pub fn spawn_map_colliders(commands: &mut Commands, scenario: Scenario, asset_server: &AssetServer) {
    match scenario {
        Scenario::Desert => spawn_gltf_scene_colliders(commands, asset_server, scenario),
        Scenario::StressTest => {
            spawn_arena_colliders(commands, scenario.map_id());
        }
    }
}

fn spawn_gltf_scene_colliders(
    commands: &mut Commands,
    asset_server: &AssetServer,
    scenario: Scenario,
) {
    let scene = scenario.require_gltf_scene_path();
    commands.spawn((
        Name::new(scenario.title()),
        WorldAssetRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(scene))),
        Transform::IDENTITY,
        shared::gltf_scene_colliders(),
    ));
}

fn spawn_arena_colliders(commands: &mut Commands, map_id: u8) {
    for piece in shared::arena_layout(map_id) {
        let collider = match piece.shape {
            shared::ArenaShape::Cuboid => {
                Collider::cuboid(piece.size.x, piece.size.y, piece.size.z)
            }
            shared::ArenaShape::Cylinder => Collider::cylinder(piece.size.x * 0.5, piece.size.y),
        };
        commands.spawn((
            Transform::from_translation(piece.translation).with_rotation(piece.rotation),
            RigidBody::Static,
            collider,
        ));
    }
}

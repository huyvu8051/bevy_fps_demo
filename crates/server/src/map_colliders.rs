use bevy::prelude::*;
use shared::Scenario;

pub fn spawn_map_colliders(commands: &mut Commands, scenario: Scenario, asset_server: &AssetServer) {
    let scene = scenario.require_gltf_scene_path();
    commands.spawn((
        Name::new(scenario.title()),
        WorldAssetRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(scene))),
        Transform::IDENTITY,
        shared::gltf_scene_colliders(),
    ));
}

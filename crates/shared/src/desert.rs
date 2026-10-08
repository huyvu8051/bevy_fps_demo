use bevy::prelude::Vec3;

/// Sketchfab "Desert_City" by Mikey x pc, CC-BY-4.0; see the map's
/// `license.txt` for the required credit.
pub const DESERT_SCENE_PATH: &str = "maps/desert_city/scene.gltf";

pub const DESERT_NAVMESH_PATH: &str = "maps/desert_city/navmesh.nav";

pub const DESERT_PLAYER_SPAWN: Vec3 = Vec3::new(0.0, 3.0, 0.0);

/// The town is third-party geometry with no spawn markers, so zombie spawns
/// are sampled from the baked navmesh within this band around the player.
pub const DESERT_SPAWN_MIN_RADIUS: f32 = 8.0;
pub const DESERT_SPAWN_MAX_RADIUS: f32 = 45.0;

/// Keeps spawns off rooftops and out of cellars, which the navmesh also covers.
pub const DESERT_SPAWN_MAX_RISE: f32 = 3.0;

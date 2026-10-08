//! Everything that differs between maps, in one place.
//!
//! Adding a map means adding a variant and filling in the `match` arms below —
//! the compiler names every one it still needs. Two call sites still live
//! elsewhere, because spawning is engine work rather than map data:
//! `server::map_colliders` and `client::scene_setup`. Both match on
//! [`Scenario`], so neither compiles until the new map is handled, and both
//! build their physics from [`scene_colliders`] so they cannot disagree about
//! where the ground is.

use avian3d::prelude::{ColliderConstructor, ColliderConstructorHierarchy, RigidBody};
use bevy::prelude::*;

use crate::player::PLAYER_SPAWN;

/// Port is derived, so a map's id picks its server port: `5031 + id`.
pub const SERVER_PORT_BASE: u16 = 5031;

#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Scenario {
    Desert,

    #[default]
    StressTest,
}

impl Scenario {
    /// Every map, in map-select order. The entries carry no data of their own;
    /// ids, labels and assets all come from the methods below.
    pub const ALL: [Scenario; 2] = [Scenario::Desert, Scenario::StressTest];

    pub fn from_map_id(id: u8) -> Self {
        Self::ALL.into_iter().find(|s| s.map_id() == id).unwrap_or_default()
    }

    pub fn map_id(self) -> u8 {
        match self {
            Scenario::Desert => 1,
            Scenario::StressTest => 2,
        }
    }

    pub fn port(self) -> u16 {
        SERVER_PORT_BASE + self.map_id() as u16
    }

    pub fn title(self) -> &'static str {
        match self {
            Scenario::Desert => "DESERT CITY",
            Scenario::StressTest => "ARENA",
        }
    }

    pub fn subtitle(self) -> &'static str {
        match self {
            Scenario::Desert => "OPEN DESERT TOWN, BAKED NAVMESH",
            Scenario::StressTest => "OPEN FLAT ARENA, CHASE-AI ZOMBIES",
        }
    }

    pub fn player_spawn(self) -> Vec3 {
        match self {
            Scenario::Desert => DESERT_PLAYER_SPAWN,
            Scenario::StressTest => PLAYER_SPAWN,
        }
    }

    /// Navmeshes are baked offline by the `navmesh_baker` binary and shipped as
    /// assets, so a town-sized map never has to voxelize at startup. Re-bake
    /// whenever the map's geometry changes, or the navmesh silently drifts.
    pub fn baked_navmesh_path(self) -> &'static str {
        match self {
            Scenario::Desert => "maps/desert_city/navmesh.nav",
            Scenario::StressTest => "maps/arena/navmesh.nav",
        }
    }

    /// `None` for maps built from code or RON rather than a glTF scene.
    pub fn scene_path(self) -> Option<&'static str> {
        match self {
            Scenario::Desert => Some("maps/desert_city/scene.gltf"),
            Scenario::StressTest => None,
        }
    }

    pub fn zombie_count(self) -> usize {
        match self {
            Scenario::Desert | Scenario::StressTest => 10,
        }
    }
}

/// Physics for a glTF map, built from its meshes.
///
/// The client predicts movement against its own colliders while the server is
/// the authority, so both must spawn this alongside the scene. Give the client
/// only the visuals and the player falls through the world.
pub fn scene_colliders() -> (RigidBody, ColliderConstructorHierarchy) {
    (
        RigidBody::Static,
        ColliderConstructorHierarchy::new(ColliderConstructor::TrimeshFromMesh),
    )
}

/// Sketchfab "Desert_City" by Mikey x pc, CC-BY-4.0; see the map's
/// `license.txt` for the required credit.
const DESERT_PLAYER_SPAWN: Vec3 = Vec3::new(0.0, 3.0, 0.0);

/// The desert town is third-party geometry with no spawn markers, so zombie
/// spawns are sampled from the baked navmesh within this band around the
/// player, restricted to the part of the mesh the player can actually reach.
pub const DESERT_SPAWN_MIN_RADIUS: f32 = 8.0;
pub const DESERT_SPAWN_MAX_RADIUS: f32 = 45.0;

/// Keeps spawns off rooftops and out of cellars, which the navmesh also covers.
pub const DESERT_SPAWN_MAX_RISE: f32 = 3.0;

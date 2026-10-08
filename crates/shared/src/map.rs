use avian3d::prelude::{ColliderConstructor, ColliderConstructorHierarchy, RigidBody};
use bevy::prelude::*;

use crate::player::PLAYER_SPAWN;

pub const SERVER_PORT_BASE: u16 = 5031;

pub const DESERT_SPAWN_MIN_RADIUS: f32 = 8.0;
pub const DESERT_SPAWN_MAX_RADIUS: f32 = 45.0;
pub const DESERT_SPAWN_MAX_RISE_ABOVE_PLAYER: f32 = 3.0;

macro_rules! scenarios {
    ($(
        $(#[$attr:meta])*
        $variant:ident {
            id: $id:literal,
            title: $title:literal,
            subtitle: $subtitle:literal,
            navmesh: $navmesh:literal,
            scene: $scene:expr,
            player_spawn: $spawn:expr,
            zombies: $zombies:literal,
        }
    )*) => {
        #[derive(Resource, Clone, Copy, PartialEq, Eq, Debug, Default)]
        pub enum Scenario {
            $( $(#[$attr])* $variant, )*
        }

        impl Scenario {
            pub const ALL: &'static [Scenario] = &[$(Scenario::$variant),*];

            pub fn map_id(self) -> u8 {
                match self { $(Scenario::$variant => $id,)* }
            }

            pub fn title(self) -> &'static str {
                match self { $(Scenario::$variant => $title,)* }
            }

            pub fn subtitle(self) -> &'static str {
                match self { $(Scenario::$variant => $subtitle,)* }
            }

            pub fn baked_navmesh_path(self) -> &'static str {
                match self { $(Scenario::$variant => $navmesh,)* }
            }

            pub fn gltf_scene_path(self) -> Option<&'static str> {
                match self { $(Scenario::$variant => $scene,)* }
            }

            pub fn player_spawn(self) -> Vec3 {
                match self { $(Scenario::$variant => $spawn,)* }
            }

            pub fn zombie_count(self) -> usize {
                match self { $(Scenario::$variant => $zombies,)* }
            }
        }
    };
}

scenarios! {
    Desert {
        id: 1,
        title: "DESERT CITY",
        subtitle: "OPEN DESERT TOWN, BAKED NAVMESH",
        navmesh: "maps/desert_city/navmesh.nav",
        scene: Some("maps/desert_city/scene.gltf"),
        player_spawn: Vec3::new(0.0, 3.0, 0.0),
        zombies: 10,
    }

    #[default]
    StressTest {
        id: 2,
        title: "ARENA",
        subtitle: "OPEN FLAT ARENA, CHASE-AI ZOMBIES",
        navmesh: "maps/arena/navmesh.nav",
        scene: None,
        player_spawn: PLAYER_SPAWN,
        zombies: 10,
    }
}

impl Scenario {
    pub fn from_map_id(id: u8) -> Self {
        Self::ALL.iter().copied().find(|s| s.map_id() == id).unwrap_or_default()
    }

    pub fn port(self) -> u16 {
        SERVER_PORT_BASE + self.map_id() as u16
    }

    pub fn require_gltf_scene_path(self) -> &'static str {
        self.gltf_scene_path().unwrap_or_else(|| panic!("{self:?} is not a glTF map"))
    }
}

pub fn gltf_scene_colliders() -> (RigidBody, ColliderConstructorHierarchy) {
    (
        RigidBody::Static,
        ColliderConstructorHierarchy::new(ColliderConstructor::TrimeshFromMesh),
    )
}

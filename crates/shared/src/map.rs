use avian3d::prelude::{ColliderConstructor, ColliderConstructorHierarchy, RigidBody};
use bevy::prelude::*;

pub const SERVER_PORT_BASE: u16 = 5031;

/// Upper bound for the zombie-count control. Previously the length of a hardcoded spawn
/// grid; spawn positions now come from each map's authored `zombie_spawns.ron`, which can be
/// any length, so the cap is its own number.
pub const ZOMBIE_MAX_COUNT: u32 = 100;

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
    #[default]
    Desert {
        id: 1,
        title: "DESERT CITY",
        subtitle: "OPEN DESERT TOWN, BAKED NAVMESH",
        navmesh: "maps/desert_city/navmesh.nav",
        scene: Some("maps/desert_city/scene.gltf"),
        player_spawn: Vec3::new(0.0, 3.0, 0.0),
        zombies: 10,
    }

    Garage {
        id: 3,
        title: "PARKING GARAGE",
        subtitle: "UNDERGROUND TEST MAP, BAKED NAVMESH",
        navmesh: "maps/basement_parking_garage/navmesh.nav",
        scene: Some("maps/basement_parking_garage/scene.glb"),
        player_spawn: Vec3::new(21.2, 3.0, 4.6),
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

    /// The map's asset directory, e.g. `maps/desert_city` — derived from the navmesh path so
    /// every per-map data file (navmesh chunks, zombie spawns) has one place to live.
    pub fn map_asset_dir(self) -> &'static str {
        let path = self.baked_navmesh_path();
        path.rsplit_once('/')
            .unwrap_or_else(|| panic!("{self:?}'s navmesh path {path:?} has no directory component"))
            .0
    }
}

/// Zombie spawn positions, authored in mapbake_editor and saved as `zombie_spawns.ron` in
/// the map's asset directory. This is the only source of spawn positions: deriving them from
/// the navmesh at runtime was dropped because it regularly placed zombies inside walls.
///
/// A map with no authored spawns yields an empty list rather than panicking — that's a
/// content gap to fill in the editor, not a reason to take the server down on boot.
pub fn zombie_spawn_points(scenario: Scenario) -> Vec<Vec3> {
    let relative = format!("{}/zombie_spawns.ron", scenario.map_asset_dir());

    #[cfg(target_os = "ios")]
    let path = {
        let Some(resource_path) = crate::ios_bundle_resource_path() else {
            error!("could not read {relative}: NSBundle.main.resourcePath was nil");
            return Vec::new();
        };
        std::path::Path::new(&resource_path).join("assets").join(&relative)
    };
    #[cfg(not(target_os = "ios"))]
    let path = std::path::PathBuf::from(format!(
        "{}/../client/assets/{relative}",
        env!("CARGO_MANIFEST_DIR")
    ));

    let Ok(text) = std::fs::read_to_string(&path) else {
        error!("no authored zombie spawns at {path:?} — author them in mapbake_editor");
        return Vec::new();
    };
    match ron::de::from_str::<Vec<[f32; 3]>>(&text) {
        Ok(raw) => raw.into_iter().map(Vec3::from).collect(),
        Err(e) => {
            error!("failed to parse {path:?}: {e}");
            Vec::new()
        }
    }
}

pub fn gltf_scene_colliders() -> (RigidBody, ColliderConstructorHierarchy) {
    (
        RigidBody::Static,
        ColliderConstructorHierarchy::new(ColliderConstructor::TrimeshFromMesh),
    )
}

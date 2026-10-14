use avian3d::prelude::{ColliderConstructor, ColliderConstructorHierarchy, RigidBody};
use bevy::prelude::*;

pub const SERVER_PORT_BASE: u16 = 5031;

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
        zombies: 10,
    }

    Garage {
        id: 3,
        title: "PARKING GARAGE",
        subtitle: "UNDERGROUND TEST MAP, BAKED NAVMESH",
        navmesh: "maps/basement_parking_garage/navmesh.nav",
        scene: Some("maps/basement_parking_garage/scene.glb"),
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

    pub fn map_asset_dir(self) -> &'static str {
        let path = self.baked_navmesh_path();
        path.rsplit_once('/')
            .unwrap_or_else(|| panic!("{self:?}'s navmesh path {path:?} has no directory component"))
            .0
    }

    pub fn player_spawn(self) -> Vec3 {
        let floor = player_spawn_floor_point(self).unwrap_or_else(|| {
            panic!(
                "{:?} has no player_spawn.ron in {} — set the player spawn in mapbake_editor",
                self,
                self.map_asset_dir()
            )
        });
        floor + Vec3::Y * crate::PLAYER_REST_HEIGHT
    }
}

pub fn player_spawn_floor_point(scenario: Scenario) -> Option<Vec3> {
    let relative = format!("{}/player_spawn.ron", scenario.map_asset_dir());
    let path = asset_file_path(&relative)?;
    let text = std::fs::read_to_string(&path).ok()?;
    match ron::de::from_str::<[f32; 3]>(&text) {
        Ok(raw) => Some(Vec3::from(raw)),
        Err(e) => {
            error!("failed to parse {path:?}: {e}");
            None
        }
    }
}

fn asset_file_path(relative: &str) -> Option<std::path::PathBuf> {
    #[cfg(target_os = "ios")]
    {
        let Some(resource_path) = crate::ios_bundle_resource_path() else {
            error!("could not resolve {relative}: NSBundle.main.resourcePath was nil");
            return None;
        };
        Some(std::path::Path::new(&resource_path).join("assets").join(relative))
    }
    #[cfg(not(target_os = "ios"))]
    Some(std::path::PathBuf::from(format!(
        "{}/../client/assets/{relative}",
        env!("CARGO_MANIFEST_DIR")
    )))
}

pub fn navmesh_chunk_paths(scenario: Scenario) -> Vec<String> {
    let asset_dir = scenario.map_asset_dir();
    let Some(dir) = asset_file_path(asset_dir) else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        error!("could not read navmesh directory {dir:?}");
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| entry.file_name().to_str().map(str::to_string))
        .filter(|name| name.starts_with("navmesh") && name.ends_with(".nav"))
        .collect();
    names.sort();
    if names.is_empty() {
        error!("no navmesh*.nav found in {dir:?} — bake the map in mapbake_editor");
    }
    names.into_iter().map(|name| format!("{asset_dir}/{name}")).collect()
}

pub fn zombie_spawn_points(scenario: Scenario) -> Vec<Vec3> {
    let relative = format!("{}/zombie_spawns.ron", scenario.map_asset_dir());
    let Some(path) = asset_file_path(&relative) else {
        return Vec::new();
    };

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

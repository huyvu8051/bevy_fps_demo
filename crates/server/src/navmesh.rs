use bevy::prelude::*;
use bevy_landmass::prelude::*;
use bevy_landmass::{Agent3d, ArchipelagoOptions, PointSampleDistance3d};
use bevy_rerecast::prelude::*;
use bevy_rerecast::NavmeshSettings;
use landmass_rerecast::{Island3dBundle, LandmassRerecastPlugin, NavMeshHandle3d};
use shared::zombie::{
    ZOMBIE_LENGTH, ZOMBIE_LOD_REST_HEIGHT, ZOMBIE_RADIUS, ZOMBIE_RUN_SPEED, ZOMBIE_WALK_SPEED,
};
use shared::Scenario;

/// Tallest step a zombie can walk up without the bake treating it as a wall.
const WALKABLE_CLIMB: f32 = 0.5;

/// Cells per agent radius; cell size is `agent_radius / fraction`. A zombie's
/// radius is small, so the default resolution both takes impractically long on
/// a town-sized map and merges into polygons landmass rejects as concave.
const CELL_SIZE_FRACTION: f32 = 1.0;

pub fn agent_height() -> f32 {
    ZOMBIE_LENGTH + ZOMBIE_RADIUS * 2.0
}

/// Used by the `navmesh_baker` binary. Navmeshes are baked offline and shipped
/// as `.nav` assets; nothing is generated at server startup.
pub fn bake_settings() -> NavmeshSettings {
    NavmeshSettings {
        walkable_climb: WALKABLE_CLIMB,
        cell_size_fraction: CELL_SIZE_FRACTION,
        ..NavmeshSettings::from_agent_3d(ZOMBIE_RADIUS, agent_height())
    }
}

pub fn archipelago() -> Archipelago3d {
    Archipelago3d::new(ArchipelagoOptions {
        point_sample_distance: PointSampleDistance3d {
            distance_above: 1.0,
            // A zombie's transform sits at the capsule's centre while the
            // navmesh lies at its feet, so sampling has to reach a full
            // half-height down or every agent reads as off-mesh.
            distance_below: ZOMBIE_LOD_REST_HEIGHT + 1.0,
            ..PointSampleDistance3d::from_agent_radius(ZOMBIE_RADIUS)
        },
        ..ArchipelagoOptions::from_agent_radius(ZOMBIE_RADIUS)
    })
}

/// The archipelago every zombie agent is attached to.
#[derive(Resource)]
pub struct ZombieArchipelago(pub Entity);

/// Reads the baked `.nav` straight off disk.
///
/// `Assets<Navmesh>` is not an option here: `landmass_rerecast` consumes the
/// loaded asset when it converts it, so by the time anything else looks the
/// entry is gone.
pub fn read_baked_navmesh(scenario: Scenario) -> Option<Navmesh> {
    let path = format!(
        "{}/../client/assets/{}",
        env!("CARGO_MANIFEST_DIR"),
        scenario.baked_navmesh_path()
    );
    let bytes = std::fs::read(&path)
        .inspect_err(|e| error!("could not read {path}: {e}"))
        .ok()?;
    bincode::serde::decode_from_slice(&bytes, bincode::config::standard())
        .inspect_err(|e| error!("could not decode {path}: {e}"))
        .ok()
        .map(|(navmesh, _)| navmesh)
}

pub struct ServerNavmeshPlugin;

impl Plugin for ServerNavmeshPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            NavmeshPlugins::default(),
            Landmass3dPlugin::default(),
            LandmassRerecastPlugin::default(),
        ))
        .add_systems(Startup, load_baked_navmesh)
        .add_systems(Update, register_zombie_agents.before(LandmassSystems::SyncExistence));
    }
}

fn load_baked_navmesh(scenario: Res<Scenario>, asset_server: Res<AssetServer>, mut commands: Commands) {
    let path = scenario.baked_navmesh_path();
    let archipelago_entity = commands.spawn(archipelago()).id();
    commands.spawn(Island3dBundle {
        island: Island,
        archipelago_ref: ArchipelagoRef3d::new(archipelago_entity),
        nav_mesh: NavMeshHandle3d(asset_server.load(path)),
    });
    commands.insert_resource(ZombieArchipelago(archipelago_entity));
    info!("loading baked navmesh {path}");
}

fn register_zombie_agents(
    archipelago: Res<ZombieArchipelago>,
    zombies: Query<Entity, (With<shared::Zombie>, Without<Agent3d>)>,
    mut commands: Commands,
) {
    for entity in &zombies {
        commands.entity(entity).insert((
            Agent3dBundle {
                agent: Agent3d::default(),
                settings: AgentSettings {
                    radius: ZOMBIE_RADIUS,
                    desired_speed: ZOMBIE_WALK_SPEED,
                    max_speed: ZOMBIE_RUN_SPEED,
                },
                archipelago_ref: ArchipelagoRef3d::new(archipelago.0),
            },
            AgentTarget3d::None,
        ));
    }
}

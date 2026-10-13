use bevy::prelude::*;
use bevy_landmass::prelude::*;
use bevy_landmass::{Agent3d, ArchipelagoOptions, PointSampleDistance3d};
use bevy_rerecast::prelude::*;
use bevy_rerecast::NavmeshSettings;
use landmass_rerecast::{Island3dBundle, LandmassRerecastPlugin, NavMeshHandle3d};
use shared::zombie::{
    ZOMBIE_LENGTH, ZOMBIE_REST_HEIGHT, ZOMBIE_RADIUS, ZOMBIE_RUN_SPEED, ZOMBIE_WALK_SPEED,
};
use shared::Scenario;

const WALKABLE_CLIMB: f32 = 0.5;
const CELL_SIZE_FRACTION: f32 = 1.0;
const MIN_REGION_SIZE: u16 = 32;
const MERGE_REGION_SIZE: u16 = 80;
const NAVMESH_SAMPLE_DISTANCE_ABOVE: f32 = 1.0;
const NAVMESH_SAMPLE_SLACK_BELOW_FEET: f32 = 1.0;

#[derive(Resource)]
pub struct ZombieArchipelago(pub Entity);

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

pub fn agent_height() -> f32 {
    ZOMBIE_LENGTH + ZOMBIE_RADIUS * 2.0
}

pub fn bake_settings() -> NavmeshSettings {
    NavmeshSettings {
        walkable_climb: WALKABLE_CLIMB,
        walkable_slope_angle: shared::MAX_WALKABLE_SLOPE_DEGREES.to_radians(),
        min_region_size: MIN_REGION_SIZE,
        merge_region_size: MERGE_REGION_SIZE,
        cell_size_fraction: CELL_SIZE_FRACTION,
        ..NavmeshSettings::from_agent_3d(ZOMBIE_RADIUS, agent_height())
    }
}

fn zombie_archipelago() -> Archipelago3d {
    Archipelago3d::new(ArchipelagoOptions {
        point_sample_distance: PointSampleDistance3d {
            distance_above: NAVMESH_SAMPLE_DISTANCE_ABOVE,
            distance_below: ZOMBIE_REST_HEIGHT + NAVMESH_SAMPLE_SLACK_BELOW_FEET,
            ..PointSampleDistance3d::from_agent_radius(ZOMBIE_RADIUS)
        },
        ..ArchipelagoOptions::from_agent_radius(ZOMBIE_RADIUS)
    })
}

/// mapbake_editor can bake a map in pieces, each piece landing as its own `navmesh_N.nav`
/// (see mapbake_editor's bake.rs). Loading every `navmesh*.nav` in the map's directory as a
/// separate landmass Island under one Archipelago — rather than a single fixed filename —
/// means those pieces don't need merging into one file: landmass stitches adjacent islands
/// together at query time. Older maps that still ship a single bare `navmesh.nav` keep
/// working unchanged, since that filename matches the same glob.
fn load_baked_navmesh(
    scenario: Res<Scenario>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    let declared_path = scenario.baked_navmesh_path();
    let Some((asset_dir, _)) = declared_path.rsplit_once('/') else {
        panic!("{scenario:?}'s baked_navmesh_path {declared_path:?} has no directory component");
    };
    let fs_dir = std::path::Path::new(&baked_navmesh_file(*scenario))
        .parent()
        .expect("baked_navmesh_file always has a parent directory")
        .to_path_buf();

    let mut chunk_files: Vec<String> = std::fs::read_dir(&fs_dir)
        .unwrap_or_else(|e| panic!("could not read navmesh directory {fs_dir:?}: {e}"))
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| entry.file_name().to_str().map(str::to_string))
        .filter(|name| name.starts_with("navmesh") && name.ends_with(".nav"))
        .collect();
    chunk_files.sort();

    if chunk_files.is_empty() {
        panic!("no navmesh*.nav files found in {fs_dir:?} for {scenario:?}");
    }

    let archipelago = commands.spawn(zombie_archipelago()).id();
    for file_name in &chunk_files {
        let path = format!("{asset_dir}/{file_name}");
        commands.spawn(Island3dBundle {
            island: Island,
            archipelago_ref: ArchipelagoRef3d::new(archipelago),
            nav_mesh: NavMeshHandle3d(asset_server.load(&path)),
        });
        info!("loading baked navmesh chunk {path}");
    }
    commands.insert_resource(ZombieArchipelago(archipelago));
    info!("{}: {} navmesh chunk(s) loaded as islands", scenario.title(), chunk_files.len());
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

pub fn baked_navmesh_file(scenario: Scenario) -> String {
    format!(
        "{}/../client/assets/{}",
        env!("CARGO_MANIFEST_DIR"),
        scenario.baked_navmesh_path()
    )
}

pub fn connectivity(navmesh: &Navmesh) -> (usize, f32) {
    let polygons = &navmesh.polygon;
    let vertices_per_polygon = polygons.max_vertices_per_polygon as usize;
    let polygon_count = polygons.polygon_count();
    if vertices_per_polygon == 0 || polygon_count == 0 {
        return (0, 0.0);
    }

    let mut component_of = vec![usize::MAX; polygon_count];
    let mut components = 0;
    let mut largest = 0;
    for seed in 0..polygon_count {
        if component_of[seed] != usize::MAX {
            continue;
        }
        let mut size = 0;
        let mut queue = std::collections::VecDeque::from([seed]);
        component_of[seed] = components;
        while let Some(polygon) = queue.pop_front() {
            size += 1;
            let edges = polygon * vertices_per_polygon..(polygon + 1) * vertices_per_polygon;
            for &neighbour in &polygons.polygon_neighbors[edges] {
                let neighbour = neighbour as usize;
                if neighbour < polygon_count && component_of[neighbour] == usize::MAX {
                    component_of[neighbour] = components;
                    queue.push_back(neighbour);
                }
            }
        }
        largest = largest.max(size);
        components += 1;
    }
    (components, largest as f32 / polygon_count as f32)
}

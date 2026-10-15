use bevy::prelude::*;
use bevy_landmass::prelude::*;
use bevy_landmass::{Agent3d, ArchipelagoOptions, PointSampleDistance3d};
use bevy_rerecast::prelude::*;
use landmass_rerecast::{Island3dBundle, NavMeshHandle3d};
use shared::zombie::{ZOMBIE_REST_HEIGHT, ZOMBIE_RADIUS, ZOMBIE_RUN_SPEED, ZOMBIE_WALK_SPEED};
use shared::Scenario;

const NAVMESH_SAMPLE_DISTANCE_ABOVE: f32 = 1.0;
const NAVMESH_SAMPLE_SLACK_BELOW_FEET: f32 = 1.0;

#[derive(Resource)]
pub struct ZombieArchipelago(pub Entity);

pub struct ZombieNavmeshPlugin;

impl Plugin for ZombieNavmeshPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_baked_navmesh)
        .add_systems(Update, register_zombie_agents.before(LandmassSystems::SyncExistence));
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

fn load_baked_navmesh(
    scenario: Res<Scenario>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    let chunks = shared::navmesh_chunk_paths(*scenario);
    let archipelago = commands.spawn(zombie_archipelago()).id();
    for path in &chunks {
        commands.spawn(Island3dBundle {
            island: Island,
            archipelago_ref: ArchipelagoRef3d::new(archipelago),
            nav_mesh: NavMeshHandle3d(asset_server.load(path)),
        });
        info!("loading baked navmesh chunk {path}");
    }
    commands.insert_resource(ZombieArchipelago(archipelago));
    info!("{}: {} navmesh chunk(s) loaded as islands", scenario.title(), chunks.len());
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

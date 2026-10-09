use bevy::prelude::*;
use bevy_landmass::prelude::*;
use bevy_landmass::{Agent3d, ArchipelagoOptions, PointSampleDistance3d};
use bevy_rerecast::prelude::*;
use bevy_rerecast::NavmeshSettings;
use landmass_rerecast::{Island3dBundle, LandmassRerecastPlugin, NavMeshHandle3d};
use rerecast::PolygonNavmesh;
use shared::zombie::{
    ZOMBIE_LENGTH, ZOMBIE_LOD_REST_HEIGHT, ZOMBIE_RADIUS, ZOMBIE_RUN_SPEED, ZOMBIE_WALK_SPEED,
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
            distance_below: ZOMBIE_LOD_REST_HEIGHT + NAVMESH_SAMPLE_SLACK_BELOW_FEET,
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
    let path = scenario.baked_navmesh_path();
    let archipelago = commands.spawn(zombie_archipelago()).id();
    commands.spawn(Island3dBundle {
        island: Island,
        archipelago_ref: ArchipelagoRef3d::new(archipelago),
        nav_mesh: NavMeshHandle3d(asset_server.load(path)),
    });
    commands.insert_resource(ZombieArchipelago(archipelago));
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

pub fn baked_navmesh_file(scenario: Scenario) -> String {
    format!(
        "{}/../client/assets/{}",
        env!("CARGO_MANIFEST_DIR"),
        scenario.baked_navmesh_path()
    )
}

pub fn read_baked_navmesh(scenario: Scenario) -> Option<Navmesh> {
    let path = baked_navmesh_file(scenario);
    let bytes = std::fs::read(&path)
        .inspect_err(|e| error!("could not read {path}: {e}"))
        .ok()?;
    bincode::serde::decode_from_slice(&bytes, bincode::config::standard())
        .inspect_err(|e| error!("could not decode {path}: {e}"))
        .ok()
        .map(|(navmesh, _)| navmesh)
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

pub fn points_reachable_from(navmesh: &Navmesh, origin: Vec3) -> Vec<Vec3> {
    let polygons = &navmesh.polygon;
    let vertices_per_polygon = polygons.max_vertices_per_polygon as usize;
    let polygon_count = polygons.polygon_count();
    if vertices_per_polygon == 0 || polygon_count == 0 || navmesh.detail.meshes.len() < polygon_count
    {
        return Vec::new();
    }

    let Some(start) = polygon_nearest_to(navmesh, origin) else {
        return Vec::new();
    };

    let mut visited = vec![false; polygon_count];
    let mut queue = std::collections::VecDeque::from([start]);
    visited[start] = true;
    let mut points = Vec::new();
    while let Some(polygon) = queue.pop_front() {
        points.extend(detail_triangle_centroids(navmesh, polygon));

        let edges = polygon * vertices_per_polygon..(polygon + 1) * vertices_per_polygon;
        for &neighbour in &polygons.polygon_neighbors[edges] {
            let neighbour = neighbour as usize;
            if neighbour < polygon_count && !visited[neighbour] {
                visited[neighbour] = true;
                queue.push_back(neighbour);
            }
        }
    }
    points
}

fn polygon_nearest_to(navmesh: &Navmesh, origin: Vec3) -> Option<usize> {
    (0..navmesh.polygon.polygon_count())
        .filter_map(|polygon| {
            polygon_centroid(navmesh, polygon).map(|c| (polygon, c.distance_squared(origin)))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(polygon, _)| polygon)
}

fn polygon_centroid(navmesh: &Navmesh, polygon: usize) -> Option<Vec3> {
    let polygons = &navmesh.polygon;
    let vertices_per_polygon = polygons.max_vertices_per_polygon as usize;
    let corners: Vec<Vec3> = polygons.polygons
        [polygon * vertices_per_polygon..(polygon + 1) * vertices_per_polygon]
        .iter()
        .copied()
        .take_while(|index| *index != PolygonNavmesh::NO_INDEX)
        .map(|index| polygon_vertex(navmesh, index))
        .collect();
    (!corners.is_empty()).then(|| corners.iter().sum::<Vec3>() / corners.len() as f32)
}

fn polygon_vertex(navmesh: &Navmesh, index: u16) -> Vec3 {
    let polygons = &navmesh.polygon;
    let vertex = polygons.vertices[index as usize];
    Vec3::new(
        polygons.aabb.min.x + vertex.x as f32 * polygons.cell_size,
        polygons.aabb.min.y + vertex.y as f32 * polygons.cell_height,
        polygons.aabb.min.z + vertex.z as f32 * polygons.cell_size,
    )
}

fn detail_triangle_centroids(navmesh: &Navmesh, polygon: usize) -> Vec<Vec3> {
    let submesh = &navmesh.detail.meshes[polygon];
    let base_vertex = submesh.base_vertex_index as usize;
    let base_triangle = submesh.base_triangle_index as usize;
    (0..submesh.triangle_count as usize)
        .filter_map(|triangle| navmesh.detail.triangles.get(base_triangle + triangle))
        .map(|corners| {
            corners
                .iter()
                .filter_map(|corner| navmesh.detail.vertices.get(base_vertex + *corner as usize))
                .copied()
                .sum::<Vec3>()
                / 3.0
        })
        .collect()
}

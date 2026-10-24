use bevy::prelude::*;
use bevy_landmass::prelude::*;
use bevy_landmass::{Agent3d, ArchipelagoOptions, PointSampleDistance3d};
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
        // FixedUpdate, with landmass: ordering against one of its sets from
        // another schedule is silently no ordering at all.
        .add_systems(FixedUpdate, register_zombie_agents.before(LandmassSystems::SyncExistence));
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


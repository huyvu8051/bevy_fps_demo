use avian3d::prelude::Collider;
use avian_rerecast::prelude::*;
use bevy::prelude::*;
use bevy_landmass::prelude::*;
use bevy_landmass::{Agent3d, ArchipelagoOptions, PointSampleDistance3d};
use bevy_rerecast::prelude::*;
use bevy_rerecast::NavmeshSettings;
use landmass_rerecast::{Island3dBundle, LandmassRerecastPlugin, NavMeshHandle3d};
use shared::zombie::{ZOMBIE_LENGTH, ZOMBIE_RADIUS, ZOMBIE_RUN_SPEED, ZOMBIE_WALK_SPEED};

/// Tallest step a zombie can walk up without it being treated as a wall; sized
/// for Map 1's stairs and ramps.
const WALKABLE_CLIMB: f32 = 0.5;

/// Colliders are spawned in `Startup`, but their `GlobalTransform`s are only
/// propagated in the first `PostUpdate`, so baking any earlier reads every
/// piece at the origin.
const BAKE_MIN_FRAMES: u32 = 3;

/// Not every collider exists by `Startup`: the item gallery spawns its car
/// obstacle boxes from `Update`. Wait for the count to stop changing rather
/// than guess a fixed delay.
const BAKE_STABLE_FRAMES: u32 = 30;

/// The archipelago every zombie agent is attached to. Absent until the bake
/// finishes, which is what gates agent registration.
#[derive(Resource)]
pub(crate) struct ZombieArchipelago(pub(crate) Entity);

#[derive(Resource)]
struct PendingNavmesh(Handle<Navmesh>);

pub(crate) struct ServerNavmeshPlugin;

impl Plugin for ServerNavmeshPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            NavmeshPlugins::default(),
            AvianBackendPlugin::default(),
            Landmass3dPlugin::default(),
            LandmassRerecastPlugin::default(),
        ))
        .add_systems(
            Update,
            (bake_navmesh_once, register_zombie_agents).before(LandmassSystems::SyncExistence),
        )
        .add_observer(on_navmesh_ready);
    }
}

fn agent_height() -> f32 {
    ZOMBIE_LENGTH + ZOMBIE_RADIUS * 2.0
}

fn bake_navmesh_once(
    mut frames: Local<u32>,
    mut last_count: Local<usize>,
    mut stable_frames: Local<u32>,
    mut done: Local<bool>,
    colliders: Query<(), With<Collider>>,
    mut generator: NavmeshGenerator,
    mut commands: Commands,
) {
    if *done {
        return;
    }
    *frames += 1;

    let count = colliders.iter().count();
    if count == *last_count {
        *stable_frames += 1;
    } else {
        *last_count = count;
        *stable_frames = 0;
    }
    if *frames < BAKE_MIN_FRAMES || *stable_frames < BAKE_STABLE_FRAMES {
        return;
    }
    *done = true;

    let settings = NavmeshSettings {
        walkable_climb: WALKABLE_CLIMB,
        ..NavmeshSettings::from_agent_3d(ZOMBIE_RADIUS, agent_height())
    };
    let handle = generator.generate(settings);
    commands.insert_resource(PendingNavmesh(handle));
    info!("navmesh bake started from {count} live Avian colliders");
}

fn on_navmesh_ready(
    trigger: On<NavmeshReady>,
    pending: Option<Res<PendingNavmesh>>,
    navmeshes: Res<Assets<Navmesh>>,
    mut commands: Commands,
) {
    let Some(pending) = pending else {
        return;
    };
    if trigger.event().0 != pending.0.id() {
        return;
    }

    let baked = navmeshes.get(pending.0.id());
    let polygons = baked.map_or(0, |n| n.polygon.polygon_count());
    if polygons == 0 {
        error!("navmesh baked empty; zombies will fall back to straight-line chasing");
        return;
    }

    let archipelago = commands
        .spawn(Archipelago3d::new(ArchipelagoOptions {
            point_sample_distance: PointSampleDistance3d {
                distance_above: 0.75,
                distance_below: 0.75,
                ..PointSampleDistance3d::from_agent_radius(ZOMBIE_RADIUS)
            },
            ..ArchipelagoOptions::from_agent_radius(ZOMBIE_RADIUS)
        }))
        .id();
    commands.spawn(Island3dBundle {
        island: Island,
        archipelago_ref: ArchipelagoRef3d::new(archipelago),
        nav_mesh: NavMeshHandle3d(pending.0.clone()),
    });
    commands.insert_resource(ZombieArchipelago(archipelago));
    let aabb = baked.map(|n| n.polygon.aabb);
    info!("navmesh ready: {polygons} polygons, bounds {aabb:.1?}; zombies now path on it");
}

fn register_zombie_agents(
    archipelago: Option<Res<ZombieArchipelago>>,
    zombies: Query<Entity, (With<shared::Zombie>, Without<Agent3d>)>,
    mut commands: Commands,
) {
    let Some(archipelago) = archipelago else {
        return;
    };
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

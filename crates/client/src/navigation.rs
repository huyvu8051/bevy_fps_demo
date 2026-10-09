
use bevy::prelude::*;
use bevy_landmass::prelude::*;
use bevy_landmass::{Agent3d, ArchipelagoOptions, PointSampleDistance3d};
use bevy_rerecast::prelude::*;
use landmass_rerecast::{Island3dBundle, LandmassRerecastPlugin, NavMeshHandle3d};
use shared::{PLAYER_LENGTH, PLAYER_RADIUS};

use crate::current_scenario;

const SAMPLE_DISTANCE_ABOVE: f32 = 2.0;
const SAMPLE_SLACK_BELOW_FEET: f32 = 2.0;

/// The navmesh is baked for a zombie, which is slimmer than the player, so
/// sampling uses the radius the mesh was built for rather than the player's.
const SAMPLE_RADIUS: f32 = shared::zombie::ZOMBIE_RADIUS;

/// Recast erodes the walkable area away from walls, so anyone hugging one is
/// horizontally outside the mesh. The desert's streets are narrow enough that
/// the autopilot is there most of the time.
const SAMPLE_HORIZONTAL: f32 = 0.8;

/// How far off the mesh a target may be and still get snapped onto it. A
/// zombie shoved off the navmesh by physics should not make the route vanish.
pub(crate) fn target_sample_distance() -> PointSampleDistance3d {
    PointSampleDistance3d {
        horizontal_distance: SAMPLE_HORIZONTAL * 2.0,
        distance_above: 4.0,
        distance_below: 4.0,
        ..PointSampleDistance3d::from_agent_radius(SAMPLE_RADIUS)
    }
}

/// The archipelago the autopilot paths on.
///
/// The server runs its own for zombies; this one is the client's, so the
/// autopilot can route around a building instead of walking into it. Both load
/// the same baked `.nav`.
#[derive(Resource)]
pub(crate) struct PlayerArchipelago(pub(crate) Entity);

pub(crate) struct ClientNavigationPlugin;

impl Plugin for ClientNavigationPlugin {
    fn build(&self, app: &mut App) {
        // Both landmass plugins default to FixedPreUpdate, and the
        // rerecast conversion only runs on the frame the navmesh AssetEvent
        // arrives. At the client's frame rate most frames have no fixed step,
        // so that event is usually missed and the island never gets a mesh.
        app.add_plugins((
            NavmeshPlugins::default(),
            Landmass3dPlugin::default().in_schedule(Update),
            LandmassRerecastPlugin::default().in_schedule(Update),
        ))
        .add_systems(Startup, load_baked_navmesh)
        .add_systems(Update, register_player_agent.before(LandmassSystems::SyncExistence));
    }
}

fn load_baked_navmesh(asset_server: Res<AssetServer>, mut commands: Commands) {
    let path = current_scenario().baked_navmesh_path();
    let archipelago = commands
        .spawn(Archipelago3d::new(ArchipelagoOptions {
            point_sample_distance: PointSampleDistance3d {
                horizontal_distance: SAMPLE_HORIZONTAL,
                distance_above: SAMPLE_DISTANCE_ABOVE,
                // A player's transform sits at the capsule's centre while the
                // navmesh lies at its feet.
                distance_below: PLAYER_LENGTH / 2.0 + PLAYER_RADIUS + SAMPLE_SLACK_BELOW_FEET,
                ..PointSampleDistance3d::from_agent_radius(SAMPLE_RADIUS)
            },
            ..ArchipelagoOptions::from_agent_radius(SAMPLE_RADIUS)
        }))
        .id();
    commands.spawn(Island3dBundle {
        island: Island,
        archipelago_ref: ArchipelagoRef3d::new(archipelago),
        nav_mesh: NavMeshHandle3d(asset_server.load(path)),
    });
    commands.insert_resource(PlayerArchipelago(archipelago));
    info!("client navmesh: loading {path}");
}

/// The locally simulated player, not the replicated one: this is the transform
/// the autopilot and movement actually drive.
fn register_player_agent(
    archipelago: Res<PlayerArchipelago>,
    players: Query<
        Entity,
        (With<shared::Player>, Without<shared::NetPlayer>, Without<Agent3d>),
    >,
    mut commands: Commands,
) {
    for entity in &players {
        commands.entity(entity).insert((
            Agent3dBundle {
                agent: Agent3d::default(),
                settings: AgentSettings {
                    radius: PLAYER_RADIUS,
                    desired_speed: shared::MOVE_SPEED,
                    max_speed: shared::MOVE_SPEED * shared::SPRINT_MULTIPLIER,
                },
                archipelago_ref: ArchipelagoRef3d::new(archipelago.0),
            },
            AgentTarget3d::None,
        ));
    }
}


/// Toggled by the NavDbg cheat button; persisted in settings. Has no visual
/// effect until a navmesh debug view is wired up again.
#[derive(Resource, Default)]
pub(crate) struct NavDebugVisible(pub(crate) bool);

/// Marks level geometry that should carve a hole in the navmesh.
#[derive(Component)]
pub(crate) struct ClientNavObstacle;

#[derive(Component)]
pub(crate) struct ArenaCarNavObstacle;

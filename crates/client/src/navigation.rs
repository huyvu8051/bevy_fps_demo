use bevy::prelude::*;
use bevy_landmass::prelude::*;
use bevy_landmass::{Agent3d, ArchipelagoOptions, PointSampleDistance3d};
use bevy_rerecast::prelude::*;
use landmass_rerecast::{Island3dBundle, LandmassRerecastPlugin, NavMeshHandle3d};
use shared::{PLAYER_LENGTH, PLAYER_RADIUS};

use crate::current_scenario;

const BAKED_AGENT_RADIUS: f32 = shared::zombie::ZOMBIE_RADIUS;
const WALL_HUG_TOLERANCE: f32 = 0.8;
const OFF_MESH_TARGET_TOLERANCE: f32 = 4.0;
const SAMPLE_ABOVE: f32 = 2.0;
const SAMPLE_BELOW_FEET_SLACK: f32 = 2.0;

fn player_centre_to_feet() -> f32 {
    PLAYER_LENGTH / 2.0 + PLAYER_RADIUS
}

pub(crate) fn off_mesh_target_tolerance() -> PointSampleDistance3d {
    PointSampleDistance3d {
        horizontal_distance: WALL_HUG_TOLERANCE * 2.0,
        distance_above: OFF_MESH_TARGET_TOLERANCE,
        distance_below: OFF_MESH_TARGET_TOLERANCE,
        ..PointSampleDistance3d::from_agent_radius(BAKED_AGENT_RADIUS)
    }
}

#[derive(Resource)]
pub(crate) struct PlayerArchipelago(pub(crate) Entity);

pub(crate) struct ClientNavigationPlugin;

impl Plugin for ClientNavigationPlugin {
    fn build(&self, app: &mut App) {
        // FixedUpdate, not Update: update_archipelagos cost 0.15ms and ran at
        // the refresh rate, while the only consumer -- move_zombies -- reads
        // the desired velocity once every 0.8s. It was computing a hundred and
        // eighteen answers a second to use one and a quarter of them.
        //
        // move_zombies is in FixedUpdate too, so it still orders after
        // LandmassSystems::Output and reads this frame's paths, not last
        // frame's.
        app.add_plugins((
            NavmeshPlugins::default(),
            Landmass3dPlugin::default().in_schedule(FixedUpdate),
            LandmassRerecastPlugin::default().in_schedule(FixedUpdate),
        ))
        .add_systems(Startup, load_baked_navmesh)
        .add_systems(FixedUpdate, register_player_agent.before(LandmassSystems::SyncExistence));
    }
}

fn player_archipelago() -> Archipelago3d {
    Archipelago3d::new(ArchipelagoOptions {
        point_sample_distance: PointSampleDistance3d {
            horizontal_distance: WALL_HUG_TOLERANCE,
            distance_above: SAMPLE_ABOVE,
            distance_below: player_centre_to_feet() + SAMPLE_BELOW_FEET_SLACK,
            ..PointSampleDistance3d::from_agent_radius(BAKED_AGENT_RADIUS)
        },
        ..ArchipelagoOptions::from_agent_radius(BAKED_AGENT_RADIUS)
    })
}

fn load_baked_navmesh(asset_server: Res<AssetServer>, mut commands: Commands) {
    let chunks = shared::navmesh_chunk_paths(current_scenario());
    let archipelago = commands.spawn(player_archipelago()).id();
    for path in &chunks {
        commands.spawn(Island3dBundle {
            island: Island,
            archipelago_ref: ArchipelagoRef3d::new(archipelago),
            nav_mesh: NavMeshHandle3d(asset_server.load(path)),
        });
    }
    commands.insert_resource(PlayerArchipelago(archipelago));
    info!("client navmesh: loading {} chunk(s)", chunks.len());
}

fn register_player_agent(
    archipelago: Res<PlayerArchipelago>,
    locally_simulated_players: Query<
        Entity,
        (With<shared::Player>, Without<shared::NetPlayer>, Without<Agent3d>),
    >,
    mut commands: Commands,
) {
    for entity in &locally_simulated_players {
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

#[derive(Resource, Default)]
pub(crate) struct NavDebugVisible(pub(crate) bool);

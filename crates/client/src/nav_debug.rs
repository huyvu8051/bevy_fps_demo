// --- Nav debug overlay (client-side, settings-gated) -------------------
//
// The real zombie navmesh and pathfinding run on the headless server, so
// there's nothing to look at there. This rebuilds the SAME navmesh locally
// from the same inputs the server uses — shared::arena_navmesh_outer_edges
// for the boundary, and the arena pillar colliders spawned by
// spawn_test_arena (tagged ClientNavObstacle) for the holes — and draws:
//
//   * the navmesh polygons, via vleue's own NavMeshDebug gizmo, and
//   * one path per replicated zombie to its nearest player, recomputed
//     here every frame.
//
// The path is an APPROXIMATION of the server's route: same navmesh, but
// the server picks each zombie's target by threat score, not nearest, and
// adds separation / obstacle-steering on top. It's meant for "does the
// navmesh route around the pillars / cars like I expect", not a
// frame-exact replay. Parked cars ARE carved here too (their colliders
// get ClientNavObstacle via tag_client_car_colliders), but the client
// copies are RigidBody::Dynamic so a hole can sit a little off from the
// server's, which uses Static cars at the exact RON rest pose.
//
// Only MapKind::Arena has a per-map navmesh; other maps get nothing.
// Toggle: the "NavDbg" button in the settings menu Cheats section (debug
// builds only) flips NavDebugVisible.

use bevy::prelude::*;
use vleue_navigator::prelude::*;

use crate::*;

/// Flipped by the "NavDbg" settings button — see nav_debug_toggle_button.
#[derive(Resource, Default)]
pub(crate) struct NavDebugVisible(pub(crate) bool);

/// On each arena pillar collider (the Cylinder rows of arena_layout) so
/// NavmeshUpdaterPlugin carves a hole there — mirrors the server's
/// ArenaObstacle. Added in spawn_test_arena.
#[derive(Component)]
pub(crate) struct ClientNavObstacle;

pub(crate) struct NavDebugPlugin;

impl Plugin for NavDebugPlugin {
    fn build(&self, app: &mut App) {
        // Release builds link vleue_navigator in but never run any of this
        // — the "NavDbg" settings toggle is debug-only, so there'd be
        // nothing to flip NavDebugVisible.
        if !cfg!(debug_assertions) {
            return;
        }
        app.add_plugins((
            VleueNavigatorPlugin,
            NavmeshUpdaterPlugin::<Collider, ClientNavObstacle>::default(),
        ))
        .init_resource::<NavDebugVisible>()
        .add_systems(
            Update,
            (
                nav_debug_toggle_button,
                update_nav_debug_button_color,
                sync_client_navmesh,
                tag_client_car_colliders,
                apply_nav_debug_visibility,
                draw_zombie_path_traces,
            ),
        );
    }
}

// Standalone rather than folded into settings_menu's cheat_toggle_buttons_3
// — that one is already near Bevy's system param-count ceiling (see its own
// doc comment).
fn nav_debug_toggle_button(
    mut nav_debug: ResMut<NavDebugVisible>,
    btn: Query<&Interaction, (With<CheatNavDebugButton>, Changed<Interaction>)>,
) {
    if tapped(&btn) {
        nav_debug.0 = !nav_debug.0;
    }
}

fn update_nav_debug_button_color(
    nav_debug: Res<NavDebugVisible>,
    mut btn: Query<&mut BackgroundColor, With<CheatNavDebugButton>>,
) {
    if !nav_debug.is_changed() {
        return;
    }
    let color = if nav_debug.0 {
        Color::srgba(0.3, 1.0, 0.4, 0.5)
    } else {
        Color::srgba(1.0, 1.0, 1.0, 0.2)
    };
    for mut bg in &mut btn {
        *bg = BackgroundColor(color);
    }
}

fn navmesh_plane() -> Transform {
    Transform::from_xyz(0.0, shared::NAVMESH_HEIGHT, 0.0)
        .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2))
}

// Keeps exactly one ManagedNavMesh alive while on an Arena map, matching
// the server's spawn_zombie_navmesh Arena branch (same boundary, same
// settings). Despawns it on any other map so a menu switch can't leave a
// stale navmesh behind.
fn sync_client_navmesh(mut commands: Commands, existing: Query<Entity, With<ManagedNavMesh>>) {
    let is_arena = current_scenario().map_kind() == shared::MapKind::Arena;
    match (is_arena, existing.iter().next()) {
        (true, None) => {
            commands.spawn((
                ManagedNavMesh::single(),
                NavMeshSettings {
                    fixed: Triangulation::from_outer_edges(&shared::arena_navmesh_outer_edges(
                        current_scenario().map_id(),
                    )),
                    // Coarse on purpose — matches the server (see its
                    // spawn_zombie_navmesh comment): the car .glb hulls are
                    // many-sided and a fine simplify times the build out.
                    simplify: 0.5,
                    build_timeout: Some(15.0),
                    ..default()
                },
                // Debounced, not Direct: the client cars are Dynamic and
                // jiggle when shot, and a retriangulation every frame while
                // one settles is wasteful for a debug overlay.
                NavMeshUpdateMode::Debounced(0.3),
                navmesh_plane(),
            ));
        }
        (false, Some(entity)) => commands.entity(entity).despawn(),
        _ => {}
    }
}

// The car colliders are convex hulls ColliderConstructorHierarchy builds
// on descendant entities once the glTF loads — same problem the server's
// tag_car_colliders_as_nav_obstacles solves. Forward ClientNavObstacle
// onto each collider that descends from an ItemGalleryTestProp root so the
// updater carves the car footprint too. Idle once the scenes have loaded.
fn tag_client_car_colliders(
    mut commands: Commands,
    new_colliders: Query<Entity, Added<Collider>>,
    child_of: Query<&ChildOf>,
    car_roots: Query<(), With<ItemGalleryTestProp>>,
) {
    for entity in &new_colliders {
        if find_animation_root(entity, &child_of, |e| car_roots.contains(e)).is_some() {
            commands.entity(entity).insert(ClientNavObstacle);
        }
    }
}

// vleue's display_navmesh only draws entities carrying NavMeshDebug, so add
// / remove it in step with the toggle. Reconciled every frame (one entity)
// rather than on Res change, so it still applies if the navmesh spawns
// after the toggle was already on.
fn apply_nav_debug_visibility(
    mut commands: Commands,
    visible: Res<NavDebugVisible>,
    navmesh: Query<(Entity, Has<NavMeshDebug>), With<ManagedNavMesh>>,
) {
    for (entity, has_debug) in &navmesh {
        if visible.0 && !has_debug {
            commands.entity(entity).insert(NavMeshDebug(Color::srgb(0.15, 0.9, 0.35)));
        } else if !visible.0 && has_debug {
            commands.entity(entity).remove::<NavMeshDebug>();
        }
    }
}

fn draw_zombie_path_traces(
    visible: Res<NavDebugVisible>,
    mut gizmos: Gizmos,
    navmesh_handle: Query<&ManagedNavMesh>,
    navmeshes: Res<Assets<NavMesh>>,
    zombies: Query<&Transform, With<Zombie>>,
    players: Query<&Transform, (Or<(With<Player>, With<NetPlayer>)>, Without<Zombie>)>,
) {
    if !visible.0 {
        return;
    }
    let Some(navmesh) = navmesh_handle.iter().next().and_then(|h| navmeshes.get(h)) else {
        return;
    };
    let player_positions: Vec<Vec3> = players.iter().map(|t| t.translation).collect();
    if player_positions.is_empty() {
        return;
    }

    let lift = Vec3::Y * 0.15; // float the line just off the floor
    for zombie in &zombies {
        let from = zombie.translation;
        let Some(&to) = player_positions
            .iter()
            .min_by(|a, b| a.distance_squared(from).total_cmp(&b.distance_squared(from)))
        else {
            continue;
        };
        let Some(path) = navmesh.transformed_path(from, to) else {
            continue;
        };
        let mut prev = from;
        for &point in &path.path {
            gizmos.line(prev + lift, point + lift, Color::srgb(1.0, 0.75, 0.1));
            prev = point;
        }
        // little marker at the destination
        gizmos.circle(Isometry3d::new(to + lift, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)), 0.3, Color::srgb(1.0, 0.4, 0.1));
    }
}

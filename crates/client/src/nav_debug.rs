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
// frame-exact replay. Cars are carved as ONE coarse box each
// (spawn_client_car_nav_boxes, from the combined collider AABB) — never
// the raw per-primitive hulls, which blew polyanya's triangulation into
// tens of GB. The client cars are RigidBody::Dynamic so a box can sit a
// little off the server's Static rest pose.
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

/// On each arena pillar collider (the Cylinder rows of arena_layout) and
/// each per-car nav box so NavmeshUpdaterPlugin carves a hole there —
/// mirrors the server's ArenaObstacle. Pillars: added in spawn_test_arena.
/// Car boxes: spawn_client_car_nav_boxes.
#[derive(Component)]
pub(crate) struct ClientNavObstacle;

/// Car prop whose nav box has been spawned — see spawn_client_car_nav_boxes.
#[derive(Component)]
struct ClientCarNavBoxDone;

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
                spawn_client_car_nav_boxes,
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
                    simplify: 0.005,
                    build_timeout: Some(1.0),
                    ..default()
                },
                // Pillars are RigidBody::Static, so Direct never churns.
                NavMeshUpdateMode::Direct,
                navmesh_plane(),
            ));
        }
        (false, Some(entity)) => commands.entity(entity).despawn(),
        _ => {}
    }
}

// Mirror of the server's spawn_car_nav_obstacle_boxes: ONE coarse box per
// car for the navmesh, from the car's combined collider AABB (XZ; pinned
// to the sampling plane so a mid-drop snapshot still carves). Sensor so it
// doesn't become an invisible wall. Idle once every car has its box.
fn spawn_client_car_nav_boxes(
    mut commands: Commands,
    cars: Query<(Entity, &RigidBodyColliders), (With<ItemGalleryTestProp>, Without<ClientCarNavBoxDone>)>,
    aabbs: Query<&ColliderAabb>,
) {
    for (car, colliders) in &cars {
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        let mut ready = !colliders.is_empty();
        for collider in colliders.iter() {
            match aabbs.get(collider) {
                Ok(aabb) if aabb.min.is_finite() && aabb.max.is_finite() => {
                    min = min.min(aabb.min);
                    max = max.max(aabb.max);
                }
                _ => ready = false,
            }
        }
        if !ready {
            continue;
        }
        let center = Vec3::new((min.x + max.x) * 0.5, shared::NAVMESH_HEIGHT, (min.z + max.z) * 0.5);
        let size = Vec3::new((max.x - min.x).max(0.1), 2.0, (max.z - min.z).max(0.1));
        commands.spawn((
            Transform::from_translation(center),
            RigidBody::Static,
            Collider::cuboid(size.x, size.y, size.z),
            Sensor,
            ClientNavObstacle,
        ));
        commands.entity(car).insert(ClientCarNavBoxDone);
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

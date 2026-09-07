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
// MapKind::Arena and MapKind::Campaign(1) (Map 1's 4 real stacked floors,
// see shared::map1_navmesh_layers) have a mirrored navmesh here; other
// maps get nothing. Toggle: the "NavDbg" button in the settings menu
// Cheats section (debug builds only) flips NavDebugVisible.

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

/// Marks a per-car obstacle box specifically (ClientNavObstacle alone
/// also covers arena pillars — spawn_test_arena's own use of it — so this
/// is the only way to find and despawn JUST the car ones). Needed once
/// cars could be despawned/respawned live (settings-menu texture/poly
/// quality buttons, respawn_item_gallery_cars_on_quality_change): without
/// this, every quality change left the old car's obstacle box behind
/// forever (nothing despawned it), so a few toggles piled up duplicate,
/// increasingly overlapping obstacle boxes at the same positions — which
/// crashed vleue_navigator's async Constrained Delaunay Triangulation
/// (spade) with a real, confirmed device panic
/// (`assertion failed: !previous_region.is_empty() || from ==
/// final_vertex`), not just a cosmetic leak.
#[derive(Component)]
pub(crate) struct ItemGalleryCarNavObstacle;

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

// Keeps the right set of ManagedNavMesh entities alive for the current
// map — 1 for Arena (matching the server's spawn_zombie_navmesh Arena
// branch), 10 for Map 1 (1 per floor/ramp layer, matching that same fn's
// Campaign(1) branch — see shared::map1_navmesh_layers's own doc comment
// for why there are 10). Despawns everything on any other map so a menu
// switch can't leave a stale navmesh behind.
fn sync_client_navmesh(mut commands: Commands, existing: Query<Entity, With<ManagedNavMesh>>) {
    let map_kind = current_scenario().map_kind();
    let existing_count = existing.iter().count();
    match map_kind {
        shared::MapKind::Arena if existing_count == 0 => {
            commands.spawn((
                ManagedNavMesh::single(),
                NavMeshSettings {
                    fixed: Triangulation::from_outer_edges(&shared::arena_navmesh_outer_edges(
                        current_scenario().map_id(),
                    )),
                    // Shared with the server (spawn_zombie_navmesh) so the
                    // overlay matches the real navmesh — see arena.rs.
                    simplify: shared::ARENA_NAVMESH_SIMPLIFY,
                    build_timeout: Some(shared::ARENA_NAVMESH_BUILD_TIMEOUT_SECS),
                    agent_radius: shared::ARENA_NAVMESH_AGENT_RADIUS,
                    ..default()
                },
                // Pillars are RigidBody::Static, so Direct never churns.
                NavMeshUpdateMode::Direct,
                navmesh_plane(),
            ));
        }
        shared::MapKind::Campaign(1) if existing_count == 0 => {
            // Exact mirror of server/main.rs's spawn_zombie_navmesh
            // Campaign(1) branch — see its own comment for why this is a
            // duplicate rather than a shared helper (client/server are
            // separate binaries; crates/shared deliberately doesn't
            // depend on vleue_navigator).
            let (layers, stitches) = shared::map1_navmesh_layers();
            for (index, layer) in layers.iter().enumerate() {
                let index = index as u8;
                let to_local = |p: Vec3| shared::map1_world_to_layer_local(layer.translation, layer.rotation, p);
                let mut fixed = Triangulation::from_outer_edges(&layer.outer_world.iter().map(|&p| to_local(p)).collect::<Vec<_>>());
                if !layer.holes_world.is_empty() {
                    fixed.add_obstacles(layer.holes_world.iter().map(|hole| hole.iter().map(|&p| to_local(p)).collect::<Vec<_>>()));
                }
                let layer_stitches: Vec<((u8, u8), [Vec2; 2])> = stitches
                    .iter()
                    .filter(|s| s.from == index || s.to == index)
                    .map(|s| ((s.from, s.to), [to_local(s.points_world[0]), to_local(s.points_world[1])]))
                    .collect();
                commands.spawn((
                    ManagedNavMesh::single(),
                    NavMeshSettings {
                        fixed,
                        layer: Some(index),
                        stitches: layer_stitches,
                        build_timeout: Some(shared::ARENA_NAVMESH_BUILD_TIMEOUT_SECS),
                        agent_radius: shared::ARENA_NAVMESH_AGENT_RADIUS,
                        ..default()
                    },
                    NavMeshUpdateMode::Direct,
                    Transform::from_translation(layer.translation).with_rotation(layer.rotation),
                ));
            }
        }
        shared::MapKind::Arena | shared::MapKind::Campaign(1) => {} // already spawned this map, nothing to do
        _ => {
            for entity in &existing {
                commands.entity(entity).despawn();
            }
        }
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
            ItemGalleryCarNavObstacle,
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

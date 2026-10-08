
use bevy::prelude::*;
use vleue_navigator::prelude::*;

use crate::*;

#[derive(Resource, Default)]
pub(crate) struct NavDebugVisible(pub(crate) bool);

#[derive(Component)]
pub(crate) struct ClientNavObstacle;

#[derive(Component)]
struct ClientCarNavBoxDone;

#[derive(Component)]
pub(crate) struct ItemGalleryCarNavObstacle;

pub(crate) struct NavDebugPlugin;

impl Plugin for NavDebugPlugin {
    fn build(&self, app: &mut App) {

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
                draw_obstacles,
            ),
        );
    }
}

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

                    simplify: shared::ARENA_NAVMESH_SIMPLIFY,
                    build_timeout: Some(shared::ARENA_NAVMESH_BUILD_TIMEOUT_SECS),
                    agent_radius: shared::ARENA_NAVMESH_AGENT_RADIUS,
                    ..default()
                },

                NavMeshUpdateMode::Direct,
                navmesh_plane(),
            ));
        }
        shared::MapKind::Campaign(1) if existing_count == 0 => {

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
        shared::MapKind::Arena | shared::MapKind::Campaign(1) => {}
        _ => {
            for entity in &existing {
                commands.entity(entity).despawn();
            }
        }
    }
}

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

    let lift = Vec3::Y * 0.15;
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

        gizmos.circle(Isometry3d::new(to + lift, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)), 0.3, Color::srgb(1.0, 0.4, 0.1));
    }
}

fn draw_obstacles(visible: Res<NavDebugVisible>, mut gizmos: Gizmos, obstacles: Query<&ColliderAabb, With<ClientNavObstacle>>) {
    if !visible.0 {
        return;
    }
    let purple = Color::srgb(0.7, 0.15, 0.9);
    for aabb in &obstacles {
        let center = (aabb.min + aabb.max) / 2.0;
        let size = aabb.max - aabb.min;
        gizmos.cube(Transform::from_translation(center.into()).with_scale(size.into()), purple);
    }
}

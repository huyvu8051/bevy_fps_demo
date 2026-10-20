use avian3d::prelude::*;
use bevy::camera::primitives::Aabb;
use bevy::math::ops;
use bevy::prelude::*;

use crate::*;

/// How far into the reload the fresh magazine appears. The gun spends the first
/// part of the move with an empty well, which is what reads as a swap rather
/// than a pause.
const MAG_SWAP_AT: f32 = 0.55;

const MAG_LIFETIME: f32 = 20.0;
const MAG_DROP_SPEED: f32 = 0.8;
const MAG_DROP_FORWARD: f32 = 0.2;
const MAG_SPIN: f32 = 6.0;
const MAG_DENSITY: f32 = 300.0;
const MAX_DROPPED_MAGS: usize = 6;

#[derive(Component)]
pub(crate) struct ViewmodelMagazine;

#[derive(Component)]
pub(crate) struct DroppedMagazine {
    remaining: f32,
}

fn scatter(seed: f32) -> Vec3 {
    Vec3::new(ops::sin(seed * 1.7), ops::sin(seed * 2.3), ops::sin(seed * 3.1))
}

/// Pulls the magazine out of the viewmodel and leaves a real one on the floor.
///
/// Triggered on the edge where the reload starts, which is the only thing the
/// two entry points — the R key and firing the last round — have in common.
pub(crate) fn drop_magazine_on_reload(
    time: Res<Time>,
    mut commands: Commands,
    gun: Query<&Gun>,
    camera: Query<&GlobalTransform, With<FpsCamera>>,
    mut magazines: Query<
        (&GlobalTransform, &Mesh3d, &MeshMaterial3d<StandardMaterial>, &Aabb, &mut Visibility),
        With<ViewmodelMagazine>,
    >,
    mut was_reloading: Local<bool>,
) {
    let reloading = gun.single().map(|g| g.reload_remaining > 0.0).unwrap_or(false);
    let started = reloading && !*was_reloading;
    *was_reloading = reloading;
    if !started {
        return;
    }
    let Ok(eye) = camera.single() else { return };

    for (global, mesh, material, aabb, mut visibility) in &mut magazines {
        if *visibility == Visibility::Hidden {
            continue;
        }
        *visibility = Visibility::Hidden;

        let placed = global.compute_transform();
        let offset = Vec3::from(aabb.center);
        let half = Vec3::from(aabb.half_extents) * placed.scale.abs();

        // The body sits on the magazine's middle so it tumbles about itself,
        // while the mesh keeps the origin the glTF gave it. Hence the child.
        let seed = time.elapsed_secs();
        let away = (Vec3::NEG_Y + *eye.forward() * MAG_DROP_FORWARD).normalize_or_zero();
        let body = commands
            .spawn((
                Name::new("DroppedMagazine"),
                Transform::from_translation(global.transform_point(offset))
                    .with_rotation(placed.rotation),
                Visibility::default(),
                RigidBody::Dynamic,
                Collider::cuboid(half.x * 2.0, half.y * 2.0, half.z * 2.0),
                ColliderDensity(MAG_DENSITY),
                // Map geometry is a trimesh with no thickness, and a magazine
                // crosses its own depth in one step once gravity has it.
                SweptCcd::default(),
                LinearVelocity(away * MAG_DROP_SPEED),
                AngularVelocity(scatter(seed * 1.9) * MAG_SPIN),
                DroppedMagazine { remaining: MAG_LIFETIME },
            ))
            .id();

        commands.spawn((
            Mesh3d(mesh.0.clone()),
            MeshMaterial3d(material.0.clone()),
            Transform::from_translation(-offset * placed.scale).with_scale(placed.scale),
            ChildOf(body),
        ));

        if magazine_log_enabled() {
            let at = global.transform_point(offset);
            info!("[magazine] dropped at {at:.3?} size {:.3?}m", half * 2.0);
        }
    }
}

fn magazine_log_enabled() -> bool {
    std::env::var("MAG_LOG").map(|value| value == "1").unwrap_or(false)
}

/// Puts the fresh magazine back partway through the move, not at the end: the
/// gun is level again well before the reload timer is, and an empty well that
/// late reads as a missing part rather than a reload.
pub(crate) fn restore_magazine_after_reload(
    gun: Query<&Gun>,
    mut magazines: Query<&mut Visibility, With<ViewmodelMagazine>>,
) {
    let Ok(gun) = gun.single() else { return };
    if gun.reload_remaining > RELOAD_DURATION * (1.0 - MAG_SWAP_AT) {
        return;
    }
    for mut visibility in &mut magazines {
        if *visibility == Visibility::Hidden {
            *visibility = Visibility::Inherited;
            if magazine_log_enabled() {
                info!("[magazine] fresh one in, {:.2}s left on the reload", gun.reload_remaining);
            }
        }
    }
}

/// Dropped magazines are rigid bodies the solver keeps revisiting, so the
/// oldest are cut short once there are more than a firefight's worth.
pub(crate) fn update_dropped_magazines(
    time: Res<Time>,
    mut commands: Commands,
    mut magazines: Query<(Entity, &Transform, &mut DroppedMagazine)>,
    mut next_log: Local<f32>,
) {
    let mut over_budget = magazines.iter().len().saturating_sub(MAX_DROPPED_MAGS);
    let now = time.elapsed_secs();
    let logging = magazine_log_enabled() && now >= *next_log;
    if logging {
        *next_log = now + 0.5;
    }
    for (entity, transform, mut magazine) in &mut magazines {
        if over_budget > 0 && magazine.remaining > 1.0 {
            over_budget -= 1;
            magazine.remaining = 1.0;
        }
        if logging {
            info!(
                "[magazine] {entity:?} at {:.3?} age {:.1}s",
                transform.translation,
                MAG_LIFETIME - magazine.remaining
            );
        }
        magazine.remaining -= time.delta_secs();
        if magazine.remaining <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}

use avian3d::prelude::*;
use bevy::camera::primitives::Aabb;
use bevy::math::ops;
use bevy::prelude::*;

use crate::*;

/// How far into the reload the fresh magazine appears. The gun spends the first
/// part of the move with an empty well, which is what reads as a swap rather
/// than a pause.
const MAG_SWAP_AT: f32 = RELOAD_HAND_BACK_AT_MAG;

const MAG_LIFETIME: f32 = 20.0;
const MAG_DROP_SPEED: f32 = 0.8;
const MAG_DROP_FORWARD: f32 = 0.2;
const MAG_SPIN: f32 = 6.0;
const MAG_DENSITY: f32 = 300.0;
const MAX_DROPPED_MAGS: usize = 6;

#[derive(Component)]
pub(crate) struct ViewmodelMagazine;

/// Where the left hand is told to be while reloading, instead of on the
/// weapon's fore grip.
///
/// A separate entity rather than moving Socket_FrontGrip: that socket belongs
/// to the mesh, is rebuilt on every weapon swap, and is what the hand must
/// return to. The solver takes whichever entity it is handed, so swapping the
/// target is the whole of the change.
#[derive(Component)]
pub(crate) struct ReloadHandTarget;

/// Fractions of the reload at which the hand reaches each point. The magazine
/// leaves when the hand is on it, not when the reload starts.
const RELOAD_HAND_AT_MAG: f32 = 0.18;
const RELOAD_HAND_AT_POUCH: f32 = 0.50;
const RELOAD_HAND_BACK_AT_MAG: f32 = 0.74;
/// Off the eye, low and on the weak-hand side: where a spare magazine lives.
const RELOAD_POUCH_FROM_EYE: Vec3 = Vec3::new(-0.16, -0.58, -0.26);

fn eased(from: Vec3, to: Vec3, t: f32) -> Vec3 {
    let t = t.clamp(0.0, 1.0);
    from.lerp(to, t * t * (3.0 - 2.0 * t))
}

/// Walks the left hand through a reload: off the fore grip, down to the
/// magazine, away to the pouch, back up with a fresh one, and home.
///
/// Authored as waypoints rather than a clip because the weapons carry no
/// skeleton at all -- every mesh reports skins=0 -- and because the hands are
/// already posed by IK, which a clip would fight over the same bones.
pub(crate) fn drive_reload_hand(
    active: Res<ActiveWeapon>,
    gun: Query<&Gun>,
    camera: Query<&GlobalTransform, With<FpsCamera>>,
    fore: Query<&GlobalTransform, With<ForeGripSocket>>,
    magazine: Query<&GlobalTransform, With<ViewmodelMagazine>>,
    mut target: Query<&mut Transform, With<ReloadHandTarget>>,
) {
    let (Ok(gun), Ok(eye), Ok(mut target), Ok(fore)) =
        (gun.single(), camera.single(), target.single_mut(), fore.single())
    else {
        return;
    };
    let home = fore.reparented_to(eye);
    // The magazine is only hidden while it is away, never despawned, so its
    // place on the weapon stays readable throughout.
    let magwell = magazine
        .single()
        .map(|m| m.reparented_to(eye).translation)
        .unwrap_or(home.translation);
    let pouch = RELOAD_POUCH_FROM_EYE - Vec3::Z * view_pullback();

    let progress = 1.0 - (gun.reload_remaining / active.def().reload_duration).clamp(0.0, 1.0);
    let span = |from: f32, to: f32| (progress - from) / (to - from);
    target.translation = if progress < RELOAD_HAND_AT_MAG {
        eased(home.translation, magwell, span(0.0, RELOAD_HAND_AT_MAG))
    } else if progress < RELOAD_HAND_AT_POUCH {
        eased(magwell, pouch, span(RELOAD_HAND_AT_MAG, RELOAD_HAND_AT_POUCH))
    } else if progress < RELOAD_HAND_BACK_AT_MAG {
        eased(pouch, magwell, span(RELOAD_HAND_AT_POUCH, RELOAD_HAND_BACK_AT_MAG))
    } else {
        eased(magwell, home.translation, span(RELOAD_HAND_BACK_AT_MAG, 1.0))
    };
    // The grip's own turn throughout: the wrist has no business rolling just
    // because the hand went for a magazine.
    target.rotation = home.rotation;
}

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
    active: Res<ActiveWeapon>,
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
    // When the hand arrives, not when the reload begins: a magazine falling
    // while the hand is still on the fore grip reads as nothing pulled it.
    let reached = gun
        .single()
        .map(|g| g.reload_remaining > 0.0 && 1.0 - g.reload_remaining / active.def().reload_duration >= RELOAD_HAND_AT_MAG)
        .unwrap_or(false);
    let started = reached && !*was_reloading;
    *was_reloading = reached;
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
    active: Res<ActiveWeapon>,
    gun: Query<&Gun>,
    mut magazines: Query<&mut Visibility, With<ViewmodelMagazine>>,
) {
    let Ok(gun) = gun.single() else { return };
    if gun.reload_remaining > active.def().reload_duration * (1.0 - MAG_SWAP_AT) {
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

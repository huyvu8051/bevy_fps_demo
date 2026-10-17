use bevy::prelude::*;

use crate::*;

/// Never let the triangle collapse into a straight line: a locked-out arm has
/// no bend plane left, so the elbow is free to flip anywhere.
const MAX_EXTENSION: f32 = 0.995;

/// Where the hands grab, as metres from the gun's own origin, measured the way
/// GUN_OFFSET and MUZZLE_OFFSET are: -Z points down the barrel. Converted into
/// the gun's local space on spawn, which is why GUN_SCALE shows up there.
const TRIGGER_GRIP_FROM_GUN: Vec3 = Vec3::new(0.0, 0.015, 0.06);
const FORE_GRIP_FROM_GUN: Vec3 = Vec3::new(0.0, 0.02, -0.14);

/// Pole targets steer which way the elbows break. Values carried over from the
/// grip_ik tuning in project-alpha, where they were dialled in by eye.
const POLE_DOWN_OFFSET: f32 = 0.3;
const POLE_OUTWARD_OFFSET: f32 = 0.4;

#[derive(Component)]
pub(crate) struct TriggerGripSocket;

#[derive(Component)]
pub(crate) struct ForeGripSocket;

#[derive(Component)]
pub(crate) struct RightPoleTarget;

#[derive(Component)]
pub(crate) struct LeftPoleTarget;

/// Sockets ride on the gun so they inherit every bit of recoil, sway and the
/// ADS pull for free — that is the whole point of hanging them off it rather
/// than recomputing hand targets each frame.
/// Visible while the grip is being dialled in: without them the socket is an
/// invisible point and the only feedback is a distance in the log.
const GRIP_MARKER_RADIUS: f32 = 0.022;

fn grip_markers_enabled() -> bool {
    std::env::var("GRIP_MARKERS").map(|value| value != "0").unwrap_or(true)
}

/// Debug rig: park the targets in clear air in front of the eye instead of on
/// the rifle, so "is the solver working" can be answered without "can the arm
/// reach the gun" clouding it. Sat at 76% of arm reach, so the elbow stays
/// visibly bent, and 17 degrees below centre so both stay on screen.
const TEST_TARGET_RIGHT: Vec3 = Vec3::new(0.16, -0.10, -0.32);
const TEST_TARGET_LEFT: Vec3 = Vec3::new(-0.16, -0.10, -0.32);

fn grip_test_targets_enabled() -> bool {
    std::env::var("GRIP_TEST").map(|value| value != "0").unwrap_or(false)
}

pub(crate) fn spawn_grip_sockets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    gun: Query<Entity, With<Gun>>,
    camera: Query<Entity, With<FpsCamera>>,
    existing: Query<(), With<TriggerGripSocket>>,
) {
    if !existing.is_empty() {
        return;
    }
    let testing = grip_test_targets_enabled();
    // In test mode the targets hang off the camera, which has no scale of its
    // own, so the offsets are plain metres. On the gun they have to be undone
    // through its resting yaw and GUN_SCALE first.
    let Ok(parent) = (if testing { camera.single() } else { gun.single() }) else {
        return;
    };
    let to_gun_local = Quat::from_rotation_y(GUN_ROTATION_Y_DEG.to_radians()).inverse();
    let local = |offset: Vec3| {
        Transform::from_translation(if testing { offset } else { to_gun_local * offset / GUN_SCALE })
    };
    let marker_scale = if testing { 1.0 } else { 1.0 / GUN_SCALE };
    let (trigger_at, fore_at) = if testing {
        // These hang off the camera, so a debug pullback drags them backwards
        // while the gun and body stay put. Cancel it the same way they do, or
        // the arms end up reaching for targets left behind the shoulders.
        let fix = Vec3::Z * view_pullback();
        (TEST_TARGET_RIGHT - fix, TEST_TARGET_LEFT - fix)
    } else {
        (TRIGGER_GRIP_FROM_GUN, FORE_GRIP_FROM_GUN)
    };

    let markers = grip_markers_enabled().then(|| {
        // the socket inherits GUN_SCALE, so undo it or the sphere is a speck
        let mesh = meshes.add(Sphere::new(GRIP_MARKER_RADIUS * marker_scale));
        let mut paint = |colour: Color, glow: LinearRgba| {
            materials.add(StandardMaterial {
                base_color: colour,
                emissive: glow,
                unlit: true,
                // the socket sits where a hand grips, which is inside the solid
                // receiver — without the bias the marker is buried in the rifle
                depth_bias: 1000.0,
                ..default()
            })
        };
        (
            mesh,
            paint(Color::srgb(1.0, 0.25, 0.2), LinearRgba::rgb(6.0, 0.6, 0.4)),
            paint(Color::srgb(0.25, 1.0, 0.4), LinearRgba::rgb(0.5, 6.0, 1.2)),
        )
    });

    let mut trigger = commands.spawn((
        Name::new("TriggerGripSocket"),
        local(trigger_at),
        TriggerGripSocket,
        ChildOf(parent),
    ));
    if let Some((mesh, red, _)) = &markers {
        trigger.insert((Mesh3d(mesh.clone()), MeshMaterial3d(red.clone()), RenderLayers::layer(VIEWMODEL_LAYER)));
    }

    let mut fore = commands.spawn((
        Name::new("ForeGripSocket"),
        local(fore_at),
        ForeGripSocket,
        ChildOf(parent),
    ));
    if let Some((mesh, _, green)) = &markers {
        fore.insert((Mesh3d(mesh.clone()), MeshMaterial3d(green.clone()), RenderLayers::layer(VIEWMODEL_LAYER)));
    }
    info!("[grip] sockets under {parent:?} (test_targets={testing}, markers={})", grip_markers_enabled());
}

pub(crate) fn spawn_pole_targets(mut commands: Commands) {
    commands.spawn((Name::new("RightPoleTarget"), Transform::default(), RightPoleTarget));
    commands.spawn((Name::new("LeftPoleTarget"), Transform::default(), LeftPoleTarget));
}

/// The elbow should swing away from the chest, so the pole sits below the
/// shoulder and out along the shoulder line.
fn shoulder_outward(left: Vec3, right: Vec3) -> Vec3 {
    (right - left).normalize_or_zero()
}

pub(crate) fn update_pole_targets(
    bones: Query<&PlayerModelSkeletonBones, With<ViewmodelBody>>,
    globals: Query<&GlobalTransform, (Without<RightPoleTarget>, Without<LeftPoleTarget>)>,
    mut right: Query<(&mut Transform, &mut GlobalTransform), With<RightPoleTarget>>,
    mut left: Query<(&mut Transform, &mut GlobalTransform), (With<LeftPoleTarget>, Without<RightPoleTarget>)>,
) {
    let Ok(bones) = bones.single() else { return };
    let (Some(right_arm), Some(left_arm)) = (bones.right_arm, bones.left_arm) else {
        return;
    };
    let (Ok(right_arm), Ok(left_arm)) = (globals.get(right_arm), globals.get(left_arm)) else {
        return;
    };
    let outward = shoulder_outward(left_arm.translation(), right_arm.translation());

    if let Ok((mut transform, mut global)) = right.single_mut() {
        transform.translation =
            right_arm.translation() - Vec3::Y * POLE_DOWN_OFFSET + outward * POLE_OUTWARD_OFFSET;
        *global = GlobalTransform::from(*transform);
    }
    if let Ok((mut transform, mut global)) = left.single_mut() {
        transform.translation =
            left_arm.translation() - Vec3::Y * POLE_DOWN_OFFSET - outward * POLE_OUTWARD_OFFSET;
        *global = GlobalTransform::from(*transform);
    }
}

/// Default off while the static arm pose is being checked: two systems both
/// writing the arm bones makes it impossible to tell whose result you are
/// looking at.
pub(crate) fn grip_ik_enabled() -> bool {
    std::env::var("GRIP_IK").map(|value| value != "0").unwrap_or(true)
}



/// The point of using IK here is that correctness becomes a number instead of
/// something only a human can judge, so report that number.

/// Measures the quantities the solver gets wrong when a rig carries scale.
///
/// `bevy_mod_inverse_kinematics` mixes a bone local offset straight into world
/// coordinates (`target - rotation * normal`), which only holds if the skeleton
/// sits at scale 1. This rig is authored in centimetres and scaled down by
/// `target_character`, so every length it derives is out by that factor. These
/// numbers say by how much, and whether the one bone it does turn ends up
/// aimed where it should be.

/// Refreshes the GlobalTransform of every constrained bone after a solve.
///
/// The solver only touches the bones it rotates — with a one-bone chain that
/// is the forearm alone, leaving the hand holding the GlobalTransform that
/// propagation computed before IK ran. Since this runs after propagation,
/// nothing else will fix it, and skinning reads exactly that transform: the
/// forearm swings while the hand stays behind.

/// Orients a bone so its +Y runs along `along`, with the twist pinned by the
/// bend-plane normal.
///
/// `Quat::from_rotation_arc` also aims a bone, but it returns the minimal arc,
/// which leaves the twist about that axis undefined — it then flips as the aim
/// sweeps past, and the wrist spins with it. That is the "strange rule".
fn bone_orientation(along: Vec3, plane_normal: Vec3) -> Quat {
    let y = along;
    let x = y.cross(plane_normal).normalize_or_zero();
    if x == Vec3::ZERO {
        return Quat::from_rotation_arc(Vec3::Y, y);
    }
    Quat::from_mat3(&Mat3::from_cols(x, y, x.cross(y)))
}

/// Analytic two-bone IK: law of cosines for the elbow angle, a pole hint for
/// the plane it bends in.
///
/// Hand-written rather than taken from the crate because of two things that
/// crate gets wrong on a scaled rig. Every length here is measured straight out
/// of GlobalTransform, so the 0.0104 the rig carries is already in the numbers
/// instead of being assumed away. And every parent rotation is read through
/// `GlobalTransform::rotation`, which decomposes the scale off first, where
/// `Quat::from_affine3a` on the same matrix returns a rotation that is simply
/// wrong once the matrix is not orthonormal.
///
/// Returns the leftover distance from hand to target.
fn solve_arm(
    arm: Entity,
    fore_arm: Entity,
    hand: Entity,
    target: Entity,
    pole: Entity,
    child_of: &Query<&ChildOf>,
    transforms: &mut Query<(&mut Transform, &mut GlobalTransform)>,
) -> Option<f32> {
    let shoulder = transforms.get(arm).ok()?.1.translation();
    let elbow = transforms.get(fore_arm).ok()?.1.translation();
    let wrist = transforms.get(hand).ok()?.1.translation();
    let target_pos = transforms.get(target).ok()?.1.translation();
    let pole_pos = transforms.get(pole).ok()?.1.translation();

    let upper_len = elbow.distance(shoulder);
    let lower_len = wrist.distance(elbow);
    if upper_len < 1e-5 || lower_len < 1e-5 {
        return None;
    }

    let axis = (target_pos - shoulder).try_normalize()?;
    let span = shoulder.distance(target_pos).clamp(
        (upper_len - lower_len).abs() + 1e-4,
        (upper_len + lower_len) * MAX_EXTENSION,
    );
    let cos_shoulder = ((upper_len * upper_len + span * span - lower_len * lower_len)
        / (2.0 * upper_len * span))
        .clamp(-1.0, 1.0);

    // Turning `axis` about this normal by +angle swings it toward the pole,
    // since normal x axis is the pole direction with the axis part removed.
    let pole_dir = (pole_pos - shoulder).try_normalize().unwrap_or(Vec3::Y);
    let plane_normal = axis
        .cross(pole_dir)
        .try_normalize()
        .unwrap_or_else(|| axis.any_orthonormal_vector());

    let upper_dir = Quat::from_axis_angle(plane_normal, cos_shoulder.acos()) * axis;
    let new_elbow = shoulder + upper_dir * upper_len;
    let lower_dir = (target_pos - new_elbow).try_normalize()?;

    let arm_world = bone_orientation(upper_dir, plane_normal);
    let fore_world = bone_orientation(lower_dir, plane_normal);

    let arm_parent = child_of.get(arm).ok()?.0;
    let arm_parent_global = *transforms.get(arm_parent).ok()?.1;
    {
        let (mut transform, mut global) = transforms.get_mut(arm).ok()?;
        transform.rotation = arm_parent_global.rotation().inverse() * arm_world;
        *global = arm_parent_global.mul_transform(*transform);
    }

    let arm_global = *transforms.get(arm).ok()?.1;
    {
        let (mut transform, mut global) = transforms.get_mut(fore_arm).ok()?;
        transform.rotation = arm_global.rotation().inverse() * fore_world;
        *global = arm_global.mul_transform(*transform);
    }

    // The hand is not rotated, but its world transform still has to follow the
    // forearm or the skinned mesh keeps drawing it where it used to be.
    let fore_global = *transforms.get(fore_arm).ok()?.1;
    {
        let (transform, mut global) = transforms.get_mut(hand).ok()?;
        let local = *transform;
        *global = fore_global.mul_transform(local);
    }

    Some(transforms.get(hand).ok()?.1.translation().distance(target_pos))
}

pub(crate) fn solve_grip_ik(
    time: Res<Time>,
    bones: Query<&PlayerModelSkeletonBones, With<ViewmodelBody>>,
    trigger: Query<Entity, With<TriggerGripSocket>>,
    fore: Query<Entity, With<ForeGripSocket>>,
    right_pole: Query<Entity, With<RightPoleTarget>>,
    left_pole: Query<Entity, With<LeftPoleTarget>>,
    child_of: Query<&ChildOf>,
    mut transforms: Query<(&mut Transform, &mut GlobalTransform)>,
    mut next_log: Local<f32>,
) {
    let Ok(bones) = bones.single() else { return };
    let (Ok(trigger), Ok(fore), Ok(right_pole), Ok(left_pole)) =
        (trigger.single(), fore.single(), right_pole.single(), left_pole.single())
    else {
        return;
    };

    let right = bones.right_arm.zip(bones.right_fore_arm).zip(bones.right_hand).and_then(
        |((arm, fore_arm), hand)| {
            solve_arm(arm, fore_arm, hand, trigger, right_pole, &child_of, &mut transforms)
        },
    );
    let left = bones.left_arm.zip(bones.left_fore_arm).zip(bones.left_hand).and_then(
        |((arm, fore_arm), hand)| {
            solve_arm(arm, fore_arm, hand, fore, left_pole, &child_of, &mut transforms)
        },
    );

    let now = time.elapsed_secs();
    if now >= *next_log {
        *next_log = now + 0.5;
        info!(
            "[grip] residual right={:?} left={:?}",
            right.map(|d| format!("{d:.4}m")),
            left.map(|d| format!("{d:.4}m")),
        );
    }
}

use bevy::prelude::*;

use crate::*;

const MAX_EXTENSION: f32 = 0.995;

const TRIGGER_GRIP_IN_GUN_MESH: Vec3 = Vec3::new(160.0, 80.0, -130.0);
const FORE_GRIP_IN_GUN_MESH: Vec3 = Vec3::new(-50.0, 160.0, 100.0);

const POLE_DROP_BELOW_SHOULDER: f32 = 0.3;
const POLE_OUT_ALONG_SHOULDER_LINE: f32 = 0.4;

const MARKER_RADIUS_METRES: f32 = 0.022;
const DRAW_OVER_SOLID_GEOMETRY_BIAS: f32 = 1000.0;
const AXIS_BAR_LENGTH: f32 = 150.0;
const AXIS_BAR_THICKNESS: f32 = 8.0;

const TEST_TARGET_RIGHT_FROM_EYE: Vec3 = Vec3::new(0.16, -0.10, -0.32);
const TEST_TARGET_LEFT_FROM_EYE: Vec3 = Vec3::new(-0.16, -0.10, -0.32);

#[derive(Component)]
pub(crate) struct TriggerGripSocket;

#[derive(Component)]
pub(crate) struct ForeGripSocket;

#[derive(Component)]
pub(crate) struct RightPoleTarget;

#[derive(Component)]
pub(crate) struct LeftPoleTarget;

fn grip_markers_enabled() -> bool {
    std::env::var("GRIP_MARKERS").map(|value| value != "0").unwrap_or(true)
}

fn grip_test_targets_enabled() -> bool {
    std::env::var("GRIP_TEST").map(|value| value != "0").unwrap_or(false)
}

pub(crate) fn grip_ik_enabled() -> bool {
    std::env::var("GRIP_IK").map(|value| value != "0").unwrap_or(true)
}

fn glowing_unlit_material(colour: Color, glow: LinearRgba) -> StandardMaterial {
    StandardMaterial {
        base_color: colour,
        emissive: glow,
        unlit: true,
        depth_bias: DRAW_OVER_SOLID_GEOMETRY_BIAS,
        ..default()
    }
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
    let Ok(parent) = (if testing { camera.single() } else { gun.single() }) else {
        return;
    };

    let (trigger_at, fore_at, metres_to_parent_units) = if testing {
        let cancel_debug_pullback = Vec3::Z * view_pullback();
        (
            TEST_TARGET_RIGHT_FROM_EYE - cancel_debug_pullback,
            TEST_TARGET_LEFT_FROM_EYE - cancel_debug_pullback,
            1.0,
        )
    } else {
        (TRIGGER_GRIP_IN_GUN_MESH, FORE_GRIP_IN_GUN_MESH, 1.0 / GUN_SCALE)
    };

    let markers = grip_markers_enabled().then(|| {
        (
            meshes.add(Sphere::new(MARKER_RADIUS_METRES * metres_to_parent_units)),
            materials.add(glowing_unlit_material(
                Color::srgb(1.0, 0.25, 0.2),
                LinearRgba::rgb(6.0, 0.6, 0.4),
            )),
            materials.add(glowing_unlit_material(
                Color::srgb(0.25, 1.0, 0.4),
                LinearRgba::rgb(0.5, 6.0, 1.2),
            )),
        )
    });

    let mut trigger = commands.spawn((
        Name::new("TriggerGripSocket"),
        Transform::from_translation(trigger_at),
        TriggerGripSocket,
        ChildOf(parent),
    ));
    if let Some((mesh, red, _)) = &markers {
        trigger.insert((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(red.clone()),
            RenderLayers::layer(VIEWMODEL_LAYER),
        ));
    }
    let trigger_entity = trigger.id();

    let mut fore = commands.spawn((
        Name::new("ForeGripSocket"),
        Transform::from_translation(fore_at),
        ForeGripSocket,
        ChildOf(parent),
    ));
    if let Some((mesh, _, green)) = &markers {
        fore.insert((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(green.clone()),
            RenderLayers::layer(VIEWMODEL_LAYER),
        ));
    }
    let fore_entity = fore.id();

    if markers.is_some() {
        spawn_axis_gizmo(&mut commands, &mut meshes, &mut materials, trigger_entity);
        spawn_axis_gizmo(&mut commands, &mut meshes, &mut materials, fore_entity);
    }
    info!("[grip] sockets under {parent:?} (test_targets={testing})");
}

fn spawn_axis_gizmo(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    parent: Entity,
) {
    let bar = meshes.add(Cuboid::new(AXIS_BAR_THICKNESS, AXIS_BAR_THICKNESS, AXIS_BAR_LENGTH));
    let half = AXIS_BAR_LENGTH * 0.5;
    let quarter_turn = std::f32::consts::FRAC_PI_2;
    let axes = [
        ("+X", Vec3::X * half, Quat::from_rotation_y(quarter_turn), LinearRgba::rgb(8.0, 0.2, 0.2)),
        ("+Y", Vec3::Y * half, Quat::from_rotation_x(-quarter_turn), LinearRgba::rgb(0.2, 8.0, 0.2)),
        ("+Z", Vec3::Z * half, Quat::IDENTITY, LinearRgba::rgb(0.3, 0.4, 9.0)),
    ];
    for (name, offset, rotation, glow) in axes {
        let colour = Color::linear_rgb(
            glow.red.min(1.0),
            glow.green.min(1.0),
            glow.blue.min(1.0),
        );
        commands.spawn((
            Name::new(format!("GripAxis {name}")),
            Mesh3d(bar.clone()),
            MeshMaterial3d(materials.add(glowing_unlit_material(colour, glow))),
            Transform::from_translation(offset).with_rotation(rotation),
            RenderLayers::layer(VIEWMODEL_LAYER),
            ChildOf(parent),
        ));
    }
}

pub(crate) fn spawn_pole_targets(mut commands: Commands) {
    commands.spawn((Name::new("RightPoleTarget"), Transform::default(), RightPoleTarget));
    commands.spawn((Name::new("LeftPoleTarget"), Transform::default(), LeftPoleTarget));
}

pub(crate) fn update_pole_targets(
    bones: Query<&PlayerModelSkeletonBones, With<ViewmodelBody>>,
    globals: Query<&GlobalTransform, (Without<RightPoleTarget>, Without<LeftPoleTarget>)>,
    mut right: Query<(&mut Transform, &mut GlobalTransform), With<RightPoleTarget>>,
    mut left: Query<
        (&mut Transform, &mut GlobalTransform),
        (With<LeftPoleTarget>, Without<RightPoleTarget>),
    >,
) {
    let Ok(bones) = bones.single() else { return };
    let (Some(right_arm), Some(left_arm)) = (bones.right_arm, bones.left_arm) else {
        return;
    };
    let (Ok(right_arm), Ok(left_arm)) = (globals.get(right_arm), globals.get(left_arm)) else {
        return;
    };
    let away_from_chest =
        (right_arm.translation() - left_arm.translation()).normalize_or_zero();

    if let Ok((mut transform, mut global)) = right.single_mut() {
        transform.translation = right_arm.translation() - Vec3::Y * POLE_DROP_BELOW_SHOULDER
            + away_from_chest * POLE_OUT_ALONG_SHOULDER_LINE;
        *global = GlobalTransform::from(*transform);
    }
    if let Ok((mut transform, mut global)) = left.single_mut() {
        transform.translation = left_arm.translation() - Vec3::Y * POLE_DROP_BELOW_SHOULDER
            - away_from_chest * POLE_OUT_ALONG_SHOULDER_LINE;
        *global = GlobalTransform::from(*transform);
    }
}

fn bone_orientation_with_twist_pinned_to(along: Vec3, plane_normal: Vec3) -> Quat {
    let y = along;
    let x = y.cross(plane_normal).normalize_or_zero();
    if x == Vec3::ZERO {
        return Quat::from_rotation_arc(Vec3::Y, y);
    }
    Quat::from_mat3(&Mat3::from_cols(x, y, x.cross(y)))
}

fn elbow_bend_plane_normal(shoulder_to_target: Vec3, shoulder_to_pole: Vec3) -> Vec3 {
    shoulder_to_target
        .cross(shoulder_to_pole)
        .try_normalize()
        .unwrap_or_else(|| shoulder_to_target.any_orthonormal_vector())
}

fn shoulder_angle_from_law_of_cosines(upper_len: f32, lower_len: f32, span: f32) -> f32 {
    (((upper_len * upper_len + span * span - lower_len * lower_len)
        / (2.0 * upper_len * span))
        .clamp(-1.0, 1.0))
    .acos()
}

fn set_bone_world_rotation(
    bone: Entity,
    parent_global: GlobalTransform,
    world_rotation: Quat,
    transforms: &mut Query<(&mut Transform, &mut GlobalTransform)>,
) -> Option<GlobalTransform> {
    let (mut transform, mut global) = transforms.get_mut(bone).ok()?;
    transform.rotation = parent_global.rotation().inverse() * world_rotation;
    *global = parent_global.mul_transform(*transform);
    Some(*global)
}

fn recompute_global_from(
    bone: Entity,
    parent_global: GlobalTransform,
    transforms: &mut Query<(&mut Transform, &mut GlobalTransform)>,
) -> Option<Vec3> {
    let (transform, mut global) = transforms.get_mut(bone).ok()?;
    let local = *transform;
    *global = parent_global.mul_transform(local);
    Some(global.translation())
}

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

    let toward_target = (target_pos - shoulder).try_normalize()?;
    let span = shoulder.distance(target_pos).clamp(
        (upper_len - lower_len).abs() + 1e-4,
        (upper_len + lower_len) * MAX_EXTENSION,
    );
    let shoulder_angle = shoulder_angle_from_law_of_cosines(upper_len, lower_len, span);

    let toward_pole = (pole_pos - shoulder).try_normalize().unwrap_or(Vec3::Y);
    let bend_plane = elbow_bend_plane_normal(toward_target, toward_pole);

    let upper_dir = Quat::from_axis_angle(bend_plane, shoulder_angle) * toward_target;
    let bent_elbow = shoulder + upper_dir * upper_len;
    let lower_dir = (target_pos - bent_elbow).try_normalize()?;

    let arm_parent_global = *transforms.get(child_of.get(arm).ok()?.0).ok()?.1;
    let arm_global = set_bone_world_rotation(
        arm,
        arm_parent_global,
        bone_orientation_with_twist_pinned_to(upper_dir, bend_plane),
        transforms,
    )?;
    let fore_global = set_bone_world_rotation(
        fore_arm,
        arm_global,
        bone_orientation_with_twist_pinned_to(lower_dir, bend_plane),
        transforms,
    )?;
    let hand_pos = recompute_global_from(hand, fore_global, transforms)?;

    Some(hand_pos.distance(target_pos))
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

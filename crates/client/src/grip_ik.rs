use bevy::prelude::*;

use crate::*;

const MAX_EXTENSION: f32 = 0.995;


const POLE_DROP_BELOW_SHOULDER: f32 = 0.3;
const POLE_OUT_ALONG_SHOULDER_LINE: f32 = 0.4;

const MARKER_RADIUS_METRES: f32 = 0.022;
const DRAW_OVER_SOLID_GEOMETRY_BIAS: f32 = 1000.0;
const AXIS_BAR_LENGTH_METRES: f32 = 0.15;
const AXIS_BAR_THICKNESS_METRES: f32 = 0.008;

/// How each hand sits on the weapon, read off the rig's own "rifle aiming
/// idle" pose. Taken by walking the exported glb forward from the hips at
/// frame 1, not measured in Blender, so no rotation had to be carried across
/// a change of axis convention by hand.
///
/// Both sides build the same frame to compare against — barrel axis from one
/// grip to the other, up from the weapon — so the pose transfers even though
/// the rig was posed around a rifle whose grips sit 404mm apart against this
/// AK's 225mm.

/// Where the weapon's grip socket sits inside the hand, in the hand bone's own
/// frame: +Y runs from the wrist to the knuckles, +X across the palm, +Z out
/// through the back of the hand. The rig puts the knuckles 112mm from the
/// wrist, so Y around 0.06 lands the socket mid-palm.
///
/// The solver moves the wrist, but the weapon rests against the palm's
/// surface, which is off the bone axis in more than one direction — a single
/// slide down Y cannot express that, which is why these are vectors and why
/// the two hands do not share one.
///
/// Not guessed: a 22mm bar was laid across each palm in Blender and every
/// finger solved onto it, so this is the axis of the thing the hand is
/// actually closed around. See FINGER_GRIP_POSE, which was measured in the
/// same pass and only means anything paired with these.
///
/// Z is not a guess either. It is the palm's own skin, measured off the 60
/// mesh vertices across the middle of the hand (+0.0265) plus the bar's
/// radius, so the bar rests on the palm instead of being half buried in it.
/// An earlier 0.022 put the bar's lower surface exactly on the bone axis,
/// which looked like the hand lying on top of the weapon.
///
/// The bar is not square to the hand: it is tilted 20 degrees so its pinky end
/// sits nearer the wrist, the way a handguard actually crosses a palm. That
/// tilt lives in LEFT_HAND_ON_GUN / RIGHT_HAND_ON_GUN, not here, because this
/// is a point and the tilt is a direction.
///
/// Tunable without a rebuild: GRIP_IN_HAND_LEFT=x,y,z and GRIP_IN_HAND_RIGHT.
const LEFT_GRIP_IN_HAND: Vec3 = Vec3::new(0.0, 0.075, 0.0485);
const RIGHT_GRIP_IN_HAND: Vec3 = Vec3::new(0.0, 0.075, 0.0485);

fn grip_in_hand(variable: &str, fallback: Vec3) -> Vec3 {
    let Ok(raw) = std::env::var(variable) else { return fallback };
    let parts: Vec<f32> = raw.split(',').filter_map(|p| p.trim().parse().ok()).collect();
    match parts[..] {
        [x, y, z] => Vec3::new(x, y, z),
        _ => {
            warn!("[grip] {variable}=\"{raw}\" khong doc duoc, can dang x,y,z");
            fallback
        }
    }
}

/// Both carry a further 20 degrees about the hand's own Z over what the rig's
/// pose gave, which is the tilt of the bar the fingers were solved onto: the
/// grip runs diagonally across the palm, pinky end toward the wrist, so the
/// turn that lands the weapon in the hand has to carry that diagonal too.
/// Opposite signs because the hands are mirrored — +X runs thumb to pinky on
/// the left and pinky to thumb on the right.
const RIGHT_HAND_ON_GUN: Quat = Quat::from_xyzw(-0.354492, -0.699015, 0.283518, 0.552567);
const LEFT_HAND_ON_GUN: Quat = Quat::from_xyzw(-0.579770, -0.302469, -0.275973, 0.704427);

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
    std::env::var("GRIP_MARKERS").map(|value| value == "1").unwrap_or(false)
}

fn grip_test_targets_enabled() -> bool {
    std::env::var("GRIP_TEST").map(|value| value != "0").unwrap_or(false)
}

/// Whether the hand is clamped to the weapon's own turn, or left hanging off
/// the forearm with the local rotation the shooter pack authored for it.
///
/// Clamping is what keeps a grip from sliding as the elbow moves, but it is
/// only as good as the rotation it is handed — and a wrong one turns the whole
/// hand end-on to the eye, which reads as the hand being cut off rather than
/// as the hand being mis-aimed.
fn hand_locked_to_weapon() -> bool {
    std::env::var("HAND_LOCK").map(|value| value != "0").unwrap_or(true)
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
    camera: Query<Entity, With<FpsCamera>>,
    existing: Query<(), With<TriggerGripSocket>>,
) {
    // The gun's own grips are the mesh's Socket_RearGrip and Socket_FrontGrip,
    // tagged where they are rather than copied into the weapon table and
    // spawned again. All that is left here is the debug rig, which puts the
    // targets at fixed points off the eye so the solver can be read on its own.
    if !existing.is_empty() || !grip_test_targets_enabled() {
        return;
    }
    let Ok(parent) = camera.single() else {
        return;
    };

    let cancel_debug_pullback = Vec3::Z * view_pullback();
    let (trigger_at, fore_at, metres_to_parent_units) = (
        TEST_TARGET_RIGHT_FROM_EYE - cancel_debug_pullback,
        TEST_TARGET_LEFT_FROM_EYE - cancel_debug_pullback,
        1.0,
    );

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
        spawn_axis_gizmo(&mut commands, &mut meshes, &mut materials, trigger_entity, metres_to_parent_units);
        spawn_axis_gizmo(&mut commands, &mut meshes, &mut materials, fore_entity, metres_to_parent_units);
    }
    info!("[grip] test targets under {parent:?}");
}

fn spawn_axis_gizmo(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    parent: Entity,
    metres_to_parent_units: f32,
) {
    let thick = AXIS_BAR_THICKNESS_METRES * metres_to_parent_units;
    let length = AXIS_BAR_LENGTH_METRES * metres_to_parent_units;
    let bar = meshes.add(Cuboid::new(thick, thick, length));
    let half = length * 0.5;
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

/// Rebuilds world transforms down a subtree the solver has just moved.
///
/// The solver runs after Bevy has already propagated this frame, which is the
/// only way it can read where the gun ended up. The cost is that everything
/// hanging off the bones it moves — every finger — still holds the world
/// transform propagation gave it, so the hand walks away and leaves the
/// fingers stretched behind it.
fn repropagate_subtree(
    root: Entity,
    root_global: GlobalTransform,
    children: &Query<&Children>,
    transforms: &mut Query<(&mut Transform, &mut GlobalTransform)>,
) {
    let mut pending = vec![(root, root_global)];
    while let Some((entity, global)) = pending.pop() {
        let Ok(kids) = children.get(entity) else { continue };
        for kid in kids.iter() {
            let Ok((transform, mut kid_global)) = transforms.get_mut(kid) else {
                continue;
            };
            let kid_world = global.mul_transform(*transform);
            *kid_global = kid_world;
            pending.push((kid, kid_world));
        }
    }
}

fn solve_arm(
    arm: Entity,
    fore_arm: Entity,
    hand: Entity,
    target: Entity,
    pole: Entity,
    hand_on_gun: Quat,
    grip_in_hand: Vec3,
    child_of: &Query<&ChildOf>,
    children: &Query<&Children>,
    transforms: &mut Query<(&mut Transform, &mut GlobalTransform)>,
) -> Option<f32> {
    let shoulder = transforms.get(arm).ok()?.1.translation();
    let elbow = transforms.get(fore_arm).ok()?.1.translation();
    let wrist = transforms.get(hand).ok()?.1.translation();
    // The socket is where the gun sits in the palm, but the bone this solver
    // moves is the wrist, and the rig puts the knuckles 112mm beyond it. Aim
    // the wrist back down the hand by half of that, or the palm and every
    // posed finger end up buried inside the receiver — which looks exactly
    // like the hand not being drawn at all.
    //
    // The hand's turn is known before the arm is solved, because it comes from
    // the gun rather than from the forearm.
    let socket_global = transforms.get(target).ok()?.1;
    let gun_rotation = socket_global.rotation();
    let grip_rotation = gun_rotation * hand_on_gun;
    let target_pos =
        socket_global.translation() - grip_rotation * grip_in_hand;
    let pole_pos = transforms.get(pole).ok()?.1.translation();

    let upper_len = elbow.distance(shoulder);
    let lower_len = wrist.distance(elbow);
    if upper_len < 1e-5 || lower_len < 1e-5 {
        return None;
    }
    if std::env::var("REACH_LOG").is_ok() {
        info!(
            "[reach] vai {:.3?} tam {:.3?} | tay dai {:.3}+{:.3}={:.3} | can {:.3}",
            shoulder, target_pos, upper_len, lower_len, upper_len + lower_len,
            shoulder.distance(target_pos),
        );
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
    // The hand does not hang off the forearm. It is clamped to the gun, so the
    // palm and the posed fingers keep their grip wherever the solver had to put
    // the elbow — and the grip turns with the weapon through recoil and sway
    // without anything here having to know about either.
    //
    let hand_global = if hand_locked_to_weapon() {
        set_bone_world_rotation(hand, fore_global, grip_rotation, transforms)?
    } else {
        let hand_local = *transforms.get(hand).ok()?.0;
        let global = fore_global.mul_transform(hand_local);
        *transforms.get_mut(hand).ok()?.1 = global;
        global
    };

    repropagate_subtree(hand, hand_global, children, transforms);

    Some(hand_global.translation().distance(target_pos))
}

pub(crate) fn solve_grip_ik(
    time: Res<Time>,
    bones: Query<&PlayerModelSkeletonBones, With<ViewmodelBody>>,
    trigger: Query<Entity, With<TriggerGripSocket>>,
    fore: Query<Entity, With<ForeGripSocket>>,
    right_pole: Query<Entity, With<RightPoleTarget>>,
    left_pole: Query<Entity, With<LeftPoleTarget>>,
    child_of: Query<&ChildOf>,
    children: Query<&Children>,
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
            let offset = grip_in_hand("GRIP_IN_HAND_RIGHT", RIGHT_GRIP_IN_HAND);
            solve_arm(arm, fore_arm, hand, trigger, right_pole, RIGHT_HAND_ON_GUN, offset, &child_of, &children, &mut transforms)
        },
    );
    let left = bones.left_arm.zip(bones.left_fore_arm).zip(bones.left_hand).and_then(
        |((arm, fore_arm), hand)| {
            let offset = grip_in_hand("GRIP_IN_HAND_LEFT", LEFT_GRIP_IN_HAND);
            solve_arm(arm, fore_arm, hand, fore, left_pole, LEFT_HAND_ON_GUN, offset, &child_of, &children, &mut transforms)
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

/// Puts a marker on the hand bones themselves, which answers a question the
/// socket markers cannot: whether the hand is where the solver says it is.
/// A zero residual only proves the bone reached the target, not that anything
/// is drawn there.
pub(crate) fn mark_hand_bones(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    bones: Query<&PlayerModelSkeletonBones, With<ViewmodelBody>>,
    mut done: Local<bool>,
) {
    if *done || !grip_markers_enabled() {
        return;
    }
    let Ok(bones) = bones.single() else { return };
    let (Some(left), Some(right)) = (bones.left_hand, bones.right_hand) else {
        return;
    };
    *done = true;
    let mesh = meshes.add(Sphere::new(MARKER_RADIUS_METRES));
    for (bone, glow) in [
        (left, LinearRgba::rgb(0.5, 6.0, 1.2)),
        (right, LinearRgba::rgb(6.0, 0.6, 0.4)),
    ] {
        let colour = Color::linear_rgb(glow.red.min(1.0), glow.green.min(1.0), glow.blue.min(1.0));
        commands.spawn((
            Name::new("HandBoneMarker"),
            Mesh3d(mesh.clone()),
            MeshMaterial3d(materials.add(glowing_unlit_material(colour, glow))),
            Transform::default(),
            RenderLayers::layer(VIEWMODEL_LAYER),
            ChildOf(bone),
        ));
    }
    info!("[grip] hand bone markers on");
}

/// Measures the hand in world space against the length the rig was built with.
///
/// A stretched finger is not a bad pose, it is a bad transform: the knuckle
/// sits 112mm from the wrist in the asset, so any other number here means
/// something upstream is scaling the skeleton the skin was bound to.
pub(crate) fn log_hand_scale(
    bones: Query<&PlayerModelSkeletonBones, With<ViewmodelBody>>,
    names: Query<&Name>,
    globals: Query<&GlobalTransform>,
    children: Query<&Children>,
    time: Res<Time>,
    mut next: Local<f32>,
) {
    if std::env::var("HAND_LOG").is_err() || time.elapsed_secs() < *next {
        return;
    }
    *next = time.elapsed_secs() + 1.0;
    let Ok(bones) = bones.single() else { return };
    for (side, hand) in [("trai", bones.left_hand), ("phai", bones.right_hand)] {
        let Some(hand) = hand else { continue };
        let Ok(hand_global) = globals.get(hand) else { continue };
        let (scale, _, _) = hand_global.to_scale_rotation_translation();
        let knuckle = children.get(hand).ok().and_then(|kids| {
            kids.iter().find(|kid| {
                names.get(*kid).is_ok_and(|n| n.as_str().ends_with("HandMiddle1"))
            })
        });
        let dist = knuckle
            .and_then(|k| globals.get(k).ok())
            .map(|k| k.translation().distance(hand_global.translation()));
        info!(
            "[hand] {side} scale={:.3?} co tay->dot giua={:?} (rig: 0.112)",
            scale,
            dist.map(|d| format!("{d:.4}m")),
        );
    }
}

use bevy::prelude::*;

use crate::*;

#[derive(Component)]
pub(crate) struct BoneRest(pub(crate) Quat);

#[derive(Component)]
pub(crate) struct ViewmodelBody;

/// Drops the body so the camera sits at the character's own eyes. Measured
/// against the rig rather than nudged: soldier_shooter stands 1.794m with its
/// feet on the origin and its head bone at 1.532, which puts the eyes near
/// 1.63.
const BODY_OFFSET_FROM_EYE: Vec3 = Vec3::new(0.0, -1.63, 0.0);

const HEAD_COLLAPSE_SCALE: f32 = 0.0001;
/// 1.0 for a rig that is already human-proportioned. This was 2.07 to stretch
/// a mesh whose arms could not otherwise reach the gun, which also doubled
/// their thickness — the slab of skin that used to fill the view.
const ARM_SCALE: f32 = 1.0;
const HAND_SCALE_CANCELLING_ARM_SCALE: f32 = 1.0 / ARM_SCALE;
const HIP_BLADE_DEG: f32 = -30.0;

const UPPER_ARM_AIM_IN_BODY_FRAME: Vec3 = Vec3::new(0.0, -0.45, -1.0);
const FORE_ARM_AIM_IN_BODY_FRAME: Vec3 = Vec3::new(0.0, -0.10, -1.0);

const MAX_BONE_DEPTH: usize = 20;

pub(crate) fn spawn_viewmodel_body(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    tuning: Res<PlayerModelTuning>,
    camera: Query<Entity, With<FpsCamera>>,
    existing: Query<(), With<ViewmodelBody>>,
) {
    if !existing.is_empty() {
        return;
    }
    let Ok(camera) = camera.single() else {
        return;
    };

    commands.spawn((
        Name::new("ViewmodelBody (first-person, headless)"),
        WorldAssetRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(tuning.mesh.clone()))),
        Transform::from_translation(BODY_OFFSET_FROM_EYE - Vec3::Z * view_pullback())
            .with_rotation(Quat::from_rotation_y(tuning.facing_deg.to_radians()))
            .with_scale(Vec3::splat(tuning.scale)),
        ViewmodelBody,
        PlayerModelAnimRoot,
        PlayerModelSkeletonBones::default(),
        ChildOf(camera),
    ));
}

pub(crate) fn wire_viewmodel_body_meshes(
    mut commands: Commands,
    child_of: Query<&ChildOf>,
    body: Query<(), With<ViewmodelBody>>,
    new_meshes: Query<Entity, Added<Mesh3d>>,
) {
    for mesh_entity in &new_meshes {
        let mut current = mesh_entity;
        for _ in 0..MAX_BONE_DEPTH {
            if body.contains(current) {
                commands.entity(mesh_entity).insert(RenderLayers::layer(VIEWMODEL_LAYER));
                break;
            }
            let Ok(parent) = child_of.get(current) else { break };
            current = parent.0;
        }
    }
}

fn rotation_from_root(
    bone: Entity,
    root: Entity,
    child_of: &Query<&ChildOf>,
    transforms: &Query<&mut Transform>,
) -> Option<Quat> {
    let mut chain = Vec::new();
    let mut current = bone;
    while current != root {
        chain.push(current);
        current = child_of.get(current).ok()?.0;
    }
    let mut rotation = Quat::IDENTITY;
    for entity in chain.into_iter().rev() {
        rotation *= transforms.get(entity).ok()?.rotation;
    }
    Some(rotation)
}

fn body_vertical_in_parent_frame_of(
    bone: Entity,
    root: Entity,
    child_of: &Query<&ChildOf>,
    transforms: &Query<&mut Transform>,
) -> Option<Vec3> {
    let parent = child_of.get(bone).ok()?.0;
    let parent_rotation = rotation_from_root(parent, root, child_of, transforms)?;
    Some(parent_rotation.inverse() * Vec3::Y)
}

fn set_scale(bone: Option<Entity>, scale: f32, transforms: &mut Query<&mut Transform>) {
    let Some(bone) = bone else { return };
    let Ok(mut transform) = transforms.get_mut(bone) else { return };
    transform.scale = Vec3::splat(scale);
}

pub(crate) fn shape_viewmodel_skeleton(
    bodies: Query<(Entity, &PlayerModelSkeletonBones), With<ViewmodelBody>>,
    rests: Query<&BoneRest>,
    child_of: Query<&ChildOf>,
    mut transforms: Query<&mut Transform>,
) {
    let Ok((root, bones)) = bodies.single() else { return };

    if bones.head.is_none() {
        warn_once!("[viewmodel] Head bone not wired, the head will block the view");
    }
    set_scale(bones.head, HEAD_COLLAPSE_SCALE, &mut transforms);
    set_scale(bones.right_arm, ARM_SCALE, &mut transforms);
    set_scale(bones.left_arm, ARM_SCALE, &mut transforms);
    set_scale(bones.right_hand, HAND_SCALE_CANCELLING_ARM_SCALE, &mut transforms);
    set_scale(bones.left_hand, HAND_SCALE_CANCELLING_ARM_SCALE, &mut transforms);

    if let Some(hips) = bones.hips {
        let axis = body_vertical_in_parent_frame_of(hips, root, &child_of, &transforms);
        if let (Some(axis), Ok(rest)) = (axis, rests.get(hips)) {
            if let Ok(mut transform) = transforms.get_mut(hips) {
                transform.rotation =
                    Quat::from_axis_angle(axis, HIP_BLADE_DEG.to_radians()) * rest.0;
            }
        }
    }
}

pub(crate) fn pose_viewmodel_arms(
    bodies: Query<(Entity, &PlayerModelSkeletonBones), With<ViewmodelBody>>,
    child_of: Query<&ChildOf>,
    mut transforms: Query<&mut Transform>,
) {
    let Ok((root, bones)) = bodies.single() else { return };

    for (bone, aim) in [
        (bones.right_arm, UPPER_ARM_AIM_IN_BODY_FRAME),
        (bones.right_fore_arm, FORE_ARM_AIM_IN_BODY_FRAME),
        (bones.left_arm, UPPER_ARM_AIM_IN_BODY_FRAME),
        (bones.left_fore_arm, FORE_ARM_AIM_IN_BODY_FRAME),
    ] {
        let Some(bone) = bone else { continue };
        let Ok(parent) = child_of.get(bone) else { continue };
        let Some(parent_rotation) = rotation_from_root(parent.0, root, &child_of, &transforms)
        else {
            continue;
        };
        let aim_in_parent = parent_rotation.inverse() * aim.normalize_or_zero();
        let Ok(mut transform) = transforms.get_mut(bone) else { continue };
        transform.rotation = Quat::from_rotation_arc(Vec3::Y, aim_in_parent);
    }
}

/// Solved in Blender against a 22mm bar laid across each palm, every joint
/// curled until it just touched. Thumbs are still the rig's own rifle idle;
/// its fingers closed around something far larger than a handguard, which is
/// why the hand read as holding nothing.
///
/// Parent-relative, the form the loader uses. Only valid paired with
/// LEFT_GRIP_IN_HAND / RIGHT_GRIP_IN_HAND, which is where that bar sits.
///
/// They are forced every frame rather than left to the clip because only two
/// of the four clips the player runs are rifle poses: walking and running
/// backwards came from Mixamo with open hands, and the grip would let go
/// halfway through a step.
///
/// The ten fingertip bones are not here: their rotations are identity.
const FINGER_GRIP_POSE: &[(&str, Quat)] = &[
    ("LeftHandIndex1", Quat::from_xyzw(0.43597, 0.02352, 0.04846, 0.89835)),
    ("LeftHandIndex2", Quat::from_xyzw(0.38315, 0.00000, 0.00000, 0.92369)),
    ("LeftHandIndex3", Quat::from_xyzw(0.27647, 0.00000, 0.00000, 0.96102)),
    ("LeftHandMiddle1", Quat::from_xyzw(0.42245, -0.00591, -0.01269, 0.90628)),
    ("LeftHandMiddle2", Quat::from_xyzw(0.46652, 0.00000, 0.00000, 0.88451)),
    ("LeftHandMiddle3", Quat::from_xyzw(0.30535, 0.00000, 0.00000, 0.95224)),
    ("LeftHandPinky1", Quat::from_xyzw(0.56899, -0.00243, -0.00351, 0.82234)),
    ("LeftHandPinky2", Quat::from_xyzw(0.56859, 0.00000, 0.00000, 0.82262)),
    ("LeftHandPinky3", Quat::from_xyzw(0.22812, 0.00000, 0.00000, 0.97363)),
    ("LeftHandRing1", Quat::from_xyzw(0.47205, -0.00991, -0.01850, 0.88132)),
    ("LeftHandRing2", Quat::from_xyzw(0.46805, 0.00000, 0.00000, 0.88370)),
    ("LeftHandRing3", Quat::from_xyzw(0.22178, 0.00000, 0.00000, 0.97510)),
    ("LeftHandThumb1", Quat::from_xyzw(0.13362, 0.06153, 0.16997, 0.97441)),
    ("LeftHandThumb2", Quat::from_xyzw(0.10248, 0.17281, -0.04630, 0.97852)),
    ("LeftHandThumb3", Quat::from_xyzw(-0.12531, 0.02766, 0.08647, 0.98796)),
    ("RightHandIndex1", Quat::from_xyzw(0.45814, -0.02501, -0.04844, 0.88721)),
    ("RightHandIndex2", Quat::from_xyzw(0.39491, 0.00000, 0.00000, 0.91872)),
    ("RightHandIndex3", Quat::from_xyzw(0.26307, 0.00000, 0.00000, 0.96478)),
    ("RightHandMiddle1", Quat::from_xyzw(0.44409, 0.00701, 0.01415, 0.89584)),
    ("RightHandMiddle2", Quat::from_xyzw(0.46223, 0.00000, 0.00000, 0.88676)),
    ("RightHandMiddle3", Quat::from_xyzw(0.26451, 0.00000, 0.00000, 0.96438)),
    ("RightHandPinky1", Quat::from_xyzw(0.54759, 0.01368, 0.02090, 0.83637)),
    ("RightHandPinky2", Quat::from_xyzw(0.54975, 0.00000, 0.00000, 0.83533)),
    ("RightHandPinky3", Quat::from_xyzw(0.33212, 0.00000, 0.00000, 0.94324)),
    ("RightHandRing1", Quat::from_xyzw(0.50519, 0.01306, 0.02231, 0.86262)),
    ("RightHandRing2", Quat::from_xyzw(0.46981, 0.00000, 0.00000, 0.88277)),
    ("RightHandRing3", Quat::from_xyzw(0.12171, 0.00000, 0.00000, 0.99257)),
    ("RightHandThumb1", Quat::from_xyzw(0.24710, -0.08230, -0.20471, 0.94353)),
    ("RightHandThumb2", Quat::from_xyzw(0.01796, -0.20757, 0.21122, 0.95498)),
    ("RightHandThumb3", Quat::from_xyzw(-0.18776, -0.04791, 0.45418, 0.86958)),
];

/// Marks one finger bone with the rotation it should hold.
#[derive(Component)]
pub(crate) struct GripFingerPose(pub(crate) Quat);

/// Writes the grip back over whatever the clip did. Has to land after the
/// animation has run and before transforms propagate, or it is either
/// overwritten or a frame late.
pub(crate) fn hold_finger_grip(mut fingers: Query<(&GripFingerPose, &mut Transform)>) {
    for (pose, mut transform) in &mut fingers {
        transform.rotation = pose.0;
    }
}

/// The rotation for a bone name, if it is one of the fingers we pose.
pub(crate) fn finger_grip_rotation(bare_name: &str) -> Option<Quat> {
    FINGER_GRIP_POSE
        .iter()
        .find(|(name, _)| *name == bare_name)
        .map(|(_, rotation)| *rotation)
}

pub(crate) fn log_body_swing(
    camera: Query<(&GlobalTransform, &FpsCamera, &ViewRecoil)>,
    body: Query<&GlobalTransform, With<ViewmodelBody>>,
    time: Res<Time>,
    mut rest_reference: Local<Option<f32>>,
    mut next: Local<f32>,
) {
    if std::env::var("PUSH_LOG").is_err() || time.elapsed_secs() < *next {
        return;
    }
    *next = time.elapsed_secs() + 0.1;
    let (Ok((eye, cam, recoil)), Ok(body)) = (camera.single(), body.single()) else {
        return;
    };

    let from_eye = body.translation() - eye.translation();
    let flat = Vec2::new(from_eye.x, from_eye.z).length();

    // Aiming moves the body too, on the same lever, so the only honest way to
    // read the recoil's share is against where the body sat at this same aim
    // with no kick on it.
    if recoil.pitch_kick.abs() < 1e-4 {
        *rest_reference = Some(flat);
    }
    let swing = rest_reference.map(|at_rest| flat - at_rest);

    info!(
        "[push] ngam={:+5.1}deg giat={:+4.1}deg | than nguoi: ngang={:.3}m doc={:.3}m | lech do giat={}",
        cam.pitch.to_degrees(),
        recoil.pitch_kick.to_degrees(),
        flat,
        from_eye.y,
        swing.map(|d| format!("{:+.3}m", d)).unwrap_or_else(|| "chua co moc".into()),
    );
}

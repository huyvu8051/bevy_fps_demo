use bevy::prelude::*;

use crate::*;

#[derive(Component)]
pub(crate) struct BoneRest(pub(crate) Quat);

#[derive(Component)]
pub(crate) struct ViewmodelBody;

const BODY_OFFSET_FROM_EYE: Vec3 = Vec3::new(0.0, -1.471, 0.0);
const BODY_YAW_DEG: f32 = 0.0;

const HEAD_COLLAPSE_SCALE: f32 = 0.0001;
const ARM_SCALE: f32 = 2.07;
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
            .with_rotation(Quat::from_rotation_y(BODY_YAW_DEG.to_radians()))
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

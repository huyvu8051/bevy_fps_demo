use bevy::prelude::*;

use crate::*;

/// A bone's local rotation as the rig authored it, captured before any clip has
/// touched it. Poses below are applied as a delta ON TOP of this: overwriting
/// the rotation outright would throw away the bind orientation and fold the
/// arm into nonsense.
// Captured at wire time and kept even though the aim-based pose below does not
// read it: it is the only chance to record a bind rotation before a clip poses
// the bone, and getting it back later means re-threading the wiring.
#[derive(Component)]
#[allow(dead_code)]
pub(crate) struct BoneRest(pub(crate) Quat);

/// The player's own body, drawn only for the first-person camera.
#[derive(Component)]
pub(crate) struct ViewmodelBody;

/// Where the body sits relative to the eye. The camera rides EYE_HEIGHT_OFFSET
/// (0.65) above the player origin and the rig stands y_offset (-0.76) below it,
/// so the body origin belongs 1.41 under the eye.
///
/// Yaw is 0, settled by measurement rather than by reasoning about the rig:
/// with the half turn applied, the RightArm bone logged at camera x -0.17 while
/// the rifle sits at +0.12, putting the trigger hand on the far side of the
/// body. That costs 0.547m of reach against a 0.452m arm. Unturned, the same
/// shoulder lands at +0.17 and the distance falls to 0.466m.
const BODY_OFFSET: Vec3 = Vec3::new(0.0, -1.41, 0.0);
const BODY_YAW_DEG: f32 = 0.0;

/// The mesh is a single skinned primitive, so there is no submesh to hide. The
/// head is a skin joint though, so collapsing that bone pulls every vertex
/// weighted to it down to a point — the standard headless first-person body.
const HEAD_COLLAPSE_SCALE: f32 = 0.0001;

/// Viewmodel arms are longer than real ones in every shooter, because the
/// weapon is placed where it reads well on screen rather than where a person
/// could hold it. This rig needs more help than most: its arms span 0.452m
/// against roughly 0.59m on a person, while the support hand has to cross the
/// body to a rifle held at x 0.22, which is 0.707m away.
///
/// Scaling the upper arm carries the whole chain, since the forearm hangs off
/// it at a scaled offset and inherits the scale itself.
const ARM_SCALE: f32 = 1.97;

/// Cancels ARM_SCALE on the hands so they stay life-sized. Only the arms
/// should look stretched; oversized hands read as a bug immediately.
const HAND_SCALE: f32 = 1.0 / ARM_SCALE;

/// Rifle carry, as deltas from the bind pose about each bone's own axes.
///
/// Bones run along local +Y, so X swings the limb forward, Z swings it in
/// toward the body, Y twists it about its own length.

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
        Transform::from_translation(BODY_OFFSET - Vec3::Z * view_pullback())
            .with_rotation(Quat::from_rotation_y(BODY_YAW_DEG.to_radians()))
            .with_scale(Vec3::splat(tuning.scale)),
        ViewmodelBody,
        PlayerModelAnimRoot,
        PlayerModelSkeletonBones::default(),
        ChildOf(camera),
    ));
    info!("[viewmodel] body spawned under FpsCamera={camera:?}");
}

/// RenderLayers does not reach descendants on its own — `wire_gun_mesh` stamps
/// the gun's meshes the same way.
pub(crate) fn wire_viewmodel_body_meshes(
    mut commands: Commands,
    child_of: Query<&ChildOf>,
    body: Query<(), With<ViewmodelBody>>,
    new_meshes: Query<Entity, Added<Mesh3d>>,
    mut stamped: Local<usize>,
) {
    for mesh_entity in &new_meshes {
        let mut current = mesh_entity;
        for _ in 0..20 {
            if body.contains(current) {
                commands.entity(mesh_entity).insert(RenderLayers::layer(VIEWMODEL_LAYER));
                *stamped += 1;
                info!("[viewmodel] mesh {mesh_entity:?} -> VIEWMODEL_LAYER (total {})", *stamped);
                break;
            }
            let Ok(parent) = child_of.get(current) else { break };
            current = parent.0;
        }
    }
}

pub(crate) fn shape_viewmodel_skeleton(
    bodies: Query<(&PlayerModelSkeletonBones, &GlobalTransform), With<ViewmodelBody>>,
    mut transforms: Query<&mut Transform>,
    mut logged: Local<bool>,
) {
    let Ok((bones, global)) = bodies.single() else { return };
    let Some(head) = bones.head else {
        if !*logged {
            *logged = true;
            info!("[viewmodel] Head bone NOT wired — head will block the view");
        }
        return;
    };
    if let Ok(mut transform) = transforms.get_mut(head) {
        transform.scale = Vec3::splat(HEAD_COLLAPSE_SCALE);
    }
    for arm in [bones.right_arm, bones.left_arm] {
        let Some(arm) = arm else { continue };
        let Ok(mut transform) = transforms.get_mut(arm) else { continue };
        transform.scale = Vec3::splat(ARM_SCALE);
    }
    for hand in [bones.right_hand, bones.left_hand] {
        let Some(hand) = hand else { continue };
        let Ok(mut transform) = transforms.get_mut(hand) else { continue };
        transform.scale = Vec3::splat(HAND_SCALE);
    }
    if !*logged {
        *logged = true;
        info!(
            "[viewmodel] head collapsed; body world pos {:.2?} scale {:.3}",
            global.translation(),
            global.scale().x
        );
    }
}

/// Aim directions in the body root frame. The body is parented to the camera
/// with zero yaw, so its axes ARE the camera axes: -Z is straight ahead.
const UPPER_ARM_AIM: Vec3 = Vec3::new(0.0, -0.45, -1.0);
const FORE_ARM_AIM: Vec3 = Vec3::new(0.0, -0.10, -1.0);

/// Accumulated rotation from the body root down to (and including) `bone`,
/// built from local transforms rather than GlobalTransform on purpose: this
/// system runs before transform propagation, so the globals are a frame stale
/// and would lag the pose by a frame every time the camera turns — exactly the
/// artefact being investigated.
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

/// Points each arm bone along a chosen direction instead of nudging it by hand-
/// picked Euler angles. The angles were guesswork; this is solved.
pub(crate) fn pose_viewmodel_arms(
    bodies: Query<(Entity, &PlayerModelSkeletonBones), With<ViewmodelBody>>,
    child_of: Query<&ChildOf>,
    mut transforms: Query<&mut Transform>,
    mut last_applied: Local<Option<usize>>,
) {
    let Ok((root, bones)) = bodies.single() else { return };

    let mut applied = 0;
    for (bone, aim) in [
        (bones.right_arm, UPPER_ARM_AIM),
        (bones.right_fore_arm, FORE_ARM_AIM),
        (bones.left_arm, UPPER_ARM_AIM),
        (bones.left_fore_arm, FORE_ARM_AIM),
    ] {
        let Some(bone) = bone else { continue };
        let Ok(parent) = child_of.get(bone) else { continue };
        let Some(parent_rotation) = rotation_from_root(parent.0, root, &child_of, &transforms) else {
            continue;
        };
        // bones run along their own +Y, so turn +Y onto the aim expressed in
        // the parent frame
        let aim_in_parent = parent_rotation.inverse() * aim.normalize_or_zero();
        let Ok(mut transform) = transforms.get_mut(bone) else { continue };
        transform.rotation = Quat::from_rotation_arc(Vec3::Y, aim_in_parent);
        applied += 1;
    }
    if *last_applied != Some(applied) {
        *last_applied = Some(applied);
        info!("[viewmodel] arm aim applied to {applied}/4 bones");
    }
}




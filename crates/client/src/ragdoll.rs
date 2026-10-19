use avian3d::prelude::*;
use bevy::prelude::*;

use crate::*;

const HANDOFF_AFTER_SECONDS: f32 = 0.45;
pub(crate) const DEATH_PUSH_SPEED: f32 = 2.2;
const SETTLE_SPEED: f32 = 0.35;
const SETTLE_HOLD_SECONDS: f32 = 0.4;
const MAX_RAGDOLL_SECONDS: f32 = 6.0;

const BONE_DENSITY: f32 = 900.0;
const MIN_BONE_LENGTH: f32 = 0.06;
const LEAF_BONE_LENGTH: f32 = 0.12;

const SWING_LIMIT_SPINE: f32 = 0.5;
const SWING_LIMIT_SHOULDER: f32 = 1.6;
const SWING_LIMIT_ELBOW: f32 = 0.1;
const SWING_LIMIT_HIP: f32 = 1.2;
const SWING_LIMIT_KNEE: f32 = 0.1;
const TWIST_LIMIT: f32 = 0.4;

const ELBOW_RANGE: (f32, f32) = (-2.4, 0.0);
const KNEE_RANGE: (f32, f32) = (0.0, 2.4);

#[derive(Component)]
pub(crate) struct RagdollArming {
    captured: Vec<(Entity, Vec3, Quat)>,
}

#[derive(Component)]
pub(crate) struct Ragdolling {
    bodies: Vec<(Entity, Entity)>,
    settled_for: f32,
    elapsed: f32,
}

#[derive(Component)]
pub(crate) struct RagdollBody;

#[derive(Clone, Copy)]
enum JointKind {
    Spine,
    Shoulder,
    Elbow,
    Hip,
    Knee,
}

struct BoneLink {
    bone: Entity,
    parent_slot: Option<usize>,
    joint: JointKind,
    radius: f32,
}

fn bone_links(bones: &ZombieHitboxBones) -> Option<Vec<BoneLink>> {
    let radius_torso = TORSO_HITBOX_RADIUS;
    let radius_limb = LIMB_HITBOX_RADIUS;
    let radius_head = HEAD_HITBOX_RADIUS;

    let mut links = Vec::with_capacity(15);
    let mut push = |bone: Option<Entity>, parent_slot, joint, radius| -> Option<usize> {
        let bone = bone?;
        links.push(BoneLink { bone, parent_slot, joint, radius });
        Some(links.len() - 1)
    };

    let belly = push(bones.belly, None, JointKind::Spine, radius_torso)?;
    let chest = push(bones.chest, Some(belly), JointKind::Spine, radius_torso)?;
    push(bones.head, Some(chest), JointKind::Spine, radius_head);

    for arm in [&bones.left_arm, &bones.right_arm] {
        let mut parent = chest;
        for (depth, joint) in [JointKind::Shoulder, JointKind::Elbow, JointKind::Elbow].into_iter().enumerate() {
            let Some(slot) = push(arm[depth], Some(parent), joint, radius_limb) else { break };
            parent = slot;
        }
    }
    for leg in [&bones.left_leg, &bones.right_leg] {
        let mut parent = belly;
        for (depth, joint) in [JointKind::Hip, JointKind::Knee, JointKind::Knee].into_iter().enumerate() {
            let Some(slot) = push(leg[depth], Some(parent), joint, radius_limb) else { break };
            parent = slot;
        }
    }
    Some(links)
}

/// Captures where every bone is one frame before physics takes over, so the
/// next frame can work out how fast each was already moving. Handing a ragdoll
/// the pose but not the motion is what makes a body stop dead and then drop.
pub(crate) fn arm_ragdoll(
    mut commands: Commands,
    globals: Query<&GlobalTransform>,
    arming: Query<(Entity, &ZombieHitboxBones), (With<DeathClipPlaying>, Without<RagdollArming>)>,
    ready: Query<Entity, With<ReadyToRagdoll>>,
) {
    for entity in &ready {
        let Ok((_, bones)) = arming.get(entity) else { continue };
        let Some(links) = bone_links(bones) else { continue };
        let captured = links
            .iter()
            .filter_map(|link| {
                let global = globals.get(link.bone).ok()?;
                Some((link.bone, global.translation(), global.rotation()))
            })
            .collect();
        commands
            .entity(entity)
            .remove::<ReadyToRagdoll>()
            .insert(RagdollArming { captured });
    }
}

#[derive(Component)]
pub(crate) struct ReadyToRagdoll;

#[derive(Component)]
pub(crate) struct DeathClipPlaying;

/// Runs before `arm_ragdoll` on purpose. Arming records bone transforms and
/// this consumes them, so putting it first leaves exactly one frame between
/// the two — which is where the per-bone velocity comes from. Run them the
/// other way round and every capture is same-frame, every velocity is zero,
/// and the corpse stops dead before it falls.
pub(crate) fn spawn_what_was_armed_last_frame(
    time: Res<Time>,
    mut commands: Commands,
    globals: Query<&GlobalTransform>,
    arming: Query<(
        Entity,
        &ZombieHitboxBones,
        &RagdollArming,
        &DeathImpulse,
        Option<&ZombieAnimationPlayer>,
    )>,
    mut players: Query<&mut AnimationPlayer>,
) {
    let dt = time.delta_secs().max(1e-4);
    for (entity, bones, armed, impulse, link) in &arming {
        // The clip has to stop here. Left running it keeps writing bone
        // rotations every frame and simply overwrites whatever physics does.
        if let Some(link) = link {
            if let Ok(mut player) = players.get_mut(link.0) {
                player.stop_all();
            }
        }

        let Some(links) = bone_links(bones) else {
            commands.entity(entity).remove::<RagdollArming>();
            continue;
        };

        let mut bodies: Vec<Option<Entity>> = vec![None; links.len()];
        let mut pairs = Vec::with_capacity(links.len());

        for (slot, link) in links.iter().enumerate() {
            let Ok(global) = globals.get(link.bone) else { continue };
            let position = global.translation();
            let rotation = global.rotation();

            let length = links
                .iter()
                .find(|child| child.parent_slot == Some(slot))
                .and_then(|child| globals.get(child.bone).ok())
                .map(|child| child.translation().distance(position))
                .unwrap_or(LEAF_BONE_LENGTH)
                .max(MIN_BONE_LENGTH);

            let (velocity, spin) = armed
                .captured
                .iter()
                .find(|(bone, _, _)| *bone == link.bone)
                .map(|&(_, was_at, was_facing)| {
                    let turn = rotation * was_facing.inverse();
                    let (axis, angle) = turn.to_axis_angle();
                    ((position - was_at) / dt, axis * (angle / dt))
                })
                .unwrap_or((Vec3::ZERO, Vec3::ZERO));

            // Avian 0.7 has no impulse component, so the shot rides in as extra
            // starting velocity on the torso.
            let velocity = if slot == 0 { velocity + impulse.push } else { velocity };

            let body = commands
                .spawn((
                    Name::new("RagdollBone"),
                    RigidBody::Dynamic,
                    Collider::capsule(link.radius, length),
                    ColliderDensity(BONE_DENSITY),
                    Transform::from_translation(position).with_rotation(rotation),
                    LinearVelocity(velocity),
                    AngularVelocity(spin),
                    RagdollBody,
                ))
                .id();
            bodies[slot] = Some(body);
            pairs.push((link.bone, body));
        }

        for (slot, link) in links.iter().enumerate() {
            let (Some(body), Some(parent_slot)) = (bodies[slot], link.parent_slot) else { continue };
            let Some(parent_body) = bodies[parent_slot] else { continue };
            commands.spawn((joint_for(link.joint, parent_body, body), JointCollisionDisabled));
        }


        commands
            .entity(entity)
            .remove::<RagdollArming>()
            .remove::<DeathClipPlaying>()
            .insert(Ragdolling { bodies: pairs, settled_for: 0.0, elapsed: 0.0 });
    }
}

fn joint_for(kind: JointKind, parent: Entity, child: Entity) -> SphericalJoint {
    let (swing, twist) = match kind {
        JointKind::Spine => (SWING_LIMIT_SPINE, TWIST_LIMIT),
        JointKind::Shoulder => (SWING_LIMIT_SHOULDER, TWIST_LIMIT),
        JointKind::Elbow => (SWING_LIMIT_ELBOW, ELBOW_RANGE.1 - ELBOW_RANGE.0),
        JointKind::Hip => (SWING_LIMIT_HIP, TWIST_LIMIT),
        JointKind::Knee => (SWING_LIMIT_KNEE, KNEE_RANGE.1 - KNEE_RANGE.0),
    };
    SphericalJoint {
        swing_limit: Some(AngleLimit { min: -swing, max: swing }),
        twist_limit: Some(AngleLimit { min: -twist, max: twist }),
        ..SphericalJoint::new(parent, child)
    }
}

/// Writes the simulated pose back onto the skeleton. Only rotation is copied:
/// bone positions come from the hierarchy, so copying those too would fight it
/// and let the body stretch.
pub(crate) fn drive_skeleton_from_ragdoll(
    time: Res<Time>,
    mut commands: Commands,
    mut corpses: Query<(Entity, &mut Ragdolling, &mut Transform), Without<RagdollBody>>,
    child_of: Query<&ChildOf>,
    globals: Query<&GlobalTransform>,
    speeds: Query<(&LinearVelocity, &AngularVelocity), With<RagdollBody>>,
    mut bones: Query<&mut Transform, (Without<Ragdolling>, Without<RagdollBody>)>,
) {
    for (entity, mut ragdoll, mut root) in &mut corpses {
        ragdoll.elapsed += time.delta_secs();

        let mut fastest: f32 = 0.0;
        for &(bone, body) in &ragdoll.bodies {
            if let Ok((linear, angular)) = speeds.get(body) {
                fastest = fastest.max(linear.0.length()).max(angular.0.length() * 0.2);
            }
            let (Ok(body_global), Ok(parent)) = (globals.get(body), child_of.get(bone)) else {
                continue;
            };
            let Ok(parent_global) = globals.get(parent.0) else { continue };
            let Ok(mut transform) = bones.get_mut(bone) else { continue };
            transform.rotation = parent_global.rotation().inverse() * body_global.rotation();
        }

        if let Some(&(_, root_body)) = ragdoll.bodies.first() {
            if let Ok(global) = globals.get(root_body) {
                root.translation = global.translation();
            }
        }

        ragdoll.settled_for = if fastest < SETTLE_SPEED {
            ragdoll.settled_for + time.delta_secs()
        } else {
            0.0
        };

        if ragdoll.settled_for >= SETTLE_HOLD_SECONDS || ragdoll.elapsed >= MAX_RAGDOLL_SECONDS {
            for &(_, body) in &ragdoll.bodies {
                commands.entity(body).despawn();
            }
            commands.entity(entity).remove::<Ragdolling>().insert(CorpseSettled);
        }
    }
}

#[derive(Component)]
pub(crate) struct CorpseSettled;

#[derive(Component)]
pub(crate) struct DeathImpulse {
    pub(crate) push: Vec3,
}

pub(crate) const RAGDOLL_HANDOFF_SECONDS: f32 = HANDOFF_AFTER_SECONDS;

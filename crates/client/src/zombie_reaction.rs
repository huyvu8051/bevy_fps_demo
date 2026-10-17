use bevy::prelude::*;

use crate::*;

pub(crate) fn spawn_zombie_hit_feedback(
    commands: &mut Commands,
    blood_assets: &BloodAssets,
    spark_assets: &SparkAssets,
    spatial_query: &SpatialQuery,
    time: &Time,
    point: Vec3,
    normal: Vec3,
    damage: f32,
    bone_for_blood: Option<(Entity, &GlobalTransform)>,
    killed: bool,
    show_floating_damage: bool,
    materials: &mut Assets<StandardMaterial>,
    shot_direction: Vec3,
) {
    if damage > 0.0 {
        if show_floating_damage {
            spawn_floating_damage(commands, point, damage);
        }
        spawn_blood_effects(commands, blood_assets, spatial_query, time, point, normal, bone_for_blood, killed, materials, shot_direction);
    } else {
        spawn_spark_effects(commands, spark_assets, time, point, normal);
    }
}

const FLINCH_DURATION: f32 = 0.32;
const FLINCH_MAX_ANGLE_DEG: f32 = 78.0;
const FLINCH_CHAIN_FALLOFF: [f32; 3] = [1.0, 0.68, 0.38];
const FLINCH_HEAD_TO_CHEST: f32 = 0.3;

#[derive(Component)]
pub(crate) struct BoneFlinch {
    world_axis: Vec3,
    angle: f32,
    remaining: f32,
}

fn flinch_chain(bones: &ZombieHitboxBones, hit: Entity) -> Vec<Entity> {
    for limb in [&bones.left_arm, &bones.right_arm, &bones.left_leg, &bones.right_leg] {
        if let Some(index) = limb.iter().position(|bone| *bone == Some(hit)) {
            return limb[..=index].iter().rev().copied().flatten().collect();
        }
    }
    if Some(hit) == bones.head {
        return [Some(hit), bones.chest].into_iter().flatten().collect();
    }
    vec![hit]
}

pub(crate) fn spawn_bone_flinch(
    commands: &mut Commands,
    bones: &ZombieHitboxBones,
    hit_bone: Entity,
    impulse: Vec3,
    damage: f32,
    globals: &Query<&GlobalTransform>,
    child_of: &Query<&ChildOf>,
) {
    let chain = flinch_chain(bones, hit_bone);
    let Some(&first) = chain.first() else { return };

    let bone_direction = child_of
        .get(first)
        .ok()
        .and_then(|parent| globals.get(parent.0).ok().zip(globals.get(first).ok()))
        .map(|(parent, bone)| (bone.translation() - parent.translation()).normalize_or_zero())
        .unwrap_or(Vec3::Y);

    let mut axis = bone_direction.cross(impulse).normalize_or_zero();
    if axis == Vec3::ZERO {
        axis = impulse.cross(Vec3::Y).normalize_or_zero();
    }
    if axis == Vec3::ZERO {
        return;
    }

    let head_hit = Some(hit_bone) == bones.head;
    // a kill skips the flinch entirely, so scale against the heaviest survivable hit, not max health
    let base = FLINCH_MAX_ANGLE_DEG.to_radians() * (damage / shared::CHEST_DAMAGE).clamp(0.4, 1.0);

    for (index, bone) in chain.into_iter().enumerate() {
        let falloff = if head_hit && index > 0 {
            FLINCH_HEAD_TO_CHEST
        } else {
            FLINCH_CHAIN_FALLOFF[index.min(FLINCH_CHAIN_FALLOFF.len() - 1)]
        };
        commands.entity(bone).insert(BoneFlinch {
            world_axis: axis,
            angle: base * falloff,
            remaining: FLINCH_DURATION,
        });
    }
}

pub(crate) fn apply_bone_flinch(
    time: Res<Time>,
    mut commands: Commands,
    child_of: Query<&ChildOf>,
    globals: Query<&GlobalTransform>,
    mut flinches: Query<(Entity, &mut BoneFlinch)>,
    mut transforms: Query<&mut Transform>,
) {
    for (entity, mut flinch) in &mut flinches {
        flinch.remaining -= time.delta_secs();
        if flinch.remaining <= 0.0 {
            commands.entity(entity).remove::<BoneFlinch>();
            continue;
        }
        let decay = (flinch.remaining / FLINCH_DURATION).clamp(0.0, 1.0);
        let angle = flinch.angle * decay * decay;
        let parent_rotation = child_of
            .get(entity)
            .ok()
            .and_then(|parent| globals.get(parent.0).ok())
            .map(|global| global.rotation())
            .unwrap_or(Quat::IDENTITY);
        let world_rotation = Quat::from_axis_angle(flinch.world_axis, angle);
        let local_rotation = parent_rotation.inverse() * world_rotation * parent_rotation;
        let Ok(mut transform) = transforms.get_mut(entity) else { continue };
        transform.rotation = local_rotation * transform.rotation;
    }
}

pub(crate) fn nearest_zombie_bone(bones: &ZombieHitboxBones, point: Vec3, globals: &Query<&GlobalTransform>) -> Option<(Entity, GlobalTransform)> {
    bones
        .head
        .into_iter()
        .chain(bones.belly)
        .chain(bones.chest)
        .chain(bones.left_arm.into_iter().flatten())
        .chain(bones.right_arm.into_iter().flatten())
        .chain(bones.left_leg.into_iter().flatten())
        .chain(bones.right_leg.into_iter().flatten())
        .filter_map(|entity| globals.get(entity).ok().map(|g| (entity, *g)))
        .min_by(|(_, a), (_, b)| a.translation().distance_squared(point).total_cmp(&b.translation().distance_squared(point)))
}

const ZOMBIE_HEAD_YAW_LIMIT_DEG: f32 = 72.0;
const ZOMBIE_HEAD_PITCH_LIMIT_DEG: f32 = 32.0;
const ZOMBIE_HEAD_TURN_RATE: f32 = 9.0;

#[derive(Component, Default)]
pub(crate) struct ZombieHeadLook {
    yaw: f32,
    pitch: f32,
}

pub(crate) fn aim_zombie_heads(
    time: Res<Time>,
    camera: Query<&GlobalTransform, With<FpsCamera>>,
    mut zombies: Query<(&Transform, &ZombieHitboxBones, &mut ZombieHeadLook), With<Zombie>>,
    child_of: Query<&ChildOf>,
    globals: Query<&GlobalTransform>,
    mut bone_transforms: Query<&mut Transform, Without<Zombie>>,
) {
    let Ok(camera_global) = camera.single() else {
        return;
    };
    let target = camera_global.translation();
    let blend = (ZOMBIE_HEAD_TURN_RATE * time.delta_secs()).min(1.0);
    let yaw_limit = ZOMBIE_HEAD_YAW_LIMIT_DEG.to_radians();
    let pitch_limit = ZOMBIE_HEAD_PITCH_LIMIT_DEG.to_radians();

    for (body, bones, mut look) in &mut zombies {
        let Some(head) = bones.head else {
            continue;
        };
        let Ok(head_global) = globals.get(head) else {
            continue;
        };

        let to_target = target - head_global.translation();
        let flat = Vec3::new(to_target.x, 0.0, to_target.z);
        let forward = body.rotation * Vec3::Z;
        let forward_flat = Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero();
        if flat.length_squared() < 1e-6 || forward_flat == Vec3::ZERO {
            continue;
        }
        let target_flat = flat.normalize();

        let yaw = forward_flat
            .cross(target_flat)
            .dot(Vec3::Y)
            .atan2(forward_flat.dot(target_flat))
            .clamp(-yaw_limit, yaw_limit);
        let pitch = to_target.y.atan2(flat.length()).clamp(-pitch_limit, pitch_limit);

        look.yaw += (yaw - look.yaw) * blend;
        look.pitch += (pitch - look.pitch) * blend;

        let yaw_rotation = Quat::from_axis_angle(Vec3::Y, look.yaw);
        let right = (yaw_rotation * forward_flat.cross(Vec3::Y)).normalize_or_zero();
        let world_rotation = Quat::from_axis_angle(right, look.pitch) * yaw_rotation;

        let parent_rotation = child_of
            .get(head)
            .ok()
            .and_then(|parent| globals.get(parent.0).ok())
            .map(|global| global.rotation())
            .unwrap_or(Quat::IDENTITY);
        let local_rotation = parent_rotation.inverse() * world_rotation * parent_rotation;
        let Ok(mut transform) = bone_transforms.get_mut(head) else {
            continue;
        };
        transform.rotation = local_rotation * transform.rotation;
    }
}


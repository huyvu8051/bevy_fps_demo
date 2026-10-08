
use avian3d::prelude::*;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::zombie::{ZombieHitboxBones, ZOMBIE_RADIUS, ZOMBIE_SCALE_FACTOR};

pub const ZOMBIE_MAX_HEALTH: f32 = 8.0;

pub const HEAD_DAMAGE: f32 = ZOMBIE_MAX_HEALTH;
pub const CHEST_DAMAGE: f32 = 1.5;
pub const BELLY_DAMAGE: f32 = 1.0;
pub const LIMB_DAMAGE: f32 = 0.5;

#[derive(Event, Clone, Copy, Serialize, Deserialize)]
pub struct SetDamageMultiplier(pub f32);

pub const HITBOX_HEAD_MIN_Y: f32 = 0.9 * ZOMBIE_SCALE_FACTOR;
pub const HITBOX_CHEST_MIN_Y: f32 = 0.3 * ZOMBIE_SCALE_FACTOR;
pub const HITBOX_BELLY_MIN_Y: f32 = -0.2 * ZOMBIE_SCALE_FACTOR;
pub const HITBOX_LIMB_LATERAL_MIN: f32 = 0.35 * ZOMBIE_SCALE_FACTOR;

pub const HEAD_HITBOX_RADIUS: f32 = 1.0 * ZOMBIE_RADIUS;

pub const TORSO_HITBOX_RADIUS: f32 = 0.85 * ZOMBIE_RADIUS;
pub const LIMB_HITBOX_RADIUS: f32 = 0.4 * ZOMBIE_RADIUS;

#[derive(Default, Clone)]
pub struct BoneWorldPositions {
    pub head: Option<Vec3>,
    pub belly: Option<Vec3>,
    pub chest: Option<Vec3>,
    pub left_arm: [Option<Vec3>; 3],
    pub right_arm: [Option<Vec3>; 3],
    pub left_leg: [Option<Vec3>; 3],
    pub right_leg: [Option<Vec3>; 3],
}

pub fn resolve_bone_world_positions(bones: &ZombieHitboxBones, bone_position: impl Fn(Entity) -> Option<Vec3>) -> BoneWorldPositions {
    let resolve_chain = |chain: &[Option<Entity>; 3]| chain.map(|joint| joint.and_then(&bone_position));
    BoneWorldPositions {
        head: bones.head.and_then(&bone_position),
        belly: bones.belly.and_then(&bone_position),
        chest: bones.chest.and_then(&bone_position),
        left_arm: resolve_chain(&bones.left_arm),
        right_arm: resolve_chain(&bones.right_arm),
        left_leg: resolve_chain(&bones.left_leg),
        right_leg: resolve_chain(&bones.right_leg),
    }
}

fn point_segment_distance(point: Vec3, a: Vec3, b: Vec3) -> f32 {
    let ab = b - a;
    let len_sq = ab.length_squared();
    if len_sq <= f32::EPSILON {
        return point.distance(a);
    }
    let t = ((point - a).dot(ab) / len_sq).clamp(0.0, 1.0);
    point.distance(a + ab * t)
}

fn point_segment_t(point: Vec3, a: Vec3, b: Vec3) -> f32 {
    let ab = b - a;
    let len_sq = ab.length_squared();
    if len_sq <= f32::EPSILON {
        return 0.0;
    }
    ((point - a).dot(ab) / len_sq).clamp(0.0, 1.0)
}

fn capsule_transform(a: Vec3, b: Vec3) -> Option<(Vec3, Quat, f32)> {
    let segment = b - a;
    let direction = Dir3::new(segment).ok()?;
    Some((a.midpoint(b), Quat::from_rotation_arc(Vec3::Y, *direction), segment.length()))
}

pub fn entity_phase(entity: Entity) -> f32 {
    (entity.index().index() as f32 * 0.618_034).fract()
}

pub fn random_aim_point(bones: &BoneWorldPositions, target: Entity) -> Option<(Vec3, &'static str)> {
    let mut joints = Vec::new();
    if let Some(p) = bones.head {
        joints.push((p, "Head"));
    }
    if let Some(p) = bones.belly {
        for _ in 0..35 {
            joints.push((p, "Belly"));
        }
    }
    if let Some(p) = bones.chest {
        for _ in 0..75 {
            joints.push((p, "Chest"));
        }
    }
    for chain in [&bones.left_arm, &bones.right_arm, &bones.left_leg, &bones.right_leg] {
        for joint in chain.iter().flatten() {
            joints.push((*joint, "Limb"));
        }
    }

    if joints.is_empty() {
        return None;
    }

    let mut seed = target.index_u32() as usize;
    seed = seed.wrapping_mul(2654435761);
    let idx = seed % joints.len();
    Some(joints[idx])
}

pub fn zombie_hit_damage_bones(hit_point: Vec3, bones: &BoneWorldPositions, _forward: Vec3) -> f32 {
    if let Some(pos) = bones.head {
        if hit_point.distance(pos) <= HEAD_HITBOX_RADIUS {
            return HEAD_DAMAGE;
        }
    }
    for chain in [&bones.left_arm, &bones.right_arm, &bones.left_leg, &bones.right_leg] {
        for pair in chain.windows(2) {
            if let (Some(a), Some(b)) = (pair[0], pair[1]) {
                if point_segment_distance(hit_point, a, b) <= LIMB_HITBOX_RADIUS {
                    return LIMB_DAMAGE;
                }
            }
        }
    }
    if let (Some(belly), Some(chest)) = (bones.belly, bones.chest) {
        let ab = chest - belly;
        let len_sq = ab.length_squared();
        if len_sq > f32::EPSILON {
            let t = ((hit_point - belly).dot(ab) / len_sq).clamp(0.0, 1.0);
            let closest = belly + ab * t;
            if hit_point.distance(closest) <= TORSO_HITBOX_RADIUS {

                return if t >= 0.5 { CHEST_DAMAGE } else { BELLY_DAMAGE };
            }
        }
    }

    if let Some(pos) = bones.chest {
        if hit_point.distance(pos) <= TORSO_HITBOX_RADIUS {
            return CHEST_DAMAGE;
        }
    }
    if let Some(pos) = bones.belly {
        if hit_point.distance(pos) <= TORSO_HITBOX_RADIUS {
            return BELLY_DAMAGE;
        }
    }
    0.0
}

pub fn zombie_hit_nearest_region(hit_point: Vec3, bones: &BoneWorldPositions, _forward: Vec3) -> Option<(&'static str, f32, f32)> {
    let mut nearest: Option<(&'static str, f32, f32)> = None;
    let mut consider = |name: &'static str, distance: f32, radius: f32| {
        if nearest.is_none_or(|(_, best, _)| distance < best) {
            nearest = Some((name, distance, radius));
        }
    };
    if let Some(pos) = bones.head {
        consider("head", hit_point.distance(pos), HEAD_HITBOX_RADIUS);
    }
    for (name, chain) in [
        ("left_arm", &bones.left_arm),
        ("right_arm", &bones.right_arm),
        ("left_leg", &bones.left_leg),
        ("right_leg", &bones.right_leg),
    ] {
        for pair in chain.windows(2) {
            if let (Some(a), Some(b)) = (pair[0], pair[1]) {
                consider(name, point_segment_distance(hit_point, a, b), LIMB_HITBOX_RADIUS);
            }
        }
    }
    if let (Some(belly), Some(chest)) = (bones.belly, bones.chest) {
        consider("torso", point_segment_distance(hit_point, belly, chest), TORSO_HITBOX_RADIUS);
    }
    nearest
}

pub fn zombie_hit_damage_ray(origin: Vec3, direction: Dir3, max_distance: f32, bones: &BoneWorldPositions, _forward: Vec3) -> Option<(Vec3, Vec3, f32)> {
    let mut best: Option<(f32, Vec3, f32)> = None;
    let mut consider = |hit: Option<(f32, Vec3)>, damage: f32| {
        if let Some((toi, normal)) = hit {
            if best.is_none_or(|(best_toi, ..)| toi < best_toi) {
                best = Some((toi, normal, damage));
            }
        }
    };

    if let Some(pos) = bones.head {
        consider(Collider::sphere(HEAD_HITBOX_RADIUS).cast_ray(pos, Quat::IDENTITY, origin, *direction, max_distance, true), HEAD_DAMAGE);
    }
    for chain in [&bones.left_arm, &bones.right_arm, &bones.left_leg, &bones.right_leg] {
        for pair in chain.windows(2) {
            if let (Some(a), Some(b)) = (pair[0], pair[1]) {
                if let Some((position, rotation, length)) = capsule_transform(a, b) {
                    consider(Collider::capsule(LIMB_HITBOX_RADIUS, length).cast_ray(position, rotation, origin, *direction, max_distance, true), LIMB_DAMAGE);
                }
            }
        }
    }
    if let (Some(belly), Some(chest)) = (bones.belly, bones.chest) {
        if let Some((position, rotation, length)) = capsule_transform(belly, chest) {
            if let Some((toi, normal)) = Collider::capsule(TORSO_HITBOX_RADIUS, length).cast_ray(position, rotation, origin, *direction, max_distance, true) {

                let t = point_segment_t(origin + *direction * toi, belly, chest);
                let damage = if t >= 0.5 { CHEST_DAMAGE } else { BELLY_DAMAGE };
                if best.is_none_or(|(best_toi, ..)| toi < best_toi) {
                    best = Some((toi, normal, damage));
                }
            }
        }
    }

    best.map(|(toi, normal, damage)| (origin + *direction * toi, normal, damage))
}

#[cfg(test)]
mod hitbox_phase4_tests {
    use super::*;

    fn bones_at(head: Vec3) -> (ZombieHitboxBones, std::collections::HashMap<Entity, Vec3>) {
        let head_entity = Entity::from_raw_u32(1).unwrap();
        let chest_entity = Entity::from_raw_u32(2).unwrap();
        let belly_entity = Entity::from_raw_u32(3).unwrap();
        let shoulder_entity = Entity::from_raw_u32(4).unwrap();
        let elbow_entity = Entity::from_raw_u32(5).unwrap();
        let hand_entity = Entity::from_raw_u32(6).unwrap();
        let bones = ZombieHitboxBones {
            head: Some(head_entity),
            belly: Some(belly_entity),
            chest: Some(chest_entity),
            left_arm: [Some(shoulder_entity), Some(elbow_entity), Some(hand_entity)],
            right_arm: [None, None, None],
            left_leg: [None, None, None],
            right_leg: [None, None, None],
        };
        let mut positions = std::collections::HashMap::new();
        positions.insert(head_entity, head);
        positions.insert(chest_entity, head - Vec3::Y * 0.6);
        positions.insert(belly_entity, head - Vec3::Y * 1.1);
        positions.insert(shoulder_entity, head + Vec3::X * 0.6);
        positions.insert(elbow_entity, head + Vec3::X * 1.0);
        positions.insert(hand_entity, head + Vec3::X * 1.4);
        (bones, positions)
    }

    #[test]
    fn hit_exactly_on_head_bone_is_a_headshot() {
        let (bones, positions) = bones_at(Vec3::new(0.0, 1.9, 0.0));
        let resolved = resolve_bone_world_positions(&bones, |e| positions.get(&e).copied());
        let head_pos = resolved.head.unwrap();
        let damage = zombie_hit_damage_bones(head_pos, &resolved, Vec3::NEG_Z);
        assert_eq!(damage, HEAD_DAMAGE);
    }

    #[test]
    fn hit_near_limb_joint_is_a_limb_hit_even_though_closer_to_head_height() {
        let (bones, positions) = bones_at(Vec3::new(0.0, 1.9, 0.0));
        let resolved = resolve_bone_world_positions(&bones, |e| positions.get(&e).copied());
        let shoulder_pos = resolved.left_arm[0].unwrap();

        let damage = zombie_hit_damage_bones(shoulder_pos, &resolved, Vec3::NEG_Z);
        assert_eq!(damage, LIMB_DAMAGE);
    }

    #[test]
    fn hit_midway_between_limb_joints_is_still_a_limb_hit() {

        let (bones, positions) = bones_at(Vec3::new(0.0, 1.9, 0.0));
        let resolved = resolve_bone_world_positions(&bones, |e| positions.get(&e).copied());
        let shoulder = resolved.left_arm[0].unwrap();
        let elbow = resolved.left_arm[1].unwrap();
        let midpoint = shoulder.lerp(elbow, 0.5);
        assert_eq!(zombie_hit_damage_bones(midpoint, &resolved, Vec3::NEG_Z), LIMB_DAMAGE);
    }

    #[test]
    fn hit_near_chest_bone_is_a_chest_hit() {
        let (bones, positions) = bones_at(Vec3::new(0.0, 1.9, 0.0));
        let resolved = resolve_bone_world_positions(&bones, |e| positions.get(&e).copied());
        let chest_pos = resolved.chest.unwrap();
        let damage = zombie_hit_damage_bones(chest_pos, &resolved, Vec3::NEG_Z);
        assert_eq!(damage, CHEST_DAMAGE);
    }

    #[test]
    fn hit_midway_on_torso_scores_belly_or_chest_not_zero() {

        let (bones, positions) = bones_at(Vec3::new(0.0, 1.9, 0.0));
        let resolved = resolve_bone_world_positions(&bones, |e| positions.get(&e).copied());
        let belly = resolved.belly.unwrap();
        let chest = resolved.chest.unwrap();
        let midpoint = belly.lerp(chest, 0.5);
        let damage = zombie_hit_damage_bones(midpoint, &resolved, Vec3::NEG_Z);
        assert!(damage == CHEST_DAMAGE || damage == BELLY_DAMAGE, "expected torso damage, got {damage}");
    }

    #[test]
    fn unresolved_bones_score_zero_damage_not_a_fallback_guess() {
        let empty = BoneWorldPositions::default();
        let hit_point = Vec3::new(0.0, 1.9, 0.0);

        assert_eq!(zombie_hit_damage_bones(hit_point, &empty, Vec3::NEG_Z), 0.0);
    }

    #[test]
    fn far_from_every_bone_scores_zero_damage() {
        let (bones, positions) = bones_at(Vec3::new(0.0, 1.9, 0.0));
        let resolved = resolve_bone_world_positions(&bones, |e| positions.get(&e).copied());

        let hit_point = Vec3::new(5.0, 1.0, 0.0);
        assert_eq!(zombie_hit_damage_bones(hit_point, &resolved, Vec3::NEG_Z), 0.0);
    }
}

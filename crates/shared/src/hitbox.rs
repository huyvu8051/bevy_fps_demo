
use avian3d::prelude::*;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::zombie::{ZombieHitboxBones, ZOMBIE_RADIUS};

pub const ZOMBIE_MAX_HEALTH: f32 = 8.0;

pub const HEAD_DAMAGE: f32 = ZOMBIE_MAX_HEALTH;
pub const CHEST_DAMAGE: f32 = 1.5;
pub const BELLY_DAMAGE: f32 = 1.0;
pub const LIMB_DAMAGE: f32 = 0.5;

#[derive(Event, Clone, Copy, Serialize, Deserialize)]
pub struct SetDamageMultiplier(pub f32);

pub const HITBOX_HEAD_MIN_Y: f32 = 2.25;
pub const HITBOX_CHEST_MIN_Y: f32 = 0.75;
pub const HITBOX_BELLY_MIN_Y: f32 = -0.5;
pub const HITBOX_LIMB_LATERAL_MIN: f32 = 0.875;

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

pub fn zombie_hit_damage_bones(hit_point: Vec3, bones: &BoneWorldPositions) -> f32 {
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

pub fn zombie_hit_nearest_region(hit_point: Vec3, bones: &BoneWorldPositions) -> Option<(&'static str, f32, f32)> {
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

/// Which part a shot landed on, and the joint it should come off at.
///
/// The segment index doubles as the cut joint: a hit on the upper arm, the
/// segment between joints 0 and 1, severs at joint 0.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ZombiePart {
    Head,
    LeftArm,
    RightArm,
    LeftLeg,
    RightLeg,
}

impl ZombiePart {
    pub const ALL: [ZombiePart; 5] = [Self::Head, Self::LeftArm, Self::RightArm, Self::LeftLeg, Self::RightLeg];

    pub fn index(self) -> usize {
        match self {
            Self::Head => 0,
            Self::LeftArm => 1,
            Self::RightArm => 2,
            Self::LeftLeg => 3,
            Self::RightLeg => 4,
        }
    }
}

pub struct BoneHit {
    pub point: Vec3,
    pub normal: Vec3,
    pub damage: f32,
    pub limb: Option<(ZombiePart, usize)>,
}

pub fn zombie_hit_damage_ray(origin: Vec3, direction: Dir3, max_distance: f32, bones: &BoneWorldPositions) -> Option<BoneHit> {
    let mut best: Option<(f32, Vec3, f32, Option<(ZombiePart, usize)>)> = None;
    let mut consider = |hit: Option<(f32, Vec3)>, damage: f32, limb: Option<(ZombiePart, usize)>| {
        if let Some((toi, normal)) = hit {
            if best.is_none_or(|(best_toi, ..)| toi < best_toi) {
                best = Some((toi, normal, damage, limb));
            }
        }
    };

    if let Some(pos) = bones.head {
        consider(Collider::sphere(HEAD_HITBOX_RADIUS).cast_ray(pos, Quat::IDENTITY, origin, *direction, max_distance, true), HEAD_DAMAGE, Some((ZombiePart::Head, 0)));
    }
    // Paired in one place rather than zipping two lists. ALL carries the head
    // as well, and zipping it against the four limb chains silently shifted
    // every part by one, so a hit on one arm was tallied against the other.
    for (part, chain) in [
        (ZombiePart::LeftArm, &bones.left_arm),
        (ZombiePart::RightArm, &bones.right_arm),
        (ZombiePart::LeftLeg, &bones.left_leg),
        (ZombiePart::RightLeg, &bones.right_leg),
    ] {
        for (segment, pair) in chain.windows(2).enumerate() {
            if let (Some(a), Some(b)) = (pair[0], pair[1]) {
                if let Some((position, rotation, length)) = capsule_transform(a, b) {
                    consider(
                        Collider::capsule(LIMB_HITBOX_RADIUS, length).cast_ray(position, rotation, origin, *direction, max_distance, true),
                        LIMB_DAMAGE,
                        Some((part, segment)),
                    );
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
                    best = Some((toi, normal, damage, None));
                }
            }
        }
    }

    best.map(|(toi, normal, damage, limb)| BoneHit { point: origin + *direction * toi, normal, damage, limb })
}

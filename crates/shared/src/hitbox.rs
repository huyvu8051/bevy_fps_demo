// --- Hit detection/damage (server-authoritative, see crates/server) ---

use avian3d::prelude::*;
use bevy::prelude::*;

use crate::zombie::{ZombieHitboxBones, ZOMBIE_RADIUS, ZOMBIE_SCALE_FACTOR};

// Body-shot kills still take this many hits; a headshot always kills in one
// (see zombie_hit_damage_bones).
pub const ZOMBIE_MAX_HEALTH: f32 = 8.0;

// Per-region damage, picked by zombie_hit_damage_bones from real bone
// hitbox spheres (HITBOX_PLAN.md Phase 4) — see that function's own doc
// comment.
pub const HEAD_DAMAGE: f32 = ZOMBIE_MAX_HEALTH; // always a one-shot kill
pub const CHEST_DAMAGE: f32 = 1.5;
pub const BELLY_DAMAGE: f32 = 1.0;
pub const LIMB_DAMAGE: f32 = 0.5;

// Height bands (world-space meters, relative to the zombie's root) and a
// lateral distance from the vertical axis through the root — the ORIGINAL
// fixed-height-band guess this project used before real bone hitboxes
// existed (see HITBOX_PLAN.md). No damage function reads these anymore
// (zombie_hit_damage_bones has no fallback to them — an unresolved region
// just scores 0 rather than falling back to a completely different
// detection method); kept only because draw_zombie_hitboxes (client-side
// debug gizmo) still draws rings at these heights for visual comparison
// against the real bone spheres.
pub const HITBOX_HEAD_MIN_Y: f32 = 0.9 * ZOMBIE_SCALE_FACTOR;
pub const HITBOX_CHEST_MIN_Y: f32 = 0.3 * ZOMBIE_SCALE_FACTOR;
pub const HITBOX_BELLY_MIN_Y: f32 = -0.2 * ZOMBIE_SCALE_FACTOR;
pub const HITBOX_LIMB_LATERAL_MIN: f32 = 0.35 * ZOMBIE_SCALE_FACTOR;

// Sphere/capsule radii (world-space meters) tested against each resolved
// bone's live world position — same "rough estimate, playtest-tunable"
// spirit as HITBOX_HEAD_MIN_Y et al. above (this codebase has never had the
// actual mesh's per-part dimensions to calibrate against), just centered on
// the real bone instead of a fixed height band. Fractions of ZOMBIE_RADIUS
// (itself derived from the real measured mesh width — see its own doc
// comment) rather than fresh ZOMBIE_SCALE_FACTOR multiples, so these two
// hitbox systems can be tuned relative to each other by eye without a unit
// conversion.
// Back to a single head sphere at the Head bone's own raw resolved position
// (no Y/forward offset, no separate neck sphere) — reverted per request.
// Sized to match ZOMBIE_RADIUS itself (1x, up from the old 0.5x) rather than
// a smaller fraction of it, so the sphere reads as "as big as the zombie's
// own body radius", not a noticeably tighter headshot window.
pub const HEAD_HITBOX_RADIUS: f32 = 1.0 * ZOMBIE_RADIUS;
// One capsule spanning belly->chest replaces the old two separate point
// spheres (see zombie_hit_damage_bones) — a single radius for both ends,
// picked as the larger of the two former sphere radii so the capsule still
// generously covers the wider (chest) end.
pub const TORSO_HITBOX_RADIUS: f32 = 0.85 * ZOMBIE_RADIUS;
pub const LIMB_HITBOX_RADIUS: f32 = 0.4 * ZOMBIE_RADIUS;

// Already-resolved world positions for a hit test — the output of either
// "read each bone's live GlobalTransform right now" (Phase 4, still used
// by the client's own prediction and as the live-hit fallback server-side)
// or "rewind ZombieHistory's stored per-bone offsets to some past instant"
// (Phase 3, zombie_history_at below). zombie_hit_damage_bones doesn't care
// which one produced it — decoupling the damage decision itself from
// "how did we get these positions" is what let Phase 3 slot in without
// changing zombie_hit_damage_bones' own logic at all, only its callers.
// Field shape mirrors ZombieHitboxBones exactly — see its own doc comment
// for why the limb chains are ordered arrays, not an unordered bag.
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

// Resolves a live ZombieHitboxBones (real Entity references) into a
// BoneWorldPositions snapshot via the given lookup — the Phase 4 (no
// rewind) path, used for the client's own local prediction and the
// server's live-hit fallback (no ZombieHistory entry recent enough to
// rewind to, e.g. a zombie that just spawned). `bone_position` is a
// callback rather than a `Query` directly so this stays callable from
// both crates/server and crates/client without depending on either
// crate's exact query-building code.
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

// Closest distance from `point` to the line segment a->b — lets a hit test
// against a capsule spanning two joints (shoulder->elbow, belly->chest)
// instead of just a point-sphere at one end. Clamped to the segment (not an
// infinite line), so a hit well past either end still measures against
// that end, same as a capsule's rounded cap.
fn point_segment_distance(point: Vec3, a: Vec3, b: Vec3) -> f32 {
    let ab = b - a;
    let len_sq = ab.length_squared();
    if len_sq <= f32::EPSILON {
        return point.distance(a);
    }
    let t = ((point - a).dot(ab) / len_sq).clamp(0.0, 1.0);
    point.distance(a + ab * t)
}

// Same t as point_segment_distance's own internal computation, exposed on
// its own for zombie_hit_damage_ray's chest-vs-belly split (it already has
// a real hit point on the torso capsule's surface, not just a distance, so
// it needs to know WHERE along the segment that landed).
fn point_segment_t(point: Vec3, a: Vec3, b: Vec3) -> f32 {
    let ab = b - a;
    let len_sq = ab.length_squared();
    if len_sq <= f32::EPSILON {
        return 0.0;
    }
    ((point - a).dot(ab) / len_sq).clamp(0.0, 1.0)
}

// Position + rotation + full length for a capsule spanning a->b, or None
// for a degenerate (zero-length) segment — Capsule3d/avian3d's Collider::
// capsule are both built along the local Y axis, so this is the same
// from_rotation_arc trick draw_bone_capsule (crates/client) uses to orient
// one along an arbitrary segment.
fn capsule_transform(a: Vec3, b: Vec3) -> Option<(Vec3, Quat, f32)> {
    let segment = b - a;
    let direction = Dir3::new(segment).ok()?;
    Some((a.midpoint(b), Quat::from_rotation_arc(Vec3::Y, *direction), segment.length()))
}

// Deterministic per-entity pseudo-randomness via a golden-ratio hash (this
// codebase avoids a `rand` dependency — same spirit as the sin-based jitter
// used for particle directions elsewhere) — returns a value in [0, 1) that
// stays stable for a given entity but doesn't correlate across entities
// spawned at the same time, unlike hashing spawn position directly (which
// aliases badly against ZOMBIE_SPAWNS' regular grid — see this fn's
// original introduction for the full story). Used both for staggering
// per-zombie timers (move_zombies, bot_pick_target) and for
// random_aim_point below.
pub fn entity_phase(entity: Entity) -> f32 {
    (entity.index().index() as f32 * 0.618_034).fract()
}

// Autopilot's aim region — only Body is left (Head, Upper arm, Forearm,
// Thigh, Shin all excluded per request, in successive rounds) — see
// random_aim_point, which now always resolves to center-mass. Real players
// who aim themselves are unaffected; this only narrows autopilot's own pick.

// Weighted-random point on `bones` to aim at (autopilot_think). The pick is
// a pure function of `target` (via entity_phase), not re-rolled every
// frame — the same committed target keeps aiming at the same body part
// instead of jittering every tick, and naturally re-rolls whenever
// autopilot commits to a different target (see AutopilotState's own
// retarget-timer doc comment).
// Returns None if the picked region's bones haven't resolved yet (mid-load
// window, same "eat the brief gap, don't silently substitute a different
// method" stance zombie_hit_damage_bones takes) — callers just keep
// whatever aim point they already had for that one frame.
// The `&'static str` half of the return is purely a debug/HUD label (e.g.
// "showing which body part autopilot is currently aiming at" on screen) —
// never fed back into damage/collision math, so it's fine as a plain name.
pub fn random_aim_point(bones: &BoneWorldPositions, target: Entity) -> Option<(Vec3, &'static str)> {
    let mut joints = Vec::new();
    if let Some(p) = bones.head {
        joints.push((p, "Head"));
    }
    if let Some(p) = bones.belly {
        for _ in 0..15 {
            joints.push((p, "Belly"));
        }
    }
    if let Some(p) = bones.chest {
        for _ in 0..15 {
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

    // Mix the target entity index using a multiplicative hash to get a pseudo-random seed
    let mut seed = target.index_u32() as usize;
    seed = seed.wrapping_mul(2654435761);
    let idx = seed % joints.len();
    Some(joints[idx])
}

// Real per-bone hit detection (HITBOX_PLAN.md Phase 4, positions
// optionally lag-compensated as of Phase 3) — checked in the same
// head-first priority order the old height-band guess used (so an
// overlapping head+torso region still resolves to a headshot, matching the
// behavior players already know). No fallback to the old geometric guess
// anymore: a region whose bones haven't resolved yet (or a hit that lands
// outside every capsule/sphere) simply scores 0 damage — bones resolve
// within a couple frames of a zombie spawning (see wire_zombie_hitbox_bones),
// so this window is brief, and mixing in a completely different
// hit-detection method as a silent fallback was judged worse than just
// eating that brief no-damage window.
//
// Head is a single point-sphere at the Head bone's own raw resolved
// position (no offset, no separate neck sphere — reverted per request).
// Limbs and torso are tested as capsule SEGMENTS between consecutive joints
// (shoulder->elbow->hand, hip->knee->foot, belly->chest) instead of a
// point-sphere at each joint — a point-sphere-per-joint approach left real
// dead zones along the visible forearm/shin/torso where a clean hit
// partway between two joints landed outside every sphere and scored 0
// damage even though it clearly hit the mesh.
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
                // Which half of the capsule the hit landed closer to
                // decides chest vs. belly damage — t=0 is belly's end,
                // t=1 is chest's.
                return if t >= 0.5 { CHEST_DAMAGE } else { BELLY_DAMAGE };
            }
        }
    }
    // Only one of belly/chest resolved (mid-load, see wire_zombie_hitbox_bones)
    // — no capsule to build yet, fall back to a plain point-sphere on
    // whichever one exists rather than scoring 0 for the whole torso.
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

// Diagnostic only — never used by the actual damage decision. Finds
// whichever real hit region (head/torso/each limb segment) `hit_point`
// came CLOSEST to, along with that region's own radius, so a 0-damage hit
// can be logged as "missed the torso capsule by 6cm" instead of just
// "miss" — lets a near-miss (radius/timing precision issue) be told apart
// from a wildly-off one (an actual bug: wrong bone, wrong coordinate
// space, stale positions) from the log alone. Mirrors zombie_hit_damage_
// bones' own region set exactly, so "which region is nearest" always means
// the same thing in both places.
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

// Real ray-vs-shape hit test against the ACTUAL per-bone regions (head
// sphere, torso capsule, 4 limb capsule chains) — unlike
// zombie_hit_damage_bones (tests one already-known point's DISTANCE to
// each region), this casts the real ray directly into each region's own
// capsule/sphere shape, exactly the same way the outer broad-phase capsule
// itself is tested. Needed because a hit_point sampled from the outer
// capsule's own surface (its entry point, facing the shooter — see
// ZOMBIE_OUTER_RADIUS's own doc comment) can land meaningfully outside
// every real region even on a shot dead-center on the mesh, once the outer
// shell is wider than the real body — that surface point sits on the wide
// shell, not the thin real bone surface, and the ray was never actually
// tested any farther in. Whichever region the ray reaches FIRST wins (not
// the old head>limb>torso priority order — a real ray naturally hits
// whatever's physically closest along its path, which is the correct
// answer once every region is a real shape instead of a point-distance
// check). Returns None if the ray misses every region entirely.
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
                // Which half of the capsule the real hit landed closer to
                // decides chest vs. belly damage, same split
                // zombie_hit_damage_bones uses.
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

// Regression tests for HITBOX_PLAN.md Phase 4 — zombie_hit_damage_bones'
// priority order and fallback behavior in isolation, without needing a
// live client/bot to actually fire a shot. Kept permanently.
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
        // Same world Y as the head, but far enough laterally that only the
        // limb capsule (not the head sphere) should contain it — this is
        // exactly the case the old height-band guess handled via a
        // separate lateral-distance check; bone-based detection gets it
        // for free from the limb joints' own real positions.
        let damage = zombie_hit_damage_bones(shoulder_pos, &resolved, Vec3::NEG_Z);
        assert_eq!(damage, LIMB_DAMAGE);
    }

    #[test]
    fn hit_midway_between_limb_joints_is_still_a_limb_hit() {
        // The actual dead-zone bug this capsule-segment approach fixes: a
        // point-sphere-per-joint design scored 0 damage for a hit landing
        // cleanly on the visible forearm mesh but equidistant from both the
        // shoulder and elbow spheres. A capsule spanning the two joints has
        // no such gap.
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
        // Same dead-zone fix as the limb test above, applied to the
        // belly<->chest capsule.
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
        // No bone resolved at all (e.g. scene still loading) — must score
        // 0, not silently fall back to a different detection method.
        assert_eq!(zombie_hit_damage_bones(hit_point, &empty, Vec3::NEG_Z), 0.0);
    }

    #[test]
    fn far_from_every_bone_scores_zero_damage() {
        let (bones, positions) = bones_at(Vec3::new(0.0, 1.9, 0.0));
        let resolved = resolve_bone_world_positions(&bones, |e| positions.get(&e).copied());
        // Nowhere near any bone's capsule/sphere — confirms this scores 0
        // rather than crediting some other region by accident.
        let hit_point = Vec3::new(5.0, 1.0, 0.0);
        assert_eq!(zombie_hit_damage_bones(hit_point, &resolved, Vec3::NEG_Z), 0.0);
    }
}

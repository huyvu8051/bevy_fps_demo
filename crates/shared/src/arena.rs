// --- Arena geometry ---
// Both sides need to agree on arena geometry so the server's authoritative
// physics and the client's rendering/navmesh line up.

use bevy::prelude::{Quat, Vec2, Vec3};

pub const ARENA_HALF_SIZE: f32 = 36.0;
pub const ARENA_WALL_HEIGHT: f32 = 5.0;
pub const ARENA_WALL_THICKNESS: f32 = 1.0;

// Single test ramp in the arena centre — for checking whether chase-AI
// zombies climb a slope (MoveAndSlide, no navmesh ramp). Tune RISE/RUN to
// change the angle; the player's own walkable limit is
// acos(GROUND_NORMAL_MIN_Y) ~= 53deg (see crate::player).
pub const ARENA_RAMP_RUN: f32 = 6.0; // horizontal length, along +Z
pub const ARENA_RAMP_RISE: f32 = 3.0; // height of the high end
pub const ARENA_RAMP_WIDTH: f32 = 4.0; // along X
pub const ARENA_RAMP_NEAR_Z: f32 = 2.0; // Z of the low (floor-level) end

/// `(center, rotation, full-size)` for the arena centre test ramp: a thin
/// tilted slab, low end at `(0, 0, ARENA_RAMP_NEAR_Z)`, high end
/// `ARENA_RAMP_RUN` further along +Z and `ARENA_RAMP_RISE` up. Same shape
/// both `spawn_flat_arena_colliders` (server) and `spawn_test_arena`
/// (client) spawn, so their physics can't drift. Mirrors map1's
/// `ramp_between` (thickness-along-normal centre nudge and all).
pub fn arena_ramp() -> (Vec3, Quat, Vec3) {
    let slope = ARENA_RAMP_RISE.atan2(ARENA_RAMP_RUN);
    let surface_len = (ARENA_RAMP_RUN * ARENA_RAMP_RUN + ARENA_RAMP_RISE * ARENA_RAMP_RISE).sqrt();
    let rotation = Quat::from_rotation_x(-slope);
    let centerline = Vec3::new(0.0, ARENA_RAMP_RISE / 2.0, ARENA_RAMP_NEAR_Z + ARENA_RAMP_RUN / 2.0);
    let center = centerline - rotation * Vec3::new(0.0, ARENA_WALL_THICKNESS / 2.0, 0.0);
    let size = Vec3::new(ARENA_RAMP_WIDTH, ARENA_WALL_THICKNESS, surface_len);
    (center, rotation, size)
}

pub const NAVMESH_HEIGHT: f32 = 1.0;
pub const NAVMESH_BOUNDS: [Vec2; 4] = [
    Vec2::new(-(ARENA_HALF_SIZE - 1.0), -(ARENA_HALF_SIZE - 1.0)),
    Vec2::new(ARENA_HALF_SIZE - 1.0, -(ARENA_HALF_SIZE - 1.0)),
    Vec2::new(ARENA_HALF_SIZE - 1.0, ARENA_HALF_SIZE - 1.0),
    Vec2::new(-(ARENA_HALF_SIZE - 1.0), ARENA_HALF_SIZE - 1.0),
];
// Radius of each USE_TEST_ARENA pillar (both the physical collider and the
// navmesh cutout column_footprint carves out of NAVMESH_BOUNDS below).
pub const PILLAR_RADIUS: f32 = 0.6;
// Scattered (x, z) positions for a handful of free-standing pillars inside
// the test arena — added as cover/obstacles to test navigation and
// line-of-sight around (autopilot targeting, zombie pathfinding) rather
// than the previously wide-open flat room. Kept clear of PLAYER_SPAWN
// (0, ARENA_HALF_SIZE - 4 = 14) and the arena center. Both crates/client's
// spawn_test_arena (visual mesh + collider) and crates/server's
// spawn_arena_colliders (collider only) iterate this same array, so the
// two processes' physical worlds can't drift out of sync with each other.
pub const NAVMESH_COLUMN_OBSTACLES: [(f32, f32); 8] = [
    (-8.0, -6.0),
    (8.0, -6.0),
    (-8.0, 6.0),
    (8.0, 6.0),
    (-4.0, 0.0),
    (4.0, 0.0),
    (0.0, -10.0),
    (0.0, 10.0),
];

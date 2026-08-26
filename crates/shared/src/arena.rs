// --- Arena geometry ---
// Both sides need to agree on arena geometry so the server's authoritative
// physics and the client's rendering/navmesh line up.

use bevy::prelude::Vec2;

pub const ARENA_HALF_SIZE: f32 = 36.0;
pub const ARENA_WALL_HEIGHT: f32 = 5.0;
pub const ARENA_WALL_THICKNESS: f32 = 1.0;

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

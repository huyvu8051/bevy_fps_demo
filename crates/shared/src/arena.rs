// --- Arena geometry ---
// Both sides need to agree on arena geometry so the server's authoritative
// physics and the client's rendering/navmesh line up.

use bevy::prelude::{EulerRot, Quat, Vec2, Vec3};

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

// --- Arena geometry, data-driven (per map id) ---------------------------
//
// The floor + 4 perimeter walls + free-standing pillars + centre ramp were
// hardcoded from the consts above (ARENA_HALF_SIZE / ARENA_WALL_* /
// NAVMESH_COLUMN_OBSTACLES / arena_ramp()) directly in both
// spawn_flat_arena_colliders (server) and spawn_test_arena (client). They
// now come from crates/client/assets/data/arena_layout_map<ID>.ron instead
// — one file per MapKind::Arena map id (2 = StressTest, 4 = LegacyArena) —
// so a wall / the ramp / a pillar can be dragged in the dev editor (F2) and
// F9-saved (crates/client/src/arena_layout.rs) without a rebuild, exactly
// like item_gallery_car_physics.ron does for the Map 5 cars.
//
// Every row is ONE static cuboid (pillars included — they're square columns
// here now, not cylinders; the navmesh cutout footprint was already square,
// see crates/server's column_footprint). Row 0 is the floor by convention
// (the client paints it with the floor material); the rest use the wall
// material. NAVMESH_COLUMN_OBSTACLES below is still the navmesh's own
// obstacle list and must be kept in step with the pillar rows by hand — the
// navmesh is built once at startup and is deliberately left out of this
// data file for now.

/// One static box in the arena: full size + translation + rotation. Both
/// the server (collider only) and the client (mesh + collider) build from
/// the same `arena_layout` slice so their physics can't drift.
#[derive(Clone, Copy, Debug)]
pub struct ArenaBox {
    pub size: Vec3,
    pub translation: Vec3,
    pub rotation: Quat,
}

/// Row shape in `arena_layout_map<ID>.ron`:
/// `(size_x, size_y, size_z, x, y, z, rot_x_deg, rot_y_deg, rot_z_deg)` —
/// full box size + translation in meters, rotation as X/Y/Z euler degrees
/// applied `EulerRot::YXZ` (same convention as item_gallery_car_physics.ron,
/// so the F9 save round-trips).
type ArenaLayoutRow = (f32, f32, f32, f32, f32, f32, f32, f32, f32);

fn load_arena_layout(map_id: u8) -> Vec<ArenaBox> {
    // CARGO_MANIFEST_DIR-relative (baked in at shared's compile time) — see
    // item_gallery_car_physics_raw's doc comment for why this path form
    // works unchanged from both the client and the server binary.
    let path = format!(
        "{}/../client/assets/data/arena_layout_map{map_id}.ron",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read arena layout data at {path}: {e}"));
    let rows: Vec<ArenaLayoutRow> = ron::de::from_str(&text)
        .unwrap_or_else(|e| panic!("failed to parse arena layout data at {path}: {e}"));
    rows.into_iter()
        .map(|(sx, sy, sz, x, y, z, rx, ry, rz)| ArenaBox {
            size: Vec3::new(sx, sy, sz),
            translation: Vec3::new(x, y, z),
            rotation: Quat::from_euler(EulerRot::YXZ, ry.to_radians(), rx.to_radians(), rz.to_radians()),
        })
        .collect()
}

/// Arena geometry for `map_id`, read from its RON file once and cached
/// (keyed by id — the client's map-select menu can switch maps without a
/// process restart, so a plain process-wide `OnceLock` isn't enough). The
/// leaked slice is tiny and only ever grows by one entry per map id opened.
pub fn arena_layout(map_id: u8) -> &'static [ArenaBox] {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    static CACHE: OnceLock<Mutex<HashMap<u8, &'static [ArenaBox]>>> = OnceLock::new();
    let mut cache = CACHE.get_or_init(|| Mutex::new(HashMap::new())).lock().expect("arena layout cache poisoned");
    if let Some(&slice) = cache.get(&map_id) {
        return slice;
    }
    let slice: &'static [ArenaBox] = Box::leak(load_arena_layout(map_id).into_boxed_slice());
    cache.insert(map_id, slice);
    slice
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

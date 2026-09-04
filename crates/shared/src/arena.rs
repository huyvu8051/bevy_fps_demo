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
// — one file per MapKind::Arena map id (only 2 = StressTest today) — so a
// wall / the ramp / a pillar can be dragged in the dev editor (F2) and
// F9-saved (crates/client/src/arena_layout.rs) without a rebuild, exactly
// like item_gallery_car_physics.ron does for the Map 5 cars.
//
// Every row is ONE static piece — a `shape` tag picks cuboid vs. cylinder
// (pillars are cylinders again; a cylinder ignores size_z, using size_x as
// its diameter and size_y as its height). Row 0 is the floor by convention
// (the client paints it with the floor material); the rest use the wall
// material.
//
// The zombie navmesh for Arena maps is now derived from this data too: the
// server tags every Cylinder row's collider as an obstacle and
// NavmeshUpdaterPlugin triangulates the walkable area automatically
// (arena_navmesh_outer_edges gives the boundary). NAVMESH_COLUMN_OBSTACLES
// below is only the fallback for the non-Arena (Building / ItemGallery)
// navmesh now — moving a pillar here no longer needs a matching hand-edit.

/// Collider/mesh shape of an `ArenaBox` row — the RON's first tuple field,
/// a bare enum variant (`Cuboid` / `Cylinder`). A `Cylinder` uses
/// `size.x` as its full diameter and `size.y` as its height (`size.z`
/// unused); a `Cuboid` uses all three as its full extents.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
pub enum ArenaShape {
    Cuboid,
    Cylinder,
}

impl ArenaShape {
    /// RON token this serializes back to (the F9 saver writes rows by hand).
    pub fn as_ron(self) -> &'static str {
        match self {
            ArenaShape::Cuboid => "Cuboid",
            ArenaShape::Cylinder => "Cylinder",
        }
    }
}

/// One static piece of the arena: shape + full size + translation +
/// rotation. Both the server (collider only) and the client (mesh +
/// collider) build from the same `arena_layout` slice so their physics
/// can't drift.
#[derive(Clone, Copy, Debug)]
pub struct ArenaBox {
    pub shape: ArenaShape,
    pub size: Vec3,
    pub translation: Vec3,
    pub rotation: Quat,
}

/// Row shape in `arena_layout_map<ID>.ron`:
/// `(shape, size_x, size_y, size_z, x, y, z, rot_x_deg, rot_y_deg, rot_z_deg)`
/// — `shape` is a bare `Cuboid` / `Cylinder`; full size + translation in
/// meters; rotation as X/Y/Z euler degrees applied `EulerRot::YXZ` (same
/// convention as item_gallery_car_physics.ron, so the F9 save round-trips).
type ArenaLayoutRow = (ArenaShape, f32, f32, f32, f32, f32, f32, f32, f32, f32);

fn load_arena_layout(map_id: u8) -> Vec<ArenaBox> {
    // CARGO_MANIFEST_DIR-relative (baked in at shared's compile time) — see
    // item_gallery_car_physics_raw's doc comment for why this path form
    // works unchanged from both the client and the server binary.
    //
    // Unlike the other RON data files, this one's path depends on a
    // runtime value (map_id) — read_ron_data_file! needs a compile-time
    // string literal (its wasm arm expands to include_str!, which can't
    // take a computed path), so wasm32 gets its own match arm per known
    // map id instead of routing through that macro. Adding a new arena
    // map's RON file means adding a match arm here too — the native arm
    // has no such restriction and stays fully dynamic.
    #[cfg(target_arch = "wasm32")]
    let text = match map_id {
        2 => include_str!("../../client/assets/data/arena_layout_map2.ron"),
        other => panic!("no wasm-embedded arena layout RON for map_id {other} — add a match arm in load_arena_layout"),
    }
    .to_string();
    // Real iOS device needs NSBundle's own resource path here, not a
    // CARGO_MANIFEST_DIR-relative one — see read_ron_data_file!'s own doc
    // comment (crate root) for the device crash this exact pattern caused
    // elsewhere before this map_id-dynamic loader (which can't route
    // through that macro — see this fn's own comment above) got the same
    // fix applied by hand.
    #[cfg(target_os = "ios")]
    let text = {
        let resource_path = crate::ios_bundle_resource_path()
            .unwrap_or_else(|| panic!("failed to read arena layout data for map {map_id}: NSBundle.main.resourcePath was nil"));
        let path = std::path::Path::new(&resource_path).join(format!("assets/data/arena_layout_map{map_id}.ron"));
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read arena layout data at {path:?}: {e}"))
    };
    #[cfg(not(any(target_arch = "wasm32", target_os = "ios")))]
    let text = {
        let path = format!("{}/../client/assets/data/arena_layout_map{map_id}.ron", env!("CARGO_MANIFEST_DIR"));
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read arena layout data at {path}: {e}"))
    };
    let rows: Vec<ArenaLayoutRow> =
        ron::de::from_str(&text).unwrap_or_else(|e| panic!("failed to parse arena layout data for map {map_id}: {e}"));
    rows.into_iter()
        .map(|(shape, sx, sy, sz, x, y, z, rx, ry, rz)| ArenaBox {
            shape,
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

// Y of the navmesh sampling plane. Obstacle colliders are carved where
// they cross this height, so it sits low enough to catch a short parked
// car (some are only ~1.1 m tall) while still clearing the floor slab
// (top at y = 0).
pub const NAVMESH_HEIGHT: f32 = 0.3;
pub const NAVMESH_BOUNDS: [Vec2; 4] = [
    Vec2::new(-(ARENA_HALF_SIZE - 1.0), -(ARENA_HALF_SIZE - 1.0)),
    Vec2::new(ARENA_HALF_SIZE - 1.0, -(ARENA_HALF_SIZE - 1.0)),
    Vec2::new(ARENA_HALF_SIZE - 1.0, ARENA_HALF_SIZE - 1.0),
    Vec2::new(-(ARENA_HALF_SIZE - 1.0), ARENA_HALF_SIZE - 1.0),
];

/// How far the zombie navmesh's outer edge is pulled in from the arena
/// floor's own extent — enough that a path never hugs the perimeter walls.
pub const NAVMESH_EDGE_INSET: f32 = 1.0;

// `NavMeshSettings` values the server's real Arena navmesh
// (spawn_zombie_navmesh) and the client's nav_debug reconstruction
// (sync_client_navmesh) both feed in — one copy here so the two can't
// drift. `AGENT_RADIUS` inflates every obstacle by this much when
// triangulating, so a gap narrower than ~2x it is closed in the navmesh
// and the pathfinder routes around it instead of sending a zombie into a
// slot it can't fit. Roughly the zombie's own ZOMBIE_MOVE_RADIUS (0.3), a
// touch under so borderline gaps aren't over-closed.
pub const ARENA_NAVMESH_AGENT_RADIUS: f32 = 0.2;
pub const ARENA_NAVMESH_SIMPLIFY: f32 = 0.005;
pub const ARENA_NAVMESH_BUILD_TIMEOUT_SECS: f32 = 1.0;

/// Outer boundary polygon (CCW, on the XZ plane) for the auto-generated
/// Arena navmesh — the floor row's footprint inset by `NAVMESH_EDGE_INSET`.
/// The server hands this to `NavMeshSettings.fixed`; obstacle cutouts are
/// then derived automatically from the pillar colliders (see
/// `NavmeshUpdaterPlugin` in crates/server), so there's no hand-kept
/// obstacle list for Arena maps anymore.
pub fn arena_navmesh_outer_edges(map_id: u8) -> Vec<Vec2> {
    let floor = arena_layout(map_id)[0]; // row 0 = floor, by convention
    let hx = floor.size.x * 0.5 - NAVMESH_EDGE_INSET;
    let hz = floor.size.z * 0.5 - NAVMESH_EDGE_INSET;
    let (cx, cz) = (floor.translation.x, floor.translation.z);
    vec![
        Vec2::new(cx - hx, cz - hz),
        Vec2::new(cx + hx, cz - hz),
        Vec2::new(cx + hx, cz + hz),
        Vec2::new(cx - hx, cz + hz),
    ]
}
// Pillar radius for the NON-Arena navmesh fallback's square cutouts
// (column_footprint). Arena pillar colliders get their real size from the
// RON's Cylinder rows instead.
pub const PILLAR_RADIUS: f32 = 0.6;
// Pillar (x, z) positions for the NON-Arena navmesh fallback only
// (Building / ItemGallery — see crates/server's spawn_zombie_navmesh).
// Arena maps derive their navmesh obstacles automatically from the pillar
// colliders spawned out of arena_layout_map<ID>.ron, so this array no
// longer has to mirror those rows. Kept clear of PLAYER_SPAWN
// (0, ARENA_HALF_SIZE - 4 = 14) and the arena center.
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

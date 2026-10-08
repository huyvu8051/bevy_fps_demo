
use bevy::prelude::{EulerRot, Quat, Vec2, Vec3};

pub const ARENA_HALF_SIZE: f32 = 36.0;
pub const ARENA_WALL_HEIGHT: f32 = 5.0;
pub const ARENA_WALL_THICKNESS: f32 = 1.0;

pub const ARENA_RAMP_RUN: f32 = 6.0;
pub const ARENA_RAMP_RISE: f32 = 3.0;
pub const ARENA_RAMP_WIDTH: f32 = 4.0;
pub const ARENA_RAMP_NEAR_Z: f32 = 2.0;

pub fn arena_ramp() -> (Vec3, Quat, Vec3) {
    let slope = ARENA_RAMP_RISE.atan2(ARENA_RAMP_RUN);
    let surface_len = (ARENA_RAMP_RUN * ARENA_RAMP_RUN + ARENA_RAMP_RISE * ARENA_RAMP_RISE).sqrt();
    let rotation = Quat::from_rotation_x(-slope);
    let centerline = Vec3::new(0.0, ARENA_RAMP_RISE / 2.0, ARENA_RAMP_NEAR_Z + ARENA_RAMP_RUN / 2.0);
    let center = centerline - rotation * Vec3::new(0.0, ARENA_WALL_THICKNESS / 2.0, 0.0);
    let size = Vec3::new(ARENA_RAMP_WIDTH, ARENA_WALL_THICKNESS, surface_len);
    (center, rotation, size)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
pub enum ArenaShape {
    Cuboid,
    Cylinder,
}

impl ArenaShape {

    pub fn as_ron(self) -> &'static str {
        match self {
            ArenaShape::Cuboid => "Cuboid",
            ArenaShape::Cylinder => "Cylinder",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ArenaBox {
    pub shape: ArenaShape,
    pub size: Vec3,
    pub translation: Vec3,
    pub rotation: Quat,
}

type ArenaLayoutRow = (ArenaShape, f32, f32, f32, f32, f32, f32, f32, f32, f32);

fn load_arena_layout(map_id: u8) -> Vec<ArenaBox> {

    #[cfg(target_os = "ios")]
    let text = {
        let resource_path = crate::ios_bundle_resource_path()
            .unwrap_or_else(|| panic!("failed to read arena layout data for map {map_id}: NSBundle.main.resourcePath was nil"));
        let path = std::path::Path::new(&resource_path).join(format!("assets/data/arena_layout_map{map_id}.ron"));
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read arena layout data at {path:?}: {e}"))
    };
    #[cfg(not(target_os = "ios"))]
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

pub const NAVMESH_HEIGHT: f32 = 0.3;
pub const NAVMESH_BOUNDS: [Vec2; 4] = [
    Vec2::new(-(ARENA_HALF_SIZE - 1.0), -(ARENA_HALF_SIZE - 1.0)),
    Vec2::new(ARENA_HALF_SIZE - 1.0, -(ARENA_HALF_SIZE - 1.0)),
    Vec2::new(ARENA_HALF_SIZE - 1.0, ARENA_HALF_SIZE - 1.0),
    Vec2::new(-(ARENA_HALF_SIZE - 1.0), ARENA_HALF_SIZE - 1.0),
];

pub const NAVMESH_EDGE_INSET: f32 = 1.0;

pub const ARENA_NAVMESH_AGENT_RADIUS: f32 = 0.2;
pub const ARENA_NAVMESH_SIMPLIFY: f32 = 0.005;
pub const ARENA_NAVMESH_BUILD_TIMEOUT_SECS: f32 = 1.0;

pub fn arena_navmesh_outer_edges(map_id: u8) -> Vec<Vec2> {
    let floor = arena_layout(map_id)[0];
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

pub const PILLAR_RADIUS: f32 = 0.6;

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

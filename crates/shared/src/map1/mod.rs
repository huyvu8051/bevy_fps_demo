
use bevy::prelude::{Quat, Vec2, Vec3, Vec3Swizzles};
use serde::{Deserialize, Serialize};

use crate::arena::ARENA_WALL_THICKNESS;
use crate::level_piece::{floor_panel, pieces_to_rows, ramp_panel, rows_to_pieces, wall_panel, BoxPiece, LevelPieceRow, PanelPiece};

pub const FLOOR_HEIGHT: f32 = 3.0;

pub const RAMP_RUN: f32 = 9.0;
pub const RAMP_WIDTH: f32 = 4.0;

pub const MAP1_HALF_X: f32 = 10.0;
pub const MAP1_HALF_Z: f32 = 10.0;
pub const MAP1_WALL_HEIGHT: f32 = 3.0;
pub const MAP1_NORTH_WALL_Z: f32 = -MAP1_HALF_Z;

pub const MAP1_B1: FloorLevel = FloorLevel::new(MAP1_HALF_X, MAP1_HALF_Z, MAP1_WALL_HEIGHT, 0.0, MAP1_NORTH_WALL_Z);
pub const MAP1_G: FloorLevel = FloorLevel::new(MAP1_HALF_X, MAP1_HALF_Z, MAP1_WALL_HEIGHT, FLOOR_HEIGHT, MAP1_NORTH_WALL_Z);
pub const MAP1_F2: FloorLevel = FloorLevel::new(MAP1_HALF_X, MAP1_HALF_Z, MAP1_WALL_HEIGHT, FLOOR_HEIGHT * 2.0, MAP1_NORTH_WALL_Z);
pub const MAP1_F3: FloorLevel = FloorLevel::new(MAP1_HALF_X, MAP1_HALF_Z, MAP1_WALL_HEIGHT, FLOOR_HEIGHT * 3.0, MAP1_NORTH_WALL_Z);

pub const MAP1_STAIR_SHAFT_Z: f32 = MAP1_B1.center_z();

pub fn map1_player_spawn_computed() -> Vec3 {
    Vec3::new(-7.0, 3.0, MAP1_B1.north_wall_z + 3.0)
}

#[derive(Clone, Copy)]
pub struct FloorLevel {
    pub half_size_x: f32,
    pub half_size_z: f32,
    pub wall_height: f32,
    pub elevation: f32,
    pub north_wall_z: f32,
}

impl FloorLevel {
    pub const fn new(half_size_x: f32, half_size_z: f32, wall_height: f32, elevation: f32, north_wall_z: f32) -> Self {
        Self { half_size_x, half_size_z, wall_height, elevation, north_wall_z }
    }

    pub const fn center_z(&self) -> f32 {
        self.north_wall_z + self.half_size_z
    }

    pub const fn south_wall_z(&self) -> f32 {
        self.center_z() + self.half_size_z
    }

    pub const fn next_north_wall_z(&self) -> f32 {
        self.south_wall_z() + RAMP_RUN
    }

    const fn half_y(&self) -> f32 {
        self.elevation + self.wall_height / 2.0
    }

    fn side_walls(&self) -> [(Vec3, f32, f32); 2] {
        let span_z = self.half_size_z * 2.0;
        [
            (Vec3::new(-self.half_size_x, self.half_y(), self.center_z()), ARENA_WALL_THICKNESS, span_z),
            (Vec3::new(self.half_size_x, self.half_y(), self.center_z()), ARENA_WALL_THICKNESS, span_z),
        ]
    }

    fn solid_wall(&self, z: f32) -> (Vec3, f32, f32) {
        let span_x = self.half_size_x * 2.0;
        (Vec3::new(0.0, self.half_y(), z), span_x, ARENA_WALL_THICKNESS)
    }

    fn gapped_wall(&self, z: f32) -> [(Vec3, f32, f32); 2] {
        let span_x = self.half_size_x * 2.0;
        let gap_half = RAMP_WIDTH / 2.0;
        let seg = span_x / 2.0 - gap_half;
        [
            (Vec3::new(-(gap_half + seg / 2.0), self.half_y(), z), seg, ARENA_WALL_THICKNESS),
            (Vec3::new(gap_half + seg / 2.0, self.half_y(), z), seg, ARENA_WALL_THICKNESS),
        ]
    }

    pub fn walls_ground_floor(&self) -> Vec<(Vec3, f32, f32)> {
        let mut walls = vec![self.solid_wall(self.north_wall_z)];
        walls.extend(self.gapped_wall(self.south_wall_z()));
        walls.extend(self.side_walls());
        walls
    }

    pub fn walls_mid_floor(&self) -> Vec<(Vec3, f32, f32)> {
        let mut walls = self.gapped_wall(self.north_wall_z).to_vec();
        walls.extend(self.gapped_wall(self.south_wall_z()));
        walls.extend(self.side_walls());
        walls
    }

    pub fn walls_top_floor(&self) -> Vec<(Vec3, f32, f32)> {
        let mut walls = self.gapped_wall(self.north_wall_z).to_vec();
        walls.push(self.solid_wall(self.south_wall_z()));
        walls.extend(self.side_walls());
        walls
    }

    pub fn walls_gapped_north_only(&self) -> Vec<(Vec3, f32, f32)> {
        let mut walls = self.gapped_wall(self.north_wall_z).to_vec();
        walls.push(self.solid_wall(self.south_wall_z()));
        walls.extend(self.side_walls());
        walls
    }

    pub fn walls_enclosed(&self) -> Vec<(Vec3, f32, f32)> {
        let mut walls = vec![self.solid_wall(self.north_wall_z), self.solid_wall(self.south_wall_z())];
        walls.extend(self.side_walls());
        walls
    }

    pub fn floor_slab(&self) -> (Vec3, f32, f32) {
        let size_x = self.half_size_x * 2.0;
        let size_z = self.half_size_z * 2.0;
        (Vec3::new(0.0, self.elevation - ARENA_WALL_THICKNESS / 2.0, self.center_z()), size_x, size_z)
    }

    pub fn floor_slab_with_hole(&self, hole_center_x: f32, hole_center_z: f32, hole_half_x: f32, hole_half_z: f32) -> [(Vec3, f32, f32); 4] {
        let y = self.elevation - ARENA_WALL_THICKNESS / 2.0;
        let (hx0, hx1) = (hole_center_x - hole_half_x, hole_center_x + hole_half_x);
        let (hz0, hz1) = (hole_center_z - hole_half_z, hole_center_z + hole_half_z);
        let (fx0, fx1) = (-self.half_size_x, self.half_size_x);
        let (fz0, fz1) = (self.north_wall_z, self.south_wall_z());
        [

            (Vec3::new(0.0, y, (fz0 + hz0) / 2.0), fx1 - fx0, hz0 - fz0),

            (Vec3::new(0.0, y, (hz1 + fz1) / 2.0), fx1 - fx0, fz1 - hz1),

            (Vec3::new((fx0 + hx0) / 2.0, y, hole_center_z), hx0 - fx0, hz1 - hz0),

            (Vec3::new((hx1 + fx1) / 2.0, y, hole_center_z), fx1 - hx1, hz1 - hz0),
        ]
    }

}

pub fn ramp_between(from: &FloorLevel, to: &FloorLevel) -> (Vec3, Quat, Vec3) {
    let rise = to.elevation - from.elevation;
    let slope_angle = rise.atan2(RAMP_RUN);
    let surface_len = (RAMP_RUN * RAMP_RUN + rise * rise).sqrt();
    let rotation = Quat::from_rotation_x(-slope_angle);
    let centerline = Vec3::new(0.0, from.elevation + rise / 2.0, from.south_wall_z() + RAMP_RUN / 2.0);
    let center = centerline - rotation * Vec3::new(0.0, ARENA_WALL_THICKNESS / 2.0, 0.0);
    let size = Vec3::new(RAMP_WIDTH, ARENA_WALL_THICKNESS, surface_len);
    (center, rotation, size)
}

pub fn ramp_guard_walls_between(from: &FloorLevel, to: &FloorLevel) -> [(Vec3, f32, f32, f32); 2] {
    let height = to.elevation + to.wall_height;
    let center_z = from.south_wall_z() + RAMP_RUN / 2.0;
    [
        (Vec3::new(-RAMP_WIDTH / 2.0, height / 2.0, center_z), ARENA_WALL_THICKNESS, height, RAMP_RUN),
        (Vec3::new(RAMP_WIDTH / 2.0, height / 2.0, center_z), ARENA_WALL_THICKNESS, height, RAMP_RUN),
    ]
}

pub enum RoomDoor {
    North,
    South,
    East,
    West,
}

pub fn small_room_walls(center: (f32, f32), half: Vec3, base_y: f32, door: RoomDoor) -> [(Vec3, f32, f32, f32); 3] {
    let (cx, cz) = center;
    let (size_x, size_z) = (half.x * 2.0, half.z * 2.0);
    let size_y = half.y * 2.0;
    match door {
        RoomDoor::East => [
            (Vec3::new(cx - half.x, base_y, cz), ARENA_WALL_THICKNESS, size_y, size_z + ARENA_WALL_THICKNESS),
            (Vec3::new(cx, base_y, cz - half.z), size_x, size_y, ARENA_WALL_THICKNESS),
            (Vec3::new(cx, base_y, cz + half.z), size_x, size_y, ARENA_WALL_THICKNESS),
        ],
        RoomDoor::West => [
            (Vec3::new(cx + half.x, base_y, cz), ARENA_WALL_THICKNESS, size_y, size_z + ARENA_WALL_THICKNESS),
            (Vec3::new(cx, base_y, cz - half.z), size_x, size_y, ARENA_WALL_THICKNESS),
            (Vec3::new(cx, base_y, cz + half.z), size_x, size_y, ARENA_WALL_THICKNESS),
        ],
        RoomDoor::South => [
            (Vec3::new(cx, base_y, cz - half.z), size_x, size_y, ARENA_WALL_THICKNESS),
            (Vec3::new(cx - half.x, base_y, cz), ARENA_WALL_THICKNESS, size_y, size_z + ARENA_WALL_THICKNESS),
            (Vec3::new(cx + half.x, base_y, cz), ARENA_WALL_THICKNESS, size_y, size_z + ARENA_WALL_THICKNESS),
        ],
        RoomDoor::North => [
            (Vec3::new(cx, base_y, cz + half.z), size_x, size_y, ARENA_WALL_THICKNESS),
            (Vec3::new(cx - half.x, base_y, cz), ARENA_WALL_THICKNESS, size_y, size_z + ARENA_WALL_THICKNESS),
            (Vec3::new(cx + half.x, base_y, cz), ARENA_WALL_THICKNESS, size_y, size_z + ARENA_WALL_THICKNESS),
        ],
    }
}

#[derive(Serialize, Deserialize)]
pub struct CampaignMapRon {
    pub pieces: Vec<LevelPieceRow>,
    pub zombie_spawns: Vec<(f32, f32, f32, f32)>,
    pub player_spawn: (f32, f32, f32),
}

struct Map1Data {
    pieces: (Vec<BoxPiece>, Vec<PanelPiece>),
    zombie_spawns: Vec<(Vec3, f32)>,
    player_spawn: Vec3,
}

fn map1_data() -> &'static Map1Data {
    static DATA: std::sync::OnceLock<Map1Data> = std::sync::OnceLock::new();
    DATA.get_or_init(|| {
        let text = crate::read_ron_data_file!("map/1.ron");
        let parsed: CampaignMapRon = ron::de::from_str(&text).unwrap_or_else(|e| panic!("failed to parse map/1.ron: {e}"));
        let (px, py, pz) = parsed.player_spawn;
        Map1Data {
            pieces: rows_to_pieces(parsed.pieces),
            zombie_spawns: parsed.zombie_spawns.into_iter().map(|(x, y, z, yaw)| (Vec3::new(x, y, z), yaw)).collect(),
            player_spawn: Vec3::new(px, py, pz),
        }
    })
}

pub fn map1_level_pieces() -> (Vec<BoxPiece>, Vec<PanelPiece>) {
    map1_data().pieces.clone()
}

pub fn map1_player_spawn() -> Vec3 {
    map1_data().player_spawn
}

pub fn map1_ron_text(pieces: &(Vec<BoxPiece>, Vec<PanelPiece>)) -> String {
    let data = map1_data();
    let ron_data = CampaignMapRon {
        pieces: pieces_to_rows(pieces),
        zombie_spawns: data.zombie_spawns.iter().map(|(p, yaw)| (p.x, p.y, p.z, *yaw)).collect(),
        player_spawn: (data.player_spawn.x, data.player_spawn.y, data.player_spawn.z),
    };
    ron::ser::to_string_pretty(&ron_data, ron::ser::PrettyConfig::default()).expect("serializing map/1.ron")
}

pub fn map1_zombie_spawn_pos(seed: usize) -> (Vec3, f32) {
    let points = &map1_data().zombie_spawns;
    points[seed % points.len()]
}

pub fn map1_zombie_spawn_points_computed() -> Vec<(Vec3, f32)> {

    const OFFSETS: [(f32, f32); 7] = [(0.0, 0.0), (8.0, 8.0), (8.0, -8.0), (-8.0, 8.0), (-8.0, -8.0), (0.0, 8.0), (0.0, -8.0)];

    let floors: [FloorLevel; 4] = [MAP1_B1, MAP1_G, MAP1_F2, MAP1_F3];
    let mut points = Vec::with_capacity(OFFSETS.len() * floors.len());

    for (dx, dz) in OFFSETS {
        for floor in floors {
            const SPAWN_CLEARANCE: f32 = 0.1;
            let y = floor.elevation
                + crate::zombie::ZOMBIE_RADIUS
                + crate::zombie::ZOMBIE_LENGTH / 2.0
                + SPAWN_CLEARANCE;
            let x = dx;
            let z = MAP1_STAIR_SHAFT_Z + dz;

            let yaw = (-dx).atan2(-dz);
            points.push((Vec3::new(x, y, z), yaw));
        }
    }
    points
}

pub fn campaign_zombie_spawn_pos(map_id: u8, seed: usize) -> (Vec3, f32) {
    if map_id == 1 {
        return map1_zombie_spawn_pos(seed);
    }
    panic!(
        "no sandbox zombie spawn bounds for campaign map_id {map_id} — \
         only map_id 1 has floor geometry today; this map needs its wave \
         data (docs/ZOMBIE_SPAWN_SYSTEM.md) before real play anyway, not a \
         sandbox scatter fallback"
    )
}

struct RonFloorExtent {
    elevation: f32,
    x0: f32,
    x1: f32,
    z0: f32,
    z1: f32,

    covered_area: f32,
}

impl RonFloorExtent {

    fn has_hole(&self) -> bool {
        let bbox_area = (self.x1 - self.x0) * (self.z1 - self.z0);
        self.covered_area < bbox_area - 1.0e-2
    }
}

fn map1_ron_floor_extents() -> Vec<RonFloorExtent> {
    let flat_rotation = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
    let (_, panels) = map1_level_pieces();
    let mut extents: Vec<RonFloorExtent> = Vec::new();
    for (translation, rotation, size_x, size_z, _color, _roughness, collider_thickness, collider_offset) in panels {
        if !collider_offset || collider_thickness.is_none() {
            continue;
        }
        if rotation.angle_between(flat_rotation) > 1.0e-3 {
            continue;
        }
        let (x0, x1) = (translation.x - size_x / 2.0, translation.x + size_x / 2.0);
        let (z0, z1) = (translation.z - size_z / 2.0, translation.z + size_z / 2.0);
        let strip_area = size_x * size_z;
        match extents.iter_mut().find(|e| (e.elevation - translation.y).abs() < 1.0e-3) {
            Some(e) => {
                e.x0 = e.x0.min(x0);
                e.x1 = e.x1.max(x1);
                e.z0 = e.z0.min(z0);
                e.z1 = e.z1.max(z1);
                e.covered_area += strip_area;
            }
            None => extents.push(RonFloorExtent { elevation: translation.y, x0, x1, z0, z1, covered_area: strip_area }),
        }
    }
    extents.sort_by(|a, b| a.elevation.total_cmp(&b.elevation));
    extents
}

pub struct Map1NavLayer {
    pub translation: Vec3,
    pub rotation: Quat,
    pub outer_world: Vec<Vec3>,
    pub holes_world: Vec<Vec<Vec3>>,
}

pub struct Map1NavStitch {
    pub from: u8,
    pub to: u8,
    pub points_world: [Vec3; 2],
}

pub fn map1_world_to_layer_local(translation: Vec3, rotation: Quat, world_point: Vec3) -> Vec2 {
    (rotation.inverse() * (world_point - translation)).xy()
}

fn map1_hole_corners_world(shaft_x: f32, elevation: f32) -> Vec<Vec3> {
    let half_x = TOWER_STAIR_WIDTH / 2.0;
    let half_z = simple_ramp_stacked_half_len();
    let (x0, x1) = (shaft_x - half_x, shaft_x + half_x);
    let (z0, z1) = (MAP1_STAIR_SHAFT_Z - half_z, MAP1_STAIR_SHAFT_Z + half_z);
    vec![
        Vec3::new(x0, elevation, z0),
        Vec3::new(x1, elevation, z0),
        Vec3::new(x1, elevation, z1),
        Vec3::new(x0, elevation, z1),
    ]
}

pub fn map1_navmesh_layers() -> (Vec<Map1NavLayer>, Vec<Map1NavStitch>) {
    let flat_rotation = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
    let floor_extents = map1_ron_floor_extents();

    let mut layers = Vec::with_capacity(floor_extents.len() * 2);
    for extent in &floor_extents {
        let (elevation, x0, x1, z0, z1) = (extent.elevation, extent.x0, extent.x1, extent.z0, extent.z1);
        let holes = if extent.has_hole() { vec![map1_hole_corners_world(TOWER_SHAFT_X, elevation)] } else { Vec::new() };
        layers.push(Map1NavLayer {
            translation: Vec3::new(0.0, elevation, MAP1_STAIR_SHAFT_Z),
            rotation: flat_rotation,
            outer_world: vec![Vec3::new(x0, elevation, z0), Vec3::new(x1, elevation, z0), Vec3::new(x1, elevation, z1), Vec3::new(x0, elevation, z1)],
            holes_world: holes,
        });
    }

    let mut stitches = Vec::with_capacity(floor_extents.len().saturating_sub(1) * 2);
    for i in 0..floor_extents.len().saturating_sub(1) {
        let below_idx = i;
        let above_idx = i + 1;

        let below = FloorLevel::new(0.0, 0.0, 0.0, floor_extents[below_idx].elevation, 0.0);
        let above = FloorLevel::new(0.0, 0.0, 0.0, floor_extents[above_idx].elevation, 0.0);
        let (raw_center, raw_rotation, size) = simple_ramp_stacked(TOWER_SHAFT_X, MAP1_STAIR_SHAFT_Z, &below, &above);

        let (translation, rotation, width, length, ..) = ramp_panel(raw_center, raw_rotation, size.x, size.y, size.z, [0.0, 0.0, 0.0], 0.0);
        let half_w = width / 2.0;
        let half_l = length / 2.0;
        let local_to_world = |local: Vec2| translation + rotation * local.extend(0.0);
        let outer_world = vec![
            local_to_world(Vec2::new(-half_w, -half_l)),
            local_to_world(Vec2::new(half_w, -half_l)),
            local_to_world(Vec2::new(half_w, half_l)),
            local_to_world(Vec2::new(-half_w, half_l)),
        ];
        let layer_index = layers.len() as u8;
        layers.push(Map1NavLayer { translation, rotation, outer_world, holes_world: Vec::new() });

        stitches.push(Map1NavStitch {
            from: below_idx as u8,
            to: layer_index,
            points_world: [local_to_world(Vec2::new(-half_w, -half_l)), local_to_world(Vec2::new(half_w, -half_l))],
        });
        stitches.push(Map1NavStitch {
            from: layer_index,
            to: above_idx as u8,
            points_world: [local_to_world(Vec2::new(-half_w, half_l)), local_to_world(Vec2::new(half_w, half_l))],
        });
    }

    (layers, stitches)
}

#[cfg(test)]
mod regenerate_spawns {
    use super::*;

    #[test]
    #[ignore]
    fn regenerate_map1_ron() {
        let pieces = map1_level_pieces_computed();
        let points = map1_zombie_spawn_points_computed();
        let spawn = map1_player_spawn_computed();
        let ron_data = CampaignMapRon {
            pieces: pieces_to_rows(&pieces),
            zombie_spawns: points.iter().map(|(p, yaw)| (p.x, p.y, p.z, *yaw)).collect(),
            player_spawn: (spawn.x, spawn.y, spawn.z),
        };
        let text = ron::ser::to_string_pretty(&ron_data, ron::ser::PrettyConfig::default()).expect("serializing map/1.ron");
        let path = format!("{}/../client/assets/data/map/1.ron", env!("CARGO_MANIFEST_DIR"));
        std::fs::write(&path, text).unwrap_or_else(|e| panic!("failed to write {path}: {e}"));
        println!("wrote {path} ({} pieces, {} zombie spawns)", pieces.0.len() + pieces.1.len(), points.len());
    }
}

pub const TOWER_SHAFT_X: f32 = 6.0;

pub const TOWER_STAIR_WIDTH: f32 = 3.0;

pub const TOWER_STAIR_RUN: f32 = 1.5;
pub const TOWER_STAIR_LANDING_RUN: f32 = 1.5;

pub const TOWER_SHAFT_Z_OFFSET: f32 = 16.0;

fn ramp_segment(start: Vec3, end: Vec3, width: f32, thickness: f32) -> (Vec3, Quat, Vec3) {
    let delta = end - start;
    let run = (delta.x * delta.x + delta.z * delta.z).sqrt();
    let rise = delta.y;

    let heading = delta.x.atan2(delta.z);
    let slope_angle = rise.atan2(run);
    let surface_len = (run * run + rise * rise).sqrt();
    let rotation = Quat::from_rotation_y(heading) * Quat::from_rotation_x(-slope_angle);
    let centerline = (start + end) / 2.0;
    let center = centerline - rotation * Vec3::new(0.0, thickness / 2.0, 0.0);
    (center, rotation, Vec3::new(width, thickness, surface_len))
}

pub const MAP1_RAMP_RUN: f32 = FLOOR_HEIGHT * 2.0;

pub fn simple_ramp_stacked(shaft_x: f32, shaft_z: f32, from: &FloorLevel, to: &FloorLevel) -> (Vec3, Quat, Vec3) {
    let half_run = MAP1_RAMP_RUN / 2.0;
    ramp_segment(
        Vec3::new(shaft_x, from.elevation, shaft_z - half_run),
        Vec3::new(shaft_x, to.elevation, shaft_z + half_run),
        TOWER_STAIR_WIDTH,
        ARENA_WALL_THICKNESS,
    )
}

pub const fn simple_ramp_stacked_half_len() -> f32 {
    MAP1_RAMP_RUN / 2.0
}

#[allow(dead_code)]
pub fn simple_stairwell(shaft_x: f32, shaft_z: f32, from: &FloorLevel, to: &FloorLevel) -> Vec<(Vec3, Quat, Vec3)> {
    let mid_elevation = (from.elevation + to.elevation) / 2.0;
    let half_landing = TOWER_STAIR_LANDING_RUN / 2.0;
    let z0 = shaft_z - half_landing - TOWER_STAIR_RUN;
    let z1 = shaft_z - half_landing;
    let z2 = shaft_z + half_landing;
    let z3 = shaft_z + half_landing + TOWER_STAIR_RUN;

    let ramp1 = ramp_segment(Vec3::new(shaft_x, from.elevation, z0), Vec3::new(shaft_x, mid_elevation, z1), TOWER_STAIR_WIDTH, ARENA_WALL_THICKNESS);
    let ramp2 = ramp_segment(Vec3::new(shaft_x, mid_elevation, z2), Vec3::new(shaft_x, to.elevation, z3), TOWER_STAIR_WIDTH, ARENA_WALL_THICKNESS);

    let landing_centerline = Vec3::new(shaft_x, mid_elevation, shaft_z);
    let landing = (landing_centerline, Quat::IDENTITY, Vec3::new(TOWER_STAIR_WIDTH, ARENA_WALL_THICKNESS, TOWER_STAIR_LANDING_RUN));

    vec![ramp1, landing, ramp2]
}

pub const fn simple_stairwell_half_len() -> f32 {
    TOWER_STAIR_RUN + TOWER_STAIR_LANDING_RUN / 2.0
}

pub fn map1_level_pieces_computed() -> (Vec<BoxPiece>, Vec<PanelPiece>) {
    let pieces: Vec<BoxPiece> = Vec::new();
    let mut panels: Vec<PanelPiece> = Vec::new();
    let floor_color = [0.5, 0.5, 0.52];
    let wall_color = [0.6, 0.58, 0.55];
    #[allow(unused_variables)]
    let stair_color = [0.45, 0.43, 0.4];

    let (c, sx, sz) = MAP1_B1.floor_slab();
    panels.push(floor_panel(c, sx, sz, ARENA_WALL_THICKNESS, floor_color, 0.85));
    for (c, sx, sz) in MAP1_B1.walls_enclosed() {
        panels.push(wall_panel(c, sx, sz, MAP1_B1.wall_height, wall_color, 0.75));
    }

    (pieces, panels)
}

pub fn campaign_level_pieces(map_id: u8) -> (Vec<BoxPiece>, Vec<PanelPiece>) {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    if map_id == 1 {
        return map1_level_pieces();
    }
    {
        static CACHE: OnceLock<Mutex<HashMap<u8, (Vec<BoxPiece>, Vec<PanelPiece>)>>> = OnceLock::new();
        let mut cache = CACHE.get_or_init(|| Mutex::new(HashMap::new())).lock().expect("campaign level pieces cache poisoned");
        if let Some(pieces) = cache.get(&map_id) {
            return pieces.clone();
        }

        #[cfg(target_os = "ios")]
        let text = {
            let resource_path = crate::ios_bundle_resource_path().unwrap_or_else(|| {
                panic!("no level pieces RON for campaign map_id {map_id}: NSBundle.main.resourcePath was nil")
            });
            let path = std::path::Path::new(&resource_path).join(format!("assets/data/map/{map_id}.ron"));
            std::fs::read_to_string(&path).unwrap_or_else(|e| {
                panic!(
                    "no level pieces RON for campaign map_id {map_id} at {path:?}: {e} — \
                     this map isn't implemented yet, see docs/maps/gdd/map{map_id:02}_dimensions.md \
                     for its design numbers"
                )
            })
        };
        #[cfg(not(target_os = "ios"))]
        let text = {
            let path = format!("{}/../client/assets/data/map/{map_id}.ron", env!("CARGO_MANIFEST_DIR"));
            std::fs::read_to_string(&path).unwrap_or_else(|e| {
                panic!(
                    "no level pieces RON for campaign map_id {map_id} at {path}: {e} — \
                     this map isn't implemented yet, see docs/maps/gdd/map{map_id:02}_dimensions.md \
                     for its design numbers"
                )
            })
        };
        let pieces = crate::level_piece::level_pieces_from_ron(&text);
        cache.insert(map_id, pieces.clone());
        pieces
    }
}

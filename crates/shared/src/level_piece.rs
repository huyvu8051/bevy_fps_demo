
use bevy::prelude::{EulerRot, Component, Quat, Vec3};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub enum LevelPieceRow {
    Box(f32, f32, f32, f32, f32, f32, f32, f32, f32, f32, f32, f32, f32),
    Panel(f32, f32, f32, f32, f32, f32, f32, f32, f32, f32, f32, f32, Option<f32>, bool),
}

fn box_piece_to_row(piece: &BoxPiece) -> LevelPieceRow {
    let &(t, r, s, c, roughness) = piece;
    let (ry, rx, rz) = r.to_euler(EulerRot::YXZ);
    LevelPieceRow::Box(t.x, t.y, t.z, rx.to_degrees(), ry.to_degrees(), rz.to_degrees(), s.x, s.y, s.z, c[0], c[1], c[2], roughness)
}

fn row_to_box_piece(row: (f32, f32, f32, f32, f32, f32, f32, f32, f32, f32, f32, f32, f32)) -> BoxPiece {
    let (tx, ty, tz, rx, ry, rz, sx, sy, sz, cr, cg, cb, roughness) = row;
    let rotation = Quat::from_euler(EulerRot::YXZ, ry.to_radians(), rx.to_radians(), rz.to_radians());
    (Vec3::new(tx, ty, tz), rotation, Vec3::new(sx, sy, sz), [cr, cg, cb], roughness)
}

fn panel_piece_to_row(piece: &PanelPiece) -> LevelPieceRow {
    let &(t, r, width, height, c, roughness, collider_thickness, collider_offset) = piece;
    let (ry, rx, rz) = r.to_euler(EulerRot::YXZ);
    LevelPieceRow::Panel(
        t.x, t.y, t.z, rx.to_degrees(), ry.to_degrees(), rz.to_degrees(), width, height, c[0], c[1], c[2], roughness, collider_thickness, collider_offset,
    )
}

fn row_to_panel_piece(row: (f32, f32, f32, f32, f32, f32, f32, f32, f32, f32, f32, f32, Option<f32>, bool)) -> PanelPiece {
    let (tx, ty, tz, rx, ry, rz, width, height, cr, cg, cb, roughness, collider_thickness, collider_offset) = row;
    let rotation = Quat::from_euler(EulerRot::YXZ, ry.to_radians(), rx.to_radians(), rz.to_radians());
    (Vec3::new(tx, ty, tz), rotation, width, height, [cr, cg, cb], roughness, collider_thickness, collider_offset)
}

pub fn pieces_to_rows(pieces: &(Vec<BoxPiece>, Vec<PanelPiece>)) -> Vec<LevelPieceRow> {
    let mut rows: Vec<LevelPieceRow> = Vec::with_capacity(pieces.0.len() + pieces.1.len());
    rows.extend(pieces.0.iter().map(box_piece_to_row));
    rows.extend(pieces.1.iter().map(panel_piece_to_row));
    rows
}

pub fn rows_to_pieces(rows: Vec<LevelPieceRow>) -> (Vec<BoxPiece>, Vec<PanelPiece>) {
    let mut boxes = Vec::new();
    let mut panels = Vec::new();
    for row in rows {
        match row {
            LevelPieceRow::Box(a, b, c, d, e, f, g, h, i, j, k, l, m) => boxes.push(row_to_box_piece((a, b, c, d, e, f, g, h, i, j, k, l, m))),
            LevelPieceRow::Panel(a, b, c, d, e, f, g, h, i, j, k, l, m, n) => panels.push(row_to_panel_piece((a, b, c, d, e, f, g, h, i, j, k, l, m, n))),
        }
    }
    (boxes, panels)
}

pub fn level_pieces_to_ron(pieces: &(Vec<BoxPiece>, Vec<PanelPiece>)) -> String {
    ron::ser::to_string_pretty(&pieces_to_rows(pieces), ron::ser::PrettyConfig::default().struct_names(false)).expect("serializing level pieces to RON")
}

pub fn level_pieces_from_ron(text: &str) -> (Vec<BoxPiece>, Vec<PanelPiece>) {
    let rows: Vec<LevelPieceRow> = ron::de::from_str(text).unwrap_or_else(|e| panic!("failed to parse level pieces RON: {e}"));
    rows_to_pieces(rows)
}

pub type BoxPiece = (Vec3, Quat, Vec3, [f32; 3], f32);

pub type PanelPiece = (Vec3, Quat, f32, f32, [f32; 3], f32, Option<f32>, bool);

pub fn wall_panel(center: Vec3, size_x: f32, size_z: f32, wall_height: f32, color: [f32; 3], roughness: f32) -> PanelPiece {
    if size_x >= size_z {
        (center, Quat::IDENTITY, size_x, wall_height, color, roughness, Some(size_z), false)
    } else {
        (center, Quat::from_rotation_y(std::f32::consts::FRAC_PI_2), size_z, wall_height, color, roughness, Some(size_x), false)
    }
}

pub fn wall_panel4(center: Vec3, size_x: f32, size_y: f32, size_z: f32, color: [f32; 3], roughness: f32) -> PanelPiece {
    if size_x >= size_z {
        (center, Quat::IDENTITY, size_x, size_y, color, roughness, Some(size_z), false)
    } else {
        (center, Quat::from_rotation_y(std::f32::consts::FRAC_PI_2), size_z, size_y, color, roughness, Some(size_x), false)
    }
}

pub fn floor_panel(center: Vec3, size_x: f32, size_z: f32, thickness: f32, color: [f32; 3], roughness: f32) -> PanelPiece {
    let rotation = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
    let translation = center + rotation * Vec3::new(0.0, 0.0, thickness / 2.0);
    (translation, rotation, size_x, size_z, color, roughness, Some(thickness), true)
}

pub fn ramp_panel(center: Vec3, ramp_rotation: Quat, width: f32, thickness: f32, length: f32, color: [f32; 3], roughness: f32) -> PanelPiece {
    let rotation = ramp_rotation * Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
    let translation = center + rotation * Vec3::new(0.0, 0.0, thickness / 2.0);
    (translation, rotation, width, length, color, roughness, Some(thickness), true)
}

pub fn pillar_piece(center: Vec3, size_x: f32, height: f32, size_z: f32, color: [f32; 3], roughness: f32) -> BoxPiece {
    (center, Quat::IDENTITY, Vec3::new(size_x, height, size_z), color, roughness)
}

#[derive(Component, Clone, Copy, Serialize, Deserialize)]
pub struct LevelBox {

    pub size: Vec3,

    pub color: [f32; 3],
    pub roughness: f32,
}

impl LevelBox {
    pub const fn new(size: Vec3, color: [f32; 3], roughness: f32) -> Self {
        Self { size, color, roughness }
    }
}

#[derive(Component, Clone, Copy, Serialize, Deserialize)]
pub struct LevelPanel {
    pub width: f32,
    pub height: f32,
    pub color: [f32; 3],
    pub roughness: f32,
    pub collider_thickness: Option<f32>,
    pub collider_offset: bool,
}

impl LevelPanel {

    pub const fn new(width: f32, height: f32, color: [f32; 3], roughness: f32) -> Self {
        Self { width, height, color, roughness, collider_thickness: None, collider_offset: false }
    }

    pub const fn solid(width: f32, height: f32, color: [f32; 3], roughness: f32, collider_thickness: f32) -> Self {
        Self { width, height, color, roughness, collider_thickness: Some(collider_thickness), collider_offset: false }
    }

    pub const fn solid_surface(width: f32, height: f32, color: [f32; 3], roughness: f32, collider_thickness: f32) -> Self {
        Self { width, height, color, roughness, collider_thickness: Some(collider_thickness), collider_offset: true }
    }
}

#[derive(Component, Clone, Copy, Serialize, Deserialize)]
pub struct LevelTransform {
    pub translation: Vec3,
    pub rotation: Quat,
}

impl LevelTransform {
    pub const IDENTITY_ROTATION: Quat = Quat::IDENTITY;

    pub const fn from_translation(translation: Vec3) -> Self {
        Self { translation, rotation: Self::IDENTITY_ROTATION }
    }

    pub const fn new(translation: Vec3, rotation: Quat) -> Self {
        Self { translation, rotation }
    }
}

#[cfg(test)]
mod regenerate {
    use super::*;

    fn write_ron(relative_path: &str, pieces: &(Vec<BoxPiece>, Vec<PanelPiece>)) {
        let path = format!("{}/../client/assets/data/{relative_path}", env!("CARGO_MANIFEST_DIR"));
        std::fs::write(&path, level_pieces_to_ron(pieces)).unwrap_or_else(|e| panic!("failed to write {path}: {e}"));
        println!("wrote {path} ({} boxes, {} panels)", pieces.0.len(), pieces.1.len());
    }

    #[test]
    #[ignore]
    fn regenerate_level_piece_ron() {
        write_ron("map/3.ron", &crate::item_gallery::item_gallery_level_pieces_computed());
    }
}

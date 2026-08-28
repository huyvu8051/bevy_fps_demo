// --- Scenario::ItemGallery — flat scale-reference rig, not a real map ---
//
// One of EVERY standard item size from `docs/maps/map01_dimensions.md`
// section 1 (the shared item-size library, not just Map 1's), laid out in
// a single grid row on a flat floor. Purpose: open this in the dev editor
// (EDITOR=1, F2 — see third_party/bevy_editor_pls) and eyeball each real-
// world-derived size next to the player capsule / a zombie BEFORE it gets
// committed into an actual room layout — cheaper to catch "that desk reads
// way too small in-engine" here than after a whole floor is built around
// wrong numbers.
//
// Each entry becomes 1 LevelBox (item_gallery_level_pieces below, called
// by both client and server — see level_piece.rs's module doc comment)
// — a plain cuboid at the item's stated (width, height, depth), no
// attempt at a real mesh/shape. Good enough to judge footprint and
// height; not meant to look like the real prop.

use bevy::prelude::{Quat, Vec3};

use crate::arena::ARENA_WALL_THICKNESS;
use crate::level_piece::{floor_panel, wall_panel, BoxPiece, PanelPiece};

/// (label, size) — label is for code-reading only (this crate has no text
/// rendering); it also becomes each box's line in the console log the
/// server prints on startup for this scenario, since there's no in-game
/// nameplate to read otherwise.
pub const ITEM_GALLERY_ENTRIES: [(&str, Vec3); 10] = [
    ("Sedan (gara)", Vec3::new(1.9, 1.5, 4.6)),
    ("Ban lam viec don", Vec3::new(1.4, 0.75, 0.7)),
    ("1 o cubicle", Vec3::new(1.6, 1.2, 1.6)),
    ("Quay le tan", Vec3::new(2.2, 1.1, 0.6)),
    ("Tu do/locker", Vec3::new(0.4, 1.8, 0.5)),
    ("Cua don", Vec3::new(0.9, 2.1, 0.15)),
    ("Cua doi/kinh lon", Vec3::new(1.8, 2.4, 0.15)),
    ("Cau thang xoan oc (footprint)", Vec3::new(4.0, 3.0, 4.0)),
    ("Tuong ngoai (1 nhip 3m)", Vec3::new(3.0, 2.8, 1.0)),
    ("Tuong/vach noi that (1 nhip 3m)", Vec3::new(3.0, 2.8, 0.15)),
];

// Grid layout: 1 hàng ngang, cách nhau đều GALLERY_SPACING (đủ rộng hơn
// item to nhất — sedan dài 4.6m — để không đè lên nhau), tâm mỗi item đặt
// ngay trên mặt sàn (y = size.y / 2).
pub const ITEM_GALLERY_SPACING: f32 = 7.0;
pub const ITEM_GALLERY_HALF_SIZE_X: f32 =
    (ITEM_GALLERY_ENTRIES.len() as f32) * ITEM_GALLERY_SPACING / 2.0 + 5.0;
pub const ITEM_GALLERY_HALF_SIZE_Z: f32 = 10.0;
pub const ITEM_GALLERY_WALL_HEIGHT: f32 = 4.0;

/// Toạ độ (x, z) tâm từng item — cùng thứ tự `ITEM_GALLERY_ENTRIES`, hàng
/// ngang qua Z=0, item đầu tiên ở cực Tây.
pub fn item_gallery_positions() -> Vec<(f32, f32)> {
    let start_x = -(ITEM_GALLERY_ENTRIES.len() as f32 - 1.0) * ITEM_GALLERY_SPACING / 2.0;
    (0..ITEM_GALLERY_ENTRIES.len())
        .map(|i| (start_x + i as f32 * ITEM_GALLERY_SPACING, 0.0))
        .collect()
}

// Player spawn: đứng lùi về cực Nam, nhìn dọc theo cả dãy item — đủ xa để
// thấy hết dãy khi mới vào, không kẹt giữa 2 item. Không spawn zombie/
// gameplay gì khác — rig này chỉ để đi lại quan sát bằng camera người
// chơi bình thường (+ editor F2 nếu cần), không phải 1 scenario chơi được.
pub const ITEM_GALLERY_PLAYER_SPAWN: Vec3 = Vec3::new(0.0, 3.0, ITEM_GALLERY_HALF_SIZE_Z - 2.0);

/// Flat floor + 4 walls + 1 LevelBox per ITEM_GALLERY_ENTRIES — pure
/// data, no spawning (see map1_level_pieces's own doc comment for why:
/// same function, called by both client and server).
pub fn item_gallery_level_pieces() -> (Vec<BoxPiece>, Vec<PanelPiece>) {
    let half_x = ITEM_GALLERY_HALF_SIZE_X;
    let half_z = ITEM_GALLERY_HALF_SIZE_Z;
    let floor_color = [0.35, 0.35, 0.38];
    let wall_color = [0.5, 0.5, 0.52];

    let mut pieces: Vec<BoxPiece> = Vec::new();
    let mut panels: Vec<PanelPiece> = Vec::new();

    panels.push(floor_panel(Vec3::ZERO, half_x * 2.0, half_z * 2.0, ARENA_WALL_THICKNESS, floor_color, 0.9));

    let wall_span_x = half_x * 2.0 + ARENA_WALL_THICKNESS;
    let wall_span_z = half_z * 2.0 + ARENA_WALL_THICKNESS;
    let walls = [
        (Vec3::new(0.0, ITEM_GALLERY_WALL_HEIGHT / 2.0, -half_z), wall_span_x, ARENA_WALL_THICKNESS),
        (Vec3::new(0.0, ITEM_GALLERY_WALL_HEIGHT / 2.0, half_z), wall_span_x, ARENA_WALL_THICKNESS),
        (Vec3::new(-half_x, ITEM_GALLERY_WALL_HEIGHT / 2.0, 0.0), ARENA_WALL_THICKNESS, wall_span_z),
        (Vec3::new(half_x, ITEM_GALLERY_WALL_HEIGHT / 2.0, 0.0), ARENA_WALL_THICKNESS, wall_span_z),
    ];
    for (center, size_x, size_z) in walls {
        panels.push(wall_panel(center, size_x, size_z, ITEM_GALLERY_WALL_HEIGHT, wall_color, 0.8));
    }

    let ident = Quat::IDENTITY;
    for (i, (_label, size)) in ITEM_GALLERY_ENTRIES.iter().enumerate() {
        let (x, z) = item_gallery_positions()[i];
        // Đổi màu xen kẽ theo chỉ số — không mang ý nghĩa gì, chỉ để dễ
        // phân biệt item liền kề nhau bằng mắt khi đứng xa.
        let color = if i % 2 == 0 { [0.75, 0.55, 0.2] } else { [0.25, 0.45, 0.75] };
        pieces.push((Vec3::new(x, size.y / 2.0, z), ident, *size, color, 0.6));
    }

    (pieces, panels)
}

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
// 20 (was 10) — the flat floor needs to reach the 2nd row of meshy.ai
// test cars (client's spawn_item_gallery_test_props) at z=-14; 6m of
// margin left to the north wall.
pub const ITEM_GALLERY_HALF_SIZE_Z: f32 = 20.0;
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

/// Quick-review override for ITEM_GALLERY_PLAYER_SPAWN — set env
/// `REVIEW_SPAWN="x,y,z"` (server-side, before starting `MAP_ID=5`) to
/// spawn directly next to whichever prop is being eyeballed instead of
/// walking/noclipping there from the default south-end spawn every time.
/// Falls back to ITEM_GALLERY_PLAYER_SPAWN if unset or malformed. See also
/// the client's REVIEW_YAW/REVIEW_PITCH (crates/client/src/lib.rs) for the
/// matching initial look-direction override.
pub fn item_gallery_player_spawn() -> Vec3 {
    let Ok(raw) = std::env::var("REVIEW_SPAWN") else {
        return ITEM_GALLERY_PLAYER_SPAWN;
    };
    let parts: Vec<f32> = raw.split(',').filter_map(|s| s.trim().parse().ok()).collect();
    match parts[..] {
        [x, y, z] => Vec3::new(x, y, z),
        _ => ITEM_GALLERY_PLAYER_SPAWN,
    }
}

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

// --- meshy.ai car test props — collision/physics layout -------------------
//
// Shared so client (visual mesh + collider, docs/PROP_IMPORT_CHECKLIST.md)
// and server (collider only — hit registration via SpatialQuery, movement
// blocking via MoveAndSlide) agree on where each car's collider actually
// is; drifting the two independently would silently break "bullets stop
// registering" or "player walks through" on whichever side didn't get the
// update.

/// (entity scale, entity Y translation, real width, height, depth in
/// meters) per car, same order/count as client's `ITEM_GALLERY_CARS`
/// (which zips this 1:1 by index for its own asset paths). scale/Y match
/// each SceneRoot's own Transform exactly (see that array's own comments
/// for why the first 4 need a scale/Y hack and the rest don't) — the
/// collider size below is pre-divided by scale so it comes out to the
/// stated real-world size once avian3d applies the entity's Transform.scale
/// to it (confirmed behavior: colliders scale with ancestor Transform).
/// Real sizes match docs/maps/map01_dimensions.md mục 1's "Xe" rows.
pub const ITEM_GALLERY_CAR_PHYSICS: [(f32, f32, f32, f32, f32); 16] = [
    (2.2, 1.5, 1.9, 1.5, 4.6),     // sedan
    (115.0, 0.0, 2.0, 2.3, 5.5),   // van
    (1.0, 0.0, 2.6, 2.2, 5.3),     // pickup
    (100.0, 0.0, 1.68, 1.6, 3.19), // vf3
    (1.0, 0.0, 1.9, 1.8, 4.6),     // suv
    (1.0, 0.0, 1.7, 1.45, 3.9),    // hatchback
    (1.0, 0.0, 1.85, 1.25, 4.5),   // sports
    (1.0, 0.0, 1.8, 1.5, 4.8),     // taxi
    (1.0, 0.0, 1.85, 1.45, 5.0),   // police
    (1.0, 0.0, 2.1, 2.6, 6.0),     // ambulance
    (1.0, 0.0, 2.2, 2.5, 6.5),     // delivery
    (1.0, 0.0, 2.3, 2.3, 6.5),     // towtruck
    (1.0, 0.0, 1.8, 1.5, 4.8),     // wagon
    (1.0, 0.0, 0.8, 1.15, 2.2),    // motorcycle
    (1.0, 0.0, 1.9, 1.35, 5.0),    // muscle
    (1.0, 0.0, 1.95, 1.75, 5.1),   // minivan
];

// 2 rows of 8, 6m apart on both axes — independent of
// item_gallery_positions() (only 10 slots, tied 1:1 to
// ITEM_GALLERY_ENTRIES) since 16 cars no longer fits that grid.
const ITEM_GALLERY_CAR_ROW_SPACING: f32 = 6.0;
const ITEM_GALLERY_CAR_ROW_LEN: usize = 8;
const ITEM_GALLERY_CAR_ROW_Z: [f32; 2] = [-6.0, -14.0];

/// (x, z) grid position for `ITEM_GALLERY_CAR_PHYSICS[i]`/client's
/// `ITEM_GALLERY_CARS[i]` — same index, same order.
pub fn item_gallery_car_positions() -> Vec<(f32, f32)> {
    let start_x = -(ITEM_GALLERY_CAR_ROW_LEN as f32 - 1.0) * ITEM_GALLERY_CAR_ROW_SPACING / 2.0;
    (0..ITEM_GALLERY_CAR_PHYSICS.len())
        .map(|i| {
            let row = i / ITEM_GALLERY_CAR_ROW_LEN;
            let col = i % ITEM_GALLERY_CAR_ROW_LEN;
            (start_x + col as f32 * ITEM_GALLERY_CAR_ROW_SPACING, ITEM_GALLERY_CAR_ROW_Z[row])
        })
        .collect()
}

/// Resolved per-car collider placement — everything a collider-only spawn
/// (server) or a collider+visual spawn (client) needs, already divided by
/// `scale` so `Collider::cuboid(size.x, size.y, size.z)` on a child at
/// local Y `collider_local_y` comes out to the real-world box once avian3d
/// re-multiplies by the parent entity's Transform.scale.
pub struct ItemGalleryCarPhysics {
    pub x: f32,
    pub z: f32,
    pub scale: f32,
    /// Rest-height Y translation for the entity itself (matches the
    /// SceneRoot's own Transform.y on the client — NOT the collider
    /// center).
    pub y: f32,
    pub collider_size: Vec3,
    pub collider_local_y: f32,
}

pub fn item_gallery_car_physics() -> Vec<ItemGalleryCarPhysics> {
    ITEM_GALLERY_CAR_PHYSICS
        .iter()
        .zip(item_gallery_car_positions())
        .map(|(&(scale, y, w, h, d), (x, z))| ItemGalleryCarPhysics {
            x,
            z,
            scale,
            y,
            collider_size: Vec3::new(w, h, d) / scale,
            collider_local_y: (h / 2.0) / scale,
        })
        .collect()
}

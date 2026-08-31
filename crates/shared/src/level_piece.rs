// --- Level geometry: compiled into BOTH client and server ---
// Static level pieces (walls, floors, ramps, cover boxes...) are built
// independently by both the server (collider-only, for its own
// authoritative physics) and the client (mesh + collider, for rendering
// + local movement prediction), each reading the exact same shared
// piece-building functions below (map1_level_pieces/
// item_gallery_level_pieces) — which in turn read the same
// shared::map1/item_gallery Rust constants everything else does. No
// network round-trip decides what a map looks like; it's baked into
// both binaries at compile time.
//
// (This crate briefly went the other way — server-authoritative,
// replicated LevelBox/LevelPanel entities, so an already-installed
// client could pick up a map change without a rebuild — see git history
// around "replicate level geometry to client" if that's ever worth
// resurrecting. Reverted: for mobile specifically, "rebuild to see a
// map change" was judged less of a problem than "the map simply
// doesn't appear if the app can't reach a live server process".)
//
// LevelBox (a solid cuboid) and LevelPanel (a thin flat rectangle, no
// real volume) stay as components — not everything in a level needs 6
// faces; a floor slab or a wall only needs 1. The client still builds
// its mesh+collider REACTIVELY off these (Added<LevelBox>/
// Added<LevelPanel>, see spawn_level_box_visuals/
// spawn_level_panel_visuals) — that reactive pattern doesn't care
// whether the entity came from the network or a local Startup spawn,
// so it needed no changes for this revert; only who calls
// `commands.spawn` and with which components changed.

use bevy::prelude::{Component, Quat, Vec3};
use serde::{Deserialize, Serialize};

// --- RON round-trip for BoxPiece/PanelPiece lists ---------------------
//
// Same idea as arena.rs's ArenaLayoutRow/item_gallery's CarPhysicsRow —
// map1/item_gallery's actual geometry (crates/client/assets/data/
// map1_level_pieces.ron, item_gallery_level_pieces.ron) now lives in a
// RON file per map instead of being (re)computed at every Startup, so
// F2/F9 hand-tuning + `docs/maps/*` measurement changes both land the
// same way arena walls already do.
//
// Rotation is stored as raw quaternion components (qx,qy,qz,qw), NOT
// XYZ euler degrees like ArenaLayoutRow — map1/item_gallery build several
// rotations as a COMPOUND product (ramp_panel: `ramp_rotation *
// Quat::from_rotation_x(-FRAC_PI_2)`) and/or sit at an exact 90° pitch
// (every floor_panel/ramp_panel), both classic gimbal-lock territory for
// a 3-angle euler round-trip. Raw quaternion components have no such
// failure mode — the tradeoff is a wall/floor's rotation isn't eyeball-
// editable by hand the way a single-axis arena wall's one euler angle
// is, but these RON files are meant to be captured by `regenerate_level_
// piece_ron` (crates/shared/src/level_piece.rs's own test) reading back
// off the parametric Rust builders (map1::map1_level_pieces_computed/
// item_gallery::item_gallery_level_pieces_computed) whenever a `docs/
// maps/*` measurement changes, not hand-typed.
pub type BoxLayoutRow = (f32, f32, f32, f32, f32, f32, f32, f32, f32, f32, f32, f32, f32, f32);
// (tx, ty, tz, qx, qy, qz, qw, sx, sy, sz, color_r, color_g, color_b, roughness)

pub type PanelLayoutRow = (f32, f32, f32, f32, f32, f32, f32, f32, f32, f32, f32, f32, f32, Option<f32>, bool);
// (tx, ty, tz, qx, qy, qz, qw, width, height, color_r, color_g, color_b, roughness, collider_thickness, collider_offset)

pub fn box_piece_to_row(piece: &BoxPiece) -> BoxLayoutRow {
    let &(t, r, s, c, roughness) = piece;
    (t.x, t.y, t.z, r.x, r.y, r.z, r.w, s.x, s.y, s.z, c[0], c[1], c[2], roughness)
}

fn row_to_box_piece(row: &BoxLayoutRow) -> BoxPiece {
    let &(tx, ty, tz, qx, qy, qz, qw, sx, sy, sz, cr, cg, cb, roughness) = row;
    (Vec3::new(tx, ty, tz), Quat::from_xyzw(qx, qy, qz, qw), Vec3::new(sx, sy, sz), [cr, cg, cb], roughness)
}

pub fn panel_piece_to_row(piece: &PanelPiece) -> PanelLayoutRow {
    let &(t, r, width, height, c, roughness, collider_thickness, collider_offset) = piece;
    (t.x, t.y, t.z, r.x, r.y, r.z, r.w, width, height, c[0], c[1], c[2], roughness, collider_thickness, collider_offset)
}

fn row_to_panel_piece(row: &PanelLayoutRow) -> PanelPiece {
    let &(tx, ty, tz, qx, qy, qz, qw, width, height, cr, cg, cb, roughness, collider_thickness, collider_offset) = row;
    (Vec3::new(tx, ty, tz), Quat::from_xyzw(qx, qy, qz, qw), width, height, [cr, cg, cb], roughness, collider_thickness, collider_offset)
}

/// Renders a `(Vec<BoxPiece>, Vec<PanelPiece>)` pair to the RON text a
/// `map<N>_level_pieces.ron` / `item_gallery_level_pieces.ron` file holds —
/// used by `regenerate_level_piece_ron` (this module's own test) to
/// (re)capture a map's parametric Rust geometry into its RON file.
pub fn level_pieces_to_ron(pieces: &(Vec<BoxPiece>, Vec<PanelPiece>)) -> String {
    let box_rows: Vec<BoxLayoutRow> = pieces.0.iter().map(box_piece_to_row).collect();
    let panel_rows: Vec<PanelLayoutRow> = pieces.1.iter().map(panel_piece_to_row).collect();
    ron::ser::to_string_pretty(&(box_rows, panel_rows), ron::ser::PrettyConfig::default().struct_names(false))
        .expect("serializing level pieces to RON")
}

/// Parses a `map<N>_level_pieces.ron` / `item_gallery_level_pieces.ron`
/// file's text back into `(Vec<BoxPiece>, Vec<PanelPiece>)` — the actual
/// runtime path `map1_level_pieces()`/`item_gallery_level_pieces()` (their
/// own doc comments) call every Startup now, replacing what used to be a
/// from-scratch parametric computation every time.
pub fn level_pieces_from_ron(text: &str) -> (Vec<BoxPiece>, Vec<PanelPiece>) {
    let (box_rows, panel_rows): (Vec<BoxLayoutRow>, Vec<PanelLayoutRow>) =
        ron::de::from_str(text).unwrap_or_else(|e| panic!("failed to parse level pieces RON: {e}"));
    (box_rows.iter().map(row_to_box_piece).collect(), panel_rows.iter().map(row_to_panel_piece).collect())
}

/// (translation, rotation, full-size, color, roughness) — 1 solid
/// LevelBox worth of data, before it's actually spawned. Vec'd up by
/// map1_level_pieces/item_gallery_level_pieces so the caller (client:
/// spawn LevelBox+LevelTransform; server: spawn Collider only) decides
/// what to attach, without duplicating the geometry math itself.
pub type BoxPiece = (Vec3, Quat, Vec3, [f32; 3], f32);

/// (translation, rotation, width, height, color, roughness,
/// collider_thickness, collider_offset) — same idea as BoxPiece, for a
/// LevelPanel. See LevelPanel's own doc comment for what the last 2
/// fields mean.
pub type PanelPiece = (Vec3, Quat, f32, f32, [f32; 3], f32, Option<f32>, bool);

/// Tường đứng trục X hoặc Z — mô tả bằng (center, size_x, size_z) như
/// garage_walls()/lobby_walls()/office_walls()/f3_walls() vẫn trả về —
/// thành 1 LevelPanel đứng đúng hướng: tự nhận diện trục nào là chiều
/// dài tường (trục lớn hơn) để chọn giữ nguyên hướng mặc định của
/// Rectangle (chạy dọc X) hay xoay 90° quanh Y (chạy dọc Z); trục còn
/// lại (mỏng) trở thành độ dày collider — vẫn chặn người chơi y hệt khối
/// hộp cũ, chỉ nhẹ hơn khi vẽ (1 mặt thay vì 6 mặt).
///
/// `translation` = `center` NGUYÊN VẸN, không dịch mặt hiển thị (khác
/// floor_panel/ramp_panel bên dưới) — mỗi tường chỉ có ĐÚNG 1 hướng "vào
/// trong phòng" cần khít với sàn, nhưng hướng đó khác nhau tùy tường
/// (Bắc/Nam/Đông/Tây), và hàm này không biết tường đang xây là hướng
/// nào. Từng thử dịch tất cả tường cùng 1 hướng cục bộ +Z cho "đều tay"
/// — hoá ra làm tường Nam/Đông trôi ra khỏi mép sàn (bug "chưa khít" đã
/// báo ở B1) vì hướng đó với chúng lại là RA NGOÀI thay vì vào trong.
/// Giữ nguyên tâm là lựa chọn AN TOÀN chung cho mọi hướng.
pub fn wall_panel(center: Vec3, size_x: f32, size_z: f32, wall_height: f32, color: [f32; 3], roughness: f32) -> PanelPiece {
    if size_x >= size_z {
        (center, Quat::IDENTITY, size_x, wall_height, color, roughness, Some(size_z), false)
    } else {
        (center, Quat::from_rotation_y(std::f32::consts::FRAC_PI_2), size_z, wall_height, color, roughness, Some(size_x), false)
    }
}

/// Như wall_panel nhưng cho tường đã có sẵn chiều cao riêng (size_y) —
/// dùng cho ramp_guard_walls()/*_room_walls() (4-tuple, đã có đúng
/// (center, size_x, size_y, size_z)).
pub fn wall_panel4(center: Vec3, size_x: f32, size_y: f32, size_z: f32, color: [f32; 3], roughness: f32) -> PanelPiece {
    if size_x >= size_z {
        (center, Quat::IDENTITY, size_x, size_y, color, roughness, Some(size_z), false)
    } else {
        (center, Quat::from_rotation_y(std::f32::consts::FRAC_PI_2), size_z, size_y, color, roughness, Some(size_x), false)
    }
}

/// 1 sàn nằm ngang — Rectangle mặc định dựng đứng (mặt phẳng XY) nên xoay
/// -90° quanh X để nó nằm PHẲNG xuống sàn (mặt phẳng XZ, ngửa lên +Y),
/// giống vệt kẻ Khu A/Khu B đã làm.
///
/// Khác wall_panel: `translation` dịch lên đúng MẶT TRÊN sàn (`center +
/// rotation * (0,0,thickness/2)`) thay vì giữ tâm độ dày — sàn thì "lên"
/// luôn là 1 hướng duy nhất (không như tường có 4 hướng khác nhau), nên
/// dịch an toàn, không lặp lại vấn đề của wall_panel. Collider bù lại
/// đúng nửa độ dày đó (collider_offset=true) nên vẫn nằm đúng tâm
/// `center` gốc.
pub fn floor_panel(center: Vec3, size_x: f32, size_z: f32, thickness: f32, color: [f32; 3], roughness: f32) -> PanelPiece {
    let rotation = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
    let translation = center + rotation * Vec3::new(0.0, 0.0, thickness / 2.0);
    (translation, rotation, size_x, size_z, color, roughness, Some(thickness), true)
}

/// 1 mặt dốc — ramp_between() trả về `rotation` vốn nghiêng 1 khối hộp
/// (mặc định NẰM NGANG) theo đúng góc dốc; ghép thêm phép xoay "nằm
/// ngang" của floor_panel TRƯỚC rotation đó để 1 tấm Rectangle (mặc định
/// dựng đứng) có cùng hiệu ứng cuối cùng như khối hộp cũ. Cùng lý do với
/// floor_panel — "lên trên mặt dốc" là 1 hướng duy nhất — nên cũng dịch
/// translation lên mặt + collider_offset=true.
pub fn ramp_panel(center: Vec3, ramp_rotation: Quat, width: f32, thickness: f32, length: f32, color: [f32; 3], roughness: f32) -> PanelPiece {
    let rotation = ramp_rotation * Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
    let translation = center + rotation * Vec3::new(0.0, 0.0, thickness / 2.0);
    (translation, rotation, width, length, color, roughness, Some(thickness), true)
}

/// 1 khối hộp tĩnh trong map (tường, sàn, dốc, bàn, vật cản...) — dữ liệu
/// đủ để dựng cả phần nhìn thấy được (mesh + material) lẫn collider,
/// không cần biết gì thêm về "đây là tầng nào/loại gì" — xem doc comment
/// đầu file.
#[derive(Component, Clone, Copy, Serialize, Deserialize)]
pub struct LevelBox {
    /// Kích thước đầy đủ (không phải half-extent) — khớp quy ước
    /// Cuboid::new(x,y,z)/Collider::cuboid(x,y,z) đã dùng xuyên suốt map1.
    pub size: Vec3,
    /// Màu phẳng (chưa texture) — linear sRGB 0..1, khớp tham số
    /// Color::srgb(r,g,b).
    pub color: [f32; 3],
    pub roughness: f32,
}

impl LevelBox {
    pub const fn new(size: Vec3, color: [f32; 3], roughness: f32) -> Self {
        Self { size, color, roughness }
    }
}

/// 1 tấm phẳng mỏng trong map (tường, sàn, dốc, vệt kẻ sàn, biển báo...)
/// — nhẹ hơn LevelBox vì phần NHÌN THẤY chỉ là 1 mặt chữ nhật (2 tam
/// giác) thay vì cả 6 mặt của 1 khối hộp. Collider là TÙY CHỌN qua
/// `collider_thickness`:
/// - `None` — thuần trang trí, đi xuyên qua được (vệt kẻ sàn, biển báo).
/// - `Some(thickness)` — vẫn chặn người chơi như 1 bức tường/sàn thật,
///   chỉ khác LevelBox ở chỗ MESH hiển thị phẳng (nhẹ hơn khi vẽ); dùng
///   cho tường/sàn/dốc — những thứ chỉ cần nhìn thấy từ 1 phía (không ai
///   đứng trong lòng tường để thấy mặt kia).
///
/// `collider_offset` chỉ có ý nghĩa khi `collider_thickness` là `Some`:
/// - `false` (tường) — `LevelTransform`'s translation ĐÃ LÀ tâm collider
///   (giống hệt tâm 1 LevelBox trước đây) — không cần bù gì thêm. Bức
///   tường nào cũng chỉ có 1 mặt "trong nhà" thật sự cần khít với sàn,
///   nhưng KHÔNG có 1 hướng lùi vào chung đúng cho mọi tường (tường Bắc
///   cần lùi khác hướng tường Nam) — nên tường ĐANG không tự dịch mặt
///   hiển thị, tránh lặp lại đúng bug đã báo ("Nam chưa khít") theo
///   hướng khác.
/// - `true` (sàn/dốc) — translation là MẶT NHÌN THẤY (mặt trên sàn/mặt
///   dốc), không phải tâm — vì "lên" luôn là 1 hướng duy nhất, không như
///   tường, nên dịch được an toàn. Collider cần lùi lại
///   `-collider_thickness/2` theo trục Z cục bộ để về đúng tâm gốc.
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
    /// Tấm trang trí thuần — không cản đường.
    pub const fn new(width: f32, height: f32, color: [f32; 3], roughness: f32) -> Self {
        Self { width, height, color, roughness, collider_thickness: None, collider_offset: false }
    }

    /// Tấm có cản đường, translation = tâm collider luôn (tường) — dùng
    /// thay cho LevelBox.
    pub const fn solid(width: f32, height: f32, color: [f32; 3], roughness: f32, collider_thickness: f32) -> Self {
        Self { width, height, color, roughness, collider_thickness: Some(collider_thickness), collider_offset: false }
    }

    /// Như `solid`, nhưng translation = mặt nhìn thấy (sàn/dốc) — collider
    /// tự lùi lại đúng tâm, xem doc comment của field `collider_offset`.
    pub const fn solid_surface(width: f32, height: f32, color: [f32; 3], roughness: f32, collider_thickness: f32) -> Self {
        Self { width, height, color, roughness, collider_thickness: Some(collider_thickness), collider_offset: true }
    }
}

/// Vị trí + xoay của 1 LevelBox — component RIÊNG thay vì replicate thẳng
/// `Transform` có sẵn, để không đụng tới Player/Zombie (2 loại entity đã
/// có Transform + Replicated từ trước, dùng NetTransform để nội suy
/// riêng — replicate thêm Transform thẳng sẽ đá nhau với hệ đó). LevelBox
/// tĩnh, không bao giờ đổi sau khi spawn, nên không cần nội suy gì cả —
/// client chỉ đọc 1 lần lúc entity xuất hiện.
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

// --- Regenerate map1_level_pieces.ron / item_gallery_level_pieces.ron --
//
// `cargo test -p shared regenerate_level_piece_ron -- --ignored --nocapture`
// re-derives both RON files from the parametric Rust builders
// (map1::map1_level_pieces_computed / item_gallery::item_gallery_level_
// pieces_computed) and overwrites the checked-in RON — run this after
// changing a measurement constant (docs/maps/*) or a wall/floor/ramp
// helper in map1/mod.rs's or item_gallery.rs's *_computed function.
// `--ignored`: never runs as part of a normal `cargo test` (it WRITES to
// the repo's asset files — a side effect, not a check), and CARGO_
// MANIFEST_DIR-relative like every other RON path in this crate (see
// read_ron_data_file!'s own doc comment), so it works from any CWD.
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
        write_ron("map1_level_pieces.ron", &crate::map1::map1_level_pieces_computed());
        write_ron("item_gallery_level_pieces.ron", &crate::item_gallery::item_gallery_level_pieces_computed());
    }
}

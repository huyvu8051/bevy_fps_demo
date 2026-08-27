// --- Replicated level geometry ---
// Static level pieces (walls, floors, ramps, cover boxes...) used to be
// built independently by both the server (collider-only) and the client
// (mesh + collider), each reading the exact same shared::map1 Rust
// constants — correct, but it means the CLIENT needs a rebuild any time
// the map's Rust code changes. That's a real problem for testing on a
// phone over a network: editing crates/shared on a dev Mac did nothing
// for an already-installed iOS build until you rebuilt + reinstalled it
// through Xcode.
//
// LevelBox flips that around: the server is now the only process that
// decides what a level looks like. It spawns one Replicated entity per
// piece (see crates/server's spawn_map1_level_pieces), and any already-
// connected client — desktop or iOS, the normal Bevy/wgpu renderer or
// the native Metal one — builds its own local mesh + collider purely by
// reacting to LevelBox/LevelPanel showing up over the network (see
// crates/client's spawn_level_box_visuals/spawn_level_panel_visuals).
// Editing a map now means editing crates/shared/src/map1's Rust code and
// restarting the SERVER — no client rebuild, on any platform, ever
// needed for a map-only change.
//
// Two shapes so far — LevelBox (a solid cuboid, real collider) and
// LevelPanel (a thin flat rectangle, no collider) — not everything in a
// level is a box; a floor stripe or a sign doesn't need volume.
//
// A real "edit a .ron file, no Mac rebuild either" version is a natural
// next step (the server would load the file at Startup instead of
// calling map1's Rust builder functions) — not done yet; today the data
// source is still Rust code, just spawned once on the server and
// replicated out instead of duplicated per-process.

use bevy::prelude::{Component, Quat, Vec3};
use serde::{Deserialize, Serialize};

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
#[derive(Component, Clone, Copy, Serialize, Deserialize)]
pub struct LevelPanel {
    pub width: f32,
    pub height: f32,
    pub color: [f32; 3],
    pub roughness: f32,
    pub collider_thickness: Option<f32>,
}

impl LevelPanel {
    /// Tấm trang trí thuần — không cản đường.
    pub const fn new(width: f32, height: f32, color: [f32; 3], roughness: f32) -> Self {
        Self { width, height, color, roughness, collider_thickness: None }
    }

    /// Tấm có cản đường — dùng cho tường/sàn/dốc thay cho LevelBox.
    pub const fn solid(width: f32, height: f32, color: [f32; 3], roughness: f32, collider_thickness: f32) -> Self {
        Self { width, height, color, roughness, collider_thickness: Some(collider_thickness) }
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

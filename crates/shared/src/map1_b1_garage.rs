// --- Map 1 "Tòa nhà văn phòng" — Tầng B1 (Gara để xe) blockout ---
// Greybox test level cho phần đầu tiên của Map 1 thật (xem
// docs/maps/map01_highrise.md cho mockup ASCII đầy đủ) — chỉ có tường +
// vật cản tạm (khối hộp thay cho xe hơi đậu), CHƯA có texture/model thật.
// Cùng "2 phía phải khớp nhau" convention như ARENA_HALF_SIZE/
// NAVMESH_COLUMN_OBSTACLES (arena.rs): dữ liệu sống ở đây, crates/client's
// spawn_map1_b1_garage (mesh + collider) và crates/server's
// spawn_map1_b1_garage_colliders (collider-only) cùng đọc y hệt mảng này,
// nên 2 process không thể lệch hình học với nhau.

use bevy::prelude::Vec3;

use crate::arena::{ARENA_WALL_HEIGHT, ARENA_WALL_THICKNESS};

// Kích thước sàn gara: 28m (X) x 20m (Z) — đủ rộng cho layout 2 khu vực
// (A gần lối vào, B sâu hơn) theo đúng mockup map01_highrise.md.
pub const GARAGE_HALF_SIZE_X: f32 = 14.0;
pub const GARAGE_HALF_SIZE_Z: f32 = 10.0;

// Điểm spawn người chơi (Khu A trong mockup) — góc Tây Bắc của gara, gần
// lối vào. Y=3.0 giống PLAYER_SPAWN gốc (player.rs) — thả rơi tự nhiên
// xuống sàn thật thay vì đoán chiều cao chính xác.
pub const GARAGE_PLAYER_SPAWN: Vec3 = Vec3::new(-11.0, 3.0, -7.0);

// "Xe hơi đậu" — vật cản tạm dùng khối hộp thay mesh xe thật (chưa có
// asset). (x, z) là tâm mỗi khối, kích thước dùng chung CAR_SIZE bên dưới.
// 4 hàng x 4 cột = 16 khối, trải đều 2 khu vực A (2 hàng đầu, gần spawn)
// và B (2 hàng sau, sâu hơn) đúng bố cục mockup.
pub const GARAGE_CAR_SIZE: Vec3 = Vec3::new(2.2, 1.4, 4.2); // rộng x cao x dài
pub const GARAGE_CAR_POSITIONS: [(f32, f32); 16] = [
    // Khu A — 2 hàng gần lối vào/spawn
    (-8.0, -3.0), (-3.0, -3.0), (2.0, -3.0), (7.0, -3.0),
    (-8.0, 0.0), (-3.0, 0.0), (2.0, 0.0), (7.0, 0.0),
    // Khu B — 2 hàng sâu hơn, tối hơn (theo lore mockup)
    (-8.0, 3.0), (-3.0, 3.0), (2.0, 3.0), (7.0, 3.0),
    (-8.0, 6.0), (-3.0, 6.0), (2.0, 6.0), (7.0, 6.0),
];

/// (center, size_x, size_z) cho 4 bức tường quây quanh gara — cùng công
/// thức "wall_span = kích thước sàn + độ dày, để 2 tường vuông góc chồng
/// nhẹ lên nhau ở góc, không hở khe" mà spawn_test_arena/
/// spawn_arena_colliders (crates/client, crates/server) đã dùng cho arena
/// chính. Tường phía Nam (+Z) CHỪA 1 khoảng hở ở giữa (không phải 1 khối
/// liền) — đây là lối lên Sảnh chính (tầng G), CHƯA xây (chỉ test B1),
/// nên để trống thay vì 1 bức tường bịt kín — tránh nhầm là ngõ cụt.
pub fn garage_walls() -> [(Vec3, f32, f32); 5] {
    let span_x = GARAGE_HALF_SIZE_X * 2.0 + ARENA_WALL_THICKNESS;
    let span_z = GARAGE_HALF_SIZE_Z * 2.0 + ARENA_WALL_THICKNESS;
    let half_y = ARENA_WALL_HEIGHT / 2.0;
    // Lối lên Sảnh rộng 4m, đặt giữa tường Nam — 2 đoạn tường 2 bên lối đó
    // cộng lại + khoảng hở đúng bằng span_x, không phải 1 tường liền.
    let gap_half = 2.0;
    let south_wall_seg = span_x / 2.0 - gap_half;
    [
        // Bắc (-Z) — liền mạch, không có lối ra.
        (Vec3::new(0.0, half_y, -GARAGE_HALF_SIZE_Z), span_x, ARENA_WALL_THICKNESS),
        // Nam (+Z) — 2 đoạn, chừa khoảng hở ở giữa (lối lên Sảnh, chưa xây).
        (Vec3::new(-(gap_half + south_wall_seg / 2.0), half_y, GARAGE_HALF_SIZE_Z), south_wall_seg, ARENA_WALL_THICKNESS),
        (Vec3::new(gap_half + south_wall_seg / 2.0, half_y, GARAGE_HALF_SIZE_Z), south_wall_seg, ARENA_WALL_THICKNESS),
        // Tây (-X) / Đông (+X)
        (Vec3::new(-GARAGE_HALF_SIZE_X, half_y, 0.0), ARENA_WALL_THICKNESS, span_z),
        (Vec3::new(GARAGE_HALF_SIZE_X, half_y, 0.0), ARENA_WALL_THICKNESS, span_z),
    ]
}

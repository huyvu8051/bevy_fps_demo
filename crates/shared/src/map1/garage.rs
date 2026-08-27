// --- Map 1 "Tòa nhà văn phòng" — Tầng B1 (Gara để xe) ---
// Greybox test level cho phần đầu tiên của Map 1 thật (xem
// docs/maps/map01_highrise.md cho mockup ASCII đầy đủ) — chỉ có tường +
// vật cản tạm (khối hộp thay cho xe hơi đậu), CHƯA có texture/model thật.
// Đây là tầng ĐÁY của Map 1 (chưa có tầng nào bên dưới) nên dùng
// FloorLevel::walls_ground_floor() — chỉ chừa khoảng hở lối lên Sảnh G ở
// cạnh Nam. crates/client's spawn_map1_b1_garage (mesh + collider) và
// crates/server's spawn_map1_b1_garage_colliders (collider-only) cùng đọc
// y hệt các hằng số/hàm này, nên 2 process không thể lệch hình học.

use bevy::prelude::Vec3;

use super::FloorLevel;

// Kích thước sàn gara: 28m (X) x 20m (Z) — đủ rộng cho layout 2 khu vực
// (A gần lối vào, B sâu hơn) theo đúng mockup map01_highrise.md.
pub const GARAGE_HALF_SIZE_X: f32 = 14.0;
pub const GARAGE_HALF_SIZE_Z: f32 = 10.0;

// Gara là tầng đáy của Map 1 — cao độ 0, tâm đặt tại gốc thế giới (Z=0)
// nên north_wall_z = -GARAGE_HALF_SIZE_Z.
pub const GARAGE_FLOOR: FloorLevel = FloorLevel::new(
    GARAGE_HALF_SIZE_X,
    GARAGE_HALF_SIZE_Z,
    crate::arena::ARENA_WALL_HEIGHT,
    0.0,
    -GARAGE_HALF_SIZE_Z,
);

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

/// (center, size_x, size_z) cho 4 bức tường quây quanh gara.
pub fn garage_walls() -> Vec<(Vec3, f32, f32)> {
    GARAGE_FLOOR.walls_ground_floor()
}

// --- Map 1 "Tòa nhà văn phòng" — Tầng F2 (Văn phòng tầng 2) ---
// Greybox test level nối tiếp lobby.rs (xem docs/maps/map01_highrise.md,
// mục "Tầng F2 — Văn phòng tầng 2" cho mockup ASCII đầy đủ) — chỉ tường +
// vật cản tạm (bàn làm việc, 2 phòng nhỏ), CHƯA có texture/model thật.
//
// F2 giờ là tầng TRUNG GIAN (có F3 bên trên, xem f3.rs) nên dùng
// FloorLevel::walls_mid_floor() — chừa khoảng hở cả 2 cạnh Bắc (từ Sảnh
// G lên) và Nam (lên F3).
//
// Đơn giản hoá: "Văn phòng riêng (cửa kính)" và "Phòng tủ đồ nhân viên"
// trong mockup đều là phòng kín có cửa — bản test này dựng mỗi phòng
// bằng 3 bức tường (chừa hẳn 1 mặt mở làm lối vào) thay vì mô hình cửa
// kính/cửa gỗ thật, đủ để test đi lại + cover, chưa cần đúng hình dạng
// cửa.

use bevy::prelude::{Quat, Vec3};

use super::lobby::LOBBY_FLOOR;
use super::{ramp_between, ramp_guard_walls_between, small_room_walls, FloorLevel, RoomDoor, FLOOR_HEIGHT};

// Cao độ sàn F2 — 1 FLOOR_HEIGHT nữa so với Sảnh G (2 * FLOOR_HEIGHT so
// với Gara B1 ở đáy).
pub const F2_ELEVATION: f32 = FLOOR_HEIGHT * 2.0;

// Trần văn phòng thấp hơn Sảnh (không gian cubicle bình thường, không
// "mở, trần cao" như Sảnh chính).
pub const F2_WALL_HEIGHT: f32 = 4.0;

// Kích thước sàn F2: 30m (X) x 28m (Z) — đủ chỗ cho dãy cubicle + 2
// phòng nhỏ + khu vực trước cầu thang lên F3.
pub const F2_HALF_SIZE_X: f32 = 15.0;
pub const F2_HALF_SIZE_Z: f32 = 14.0;

pub const F2_FLOOR: FloorLevel =
    FloorLevel::new(F2_HALF_SIZE_X, F2_HALF_SIZE_Z, F2_WALL_HEIGHT, F2_ELEVATION, LOBBY_FLOOR.next_north_wall_z());

pub const F2_NORTH_WALL_Z: f32 = F2_FLOOR.north_wall_z;
pub const F2_CENTER_Z: f32 = F2_FLOOR.center_z();
pub const F2_SOUTH_WALL_Z: f32 = F2_FLOOR.south_wall_z();

// Điểm player có thể đi tới ngay sau khi lên hết dốc F2 — dùng để test
// riêng tầng này mà không cần đi bộ hết các tầng dưới trước.
pub const F2_TEST_SPAWN: Vec3 = Vec3::new(0.0, F2_ELEVATION + 3.0, F2_NORTH_WALL_Z + 2.0);

// "Dãy bàn làm việc (cubicle mở)" — 5 bàn thấp, dễ bắn qua đầu, trải
// đều theo trục X ngay khu vực đầu tiên sau khi lên dốc.
pub const F2_DESK_SIZE: Vec3 = Vec3::new(1.8, 0.9, 1.4);
pub const F2_DESK_POSITIONS: [(f32, f32); 5] = [
    (-10.0, F2_NORTH_WALL_Z + 5.0),
    (-5.0, F2_NORTH_WALL_Z + 5.0),
    (0.0, F2_NORTH_WALL_Z + 5.0),
    (5.0, F2_NORTH_WALL_Z + 5.0),
    (10.0, F2_NORTH_WALL_Z + 5.0),
];

// "Văn phòng riêng (cửa kính)" — phòng kín phía Tây, mở về hướng Đông
// (vào khu cubicle). L1 (audio log) theo mockup nằm trong phòng này.
pub const F2_OFFICE_ROOM_HALF: Vec3 = Vec3::new(3.0, F2_WALL_HEIGHT / 2.0, 2.5);
pub const F2_OFFICE_ROOM_CENTER: (f32, f32) = (-11.0, F2_NORTH_WALL_Z + 11.0);

// "Phòng tủ đồ nhân viên" — phòng kín phía Đông, mở về hướng Tây (vào
// khu cubicle). Ammo pickup theo mockup nằm trong phòng này.
pub const F2_LOCKER_ROOM_HALF: Vec3 = Vec3::new(3.0, F2_WALL_HEIGHT / 2.0, 2.5);
pub const F2_LOCKER_ROOM_CENTER: (f32, f32) = (11.0, F2_NORTH_WALL_Z + 11.0);

// "Chân cầu thang xoắn ốc" lên F3 — trụ placeholder gần cuối F2 (Med kit
// theo mockup nằm ngay trước điểm này). Lệch sang Đông (X=6) để không
// chặn khoảng hở tường Nam (X trong [-2,2]) — lối thật lên F3 giờ là
// đường dốc ở đó (xem f3.rs's ramp3_geometry()); trụ chỉ là mốc trực
// quan đánh dấu "đây là chỗ lên tầng trên".
pub const F2_STAIRCASE_RADIUS: f32 = 2.2;
pub const F2_STAIRCASE_HEIGHT: f32 = F2_WALL_HEIGHT;
pub const F2_STAIRCASE_POS: (f32, f32) = (6.0, F2_SOUTH_WALL_Z - 3.0);

/// Đường dốc nối Sảnh G lên F2.
pub fn ramp2_geometry() -> (Vec3, Quat, Vec3) {
    ramp_between(&LOBBY_FLOOR, &F2_FLOOR)
}

/// 2 tường chắn 2 bên đường dốc Sảnh→F2.
pub fn ramp2_guard_walls() -> [(Vec3, f32, f32, f32); 2] {
    ramp_guard_walls_between(&LOBBY_FLOOR, &F2_FLOOR)
}

/// (center, size_x, size_z) cho 6 bức tường quây quanh F2 — chừa khoảng
/// hở cả tường Bắc (giáp chân dốc từ Sảnh) và tường Nam (giáp chân dốc
/// lên F3).
pub fn office_walls() -> Vec<(Vec3, f32, f32)> {
    F2_FLOOR.walls_mid_floor()
}

/// Tường phòng "Văn phòng riêng (cửa kính)" — mở về hướng Đông (vào khu
/// cubicle).
pub fn office_room_walls() -> [(Vec3, f32, f32, f32); 3] {
    small_room_walls(F2_OFFICE_ROOM_CENTER, F2_OFFICE_ROOM_HALF, F2_ELEVATION + F2_OFFICE_ROOM_HALF.y, RoomDoor::East)
}

/// Tường phòng "Phòng tủ đồ nhân viên" — mở về hướng Tây (vào khu
/// cubicle).
pub fn locker_room_walls() -> [(Vec3, f32, f32, f32); 3] {
    small_room_walls(F2_LOCKER_ROOM_CENTER, F2_LOCKER_ROOM_HALF, F2_ELEVATION + F2_LOCKER_ROOM_HALF.y, RoomDoor::West)
}

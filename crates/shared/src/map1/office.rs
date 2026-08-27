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
use super::{small_room_walls, simple_stairwell, FloorLevel, RoomDoor, FLOOR_HEIGHT, TOWER_SHAFT_X, TOWER_SHAFT_Z_OFFSET};

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

// F2 giờ CHỒNG THẬT ngay trên Sảnh G — dùng chung north_wall_z với
// LOBBY_FLOOR (không phải LOBBY_FLOOR.next_north_wall_z() nữa, cái đó
// từng đẩy F2 nằm xéo xa hơn về phía Nam) nên toàn bộ nội thất F2 vẫn
// giữ nguyên toạ độ TƯƠNG ĐỐI như cũ (mọi hằng số dưới đây đều tính từ
// F2_NORTH_WALL_Z, không đổi công thức) — chỉ có toạ độ TUYỆT ĐỐI trong
// thế giới là chồng lên G thay vì nằm xa hơn.
pub const F2_FLOOR: FloorLevel = FloorLevel::new(F2_HALF_SIZE_X, F2_HALF_SIZE_Z, F2_WALL_HEIGHT, F2_ELEVATION, LOBBY_FLOOR.north_wall_z);

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

// Tâm X/Z giếng thang G→F2 — 1 đoạn của giếng thang chung xuyên suốt
// G→F2→F3 (xem TOWER_SHAFT_X/TOWER_SHAFT_Z_OFFSET's doc comment ở
// mod.rs cho lý do chọn đúng vị trí này, tránh mọi phòng nội thất).
pub const F2_SHAFT_Z: f32 = F2_NORTH_WALL_Z + TOWER_SHAFT_Z_OFFSET;

/// Cầu thang đơn giản nối Sảnh G lên F2 — thay cho đường dốc nằm xéo cũ
/// (ramp2_geometry/ramp2_guard_walls, đã xoá) vì F2 giờ chồng thật ngay
/// trên G. Xem simple_stairwell's doc comment.
pub fn stairwell_g_to_f2() -> Vec<(Vec3, Quat, Vec3)> {
    simple_stairwell(TOWER_SHAFT_X, F2_SHAFT_Z, &LOBBY_FLOOR, &F2_FLOOR)
}

/// (center, size_x, size_z) cho 6 bức tường quây KÍN F2 — không còn
/// khoảng hở tường nào (cả lối vào từ G lẫn lối lên F3 giờ đều là giếng
/// thang xuyên sàn, không đi qua tường).
pub fn office_walls() -> Vec<(Vec3, f32, f32)> {
    F2_FLOOR.walls_enclosed()
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

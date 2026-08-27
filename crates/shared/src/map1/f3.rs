// --- Map 1 "Tòa nhà văn phòng" — Tầng F3 (Văn phòng tầng 3) ---
// Greybox test level nối tiếp office.rs (xem docs/maps/map01_highrise.md,
// mục "Tầng F3 — Văn phòng tầng 3" cho mockup ASCII đầy đủ) — chỉ tường +
// vật cản tạm (phòng họp kính, 2 văn phòng bên hông), CHƯA có texture/
// model thật.
//
// F3 hiện là tầng CAO NHẤT đã xây (F4 chưa xây) nên dùng
// FloorLevel::walls_top_floor() — chỉ chừa khoảng hở cạnh Bắc (từ F2
// lên); khi nào xây F4, đổi hàm này sang walls_mid_floor() và thêm
// ramp_between(&F3_FLOOR, &F4_FLOOR).
//
// Đơn giản hoá: mockup mô tả "Phòng họp kính (đã nứt sẵn)" với trigger
// làm vỡ nốt mảng kính còn lại → kích hoạt báo cháy + horde nhiều hướng
// (mục 17's mission-state-machine sẽ xử lý phần trigger/spawn thật khi
// tới lượt gameplay logic — bản test greybox này chỉ dựng ĐÚNG hình dạng
// không gian: phòng họp kín (mở về hành lang), hành lang chính dài, và 2
// văn phòng bên hông cũng kín mở về hành lang — không có logic
// X/trigger/horde thật, chỉ có hình học để test đi lại).

use bevy::prelude::{Quat, Vec3};

use super::office::F2_FLOOR;
use super::{small_room_walls, simple_stairwell, FloorLevel, RoomDoor, FLOOR_HEIGHT, TOWER_SHAFT_X, TOWER_SHAFT_Z_OFFSET};

// Cao độ sàn F3 — 1 FLOOR_HEIGHT nữa so với F2 (3 * FLOOR_HEIGHT so với
// Gara B1 ở đáy).
pub const F3_ELEVATION: f32 = FLOOR_HEIGHT * 3.0;

// Trần văn phòng — giống F2 (không gian cubicle/hành lang bình thường).
pub const F3_WALL_HEIGHT: f32 = 4.0;

// Kích thước sàn F3: 30m (X) x 34m (Z) — dài hơn F2 hẳn để chứa "hành
// lang chính, dài, mở" theo đúng mockup (đây là tầng sustain-peak, cần
// không gian đủ rộng cho horde áp sát từ nhiều hướng).
pub const F3_HALF_SIZE_X: f32 = 15.0;
pub const F3_HALF_SIZE_Z: f32 = 17.0;

// F3 giờ CHỒNG THẬT ngay trên F2 (cùng north_wall_z với F2_FLOOR, không
// phải F2_FLOOR.next_north_wall_z() nữa) — cùng lý do với F2_FLOOR ở
// office.rs, xem doc comment ở đó.
pub const F3_FLOOR: FloorLevel = FloorLevel::new(F3_HALF_SIZE_X, F3_HALF_SIZE_Z, F3_WALL_HEIGHT, F3_ELEVATION, F2_FLOOR.north_wall_z);

pub const F3_NORTH_WALL_Z: f32 = F3_FLOOR.north_wall_z;
pub const F3_CENTER_Z: f32 = F3_FLOOR.center_z();
pub const F3_SOUTH_WALL_Z: f32 = F3_FLOOR.south_wall_z();

// Điểm player có thể đi tới ngay sau khi lên hết dốc F3 — dùng để test
// riêng tầng này mà không cần đi bộ hết các tầng dưới trước.
pub const F3_TEST_SPAWN: Vec3 = Vec3::new(0.0, F3_ELEVATION + 3.0, F3_NORTH_WALL_Z + 2.0);

// "Phòng họp kính (đã nứt sẵn)" — phòng kín ngay đầu hành lang (gần lối
// vào từ F2), mở về hướng Nam (vào hành lang chính). Trigger X (làm vỡ
// nốt kính, kích hoạt báo cháy + horde) đánh dấu bằng F3_ALARM_TRIGGER_POS
// — CHƯA nối logic thật, chỉ ghi lại toạ độ cho lần build gameplay tới.
pub const F3_MEETING_ROOM_HALF: Vec3 = Vec3::new(4.0, F3_WALL_HEIGHT / 2.0, 3.0);
pub const F3_MEETING_ROOM_CENTER: (f32, f32) = (0.0, F3_NORTH_WALL_Z + 4.0);
pub const F3_ALARM_TRIGGER_POS: Vec3 = Vec3::new(0.0, F3_ELEVATION + 0.1, F3_NORTH_WALL_Z + 4.0);

// "Văn phòng A" / "Văn phòng B" — 2 phòng kín bên hông hành lang chính,
// mở về phía hành lang (A mở Đông, B mở Tây) — đúng bố cục "áp sát từ 3
// hướng" quanh khu vực giữa hành lang trong mockup.
pub const F3_OFFICE_A_HALF: Vec3 = Vec3::new(3.5, F3_WALL_HEIGHT / 2.0, 3.5);
pub const F3_OFFICE_A_CENTER: (f32, f32) = (-10.5, F3_NORTH_WALL_Z + 22.0);
pub const F3_OFFICE_B_HALF: Vec3 = Vec3::new(3.5, F3_WALL_HEIGHT / 2.0, 3.5);
pub const F3_OFFICE_B_CENTER: (f32, f32) = (10.5, F3_NORTH_WALL_Z + 22.0);

// Tâm X/Z giếng thang F2→F3 — cùng 1 giếng liên tục xuyên suốt
// G→F2→F3, xem TOWER_SHAFT_X/TOWER_SHAFT_Z_OFFSET's doc comment ở
// mod.rs. Vị trí này cũng chính là "chân cầu thang xoắn ốc lên F4" cho
// lần build tới (giếng vẫn tiếp tục thẳng lên, chỉ chưa có F4 để nối).
pub const F3_SHAFT_Z: f32 = F3_NORTH_WALL_Z + TOWER_SHAFT_Z_OFFSET;

/// Cầu thang đơn giản nối F2 lên F3 — thay cho đường dốc nằm xéo cũ
/// (ramp3_geometry/ramp3_guard_walls, đã xoá) vì F3 giờ chồng thật ngay
/// trên F2.
pub fn stairwell_f2_to_f3() -> Vec<(Vec3, Quat, Vec3)> {
    simple_stairwell(TOWER_SHAFT_X, F3_SHAFT_Z, &F2_FLOOR, &F3_FLOOR)
}

/// (center, size_x, size_z) cho 4 bức tường quây KÍN F3 — không còn
/// khoảng hở tường nào (lối vào từ F2 giờ là giếng thang xuyên sàn; F4
/// chưa xây nên chưa có gì để chừa hở phía Nam nữa).
pub fn f3_walls() -> Vec<(Vec3, f32, f32)> {
    F3_FLOOR.walls_enclosed()
}

/// Tường phòng họp kính — mở về hướng Nam (vào hành lang chính).
pub fn meeting_room_walls() -> [(Vec3, f32, f32, f32); 3] {
    small_room_walls(F3_MEETING_ROOM_CENTER, F3_MEETING_ROOM_HALF, F3_ELEVATION + F3_MEETING_ROOM_HALF.y, RoomDoor::South)
}

/// Tường "Văn phòng A" — mở về hướng Đông (vào hành lang chính).
pub fn office_a_walls() -> [(Vec3, f32, f32, f32); 3] {
    small_room_walls(F3_OFFICE_A_CENTER, F3_OFFICE_A_HALF, F3_ELEVATION + F3_OFFICE_A_HALF.y, RoomDoor::East)
}

/// Tường "Văn phòng B" — mở về hướng Tây (vào hành lang chính).
pub fn office_b_walls() -> [(Vec3, f32, f32, f32); 3] {
    small_room_walls(F3_OFFICE_B_CENTER, F3_OFFICE_B_HALF, F3_ELEVATION + F3_OFFICE_B_HALF.y, RoomDoor::West)
}

// --- Map 1 "Tòa nhà văn phòng" — Tầng G (Sảnh chính) ---
// Greybox test level nối tiếp garage.rs (xem docs/maps/map01_highrise.md,
// mục "Tầng G — Sảnh chính" cho mockup ASCII đầy đủ) — chỉ tường + vật
// cản tạm (quầy lễ tân, chân cầu thang xoắn ốc), CHƯA có texture/model
// thật.
//
// Sảnh G thật sự cao hơn Gara B1 (FLOOR_HEIGHT), nối bằng 1 đường dốc
// nghiêng (ramp) thay vì bậc thang vuông góc — controller di chuyển hiện
// tại (GROUND_NORMAL_MIN_Y trong player.rs) đã tự chấp nhận đi trên mặt
// dốc tới ~53°, nên ramp chạy được ngay mà không cần thêm logic
// "step-up" nào cả. Bậc thang thật (art đẹp hơn) để dành cho lúc convert
// sang mesh Blender thật — lúc đó vẫn nên giữ collision là mặt dốc như
// thế này (mesh nhìn có bậc, collider vẫn là 1 mặt phẳng nghiêng), đúng
// kiểu stair-collision phổ biến trong FPS.
//
// Sảnh là tầng TRUNG GIAN (có cả Gara bên dưới lẫn F2 bên trên) nên dùng
// FloorLevel::walls_mid_floor() — chừa khoảng hở cả 2 cạnh Bắc/Nam.

use bevy::prelude::{Quat, Vec3};

use super::garage::GARAGE_FLOOR;
use super::{ramp_between, ramp_guard_walls_between, FloorLevel, FLOOR_HEIGHT};

// Trần Sảnh cao hơn hẳn Gara ("không gian mở, trần cao" theo mockup) —
// hằng số riêng thay vì dùng chung ARENA_WALL_HEIGHT.
pub const LOBBY_WALL_HEIGHT: f32 = 8.0;

// Kích thước sàn Sảnh: 30m (X) x 24m (Z) — rộng hơn Gara, đúng tinh thần
// "không gian mở" trước khi vào nút thắt cầu thang xoắn ốc.
pub const LOBBY_HALF_SIZE_X: f32 = 15.0;
pub const LOBBY_HALF_SIZE_Z: f32 = 12.0;

pub const LOBBY_FLOOR: FloorLevel =
    FloorLevel::new(LOBBY_HALF_SIZE_X, LOBBY_HALF_SIZE_Z, LOBBY_WALL_HEIGHT, FLOOR_HEIGHT, GARAGE_FLOOR.next_north_wall_z());

pub const LOBBY_NORTH_WALL_Z: f32 = LOBBY_FLOOR.north_wall_z;
pub const LOBBY_CENTER_Z: f32 = LOBBY_FLOOR.center_z();
pub const LOBBY_SOUTH_WALL_Z: f32 = LOBBY_FLOOR.south_wall_z();

// Điểm player có thể đi tới ngay sau khi lên hết dốc — dùng để test
// riêng Sảnh G mà không cần đi bộ hết Gara + dốc trước.
pub const LOBBY_TEST_SPAWN: Vec3 = Vec3::new(0.0, FLOOR_HEIGHT + 3.0, LOBBY_NORTH_WALL_Z + 2.0);

// "Quầy lễ tân" — cover box phía Tây, gần lối vào, đặt trên mặt sàn đã
// nâng cao FLOOR_HEIGHT.
pub const LOBBY_DESK_SIZE: Vec3 = Vec3::new(4.0, 1.1, 1.6);
pub const LOBBY_DESK_POS: (f32, f32) = (-9.0, LOBBY_NORTH_WALL_Z + 4.0);

/// Đường dốc nối Gara B1 lên Sảnh G.
pub fn ramp_geometry() -> (Vec3, Quat, Vec3) {
    ramp_between(&GARAGE_FLOOR, &LOBBY_FLOOR)
}

/// 2 tường chắn 2 bên đường dốc Gara→Sảnh.
pub fn ramp_guard_walls() -> [(Vec3, f32, f32, f32); 2] {
    ramp_guard_walls_between(&GARAGE_FLOOR, &LOBBY_FLOOR)
}

/// (center, size_x, size_z) cho các bức tường quây quanh Sảnh — chừa
/// khoảng hở tường Bắc (giáp đỉnh dốc lên từ Gara, B1→G vẫn nằm xéo như
/// cũ) nhưng tường Nam giờ LIỀN MẠCH — F2 chồng thật ngay phía trên
/// (cùng tâm X/Z), lối lên F2 là giếng thang xuyên sàn (xem
/// switchback_stairwell trong mod.rs), không còn đi qua khoảng hở tường
/// Nam nữa.
pub fn lobby_walls() -> Vec<(Vec3, f32, f32)> {
    LOBBY_FLOOR.walls_gapped_north_only()
}

// --- Map 1 "Tòa nhà văn phòng" — Tầng G (Sảnh chính) blockout ---
// Greybox test level nối tiếp map1_b1_garage.rs (xem
// docs/maps/map01_highrise.md, mục "Tầng G — Sảnh chính" cho mockup ASCII
// đầy đủ) — chỉ tường + vật cản tạm (quầy lễ tân, chân cầu thang xoắn
// ốc), CHƯA có texture/model thật. Cùng convention với map1_b1_garage.rs:
// dữ liệu sống ở đây, crates/client's spawn_map1_g_lobby (mesh + collider)
// và crates/server's spawn_map1_g_lobby_colliders (collider-only) cùng
// đọc y hệt mảng này.
//
// Sảnh G thật sự cao hơn Gara B1 (FLOOR_HEIGHT), nối bằng 1 đường dốc
// nghiêng (ramp) thay vì bậc thang vuông góc — controller di chuyển hiện
// tại (GROUND_NORMAL_MIN_Y trong player.rs) đã tự chấp nhận đi trên mặt
// dốc tới ~53°, nên ramp chạy được ngay mà không cần thêm logic
// "step-up" nào cả. Bậc thang thật (art đẹp hơn) để dành cho lúc convert
// sang mesh Blender thật — lúc đó vẫn nên giữ collision là mặt dốc như
// thế này (mesh nhìn có bậc, collider vẫn là 1 mặt phẳng nghiêng), đúng
// kiểu stair-collision phổ biến trong FPS.

use bevy::prelude::{Quat, Vec3};

use crate::arena::ARENA_WALL_THICKNESS;
use crate::map1_b1_garage::GARAGE_HALF_SIZE_Z;

// Trần Sảnh cao hơn hẳn Gara ("không gian mở, trần cao" theo mockup) —
// hằng số riêng thay vì dùng chung ARENA_WALL_HEIGHT.
pub const LOBBY_WALL_HEIGHT: f32 = 8.0;

// Kích thước sàn Sảnh: 30m (X) x 24m (Z) — rộng hơn Gara, đúng tinh thần
// "không gian mở" trước khi vào nút thắt cầu thang xoắn ốc.
pub const LOBBY_HALF_SIZE_X: f32 = 15.0;
pub const LOBBY_HALF_SIZE_Z: f32 = 12.0;

// Chênh cao độ thật giữa sàn Gara (Y=0) và sàn Sảnh — xấp xỉ 1 tầng nhà
// thực tế (~3m).
pub const FLOOR_HEIGHT: f32 = 3.0;

// Đường dốc nối 2 tầng: chiều dài mặt bằng (hình chiếu ngang) 9m, cùng
// bề rộng với khoảng hở đã chừa ở tường Nam Gara / Bắc Sảnh (RAMP_WIDTH
// = 2 * gap_half, xem garage_walls()). 9m ngang / 3m cao ≈ góc dốc 18.4°
// — thoải mái đi bộ, còn dư nhiều so với giới hạn ~53° của controller.
pub const RAMP_RUN: f32 = 9.0;
pub const RAMP_WIDTH: f32 = 4.0;

// Sảnh G nối tiếp ngay sau đường dốc — cạnh Bắc của Sảnh giờ nằm ở
// GARAGE_HALF_SIZE_Z + RAMP_RUN (chứ không sát Gara như trước khi chưa
// có ramp), Sảnh trải dài thêm LOBBY_HALF_SIZE_Z * 2 về phía Nam từ đó.
// LOBBY_CENTER_Z là tâm Sảnh theo trục Z thế giới (không phải tâm cục bộ
// 0.0 như Gara).
pub const LOBBY_NORTH_WALL_Z: f32 = GARAGE_HALF_SIZE_Z + RAMP_RUN;
pub const LOBBY_CENTER_Z: f32 = LOBBY_NORTH_WALL_Z + LOBBY_HALF_SIZE_Z;

// Điểm player có thể đi tới ngay sau khi lên hết dốc — dùng để test
// riêng Sảnh G mà không cần đi bộ hết Gara + dốc trước.
pub const LOBBY_TEST_SPAWN: Vec3 = Vec3::new(0.0, FLOOR_HEIGHT + 3.0, LOBBY_NORTH_WALL_Z + 2.0);

// "Quầy lễ tân" — cover box phía Tây, gần lối vào, đặt trên mặt sàn đã
// nâng cao FLOOR_HEIGHT.
pub const LOBBY_DESK_SIZE: Vec3 = Vec3::new(4.0, 1.1, 1.6);
pub const LOBBY_DESK_POS: (f32, f32) = (-9.0, LOBBY_NORTH_WALL_Z + 4.0);

// "Chân cầu thang xoắn ốc" — trụ placeholder phía Đông, đánh dấu vị trí
// nút thắt lên F2 (chưa xây tầng đó, nên trụ này hiện tại không dẫn đi
// đâu — chỉ là điểm mốc hình học cho lần build tiếp theo).
pub const LOBBY_STAIRCASE_RADIUS: f32 = 2.2;
pub const LOBBY_STAIRCASE_HEIGHT: f32 = LOBBY_WALL_HEIGHT;
pub const LOBBY_STAIRCASE_POS: (f32, f32) = (10.0, LOBBY_NORTH_WALL_Z + 12.0);

/// (center, rotation quanh trục X, full-size xyz) cho khối collider/mesh
/// của đường dốc — 1 cuboid mỏng xoay nghiêng, mặt trên đi từ sàn Gara
/// (Y=0, Z=GARAGE_HALF_SIZE_Z) lên tới sàn Sảnh (Y=FLOOR_HEIGHT,
/// Z=LOBBY_NORTH_WALL_Z). Rotation quanh trục X là góc ÂM của góc dốc vì
/// Bevy's Rx(φ) đưa local +Z thành world (0, -sin φ, cos φ) — cần
/// φ = -góc_dốc để mặt dốc nghiêng LÊN khi Z tăng (xem doc comment ở đầu
/// file cho lý do chọn ramp thay vì bậc thang).
pub fn ramp_geometry() -> (Vec3, Quat, Vec3) {
    let slope_angle = FLOOR_HEIGHT.atan2(RAMP_RUN);
    let surface_len = (RAMP_RUN * RAMP_RUN + FLOOR_HEIGHT * FLOOR_HEIGHT).sqrt();
    let center = Vec3::new(0.0, FLOOR_HEIGHT / 2.0, GARAGE_HALF_SIZE_Z + RAMP_RUN / 2.0);
    let rotation = Quat::from_rotation_x(-slope_angle);
    let size = Vec3::new(RAMP_WIDTH, ARENA_WALL_THICKNESS, surface_len);
    (center, rotation, size)
}

/// (center, size_x, size_y, size_z) cho 2 tường chắn 2 bên đường dốc —
/// tường thẳng đứng (không nghiêng theo dốc), đủ cao để phủ hết từ chân
/// dốc tới đỉnh dốc + hết chiều cao Sảnh, tránh người chơi rơi ra ngoài
/// hai bên khi đang lên/xuống dốc.
pub fn ramp_guard_walls() -> [(Vec3, f32, f32, f32); 2] {
    let height = FLOOR_HEIGHT + LOBBY_WALL_HEIGHT;
    let center_z = GARAGE_HALF_SIZE_Z + RAMP_RUN / 2.0;
    [
        (Vec3::new(-RAMP_WIDTH / 2.0, height / 2.0, center_z), ARENA_WALL_THICKNESS, height, RAMP_RUN),
        (Vec3::new(RAMP_WIDTH / 2.0, height / 2.0, center_z), ARENA_WALL_THICKNESS, height, RAMP_RUN),
    ]
}

/// (center, size_x, size_z) cho 4 bức tường quây quanh Sảnh — cùng công
/// thức wall_span như garage_walls(). Tường Bắc (giáp đỉnh dốc) CHỪA
/// khoảng hở đúng bằng RAMP_WIDTH để nối liền với đường dốc, không hở
/// khe lệch nhau. Toàn bộ tường đặt trên sàn đã nâng cao FLOOR_HEIGHT.
pub fn lobby_walls() -> [(Vec3, f32, f32); 5] {
    let span_x = LOBBY_HALF_SIZE_X * 2.0 + ARENA_WALL_THICKNESS;
    let span_z = LOBBY_HALF_SIZE_Z * 2.0 + ARENA_WALL_THICKNESS;
    let half_y = FLOOR_HEIGHT + LOBBY_WALL_HEIGHT / 2.0;
    let north_z = LOBBY_NORTH_WALL_Z;
    let south_z = LOBBY_CENTER_Z + LOBBY_HALF_SIZE_Z;
    let gap_half = RAMP_WIDTH / 2.0;
    let north_wall_seg = span_x / 2.0 - gap_half;
    [
        // Bắc (giáp đỉnh dốc) — 2 đoạn, chừa khoảng hở khớp bề rộng dốc.
        (Vec3::new(-(gap_half + north_wall_seg / 2.0), half_y, north_z), north_wall_seg, ARENA_WALL_THICKNESS),
        (Vec3::new(gap_half + north_wall_seg / 2.0, half_y, north_z), north_wall_seg, ARENA_WALL_THICKNESS),
        // Nam — liền mạch, chưa có gì nối tiếp phía sau.
        (Vec3::new(0.0, half_y, south_z), span_x, ARENA_WALL_THICKNESS),
        // Tây / Đông
        (Vec3::new(-LOBBY_HALF_SIZE_X, half_y, LOBBY_CENTER_Z), ARENA_WALL_THICKNESS, span_z),
        (Vec3::new(LOBBY_HALF_SIZE_X, half_y, LOBBY_CENTER_Z), ARENA_WALL_THICKNESS, span_z),
    ]
}

// --- Map 1 "Tòa nhà văn phòng" — hạ tầng dùng chung cho mọi tầng ---
// Trước đây mỗi tầng (garage.rs/lobby.rs/office.rs) tự tay tính lại công
// thức "2 tường chừa khoảng hở khớp bề rộng dốc" + "1 khối dốc nghiêng nối
// 2 tầng" — 3 bản gần như y hệt nhau, chỉ khác vài con số. Module này gom
// phần đó thành 1 chỗ (FloorLevel + ramp_between/ramp_guard_walls_between),
// nên xây thêm tầng mới (F3, F4, Mái...) chỉ cần khai 1 FloorLevel + viết
// nội thất riêng của tầng đó — không phải chép lại phần lượng giác/khoảng
// hở tường lần nữa. Xem garage.rs/lobby.rs/office.rs để thấy cách dùng.

use bevy::prelude::{Quat, Vec3};

use crate::arena::ARENA_WALL_THICKNESS;

pub mod f3;
pub mod garage;
pub mod lobby;
pub mod office;

pub use f3::*;
pub use garage::*;
pub use lobby::*;
pub use office::*;

// Chênh cao độ thật giữa 2 tầng liên tiếp — xấp xỉ 1 tầng nhà thực tế (~3m).
pub const FLOOR_HEIGHT: f32 = 3.0;

// Đường dốc nối 2 tầng: chiều dài mặt bằng (hình chiếu ngang) 9m, cùng bề
// rộng với khoảng hở tường ở cả 2 đầu. 9m ngang / FLOOR_HEIGHT cao ≈ góc
// dốc 18.4° — thoải mái đi bộ, còn dư nhiều so với giới hạn ~53° của
// controller (GROUND_NORMAL_MIN_Y trong player.rs).
pub const RAMP_RUN: f32 = 9.0;
pub const RAMP_WIDTH: f32 = 4.0;

/// Mô tả 1 tầng hình chữ nhật của Map 1 — kích thước mặt bằng + cao độ +
/// đường tường Bắc. Mọi tầng từ B1 trở lên đều khai qua struct này thay vì
/// tự tính center_z/south_wall_z bằng tay, nên các tầng luôn nối khớp
/// nhau (tầng sau lấy `north_wall_z` từ `next_north_wall_z()` của tầng
/// trước) — không thể lệch cộng dồn qua nhiều tầng như khi mỗi file tự
/// tính lại từ đầu.
pub struct FloorLevel {
    pub half_size_x: f32,
    pub half_size_z: f32,
    pub wall_height: f32,
    pub elevation: f32,   // cao độ Y của mặt sàn đi lại được
    pub north_wall_z: f32, // toạ độ Z thế giới của đường tường Bắc
}

impl FloorLevel {
    pub const fn new(half_size_x: f32, half_size_z: f32, wall_height: f32, elevation: f32, north_wall_z: f32) -> Self {
        Self { half_size_x, half_size_z, wall_height, elevation, north_wall_z }
    }

    pub const fn center_z(&self) -> f32 {
        self.north_wall_z + self.half_size_z
    }

    pub const fn south_wall_z(&self) -> f32 {
        self.center_z() + self.half_size_z
    }

    /// Toạ độ Z Bắc mà TẦNG KẾ TIẾP nên dùng làm north_wall_z của nó, chừa
    /// đúng khoảng RAMP_RUN cho đường dốc nối từ khoảng hở Nam tầng này.
    pub const fn next_north_wall_z(&self) -> f32 {
        self.south_wall_z() + RAMP_RUN
    }

    const fn half_y(&self) -> f32 {
        self.elevation + self.wall_height / 2.0
    }

    /// Tường Tây/Đông — giống nhau ở mọi tầng, không phụ thuộc có chừa
    /// khoảng hở Bắc/Nam hay không.
    fn side_walls(&self) -> [(Vec3, f32, f32); 2] {
        let span_z = self.half_size_z * 2.0 + ARENA_WALL_THICKNESS;
        [
            (Vec3::new(-self.half_size_x, self.half_y(), self.center_z()), ARENA_WALL_THICKNESS, span_z),
            (Vec3::new(self.half_size_x, self.half_y(), self.center_z()), ARENA_WALL_THICKNESS, span_z),
        ]
    }

    /// 1 đoạn tường liền mạch dọc theo đường Z cho trước, phủ hết bề rộng
    /// tầng — dùng cho cạnh không có lối đi (chưa xây tầng kế/tầng dưới
    /// cùng).
    fn solid_wall(&self, z: f32) -> (Vec3, f32, f32) {
        let span_x = self.half_size_x * 2.0 + ARENA_WALL_THICKNESS;
        (Vec3::new(0.0, self.half_y(), z), span_x, ARENA_WALL_THICKNESS)
    }

    /// 2 đoạn tường dọc theo đường Z cho trước, chừa khoảng hở rộng
    /// RAMP_WIDTH ở giữa (X=0) — lối đi/cửa dốc.
    fn gapped_wall(&self, z: f32) -> [(Vec3, f32, f32); 2] {
        let span_x = self.half_size_x * 2.0 + ARENA_WALL_THICKNESS;
        let gap_half = RAMP_WIDTH / 2.0;
        let seg = span_x / 2.0 - gap_half;
        [
            (Vec3::new(-(gap_half + seg / 2.0), self.half_y(), z), seg, ARENA_WALL_THICKNESS),
            (Vec3::new(gap_half + seg / 2.0, self.half_y(), z), seg, ARENA_WALL_THICKNESS),
        ]
    }

    /// Tường quây quanh tầng ĐÁY (chưa có tầng nào bên dưới) — chỉ chừa
    /// khoảng hở ở cạnh Nam (lối lên tầng kế), cạnh Bắc liền mạch.
    pub fn walls_ground_floor(&self) -> Vec<(Vec3, f32, f32)> {
        let mut walls = vec![self.solid_wall(self.north_wall_z)];
        walls.extend(self.gapped_wall(self.south_wall_z()));
        walls.extend(self.side_walls());
        walls
    }

    /// Tường quây quanh tầng TRUNG GIAN — chừa khoảng hở CẢ 2 cạnh Bắc
    /// (lối vào từ tầng dưới) và Nam (lối lên tầng trên).
    pub fn walls_mid_floor(&self) -> Vec<(Vec3, f32, f32)> {
        let mut walls = self.gapped_wall(self.north_wall_z).to_vec();
        walls.extend(self.gapped_wall(self.south_wall_z()));
        walls.extend(self.side_walls());
        walls
    }

    /// Tường quây quanh tầng CAO NHẤT đã xây (chưa xây tầng kế tiếp phía
    /// trên) — chỉ chừa khoảng hở ở cạnh Bắc (lối vào từ tầng dưới), cạnh
    /// Nam liền mạch. Khi xây tầng kế tiếp, đổi tầng này sang
    /// walls_mid_floor() và gọi ramp_between() nối nó với tầng mới.
    pub fn walls_top_floor(&self) -> Vec<(Vec3, f32, f32)> {
        let mut walls = self.gapped_wall(self.north_wall_z).to_vec();
        walls.push(self.solid_wall(self.south_wall_z()));
        walls.extend(self.side_walls());
        walls
    }

    /// (center, size_x, size_z) cho tấm sàn của tầng — center.y đã trừ
    /// nửa độ dày, khớp quy ước "sàn dày ARENA_WALL_THICKNESS, mặt trên ở
    /// đúng elevation" dùng xuyên suốt Map 1.
    pub fn floor_slab(&self) -> (Vec3, f32, f32) {
        let size_x = self.half_size_x * 2.0;
        let size_z = self.half_size_z * 2.0;
        (Vec3::new(0.0, self.elevation - ARENA_WALL_THICKNESS / 2.0, self.center_z()), size_x, size_z)
    }
}

/// (center, rotation quanh trục X, full-size xyz) cho khối collider/mesh
/// của đường dốc nối khoảng hở Nam của `from` lên khoảng hở Bắc của `to`.
/// Rotation quanh trục X là góc ÂM của góc dốc vì Bevy's Rx(φ) đưa local
/// +Z thành world (0, -sin φ, cos φ) — cần φ = -góc_dốc để mặt dốc nghiêng
/// LÊN khi Z tăng.
///
/// `center` là tâm ĐƯỜNG TÂM (centerline) của khối, không phải mặt trên
/// (mặt người chơi thật sự đi trên đó) — mặt trên lệch khỏi centerline
/// đúng `ARENA_WALL_THICKNESS/2` theo PHÁP TUYẾN của mặt dốc (vuông góc
/// mặt nghiêng), không phải thẳng đứng. Với 1 mặt phẳng nghiêng, "lệch
/// theo pháp tuyến" ăn bớt 1 phần theo trục đứng (nhân cos góc dốc) và
/// thêm 1 chút theo trục ngang (nhân sin góc dốc) — nên nếu cứ đặt tâm
/// centerline đúng ngay từ.elevation/to.elevation, MẶT TRÊN sẽ nằm cao
/// hơn hẳn 2 mặt sàn ở cả 2 đầu dốc (đúng bug "cầu thang cao hơn floor"
/// đã báo — lỗi có sẵn từ trước, không phải do panel gây ra). Dịch tâm
/// lùi lại đúng nửa độ dày theo pháp tuyến (xuống dưới) để bù trừ, mặt
/// trên mới thật sự chạm khít cả 2 đầu sàn.
pub fn ramp_between(from: &FloorLevel, to: &FloorLevel) -> (Vec3, Quat, Vec3) {
    let rise = to.elevation - from.elevation;
    let slope_angle = rise.atan2(RAMP_RUN);
    let surface_len = (RAMP_RUN * RAMP_RUN + rise * rise).sqrt();
    let rotation = Quat::from_rotation_x(-slope_angle);
    let centerline = Vec3::new(0.0, from.elevation + rise / 2.0, from.south_wall_z() + RAMP_RUN / 2.0);
    let center = centerline - rotation * Vec3::new(0.0, ARENA_WALL_THICKNESS / 2.0, 0.0);
    let size = Vec3::new(RAMP_WIDTH, ARENA_WALL_THICKNESS, surface_len);
    (center, rotation, size)
}

/// (center, size_x, size_y, size_z) cho 2 tường chắn 2 bên đường dốc nối
/// `from` lên `to` — tường thẳng đứng (không nghiêng theo dốc), đủ cao để
/// phủ từ mặt đất (Y=0) tới hết trần của tầng đích, tránh người chơi rơi
/// ra ngoài khi lên/xuống dốc.
pub fn ramp_guard_walls_between(from: &FloorLevel, to: &FloorLevel) -> [(Vec3, f32, f32, f32); 2] {
    let height = to.elevation + to.wall_height;
    let center_z = from.south_wall_z() + RAMP_RUN / 2.0;
    [
        (Vec3::new(-RAMP_WIDTH / 2.0, height / 2.0, center_z), ARENA_WALL_THICKNESS, height, RAMP_RUN),
        (Vec3::new(RAMP_WIDTH / 2.0, height / 2.0, center_z), ARENA_WALL_THICKNESS, height, RAMP_RUN),
    ]
}

/// Hướng mở (không có tường) của 1 phòng nhỏ hình hộp — xem
/// small_room_walls().
pub enum RoomDoor {
    North,
    South,
    East,
    West,
}

/// 3 tường cho 1 phòng kín hình chữ nhật, mở duy nhất 1 mặt theo `door` —
/// dùng chung cho mọi phòng nhỏ kiểu "văn phòng riêng"/"phòng họp"/"phòng
/// tủ đồ" xuất hiện lặp lại qua nhiều tầng Map 1 (F2's office/locker
/// room, F3's meeting/side offices, v.v.) thay vì mỗi tầng tự viết lại 1
/// bản `match`/if-else riêng cho đúng 1 hướng cửa của nó.
pub fn small_room_walls(center: (f32, f32), half: Vec3, base_y: f32, door: RoomDoor) -> [(Vec3, f32, f32, f32); 3] {
    let (cx, cz) = center;
    let (size_x, size_z) = (half.x * 2.0, half.z * 2.0);
    let size_y = half.y * 2.0;
    match door {
        RoomDoor::East => [
            (Vec3::new(cx - half.x, base_y, cz), ARENA_WALL_THICKNESS, size_y, size_z + ARENA_WALL_THICKNESS),
            (Vec3::new(cx, base_y, cz - half.z), size_x, size_y, ARENA_WALL_THICKNESS),
            (Vec3::new(cx, base_y, cz + half.z), size_x, size_y, ARENA_WALL_THICKNESS),
        ],
        RoomDoor::West => [
            (Vec3::new(cx + half.x, base_y, cz), ARENA_WALL_THICKNESS, size_y, size_z + ARENA_WALL_THICKNESS),
            (Vec3::new(cx, base_y, cz - half.z), size_x, size_y, ARENA_WALL_THICKNESS),
            (Vec3::new(cx, base_y, cz + half.z), size_x, size_y, ARENA_WALL_THICKNESS),
        ],
        RoomDoor::South => [
            (Vec3::new(cx, base_y, cz - half.z), size_x, size_y, ARENA_WALL_THICKNESS),
            (Vec3::new(cx - half.x, base_y, cz), ARENA_WALL_THICKNESS, size_y, size_z + ARENA_WALL_THICKNESS),
            (Vec3::new(cx + half.x, base_y, cz), ARENA_WALL_THICKNESS, size_y, size_z + ARENA_WALL_THICKNESS),
        ],
        RoomDoor::North => [
            (Vec3::new(cx, base_y, cz + half.z), size_x, size_y, ARENA_WALL_THICKNESS),
            (Vec3::new(cx - half.x, base_y, cz), ARENA_WALL_THICKNESS, size_y, size_z + ARENA_WALL_THICKNESS),
            (Vec3::new(cx + half.x, base_y, cz), ARENA_WALL_THICKNESS, size_y, size_z + ARENA_WALL_THICKNESS),
        ],
    }
}

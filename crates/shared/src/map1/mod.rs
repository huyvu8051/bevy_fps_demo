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
#[derive(Clone, Copy)]
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
    ///
    /// span_z KHÔNG cộng thêm ARENA_WALL_THICKNESS nữa (khác hồi còn là
    /// LevelBox) — hồi đó walls là khối hộp, cần kéo dài thêm 1 chút để 2
    /// khối chồng lên nhau ở góc, tránh lộ 1 khe tam giác nhỏ chưa được
    /// khối nào phủ tới. Giờ walls là LevelPanel (mặt phẳng không có khối
    /// tích) — 2 mặt phẳng vuông góc chỉ cần span tới ĐÚNG toạ độ góc là
    /// tự khít nhau hoàn hảo, phần "thừa ra" trước đây chỉ còn là 1 mép
    /// thừa vô ích (đã báo).
    fn side_walls(&self) -> [(Vec3, f32, f32); 2] {
        let span_z = self.half_size_z * 2.0;
        [
            (Vec3::new(-self.half_size_x, self.half_y(), self.center_z()), ARENA_WALL_THICKNESS, span_z),
            (Vec3::new(self.half_size_x, self.half_y(), self.center_z()), ARENA_WALL_THICKNESS, span_z),
        ]
    }

    /// 1 đoạn tường liền mạch dọc theo đường Z cho trước, phủ hết bề rộng
    /// tầng — dùng cho cạnh không có lối đi (chưa xây tầng kế/tầng dưới
    /// cùng). Xem side_walls's doc comment vì sao không cộng thêm
    /// ARENA_WALL_THICKNESS nữa.
    fn solid_wall(&self, z: f32) -> (Vec3, f32, f32) {
        let span_x = self.half_size_x * 2.0;
        (Vec3::new(0.0, self.half_y(), z), span_x, ARENA_WALL_THICKNESS)
    }

    /// 2 đoạn tường dọc theo đường Z cho trước, chừa khoảng hở rộng
    /// RAMP_WIDTH ở giữa (X=0) — lối đi/cửa dốc. Xem side_walls's doc
    /// comment vì sao không cộng thêm ARENA_WALL_THICKNESS nữa.
    fn gapped_wall(&self, z: f32) -> [(Vec3, f32, f32); 2] {
        let span_x = self.half_size_x * 2.0;
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

    /// Tường quây quanh tầng có lối vào từ DƯỚI lên (khoảng hở Bắc, như
    /// walls_mid_floor) nhưng KHÔNG có khoảng hở tường Nam nữa — dùng khi
    /// tầng ngay trên nó chồng thật (cùng tâm X/Z) thay vì nằm xéo, nên
    /// lối lên tầng trên giờ là 1 giếng thang xuyên sàn, không phải
    /// khoảng hở tường.
    pub fn walls_gapped_north_only(&self) -> Vec<(Vec3, f32, f32)> {
        let mut walls = self.gapped_wall(self.north_wall_z).to_vec();
        walls.push(self.solid_wall(self.south_wall_z()));
        walls.extend(self.side_walls());
        walls
    }

    /// Tường quây KÍN tầng — không khoảng hở nào cả. Dùng cho tầng chồng
    /// thật lên tầng dưới VÀ có tầng trên (cũng chồng thật) — cả 2 lối
    /// nối đều là giếng thang xuyên sàn, không đi qua tường.
    pub fn walls_enclosed(&self) -> Vec<(Vec3, f32, f32)> {
        let mut walls = vec![self.solid_wall(self.north_wall_z), self.solid_wall(self.south_wall_z())];
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

    /// 4 dải sàn bao quanh 1 lỗ vuông ở giữa (chỗ giếng thang xuyên qua từ
    /// tầng dưới lên) — thay cho floor_slab() khi tầng này có 1 giếng
    /// thang chồng thật xuyên qua. Cùng quy ước center.y với floor_slab().
    /// Người gọi tự đảm bảo lỗ nằm trọn trong biên sàn — không tự kiểm
    /// tra, chỉ là hình học thuần túy.
    pub fn floor_slab_with_hole(&self, hole_center_x: f32, hole_center_z: f32, hole_half: f32) -> [(Vec3, f32, f32); 4] {
        let y = self.elevation - ARENA_WALL_THICKNESS / 2.0;
        let (hx0, hx1) = (hole_center_x - hole_half, hole_center_x + hole_half);
        let (hz0, hz1) = (hole_center_z - hole_half, hole_center_z + hole_half);
        let (fx0, fx1) = (-self.half_size_x, self.half_size_x);
        let (fz0, fz1) = (self.north_wall_z, self.south_wall_z());
        [
            // Bắc: mép Bắc sàn → mép Bắc lỗ, đủ bề rộng X.
            (Vec3::new(0.0, y, (fz0 + hz0) / 2.0), fx1 - fx0, hz0 - fz0),
            // Nam: mép Nam lỗ → mép Nam sàn.
            (Vec3::new(0.0, y, (hz1 + fz1) / 2.0), fx1 - fx0, fz1 - hz1),
            // Tây: đúng dải Z của lỗ, mép Tây sàn → mép Tây lỗ.
            (Vec3::new((fx0 + hx0) / 2.0, y, hole_center_z), hx0 - fx0, hz1 - hz0),
            // Đông: đúng dải Z của lỗ, mép Đông lỗ → mép Đông sàn.
            (Vec3::new((hx1 + fx1) / 2.0, y, hole_center_z), fx1 - hx1, hz1 - hz0),
        ]
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

/// Rải 1 điểm spawn zombie lên 1 TRONG 4 TẦNG THẬT của Map 1, thay vì mù
/// quáng dùng lại lưới của arena phẳng cũ (`shared::zombie::
/// random_zombie_spawn_pos`, dựa trên `ARENA_HALF_SIZE=36` — không liên
/// quan gì tới B1/G/F2/F3, khiến phần lớn zombie rơi vào khoảng trống
/// giữa các tầng, xem map1 coordinate audit). Cùng cách R2
/// low-discrepancy `random_zombie_spawn_pos` đã dùng (deterministic theo
/// `seed`, không cần crate `rand`) — chỉ khác là CHỌN TẦNG trước (round-
/// robin theo `seed % 4`) rồi mới rải trong đúng biên tầng đó.
pub fn random_map1_zombie_spawn_pos(seed: usize) -> Vec3 {
    const INV_G: f32 = 0.754_877_7; // 1/g, g = plastic number (R2 sequence)
    const INV_G2: f32 = 0.569_840_3; // 1/g^2
    // Lùi vào trong 1 chút so với tường, tránh spawn dính luôn vào tường/
    // góc phòng — cùng tinh thần "half = ARENA_HALF_SIZE - 3.0" bên
    // random_zombie_spawn_pos, chỉ nhỏ hơn vì các tầng Map 1 hẹp hơn
    // nhiều so với arena test 72×72m.
    const WALL_INSET: f32 = 2.0;

    let floors: [FloorLevel; 4] = [GARAGE_FLOOR, LOBBY_FLOOR, F2_FLOOR, F3_FLOOR];
    let floor = floors[seed % floors.len()];
    // Seed riêng cho chuỗi R2 TRONG tầng đó — nếu dùng thẳng `seed` thì 4
    // tầng sẽ lặp lại đúng 1/4 số điểm trong chuỗi chung, dễ trùng cụm;
    // chia cho số tầng để mỗi tầng có chuỗi rải đều của riêng nó.
    let in_floor_seed = (seed / floors.len()) as f32;

    let half_x = (floor.half_size_x - WALL_INSET).max(0.5);
    let half_z = (floor.half_size_z - WALL_INSET).max(0.5);
    let x = (-1.0 + 2.0 * (in_floor_seed * INV_G).fract()) * half_x;
    let z = floor.center_z() + (-1.0 + 2.0 * (in_floor_seed * INV_G2).fract()) * half_z;
    let y = floor.elevation + crate::zombie::ZOMBIE_RADIUS + crate::zombie::ZOMBIE_LENGTH / 2.0;
    Vec3::new(x, y, z)
}

// --- Giếng thang bộ "4 góc cuộn" — nối các tầng CHỒNG THẬT (cùng tâm
// X/Z, chỉ khác cao độ) thay cho ramp_between() (vốn giả định 2 tầng nằm
// xéo dọc Z, cách nhau đúng RAMP_RUN). G/F2/F3 giờ dùng chung 1
// north_wall_z (xem doc comment ở office.rs/f3.rs) nên "khoảng hở tường
// Nam nối sang tầng kế" không còn nghĩa gì — lối lên thật là 1 giếng
// vuông xuyên qua lỗ sàn tầng trên, đi 1 vòng quanh 4 cạnh giếng (ngược
// chiều kim đồng hồ) trong lúc leo hết 1 tầng cao — đúng tinh thần "cầu
// thang xoắn ốc" trong GDD, chỉ là góc vuông thay vì cong tròn cho dễ
// dựng bằng panel phẳng.

/// Vị trí X của giếng — dùng chung cho mọi cặp tầng chồng thật (1 giếng
/// liên tục xuyên suốt G→F2→F3→...), giữ nguyên quy ước X=6 các mốc
/// "chân cầu thang xoắn ốc" cũ (LOBBY_STAIRCASE_POS/F2_STAIRCASE_POS...)
/// đã dùng.
pub const TOWER_SHAFT_X: f32 = 6.0;
/// Nửa bề rộng giếng (hình vuông TOWER_SHAFT_HALF*2 mỗi cạnh).
pub const TOWER_SHAFT_HALF: f32 = 2.0;
/// Bề rộng mỗi nhịp thang (dải panel người chơi thật sự đi trên đó) —
/// nhỏ hơn TOWER_SHAFT_HALF*2 để chừa 1 khoảng trống hình vuông ở giữa
/// giếng (giống giếng trời/light-well của cầu thang xoắn ốc thật).
pub const TOWER_STAIR_WIDTH: f32 = 1.6;
/// Khoảng cách từ north_wall_z (chung của mọi tầng chồng thật) tới tâm Z
/// giếng — chọn 16.5 vì đây là dải Z DUY NHẤT không đụng bất kỳ phòng
/// nội thất nào đã có sẵn ở CẢ F2 (bàn ở +5, phòng riêng/tủ đồ ở 8.5-13.5)
/// lẫn F3 (phòng họp 1-7, văn phòng A/B 34.5-44.5) cùng lúc — xem map1
/// coordinate audit. Đổi số này thì phải kiểm tra lại cả 2 tầng.
pub const TOWER_SHAFT_Z_OFFSET: f32 = 16.5;

/// (center, rotation, full-size) cho 1 đoạn dốc thẳng nối 2 điểm bất kỳ
/// trong không gian — tổng quát hoá ramp_between() (vốn chỉ chạy dọc
/// trục Z) để dùng cho các nhịp switchback chạy dọc trục X hoặc Z tuỳ
/// cạnh giếng. Cùng phép bù "centerline → mặt trên" ramp_between() đã có
/// (xem doc comment của nó) để mặt dốc chạm khít cả 2 đầu, không lơ lửng.
fn ramp_segment(start: Vec3, end: Vec3, width: f32, thickness: f32) -> (Vec3, Quat, Vec3) {
    let delta = end - start;
    let run = (delta.x * delta.x + delta.z * delta.z).sqrt();
    let rise = delta.y;
    // Quay quanh Y trước để local +Z hướng đúng theo (delta.x, delta.z)
    // nằm ngang, RỒI mới nghiêng quanh trục X cục bộ (đã xoay) theo góc
    // dốc — xem doc comment ramp_between() cho công thức Rx cơ sở.
    let heading = delta.x.atan2(delta.z);
    let slope_angle = rise.atan2(run);
    let surface_len = (run * run + rise * rise).sqrt();
    let rotation = Quat::from_rotation_y(heading) * Quat::from_rotation_x(-slope_angle);
    let centerline = (start + end) / 2.0;
    let center = centerline - rotation * Vec3::new(0.0, thickness / 2.0, 0.0);
    (center, rotation, Vec3::new(width, thickness, surface_len))
}

/// (center, size_x, size_y, size_z) × 2 — lan can 2 bên 1 nhịp thang
/// switchback (thẳng đứng, không nghiêng theo dốc, giống
/// ramp_guard_walls_between nhưng chỉ cao GUARD_HEIGHT cục bộ quanh
/// nhịp đó thay vì suốt chiều cao tầng — nhịp này nằm kín trong giếng
/// riêng, không phải ngoài trời). `start`/`end` luôn thẳng theo trục X
/// hoặc Z (giếng vuông góc), nên chỉ cần biết trục nào để chọn hướng
/// lệch sang 2 bên.
fn switchback_guard_walls(start: Vec3, end: Vec3, width: f32) -> [(Vec3, f32, f32, f32); 2] {
    const GUARD_HEIGHT: f32 = 1.2;
    let mid = (start + end) / 2.0;
    let mid_y = mid.y + GUARD_HEIGHT / 2.0;
    if (end.x - start.x).abs() >= (end.z - start.z).abs() {
        let len = (end.x - start.x).abs();
        [
            (Vec3::new(mid.x, mid_y, mid.z - width / 2.0), len, GUARD_HEIGHT, ARENA_WALL_THICKNESS),
            (Vec3::new(mid.x, mid_y, mid.z + width / 2.0), len, GUARD_HEIGHT, ARENA_WALL_THICKNESS),
        ]
    } else {
        let len = (end.z - start.z).abs();
        [
            (Vec3::new(mid.x - width / 2.0, mid_y, mid.z), ARENA_WALL_THICKNESS, GUARD_HEIGHT, len),
            (Vec3::new(mid.x + width / 2.0, mid_y, mid.z), ARENA_WALL_THICKNESS, GUARD_HEIGHT, len),
        ]
    }
}

/// 4 nhịp dốc + lan can cho 1 giếng thang "4 góc cuộn" nối `from` lên
/// `to` (2 tầng CHỒNG THẬT, cùng tâm X/Z) — đi 1 vòng ngược chiều kim
/// đồng hồ quanh giếng vuông tâm `(shaft_x, shaft_z)`, mỗi cạnh leo đúng
/// 1/4 tổng độ cao. Điểm cuối nhịp N luôn trùng khít điểm đầu nhịp N+1
/// (cùng toạ độ, cùng cao độ) nên 4 nhịp nối liền mạch không cần chiếu
/// nghỉ riêng. Trả về (4 nhịp dốc, 8 tường lan can) — người gọi tự
/// `ramp_panel`/`wall_panel4` từng cái như mọi mảnh khác của map1.
pub fn switchback_stairwell(shaft_x: f32, shaft_z: f32, from: &FloorLevel, to: &FloorLevel) -> (Vec<(Vec3, Quat, Vec3)>, Vec<(Vec3, f32, f32, f32)>) {
    let rise_per_flight = (to.elevation - from.elevation) / 4.0;
    let s = TOWER_SHAFT_HALF;
    let w = TOWER_STAIR_WIDTH;
    // 5 góc = 4 cạnh, ngược chiều kim đồng hồ nhìn từ trên xuống, đúng
    // MÉP giếng (không lùi vào theo width nữa — xem lý do bên dưới).
    let corners_xz = [(shaft_x - s, shaft_z + s), (shaft_x + s, shaft_z + s), (shaft_x + s, shaft_z - s), (shaft_x - s, shaft_z - s), (shaft_x - s, shaft_z + s)];
    // Hướng đi (dx,dz) của từng nhịp — luôn dọc trục X hoặc Z vì giếng
    // vuông góc.
    let dirs = [(1.0, 0.0), (0.0, -1.0), (-1.0, 0.0), (0.0, 1.0)];

    let mut flights = Vec::with_capacity(7);
    let mut guards = Vec::with_capacity(8);
    for i in 0..4 {
        let (x0, z0) = corners_xz[i];
        let (x1, z1) = corners_xz[i + 1];
        let (dx, dz) = dirs[i];
        // Mỗi nhịp dốc LÙI VÀO w/2 ở đầu nào tiếp giáp 1 góc RẼ (i>0 ở
        // đầu, i<3 ở cuối) — góc bắt đầu (i=0) tiếp giáp sàn tầng dưới,
        // góc kết thúc (i=3) tiếp giáp lỗ sàn tầng trên, cả 2 không cần
        // lùi. Phần bị lùi ra đó nhường chỗ cho 1 tấm chiếu nghỉ PHẲNG
        // (xem bên dưới) — nếu để 2 nhịp dốc vuông góc chạm thẳng vào
        // nhau tại đúng 1 điểm góc như trước, phần bề rộng w của chúng
        // xoè ra quanh góc đó ĐÈ LÊN NHAU (2 mặt nghiêng cắt nhau) — đây
        // chính là lý do giếng "không đi được" đã báo: người chơi kẹt
        // giữa 2 mặt dốc chồng chéo ở mỗi góc rẽ.
        let x0b = if i > 0 { x0 + dx * (w / 2.0) } else { x0 };
        let z0b = if i > 0 { z0 + dz * (w / 2.0) } else { z0 };
        let x1b = if i < 3 { x1 - dx * (w / 2.0) } else { x1 };
        let z1b = if i < 3 { z1 - dz * (w / 2.0) } else { z1 };
        let start = Vec3::new(x0b, from.elevation + rise_per_flight * i as f32, z0b);
        let end = Vec3::new(x1b, from.elevation + rise_per_flight * (i as f32 + 1.0), z1b);
        flights.push(ramp_segment(start, end, w, ARENA_WALL_THICKNESS));
        guards.extend(switchback_guard_walls(start, end, w));
    }
    // 3 chiếu nghỉ PHẲNG (không nghiêng) ở 3 góc rẽ giữa (không cần ở góc
    // vào/ra) — vuông w×w, lấp đúng phần các nhịp dốc đã lùi ra, cho
    // người chơi 1 chỗ đứng thẳng để xoay hướng thay vì rẽ ngay trên mặt
    // dốc.
    let landing_rotation = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
    for i in 1..4 {
        let (cx, cz) = corners_xz[i];
        // Lùi vào w/2 theo cả 2 trục, hướng về tâm giếng — đúng khớp mép
        // 2 nhịp dốc liền kề đã lùi vào (xem vòng lặp trên), không thừa
        // không thiếu.
        let x = cx + (shaft_x - cx).signum() * (w / 2.0);
        let z = cz + (shaft_z - cz).signum() * (w / 2.0);
        let y = from.elevation + rise_per_flight * i as f32;
        let centerline = Vec3::new(x, y, z);
        let center = centerline - landing_rotation * Vec3::new(0.0, ARENA_WALL_THICKNESS / 2.0, 0.0);
        flights.push((center, landing_rotation, Vec3::new(w, ARENA_WALL_THICKNESS, w)));
    }
    (flights, guards)
}

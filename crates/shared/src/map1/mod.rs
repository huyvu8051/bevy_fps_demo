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
use crate::level_piece::{floor_panel, ramp_panel, wall_panel, wall_panel4, BoxPiece, PanelPiece};

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
    pub fn floor_slab_with_hole(&self, hole_center_x: f32, hole_center_z: f32, hole_half_x: f32, hole_half_z: f32) -> [(Vec3, f32, f32); 4] {
        let y = self.elevation - ARENA_WALL_THICKNESS / 2.0;
        let (hx0, hx1) = (hole_center_x - hole_half_x, hole_center_x + hole_half_x);
        let (hz0, hz1) = (hole_center_z - hole_half_z, hole_center_z + hole_half_z);
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

/// Vị trí X của giếng thang — dùng chung cho mọi cặp tầng chồng thật (1
/// đường đi liên tục xuyên suốt G→F2→F3→...), giữ nguyên quy ước X=6 các
/// mốc "chân cầu thang xoắn ốc" cũ (LOBBY_STAIRCASE_POS/
/// F2_STAIRCASE_POS...) đã dùng.
pub const TOWER_SHAFT_X: f32 = 6.0;
/// Bề rộng lối đi (không có tường/lan can 2 bên — đơn giản hoá theo yêu
/// cầu, chỉ có sàn).
pub const TOWER_STAIR_WIDTH: f32 = 3.0;
/// Chiều dài mặt bằng mỗi nhịp dốc (RUN) và chiếu nghỉ ở giữa (LANDING).
/// Nhỏ hơn hẳn RAMP_RUN (dốc chính, 9m cho 3m cao — ~18°) vì phải nằm
/// gọn trong dải Z trống duy nhất giữa các phòng nội thất F2/F3 đã có
/// sẵn (~5m, xem TOWER_SHAFT_Z_OFFSET) — dốc hơn (~45°) nhưng vẫn trong
/// giới hạn đi được của controller (GROUND_NORMAL_MIN_Y trong player.rs
/// cho phép tới ~53°), giống độ dốc 1 cầu thang bộ thật hơn là 1 đường
/// dốc xe.
pub const TOWER_STAIR_RUN: f32 = 1.5;
pub const TOWER_STAIR_LANDING_RUN: f32 = 1.5;
/// Khoảng cách từ north_wall_z (chung của mọi tầng chồng thật) tới tâm Z
/// giếng — chọn để dải Z toàn bộ cầu thang (2×TOWER_STAIR_RUN +
/// TOWER_STAIR_LANDING_RUN = 4.5m) không đụng phòng nội thất nào đã có
/// sẵn ở CẢ F2 (phòng riêng/tủ đồ ở Z 27.5-32.5) lẫn F3 (văn phòng A/B ở
/// Z 37.5-44.5) — xem map1 coordinate audit. Đổi số này (hoặc
/// TOWER_STAIR_RUN/LANDING_RUN) thì phải kiểm tra lại cả 2 tầng.
pub const TOWER_SHAFT_Z_OFFSET: f32 = 16.0;

/// (center, rotation, full-size) cho 1 đoạn dốc thẳng nối 2 điểm bất kỳ
/// trong không gian — tổng quát hoá ramp_between() (vốn chỉ chạy dọc
/// trục Z) để dùng chung cho cả dốc chính lẫn simple_stairwell bên dưới.
/// Cùng phép bù "centerline → mặt trên" ramp_between() đã có (xem doc
/// comment của nó) để mặt dốc chạm khít cả 2 đầu, không lơ lửng.
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

/// Cầu thang ĐƠN GIẢN nối `from` lên `to` (2 tầng CHỒNG THẬT, cùng tâm
/// X/Z) — bản rút gọn tối đa: chỉ 2 nhịp dốc thẳng hàng dọc trục Z, nối
/// nhau bằng 1 tấm chiếu nghỉ PHẲNG ở đúng giữa 2 tầng, KHÔNG có
/// tường/lan can 2 bên (chỉ có sàn, theo đúng yêu cầu). Nhịp 1 leo nửa
/// đầu độ cao, đi ngang qua chiếu nghỉ, nhịp 2 leo nốt nửa còn lại —
/// toàn bộ nằm thẳng 1 đường dọc Z, không rẽ góc.
pub fn simple_stairwell(shaft_x: f32, shaft_z: f32, from: &FloorLevel, to: &FloorLevel) -> Vec<(Vec3, Quat, Vec3)> {
    let mid_elevation = (from.elevation + to.elevation) / 2.0;
    let half_landing = TOWER_STAIR_LANDING_RUN / 2.0;
    let z0 = shaft_z - half_landing - TOWER_STAIR_RUN;
    let z1 = shaft_z - half_landing;
    let z2 = shaft_z + half_landing;
    let z3 = shaft_z + half_landing + TOWER_STAIR_RUN;

    let ramp1 = ramp_segment(Vec3::new(shaft_x, from.elevation, z0), Vec3::new(shaft_x, mid_elevation, z1), TOWER_STAIR_WIDTH, ARENA_WALL_THICKNESS);
    let ramp2 = ramp_segment(Vec3::new(shaft_x, mid_elevation, z2), Vec3::new(shaft_x, to.elevation, z3), TOWER_STAIR_WIDTH, ARENA_WALL_THICKNESS);

    let landing_rotation = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
    let landing_centerline = Vec3::new(shaft_x, mid_elevation, shaft_z);
    let landing_center = landing_centerline - landing_rotation * Vec3::new(0.0, ARENA_WALL_THICKNESS / 2.0, 0.0);
    let landing = (landing_center, landing_rotation, Vec3::new(TOWER_STAIR_WIDTH, ARENA_WALL_THICKNESS, TOWER_STAIR_LANDING_RUN));

    vec![ramp1, landing, ramp2]
}

/// Nửa chiều dài (dọc Z) toàn bộ khoảng cầu thang chiếm — dùng để đục lỗ
/// sàn tầng trên đúng khớp (xem FloorLevel::floor_slab_with_hole).
pub const fn simple_stairwell_half_len() -> f32 {
    TOWER_STAIR_RUN + TOWER_STAIR_LANDING_RUN / 2.0
}

/// Toàn bộ hình học Map 1 (B1 Gara → G Sảnh → F2 → F3) dưới dạng dữ liệu
/// thuần — (BoxPiece cho nội thất đặc như xe/bàn, PanelPiece cho tường/
/// sàn/dốc) — KHÔNG tự spawn gì cả. Gọi từ CẢ client lẫn server (xem
/// level_piece.rs's module doc comment cho lý do): server dùng để spawn
/// Collider-only cho vật lý thật, client dùng để spawn LevelBox/
/// LevelPanel cục bộ (không Replicated) rồi để spawn_level_box_visuals/
/// spawn_level_panel_visuals tự dựng mesh — cùng 1 hàm này đảm bảo 2 bên
/// không bao giờ lệch hình học.
pub fn map1_level_pieces() -> (Vec<BoxPiece>, Vec<PanelPiece>) {
    let mut pieces: Vec<BoxPiece> = Vec::new();
    let mut panels: Vec<PanelPiece> = Vec::new();
    let ident = Quat::IDENTITY;

    // --- B1 Garage ---
    // Tô 2 màu khác nhau cho Khu A (gần lối vào/spawn) và Khu B (sâu
    // hơn) để phân biệt trực quan — Khu B tối hơn hẳn (sàn lẫn xe), đúng
    // tinh thần "sâu hơn, tối hơn" của mockup (docs/maps/map01_highrise.md).
    // Ranh giới đặt ở Z=1.5 — giữa hàng xe cuối Khu A (z=0) và hàng xe
    // đầu Khu B (z=3), xem GARAGE_CAR_POSITIONS.
    let garage_wall_color = [0.55, 0.5, 0.45];
    let khu_boundary_z = 1.5;
    let khu_a_floor_color = [0.42, 0.42, 0.46];
    let khu_b_floor_color = [0.18, 0.18, 0.21];
    let khu_a_car_color = [0.65, 0.15, 0.15]; // đỏ
    let khu_b_car_color = [0.15, 0.25, 0.55]; // xanh dương đậm

    let floor_size_x = GARAGE_HALF_SIZE_X * 2.0;
    let khu_a_size_z = khu_boundary_z - -GARAGE_HALF_SIZE_Z;
    let khu_b_size_z = GARAGE_HALF_SIZE_Z - khu_boundary_z;
    panels.push(floor_panel(
        Vec3::new(0.0, -ARENA_WALL_THICKNESS / 2.0, -GARAGE_HALF_SIZE_Z + khu_a_size_z / 2.0),
        floor_size_x,
        khu_a_size_z,
        ARENA_WALL_THICKNESS,
        khu_a_floor_color,
        0.9,
    ));
    panels.push(floor_panel(
        Vec3::new(0.0, -ARENA_WALL_THICKNESS / 2.0, khu_boundary_z + khu_b_size_z / 2.0),
        floor_size_x,
        khu_b_size_z,
        ARENA_WALL_THICKNESS,
        khu_b_floor_color,
        0.9,
    ));
    for (center, size_x, size_z) in garage_walls() {
        panels.push(wall_panel(center, size_x, size_z, crate::arena::ARENA_WALL_HEIGHT, garage_wall_color, 0.85));
    }
    let car_size = GARAGE_CAR_SIZE;
    for &(x, z) in &GARAGE_CAR_POSITIONS {
        let car_color = if z < khu_boundary_z { khu_a_car_color } else { khu_b_car_color };
        pieces.push((Vec3::new(x, car_size.y / 2.0, z), ident, car_size, car_color, 0.6));
    }

    // --- G Lobby ---
    let lobby_wall_color = [0.6, 0.58, 0.52];
    panels.push(floor_panel(
        Vec3::new(0.0, FLOOR_HEIGHT - ARENA_WALL_THICKNESS / 2.0, LOBBY_CENTER_Z),
        LOBBY_HALF_SIZE_X * 2.0,
        LOBBY_HALF_SIZE_Z * 2.0,
        ARENA_WALL_THICKNESS,
        [0.42, 0.4, 0.36],
        0.8,
    ));
    for (center, size_x, size_z) in lobby_walls() {
        panels.push(wall_panel(center, size_x, size_z, LOBBY_WALL_HEIGHT, lobby_wall_color, 0.7));
    }
    let (ramp_center, ramp_rotation, ramp_size) = ramp_geometry();
    panels.push(ramp_panel(ramp_center, ramp_rotation, ramp_size.x, ramp_size.y, ramp_size.z, [0.38, 0.36, 0.33], 0.9));
    for (center, size_x, size_y, size_z) in ramp_guard_walls() {
        panels.push(wall_panel4(center, size_x, size_y, size_z, lobby_wall_color, 0.7));
    }
    let desk_size = LOBBY_DESK_SIZE;
    let (desk_x, desk_z) = LOBBY_DESK_POS;
    pieces.push((Vec3::new(desk_x, FLOOR_HEIGHT + desk_size.y / 2.0, desk_z), ident, desk_size, [0.25, 0.35, 0.45], 0.6));
    // Không còn khối "chân cầu thang" trang trí ở đây nữa — giếng thang
    // thật (stairwell_g_to_f2, xây ở đoạn F2 bên dưới vì nó xuyên từ G
    // lên F2) đã thay thế nó.

    // --- F2 Office (chồng thật ngay trên G — xem F2_FLOOR's doc comment) ---
    let f2_wall_color = [0.72, 0.7, 0.64];
    let stairwell_color = [0.4, 0.38, 0.35];
    for (center, size_x, size_z) in F2_FLOOR.floor_slab_with_hole(TOWER_SHAFT_X, F2_SHAFT_Z, TOWER_STAIR_WIDTH / 2.0, simple_stairwell_half_len()) {
        panels.push(floor_panel(center, size_x, size_z, ARENA_WALL_THICKNESS, [0.5, 0.48, 0.44], 0.8));
    }
    for (center, size_x, size_z) in office_walls() {
        panels.push(wall_panel(center, size_x, size_z, F2_WALL_HEIGHT, f2_wall_color, 0.7));
    }
    // Cầu thang đơn giản G→F2 — thay đường dốc nằm xéo cũ, xem
    // stairwell_g_to_f2's doc comment.
    for (center, rotation, size) in stairwell_g_to_f2() {
        panels.push(ramp_panel(center, rotation, size.x, size.y, size.z, stairwell_color, 0.9));
    }
    let f2_desk_size = F2_DESK_SIZE;
    for &(x, z) in &F2_DESK_POSITIONS {
        pieces.push((Vec3::new(x, F2_ELEVATION + f2_desk_size.y / 2.0, z), ident, f2_desk_size, [0.55, 0.4, 0.25], 0.6));
    }
    for (center, size_x, size_y, size_z) in office_room_walls().into_iter().chain(locker_room_walls()) {
        panels.push(wall_panel4(center, size_x, size_y, size_z, [0.35, 0.5, 0.55], 0.65));
    }

    // --- F3 Office (sustain peak) — chồng thật ngay trên F2 ---
    let f3_wall_color = [0.68, 0.62, 0.6];
    for (center, size_x, size_z) in F3_FLOOR.floor_slab_with_hole(TOWER_SHAFT_X, F3_SHAFT_Z, TOWER_STAIR_WIDTH / 2.0, simple_stairwell_half_len()) {
        panels.push(floor_panel(center, size_x, size_z, ARENA_WALL_THICKNESS, [0.48, 0.46, 0.42], 0.8));
    }
    for (center, size_x, size_z) in f3_walls() {
        panels.push(wall_panel(center, size_x, size_z, F3_WALL_HEIGHT, f3_wall_color, 0.7));
    }
    // Cầu thang F2→F3 — cùng 1 trục X/Z với đoạn G→F2 ở trên (1 đường đi
    // liên tục), chỉ khác đoạn cao độ.
    for (center, rotation, size) in stairwell_f2_to_f3() {
        panels.push(ramp_panel(center, rotation, size.x, size.y, size.z, stairwell_color, 0.9));
    }
    for (center, size_x, size_y, size_z) in meeting_room_walls() {
        panels.push(wall_panel4(center, size_x, size_y, size_z, [0.55, 0.65, 0.7], 0.4));
    }
    for (center, size_x, size_y, size_z) in office_a_walls().into_iter().chain(office_b_walls()) {
        panels.push(wall_panel4(center, size_x, size_y, size_z, [0.55, 0.45, 0.35], 0.65));
    }

    // Vệt kẻ vàng đánh dấu ranh giới Khu A / Khu B trên sàn Gara — tấm
    // trang trí thuần (không collider, đè lên sàn Khu A/B đã có collider
    // riêng rồi nên không cần chặn gì thêm). Rectangle nằm mặc định
    // trong mặt phẳng XY (thẳng đứng), nên xoay -90° quanh trục X để nó
    // nằm PHẲNG xuống sàn (mặt phẳng XZ) thay vì dựng đứng như 1 tấm biển.
    let flat_on_floor = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
    panels.push((Vec3::new(0.0, 0.02, khu_boundary_z), flat_on_floor, GARAGE_HALF_SIZE_X * 2.0, 1.0, [1.0, 0.85, 0.1], 0.5, None, false));

    (pieces, panels)
}

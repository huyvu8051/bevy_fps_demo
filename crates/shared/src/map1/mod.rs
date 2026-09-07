// --- Map 1 "Tòa nhà văn phòng" — hạ tầng dùng chung cho mọi tầng ---
// Trước đây mỗi tầng (garage.rs/lobby.rs/office.rs) tự tay tính lại công
// thức "2 tường chừa khoảng hở khớp bề rộng dốc" + "1 khối dốc nghiêng nối
// 2 tầng" — 3 bản gần như y hệt nhau, chỉ khác vài con số. Module này gom
// phần đó thành 1 chỗ (FloorLevel + ramp_between/ramp_guard_walls_between),
// nên xây thêm tầng mới (F3, F4, Mái...) chỉ cần khai 1 FloorLevel + viết
// nội thất riêng của tầng đó — không phải chép lại phần lượng giác/khoảng
// hở tường lần nữa. Xem garage.rs/lobby.rs/office.rs để thấy cách dùng.
//
// LƯU Ý: sửa 1 hằng số/công thức ở đây (hoặc trong f3.rs/garage.rs/
// lobby.rs/office.rs) không tự động đổi hình học lúc chơi game nữa —
// `map1_level_pieces()` (cuối file) mới là đường chạy chính, đọc từ
// `crates/client/assets/data/map1_level_pieces.ron` (snapshot đông cứng
// của `map1_level_pieces_computed()`). Sửa xong, chạy `cargo test -p
// shared regenerate_level_piece_ron -- --ignored --nocapture`
// (level_piece.rs) để bake thay đổi vào file RON đó.

use bevy::prelude::{Quat, Vec3};

use crate::arena::ARENA_WALL_THICKNESS;
use crate::level_piece::{floor_panel, ramp_panel, wall_panel, BoxPiece, PanelPiece};

// Chênh cao độ thật giữa 2 tầng liên tiếp — xấp xỉ 1 tầng nhà thực tế (~3m).
pub const FLOOR_HEIGHT: f32 = 3.0;

// Đường dốc nối 2 tầng: chiều dài mặt bằng (hình chiếu ngang) 9m, cùng bề
// rộng với khoảng hở tường ở cả 2 đầu. 9m ngang / FLOOR_HEIGHT cao ≈ góc
// dốc 18.4° — thoải mái đi bộ, còn dư nhiều so với giới hạn ~53° của
// controller (GROUND_NORMAL_MIN_Y trong player.rs).
pub const RAMP_RUN: f32 = 9.0;
pub const RAMP_WIDTH: f32 = 4.0;

// Simplified Map 1 (decision 2026-09-07, corrected same day — floors
// must stack, see below): 4 identical square floors (B1 Garage / G
// Lobby / F2 Office / F3 Office), no per-floor custom rooms. First cut
// used ramp_between()/next_north_wall_z(), which offsets each floor
// along Z (that's the right tool for 2 floors joined by a diagonal
// ramp, e.g. the old B1-to-G connection) — but stacked ON TOP of 4
// times over, it sprawled the "building" across ~90m of Z instead of
// actually stacking, which doesn't read as a building at all. Real
// fix: all 4 share the SAME north_wall_z (so same X/Z footprint,
// directly on top of each other, only `elevation` differs) — the
// existing `simple_stairwell`/`floor_slab_with_hole` machinery (already
// in this file, originally written for the old G→F2→F3 vertical
// stretch) is the correct connector for stacked floors, not
// ramp_between. spawn_map1_lights (crate::client's lib.rs) reads these
// same constants for light placement, so geometry and lighting can't
// drift apart.
pub const MAP1_HALF_X: f32 = 10.0;
pub const MAP1_HALF_Z: f32 = 10.0;
pub const MAP1_WALL_HEIGHT: f32 = 3.0;
pub const MAP1_NORTH_WALL_Z: f32 = -MAP1_HALF_Z;

pub const MAP1_B1: FloorLevel = FloorLevel::new(MAP1_HALF_X, MAP1_HALF_Z, MAP1_WALL_HEIGHT, 0.0, MAP1_NORTH_WALL_Z);
pub const MAP1_G: FloorLevel = FloorLevel::new(MAP1_HALF_X, MAP1_HALF_Z, MAP1_WALL_HEIGHT, FLOOR_HEIGHT, MAP1_NORTH_WALL_Z);
pub const MAP1_F2: FloorLevel = FloorLevel::new(MAP1_HALF_X, MAP1_HALF_Z, MAP1_WALL_HEIGHT, FLOOR_HEIGHT * 2.0, MAP1_NORTH_WALL_Z);
pub const MAP1_F3: FloorLevel = FloorLevel::new(MAP1_HALF_X, MAP1_HALF_Z, MAP1_WALL_HEIGHT, FLOOR_HEIGHT * 3.0, MAP1_NORTH_WALL_Z);

// Stairwell shaft position — same X/Z reused for every floor pair (1
// continuous vertical shaft, not a new one per pair), reusing the
// existing TOWER_SHAFT_X convention. Z = the shared center_z (all 4
// floors have the same one now), simplest choice with no interior rooms
// left to dodge.
pub const MAP1_STAIR_SHAFT_Z: f32 = MAP1_B1.center_z();

// Player spawn — just inside B1's solid North wall, near the West side
// (matches the old GARAGE_PLAYER_SPAWN's "Northwest corner, near the
// entrance" placement). Y=3.0 so the player drops onto the real floor
// instead of guessing the exact touchdown height.
pub const MAP1_PLAYER_SPAWN: Vec3 = Vec3::new(-7.0, 3.0, MAP1_B1.north_wall_z + 3.0);

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

    /// Như `floor_slab_with_hole`, nhưng đục 2 lỗ CÙNG dải Z (khác tâm X)
    /// — dùng khi 1 tầng có cả cầu thang chính LẪN cầu thang thoát hiểm
    /// (Fire Escape, xem GDD/`docs/maps/map01_highrise.md` mục "Lối thoát
    /// hiểm") chạy song song, cùng chiều dài đường dốc nên trùng dải Z.
    /// Người gọi tự đảm bảo 2 lỗ không chồng nhau (hole_a bên Đông, hole_b
    /// bên Tây) — chỉ là hình học thuần túy, không tự kiểm tra.
    pub fn floor_slab_with_two_holes(
        &self,
        hole_a_x: f32,
        hole_b_x: f32,
        hole_center_z: f32,
        hole_half_x: f32,
        hole_half_z: f32,
    ) -> [(Vec3, f32, f32); 5] {
        let y = self.elevation - ARENA_WALL_THICKNESS / 2.0;
        let (ax0, ax1) = (hole_a_x - hole_half_x, hole_a_x + hole_half_x);
        let (bx0, bx1) = (hole_b_x - hole_half_x, hole_b_x + hole_half_x);
        let (hz0, hz1) = (hole_center_z - hole_half_z, hole_center_z + hole_half_z);
        let (fx0, fx1) = (-self.half_size_x, self.half_size_x);
        let (fz0, fz1) = (self.north_wall_z, self.south_wall_z());
        [
            // Bắc: mép Bắc sàn → mép Bắc lỗ, đủ bề rộng X (cả 2 lỗ đều
            // nằm trong dải Z này nên không cần chia theo lỗ).
            (Vec3::new(0.0, y, (fz0 + hz0) / 2.0), fx1 - fx0, hz0 - fz0),
            // Nam: mép Nam lỗ → mép Nam sàn.
            (Vec3::new(0.0, y, (hz1 + fz1) / 2.0), fx1 - fx0, fz1 - hz1),
            // Tây xa: mép Tây sàn → mép Tây lỗ B (lỗ B ở phía Tây hơn).
            (Vec3::new((fx0 + bx0) / 2.0, y, hole_center_z), bx0 - fx0, hz1 - hz0),
            // Giữa 2 lỗ.
            (Vec3::new((bx1 + ax0) / 2.0, y, hole_center_z), ax0 - bx1, hz1 - hz0),
            // Đông xa: mép Đông lỗ A → mép Đông sàn.
            (Vec3::new((ax1 + fx1) / 2.0, y, hole_center_z), fx1 - ax1, hz1 - hz0),
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

/// (position, yaw radians) — 1 điểm spawn zombie ĐÃ ĐƯỢC KIỂM CHỨNG AN
/// TOÀN, đọc từ `crates/client/assets/data/map1_zombie_spawns.ron`.
///
/// Trước đây hàm này (`random_map1_zombie_spawn_pos`) rải ngẫu nhiên theo
/// R2 low-discrepancy trong biên NGOÀI của cả tầng — an toàn hồi tầng còn
/// là 1 sàn phẳng liền mạch, nhưng từ khi tầng chồng thật xuyên giếng
/// thang (`floor_slab_with_two_holes`, có LỖ THẬT giữa sàn), random thuần
/// theo biên ngoài có thể rơi trúng lỗ — zombie spawn giữa không trung,
/// không có sàn bên dưới nên không rơi xuống (zombie không có RigidBody
/// vật lý thật, xem `spawn_zombies` — di chuyển do AI/replication tự
/// set Transform, không phải do gravity mô phỏng). Bug này báo trực tiếp
/// trong lúc chơi thật ("zombie bị spawn trên trời và không rơi xuống").
///
/// Sửa bằng cách bỏ hẳn việc TÍNH TOÁN vị trí lúc runtime — thay bằng 1
/// danh sách vị trí CỐ ĐỊNH, đã kiểm chứng nằm trên sàn thật (tránh cả 2
/// giếng thang), đọc từ RON. Vẫn giữ tính "seed xoay vòng" (cùng 1 seed
/// luôn ra cùng 1 điểm, seed tăng dần đi hết danh sách rồi quay lại đầu)
/// — chỉ khác nguồn dữ liệu, không đổi cách gọi ở server/main.rs.
pub fn map1_zombie_spawn_pos(seed: usize) -> (Vec3, f32) {
    let points = map1_zombie_spawn_points();
    points[seed % points.len()]
}

/// Đọc + cache `map1_zombie_spawns.ron` — mỗi dòng `(x, y, z, yaw_rad)`.
fn map1_zombie_spawn_points() -> &'static Vec<(Vec3, f32)> {
    static DATA: std::sync::OnceLock<Vec<(Vec3, f32)>> = std::sync::OnceLock::new();
    DATA.get_or_init(|| {
        let text = crate::read_ron_data_file!("map1_zombie_spawns.ron");
        let rows: Vec<(f32, f32, f32, f32)> =
            ron::de::from_str(&text).unwrap_or_else(|e| panic!("failed to parse map1_zombie_spawns.ron: {e}"));
        rows.into_iter().map(|(x, y, z, yaw)| (Vec3::new(x, y, z), yaw)).collect()
    })
}

/// Tính toán tham số hoá danh sách spawn point an toàn cho `map1_zombie_
/// spawns.ron` — KHÔNG phải đường chạy chính lúc chơi game (đó là `map1_
/// zombie_spawn_pos`, đọc RON), chỉ được `regenerate_map1_zombie_spawns_
/// ron` (test dưới, `--ignored`) gọi để (tái) sinh file RON mỗi khi hình
/// học Map 1 đổi. Cùng tinh thần "Rust tính, RON lưu kết quả" như
/// `map1_level_pieces_computed`/`level_pieces_to_ron`.
///
/// 7 điểm/tầng, chọn thủ công để CHẮC CHẮN né cả 2 lỗ giếng thang (mỗi lỗ
/// rộng `TOWER_STAIR_WIDTH` quanh `TOWER_SHAFT_X`/`FIRE_ESCAPE_SHAFT_X`,
/// dài `simple_ramp_stacked_half_len()*2` quanh `MAP1_STAIR_SHAFT_Z`) —
/// không phải random/thuật toán tổng quát, vì chỉ có đúng 1 layout tầng
/// (4 tầng giống hệt nhau) cần tính, làm thuật toán né-lỗ tổng quát cho
/// đúng 1 layout là phí công hơn liệt kê tay. Hướng nhìn ban đầu (yaw)
/// quay mặt vào tâm phòng — chỉ mang tính thẩm mỹ lúc vừa spawn, AI
/// (`ChaseAction`) ghi đè hướng thật ngay khi phát hiện người chơi.
pub fn map1_zombie_spawn_points_computed() -> Vec<(Vec3, f32)> {
    // Cách tâm (0, MAP1_STAIR_SHAFT_Z) đủ xa để không dính cả 2 lỗ dù ở
    // cạnh nào: |x|=8 > TOWER_SHAFT_X(6) + TOWER_STAIR_WIDTH/2(1.5) =
    // 7.5, và |z|=8 > simple_ramp_stacked_half_len()(3.0) — kiểm tra lại
    // nếu đổi TOWER_SHAFT_X/TOWER_STAIR_WIDTH/MAP1_RAMP_RUN.
    const OFFSETS: [(f32, f32); 7] = [(0.0, 0.0), (8.0, 8.0), (8.0, -8.0), (-8.0, 8.0), (-8.0, -8.0), (0.0, 8.0), (0.0, -8.0)];

    let floors: [FloorLevel; 4] = [MAP1_B1, MAP1_G, MAP1_F2, MAP1_F3];
    let mut points = Vec::with_capacity(OFFSETS.len() * floors.len());
    // Vòng ngoài chạy theo OFFSETS, vòng trong theo floors (không phải
    // ngược lại) — để danh sách XEN KẼ theo tầng (B1,G,F2,F3, B1,G,F2,F3,
    // ...) thay vì gom hết 1 tầng rồi mới sang tầng khác. seed tăng dần
    // (`map1_zombie_spawn_pos`, cycling `seed % len`) nên thứ tự này mới
    // đảm bảo N zombie đầu tiên trải đều cả 4 tầng ngay từ đầu — gom theo
    // tầng sẽ làm ZombieTargetCount mặc định (10) chỉ lấp đầy B1 rồi mới
    // tràn sang G, chưa từng chạm F2/F3 — đúng hành vi round-robin hàm cũ
    // (`random_map1_zombie_spawn_pos`, chọn tầng qua `seed % 4`) đã có.
    for (dx, dz) in OFFSETS {
        for floor in floors {
            let y = floor.elevation + crate::zombie::ZOMBIE_RADIUS + crate::zombie::ZOMBIE_LENGTH / 2.0;
            let x = dx;
            let z = MAP1_STAIR_SHAFT_Z + dz;
            // Quay mặt về tâm phòng — atan2(-dx, -dz) khớp quy ước
            // Quat::from_rotation_y (yaw=0 nhìn về -Z, xem movement.rs).
            let yaw = (-dx).atan2(-dz);
            points.push((Vec3::new(x, y, z), yaw));
        }
    }
    points
}

/// `MapKind::Campaign(map_id)` counterpart to `map1_zombie_spawn_pos` —
/// same generalization relationship as `campaign_level_pieces` has to
/// `map1_level_pieces` above. This is ONLY the sandbox-style "scatter
/// baseline population" spawn (`ZOMBIE_SPAWNS`-equivalent) — real campaign
/// play should use the scripted wave system instead (see
/// `docs/ZOMBIE_SPAWN_SYSTEM.md`, not implemented yet), which doesn't call
/// this at all. This function only exists so `MapKind::Campaign` maps
/// still have SOME zombie spawn behavior for dev/debug purposes (F2
/// editor, manual `SetZombieCount`) before their real wave data exists.
/// Only `map_id: 1` has real floor bounds to scatter within; any other id
/// panics — same reasoning as `campaign_level_pieces`' panic branch.
pub fn campaign_zombie_spawn_pos(map_id: u8, seed: usize) -> (Vec3, f32) {
    if map_id == 1 {
        return map1_zombie_spawn_pos(seed);
    }
    panic!(
        "no sandbox zombie spawn bounds for campaign map_id {map_id} — \
         only map_id 1 has floor geometry today; this map needs its wave \
         data (docs/ZOMBIE_SPAWN_SYSTEM.md) before real play anyway, not a \
         sandbox scatter fallback"
    )
}

#[cfg(test)]
mod regenerate_spawns {
    use super::*;

    /// `cargo test -p shared regenerate_map1_zombie_spawns_ron -- --ignored --nocapture`
    /// — re-derives `map1_zombie_spawns.ron` from `map1_zombie_spawn_
    /// points_computed()`, same pattern as `level_piece::regenerate::
    /// regenerate_level_piece_ron`. Run after changing Map 1's floor
    /// geometry (footprint size, stairwell shaft positions) so the safe
    /// spawn points stay actually safe.
    #[test]
    #[ignore]
    fn regenerate_map1_zombie_spawns_ron() {
        let points = map1_zombie_spawn_points_computed();
        let rows: Vec<(f32, f32, f32, f32)> = points.iter().map(|(p, yaw)| (p.x, p.y, p.z, *yaw)).collect();
        let text = ron::ser::to_string_pretty(&rows, ron::ser::PrettyConfig::default().struct_names(false))
            .expect("serializing map1 zombie spawns to RON");
        let path = format!("{}/../client/assets/data/map1_zombie_spawns.ron", env!("CARGO_MANIFEST_DIR"));
        std::fs::write(&path, text).unwrap_or_else(|e| panic!("failed to write {path}: {e}"));
        println!("wrote {path} ({} points)", rows.len());
    }
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
/// Thang thoát hiểm (Fire Escape, xem GDD/`docs/maps/map01_highrise.md`
/// mục "Mái nhà & Lối thoát hiểm") — 1 giếng thang RIÊNG, chạy song song
/// với thang chính, phía bên Tây tòa nhà (đối xứng X qua tâm). Cùng chiều
/// dài/độ dốc/dải Z với thang chính (`simple_ramp_stacked` dùng chung cho
/// cả 2, chỉ khác shaft_x) nên `floor_slab_with_two_holes` đục được cả 2
/// lỗ cùng lúc mà không cần tính lại dải Z riêng.
pub const FIRE_ESCAPE_SHAFT_X: f32 = -TOWER_SHAFT_X;
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

// Đơn giản hoá thêm 1 bước (quyết định 2026-09-07, cùng ngày với việc
// chồng tầng thẳng): bỏ hẳn cầu thang 2 nhịp + chiếu nghỉ, thay bằng
// ĐÚNG 1 đường dốc chéo thẳng nối 2 tầng — không rẽ, không chiếu nghỉ,
// đơn giản tối đa như yêu cầu. `simple_stairwell` bên dưới vẫn giữ lại
// (không xoá, không hại gì) phòng khi sau này muốn cầu thang có chiếu
// nghỉ trở lại, nhưng `map1_level_pieces_computed` giờ gọi
// `simple_ramp_stacked` thay vào đó.
// 6m ngang cho 3m cao -> ~26.6° — leo được thoải mái, không bị hụt tốc
// độ đi bộ như ~45° ban đầu (báo trực tiếp là "hơi khó leo"). Vẫn còn dư
// nhiều so với giới hạn ~53° của controller (GROUND_NORMAL_MIN_Y,
// player.rs) và vẫn nằm gọn trong nửa sàn (half_run=3.0 < MAP1_HALF_Z=10).
pub const MAP1_RAMP_RUN: f32 = FLOOR_HEIGHT * 2.0;

/// (center, rotation, full-size) cho ĐÚNG 1 đường dốc chéo nối `from` lên
/// `to` (2 tầng CHỒNG THẬT, cùng tâm X/Z) — không rẽ, không chiếu nghỉ.
pub fn simple_ramp_stacked(shaft_x: f32, shaft_z: f32, from: &FloorLevel, to: &FloorLevel) -> (Vec3, Quat, Vec3) {
    let half_run = MAP1_RAMP_RUN / 2.0;
    ramp_segment(
        Vec3::new(shaft_x, from.elevation, shaft_z - half_run),
        Vec3::new(shaft_x, to.elevation, shaft_z + half_run),
        TOWER_STAIR_WIDTH,
        ARENA_WALL_THICKNESS,
    )
}

/// Nửa chiều dài (dọc Z) đường dốc chiếm — dùng để đục lỗ sàn tầng trên
/// đúng khớp (xem FloorLevel::floor_slab_with_hole).
pub const fn simple_ramp_stacked_half_len() -> f32 {
    MAP1_RAMP_RUN / 2.0
}

/// Cầu thang 2 nhịp + chiếu nghỉ — KHÔNG còn được `map1_level_pieces_
/// computed` gọi nữa (xem `simple_ramp_stacked` ở trên), giữ lại làm tuỳ
/// chọn dự phòng nếu sau này muốn quay lại kiểu có chiếu nghỉ.
#[allow(dead_code)]
pub fn simple_stairwell(shaft_x: f32, shaft_z: f32, from: &FloorLevel, to: &FloorLevel) -> Vec<(Vec3, Quat, Vec3)> {
    let mid_elevation = (from.elevation + to.elevation) / 2.0;
    let half_landing = TOWER_STAIR_LANDING_RUN / 2.0;
    let z0 = shaft_z - half_landing - TOWER_STAIR_RUN;
    let z1 = shaft_z - half_landing;
    let z2 = shaft_z + half_landing;
    let z3 = shaft_z + half_landing + TOWER_STAIR_RUN;

    let ramp1 = ramp_segment(Vec3::new(shaft_x, from.elevation, z0), Vec3::new(shaft_x, mid_elevation, z1), TOWER_STAIR_WIDTH, ARENA_WALL_THICKNESS);
    let ramp2 = ramp_segment(Vec3::new(shaft_x, mid_elevation, z2), Vec3::new(shaft_x, to.elevation, z3), TOWER_STAIR_WIDTH, ARENA_WALL_THICKNESS);

    // `Quat::IDENTITY`, NOT the already-flat `Rx(-90°)` — same "raw"
    // convention ramp1/ramp2 use (their rotation is just heading+tilt,
    // no "lie flat" baked in yet). The caller always finishes every
    // piece here through ramp_panel(), which adds its OWN `Rx(-90°)` —
    // passing an already-flat rotation here made that happen TWICE
    // (net `Rx(-180°)`), standing the landing up like a wall instead of
    // lying it flat (reported live as "chiếu nghỉ nằm dọc"). No heading/
    // tilt needed for a flat landing anyway (zero rotation IS the raw
    // form here), so identity is also simply correct, not just
    // consistent.
    let landing_centerline = Vec3::new(shaft_x, mid_elevation, shaft_z);
    let landing = (landing_centerline, Quat::IDENTITY, Vec3::new(TOWER_STAIR_WIDTH, ARENA_WALL_THICKNESS, TOWER_STAIR_LANDING_RUN));

    vec![ramp1, landing, ramp2]
}

/// Nửa chiều dài (dọc Z) toàn bộ khoảng cầu thang chiếm — dùng để đục lỗ
/// sàn tầng trên đúng khớp (xem FloorLevel::floor_slab_with_hole).
pub const fn simple_stairwell_half_len() -> f32 {
    TOWER_STAIR_RUN + TOWER_STAIR_LANDING_RUN / 2.0
}

/// Toàn bộ hình học Map 1 (B1 Gara → G Sảnh → F2 → F3) TÍNH TOÁN TỪ ĐẦU
/// bằng các hằng số/công thức tham số hoá bên dưới — (BoxPiece cho nội
/// thất đặc như xe/bàn, PanelPiece cho tường/sàn/dốc) — KHÔNG tự spawn gì
/// cả. KHÔNG còn là đường chạy chính lúc chơi game nữa — `map1_level_
/// pieces()` (đọc RON, cuối file) mới là hàm client/server thực sự gọi.
/// Hàm này giờ chỉ được `regenerate_level_piece_ron` (test dưới
/// level_piece.rs) gọi, để (tái) sinh `map1_level_pieces.ron` mỗi khi 1
/// hằng số đo đạc (docs/maps/*) đổi — xem level_piece.rs's module doc
/// comment cho lý do tách RON ra khỏi việc tính toán.
pub fn map1_level_pieces_computed() -> (Vec<BoxPiece>, Vec<PanelPiece>) {
    let pieces: Vec<BoxPiece> = Vec::new();
    let mut panels: Vec<PanelPiece> = Vec::new();
    let floor_color = [0.5, 0.5, 0.52];
    let wall_color = [0.6, 0.58, 0.55];
    let stair_color = [0.45, 0.43, 0.4];
    // Đỏ cam cảnh báo — khớp màu sơn/biển báo thang thoát hiểm thật ngoài
    // đời, giúp người chơi phân biệt ngay với thang chính (màu xám trung
    // tính ở trên) chỉ bằng mắt.
    let fire_escape_color = [0.75, 0.25, 0.15];

    // B1 Garage — ground floor: solid slab (nothing below to cut a
    // stairwell hole for), walls fully enclosed (the only way up is the
    // stairwell hole in G's floor above, not a wall gap).
    let (c, sx, sz) = MAP1_B1.floor_slab();
    panels.push(floor_panel(c, sx, sz, ARENA_WALL_THICKNESS, floor_color, 0.85));
    for (c, sx, sz) in MAP1_B1.walls_enclosed() {
        panels.push(wall_panel(c, sx, sz, MAP1_B1.wall_height, wall_color, 0.75));
    }

    // G / F2 / F3 — each stacks DIRECTLY on the floor below (same X/Z
    // footprint, see MAP1_B1..MAP1_F3's own doc comment for why this
    // replaced the earlier Z-offset layout): floor slab has a stairwell
    // hole cut in it, walls fully enclosed, connected down to the floor
    // below by simple_stairwell's real 2-flight stair + landing.
    for (level, below) in [(MAP1_G, MAP1_B1), (MAP1_F2, MAP1_G), (MAP1_F3, MAP1_F2)] {
        for (c, sx, sz) in level.floor_slab_with_two_holes(
            TOWER_SHAFT_X,
            FIRE_ESCAPE_SHAFT_X,
            MAP1_STAIR_SHAFT_Z,
            TOWER_STAIR_WIDTH / 2.0,
            simple_ramp_stacked_half_len(),
        ) {
            panels.push(floor_panel(c, sx, sz, ARENA_WALL_THICKNESS, floor_color, 0.85));
        }
        for (c, sx, sz) in level.walls_enclosed() {
            panels.push(wall_panel(c, sx, sz, level.wall_height, wall_color, 0.75));
        }
        let (center, rotation, size) = simple_ramp_stacked(TOWER_SHAFT_X, MAP1_STAIR_SHAFT_Z, &below, &level);
        panels.push(ramp_panel(center, rotation, size.x, size.y, size.z, stair_color, 0.9));
        let (center, rotation, size) = simple_ramp_stacked(FIRE_ESCAPE_SHAFT_X, MAP1_STAIR_SHAFT_Z, &below, &level);
        panels.push(ramp_panel(center, rotation, size.x, size.y, size.z, fire_escape_color, 0.9));
    }

    (pieces, panels)
}

/// Toàn bộ hình học Map 1, đọc từ `crates/client/assets/data/
/// map1_level_pieces.ron` — đường chạy CHÍNH client/server thực sự gọi
/// (`map1_level_pieces_computed()` ở trên chỉ còn là nguồn sinh ra file
/// RON này, xem doc comment của nó). Đọc 1 lần rồi cache, giống hệt
/// `arena_layout`/`item_gallery_car_physics_raw`.
///
/// Giữ lại tên riêng cho Map 1 cụ thể (thay vì gọi thẳng
/// `campaign_level_pieces(1)`) vì đây vẫn là map DUY NHẤT có 1 bộ builder
/// tham số hoá thật (`map1_level_pieces_computed` ở trên) đứng sau file
/// RON này — regenerate_level_piece_ron (level_piece.rs) cần 1 cái tên cụ
/// thể để gọi, không đi qua map_id chung.
pub fn map1_level_pieces() -> (Vec<BoxPiece>, Vec<PanelPiece>) {
    static DATA: std::sync::OnceLock<(Vec<BoxPiece>, Vec<PanelPiece>)> = std::sync::OnceLock::new();
    DATA.get_or_init(|| {
        let text = crate::read_ron_data_file!("map1_level_pieces.ron");
        crate::level_piece::level_pieces_from_ron(&text)
    })
    .clone()
}

/// Hình học của `MapKind::Campaign(map_id)` bất kỳ, đọc từ
/// `crates/client/assets/data/map<map_id>_level_pieces.ron` — bản tổng
/// quát hoá của `map1_level_pieces()` ở trên theo đúng cách `arena_layout`
/// (arena.rs) đã tổng quát hoá arena theo `map_id`: thêm map campaign mới
/// (2-20, xem `docs/maps/mapNN_dimensions.md`) chỉ cần thêm 1 file RON,
/// KHÔNG cần thêm hàm/nhánh Rust nào — mọi call site (server's
/// `spawn_campaign_level_pieces`, client's map-switch handlers) đều gọi
/// đúng 1 hàm này bất kể `map_id` là bao nhiêu.
///
/// `map_id: 1` vẫn thực chất đọc đúng `map1_level_pieces.ron` (chuyển tiếp
/// qua `map1_level_pieces()` ở trên, không đọc lại file — giữ đúng 1
/// nguồn thật duy nhất cho Map 1). Mọi `map_id` khác panic rõ ràng — CHƯA
/// có map nào trong số 2-20 được code thật (chỉ mới có
/// `docs/maps/mapNN_dimensions.md` ở mức thiết kế), thêm 1 map nghĩa là
/// tạo `map<id>_level_pieces.ron` (viết tay theo đúng dữ liệu đã có trong
/// doc thiết kế map đó, giống cách `arena_layout_map2.ron` được viết tay/
/// F9-save, không cần 1 `map<id>_level_pieces_computed()` tham số hoá như
/// Map 1 đã có — quá tốn công cho 19 map còn lại) rồi xoá nhánh panic của
/// đúng id đó bên dưới.
pub fn campaign_level_pieces(map_id: u8) -> (Vec<BoxPiece>, Vec<PanelPiece>) {
    #[cfg(not(target_arch = "wasm32"))]
    use std::collections::HashMap;
    #[cfg(not(target_arch = "wasm32"))]
    use std::sync::{Mutex, OnceLock};
    if map_id == 1 {
        return map1_level_pieces();
    }
    #[cfg(target_arch = "wasm32")]
    panic!(
        "no wasm-embedded level pieces RON for campaign map_id {map_id} — \
         only map_id 1 has one today; add a match arm here once map<id>_level_pieces.ron exists \
         (same as arena.rs's load_arena_layout does per Arena map id)"
    );
    #[cfg(not(target_arch = "wasm32"))]
    {
        static CACHE: OnceLock<Mutex<HashMap<u8, (Vec<BoxPiece>, Vec<PanelPiece>)>>> = OnceLock::new();
        let mut cache = CACHE.get_or_init(|| Mutex::new(HashMap::new())).lock().expect("campaign level pieces cache poisoned");
        if let Some(pieces) = cache.get(&map_id) {
            return pieces.clone();
        }
        // Real iOS device needs NSBundle's own resource path — see
        // read_ron_data_file!'s own doc comment (crate root) for the
        // device crash this exact CARGO_MANIFEST_DIR pattern caused
        // elsewhere; this loader is map_id-dynamic so it can't route
        // through that macro (same reasoning as arena.rs's
        // load_arena_layout, fixed the same way by hand).
        #[cfg(target_os = "ios")]
        let text = {
            let resource_path = crate::ios_bundle_resource_path().unwrap_or_else(|| {
                panic!("no level pieces RON for campaign map_id {map_id}: NSBundle.main.resourcePath was nil")
            });
            let path = std::path::Path::new(&resource_path).join(format!("assets/data/map{map_id}_level_pieces.ron"));
            std::fs::read_to_string(&path).unwrap_or_else(|e| {
                panic!(
                    "no level pieces RON for campaign map_id {map_id} at {path:?}: {e} — \
                     this map isn't implemented yet, see docs/maps/map{map_id:02}_dimensions.md \
                     for its design numbers"
                )
            })
        };
        #[cfg(not(target_os = "ios"))]
        let text = {
            let path = format!("{}/../client/assets/data/map{map_id}_level_pieces.ron", env!("CARGO_MANIFEST_DIR"));
            std::fs::read_to_string(&path).unwrap_or_else(|e| {
                panic!(
                    "no level pieces RON for campaign map_id {map_id} at {path}: {e} — \
                     this map isn't implemented yet, see docs/maps/map{map_id:02}_dimensions.md \
                     for its design numbers"
                )
            })
        };
        let pieces = crate::level_piece::level_pieces_from_ron(&text);
        cache.insert(map_id, pieces.clone());
        pieces
    }
}

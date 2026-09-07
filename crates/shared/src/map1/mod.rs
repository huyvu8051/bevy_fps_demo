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
use crate::level_piece::{floor_panel, ramp_panel, wall_panel, wall_panel4, BoxPiece, PanelPiece};

// Chênh cao độ thật giữa 2 tầng liên tiếp — xấp xỉ 1 tầng nhà thực tế (~3m).
pub const FLOOR_HEIGHT: f32 = 3.0;

// Đường dốc nối 2 tầng: chiều dài mặt bằng (hình chiếu ngang) 9m, cùng bề
// rộng với khoảng hở tường ở cả 2 đầu. 9m ngang / FLOOR_HEIGHT cao ≈ góc
// dốc 18.4° — thoải mái đi bộ, còn dư nhiều so với giới hạn ~53° của
// controller (GROUND_NORMAL_MIN_Y trong player.rs).
pub const RAMP_RUN: f32 = 9.0;
pub const RAMP_WIDTH: f32 = 4.0;

// Simplified Map 1 (decision 2026-09-07): 4 identical square floors (B1
// Garage / G Lobby / F2 Office / F3 Office) instead of the old hand-built
// per-floor rooms (car grid, reception desk, cubicles, locker/meeting
// rooms) — just floor_slab() + walls_*_floor() + ramp_between() per pair,
// straight from FloorLevel's own generic helpers, no per-floor custom
// layout code at all. spawn_map1_lights (crate::client's lib.rs) reads
// these same 4 constants for light placement, so geometry and lighting
// can't drift apart the way the old per-submodule constants could.
pub const MAP1_HALF_X: f32 = 10.0;
pub const MAP1_HALF_Z: f32 = 10.0;
pub const MAP1_WALL_HEIGHT: f32 = 3.0;

pub const MAP1_B1: FloorLevel = FloorLevel::new(MAP1_HALF_X, MAP1_HALF_Z, MAP1_WALL_HEIGHT, 0.0, -MAP1_HALF_Z);
pub const MAP1_G: FloorLevel = FloorLevel::new(MAP1_HALF_X, MAP1_HALF_Z, MAP1_WALL_HEIGHT, FLOOR_HEIGHT, MAP1_B1.next_north_wall_z());
pub const MAP1_F2: FloorLevel = FloorLevel::new(MAP1_HALF_X, MAP1_HALF_Z, MAP1_WALL_HEIGHT, FLOOR_HEIGHT * 2.0, MAP1_G.next_north_wall_z());
pub const MAP1_F3: FloorLevel = FloorLevel::new(MAP1_HALF_X, MAP1_HALF_Z, MAP1_WALL_HEIGHT, FLOOR_HEIGHT * 3.0, MAP1_F2.next_north_wall_z());

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

    let floors: [FloorLevel; 4] = [MAP1_B1, MAP1_G, MAP1_F2, MAP1_F3];
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

/// `MapKind::Campaign(map_id)` counterpart to `random_map1_zombie_spawn_pos`
/// — same generalization relationship as `campaign_level_pieces` has to
/// `map1_level_pieces` above. This is ONLY the sandbox-style "scatter
/// baseline population" spawn (`ZOMBIE_SPAWNS`-equivalent) — real campaign
/// play should use the scripted wave system instead (see
/// `docs/ZOMBIE_SPAWN_SYSTEM.md`, not implemented yet), which doesn't call
/// this at all. This function only exists so `MapKind::Campaign` maps
/// still have SOME zombie spawn behavior for dev/debug purposes (F2
/// editor, manual `SetZombieCount`) before their real wave data exists.
/// Only `map_id: 1` has real floor bounds to scatter within; any other id
/// panics — same reasoning as `campaign_level_pieces`' panic branch.
pub fn campaign_zombie_spawn_pos(map_id: u8, seed: usize) -> Vec3 {
    if map_id == 1 {
        return random_map1_zombie_spawn_pos(seed);
    }
    panic!(
        "no sandbox zombie spawn bounds for campaign map_id {map_id} — \
         only map_id 1 has floor geometry today; this map needs its wave \
         data (docs/ZOMBIE_SPAWN_SYSTEM.md) before real play anyway, not a \
         sandbox scatter fallback"
    )
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

    // B1 Garage — ground floor, gap only South (up to G).
    let (c, sx, sz) = MAP1_B1.floor_slab();
    panels.push(floor_panel(c, sx, sz, ARENA_WALL_THICKNESS, floor_color, 0.85));
    for (c, sx, sz) in MAP1_B1.walls_ground_floor() {
        panels.push(wall_panel(c, sx, sz, MAP1_B1.wall_height, wall_color, 0.75));
    }

    // G Lobby — mid floor (gap North from B1, gap South up to F2).
    let (c, sx, sz) = MAP1_G.floor_slab();
    panels.push(floor_panel(c, sx, sz, ARENA_WALL_THICKNESS, floor_color, 0.85));
    for (c, sx, sz) in MAP1_G.walls_mid_floor() {
        panels.push(wall_panel(c, sx, sz, MAP1_G.wall_height, wall_color, 0.75));
    }
    let (rc, rr, rs) = ramp_between(&MAP1_B1, &MAP1_G);
    panels.push(ramp_panel(rc, rr, rs.x, rs.y, rs.z, wall_color, 0.85));
    for (c, sx, sy, sz) in ramp_guard_walls_between(&MAP1_B1, &MAP1_G) {
        panels.push(wall_panel4(c, sx, sy, sz, wall_color, 0.75));
    }

    // F2 Office — mid floor (gap North from G, gap South up to F3).
    let (c, sx, sz) = MAP1_F2.floor_slab();
    panels.push(floor_panel(c, sx, sz, ARENA_WALL_THICKNESS, floor_color, 0.85));
    for (c, sx, sz) in MAP1_F2.walls_mid_floor() {
        panels.push(wall_panel(c, sx, sz, MAP1_F2.wall_height, wall_color, 0.75));
    }
    let (rc, rr, rs) = ramp_between(&MAP1_G, &MAP1_F2);
    panels.push(ramp_panel(rc, rr, rs.x, rs.y, rs.z, wall_color, 0.85));
    for (c, sx, sy, sz) in ramp_guard_walls_between(&MAP1_G, &MAP1_F2) {
        panels.push(wall_panel4(c, sx, sy, sz, wall_color, 0.75));
    }

    // F3 Office — top floor (gap only North from F2).
    let (c, sx, sz) = MAP1_F3.floor_slab();
    panels.push(floor_panel(c, sx, sz, ARENA_WALL_THICKNESS, floor_color, 0.85));
    for (c, sx, sz) in MAP1_F3.walls_top_floor() {
        panels.push(wall_panel(c, sx, sz, MAP1_F3.wall_height, wall_color, 0.75));
    }
    let (rc, rr, rs) = ramp_between(&MAP1_F2, &MAP1_F3);
    panels.push(ramp_panel(rc, rr, rs.x, rs.y, rs.z, wall_color, 0.85));
    for (c, sx, sy, sz) in ramp_guard_walls_between(&MAP1_F2, &MAP1_F3) {
        panels.push(wall_panel4(c, sx, sy, sz, wall_color, 0.75));
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

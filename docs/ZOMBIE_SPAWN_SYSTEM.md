# Zombie Spawn System — state machine dùng chung mọi campaign map

Thiết kế hệ thống spawn theo nhịp (wave) cho `map01_dimensions.md` →
`map20_dimensions.md` — tài liệu này quyết **cơ chế chạy** (state
machine, dữ liệu, code hook), các file `mapNN_dimensions.md`/`mapNN_
highrise.md`-tương-đương chỉ cung cấp **dữ liệu** (bao nhiêu zombie, ở
đâu, trigger gì) đổ vào cơ chế này.

## 1. Đối chiếu với code hiện có — đừng làm lại cái đã có

Code hiện tại (`crates/server/src/main.rs`, `crates/shared/src/zombie.rs`)
đã có **1 hệ thống spawn hoàn chỉnh nhưng thuộc loại khác** — "sandbox
population", không phải "scripted campaign":

- `ZOMBIE_SPAWNS` (100 vị trí cố định, lưới hoặc `random_zombie_spawn_pos`
  theo R2 sequence) + `ZombieTargetCount` + `on_set_zombie_count`/
  `tick_respawns`: giữ **1 số lượng zombie cố định luôn hồi sinh 1:1**
  khi bị giết — mô hình "hồ bơi zombie ổn định", không có khái niệm
  "đợt"/"trigger theo vị trí".
- **AI Director** (`DIRECTOR_*` const, `run_ai_director`): bơm thêm 1
  cụm 10 zombie khi "áp lực" (số zombie trong tầm cận chiến) thấp liên
  tục 20s — đây LÀ 1 dạng "nhịp" nhưng dựa trên đo lường trạng thái
  chiến đấu hiện tại, không phải kịch bản cố định theo vị trí map.
- **`MapKind`** hiện chỉ có 3 giá trị (`Building` = Map 1, `Arena`,
  `ItemGallery`) — **chưa có khái niệm "Map 2..20"** trong code ở bất
  kỳ đâu.

**Quyết định kiến trúc:** hệ thống mới ở tài liệu này **không thay thế**
population/Director ở trên — 2 hệ thống phục vụ 2 mục đích khác nhau:

| | Sandbox population + Director (đã có) | Scripted wave (tài liệu này) |
|---|---|---|
| Dùng cho | `MapKind::Arena`/`ItemGallery`, chế độ Free Play/stress test | `MapKind::Campaign(map_id)` — Map 1-20 chơi chiến dịch thật |
| Số lượng | Giữ ổn định 1 target, hồi sinh 1:1 | Cố định theo thiết kế từng khu, spawn 1 lần rồi thôi (trừ khi trigger lại) |
| Vị trí | Lưới/random toàn map | Cụm cố định theo zone, do designer đặt tay |
| Khi nào spawn | Ngay từ đầu + khi chết | Khi trigger (vào khu, nhặt item, timer, sự kiện) |

Khi `MapKind::Campaign(_)` được thêm (việc code map 2-20 sau này), cả
`spawn_initial_zombies`/`maintain_zombie_population`/`run_ai_director`
đều **tắt hẳn** cho scenario đó (giống cách `MapKind::Arena` hiện đã có
nhánh riêng) — chỉ hệ thống wave dưới đây chạy.

---

## 2. Dữ liệu — `SpawnWave`

Mỗi dòng trong bảng "Nhịp spawn" ở `mapNN_dimensions.md`/`mapNN_
highrise.md` map 1-1 vào 1 `SpawnWave`:

```rust
pub struct SpawnWave {
    pub id: WaveId,                    // vd "map01.f3.wave3" — duy nhất
                                        // toàn map, dùng làm khoá cho
                                        // "đã trigger chưa" + debug log
    pub kind: ZombieKind,               // zombie thường | biến thể lab |
                                        // Boomer/Hunter/... | lính BRDF |
                                        // boss — biến thể/đặc biệt/boss/
                                        // lính KHÔNG đi qua x12, nhưng vẫn
                                        // dùng chung struct này (count nhỏ)
    pub count: u32,
    pub region: SpawnRegion,            // xem mục 3
    pub trigger: SpawnTrigger,          // xem mục 4
    pub concurrent_cap: Option<u32>,    // None = dùng mặc định toàn cục
                                        // (mục 6); Some(n) override riêng
                                        // (vd boss luôn Some(1))
}

pub enum ZombieKind {
    Regular,        // zombie thường — x12 khi scale diện tích
    LabVariant,      // biến thể phòng TN Map 18 — x12 (cosmetic only)
    Special(SpecialKind),  // Boomer/Hunter/Smoker/Spitter/Jockey/Witch —
                            // KHÔNG x12
    BrdfSoldier,     // lính BRDF — KHÔNG x12
    BrdfCommander,   // chỉ huy Map 19 — KHÔNG x12, luôn count=1
    Boss,            // Tank-style — KHÔNG x12, luôn count=1
}
```

`WaveId` đặt tên theo `mapNN.<zone-slug>.wave<n>` khớp trực tiếp với
mục lục "Nhịp spawn" trong doc thiết kế — tra ngược từ log runtime về
đúng dòng trong `mapNN_dimensions.md` không cần đoán.

## 3. `SpawnRegion` — nơi zombie xuất hiện

2 dạng, khớp đúng 2 cách các file thiết kế đã mô tả:

```rust
pub enum SpawnRegion {
    // Toạ độ đơn — dùng cho đợt nhỏ đã ghi rõ x,y,z (Map 1's B1/G/F2 v1
    // waves, boss vị trí cố định).
    Points(Vec<Vec3>),
    // Cụm — tâm + bán kính rải, dùng cho đợt lớn (Map 1 F3's 24-con
    // waves, mọi zone ở Map 2-20 hiện chỉ có "rải trong khu X").
    // Vị trí thật random trong hình chữ nhật/tròn quanh center khi
    // trigger nổ ra — KHÔNG cố định trước, để 2 lần chơi không giống hệt
    // nhau dù cùng 1 wave.
    Cluster { center: Vec3, half_extent: Vec2 },
}
```

Với các zone ở `mapNN_dimensions.md` (map 2-20) hiện chỉ có mô tả bằng
lời ("rải trong khu", "rải dọc hành lang") — khi thật sự code map đó,
bước bắt buộc là cụ thể hoá thành `Cluster` (đã có center = tâm zone
trong file dimension, half_extent suy từ half-size zone trừ margin an
toàn khỏi tường/vật cản) — **không cần quay lại thiết kế doc**, chỉ cần
đọc số đã có sẵn.

## 4. `SpawnTrigger` — khi nào bắn wave

```rust
pub enum SpawnTrigger {
    MissionStart,
    // Vùng hình cầu quanh 1 điểm — kích hoạt khi BẤT KỲ player nào bước
    // vào (server-authoritative, check mỗi tick di chuyển player, không
    // phải client báo).
    ZoneEnter { center: Vec3, radius: f32 },
    // Tương đối 1 wave khác đã trigger — dùng cho chuỗi liên tục kiểu
    // F3 Map 1 (t=0/6/12/18/24s không phụ thuộc đợt trước còn sống hay
    // đã chết hết).
    TimerAfter { after: WaveId, delay_secs: f32 },
    // Nhặt 1 item cụ thể (ammo/vũ khí/audio log) — dùng cho các đợt
    // "tới gần tủ đồ ammo" đã mô tả rải rác trong nhiều map.
    ItemPickup { item_id: ItemId },
    // Named event đã tồn tại trong logic map riêng (vd Map 1 F3's `X` —
    // bước vào phòng họp làm vỡ kính; Map 19's trigger tự hủy) — map
    // code tự bắn event này qua `SpawnEventBus` (mục 5), wave system chỉ
    // lắng nghe tên, không biết nội dung sự kiện.
    NamedEvent(EventId),
}
```

**Quyết định quan trọng đã chốt (khác với để ngỏ trước đây):** mọi
`TimerAfter` tính từ **lúc wave nguồn TRIGGER**, không phải lúc wave
nguồn **spawn xong** hay **bị giết hết** — khớp đúng cách các file thiết
kế đã mô tả F3 Map 1 ("5 đợt liên tục... không phải chờ đợt trước chết
hết"). Muốn 1 wave chờ dọn sạch đợt trước (kiểu "chỉ tăng viện khi còn
sống sót" cho map dễ hơn) thì dùng `SpawnTrigger::NamedEvent` bắn thủ
công từ 1 system kiểm tra `Query<&Zombie, With<WaveId(prev)>>::is_empty()`
— không phải trường hợp mặc định, phải làm rõ thiết kế đó ở
`mapNN_dimensions.md` trước.

## 5. State machine mỗi wave

```
        MissionStart/ZoneEnter/Timer/Pickup/Event
Armed ─────────────────────────────────────────────▶ Triggered
  │                                                        │
  │ (chưa đủ điều kiện)                                    ▼
  └── giữ nguyên, check lại mỗi tick               Spawning (queue)
                                                            │
                                            queue rỗng      │ mỗi tick: rút
                                            (đã spawn hết)  │ tối đa N từ
                                                            │ queue (mục 6)
                                                            ▼
                                                        Complete
```

- **Armed**: wave tồn tại nhưng điều kiện trigger chưa xảy ra — hệ thống
  chỉ tốn chi phí check điều kiện (rẻ: distance check hoặc so sánh timer),
  chưa spawn gì.
- **Triggered**: điều kiện vừa thoả — sinh `count` vị trí thật (resolve
  `SpawnRegion` thành toạ độ cụ thể) và đẩy vào 1 hàng chờ spawn dùng
  chung toàn server (không riêng từng wave — mục 6 giải thích lý do).
- **Spawning**: hàng chờ đang rút dần theo `concurrent_cap`.
- **Complete**: wave không làm gì thêm — tồn tại lại chỉ để các
  `TimerAfter { after: this_wave, .. }` khác biết mốc thời gian bắt
  đầu đếm (từ lúc **Triggered**, không phải Complete — xem mục 4).

Mỗi `SpawnWave` chỉ trigger **đúng 1 lần** trong 1 lượt chơi map (không
lặp lại nếu player đi ra rồi quay lại `ZoneEnter` — tránh farm vô hạn).
Ngoại lệ: map thiết kế rõ ràng muốn lặp (không có trong 20 map hiện tại)
sẽ cần thêm `repeatable: bool` sau, chưa làm vì YAGNI.

## 6. Spawn queue dùng chung + concurrent cap — lý do bắt buộc phải có

Nhiều wave trong thiết kế đã có count rất lớn sau x12 (Map 2 Khu B: 78
con/đợt, Map 18 phòng TN: 36+18 con/đợt cùng lúc) — spawn hết ngay trong
1 frame:

1. **Tốn hiệu năng đột biến 1 frame** (spawn 78 entity + physics
   collider + navmesh agent cùng lúc) — đúng loại vấn đề code hiện tại
   đã né bằng `STRESS_TEST_ZOMBIE_COUNT` cap và LOD (`ZOMBIE_LOD_DISTANCE`).
2. **Trông giả** — 78 zombie hiện ra cùng 1 khắc từ hư không lộ liễu hơn
   nhiều so với nhỏ giọt vài giây.

**Cơ chế:** 1 `Resource` `SpawnQueue(VecDeque<(Vec3, ZombieKind, WaveId)>)`
dùng chung toàn server (không phải mỗi wave 1 hàng chờ riêng, để tổng số
entity spawn/tick trên toàn map luôn bị chặn bởi đúng 1 con số, dù bao
nhiêu wave trigger cùng lúc — quan trọng khi player kích hoạt 2 zone
overlap gần nhau). Mỗi tick server rút tối đa
`SPAWN_QUEUE_DRAIN_PER_TICK` phần tử (đề xuất **6/tick ở 60Hz ≈ 360
con/giây tối đa**, đủ để 1 wave 78 con drain hết trong ~0.22s — nhanh
tới mức người chơi không phân biệt được với "spawn ngay" nhưng vẫn trải
đều tải CPU/network replication qua vài chục tick thay vì 1 tick duy
nhất). Wave có `concurrent_cap: Some(n)` (boss luôn `Some(1)`, không bao
giờ đi qua queue rate limit chung — spawn ngay lập tức, đúng cảm giác
"boss xuất hiện đột ngột" các map đã thiết kế) bỏ qua rate limit toàn
cục, dùng cap riêng.

## 7. Spawn ngoài tầm nhìn (off-screen spawn)

Rút từ queue, vị trí cuối cùng (sau khi resolve `Cluster` thành điểm
thật) phải qua 1 bước lọc: **loại bỏ điểm nằm trong tầm nhìn của BẤT KỲ
player nào** (dot product hướng nhìn + raycast che khuất đơn giản, tái
dùng cách camera-dot-product đã ghi trong `ZOMBIE_BESTIARY.md` cho loại
"Sợ ánh sáng"/tương tự) — nếu điểm bị loại, thử lại tối đa 5 lần trong
cùng `Cluster` trước khi chấp nhận spawn trong tầm nhìn (không để queue
kẹt vĩnh viễn nếu cả cụm đều lộ). Điều này **chưa tồn tại trong code
hiện có** (`ZOMBIE_SPAWNS`/`random_zombie_spawn_pos` không check tầm
nhìn — sandbox mode chấp nhận spawn lộ vì không phải trải nghiệm kịch
bản) — là phần thực sự MỚI so với hệ thống cũ, không phải tái dùng.

## 8. Scale theo số người chơi co-op

GDD định 4 người chơi nhưng match có thể chạy 1-4. Toàn bộ số liệu
"x12" ở các file `mapNN_dimensions.md` được tính **cho 4 người** (baseline
thiết kế). Khi số người chơi thật < 4:

```
actual_count = round(designed_count * (0.55 + 0.15 * player_count))
```

→ 1 người: 70% số designed; 2 người: 85%; 3 người: 100%... (làm tròn số
liệu, 4 người vẫn đúng 100% design gốc — không giảm). Hệ số 0.55/0.15
là điểm khởi đầu hợp lý (1 người không nên đối mặt 100% số zombie tính
cho 4 người — sẽ vỡ trận), **cần playtest thật để tune**, không phải số
cuối. Việc cần làm khi code: đọc `player_count` từ lobby lúc map bắt
đầu (giá trị cố định suốt trận, không scale động giữa chừng nếu có
người rời — tránh zombie biến mất giữa combat trông giả).

## 9. Despawn / leash

Wave đã `Complete` không tự despawn zombie của nó — chúng sống như
zombie thường tới khi bị giết hoặc quá xa mọi player quá lâu:

- Nếu 1 zombie (spawn từ wave nào cũng vậy) cách **mọi** player quá
  `SPAWN_LEASH_DESPAWN_DISTANCE` (đề xuất 40m — xa hơn hẳn `ZOMBIE_LOD_
  DISTANCE` 15m hiện có, chỉ để dọn zombie bị bỏ lại phía sau khi nhóm
  đã đi xa, không phải cơ chế LOD) liên tục quá 30s → despawn im lặng,
  không hồi sinh (khác hẳn `tick_respawns` 1:1 của sandbox mode — wave
  đã spent thì thôi, đúng tinh thần "scripted", zombie không phải tài
  nguyên vô hạn).

## 10. Định dạng dữ liệu — RON, theo đúng convention đã có

Không tạo cơ chế mới — dùng lại `read_ron_data_file!` (macro đã có ở
`crates/shared/src/lib.rs`, đang phục vụ `arena_layout_mapN.ron`) cho 1
file `spawn_waves_mapN.ron` mỗi map, ví dụ:

```ron
// crates/client/assets/data/spawn_waves_map01.ron (mẫu, F3 3 đợt đầu)
[
  (
    id: "map01.f3.wave1",
    kind: Regular,
    count: 24,
    region: Cluster(center: (-16.0, 0.0, -4.0), half_extent: (5.0, 5.0)),
    trigger: NamedEvent("map01_f3_glass_trigger"),
    concurrent_cap: None,
  ),
  (
    id: "map01.f3.wave2",
    kind: Regular,
    count: 24,
    region: Cluster(center: (0.0, 0.0, 0.0), half_extent: (12.0, 2.0)),
    trigger: TimerAfter(after: "map01.f3.wave1", delay_secs: 6.0),
    concurrent_cap: None,
  ),
  // ...
]
```

Cột "Nhịp spawn" trong mỗi `mapNN_dimensions.md` chuyển thẳng sang định
dạng này khi tới lúc code — không cần thiết kế lại, chỉ cần transcribe.

## 11. Việc còn lại trước khi code (theo đúng độ ưu tiên)

1. Thêm `MapKind::Campaign(u8)` (map_id 1-20) — hiện chưa tồn tại, mọi
   thứ ở tài liệu này phụ thuộc vào đó.
2. Viết `SpawnWave`/`SpawnRegion`/`SpawnTrigger`/`SpawnQueue` (mục 2-6)
   trong `crates/shared` (dùng chung server + để client mirror hiển
   thị/dự đoán nếu cần) — port từ struct phác thảo ở trên sang Rust
   thật, thêm `Serialize`/`Deserialize` cho RON.
3. Viết hệ thống off-screen-filter (mục 7) — phần duy nhất thực sự mới
   so với code spawn đã có, cần thiết kế kỹ thuật riêng (raycast/dot
   product cụ thể) trước khi code, không chỉ mô tả ý tưởng như ở đây.
4. Transcribe bảng "Nhịp spawn" của Map 1 (đã đủ chi tiết nhất) sang
   `spawn_waves_map01.ron` làm map đầu tiên chạy thử toàn bộ pipeline.
5. Playtest tune hệ số co-op scaling (mục 8) và
   `SPAWN_QUEUE_DRAIN_PER_TICK` (mục 6) — 2 con số duy nhất trong tài
   liệu này chưa có cơ sở thực nghiệm, chỉ là điểm khởi đầu hợp lý.

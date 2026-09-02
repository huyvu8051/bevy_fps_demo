# Game Design Document — Bevy FPS Demo

Trạng thái: **Nháp v0.1** — tổng hợp từ buổi confirm mục tiêu ngày 2026-08-27.
Mục đích: nguồn tham chiếu duy nhất cho hướng đi của game, để mọi quyết định
kỹ thuật sau này (kiến trúc, ưu tiên tính năng, refactor) đối chiếu lại đây
thay vì đoán ý.

## 1. Tầm nhìn

- **Mục tiêu:** phát hành thật, **thương mại** (có kế hoạch kiếm tiền), không
  phải chỉ để học/thử nghiệm.
- **Timeline:** không còn 1 mốc "3 tháng launch full" duy nhất — chia theo
  **giai đoạn** (xem mục 16, quyết định cập nhật 2026-08-27). Ước tính full
  scope ngang L4D (~20 map) là **~16-20 tháng liền mạch**, nhưng chia giai
  đoạn giúp có bản chơi được thật sớm hơn nhiều (Giai đoạn 1: **1 map hoàn
  chỉnh**, không phải 3 tháng cho toàn bộ nội dung).
- **Đội ngũ:** 1 người làm (solo dev), không có team hỗ trợ.
- **Asset:** dùng AI generate toàn bộ (3D model, texture, âm thanh, nhạc —
  không có artist/sound designer riêng).

Xem mục 16 để biết chi tiết từng giai đoạn và mục tiêu cụ thể của Giai đoạn
1 hiện tại.

## 2. Core Loop & Gameplay

- **Kiểu chơi:** giống **Left 4 Dead** — mission-based, đi dọn map, tới điểm
  **extract**. Có **cốt truyện đơn giản** làm nền, nhưng trọng tâm vẫn là
  **trải nghiệm bắn súng** (gunplay), không phải kể chuyện.
- **Progression:** có hệ thống tiến triển (unlock/nâng cấp — chi tiết cụ thể
  chưa quyết, xem mục 5).
- **Hệ thống đánh giá kỹ năng người chơi (Dynamic Difficulty Adjustment):**
  - Đánh giá **theo từng màn chơi / từng lần chơi** của 1 map.
  - Nếu người chơi **tiến bộ** trong lúc chơi → **tăng độ khó** để tránh
    nhàm chán.
  - Nếu người chơi **fail nhiều lần** → **hạ độ khó** để tránh bỏ cuộc.
  - Cần thiết kế: đánh giá dựa trên tiêu chí gì (accuracy, thời gian sống
    sót, số lần chết, headshot rate...)? — **TBD, cần thiết kế công thức cụ
    thể trước khi implement.**

## 3. Chế độ chơi

- **PvE thuần** — không có PvP.
- **Solo hoặc Co-op online** — kể cả chơi 1 mình cũng **tự tạo phòng** (qua
  server), không có chế độ offline thuần.
- **Bắt buộc online** — kể cả solo — lý do: **giữ code path nhất quán**
  (1 luồng logic server-authoritative duy nhất, không maintain 2 bản
  offline/online riêng biệt).
- **Ghép trận:** cả 2 kiểu —
  - **Matchmaking ngẫu nhiên** (ghép với người lạ).
  - **Tạo phòng chơi với bạn bè** (party/room code).
- **Autopilot** — xác nhận là **tính năng gameplay thật** (không chỉ công cụ
  dev), dùng để người chơi **tạm AFK** giữa trận mà không bị treo/chết oan.

## 4. Nền tảng

- **Đa nền tảng, ưu tiên đồng thời (song song) ngay từ đầu**, không làm
  platform nào trước platform nào — lý do chủ động: tránh bug khó fix về
  sau (bug đặc thù platform phát hiện sớm) + giám sát performance
  bottleneck sớm trên từng nền tảng.
- **Ưu tiên hiện tại:** iOS, macOS, Windows, Android, Linux.
- **Sau này (không phải bây giờ):** console.
- **Input:** hỗ trợ **chuột + bàn phím trên thiết bị di động** (qua
  Bluetooth/AssistiveTouch) — đây là 1 trong các USP chính (mục 9).

## 5. Nội dung (Content Scope)

Chưa chốt số lượng — để mở, quyết định dần theo playtest:
- **Số map:** TBD.
- **Số loại zombie:** TBD — nhưng **bắt buộc có boss**.
- **Số vũ khí:** TBD — có **đổi vũ khí giữa trận**.
- **Weapon customization:** có — kiểu **lắp ráp gắn phụ kiện súng như
  PUBG** (attachment system: ống ngắm, báng, nòng, băng đạn...).
- **Character customization:** có — nhân vật tùy chỉnh được (chi tiết mức
  độ tùy chỉnh — TBD).

## 6. Kỹ thuật & Hạ tầng

- **Server:**
  - Giai đoạn đầu: **self-host**.
  - Sau này khi scale: cân nhắc chuyển lên **AWS** (hoặc cloud provider
    tương đương).
- **Ngân sách hiện tại: 0** — tăng dần khi cần scale.
- **Kiến trúc:** server-authoritative, đa nền tảng cùng 1 server (đã có sẵn
  nền móng: Renet2/Replicon netcode).

## 7. Kiếm tiền (Monetization)

- **Chưa chốt mô hình chính** — đang cân nhắc giữa: trả phí 1 lần / free-to-
  play / có quảng cáo. **TBD — cần quyết định trước khi launch** (ảnh hưởng
  trực tiếp tới App Store category, onboarding flow, và cả compliance như
  ATT/privacy nếu có ads).
- **Đã chốt sẽ có:** bán **skin** và **vũ khí** qua IAP (dù mô hình chính là
  gì thì đây vẫn tồn tại như một lớp thu nhập bổ sung).

## 8. Nguồn lực

- 1 người làm toàn bộ (code + design + production).
- Asset: **AI-generated toàn bộ** (3D model, texture, âm thanh, nhạc nền) —
  không có ngân sách mua asset pack hay thuê artist ở giai đoạn này.

## 9. USP — Điểm khác biệt

1. **Trải nghiệm bắn súng chân thật** (gunplay authenticity) — trọng tâm
   cốt lõi, ưu tiên hơn cốt truyện/nội dung diện rộng.
2. **Chơi chung đa nền tảng** — iOS/Android/macOS/Windows/Linux cùng 1
   server, cùng chơi với nhau.
3. **Hỗ trợ chuột + bàn phím trên di động** — hiếm game mobile FPS làm tốt
   điều này, là lợi thế cạnh tranh rõ ràng nếu làm mượt.

## 10. Câu hỏi mở (cần quyết định trước khi implement tương ứng)

- [x] Công thức khởi điểm cho DDA — xem mục 13 (cần playtest thật để tune
      trọng số, nhưng khung đã có).
- [ ] Mô hình kiếm tiền chính thức (trả phí/F2P/ads).
- [x] Số lượng map/zombie/vũ khí — chốt **ngang L4D** (~20 map, ~8 loại
      địch, ~12 vũ khí), xem mục 15.
- [ ] Mức độ character customization (chỉ skin, hay cả model/tỉ lệ cơ thể).
- [x] Cốt truyện — lore đầy đủ (thành phố "Blackridge", nguồn gốc dịch
      "LAZARUS", twist chính Campaign 4) + chi tiết ĐẦY ĐỦ toàn bộ 20 map
      / 4 campaign (layout, pacing, briefing, audio log, environmental
      storytelling, zombie/economy placement) ở mục 14.
- [x] Định nghĩa "bản chơi được" ở Giai đoạn 1 — chốt là **1 map hoàn
      chỉnh** (mục 16), không phải toàn bộ campaign.
- [x] Thiết kế hệ thống account/save — xem mục 12 (Keycloak + Postgres).

## 11. Chiến lược kỹ thuật (quyết định ngày 2026-08-27)

### 11.1 Renderer iOS: giữ native Metal, đầu tư bài bản dần
- **Không** chuyển về Bevy/wgpu chuẩn cho iOS — giữ renderer Metal tự viết
  (`crates/client/src/cadisplaylink/`) vì performance tốt hơn hẳn (wgpu đo
  được 88-100% CPU encoding overhead ngay cả trên scene rỗng, theo
  README).
- **Cách tiếp cận:** KHÔNG dừng lại để audit toàn bộ renderer trước —
  **tiếp tục fix từng bug/thiếu sót khi gặp** trong lúc phát triển tính
  năng thật, giống cách đã làm trong session 2026-08-26/27 (zombie ẩn sau
  vật trong suốt, scope PIP không di chuyển/xoay theo súng...).
- Hệ quả chấp nhận: mỗi thay đổi ảnh hưởng gameplay/render (đặc biệt liên
  quan tới camera/gun/scope) cần **nhớ kiểm tra và mirror sang cả 2 phía**
  (Bevy path cho desktop + native Metal path cho iOS) — đây là chi phí bảo
  trì đã biết trước, chấp nhận đánh đổi lấy performance.

### 11.2 Thứ tự nền tảng: iOS trước
- Vì đã đầu tư nhiều nhất vào iOS (native Metal renderer riêng), **hoàn
  thiện/ổn định iOS trước**, rồi mới lan sang desktop (macOS/Windows/Linux
  — dùng chung code Bevy/wgpu, ít việc hơn) và Android (chưa có renderer
  bridge nào, cần xây từ đầu — kiểu cadisplaylink nhưng cho Android/
  Vulkan hoặc dùng wgpu chuẩn, cần quyết định riêng khi tới lượt).
- Không có nghĩa là bỏ hoàn toàn desktop/Android trong lúc làm iOS — chỉ là
  **không đầu tư tính năng mới cho chúng cho tới khi iOS đạt trạng thái ổn
  định/chơi được**.

### 11.3 Hệ thống Account/Save — cần thiết kế SỚM
- Xác nhận: **cần thiết kế sớm**, không để tới cuối — vì "bắt buộc online"
  (mục 3) + progression (mục 2) + IAP skin/vũ khí (mục 7) đều phụ thuộc
  trực tiếp vào việc có tài khoản người chơi persistent.
- Càng để muộn càng phải **retrofit** vào nhiều hệ thống đã build sẵn
  (matchmaking, inventory, DDA stats theo mục 2 cũng cần lưu theo
  account/map để đánh giá "tiến bộ" qua nhiều lần chơi).
- **Việc tiếp theo cần làm:** thiết kế schema dữ liệu tối thiểu (player id,
  progression, inventory skin/vũ khí, DDA stats lịch sử) + chọn hướng auth
  (tự viết đơn giản qua server hiện có, hay dùng dịch vụ auth có sẵn như
  Firebase Auth/Supabase để tiết kiệm thời gian solo-dev).

## 12. Backend: Auth (Keycloak) + Data (Postgres) — quyết định 2026-08-27

### 12.1 Auth — reuse Keycloak đã có sẵn
- **Kiến trúc:** Keycloak lo danh tính (identity), Postgres lo dữ liệu
  gameplay (progression/inventory/DDA), liên kết qua `keycloak_user_id`
  (claim `sub` trong JWT) — Keycloak **không** lưu bất kỳ dữ liệu gameplay
  nào, giữ tách biệt rõ ràng.
- **Flow đăng nhập đề xuất cho MVP:** **Direct Access Grant** (Resource
  Owner Password Credentials) — client (game) nhận username/password
  ngay trong UI game (không cần webview/browser popup), gửi thẳng tới
  Keycloak's token endpoint, nhận về JWT access token + refresh token.
  - Lý do chọn flow này thay vì Authorization Code + PKCE (chuẩn OAuth2
    khuyến nghị hơn): **đơn giản hơn nhiều cho 1 game client không có
    webview sẵn** (Bevy không có trình duyệt nhúng), và đây là **first-
    party client** (chính bạn sở hữu cả Keycloak lẫn game) nên rủi ro bảo
    mật của ROPC (thường đáng ngại với client bên thứ 3) không áp dụng ở
    đây.
  - **Nâng cấp sau này** (không gấp): chuyển sang Authorization Code +
    PKCE qua system browser (`ASWebAuthenticationSession` trên iOS,
    Custom Tabs trên Android) nếu muốn thêm **social login** (Google/
    Apple) — Keycloak hỗ trợ sẵn qua Identity Provider federation.
- **Server-side:** game server (Rust) validate JWT bằng public key lấy từ
  Keycloak's JWKS endpoint (`/realms/{realm}/protocol/openid-connect/
  certs`) — không cần gọi Keycloak mỗi request, chỉ cache JWKS và verify
  chữ ký + hạn token cục bộ (nhanh, không tạo bottleneck).

### 12.2 Postgres — schema tối thiểu đề xuất
```sql
-- Liên kết 1-1 với user Keycloak qua sub claim, KHÔNG lưu password/email
-- ở đây (Keycloak lo hết phần đó).
players (
    id                UUID PRIMARY KEY,       -- = Keycloak `sub`
    display_name      TEXT NOT NULL,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Vũ khí/skin đã sở hữu (mua hoặc unlock).
player_inventory (
    player_id   UUID REFERENCES players(id),
    item_id     TEXT NOT NULL,        -- vd "ak47_skin_desert", "attachment_reddot"
    acquired_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (player_id, item_id)
);

-- Loadout hiện tại (vũ khí + attachment đang gắn) — tách khỏi inventory
-- vì đây là TRẠNG THÁI hiện tại, inventory là danh sách SỞ HỮU.
player_loadout (
    player_id     UUID REFERENCES players(id) PRIMARY KEY,
    weapon_id     TEXT NOT NULL,
    attachments   JSONB NOT NULL DEFAULT '{}'  -- vd {"scope": "reddot", "stock": "heavy"}
);

-- Xem mục 13 cho bảng skill/DDA riêng.
```

## 13. Thiết kế hệ thống DDA (Dynamic Difficulty Adjustment) — cần cho mục 2

Bạn nói chưa rõ cần lưu gì — đây là plan cụ thể, chia làm **2 tầng** (khớp
đúng ý bạn: "tiến bộ trong lúc chơi thì tăng độ khó ngay" = tầng ngắn hạn,
"đánh giá theo từng lần chơi" = tầng dài hạn):

### 13.1 Tầng ngắn hạn — điều chỉnh SỐNG trong 1 trận (không cần Postgres)
- Server track 1 "performance score" **cuộn** (rolling) trong lúc chơi,
  cập nhật mỗi khi có sự kiện: trúng đạn (+), headshot (+ nhiều hơn), bị
  zombie cắn (-), chết/downed (- nhiều).
- Mỗi N giây (vd 30-60s), so sánh performance score hiện tại với
  **baseline kỳ vọng** cho map/độ khó hiện tại → nếu vượt xa baseline
  (đang chơi tốt hơn dự kiến) → tăng nhẹ spawn rate/aggression zombie;
  nếu thấp hơn nhiều → giảm.
- **Không cần lưu DB** — chỉ tồn tại trong session, mất khi trận kết thúc
  (đã tổng hợp lại thành 1 con số ở tầng dài hạn, mục dưới).

### 13.2 Tầng dài hạn — skill rating persistent (cần Postgres)
Bảng đề xuất:
```sql
player_skill (
    player_id      UUID REFERENCES players(id),
    map_id         TEXT NOT NULL,
    skill_rating   REAL NOT NULL DEFAULT 1000,  -- kiểu ELO, bắt đầu 1000
    matches_played INT NOT NULL DEFAULT 0,
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (player_id, map_id)
);

match_summary (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    player_id      UUID REFERENCES players(id),
    map_id         TEXT NOT NULL,
    completed      BOOLEAN NOT NULL,     -- extract thành công hay fail
    accuracy       REAL,                 -- % trúng đạn
    headshot_rate  REAL,
    damage_taken   REAL,
    deaths         INT,
    duration_secs  REAL,
    zombies_killed INT,
    played_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
```
- **Sau mỗi trận:** tính 1 "performance score" tổng hợp từ các field trên
  (công thức cụ thể — cần playtest để tune trọng số, ví dụ điểm khởi đầu:
  `score = accuracy*0.3 + headshot_rate*0.2 + (completed ? 0.3 : 0) +
  (1 - deaths/expected_deaths)*0.2`), rồi cập nhật `skill_rating` kiểu
  ELO: `new_rating = old_rating + K * (actual_score - expected_score)`.
- **`skill_rating`** (theo từng map riêng, vì độ khó khác nhau giữa map)
  dùng để: (a) set độ khó khởi điểm cho lần chơi tiếp theo trên map đó,
  (b) làm tiêu chí matchmaking sau này (ghép người có skill tương đồng).
- **`match_summary`** giữ lịch sử để: hiển thị thống kê cho người chơi
  (stats screen), và để **tune lại công thức DDA sau này** khi có đủ dữ
  liệu thật (không đoán trọng số mãi mãi — cần dữ liệu thật để chỉnh).

## 14. Cốt truyện — bản nháp

Giữ tối giản trong CÁCH KỂ (kiểu L4D — môi trường + briefing ngắn, không
cutscene nặng, hợp solo dev), nhưng **nội dung/lore đằng sau** được viết
đầy đủ chiều sâu — người chơi không cần xem hết mới hiểu core loop, nhưng
người muốn đào sâu (đọc hết ghi chú/nghe hết radio log) sẽ có 1 câu chuyện
trọn vẹn thật sự.

### 14.1 Thế giới & Bối cảnh dịch bệnh (mở rộng 2026-08-27)

**Thành phố Blackridge** — đô thị công nghiệp cỡ trung (dân số hư cấu
~2.8 triệu trước dịch), có 1 khu trung tâm tài chính/văn phòng cao tầng,
bao quanh bởi các khu dân cư/thương mại, rồi tới vành đai công nghiệp và
ngoại ô ở rìa. Địa hình đa dạng vừa đủ để mỗi campaign có bản sắc riêng
(mục 15/16) mà không cần bịa ra 1 thành phố phi thực tế.

**Nguồn gốc dịch bệnh (chỉ tiết lộ dần qua audio log, KHÔNG giải thích hết
ngay từ đầu — giữ bí ẩn xuyên suốt Campaign 1-3, trả lời ở Campaign 4):**
- Tên gọi không chính thức trong game: **"Chủng Blackridge"** — ban đầu bị
  nhầm là 1 đợt cúm nặng bất thường lây lan nhanh (thời gian ủ bệnh cực
  ngắn, vài giờ thay vì vài ngày).
- **Sự thật (twist Campaign 4):** đây không phải dịch bệnh tự nhiên — là
  hệ quả rò rỉ từ **1 cơ sở nghiên cứu quân sự** nằm ẩn dưới khu vực gọi
  là "Cổng thành" (nơi tin đồn khu an toàn tồn tại) — chính nơi nhóm sống
  sót đang cố tới để **lánh nạn** lại chính là **nguồn gốc thảm họa**. Khu
  "an toàn" có thật (quân đội thật sự đóng ở đó), nhưng họ không di tản
  dân thường — họ đang cố **dọn dẹp bằng chứng** trước khi rút lui hoàn
  toàn, sẵn sàng coi bất kỳ ai tới gần (kể cả nhóm người chơi) là rủi ro
  rò rỉ thông tin cần loại bỏ. Đây là twist chính (không còn "nhẹ" như bản
  nháp cũ) khiến Campaign 4 chuyển từ "tới nơi an toàn" thành 1 tình huống
  đối đầu mới (quân đội, không chỉ zombie) — mở đường rõ ràng cho
  DLC/Campaign 5+ (nhóm phải quyết định: chạy tiếp, hay công khai sự thật).
- **Timeline ẩn** (người chơi ráp lại qua log, không có trong briefing):
  Ngày 0 — rò rỉ tại cơ sở nghiên cứu. Ngày 1-2 — ca nhiễm đầu tiên ở khu
  dân cư gần đó (tiền đề Campaign 2). Ngày 3 — dịch lan tới trung tâm
  thành phố, chính quyền tuyên bố phong tỏa. Ngày 5 — phong tỏa thất bại,
  quân đội rút khỏi trung tâm, để mặc khu vực (đây là điểm nhóm người chơi
  "tỉnh dậy" ở đầu Campaign 1 — không rõ đã bất tỉnh/trốn được bao lâu).

**Các phe liên quan (không phải NPC chiến đấu, chỉ tồn tại qua dấu vết môi
trường/log — giữ đúng nguyên tắc không cutscene nặng):**
- **Quân đội Blackridge (BRDF — Blackridge Defense Force):** ban đầu cố
  phong tỏa/di tản, sau đó rút lui bỏ mặc trung tâm thành phố (Campaign
  1-3), cuối cùng lộ diện thật sự là bên đang che giấu nguồn gốc dịch
  (Campaign 4).
- **Nhóm sống sót khác:** vài nhóm nhỏ khác cũng cố thoát thành phố song
  song — không gặp trực tiếp (tránh cần thêm NPC AI phe thứ 3), chỉ để
  lại dấu vết (xác chết đã cháy do tự vệ thất bại, ghi chú cảnh báo cho
  "ai tới sau", đôi khi 1 đoạn radio log của nhóm đó đã không qua khỏi) —
  dùng để tạo cảm giác thế giới sống, không phải chỉ nhóm người chơi tồn
  tại.

### 14.2 Cấu trúc kể chuyện & Nhịp độ (Pacing) — mở rộng 2026-08-27

- Mỗi **map/mission = 1 chặng di chuyển** từ điểm A tới điểm extract B,
  cấu trúc theo đúng 4 giai đoạn nhịp độ mà chính Left 4 Dead's AI
  Director dùng làm nguyên tắc thiết kế (build-up → sustain peak → peak
  fade → relax) — áp dụng thủ công vào layout map (không cần AI Director
  thật ở Giai đoạn 1, DDA ở mục 13 điều chỉnh độ khó tổng thể, còn nhịp độ
  TRONG 1 map vẫn do level design cố định quyết định):
  1. **Build-up:** đoạn đầu map, zombie thưa, cho người chơi làm quen
     không gian, nhặt đạn/item mở màn.
  2. **Sustain peak:** 1 đoạn giao tranh kéo dài liên tục (không phải 1
     đợt rồi hết) — thường gắn với 1 sự kiện kích hoạt (mở cửa, kéo cần
     gạt, bật báo động) đúng kiểu "event push" của L4D.
  3. **Peak fade:** giao tranh giảm dần, không dừng đột ngột — vài zombie
     lẻ tẻ còn sót lại.
  4. **Relax:** đoạn cuối/trước safe room (mục 14.4) — gần như không có
     địch, chỉ còn environmental storytelling để người chơi kịp "thở" và
     đọc log trước khi qua map tiếp theo.
- Trước mỗi mission: 1 đoạn text/audio briefing ngắn (2-3 câu) — lý do
  phải đi qua khu vực này, có gì nguy hiểm đã biết.
- Environmental storytelling: xác chết, biển báo, tin nhắn radio còn sót
  lại trong map — không cần nhân vật NPC thoại nhiều. **Audio log là
  phương tiện lore chính** — cụ thể hóa nội dung thật ở mục 14.3 (không
  chỉ nói chung chung "có ghi chú" nữa).

### 14.3 Campaign 1 — "Ground Zero" (chi tiết đầy đủ 5 map)

*Bối cảnh chung:* Nhóm sống sót tỉnh dậy giữa đống đổ nát trung tâm
Blackridge, không rõ đã bao lâu kể từ khi dịch bùng phát. Mục tiêu: thoát
khỏi khu phong tỏa trung tâm trước khi BRDF hoàn tất "thanh lọc" khu vực.

---

**Map 1 — "Tòa nhà văn phòng" (Highrise)**
- *Layout/flow:* Bắt đầu ở tầng hầm để xe (nơi nhóm trú ẩn qua đêm) → đi
  lên qua sảnh chính (đổ nát, kính vỡ khắp nơi) → cầu thang bộ xoắn ốc lên
  vài tầng văn phòng → ra được mái/lối thoát hiểm → xuống đường phố qua
  thang cứu hỏa ngoài tòa nhà. *Verticality* chủ đạo — đi lên rồi lại đi
  xuống, tạo cảm giác "phải trèo qua cả tòa nhà mới ra được".
- *Pacing:* Build-up ở gara + sảnh (zombie thưa, dễ học control) → Sustain
  peak khi kích hoạt báo cháy tự động ở tầng 3 (kính vỡ do nổ trước đó
  khiến hệ thống hoảng loạn kích hoạt, thu hút toàn bộ zombie tầng trên) →
  Peak fade khi lên gần mái → Relax ở lối thoát hiểm trước khi xuống phố.
- *Briefing:* "Thang máy hỏng. Cầu thang bộ là đường duy nhất lên — nhưng
  không phải mình mày nghĩ ra điều đó."
- *Audio log #1 (tìm thấy trong văn phòng tầng 2, giọng nhân viên bảo
  vệ):* "...nếu ai nghe được cái này, đừng xuống gara B2, tụi nó tụ ở dưới
  đó đông lắm. Tao với thằng Dan đang kẹt ở tầng 5, chờ... [tiếng ồn, cắt
  ngang]"
- *Environmental storytelling:* xác nhân viên văn phòng gục trên bàn làm
  việc, ba lô sơ tán bỏ lại hàng loạt ở sảnh (gợi ý sơ tán hoảng loạn,
  không kịp mang theo), biển chỉ dẫn "ĐIỂM TẬP KẾT" bị gạch chéo bằng sơn
  đỏ kèm chữ "ĐỪNG TIN".
- *Zombie/economy placement:* zombie thường rải đều theo mật độ tăng dần
  theo tầng; 2-3 điểm pickup đạn ở khu vực an ninh/tủ đồ nhân viên; 1 med
  kit ngay trước đoạn cầu thang xoắn ốc (thử thách sustain peak).

**Map 2 — "Phố mua sắm" (Meridian Plaza)**
- *Layout/flow:* Không gian mở hơn hẳn Map 1 — 1 quảng trường trung tâm
  bao quanh bởi các cửa hàng (có thể ghé vào lục đồ/né zombie), dẫn tới 1
  bãi đỗ xe nhiều tầng ở cuối map làm điểm chuyển tiếp sang Map 3 (ga tàu
  điện ngầm nằm dưới bãi đỗ xe).
- *Pacing:* Build-up ở lối vào quảng trường → Sustain peak khi 1 nhóm
  zombie đông phục kích từ trong các cửa hàng khi người chơi đi ngang khu
  trung tâm quảng trường (kích hoạt bằng vị trí, không phải hành động chủ
  động — khác Map 1) → Peak fade dọc theo lối vào bãi đỗ xe → Relax ở tầng
  trên bãi đỗ xe trước khi xuống ga tàu điện ngầm.
- *Briefing:* "Radio bắt được tín hiệu yếu — một nhóm khác từng ở đây
  trước mình. Không nghe được tin gì từ họ nữa."
- *Audio log #2 (trong 1 cửa hàng đồ điện tử, giọng 1 người trong "nhóm
  khác" nhắc ở Map 1):* "Ngày thứ ba trong này rồi. Đạn gần hết. Nếu tụi
  bay tìm thấy cái này — đừng ở lại quảng trường lâu, tụi nó nghe tiếng
  động là kéo tới cả đàn."
- *Environmental storytelling:* hàng loạt cửa hàng bị cướp phá (kệ đồ ăn
  trống trơn, kệ thuốc/y tế cũng trống — gợi ý người sống sót khác đã lấy
  hết trước, giải thích tự nhiên vì sao med kit khan hiếm hơn ở map này),
  1 khu vực dựng rào chắn tạm bằng bàn ghế (nơi nhóm trước cố thủ và thất
  bại — xác họ ở đó, súng đã hết đạn nằm cạnh).
- *Zombie/economy placement:* mật độ cao hơn Map 1, tập trung ở khu trung
  tâm quảng trường (đúng điểm phục kích); đạn khan hiếm hơn (đã bị nhóm
  trước lấy — lý do trong lore), bù lại 1 vũ khí thứ 2 xuất hiện lần đầu
  (khẩu súng ngắn dự phòng, nằm cạnh xác nhóm sống sót trước) làm điểm
  giới thiệu hệ thống đổi vũ khí (mục 2/15).

**Map 3 — "Ga tàu điện ngầm" (Meridian Station)**
- *Layout/flow:* Đi xuống từ bãi đỗ xe Map 2, qua khu vực soát vé, xuống
  sân ga, rồi đi dọc theo đường ray trong đường hầm tối để ra 1 ga khác ở
  đầu bên kia (gần chân cầu vượt của Map 4). Không gian **hẹp, tuyến tính,
  tối** — khác biệt rõ rệt với 2 map mở trước đó.
- *Pacing:* Build-up ở khu soát vé/sân ga (vẫn còn ánh sáng khẩn cấp) →
  Sustain peak trong đường hầm tối (tầm nhìn giảm mạnh, đèn pin là công cụ
  chính, đây là nơi **Boomer-style giới thiệu lần đầu** — mục 15/17.5,
  khói mù trong không gian kín cực kỳ nguy hiểm vì không có chỗ tránh) →
  Peak fade khi gần ga đến → Relax ở sân ga cuối, sáng sủa hơn hẳn để nhấn
  mạnh cảm giác "thoát khỏi bóng tối".
- *Briefing:* "Đường hầm dẫn ra ngoài khu phong tỏa. Không ai muốn xuống
  đó, nhưng không còn lựa chọn."
- *Audio log #3 (bảng điều khiển phòng kỹ thuật ga, giọng nhân viên đường
  sắt):* "Tàu cuối cùng chạy lúc 6 giờ sáng ngày phong tỏa. Sau đó chúng
  tôi cắt điện đường hầm theo lệnh — không phải để ngăn tụi nó, mà để
  ngăn người sống chạy trốn qua đó. Chúa ơi, họ đã biết trước." *(gợi ý
  ngầm đầu tiên về twist Campaign 4 — BRDF biết trước và cố kiểm soát
  luồng di tản, không phải chỉ ứng phó dịch bệnh)*.
- *Environmental storytelling:* biển "KHU VỰC CẤM — CHỈ NHÂN VIÊN BRDF"
  dán đè lên biển ga cũ (gợi ý quân đội từng kiểm soát khu vực này trước
  khi rút), đèn pin/mũ bảo hộ công nhân vứt lại dọc đường ray.
- *Zombie/economy placement:* mật độ vừa nhưng nguy hiểm hơn do tầm nhìn
  hẹp; 1-2 Boomer đặt tại các khúc cua hẹp trong hầm (buộc người chơi phản
  ứng nhanh, không thấy trước); pin đèn pin (nếu có cơ chế pin — cân nhắc
  thêm ở giai đoạn polish) và đạn đặt xen kẽ dọc đường ray để duy trì nhịp
  di chuyển liên tục thay vì dồn hết vào 1 điểm.

**Map 4 — "Cầu vượt" (Route 9 Overpass)**
- *Layout/flow:* Ra khỏi lòng đất tại ga cuối, leo lên 1 cây cầu vượt lớn
  bắc qua sông/đường cao tốc, bị kẹt xe hàng loạt (obstacle course tự
  nhiên bằng xe cộ chắn lối, buộc đi vòng/leo qua). Cầu dài, ngoài trời,
  **sightline xa nhất trong Campaign 1**.
- *Pacing:* Build-up ở đầu cầu (vẫn còn dốc lên, ít địch) → Sustain peak ở
  đoạn giữa cầu khi kẹt xe dày đặc nhất (zombie ẩn trong/dưới gầm xe, bật
  ra khi người chơi đi ngang — jump-scare kiểu closet nhẹ, không lạm dụng)
  → Peak fade ở đoạn cầu thoáng cuối, tận dụng sightline dài cho vũ khí
  tầm xa → Relax khi gần tới điểm hẹn cuối cầu.
- *Briefing:* "Cầu là điểm hẹn — nếu đội cứu hộ còn ở đó."
- *Audio log #4 (radio xe cứu hộ bỏ lại giữa cầu, giọng chỉ huy BRDF —
  đây là log then chốt hé lộ twist, đặt ở map áp chót có chủ đích):*
  "...lệnh từ trên là rút toàn bộ đơn vị khỏi khu vực Cổng thành trước
  18:00. Không giải thích lý do. Nếu còn dân thường ở khu trung tâm,
  mặc kệ — ưu tiên là bịt kín [nhiễu sóng] ...không được để lộ ra ngoài."
- *Environmental storytelling:* xe quân sự BRDF bị lật/cháy giữa cầu
  (gợi ý quân đội cũng đã rút chạy hỗn loạn, không phải rút lui có trật
  tự như tuyên truyền), hàng loạt xe dân thường bỏ trống với cửa mở, đồ
  đạc cá nhân còn nguyên trong xe.
- *Zombie/economy placement:* nhiều zombie "ẩn" (đứng yên giả chết trong
  xe, kích hoạt khi lại gần — dùng lại animation Idle/Crawl đã có) xen kẽ
  zombie thường; 1 vị trí đặt vũ khí sniper lần đầu (tận dụng sightline
  dài của map để giới thiệu tự nhiên); đạn/med kit đặt trong các xe cứu
  hộ/xe cứu thương bỏ lại.

**Map 5 — "Điểm hẹn" (Terminus Overlook) — Boss map, kết thúc Campaign 1**
- *Layout/flow:* Tới điểm hẹn — 1 bãi đáp trực thăng trên nóc 1 gara nhiều
  tầng cuối khu trung tâm — chỉ thấy đống đổ nát, đội cứu hộ đã rút đi.
  Map ngắn hơn 4 map trước (tập trung vào boss fight, không phải di
  chuyển dài) — không gian đấu boss là 1 khoảng sân rộng có nhiều cột/vật
  cản để né đòn.
- *Pacing:* Build-up ngắn khi leo lên các tầng gara (dọn zombie lẻ tẻ, thu
  thập đạn/med kit cuối cùng chuẩn bị boss fight — đây là "safe room"
  trước boss, mục 14.4) → **Sustain peak = toàn bộ boss fight** (Tank-
  style, mục 15, xuất hiện ngay khi bước ra bãi đáp) → không có "peak
  fade" truyền thống (boss chết là kết thúc đột ngột, đúng kiểu climax) →
  Relax là đoạn credit/kết thúc campaign trên trực thăng/xe rời đi.
- *Briefing:* "Đây là cơ hội cuối để ra khỏi Ground Zero. Sau đây, không
  quay đầu được nữa."
- *Audio log #5 (cuối cùng, phát tự động khi tới gần bãi đáp, giọng phi
  công cứu hộ, ghi âm lại — không phải log tìm thấy mà là trigger sự
  kiện):* "Đây là điểm hẹn cuối, lặp lại, điểm hẹn cuối. Nếu còn ai ở khu
  trung tâm nghe được — chúng tôi không quay lại nữa. Chúc may mắn." *(kết
  thúc bằng tiếng cánh quạt xa dần — xác nhận trực thăng đã bỏ đi TRƯỚC
  khi nhóm người chơi tới, tạo cảm giác "chậm một bước" xuyên suốt)*.
- *Twist nhẹ cuối campaign (không phải twist chính, đó dành cho Campaign
  4):* sau khi hạ boss, 1 đoạn radio bắt được tín hiệu khác — không phải
  cứu hộ, mà là **tín hiệu dân sự tự phát** từ 1 nhóm sống sót ở khu vực
  gọi là "Cổng thành" (rìa thành phố), tuyên bố đó là nơi an toàn cuối
  cùng. Đây là điểm khởi đầu hành trình Campaign 2-4 — không đảm bảo là
  thật (người chơi/lore chưa biết Cổng thành thực chất là gì cho tới
  Campaign 4).
- *Zombie/economy placement:* zombie thường tối thiểu trước boss (để dồn
  tài nguyên chuẩn bị); đầy đủ đạn/med kit ở khu vực ngay trước bãi đáp
  (safe room trước boss, mục 14.4); boss Tank-style chiếm trọn không gian
  đấu, không có zombie thường xen vào cùng lúc (giữ fight rõ ràng, dễ đọc
  cho co-op 4 người).

### 14.4 Campaign 2 — "Vành đai" (Perimeter, chi tiết đầy đủ 5 map)

*Bối cảnh chung:* Rời trung tâm sau tín hiệu dân sự cuối Campaign 1, nhóm
tới khu dân cư ngoại ô — nơi **ca nhiễm đầu tiên** thực sự xảy ra (timeline
ẩn, mục 14.1), dù chưa ai trong nhóm biết điều đó. Không khí đổi hẳn: từ
hỗn loạn đô thị (Campaign 1) sang **tĩnh lặng đáng sợ** — nhà cửa còn gần
như nguyên vẹn, chỉ trống rỗng, khiến từng tiếng động nhỏ đều đáng ngờ.
Đây là campaign giới thiệu đủ 4 loại zombie đặc biệt còn lại (mỗi map 1
loại, mục 15), mỗi loại gắn với 1 kiểu không gian tận dụng đúng cơ chế
của nó.

**Map 6 — "Khu nhà mẫu" (Sundown Gardens)**
- *Layout/flow:* Cổng vào khu dân cư — dãy nhà mẫu giống hệt nhau (dùng
  lại module nhà lặp lại, tiết kiệm công dựng map) tạo cảm giác mê cung
  ngoại ô, dẫn tới 1 công viên nhỏ trung tâm khu rồi ra đường chính.
- *Pacing:* Build-up dọc dãy nhà đầu (yên tĩnh bất thường) → Sustain peak
  khi băng qua công viên trung tâm (không gian mở, dễ bị bao vây) → Peak
  fade ở đường chính → Relax gần trạm xe buýt cuối khu.
- *Giới thiệu Hunter-style:* nhảy vọt từ mái nhà xuống — không gian nhà 1
  tầng liền kề là địa hình lý tưởng để nó di chuyển trên mái, tấn công bất
  ngờ từ trên cao thay vì mặt đất như zombie thường.
- *Briefing:* "Yên tĩnh quá. Không phải kiểu yên tĩnh tốt."
- *Audio log #6 (máy trả lời tự động trong 1 căn nhà mẫu, giọng người mẹ):*
  "Con ơi nếu nghe được tin nhắn này, đừng về nhà, nghe mẹ đi. Có người
  bị sốt cao ở nhà số 14 từ tuần trước, họ nói không sao nhưng... mẹ không
  biết nữa. Gọi lại cho mẹ ngay."
- *Environmental storytelling:* đồ đạc y tế gia đình vứt đầy sân (nhiệt kế
  vỡ, vỏ thuốc hạ sốt) tại đúng "nhà số 14" trong log — đây là **điểm
  ground-zero dân sự** đầu tiên người chơi thực sự đi ngang qua, dù không
  ai trong nhóm nhận ra ý nghĩa của nó.
- *Zombie/economy:* mật độ thấp hơn hẳn Campaign 1 (đúng lời hứa "vành đai
  thưa dân hơn"), nhưng Hunter khiến cảm giác nguy hiểm không giảm; đạn dự
  trữ đặt trong nhà để xe các căn nhà mẫu.

**Map 7 — "Trường tiểu học" (Fairview Elementary)**
- *Layout/flow:* Trường học bỏ hoang — sân trường, dãy lớp học 2 tầng,
  thư viện, nhà thể chất — không gian nhiều phòng nhỏ nối hành lang dài,
  hợp giới thiệu tấn công tầm xa từ góc khuất.
- *Pacing:* Build-up ở sân trường → Sustain peak trong hành lang lớp học
  tầng 2 (Smoker ẩn trong 1 lớp học, kéo người chơi qua cửa sổ vỡ khi đi
  ngang) → Peak fade ở thư viện → Relax ở nhà thể chất (không gian rộng,
  sáng, làm điểm nghỉ tương phản với hành lang tối trước đó).
- *Giới thiệu Smoker-style:* tấn công/kéo người từ xa qua hành lang dài —
  buộc đồng đội phải bắn đứt dây kéo kịp thời, giới thiệu tự nhiên cơ chế
  cứu đồng đội (khác revive ở mục 17.11, đây là 1 dạng "cứu" khác — bắn
  đứt ràng buộc, không phải hồi sinh).
- *Briefing:* "Trường học đóng cửa từ trước dịch bùng theo lịch — không
  phải vì dịch. Sao lại có nhiều xe phụ huynh đậu ngoài sân đến vậy?"
- *Audio log #7 (loa phát thanh trường, ghi âm tự động, giọng hiệu
  trưởng):* "Thông báo khẩn: phụ huynh vui lòng đến đón con ngay lập tức.
  Đây không phải diễn tập." *(để lửng — không giải thích gì thêm, tăng
  cảm giác bất an do thiếu thông tin)*.
- *Environmental storytelling:* cặp sách/đồ chơi trẻ em bỏ lại khắp sân,
  1 lớp học có dấu hiệu cố thủ (bàn ghế chặn cửa từ bên trong, không có
  xác — gợi ý ai đó đã trốn thoát được, không rõ đi đâu).
- *Zombie/economy:* mật độ vừa, phần lớn zombie kích thước nhỏ hơn (dùng
  scale model thấp hơn cho biến thể "trẻ em" — cân nhắc kỹ về mức độ nhạy
  cảm nội dung, có thể thay bằng zombie giáo viên/nhân viên trường thay vì
  học sinh nếu cần giảm nhạy cảm khi làm thật); med kit đặt trong phòng y
  tế trường.

**Map 8 — "Công viên & Hồ nước" (Miller's Pond)**
- *Layout/flow:* Không gian ngoài trời rộng nhất Campaign 2 — công viên
  cây xanh bao quanh 1 hồ nước nhỏ, đường chạy bộ vòng quanh hồ, dẫn tới 1
  nhà nghỉ mát (gazebo) ở giữa làm điểm giữa map.
- *Pacing:* Build-up dọc đường chạy bộ → Sustain peak khi băng qua khu vực
  mở nhất cạnh hồ (không có vật cản, buộc di chuyển liên tục né vùng chất
  lỏng) → Peak fade ở phía xa hồ → Relax tại bãi đỗ xe công viên.
- *Giới thiệu Spitter-style:* nhả vùng chất ăn mòn từ xa — không gian mở
  của công viên là nơi lý tưởng để thể hiện cơ chế "vùng nguy hiểm phải né
  ra" vì có đủ chỗ để di chuyển tránh, không bị kẹt như không gian hẹp.
- *Briefing:* "Không gian trống thế này lẽ ra phải an toàn hơn trong nhà.
  Đừng nói cho ai biết mình nghĩ vậy nếu sai."
- *Audio log #8 (ghi âm trên điện thoại bỏ quên ở ghế đá công viên, giọng
  2 người đang chạy bộ, thu được trước khi dịch bùng phát vài ngày):*
  "...tao thề tao thấy có xe không biển số đậu cạnh khu xử lý nước cả tuần
  nay. Chắc lại kiểm tra chất lượng nước gì đó thôi." *(chi tiết nhỏ, gieo
  manh mối: dịch có thể lan qua nguồn nước, dù chưa xác nhận — 1 giả
  thuyết ngầm, không phải sự thật chính thức, tăng chiều sâu lore mà không
  cần giải thích hết)*.
- *Environmental storytelling:* xe tuần tra không biển số (nhắc trong log)
  đậu gần trạm xử lý nước cạnh hồ, cửa trạm bị phá khóa từ bên ngoài (gợi
  ý ai đó đã đột nhập, không rõ BRDF hay dân thường).
- *Zombie/economy:* mật độ vừa, dàn trải quanh hồ; đạn/med kit đặt trong
  gazebo (điểm giữa map, khuyến khích đi vòng thu thập trước khi tiếp
  tục).

**Map 9 — "Khu chợ đêm ngoại ô" (Perimeter Night Market)**
- *Layout/flow:* Chuyển sang bối cảnh **đêm** (khác 8 map trước đều là
  ban ngày/chạng vạng — tương phản có chủ đích) — khu chợ đêm ngoài trời
  với hàng loạt quầy hàng, lều bạt, dây đèn đứt còn nhấp nháy chập chờn —
  không gian đông đúc/chật hẹp giữa các quầy.
- *Pacing:* Build-up ở lối vào chợ (đèn còn sáng 1 phần) → Sustain peak
  giữa chợ (mật độ quầy hàng dày đặc, tầm nhìn bị chia cắt liên tục) →
  Peak fade ở cuối chợ → Relax tại bãi đậu xe tải giao hàng phía sau chợ.
- *Giới thiệu Jockey-style:* kéo tách 1 người chơi ra khỏi nhóm — không
  gian đông đúc/nhiều lối rẽ của chợ đêm khiến việc bị tách ra đặc biệt
  nguy hiểm (không thấy đồng đội ngay, phải tự xoay sở vài giây).
- *Briefing:* "Đèn còn sáng nghĩa là còn điện. Còn điện nghĩa là còn hy
  vọng có người ở gần đây. Đừng để hy vọng che mắt."
- *Audio log #9 (loa quầy hàng vẫn tự phát nhạc + quảng cáo bị lỗi lặp lại,
  xen giữa 1 đoạn ghi âm khẩn cấp bị chèn vào do lỗi hệ thống loa công
  cộng):* "...[nhạc quảng cáo]... tất cả người dân vui lòng về nhà ngay,
  đây là lệnh giới nghiêm bắt buộc, không tuân thủ sẽ bị xử lý theo quy
  định khẩn cấp... [nhạc quảng cáo tiếp tục]" *(cho thấy lệnh giới nghiêm
  BRDF đã ban hành từ sớm, chồng lấp lên đời sống dân thường bình thường
  đang diễn ra dở dang — hình ảnh tương phản ám ảnh)*.
- *Environmental storytelling:* quầy hàng vẫn còn nguyên đồ chưa kịp dọn
  (thức ăn thiu, đồ chơi, quần áo), 1 khu vực có dấu hiệu đụng độ giữa
  dân thường và lính BRDF (vỏ đạn quân sự lẫn trong đống đổ vỡ — gợi ý
  ngầm sớm rằng BRDF đã dùng vũ lực với chính dân thường của họ).
- *Zombie/economy:* mật độ cao nhất Campaign 2 (đêm + không gian chật hẹp
  = nguy hiểm nhất); đạn/med kit đặt trong các xe tải giao hàng ở khu vực
  Relax cuối map.

**Map 10 — "Trạm kiểm soát BRDF bỏ hoang" (Checkpoint Delta) — Boss map,
kết thúc Campaign 2**
- *Layout/flow:* Ra khỏi khu dân cư, tới 1 trạm kiểm soát quân sự lớn từng
  chặn đường ra vào vành đai — hàng rào bê tông, tháp canh, lô cốt — giờ
  bỏ hoang, cửa mở toang. Không gian mang tính phòng thủ quân sự rõ rệt,
  khác hẳn khu dân cư/thương mại các map trước.
- *Pacing:* Build-up qua khu vực hàng rào ngoài (safe room trước, mục
  14.7) → **Sustain peak = boss fight thứ 2** (Tank-style tái xuất hiện,
  đây không phải "1 boss duy nhất cả game" mà là 1 LOẠI encounter tái diễn
  ở cuối mỗi campaign lớn, đúng cách L4D dùng Tank nhiều lần) → Relax là
  đoạn rời trạm kiểm soát, tiến vào vùng công nghiệp (Campaign 3).
- *Briefing:* "Trạm kiểm soát bỏ trống nghĩa là không còn ai giữ ranh giới
  nữa. Nghĩa là cả 2 phía đều đã thua."
- *Audio log #10 (bảng ghi chép trực ca bỏ lại trong lô cốt, viết tay, nét
  chữ vội vàng):* "23:40 — lệnh rút toàn bộ trạm kiểm soát vành đai, hiệu
  lực ngay. Không có lý do chính thức. Trung úy nói đây là lệnh trực tiếp
  từ Cổng thành." *(lần đầu tiên tên "Cổng thành" xuất hiện TRONG lore
  BRDF, không chỉ trong lời đồn dân sự ở cuối Campaign 1 — xác nhận ngầm
  đây là 1 địa điểm thật, có thật quyền lực ra lệnh, tăng trọng lượng cho
  twist sắp tới)*.
- *Environmental storytelling:* vũ khí/đạn quân sự bỏ lại vội vàng (rút
  quá gấp để mang theo hết), 1 tấm bản đồ khu vực dán trên tường lô cốt có
  đánh dấu rõ vị trí "Cổng thành" ở góc bản đồ — người chơi lần đầu **nhìn
  thấy trực quan** hướng đi tiếp theo, không chỉ nghe qua lời kể.
- *Zombie/economy:* zombie thường tối thiểu trước boss; đầy đủ đạn/med kit
  ở lô cốt (safe room); boss chiếm trọn không gian đấu trong sân trạm kiểm
  soát (nhiều vật cản bê tông để né đòn, tái dùng bố cục tương tự Map 5
  nhưng địa hình quân sự thay vì gara dân sự).

### 14.5 Campaign 3 — "Vùng công nghiệp" (Industrial Sprawl, chi tiết đầy
đủ 5 map)

*Bối cảnh chung:* Rời vành đai, nhóm tiến vào khu công nghiệp/cảng hàng
hóa rìa thành phố — địa hình đổi hẳn sang nhà máy, kho bãi, container,
đường ống — không gian rộng, nhiều tầng/độ cao, ít dấu vết dân thường hơn
(khu vực làm việc, không phải nơi ở) nhưng biển báo/hàng rào an ninh BRDF
dày đặc dần, báo hiệu đang tới gần "Cổng thành".

**Map 11 — "Nhà máy dệt cũ" (Ironloom Textile Mill)**
- *Layout/flow:* Nhà máy dệt bỏ hoang từ trước dịch (không liên quan trực
  tiếp BRDF, chỉ là nhà máy đóng cửa vì phá sản) — không gian tối, nhiều
  máy móc lớn làm vật cản/che khuất tầm nhìn, dây chuyền sản xuất dẫn từ
  tầng trệt lên các tầng lửng.
- *Pacing:* Build-up ở khu văn phòng nhà máy → Sustain peak giữa dây
  chuyền máy dệt (không gian mê cung máy móc, dễ bị bao vây từ nhiều
  hướng) → Peak fade ở kho vải → Relax ở bãi bốc dỡ hàng phía sau nhà máy.
- *Briefing:* "Nhà máy này đóng cửa cả năm trước dịch. Không phải nơi đầu
  tiên tao nghĩ tới khi tìm chỗ trú, nhưng cửa vẫn khóa từ ngoài — có
  nghĩa là bên trong có thể còn sạch."
- *Audio log #11 (sổ trực ca cũ của bảo vệ nhà máy, còn sót lại từ trước
  khi nhà máy đóng cửa — không liên quan trực tiếp dịch bệnh, chỉ là chi
  tiết đời thường tạo chiều sâu thế giới):* "Ca cuối cùng. Không ngờ công
  ty phá sản nhanh vậy. Chúc ai đọc được cuốn sổ này may mắn hơn tụi tôi."
  *(một khoảnh khắc nhân văn không liên quan cốt truyện chính — nhắc rằng
  Blackridge có vấn đề kinh tế/xã hội từ trước khi dịch bệnh xảy ra, thế
  giới có chiều sâu ngoài mạch truyện chính)*.
- *Environmental storytelling:* máy móc phủ bụi dày (đóng cửa đã lâu,
  không phải do dịch), tương phản với 1 góc nhà máy có dấu hiệu người sống
  sót mới dùng làm nơi trú ẩn gần đây (đống lửa trại còn ấm, đồ hộp mới
  mở).
- *Zombie/economy:* mật độ vừa; đạn dự trữ trong tủ đồ bảo hộ lao động;
  med kit trong phòng y tế nhà máy (theo luật lao động, mọi nhà máy đều
  có).

**Map 12 — "Bãi container cảng" (Blackridge Freight Yard)**
- *Layout/flow:* Bãi container khổng lồ — mê cung thùng container xếp
  chồng nhiều tầng, lối đi ngoằn ngoèo giữa các dãy, cần trục cảng làm
  điểm mốc thị giác từ xa để định hướng.
- *Pacing:* Build-up ở lối vào bãi (còn thấy toàn cảnh từ trên cao) →
  Sustain peak sâu trong mê cung container (tầm nhìn bị chặn liên tục,
  âm thanh dội lại giữa các thùng kim loại gây khó định vị nguồn tiếng
  động — tận dụng đúng bản chất không gian) → Peak fade khi ra gần cần
  trục → Relax dưới chân cần trục, view rộng trở lại.
- *Briefing:* "Mê cung kim loại này sẽ làm tụi mình lạc nếu không có gì để
  định hướng. Nhìn cần trục ở giữa bãi — đi về hướng đó là ra được."
- *Audio log #12 (radio 2 chiều bỏ lại trong 1 container mở, giọng công
  nhân cảng):* "Cont số 447 tới 460 — đóng dấu ưu tiên, không ai được mở
  ngoại trừ áp tải BRDF trực tiếp. Tao không hỏi trong đó có gì, tao chỉ
  làm đúng việc được giao." *(gieo manh mối: BRDF vận chuyển hàng hóa bí
  mật qua cảng dân sự — có thể liên quan tới cơ sở nghiên cứu, không xác
  nhận rõ)*.
- *Environmental storytelling:* dãy container 447-460 (nhắc trong log) bị
  niêm phong bằng dấu BRDF, 1 container trong dãy đó đã bị cạy mở từ bên
  trong (không rõ ai/cái gì đã ra khỏi đó — chi tiết rùng rợn, không giải
  thích, để người chơi tự suy diễn).
- *Zombie/economy:* mật độ cao (không gian hẹp giữa container = zombie
  dễ phục kích từ góc khuất); đạn/med kit rải trong các container thường
  (không niêm phong) dọc đường.

**Map 13 — "Nhà kho hóa chất" (Cold Storage 7)**
- *Layout/flow:* 1 nhà kho lạnh công nghiệp (bảo quản hóa chất/thực phẩm
  đông lạnh) — không gian kín, nhiệt độ thấp (thể hiện qua hơi sương/hiệu
  ứng particle, không ảnh hưởng gameplay thực) — nhiều phòng lạnh riêng
  biệt nối hành lang chính.
- *Pacing:* Build-up ở khu vực nhận hàng → Sustain peak dọc hành lang
  chính (nhiều cửa phòng lạnh 2 bên, mỗi cửa là 1 rủi ro tiềm ẩn) → Peak
  fade khi gần phòng lạnh sâu nhất → **Relax đặc biệt: không phải hết
  nguy hiểm, mà là gặp Witch-style đứng yên** (khác cấu trúc 4 map trước —
  đây không phải combat cao trào rồi giảm dần, mà là 1 khoảnh khắc CĂNG
  THẲNG TĨNH: phải đi rón rén qua, không được gây tiếng động).
- *Giới thiệu Witch-style:* đứng yên trong phòng lạnh sâu nhất, chỉ tấn
  công cực mạnh nếu bị làm phiền (ánh đèn pin chiếu trực tiếp, tiếng súng
  gần, hoặc chạm phải) — người chơi có thể chọn đi vòng tránh hẳn (nếu
  map có lối phụ) hoặc liều lĩnh đi ngang qua rón rén.
- *Briefing:* "Cảm biến nhiệt độ khu kho lạnh vẫn hoạt động — có thứ gì đó
  bên trong, và nó không di chuyển. Đừng làm nó thức dậy."
- *Audio log #13 (biên bản kiểm tra kho lạnh, dán trên bảng thông báo,
  ngày trước dịch bùng phát):* "Phát hiện 1 khoang lạnh không rõ nguồn
  gốc, không có trong danh mục hàng hóa đã đăng ký, dấu niêm phong BRDF.
  Đã báo cáo lên quản lý, chưa nhận phản hồi." *(tiếp nối manh mối từ Map
  12 — container bí mật BRDF giờ được xác nhận đã tới đúng khu vực kho
  lạnh này, ngầm liên kết Witch-style với chính hàng hóa bí mật đó — gợi ý
  witch không phải zombie thường mà là 1 kết quả thí nghiệm, dù không nói
  thẳng)*.
- *Environmental storytelling:* 1 khoang lạnh riêng biệt có dấu niêm
  phong BRDF đã bị phá vỡ từ bên trong (nơi Witch-style xuất hiện), khác
  hẳn các khoang hàng hóa thông thường xung quanh.
- *Zombie/economy:* mật độ zombie thường THẤP nhất map này (Witch là mối
  đe dọa chính, không cần đông zombie thường cạnh tranh sự chú ý); med kit
  đặt ngay trước khu vực Witch (phần thưởng cho ai chọn đối đầu thay vì né
  tránh).

**Map 14 — "Đường ống & Nhà máy lọc" (Refinery Loop)**
- *Layout/flow:* Nhà máy lọc dầu/hóa chất — verticality cao nhất toàn
  Campaign 3: đi trên giàn giáo, đường ống dẫn trên cao, cầu thang sắt
  xoắn ốc giữa các bồn chứa khổng lồ. Rơi khỏi đường đi = phải vòng lại
  (không chết ngay, chỉ mất thời gian/gặp thêm zombie).
- *Pacing:* Build-up ở khu vực mặt đất nhà máy → Sustain peak trên giàn
  giáo cao (không gian hẹp, không có chỗ né rộng, buộc chiến đấu cẩn thận
  hơn hẳn mặt đất) → Peak fade khi xuống thang cuối → Relax ở phòng điều
  khiển trung tâm nhà máy.
- *Briefing:* "Đi trên cao thì thấy xa hơn, nhưng ngã xuống thì mất mạng
  y như bị cắn. Cẩn thận từng bước."
- *Audio log #14 (nhật ký kỹ sư vận hành nhà máy lọc, viết tay để lại
  trong phòng điều khiển):* "BRDF thuê nguyên khu nhà máy này làm bãi
  trung chuyển 3 tháng trước, nói là 'dự trữ chiến lược'. Tụi tôi bị cấm
  vào khu bồn chứa số 9 tới số 12. Tôi làm ở đây 15 năm, chưa từng có khu
  vực nào cấm nhân viên vào cả." *(gieo manh mối: BRDF đã âm thầm chiếm
  dụng hạ tầng công nghiệp dân sự làm hậu cần cho hoạt động bí mật từ
  trước khi dịch bùng phát — không chỉ phản ứng SAU dịch, mà đã chuẩn bị
  TRƯỚC)*.
- *Environmental storytelling:* bồn chứa số 9-12 (nhắc trong log) được rào
  riêng bằng hàng rào BRDF mới hơn hẳn phần còn lại của nhà máy (rõ ràng
  mới lắp gần đây, tương phản với kết cấu nhà máy cũ kỹ xung quanh).
- *Zombie/economy:* mật độ vừa nhưng độ khó tăng do địa hình hẹp trên
  cao; đạn/med kit đặt trong phòng điều khiển (Relax cuối map, safe room
  tự nhiên trước khi qua map cuối campaign).

**Map 15 — "Hàng rào an ninh BRDF" (Perimeter Fence Alpha) — Boss map,
kết thúc Campaign 3**
- *Layout/flow:* Tới sát hàng rào an ninh ngoài cùng của khu vực "Cổng
  thành" — dày đặc lô cốt, camera an ninh (không hoạt động, chỉ là set
  dressing), tháp canh — đây là ranh giới cuối cùng trước khi Campaign 4
  bắt đầu thật sự đi vào bên trong.
- *Pacing:* Build-up dọc hàng rào ngoài (safe room trước, mục 14.7) →
  **Sustain peak = boss fight thứ 3** (Tank-style tái xuất hiện lần cuối
  trước Campaign 4, có thể tăng nhẹ chỉ số so với 2 lần trước để phản ánh
  độ khó leo thang tổng thể — cụ thể hóa khi tune ở Giai đoạn 3) → Relax
  ngắn ngay trước cổng chính dẫn vào Campaign 4 (kết thúc campaign ở đúng
  ngưỡng cửa, tạo cliffhanger tự nhiên).
- *Briefing:* "Đây là ranh giới cuối. Sau hàng rào này là nơi tụi mình
  đang tìm suốt từ đầu. Cầu mong nó đúng như những gì được đồn."
- *Audio log #15 (bảng hiệu điện tử an ninh vẫn còn nguồn dự phòng, phát
  cảnh báo tự động lặp lại):* "Khu vực hạn chế. Mọi cá nhân không có thẩm
  quyền sẽ bị coi là mối đe dọa an ninh và xử lý theo quy định. Đây không
  phải cảnh báo cuối cùng." *(giọng điệu đe dọa rõ ràng hơn hẳn mọi cảnh
  báo BRDF trước đó trong game — báo trước bản chất thật của Campaign 4:
  không chỉ có zombie chờ đợi bên trong)*.
- *Environmental storytelling:* xác lính BRDF (không phải zombie hóa,
  chết vì đạn) nằm rải rác gần hàng rào — gợi ý ngay cả chính BRDF cũng
  từng có xung đột nội bộ hoặc chống lại mệnh lệnh rút lui/dọn dẹp, không
  phải toàn bộ tổ chức đồng lòng.
- *Zombie/economy:* zombie thường tối thiểu; đầy đủ trang bị ở safe room
  trước boss (khuyến khích dùng hết progression tích lũy được trước khi
  bước vào Campaign 4 khó nhất); boss đấu trong khoảng sân giữa các lô cốt
  (nhiều vật cản bê tông, tái dùng bố cục quen thuộc nhưng quy mô lớn
  hơn Map 5/Map 10).

### 14.6 Campaign 4 — "Cổng thành" (The Gate, chi tiết đầy đủ 5 map, kết
thúc game)

*Bối cảnh chung:* Vào bên trong khu vực bị cấm suốt 3 campaign trước —
không phải khu tị nạn như lời đồn, mà là **cơ sở nghiên cứu quân sự BRDF,
nguồn gốc thật của dịch bệnh** (mục 14.1). Từ map đầu tiên, kẻ địch không
chỉ còn là zombie — **lính BRDF** xuất hiện như 1 loại "địch" mới (PvE,
tái dùng framework AI zombie với model lính để đơn giản kỹ thuật, mục
14.4 cũ đã ghi rõ hướng này), đóng vai trò ngăn cản nhóm tiếp cận sự thật.

**Map 16 — "Trạm kiểm soát chính" (Main Gatehouse)**
- *Layout/flow:* Cổng vào chính thức của cơ sở — trạm gác kiên cố, hàng
  rào điện, sân trong. Đây là map đầu tiên **lính BRDF còn sống và có tổ
  chức** xuất hiện (không phải xác chết như các campaign trước) — canh
  gác chủ động, không phải lực lượng đã sụp đổ.
- *Pacing:* Build-up ở khu vực ngoài hàng rào (vẫn còn zombie như bình
  thường) → Sustain peak khi vào trạm gác — **lần đầu chạm trán lính BRDF
  đang tuần tra** (xen kẽ zombie, 2 loại địch cùng lúc — lính đôi khi bắn
  cả zombie nếu zombie tới gần họ trước, tạo cảm giác hỗn loạn 3 phe) →
  Peak fade ở sân trong → Relax tại 1 trạm gác phụ bỏ trống.
- *Briefing:* "Có lính canh nghĩa là có gì đó đáng canh giữ. Không phải
  người tị nạn, không phải trại cứu trợ. Cẩn thận."
- *Audio log #16 (bộ đàm lính BRDF nhặt được từ 1 lính đã hạ, đoạn hội
  thoại nội bộ ghi tự động):* "Có nhóm dân thường tiếp cận từ hướng Nam.
  Xin chỉ thị." — "Xử lý theo quy trình tiêu chuẩn. Không có ngoại lệ cho
  dân thường trong bán kính này, kể cả trẻ em." *(xác nhận trực tiếp,
  không còn ẩn ý — BRDF coi bất kỳ ai tiếp cận là mối đe dọa cần loại bỏ,
  đây là điểm twist chính thức được xác nhận rõ ràng lần đầu, không còn
  chỉ là suy đoán qua log)*.
- *Environmental storytelling:* biển hiệu chính thức "CƠ SỞ NGHIÊN CỨU
  SINH HỌC BLACKRIDGE — BRDF" (lần đầu tên đầy đủ cơ sở xuất hiện, xác
  nhận trực quan điều audio log vừa tiết lộ), phòng chờ có TV vẫn phát
  loop bản tin tuyên truyền cũ nói "tình hình đang được kiểm soát tốt".
- *Zombie/economy:* mật độ zombie vừa + lính BRDF (số lượng nhỏ, vài
  người/đợt, không phải đông như zombie horde — vai trò khác: nguy hiểm
  hơn từng cá thể, không phải áp đảo số lượng); lính rơi lại đạn dược khi
  hạ (nguồn đạn mới, khác hẳn pickup dân sự trước đó).

**Map 17 — "Khu hành chính" (Sector 3 Admin Block)**
- *Layout/flow:* Tòa nhà văn phòng/hành chính của cơ sở — phòng họp,
  văn phòng chỉ huy, phòng lưu trữ hồ sơ. Không gian giống Map 1 (tòa nhà
  văn phòng Campaign 1) một cách có chủ đích — tạo cảm giác "quay vòng",
  nhấn mạnh nhóm đã đi 1 vòng lớn quanh Blackridge chỉ để quay lại đúng
  kiểu không gian đã bắt đầu, nhưng giờ mọi thứ đã rõ ràng hơn.
- *Pacing:* Build-up ở sảnh hành chính → Sustain peak tại khu văn phòng
  chỉ huy (lính BRDF phòng thủ dày hơn Map 16, biết trước có xâm nhập) →
  Peak fade ở phòng lưu trữ → Relax tại 1 phòng nghỉ nhân viên.
- *Briefing:* "Hồ sơ ở đây có thể có câu trả lời tụi mình cần. Nếu có, nó
  đáng để mạo hiểm."
- *Audio log #17 (không phải audio — LẦN DUY NHẤT trong game 1 tài liệu
  VĂN BẢN đầy đủ thay vì ghi âm, tìm thấy trong phòng lưu trữ, đóng dấu
  MẬT):* "Báo cáo sự cố #BR-001: Rò rỉ chủng thử nghiệm 'Chủng Blackridge'
  (mã nội bộ: LAZARUS) tại khu vực nghiên cứu ngầm, nguyên nhân do lỗi
  quy trình containment cấp 4. Khuyến nghị: phong tỏa toàn bộ thành phố
  theo kịch bản ứng phó đã lập sẵn, đồng thời tiêu hủy mọi bằng chứng liên
  kết trực tiếp cơ sở với đợt bùng phát trước khi dư luận/chính quyền dân
  sự vào cuộc." *(tài liệu then chốt — XÁC NHẬN HOÀN TOÀN twist chính bằng
  văn bản chính thức, không còn nghi ngờ gì nữa: dịch bệnh là nhân tạo, có
  kế hoạch che giấu từ đầu, thậm chí có mã tên riêng "LAZARUS")*.
- *Environmental storytelling:* hàng loạt tủ hồ sơ đã bị đốt/tiêu hủy
  dở dang (BRDF đang thực hiện đúng "khuyến nghị tiêu hủy bằng chứng"
  trong báo cáo), máy hủy tài liệu vẫn còn chạy, quá tải giấy kẹt.
- *Zombie/economy:* lính BRDF nhiều hơn Map 16 (bảo vệ khu vực nhạy cảm
  hơn); vẫn có zombie xen kẽ (kể cả trong khu vực an ninh cao, gợi ý cơ sở
  cũng không hoàn toàn kiểm soát được tình hình nội bộ); đạn/med kit trong
  phòng nghỉ nhân viên.

**Map 18 — "Phòng thí nghiệm chính" (Subterranean Lab Alpha)**
- *Layout/flow:* Đi xuống khu nghiên cứu ngầm — nơi thực sự xảy ra rò rỉ
  (nhắc trong báo cáo Map 17) — không gian phòng thí nghiệm vô trùng, ống
  nghiệm, buồng nuôi cấy, lồng thí nghiệm động vật (trống rỗng, cửa mở) —
  **tối tăm và ám ảnh nhất toàn game**, cả về thị giác lẫn nội dung.
- *Pacing:* Build-up ở khu vực tiếp nhận/thay đồ bảo hộ (vẫn nguyên vẹn,
  không ai kịp phản ứng khi sự cố xảy ra) → Sustain peak trong khu phòng
  thí nghiệm chính (mật độ zombie CAO NHẤT toàn game — đây là nơi rò rỉ
  bắt đầu, hợp lý về mặt lore) → Peak fade ở hành lang thoát hiểm → Relax
  ngắn tại 1 trạm kiểm soát an toàn sinh học đã hỏng.
- *Briefing:* "Đây rồi. Nơi mọi chuyện bắt đầu. Tao ước gì tụi mình không
  bao giờ phải nhìn thấy chỗ này."
- *Audio log #18 (ghi âm an ninh camera lab, log cuối cùng trước khi mất
  tín hiệu — giọng nhà nghiên cứu, hoảng loạn):* "Containment thất bại,
  lặp lại, containment thất bại ở khu vực B. [tiếng động lớn, la hét xa
  xa] Không, đừng mở cửa đó — [tín hiệu ngắt]" *(log tái hiện trực tiếp
  khoảnh khắc rò rỉ xảy ra — đặt đúng tại nơi nó xảy ra, tạo tác động cảm
  xúc mạnh nhất, đúng vị trí/thời điểm câu chuyện nên được kể)*.
- *Environmental storytelling:* xác nhà nghiên cứu trong đồ bảo hộ (chưa
  kịp cởi ra, chết ngay tại bàn làm việc), bảng trắng còn ghi công thức/sơ
  đồ dở dang, 1 lồng thí nghiệm có tên "LAZARUS - Subject 0" trống rỗng,
  cửa bị phá từ BÊN TRONG (gợi ý ca nhiễm/biến thể đầu tiên chính là từ
  đây, không phải từ bên ngoài vào).
- *Zombie/economy:* mật độ cao nhất game, có thể cân nhắc thêm biến thể
  zombie phòng thí nghiệm riêng (mặc đồ bảo hộ rách, dùng lại model zombie
  thường + texture khác — tiết kiệm chi phí asset) để phân biệt trực quan
  với zombie "ngoài đường phố" đã gặp suốt game; đạn/med kit khan hiếm có
  chủ đích (tạo áp lực đúng lúc câu chuyện nặng nề nhất).

**Map 19 — "Boong-ke chỉ huy" (Command Bunker)**
- *Layout/flow:* Trung tâm chỉ huy kiên cố của cơ sở — nơi chỉ huy BRDF
  đang trực tiếp giám sát việc tiêu hủy bằng chứng/tự hủy cơ sở. Không
  gian boong-ke bê tông dày, hành lang hẹp, phòng chỉ huy trung tâm ở
  cuối.
- *Pacing:* Build-up ở hành lang ngoài (lính BRDF phòng thủ dày đặc nhất
  game, đây là điểm họ bảo vệ quyết liệt nhất) → Sustain peak khi đột nhập
  phòng chỉ huy (đối đầu trực tiếp với **chỉ huy BRDF** — không phải boss
  Tank-style, mà là 1 encounter khác: 1 nhân vật người thật, có thể chạy
  trốn thay vì đứng đánh tay đôi, dùng chính hệ thống tự hủy cơ sở làm vũ
  khí cuối cùng thay vì đối đầu trực diện — tạo khác biệt rõ với 3 lần
  Tank-style trước, tránh lặp lại công thức) → Peak fade khi hệ thống tự
  hủy được kích hoạt (đếm ngược bắt đầu — chuyển tông sang tẩu thoát) →
  không có Relax truyền thống, map kết thúc bằng báo động sơ tán khẩn cấp
  dẫn thẳng sang Map 20.
- *Briefing:* "Đây là trung tâm đầu não của cả chỗ này. Nếu có ai ra lệnh
  bỏ mặc tụi mình chết ở Ground Zero, người đó ở đây."
- *Audio log #19 (không phải log tìm thấy — LỜI THOẠI TRỰC TIẾP của chỉ
  huy BRDF khi bị dồn vào đường cùng, phát qua loa boong-ke, đây là lần
  DUY NHẤT trong toàn game có 1 "nhân vật" thực sự lên tiếng trực tiếp với
  nhóm, phá vỡ nguyên tắc "chỉ kể qua log" một cách có chủ đích ở đúng
  cao trào quan trọng nhất):* "Các người không hiểu đâu. Nếu tin này lọt
  ra ngoài, không chỉ Blackridge — mọi thành phố có cơ sở như thế này sẽ
  hoảng loạn. Tôi đang cứu nhiều mạng hơn các người tưởng. Cơ sở sẽ tự hủy
  trong 5 phút. Lựa chọn của các người: chạy, hoặc chết cùng sự thật này."
  *(đặt nhóm — và người chơi — vào 1 tình huống đạo đức mơ hồ có chủ đích,
  không phải villain hoàn toàn 1 chiều, tạo chiều sâu cho cái kết mở)*.
- *Environmental storytelling:* phòng chỉ huy có nhiều màn hình giám sát
  camera an ninh khắp cơ sở (bao gồm cả các khu vực nhóm vừa đi qua ở Map
  16-18 — 1 chi tiết nhỏ xác nhận BRDF đã theo dõi nhóm suốt từ đầu
  campaign, không phải tình cờ chạm trán).
- *Zombie/economy:* lính BRDF dày đặc nhất game trước khi tự hủy kích
  hoạt (sau đó họ cũng phải bỏ chạy, không còn là mối đe dọa chính ở Map
  20); không có med kit/đạn pickup mới trong map này (áp lực thời gian —
  không có thời gian dừng lại tìm kiếm, chỉ dùng những gì đã tích lũy).

**Map 20 — "Lối thoát cuối cùng" (Final Breach) — Boss cuối, kết thúc
game (bản 1.0)**
- *Layout/flow:* Chạy trốn khỏi cơ sở đang tự hủy — hành lang sập dần,
  lửa/khói, hệ thống cảnh báo đỏ khắp nơi — một "escape sequence" tuyến
  tính tốc độ cao, khác hẳn nhịp độ khám phá của 19 map trước, dẫn thẳng
  ra cửa thoát hiểm trên mặt đất.
- *Pacing:* Không có build-up truyền thống — map bắt đầu NGAY TRONG sustain
  peak (tiếp nối trực tiếp từ báo động cuối Map 19) → đỉnh điểm là **boss
  cuối cùng, Tank-style biến thể mạnh hơn** ("LAZARUS Subject" trưởng
  thành — chính là kết quả cuối cùng của thí nghiệm nhắc ở Map 18, xuất
  hiện chặn đường thoát chính, kẹt giữa hành lang đang sập) → Peak fade
  cực ngắn ngay sau khi hạ boss (không có thời gian nán lại, cơ sở vẫn
  đang sập) → Relax = đoạn credit/kết thúc, nhóm thoát ra ngoài trời, nhìn
  cơ sở sụp đổ phía sau.
- *Briefing:* "5 phút. Chạy."
- *Audio log #20 (không có audio log truyền thống — thay vào đó là 1 đoạn
  monologue nội tâm ngắn hiện dưới dạng text khi tới cửa thoát cuối cùng,
  không gán cho nhân vật cụ thể nào — giữ đúng nguyên tắc "người chơi
  không thoại", đây là suy nghĩ chung của cả nhóm):* "Tụi mình biết sự
  thật rồi. Câu hỏi bây giờ không phải là có tin hay không — mà là làm gì
  với nó."
- *Environmental storytelling:* toàn bộ map là 1 chuỗi hình ảnh sụp đổ —
  màn hình giám sát vỡ nát (nhắc lại Map 19), phòng thí nghiệm (nhắc Map
  18) cháy rụi khi chạy ngang qua, biển hiệu "CƠ SỞ NGHIÊN CỨU SINH HỌC
  BLACKRIDGE" (nhắc Map 16) đổ sập ngay khi nhóm ra tới cửa ngoài — hình
  ảnh khép vòng lặp trực quan cho toàn bộ Campaign 4.
- *Zombie/economy:* rất ít zombie/lính thường (tốc độ chạy trốn là trọng
  tâm, không phải combat rộng); tập trung toàn bộ độ khó vào boss cuối;
  không có pickup mới — người chơi dùng đúng những gì mang theo tới cuối,
  đúng tinh thần "không còn gì để chuẩn bị thêm, chỉ còn chạy".
- **Kết thúc mở (bản 1.0):** sau khi thoát, màn hình kết thúc campaign để
  ngỏ 2 hướng — công khai sự thật (rủi ro hoảng loạn diện rộng như chỉ huy
  BRDF cảnh báo) hay im lặng bỏ đi — không quyết định thay người chơi,
  chỉ đặt câu hỏi (đúng monologue ở trên) — mở đường rõ ràng cho
  DLC/Campaign 5+ khai thác hệ quả của lựa chọn đó nếu game thành công.

### 14.7 Safe Room / Điểm dừng giữa map (bổ sung — lấp khoảng trống loadout)

Phát hiện khi viết chi tiết map: cần 1 khái niệm **"Safe Room"** giữa các
map (đứng giữa 2 mission, không phải trong lúc combat) — vừa hợp lore
(chỗ nhóm nghỉ chân, khớp với "Relax" cuối mỗi map ở mục 14.2), vừa giải
quyết 1 câu hỏi thiết kế còn thiếu: **chọn/đổi vũ khí đầu mỗi map lấy từ
đâu?**

- **Cơ chế:** cuối mỗi map (đã qua "Relax", trước khi bấm extract chính
  thức sang map tiếp theo) → vào **1 phòng an toàn nhỏ, không có địch** —
  có "giá vũ khí" (armory) hiển thị các vũ khí đã unlock (progression, mục
  2) + bàn gắn attachment (mục 5/17.6) để đổi loadout trước khi tiếp tục.
  Đây chính là điểm neo tự nhiên cho UI attachment system, không cần thiết
  kế 1 màn hình loadout riêng biệt tách khỏi luồng chơi.
- Đồng thời là điểm **lưu tiến độ** (checkpoint cấp-campaign, không phải
  cấp-map — nếu rời game giữa chừng, vào lại từ Safe Room gần nhất, không
  phải chơi lại từ đầu Campaign) — dùng `match_summary`/`player_loadout`
  (mục 12) để lưu trạng thái tại đây.
- Map 5 (boss) có Safe Room **ngay trước khu vực boss fight** (đã mô tả ở
  trên) — cùng cơ chế, không cần thiết kế riêng.

### 14.8 Nguyên tắc viết cốt truyện (để nhất quán khi mở rộng)

- **Không bao giờ dừng gameplay để kể chuyện** — mọi context (briefing,
  twist) truyền qua: 1 màn hình text ngắn trước mission (không phải
  cutscene dựng cảnh, tránh tốn công AI-gen animation riêng cho cutscene)
  + environmental storytelling trong map (xác chết, ghi chú, banner) + tối
  đa 1 đoạn audio log/radio mỗi map (nội dung cụ thể đã viết ở mục 14.3,
  dùng làm mẫu cho Campaign 2-4 khi chi tiết hóa sau).
- **Nhân vật người chơi không thoại** (giữ đúng mô hình L4D — nhân vật là
  "vessel" của người chơi, không phải nhân vật có tính cách riêng do
  writer áp đặt) — tất cả lời thoại thuộc về NPC gián tiếp (radio/ghi âm),
  không phải hội thoại trực tiếp giữa các nhân vật người chơi.
- **Gieo twist Campaign 4 dần dần, không phải bất ngờ vô căn cứ** — mỗi
  audio log trong Campaign 1 (đặc biệt #3 và #4) đều có 1 câu ẩn ý về
  BRDF/Cổng thành để khi twist lộ ra ở Campaign 4, người chơi có thể quay
  lại nhớ ra manh mối đã có sẵn (kỹ thuật "fair twist" — không phải
  reveal ngẫu nhiên không báo trước).

## 15. Content scope — ngang L4D (quyết định 2026-08-27, cập nhật)

**L4D1 gốc** (tham chiếu): 4 campaign x ~5 map = **20 map**, 1 loại zombie
thường + **5 special infected** (Boomer/Hunter/Smoker/Spitter/Jockey) +
**1 boss** (Tank) + 1 special encounter (Witch) = **~8 loại địch**, **~12
vũ khí** (2 pistol, 2 shotgun, 3-4 rifle, 2 sniper, 2 SMG, melee, cộng
throwable: pipe bomb/molotov/bile bomb).

**Target scope chính thức — ngang L4D:**
- **Map:** **~20 map**, chia thành **4 campaign x 5 map** (giữ đúng cấu
  trúc L4D: mỗi campaign là 1 hành trình A→B riêng, có mở đầu/kết thúc
  riêng, nối vào cốt truyện chung ở mục 14).
- **Zombie:** zombie thường (đã có) + **~5 loại đặc biệt kiểu L4D** (vai
  trò khác nhau: 1 loại nổ/gây khói mù kiểu Boomer, 1 loại tốc độ cao/leo
  trèo kiểu Hunter, 1 loại tấn công tầm xa/kéo người kiểu Smoker, 1 loại
  gây debuff diện rộng kiểu Spitter, 1 loại tách người chơi ra khỏi nhóm
  kiểu Jockey) + **1 boss** (Tank-style, sát thương/máu rất cao) + **1
  special encounter** (đứng yên, chỉ tấn công nếu bị làm phiền, kiểu
  Witch) + **3 loại đặc biệt bổ sung tham khảo ngoài L4D** (quyết định
  2026-09-01 — Charger-style lao/tóm kéo, Clicker-style mù chỉ dò bằng
  âm thanh, Screamer-style gọi bầy rồi bỏ chạy) — tổng **~10 loại đặc
  biệt/boss/encounter** phân biệt bằng AI/kỹ năng. **Thiết kế đầy đủ (tên
  tiếng Việt, kỹ năng, ngoại hình, map gán) cho cả 10 loại này đã có ở
  `docs/ZOMBIE_BESTIARY.md`** — không dừng ở mức "kiểu XYZ" nữa. **Cộng
  thêm** (quyết định
  2026-09-01, chi tiết ở `docs/ZOMBIE_BESTIARY.md`): mỗi map có **đúng 4**
  **archetype môi trường** riêng (nhân viên văn phòng, công nhân cảng,
  lính BRDF...) — tái dùng NGUYÊN AI zombie thường, chỉ đổi model/trang
  phục (2-3 variant/archetype) + tuỳ chọn lệch nhẹ máu/tốc độ, không phải
  8 loại địch nói trên tăng thêm — mục đích thuần thị giác, để mỗi map
  không lặp lại đúng 1 hình ảnh zombie xuyên suốt 20 map.
- **Vũ khí:** nâng lên **~41 vũ khí** (quyết định 2026-09-01, chi tiết ở
  `docs/WEAPON_ARSENAL.md`) — 36 súng + 4 throwable + 1 vũ khí thực
  nghiệm gắn cốt truyện, hơn hẳn ~16 súng của L4D1+2 gộp lại. Phân theo
  **13 handling class** (súng ngắn/SMG/shotgun/AR/DMR/sniper/LMG/nỏ/
  phóng lựu/phun lửa/cận chiến nhẹ/cận chiến nặng/cưa máy) — animation
  player chỉ cần gen **theo class** (Fire/Reload/ADS), không phải theo
  từng khẩu, nên số súng tăng gấp 3+ mà chi phí animation gần như không
  đổi, đúng đòn bẩy đã dùng cho archetype zombie ở mục trên. AK47 hiện có
  là 1 khẩu trong nhóm AR. **Cộng thêm** attachment system (đã chốt ở
  mục 5) áp **theo class** (không phải theo từng khẩu) gắn lên từng
  khẩu, tạo variety nhân lên gấp bội nữa. Giai đoạn 1 (mục 16) vẫn chỉ
  cần AK47 + 1-2 khẩu khác — 41 khẩu là target Giai đoạn 3.

⚠️ **Ghi chú thực tế bắt buộc phải nêu:** scope này **lớn hơn nhiều** so
với timeline 3 tháng solo dev đã chốt ở mục 1 — 20 map + 8 loại địch (mỗi
loại cần AI-gen model + rig + animation riêng + logic AI riêng) + 12 vũ khí
+ attachment system + DDA + account system + đa nền tảng là khối lượng
công việc của cả 1 studio nhỏ, không phải 1 người trong 3 tháng theo nghĩa
đen.

**Cách xử lý đề xuất (không cắt giảm mục tiêu, chỉ chia giai đoạn):**
- Coi đây là **target scope cuối cùng** của bản 1.0 (không giới hạn thời
  gian cụ thể), còn **mốc 3 tháng** (mục 1) là **giai đoạn đầu** trong lộ
  trình dài hơn — ví dụ: 3 tháng đầu ra **1 campaign đầy đủ (5 map)** +
  **3 loại địch thường/đặc biệt + boss** + **4-5 vũ khí có attachment** +
  hệ thống lõi (DDA, account, đa nền tảng) chạy ổn định — đủ để **launch
  sớm (early access / soft launch)**, rồi bổ sung 3 campaign còn lại +
  địch/vũ khí còn thiếu dần theo các bản cập nhật hậu ra mắt (đã có sẵn
  trong mục 6 kế hoạch).
- **Cần bạn xác nhận:** đồng ý cách chia giai đoạn này (target lớn, launch
  đầu tiên là 1 campaign con), hay muốn giữ nguyên "3 tháng = launch full
  20 map luôn" (không khuyến nghị, nhưng là quyết định của bạn)?

## 16. Lộ trình theo giai đoạn (Roadmap) — quyết định 2026-08-27

Thay vì cố launch full ~18 tháng 1 lần, chia nhỏ để **có bản chơi được sớm,
lấy feedback thật, giảm rủi ro bỏ dở giữa chừng**. Mỗi giai đoạn kết thúc
bằng 1 bản build chơi được thật (không phải chỉ code xong ngầm).

### Giai đoạn 1 — 1 map hoàn chỉnh (vertical slice)
**Mục tiêu:** chứng minh core loop thật sự vui — không phải chỉ demo kỹ
thuật như hiện tại.
- **1 map duy nhất**, hoàn chỉnh: có điểm bắt đầu → gameplay → điểm
  extract, môi trường/lighting/audio đã polish (không phải greybox).
- Địch: zombie thường (đã có) + **1 loại đặc biệt: nổ/gây khói mù
  (Boomer-style)** — chọn ở mục 17.5 để làm trước, test hệ thống AI đặc
  biệt có mở rộng tốt không trước khi làm tiếp 4 loại kia.
- **Downed/Revive system** (chốt ở mục 17.1, thay cho spectate-khi-chết ở
  bản nháp trước) — người chơi mất máu về 0 thì `Downed` (nằm chờ cứu),
  không chết ngay; đồng đội cứu được — đây là 1 hạng mục việc riêng, không
  nhỏ, cần tính vào khối lượng công việc Giai đoạn 1.
- Vũ khí: AK47 (đã có) + **attachment system cơ bản** (2-3 loại attachment
  để chứng minh cơ chế, không cần đủ bộ) — chưa cần vũ khí thứ 2.
- **Account/backend tối thiểu:** Keycloak login hoạt động + Postgres lưu
  được `players`/`match_summary` cơ bản — bắt buộc vì "online ngay từ đầu"
  (mục 3), không thể hoãn sang giai đoạn sau.
- **DDA phiên bản đầu:** implement khung ở mục 13, chưa cần tune kỹ (tune
  cần dữ liệu thật từ người chơi thật, chưa có ở giai đoạn này).
- **Platform:** tiếp tục ưu tiên iOS (đã đầu tư native Metal renderer);
  desktop (macOS/Windows/Linux) chạy song song **nếu không tốn thêm việc
  đáng kể** (đa phần share code Bevy sẵn) — **chưa làm Android** (cần
  renderer bridge riêng, để giai đoạn sau).
- Character customization, matchmaking ngẫu nhiên, monetization: **chưa
  làm** ở giai đoạn này — chỉ cần room-code chơi với bạn bè là đủ để test.

### Giai đoạn 2 — Đủ 1 campaign (thêm 4 map)
- Hoàn thiện campaign đầu tiên: **5 map** tổng cộng.
- Thêm dần địch/vũ khí còn lại trong "bộ khởi đầu" (mục tiêu ~3 loại địch,
  ~4-5 vũ khí theo đề xuất MVP trước đó).
- Bắt đầu tune DDA dựa trên dữ liệu thật thu được từ giai đoạn 1.
- **Cân nhắc soft-launch/early access** ở cuối giai đoạn này nếu campaign
  đầu đã đủ vui để giữ chân người chơi.

### Giai đoạn 3 — Mở rộng nội dung (3 campaign còn lại, ~15 map)
- Hoàn thiện toàn bộ ~20 map, ~8 loại địch, ~12 vũ khí (đủ target mục 15).
- Random matchmaking (nếu chưa làm ở giai đoạn 2).
- Character customization đầy đủ hơn.

### Giai đoạn 4 — Android + đa nền tảng đầy đủ
- Xây renderer bridge cho Android (khối lượng công việc tương đương những
  gì đã bỏ ra cho iOS native Metal renderer).
- QA/polish trên đủ 5 nền tảng.

### Giai đoạn 5 — Hoàn thiện & Launch chính thức
- Chốt mô hình kiếm tiền (mục 7), tích hợp IAP đầy đủ.
- Store listing, marketing, submit App Store/Play Store.

**Nguyên tắc xuyên suốt:** mỗi giai đoạn xong mới sang giai đoạn tiếp theo
— không mở nhiều mặt trận cùng lúc (đúng bài học từ chính native Metal
renderer: làm dở 1 thứ rồi phải quay lại vá nhiều lần tốn công hơn làm
trọn vẹn từ đầu).

## 17. Plan thực thi Giai đoạn 1 (quyết định 2026-08-27)

Chốt các điểm còn mở ở mục 16 để có thể bắt tay code ngay — quyết định
dưới đây theo hướng **đơn giản nhất khả thi**, sẽ tinh chỉnh khi có dữ liệu
thật từ playtest, không cố đúng ngay từ đầu.

### 17.1 Kiến trúc Mission (mới, thay thế/mở rộng `Scenario` hiện có)

`Scenario` hiện tại (RenderCompare/HitboxTest/StressTest...) là **debug
scenario**, giữ nguyên riêng biệt — Mission là khái niệm gameplay mới,
không lẫn vào đó.

```rust
// crates/shared — state máy đơn giản, replicate qua Renet2 như GameOver
// hiện có.
enum MissionState {
    InProgress,
    Completed,   // cả nhóm đã tới extract
    Failed,      // wipe — xem 17.2
}

// Extract point: 1 volume/trigger cố định trong map (giống spawn_test_arena
// định nghĩa tường bằng code — extract zone cũng định nghĩa bằng 1 Vec3 +
// radius trong map data, không cần mesh phức tạp cho MVP).
struct ExtractZone {
    center: Vec3,
    radius: f32,
}
```

- **Extract rule (co-op) — chọn "chờ cả nhóm" kiểu L4D safe room:**
  người chơi đầu tiên vào `ExtractZone` không kết thúc mission ngay — bắt
  đầu đếm ngược (vd 10s) hiển thị cho tất cả; nếu **toàn bộ** người chơi
  còn sống đứng trong zone khi đếm hết → `MissionState::Completed`. Lý do
  chọn cách này: đơn giản hơn transport/vehicle thật của L4D, vẫn giữ được
  cảm giác "phải đợi đồng đội" tạo tension.
- **Downed / Revive (quyết định 2026-08-27 — thay cho spectate-khi-chết ở
  bản nháp trước):** giữ đúng tinh thần L4D thay vì đơn giản hóa.
  ```rust
  enum PlayerLifeState {
      Alive,
      Downed { bleed_out_timer: f32 },  // đếm ngược, hết giờ mà chưa được
                                          // cứu -> Dead
      Dead,                              // hết cứu được, ra khỏi vòng
                                          // chiến (không phải spectate rời
                                          // phòng, vẫn xem tiếp tới khi
                                          // mission kết thúc)
  }
  ```
  - Máu về 0 lần đầu → `Downed` (không chết ngay): nằm tại chỗ, không tự
    di chuyển được, bắn được nhưng damage/tầm bắn giảm mạnh (kiểu súng
    ngắn cầm tay lết trên L4D), đếm ngược `bleed_out_timer` (đề xuất 60s).
  - Đồng đội **đứng gần + giữ nút "hồi sinh" trong X giây** (đề xuất 3-4s,
    không thể bắn/di chuyển trong lúc hồi sinh đồng đội — tạo rủi ro/quyết
    định chiến thuật) → về lại `Alive` với % máu thấp (không full máu,
    tránh lạm dụng revive liên tục làm mất tính sát thương của mất máu).
  - Hết `bleed_out_timer` mà chưa được cứu, hoặc bị zombie đặc biệt tấn
    công thẳng lúc đang `Downed` (instakill kiểu Hunter pounce của L4D,
    để giai đoạn sau khi có zombie đó) → chuyển `Dead`.
  - `Dead` khác solo cũ: **không rời phòng ngay**, chuyển sang camera xem
    theo góc đồng đội còn sống (spectate), chờ mission kết thúc — vẫn tính
    là "đã tham gia trận" cho `match_summary`, nhưng không tính vào điều
    kiện "còn sống" ở Extract Zone (17.1) hay điều kiện fail (17.2 dưới).

### 17.2 Wipe & Fail

- **Điều kiện fail:** **toàn bộ** người chơi trong phòng đều ở trạng thái
  `Dead` (đã hết cơ hội cứu, không phải `Downed` — còn `Downed` nghĩa là
  còn cơ hội cứu, chưa fail) trước khi tới extract.
- **Khi fail:** ghi `match_summary.completed = false` (schema mục 12) →
  cập nhật `skill_rating` giảm nhẹ theo công thức mục 13 → toàn bộ người
  chơi được đưa về **màn hình kết quả** (stats trận vừa rồi) → nút "Thử
  lại" (mission restart, spawn zombie mới, giữ nguyên loadout) hoặc "Về
  sảnh" (rời phòng).
- **Solo:** không có đồng đội cứu → `Downed` chắc chắn hết giờ thành
  `Dead` → fail ngay lập tức, không có ai để chờ. Cơ chế downed/revive vẫn
  chạy (vì đây là 1 state máy chung, không rẽ nhánh riêng cho solo), chỉ
  là không ai bấm nút cứu được nên luôn dẫn tới `Dead`.

### 17.3 Reconnect

- **Giai đoạn 1: cho phép reconnect trong khi mission còn `InProgress`** —
  nhân vật của người rớt mạng đứng yên tại chỗ (server vẫn giữ entity,
  không despawn ngay) trong 1 khoảng chờ (vd 60s); nếu người chơi
  reconnect trong lúc đó (cùng `player_id` từ JWT) → chiếm lại đúng entity
  đó, tiếp tục chơi ở đúng `PlayerLifeState` lúc rớt mạng (kể cả đang
  `Downed` — không tự hồi sinh chỉ vì mất kết nối). Quá 60s không quay lại
  → coi như `Downed` hết giờ bình thường (chuyển `Dead` theo đúng luật
  17.1, không có xử lý đặc biệt gì thêm cho trường hợp rớt mạng).
- Đây là lý do `player_id` (Keycloak `sub`) phải là identity xuyên suốt
  connection, không phải Renet2 client_id tạm thời — cần thiết kế map
  `client_id ↔ player_id` ở server ngay từ đầu.

### 17.4 Server hosting Giai đoạn 1

- **Self-host tại máy cá nhân** (không phải VPS trả phí — ngân sách 0 theo
  mục 6), expose qua **playit.gg** (chọn thay vì Tailscale — quyết định
  2026-08-27): tunnel công khai, người test chỉ cần 1 link/địa chỉ, không
  phải cài thêm phần mềm VPN hay được mời vào tailnet riêng — thuận tiện
  hơn để mời người ngoài (không chỉ bạn bè kỹ thuật) test thử. Đánh đổi:
  ít kiểm soát ai kết nối được hơn Tailscale — chấp nhận được cho quy mô
  test nhỏ ở Giai đoạn 1, cần thêm rate-limit/kick cơ bản ở server nếu
  thấy bị lạm dụng.
- Đây **CHỈ dùng cho Giai đoạn 1** (test nội bộ/bạn bè/người lạ số lượng
  nhỏ) — không phải hạ tầng cho soft-launch công khai (Giai đoạn 2 mới cần
  cân nhắc VPS thật, vì lúc đó cần khả năng chịu tải nhiều người chơi đồng
  thời hơn playit.gg free tier đáp ứng được).

### 17.5 Chọn zombie đặc biệt đầu tiên

**Chọn: loại "nổ/gây khói mù" (Boomer-style)** — lý do chọn cái này làm
trước trong số 5 loại ở mục 15:
- **AI đơn giản nhất:** không cần pathing đặc biệt (không kéo/bay/leo trèo
  khác zombie thường) — chỉ cần: tiến gần → nổ (AoE) khi chết hoặc khi đủ
  gần người chơi. Tái dùng gần như nguyên vẹn AI di chuyển/aggro đã có của
  zombie thường (`crates/shared/src/zombie.rs`, `move_zombies` server-
  side).
- **Test đúng thứ cần test:** chứng minh hệ thống "zombie đặc biệt" mở
  rộng được từ codebase zombie hiện có (thêm 1 component `SpecialZombie`
  hoặc enum variant, 1 hiệu ứng AoE khi chết) mà không phải viết lại AI
  framework — nếu cái đơn giản nhất đã khó mở rộng, biết sớm để sửa kiến
  trúc trước khi làm tiếp 4 loại phức tạp hơn (Hunter/Smoker/Spitter/
  Jockey đều cần cơ chế mới hẳn: nhảy/kéo dây/tấn công tầm xa/tách nhóm).
- **Cơ chế cụ thể (MVP):** khi chết (bị bắn hết máu) HOẶC khi vào bán kính
  X so với người chơi bất kỳ → nổ, gây 1 vùng khói/khí độc tồn tại N giây,
  người chơi đứng trong đó bị giảm tầm nhìn (fog/particle overlay) + mất
  máu nhẹ theo thời gian. Tái dùng particle system đã có (`update_smoke`,
  `spawn_spark_effects` — đã thấy trong code) làm nền cho hiệu ứng khói.

### 17.6 Attachment system — cơ chế gameplay cụ thể (MVP)

Giai đoạn 1 chỉ cần **2-3 attachment** để chứng minh cơ chế, map thẳng vào
các hằng số **đã có sẵn** trong `crates/client/src/lib.rs` (không cần
thiết kế field mới, chỉ cần nhân hệ số theo attachment đang gắn):

| Attachment | Ảnh hưởng | Field/hằng số liên quan (đã có trong code) |
|---|---|---|
| Ống ngắm đỏ (Red Dot) | Giảm `BULLET_SPREAD` khi ADS | nhân hệ số vào `current_bullet_spread` |
| Băng đạn mở rộng | Tăng `MAGAZINE_SIZE` | cộng thẳng vào field trên `Gun` |
| Báng nặng | Giảm `RECOIL_PITCH_KICK`/`RECOIL_ROLL_DEG` | nhân hệ số giật khi bắn |

- Lưu trong `player_loadout.attachments` (JSONB, mục 12) dạng
  `{"scope": "reddot", "mag": "extended"}` — server đọc loadout lúc spawn
  gun, áp hệ số tương ứng vào `Gun` component 1 lần (không phải tính lại
  mỗi frame) → không đổi hot-path gameplay hiện có, chỉ thêm 1 bước "áp hệ
  số theo loadout" tại thời điểm spawn.

### 17.7 DDA baseline khởi điểm (trước khi có dữ liệu thật)

- Tự đặt tay 3 mốc độ khó cho map đầu tiên: **Dễ/Vừa/Khó**, mỗi mốc là 1
  bộ số (spawn rate zombie, tốc độ zombie theo `ZOMBIE_WALK_SPEED`/
  `ZOMBIE_RUN_SPEED` đã có, máu zombie) — chọn **Vừa làm baseline mặc
  định** cho người chơi mới (chưa có `player_skill` row nào).
  - Tầng ngắn hạn (mục 13.1) dao động quanh mốc Vừa trong 1 trận.
  - Tầng dài hạn (mục 13.2): sau vài trận, `skill_rating` lệch dần khỏi
    1000 (baseline) → server nội suy giữa 3 mốc Dễ/Vừa/Khó theo
    `skill_rating` thay vì cần thêm mốc mới.

### 17.8 Kiến trúc Server đa phòng (quyết định 2026-08-27)

Server hiện tại (`crates/server/src/main.rs`) xử lý **1 world duy nhất**
mỗi khi chạy — không đủ khi cần nhiều phòng (bạn A+B chơi co-op, bạn C tự
chơi solo) hoạt động **đồng thời**, độc lập với nhau.

- **Chọn: 1 process game-server = 1 phòng, spawn động** (thay vì 1
  process quản lý nhiều room trong cùng world logic). Lý do: gần như
  **không cần sửa gì trong `crates/server` hiện có** — mỗi phòng vẫn là 1
  world độc lập đúng như code đang giả định (global resource, 1
  `AutoMaintainZombies`, 1 `Director`... đều không cần mang theo
  `room_id` để phân biệt). Đánh đổi: tốn tài nguyên hơn khi có nhiều
  phòng (mỗi process là 1 Bevy App headless riêng) — chấp nhận được ở quy
  mô test Giai đoạn 1 (vài phòng cùng lúc), sẽ cần đánh giá lại ở Giai
  đoạn 2-3 khi scale lên nhiều người chơi đồng thời hơn.
- **Thành phần mới cần thêm — "Lobby Service"** (KHÔNG PHẢI game server,
  đứng riêng): 1 service nhẹ (có thể là 1 Rust binary nhỏ khác, hoặc 1
  route thêm vào cùng backend Keycloak-adjacent) chịu trách nhiệm:
  1. Nhận request "tạo phòng" từ client (đã xác thực qua JWT Keycloak) →
     `spawn` 1 tiến trình `crates/server` mới, gán 1 port UDP còn trống
     **lấy từ pool cố định đã cấu hình tunnel playit.gg sẵn** (xem
     "Port pool" ngay dưới — quyết định 2026-08-27, không tự động hóa tạo
     tunnel qua playit API ở Giai đoạn 1).
  2. Sinh **room code** ngắn (vd 6 ký tự) map tới `(host, port)` đó, lưu
     tạm trong bộ nhớ (chưa cần Postgres cho cái này — phòng là tạm thời,
     sống-chết theo phiên chơi, không phải dữ liệu cần persist).
  3. Trả **địa chỉ public qua playit.gg** (không phải port nội bộ) cho
     client khi có ai "join phòng" bằng room code đúng, hoặc qua
     matchmaking ngẫu nhiên (ghép vào phòng đang chờ người, ưu tiên theo
     `skill_rating` gần nhau — mục 13.2 — nếu có nhiều phòng đang chờ).
  4. Theo dõi tiến trình game-server đã spawn — khi nó tự thoát (mission
     xong, mọi người rời phòng) → giải phóng port trong pool, xóa room
     code.
  5. Nếu **hết port trong pool** (đã đạt giới hạn đồng thời) → trả lỗi rõ
     ràng "server đang đầy, thử lại sau" cho client, không phải crash/lỗi
     mơ hồ.

- **Port pool (quyết định 2026-08-27):** cấu hình tay sẵn **10 tunnel
  playit.gg cố định**, mỗi tunnel trỏ 1 port UDP nội bộ khác nhau (vd
  6000-6009) → **tối đa 10 phòng chơi đồng thời** ở Giai đoạn 1. Không
  cần tự động hóa tạo tunnel qua playit.gg API — vừa đủ cho quy mô test
  bạn bè/nhóm nhỏ, tránh tốn công tích hợp thêm 1 API bên ngoài chỉ để
  giải quyết vấn đề chưa xảy ra ở quy mô này. Cần đánh giá lại (có thể
  cần playit.gg Pro hoặc chuyển hẳn sang VPS thật) khi lên Giai đoạn 2-3.
- **Vòng đời phòng khi host rời (mục còn mở trước đó, chốt tại đây):**
  vì đây là kiến trúc **server-authoritative theo process**, không có khái
  niệm "host" đặc biệt ở tầng network (khác các game peer-to-peer cần host
  migration) — người tạo phòng chỉ là người đầu tiên gọi API "tạo phòng",
  sau đó **mọi người chơi bình đẳng** kết nối vào cùng 1 game-server
  process. Người tạo phòng rời đi **không** làm sập phòng — game-server
  process vẫn chạy cho tới khi **tất cả** người chơi rời/mission kết thúc.
  Không cần thiết kế host migration.

### 17.9 UI nhập text — tự viết trong bevy_ui (quyết định 2026-08-27)

Chọn tự viết text input widget (thay vì né qua OAuth browser flow) — cả
login (username/password) và nhập mã phòng đều dùng UI trong game.

- **Cấu trúc kỹ thuật đề xuất:** 1 component `TextInputField { buffer:
  String, cursor: usize, focused: bool, masked: bool }` (dùng `masked:
  true` cho password field — hiện `•` thay vì ký tự thật) gắn trên 1
  `Node`/`Text` bevy_ui bình thường.
- **Input desktop:** đọc `bevy::input::keyboard::KeyboardInput` message
  (không phải `ButtonInput<KeyCode>` — cái đó chỉ biết phím đang giữ, cần
  `KeyboardInput`'s `logical_key`/text event để lấy đúng ký tự đã gõ, kể
  cả ký tự có dấu/shift).
- **Input mobile:** cần trigger **bàn phím ảo hệ thống** khi field được
  chạm — trên iOS qua `UITextField`/`UITextView` native (giống cách
  `GameControllerInput.swift`/`ViewController.swift` đã cầu nối Swift↔Rust
  cho input khác), KHÔNG tự vẽ bàn phím ảo bằng bevy_ui (tốn công vô ích,
  UX kém hơn bàn phím hệ thống nhiều). Đây là hạng mục cầu nối Swift mới,
  tương tự cách `TouchAimButton` cầu nối touch hiện có nhưng phức tạp hơn
  (cần callback text-changed từ Swift về Rust).
- **native Metal renderer riêng cần vẽ được ô nhập liệu** — dùng lại kỹ
  thuật `ndc_ui_rect`/font atlas đã có (`UI_SHADER_SOURCE`) để vẽ khung +
  con trỏ nhấp nháy + text đã gõ, không cần thêm gì mới về render, chỉ
  thêm 1 snapshot struct (`TextInputSnapshot`) theo đúng pattern
  `TouchButtonSnapshot` đã có.
- **Rủi ro lớn nhất của lựa chọn này:** input text đa nền tảng (đặc biệt
  bàn phím ảo mobile + IME cho ngôn ngữ có dấu) là 1 trong những phần
  "nhìn đơn giản nhưng dễ tốn thời gian nhất" khi tự viết từ đầu — cần
  chấp nhận đây có thể là 1 trong các mốc trễ tiến độ nếu phát sinh vấn đề
  ngoài dự kiến (ví dụ safe-area/bàn phím che UI trên iPhone, khác biệt
  keyboard event giữa desktop/mobile).

### 17.10 Lựa chọn kỹ thuật cụ thể (Rust crates & tooling)

Quyết định thuần kỹ thuật, không cần bạn duyệt lại trừ khi có phản đối cụ
thể — chọn theo tiêu chí "ổn định, phổ biến trong ecosystem Bevy/Rust,
không thêm phụ thuộc nặng không cần thiết":

- **JWT validation (server đọc token Keycloak):** crate `jsonwebtoken` —
  validate chữ ký RS256 bằng public key lấy từ JWKS endpoint của Keycloak,
  cache JWKS trong bộ nhớ (refresh định kỳ, vd mỗi giờ hoặc khi gặp `kid`
  lạ) để không phải gọi Keycloak mỗi request.
- **Kết nối Postgres:** crate `sqlx` (async, compile-time query checking,
  không cần ORM nặng như Diesel — phù hợp schema đơn giản đã thiết kế ở
  mục 12) + `sqlx-cli` cho migration (file SQL thuần trong thư mục
  `migrations/`, chạy `sqlx migrate run` — dễ review diff schema qua git).
- **Lobby Service:** viết như 1 binary Rust riêng trong cùng workspace
  (`crates/lobby` mới) — dùng `axum` (đã nhẹ, phổ biến, dễ tích hợp
  `sqlx`/`jsonwebtoken`) cho HTTP API (tạo phòng/join phòng/matchmaking),
  KHÔNG dùng Renet2/UDP cho lobby (lobby chỉ cần request-response đơn
  giản, HTTP/JSON là đủ và dễ debug hơn nhiều so với thêm 1 giao thức UDP
  thứ hai).
- **Spawn process con:** `std::process::Command` thuần (không cần crate
  ngoài) — Lobby Service gọi `cargo run --bin server -- --port 6003`
  (hoặc gọi thẳng binary đã build sẵn ở production, không qua `cargo run`
  — nhanh hơn và không phụ thuộc toolchain Rust có mặt lúc runtime).

### 17.11 Chi tiết Downed/Revive (điền nốt input + animation)

- **Input "giữ nút hồi sinh đồng đội":**
  - Desktop: giữ phím **F** (tương tác chung — tái dùng, không cần phím
    mới nếu sau này có thêm tương tác khác như nhặt đồ, dùng chung 1 phím
    "interact" theo ngữ cảnh, giống nhiều FPS khác làm).
  - Mobile: 1 nút chạm mới **"REVIVE"** xuất hiện **theo ngữ cảnh** (chỉ
    hiện khi đứng đủ gần 1 đồng đội đang `Downed`, giống cách nút ADS/
    Reload hiện tại luôn có mặt — nút này khác ở chỗ ẩn/hiện động, cần 1
    system mới kiểu `update_revive_button_visibility` theo dõi khoảng
    cách tới đồng đội `Downed` gần nhất).
- **Animation/Pose Downed:** cần thêm 1 pose mới cho player model
  (`PlayerModelAnim`, đã có `idle/walk/run/shoot/death` — thêm `downed`).
  Vì asset là AI-gen (mục 8), pose `downed` cần được generate cùng lúc
  với các pose khác của model nhân vật, không thể thiếu sót tới lúc code
  animation mới phát hiện ra chưa có clip này — **cần đưa vào brief khi
  gen asset nhân vật ngay từ đầu**, tương tự cách zombie hiện có sẵn
  `ZOMBIE_ANIM_BITE/CRAWL/IDLE/RUN/WALK` đủ bộ ngay từ khi model được tạo.
- **Camera/view khi Downed:** hạ thấp `EYE_HEIGHT_OFFSET` (hằng số đã có
  trong `crates/client/src/lib.rs`) tạm thời trong lúc `Downed` — nhân vật
  nằm/bò thì góc nhìn phải thấp xuống tương ứng, tái dùng đúng cơ chế
  chuyển đổi eye-height đã có sẵn cho Crouch/Prone (`CROUCH_EYE_HEIGHT_
  OFFSET`/`PRONE_EYE_HEIGHT_OFFSET`) — Downed chỉ là 1 giá trị offset mới
  trong cùng hệ thống, không cần cơ chế riêng.

### 17.12 Room lifecycle & giới hạn (quyết định 2026-08-27)

- **Sĩ số tối đa mỗi phòng: 4 người** (đúng chuẩn L4D) — áp dụng luôn cho
  cả co-op tạo phòng bạn bè lẫn matchmaking ngẫu nhiên.
- **State máy của 1 room** (ở Lobby Service, không phải Mission State ở
  17.1 — 2 cái khác tầng: room tồn tại trước khi mission bắt đầu, sống
  qua nhiều lần retry):
  ```rust
  enum RoomState {
      Waiting,       // đang chờ đủ người / chờ host bấm "Bắt đầu"
      InMission,     // game-server đang chạy 1 mission (map 17.1's
                      // MissionState sống bên trong giai đoạn này)
      Closed,        // toàn bộ người chơi đã rời — Lobby giải phóng port
  }
  ```
  - `Waiting → InMission`: khi host bấm "Bắt đầu" (không tự động bắt đầu
    ngay khi đủ 4 người — vẫn cho phép nhóm ít hơn 4 chủ động chơi luôn
    nếu muốn, đúng yêu cầu "chơi 1 mình cũng tự tạo phòng").
  - `InMission → Waiting`: khi mission `Completed` — quay lại sảnh phòng
    (không tự đóng room) để chuẩn bị map/mission tiếp theo (áp dụng từ
    Giai đoạn 2 trở đi khi có nhiều map nối tiếp trong 1 campaign).
  - **"Thử lại" sau Fail (quyết định 2026-08-27) — giữ nguyên phòng,
    KHÔNG tạo phòng mới:** `InMission (Failed) → InMission` trực tiếp —
    game-server process reset lại state mission (respawn zombie, hồi máu
    player, `MissionState::InProgress` lại) mà không thoát process, không
    cần round-trip qua Lobby Service, không cần chia sẻ lại room code.
    Đây là lý do quan trọng khiến "1 process = 1 phòng" (17.8) phải sống
    **qua nhiều lần chơi trong cùng phòng**, không phải chỉ 1 lần chơi rồi
    thoát — cần 1 message/RPC nội bộ game-server "reset mission" thay vì
    chỉ có "mission kết thúc → thoát process".
  - `* → Closed`: người chơi cuối cùng ngắt kết nối (không phải Downed —
    ngắt kết nối Renet2 thật sự) → game-server tự thoát → Lobby giải
    phóng port.

### 17.13 Luồng kết nối Client ↔ Lobby ↔ Game-server (mới, lấp khoảng trống)

Kiến trúc trước giờ mới nói "Lobby trả về địa chỉ game-server", nhưng
chưa nói **client gọi Lobby bằng cách nào** — cần chi tiết hóa toàn bộ
luồng, vì đây là thay đổi kiến trúc lớn so với client hiện tại (đang nối
thẳng 1 địa chỉ server cố định qua `SERVER_ADDR` — mục "Đã kiểm tra App
Store readiness" trước đây).

1. **Lobby Service cũng cần lộ ra ngoài Internet** — thêm 1 tunnel
   playit.gg thứ 11 (ngoài 10 tunnel UDP cho game-server ở 17.8), nhưng
   loại **TCP/HTTP** thay vì UDP, trỏ vào cổng HTTP cố định Lobby Service
   lắng nghe (vd 8080). Địa chỉnh Lobby này **hardcode trong client build**
   (giống cách `SERVER_ADDR` từng hardcode, nhưng giờ chỉ hardcode ĐỊA CHỈ
   LOBBY, không hardcode địa chỉ game-server nữa — địa chỉ game-server
   giờ là **động**, Lobby trả về runtime).
2. **HTTP client trong Bevy app:** dùng crate **`ureq`** (blocking, đơn
   giản) chạy trong 1 **background thread** qua Bevy's `AsyncComputeTaskPool`
   (đã có sẵn pattern tương tự cho tác vụ nặng khác trong Bevy ecosystem)
   — KHÔNG gọi HTTP trực tiếp trên main thread (sẽ đứng hình 1 frame chờ
   network). Không chọn `reqwest` (cần runtime async Tokio riêng, nặng
   hơn cần thiết cho vài request đơn giản: login, tạo phòng, join phòng).
3. **Luồng cụ thể khi mở app:**
   - App khởi động → check có JWT đã lưu từ lần trước không (17.16) → có
     thì thử refresh token luôn (skip màn login) → không có/hết hạn hẳn
     thì hiện màn Login (UI nhập text, mục 17.9).
   - Login thành công (Lobby xác thực hộ qua Keycloak, hoặc client tự gọi
     thẳng Keycloak token endpoint rồi chỉ gửi JWT cho Lobby — **chọn cách
     sau**: client gọi thẳng Keycloak, Lobby không cần biết password) →
     lưu JWT.
   - Màn hình chính: nút "Tạo phòng" (gọi `POST /rooms` tới Lobby, kèm
     JWT) hoặc "Nhập mã phòng" (UI nhập text, gọi `POST /rooms/{code}/join`)
     hoặc "Chơi ngẫu nhiên" (`POST /matchmaking/queue`).
   - Lobby trả về `{game_server_host, game_server_port}` (địa chỉ public
     qua playit.gg tunnel tương ứng, không phải port nội bộ).
   - Client dùng địa chỉ đó để mở kết nối **Renet2** như cũ (phần này
     KHÔNG đổi — chỉ đổi chỗ lấy địa chỉ, từ hardcode/env var sang gọi
     Lobby runtime).

### 17.14 Lưu JWT giữa các lần mở app

- **iOS/macOS:** Keychain (qua crate `security-framework` hoặc gọi thẳng
  Keychain Services API từ Swift, lưu rồi cho Rust đọc qua FFI — tương tự
  cách `cadisplaylink` cầu nối Swift↔Rust hiện có cho input).
- **Windows/Linux/Android:** dùng crate `keyring` (đã hỗ trợ cross-platform
  credential storage: Windows Credential Manager, libsecret trên Linux,
  Android Keystore) — **KHÔNG lưu JWT dạng plaintext trong file** (dù chỉ
  là refresh token, vẫn là thứ có thể chiếm đoạt tài khoản nếu rò rỉ).
- Giai đoạn 1 có thể tạm chấp nhận **file plaintext trong thư mục
  save-data riêng của app** nếu tích hợp `keyring`/Keychain tốn quá nhiều
  thời gian so với lợi ích ở quy mô test nhỏ — nhưng đây là **nợ kỹ thuật
  phải trả trước khi mở rộng người dùng thật** (Giai đoạn 2+), ghi rõ vào
  mục 18 (nợ kỹ thuật, thêm bên dưới) để không quên.

### 17.15 Giám sát process game-server con (health check)

- Lobby Service cần biết game-server con **còn sống hay đã chết/crash**
  để giải phóng port kịp thời (không để port "rò rỉ" — bị chiếm giữ vĩnh
  viễn bởi 1 process đã chết mà Lobby không biết).
- **Cơ chế:** poll `Child::try_wait()` (API chuẩn của
  `std::process::Command`) mỗi vài giây trong 1 task nền của Lobby Service
  — không cần thêm giao thức heartbeat riêng giữa Lobby và game-server ở
  Giai đoạn 1 (đơn giản hơn, đủ dùng vì chỉ chạy trên 1 máy, không phải
  qua network riêng biệt để check).
- **Khi Lobby Service tự khởi động lại** (restart do deploy bản mới, hoặc
  crash) — **các game-server con đang chạy từ trước sẽ bị mồ côi** (Lobby
  mất hết state `RoomState` trong bộ nhớ). Giai đoạn 1 chấp nhận: cần 1
  thao tác thủ công dọn dẹp (`pkill server` trước khi khởi động lại Lobby)
  — không tự động hóa việc "Lobby mới nhận diện lại process cũ" (phức tạp
  không cần thiết ở quy mô test).

### 17.17 Gameplay economy (quyết định 2026-08-27)

- **Đạn dược: giới hạn, nhặt trong map** (kiểu L4D) — mỗi vũ khí có
  `magazine_size` (đã có, vd `MAGAZINE_SIZE` cho AK47) VÀ 1 pool đạn dự
  trữ tổng (reload trừ vào pool này, hết pool = hết đạn hẳn, không chỉ hết
  băng). Item pickup "hộp đạn" rải trong map hồi 1 phần pool. **Hệ quả cho
  level design (mục 17 nói chung):** mỗi map cần đặt trước vị trí pickup
  đạn — không thể hoàn thiện map nghệ thuật (bước 9 dưới) mà thiếu bước
  đặt item, cần tính vào khối lượng công việc làm map, không phải việc
  tách biệt.
- **Máu/Health items:** cùng logic — thêm item "bộ y tế" (med kit) hồi %
  máu, đặt rải rác trong map (thường ở gần điểm giao tranh lớn/trước
  boss, đúng L4D placement convention: đặt y tế trước thử thách khó, không
  đặt ngẫu nhiên). Không có sẵn trong code hiện tại — cần 1 `HealthPickup`
  component + trigger tương tự cách item khác sẽ được thêm.
- **Friendly fire: BẬT, giảm damage** (không phải 100% sát thương như bắn
  zombie) — hệ số giảm cụ thể để playtest sau (đề xuất khởi điểm: 20-30%
  sát thương thường so với bắn zombie), tính vào `zombie_hit_damage_ray`/
  tương đương logic nhưng target là `Player` khác thay vì `Zombie`. Cần
  thêm 1 check "không tự bắn trúng chính mình" (loại trừ chủ shot ra khỏi
  danh sách target hợp lệ).
- **Autopilot trong co-op (khác hẳn vai trò debug-tool solo cũ):** khi 1
  người bật AFK giữa trận, nhân vật họ **chuyển sang autopilot chiến đấu
  hộ** (dùng lại `autopilot.rs` hiện có — vốn thiết kế cho solo/debug,
  giờ cần chạy đồng thời cho **nhiều entity độc lập cùng lúc** trong 1
  world co-op, không phải chỉ 1 người chơi local). Cần kiểm tra: hệ thống
  hiện tại (`Autopilot`/`AutopilotIntent`/`AutopilotState` là resource
  toàn cục theo tài liệu code cũ, giả định "chỉ có 1 người chơi local
  cần autopilot") — **có khả năng cần refactor thành component-per-player**
  thay vì resource toàn cục, để nhiều người AFK cùng lúc trong 1 phòng
  không đá nhau. **Đây là rủi ro kỹ thuật cần đánh giá kỹ trước khi làm**
  (bước 6 trong thứ tự thực thi, mục 17.18).

### 17.18 Kiến trúc Zombie đặc biệt (ECS, lấp khoảng trống 17.5)

- **Component mới:** `SpecialZombieKind` enum (`Boomer` cho MVP, mở rộng
  dần `Hunter/Smoker/Spitter/Jockey` ở Giai đoạn 3) gắn thêm lên entity
  `Zombie` đã có — zombie thường không có component này (query
  `Without<SpecialZombieKind>` để phân biệt, giữ nguyên toàn bộ code
  zombie thường không đổi).
- **AI:** tái dùng nguyên `move_zombies`/pathing hiện có cho di
  chuyển/aggro — chỉ thêm 1 system mới `special_zombie_behavior` chạy
  riêng cho entity có `SpecialZombieKind`, xử lý phần khác biệt (nổ khi
  chết/khi đủ gần). Không sửa system zombie thường.
- **Native Metal renderer (đã có pattern sẵn để tái dùng — không phải
  việc mới):** `native_metal_probe.rs` đã có `skinned_models: HashMap<
  &str, SkinnedModel>` tra theo tên model — zombie Boomer chỉ cần đăng ký
  thêm 1 entry mới (`"zombie_boomer"`) trỏ tới model AI-gen riêng, đúng
  hệt cách zombie thường/soldier hiện tại đã đăng ký — không cần sửa kiến
  trúc renderer.
- **Đối chiếu với `Director`/`run_ai_director` đã tắt (`crates/server/
  src/main.rs`, comment "TEMPORARY disabled — stacking hordes on top of
  ZombieTargetCount"):** hệ thống DDA mới (mục 13) **thay thế hẳn**
  `Director` cũ, không chạy song song — khi implement DDA (bước 8 dưới),
  cần xóa hẳn `run_ai_director`/`Director` (đang dead code) thay vì để
  tồn tại song song 2 hệ thống điều chỉnh độ khó gây xung đột lẫn nhau
  (đúng lý do gốc nó bị tắt).

### 17.19 Thứ tự thực thi đề xuất cho Giai đoạn 1 (cập nhật)

1. **Backend tối thiểu** (17.4 + mục 12 + crate choices ở 17.10): Keycloak
   login (server-side JWT validate qua `jsonwebtoken`) + Postgres schema
   qua `sqlx` — làm trước vì mọi thứ khác (mission result, loadout) đều
   ghi vào đây.
2. **UI nhập text** (17.9) — làm sớm ngay sau backend vì login/tạo-join
   phòng đều cần nó trước khi có thể test bất kỳ luồng multiplayer nào
   bằng tay.
3. **Lobby Service + spawn server động** (17.8/17.13, crate `axum`) — cần
   trước mission state machine vì cần có cách thực sự tạo/join 1 phòng để
   test.
4. **Mission state machine** (17.1/17.2/17.12, bao gồm cả `PlayerLifeState`
   Alive/Downed/Dead + revive, room lifecycle ở 17.12) trên map test hiện
   có (`spawn_test_arena`) — chưa cần map nghệ thuật đẹp, chỉ cần chứng
   minh luồng `InProgress → Completed/Failed`, downed/revive, và "Thử lại
   giữ nguyên phòng" chạy đúng. Đây là hạng mục nặng nhất trong Giai đoạn
   1, nên tách làm 2 bước con: (a) extract + win/fail cơ bản không có
   downed trước (chết = Dead luôn, tạm thời) để có 1 vòng chơi được sớm,
   (b) thêm downed/revive + retry-giữ-phòng vào sau khi (a) đã chạy ổn.
5. **Reconnect** (17.3) — làm sớm vì càng để muộn càng khó chèn vào state
   machine đã phức tạp hơn.
6. **Autopilot co-op** (17.17) — đánh giá rủi ro refactor
   resource-toàn-cục → component-per-player **trước khi** làm các bước
   sau, vì nếu cần refactor lớn thì ảnh hưởng ước tính thời gian toàn bộ
   Giai đoạn 1.
7. **Zombie Boomer-style** (17.5/17.18) — mở rộng hệ thống zombie hiện có.
8. **Attachment system MVP** (17.6) — 2-3 attachment đầu tiên.
9. **Đạn dược/Health pickup + Friendly fire** (17.17) — hệ thống item mới.
10. **DDA khung + baseline tay** (17.7) — **xóa `Director`/`run_ai_director`
    cũ** cùng lúc (17.18) thay vì giữ song song.
11. **Map nghệ thuật thật** (thay thế arena test, dựa theo Map 1 "Tòa nhà
    văn phòng" — mục 14.1) — làm SAU CÙNG, khi toàn bộ hệ thống (1-10) đã
    chạy đúng trên map placeholder — tránh vừa chỉnh gameplay vừa chỉnh
    map cùng lúc, dễ lẫn lộn nguyên nhân khi có bug. Bao gồm đặt vị trí
    item đạn/y tế (17.17) như 1 phần của việc làm map, không tách riêng.

## 18. Nợ kỹ thuật đã biết trước (track để không quên)

- **Lưu JWT plaintext tạm thời ở Giai đoạn 1** (nếu không kịp tích hợp
  Keychain/`keyring` ngay — mục 17.14) — phải thay bằng lưu trữ bảo mật
  thật trước khi mở rộng người dùng thật ở Giai đoạn 2+.
- **Lobby Service mất state khi restart** (mục 17.15) — chấp nhận thao
  tác thủ công dọn process mồ côi ở Giai đoạn 1, cần cơ chế tự phục hồi
  nếu scale lên nhiều phòng hơn ở Giai đoạn 2-3.
- **Port pool cố định 10 tunnel playit.gg** (mục 17.8) — giới hạn cứng 10
  phòng đồng thời, cần đánh giá lại hạ tầng (playit.gg Pro/VPS thật) khi
  vượt quy mô test.
- **`Director`/`run_ai_director` cũ cần xóa hẳn** khi implement DDA (mục
  17.18) — hiện đang là dead code (`#[warn(dead_code)]`), không được để
  tồn tại song song với DDA mới gây xung đột logic.
- **`crates/client`'s `ARENA_HALF_SIZE`/`ARENA_WALL_HEIGHT`/
  `ARENA_WALL_THICKNESS`** — đã fix 1 lần (session 2026-08-27, import từ
  `shared` thay vì bản copy cục bộ) nhưng đáng để **rà soát toàn bộ file
  `crates/client/src/lib.rs`** tìm hằng số nào khác có thể còn bị
  duplicate cục bộ thay vì import từ `shared`, phòng khi map/scale thay
  đổi lại ở Giai đoạn 3.
- **Autopilot resource-toàn-cục có thể cần refactor thành
  component-per-player** cho co-op nhiều người AFK cùng lúc (mục 17.17) —
  đây có thể không phải "nợ" mà là việc **bắt buộc phải làm** ngay ở Giai
  đoạn 1 nếu đánh giá (bước 6, mục 17.19) xác nhận kiến trúc hiện tại
  không chịu được nhiều entity autopilot cùng lúc — ghi ở đây để không
  quên đánh giá sớm, tránh phát hiện muộn giữa chừng code các bước sau.

---
*Cập nhật file này bất cứ khi nào quyết định thay đổi — đây là nguồn tham
chiếu chính, không phải chat log.*

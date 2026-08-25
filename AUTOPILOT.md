# Cách Autopilot (AI tự chơi) hoạt động

Autopilot là một chế độ AI thay người chơi tự ngắm, tự di chuyển, tự bắn zombie.
**Mặc định BẬT** khi mở game, tắt/bật được ngay trong menu Settings (trang đầu
tiên, luôn hiển thị kể cả bản release — không phải cheat debug-only).

Code nằm ở `crates/client/src/autopilot.rs`. Wiring vào 3 hệ thống input có sẵn
nằm ở `crates/client/src/lib.rs` (`mouse_look`, `touch_look`, `send_player_input`,
`shoot`). Toggle FFI nằm ở `crates/client/src/cadisplaylink/native_menu.rs` +
`ffi.rs`, UI nằm ở `SettingsMenuView.swift`/`ViewController.swift`.

## Nguyên tắc thiết kế: chỉ giả lập "ý định", không viết lại gameplay

Autopilot **không** viết lại logic movement prediction, đạn dược/reload,
spread/hitscan, hay animation — tất cả những cái đó đã có sẵn và chạy đúng rồi.
Mỗi frame, autopilot chỉ tính ra một `AutopilotIntent` nhỏ gọn (`move_axis`,
`sprint`, `fire`), rồi 3 hệ thống vốn đọc input thật (`mouse_look`/`touch_look`,
`send_player_input`, `shoot`) chỉ thêm một nhánh `if autopilot.0 { dùng intent }
else { đọc input thật }` — giống hệt cách `Noclip` đã làm trong `send_player_input`.

Nhờ vậy autopilot không thể "gian lận": nó bắn qua đúng `shoot()` (vẫn tốn đạn,
vẫn cooldown, vẫn phải reload), di chuyển qua đúng `simulate_player_step` (vẫn
va chạm tường, vẫn bị knockback) — chỉ có nguồn phát input là AI thay vì ngón tay.

## 2 hệ thống chạy mỗi frame (theo đúng thứ tự trong `Update` chain)

### 1. `autopilot_pick_target` — chọn zombie nào để đánh

Copy y hệt pattern `bot_pick_target` của debug bot bên server
(`crates/server/src/main.rs`): chọn zombie **gần nhất trong tầm phát hiện**
(`AUTOPILOT_DETECT_RANGE = 35m`), rồi **giữ mục tiêu đó** cho tới khi:
- zombie đó chết/biến mất, hoặc
- hết giờ `retarget_timer` (`AUTOPILOT_RETARGET_INTERVAL = 1.2s`)

Lý do phải "giữ mục tiêu" thay vì chọn lại mỗi frame: nếu 2 zombie đứng gần
bằng nhau, chọn lại liên tục mỗi frame sẽ làm súng giật qua giật lại liên tục
giữa 2 con (flicker) — không bắn trúng con nào cả.

### 2. `autopilot_think` — biến mục tiêu thành hành động

**Ngắm (aim):** Tính góc yaw/pitch cần quay để nhìn thẳng vào mục tiêu, rồi
xoay `FpsCamera.yaw`/`pitch` **có giới hạn tốc độ** (`AUTOPILOT_TURN_RATE_DEG =
360°/s`) thay vì snap thẳng vào — để trông giống đang "quay ngắm" chứ không
giống aimbot dán mắt tức thời. Cùng lý do zombie/debug bot quay người cũng bị
giới hạn tốc độ (`ZOMBIE_TURN_RATE_DEG`).

**Di chuyển (move):** chia 3 vùng khoảng cách tới mục tiêu:

| Khoảng cách | Hành động |
|---|---|
| xa hơn `AUTOPILOT_ENGAGE_RANGE` (14m) | tiến tới (vì yaw đã ngắm thẳng mục tiêu nên "tiến" = đi thẳng vào nó) |
| trong khoảng 5m–14m ("vùng thoải mái") | đứng tại chỗ, lắc qua lại nhẹ (dodge cho có tí sinh động) |
| gần hơn `AUTOPILOT_RETREAT_RANGE` (5m) | **lùi lại**, vẫn giữ mặt hướng về mục tiêu để vừa lùi vừa bắn |

**Lùi có nhận biết bầy đàn (swarm-aware retreat)** — đây là phần quan trọng
nhất, được thêm vào để sửa lỗi kinh điển của AI đồng đội trong Left 4 Dead/
World War Z: 2 game đó có bot chỉ để ý 1 mục tiêu đang ngắm, bị cả đàn bao vây
từ bên hông vẫn không phản ứng. Autopilot ở đây không chỉ tính khoảng cách tới
*mục tiêu đang ngắm*, mà quét **toàn bộ zombie trong bán kính lùi**, lấy vị trí
trung bình của tất cả rồi lùi ra xa khỏi điểm trung bình đó — tức nếu bị vây từ
nhiều hướng, nó lùi về hướng "thoáng" nhất chứ không lùi thẳng theo hướng của
riêng 1 con.

**Cảnh giác khi máu thấp:** nếu HP ≤ 30% máu tối đa, bán kính lùi tự nới rộng
thêm 4m (`AUTOPILOT_LOW_HEALTH_RETREAT_BONUS`) — máu càng ít càng lùi sớm hơn.

**Sprint:** chỉ chạy nước rút khi đang tiến từ xa (> 22m) và không có zombie
nào gần — không sprint khi đang giao chiến/lùi (sprint thường làm giảm độ
chính xác súng trong game này).

**Quyết định bắn:** chỉ bắn khi **đồng thời** thỏa 2 điều kiện:
1. khoảng cách ≤ tầm giao chiến (14m, ± chút dung sai), **và**
2. góc ngắm đã "đuổi kịp" hướng mục tiêu (< 12°, `AUTOPILOT_FIRE_AIM_TOLERANCE_DEG`)

Điều kiện (2) là để không bắn lung tung trong lúc súng còn đang quay tới —
đợi ngắm chuẩn rồi mới bắn, giống người chơi thật.

**Không thấy zombie nào:** đứng yên, tự xoay chậm (`AUTOPILOT_IDLE_SCAN_DEG_PER_SEC
= 20°/s`) như đang quan sát xung quanh. Không cần logic đi tuần tra phức tạp vì
zombie trong game này đã tự động đuổi theo người chơi trong bán kính
`ZOMBIE_CHASE_RANGE` (60m) — đứng yên "để bị tìm thấy" là đủ cho v1.

## Toàn bộ hằng số có thể chỉnh (đều nằm trong `autopilot.rs`)

| Hằng số | Giá trị | Ý nghĩa |
|---|---|---|
| `AUTOPILOT_DETECT_RANGE` | 35m | tầm phát hiện zombie để chọn mục tiêu |
| `AUTOPILOT_ENGAGE_RANGE` | 14m | khoảng cách "thoải mái" để đứng bắn |
| `AUTOPILOT_RETREAT_RANGE` | 5m | gần hơn mức này thì lùi |
| `AUTOPILOT_RETARGET_INTERVAL` | 1.2s | bao lâu đánh giá lại mục tiêu 1 lần |
| `AUTOPILOT_TURN_RATE_DEG` | 360°/s | tốc độ xoay người/súng khi ngắm |
| `AUTOPILOT_FIRE_AIM_TOLERANCE_DEG` | 12° | sai số ngắm tối đa để được phép bắn |
| `AUTOPILOT_SPRINT_APPROACH_DISTANCE` | 22m | xa hơn mức này (và không có zombie gần) thì chạy nước rút |
| `AUTOPILOT_LOW_HEALTH_FRACTION` | 30% | ngưỡng máu được coi là "thấp" |
| `AUTOPILOT_LOW_HEALTH_RETREAT_BONUS` | +4m | nới thêm bán kính lùi khi máu thấp |
| `AUTOPILOT_IDLE_SCAN_DEG_PER_SEC` | 20°/s | tốc độ xoay quan sát khi chưa thấy zombie |

## Đã test thực tế trên Simulator (log + screenshot)

Bật log debug (`info!` tạm thời trong `autopilot_think`) và quan sát qua
`xcrun simctl spawn <udid> log stream`, xác nhận đúng như thiết kế:
- Từ xa (dist ~20m): tiến thẳng vào mục tiêu, `fire=false` (ngoài tầm).
- Vào vùng thoải mái (~6-9m): đứng lắc qua lại, `aim_err≈0°`, `fire=true` liên tục.
- Zombie khác áp sát dưới 5m (`nearby=1`): lùi ra xa, `fire` tạm tắt trong lúc
  súng đang quay theo hướng lùi mới (aim_err tăng vọt rồi giảm dần về 0).

Ảnh chụp màn hình xác nhận máu giữ ổn định, có hiệu ứng máu bắn ra (hit) khi
autopilot tự bắn trúng zombie.

## Giới hạn hiện tại (v1) / hướng cải thiện

- Không tự đi tìm zombie ở xa (chỉ đứng quan sát khi không thấy con nào) — ổn
  vì zombie tự đuổi theo người chơi, nhưng có thể thêm patrol nếu thấy cần.
- Chọn mục tiêu thuần theo khoảng cách gần nhất, không tính "mối đe dọa" như
  zombie đặc biệt/threat score (game hiện chưa có loại zombie đặc biệt nào
  khác biệt nguy hiểm hơn để cần ưu tiên).
- Lùi khỏi bầy đàn là "lùi thẳng khỏi điểm trung bình", chưa tính đường thoát
  bị chặn bởi tường/hành lang hẹp — nếu bị dồn vào góc cụt vẫn có thể chết.

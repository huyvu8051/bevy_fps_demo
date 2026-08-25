# Bevy FPS Demo

FPS bắn zombie nhiều người chơi (multiplayer), viết bằng Bevy 0.18 (Rust). Server-authoritative, dùng `bevy_replicon` + `renet2` để đồng bộ mạng qua UDP.

## Cấu trúc workspace

```
crates/
  client/   # binary client (desktop + iOS), rendering + input + UI
  server/   # binary server (headless), spawn zombie/player, AI director, replication
  shared/   # types/event/const dùng chung giữa client & server (network protocol)
```

## Chạy trên desktop

Cần cài Rust (`rustup`).

**Chạy server:**
```bash
cargo run -p server
```
Mặc định lắng nghe UDP port `5031`.

**Chạy client** (mở terminal khác):
```bash
cargo run -p client
```
Mặc định client kết nối tới `SERVER_ADDR` (env var), fallback hiện tại đang **tạm hardcode** là IP Tailscale của máy dev (`100.97.13.68:5031`) để test qua điện thoại — xem mục "Trạng thái tạm/test" bên dưới. Muốn nối vào server chạy local bình thường:
```bash
SERVER_ADDR=127.0.0.1:5031 cargo run -p client
```

Có thể chạy nhiều client cùng lúc (nhiều terminal) để test multiplayer trên 1 máy.

## Điều khiển (desktop)

- **WASD**: di chuyển, **Shift**: chạy (sprint), **Space**: nhảy
- **Chuột**: nhìn/xoay camera (khoá con trỏ khi vào game, **Esc**: mở menu Settings/nhả chuột)
- **Chuột trái**: bắn, **Chuột phải (giữ)**: ngắm (ADS)
- **R**: nạp đạn / restart khi game over
- Còn nhiều phím debug khác (noclip, god mode, spawn bot, chỉnh FOV bằng `[`/`]`, đổi số lượng zombie, v.v.) — nguồn chính xác nhất là menu **Settings/Cheats** trong game (bấm Esc) hoặc grep `KeyCode::` trong `crates/client/src/lib.rs`.

## Điều khiển (mobile/touch)

Khi build cho iOS (hoặc set `TOUCH_UI_PREVIEW=1` trên desktop để xem trước UI), game tự chuyển sang UI cảm ứng:
- **Joystick ảo** (góc dưới trái): di chuyển
- **Kéo nửa phải màn hình**: xoay camera
- **Nút đỏ** (góc dưới phải): bắn, **nút ADS**: ngắm
- **Menu Settings/Cheats**: nút MENU riêng để mở — UI **native Swift** (`SettingsMenuView.swift`), chia 4 trang bấm `<`/`>` để chuyển (Settings / Cheats / Zombies / Visual), không phải bevy_ui nữa (xem lý do ở mục render bên dưới). Trang Cheats/Zombies/Visual chỉ có ở debug build.
- Chết thì hiện màn **GAME OVER** + nút Restart, cũng là UI native Swift riêng (`GameOverView.swift`), không phải bevy_ui.

## Build & chạy trên iOS

Project chính thức nằm ở `crates/client/gen/apple` — Xcode project viết tay (Swift `ViewController`+`MetalView`+`CADisplayLink`, **không dùng winit trên iOS nữa**), link thẳng vào `libclient.a` build từ `crates/client`. Bản winit cũ (dùng `cargo-mobile2`/`xcodegen`) vẫn còn giữ lại ở `crates/client/gen/apple_winit_legacy` để tham khảo/rollback nếu cần, không còn build/dùng mặc định.

Lý do đổi: winit trên iOS có bug đã biết (không đạt đúng tốc độ khung hình cấu hình — đo thực tế chỉ ~50fps dù set target 120fps) do cách nó điều khiển event loop trên iOS; CADisplayLink (API native của Apple cho vsync-driven rendering) điều khiển vòng lặp render trực tiếp, đạt đúng 120fps mượt trên ProMotion.

**Quan trọng hơn**: iOS **không dùng `bevy_render`/wgpu nữa** — dù CADisplayLink giải quyết được vấn đề framerate, đo thực tế cho thấy bản thân wgpu tốn CPU encode command buffer rất cao trên iOS (88-100% "encoding cost" ngay cả với scene rỗng, theo Metal HUD). Đã thay bằng renderer Metal viết tay 100% bằng Rust (crate `metal`, không qua `bevy_render`) — vẽ thẳng state ECS thật của game (camera, mesh, texture, animation zombie/soldier, scope camera...) được "extract" mỗi frame từ một `bevy::app::App` headless (không window, không renderer) chạy y hệt logic game desktop (`build_game_common`). Chi tiết implementation:
- `crates/client/src/cadisplaylink/native_metal_probe.rs` — renderer chính (pipeline Metal, shader MSL viết tay, 2 camera FPS+scope).
- `crates/client/src/cadisplaylink/native_game_app.rs` — App headless chạy logic game thật, không GPU/window.
- `crates/client/src/cadisplaylink/native_game_extract.rs` — "extract" mỗi frame: đọc camera/zombie/player/mesh/texture từ World thật, không đụng bevy_render.
- `crates/client/src/cadisplaylink/skinned_gltf.rs`, `gltf_scene.rs` — parse glTF/skin/animation trực tiếp bằng crate `gltf`, không qua `bevy_gltf`.
- `crates/client/src/cadisplaylink/native_menu.rs` — cầu FFI đọc/ghi resource thật (SettingsMenuVisible, GodMode, GameOver...) cho 2 UI native Swift ở trên.

Vì bevy_ui **không được vẽ** trong renderer Metal riêng này (chỉ vẽ `Mesh3d`/`MeshMaterial3d`), toàn bộ UI trên iOS (settings/cheats menu, màn game-over) đã được viết lại thành view Swift/UIKit native thay vì dùng bevy_ui của game gốc — xem `ViewController.swift`/`SettingsMenuView.swift`/`GameOverView.swift`. Desktop vẫn dùng bevy_ui + `bevy_render`/wgpu bình thường, không đổi gì.

Cờ bật/tắt renderer mới nằm ở đầu `ViewController.swift`: `USE_RUST_METAL_DEMO`/`USE_REAL_GAME_NATIVE_RENDERER` (mặc định cả hai `true` — renderer Metal viết tay, vẽ game thật).

**Build + cài lên thiết bị thật** — 1 lệnh duy nhất, tự lo hết (cargo build → copy `.a` vào Xcode project → `xcodebuild` cho đúng thiết bị đang cắm → `devicectl install` + `launch`):
```bash
scripts/deploy_ios_device.sh
```
Nhiều thiết bị cắm cùng lúc thì lọc theo tên:
```bash
scripts/deploy_ios_device.sh Huy
```
Máy đang khoá thì install vẫn thành công nhưng launch sẽ báo lỗi (`Locked`) — mở khoá máy rồi tự bấm icon hoặc chạy lại lệnh trên.

**Build + cài lên Simulator** — script song song, dùng `simctl` thay `devicectl`, tự boot Simulator nếu chưa chạy:
```bash
scripts/deploy_ios_simulator.sh
```

<details>
<summary>Các bước thủ công bên trong script (tham khảo khi cần debug riêng từng bước, thay UUID bằng thiết bị của bạn — xem qua <code>xcrun devicectl list devices</code>)</summary>

```bash
cargo build -p client --target aarch64-apple-ios   # từ workspace root
cp target/aarch64-apple-ios/debug/libclient.a crates/client/gen/apple/iOS/libs/debug/libclient.a
cp target/aarch64-apple-ios/debug/libclient.a crates/client/gen/apple/iOS/libs/debug/libbevy_in_app.a

cd crates/client/gen/apple/iOS
xcodebuild -project bevy_in_iOS.xcodeproj -scheme bevy_in_iOS -sdk iphoneos -configuration Debug \
  -allowProvisioningUpdates -destination 'id=<DEVICE_UUID>' build

xcrun devicectl device install app --device <DEVICE_UUID> \
  "$(xcodebuild -project bevy_in_iOS.xcodeproj -scheme bevy_in_iOS -sdk iphoneos -configuration Debug -showBuildSettings 2>/dev/null | awk -F'= ' '/ CONFIGURATION_BUILD_DIR/{d=$2} / FULL_PRODUCT_NAME /{n=$2} END{print d"/"n}')"

xcrun devicectl device process launch --device <DEVICE_UUID> com.huyvu8051.bevyfpsdemo
```
</details>

Mở app bằng cách bấm icon trên máy như bình thường — mặc định chạy game đầy đủ.

### Build nhanh hơn khi lặp lại (sửa code → deploy lại nhiều lần)

Đo thật bằng `xcodebuild -showBuildTimingSummary` để tìm đúng chỗ chậm thay vì đoán — kết quả bất ngờ: **không phải** lúc compile Rust, **không phải** lúc copy file `.a` (2.5GB nhưng APFS copy-on-write nên gần như tức thời), mà là bước **CodeSign** của Xcode, chiếm gần hết thời gian build (19s / ~22s tổng lúc mọi thứ khác đã cache ấm). CodeSign chậm vì nó phải hash toàn bộ nội dung binary — mà binary debug của Rust ở đây link tĩnh `bevy`+`avian3d`+mọi dependency kèm đầy đủ debug info, ra tới **191MB**.

Cách sửa (đã áp dụng sẵn trong `scripts/deploy_ios_*.sh`, không cần làm gì thêm):
1. **Profile `dev-fast`** (`Cargo.toml`, root): `debug = "line-tables-only"` (giữ số dòng cho backtrace, bỏ debug info chi tiết biến/kiểu) + `strip = "debuginfo"`. Cắt binary từ 2.58GB xuống **1.24GB** (~52%) → CodeSign còn **~9.5s** (đo thật, giảm gần một nửa đúng theo tỉ lệ size). Cần debug LLDB thật (breakpoint, xem biến) thì build tay bằng profile `dev` gốc (`cargo build --profile dev`) — không đổi gì, vẫn còn nguyên.
2. **Tái sử dụng derived data** giữa các lần chạy thay vì `rm -rf` mỗi lần (2 script cũ trước đây xoá sạch, ép Xcode build lại từ đầu) — để Xcode tự incremental, đo được tiết kiệm thêm ~13s. Nghi ngờ derived data hỏng/kỳ lạ thì thêm `--clean` làm tham số đầu tiên: `scripts/deploy_ios_simulator.sh --clean`.

Kết quả: **cả chu trình sửa code → build → deploy → launch còn ~20 giây** (đo thật, cache ấm), so với trước đây mỗi lần đều full build lại (>1 phút).

**Về hot reload**: đã tìm hiểu (`dexterous_developer`/`hot-lib-reloader`) — **không khả thi trên thiết bị thật**, đây là giới hạn cứng của iOS từ iOS 10 trở đi (sandbox chặn `dlopen` bất kỳ code nào không nằm sẵn trong app bundle lúc cài, kể cả build dev có entitlement `get-task-allow`). Trên Simulator thì có khả năng khả thi hơn (Simulator chạy như 1 process macOS bình thường, `dlopen` ít bị chặn hơn) nhưng cần đổi `crate-type` sang thêm `cdylib` riêng cho path Simulator — dự án này cố tình chỉ dùng `staticlib` vì từng dính crash `dyld` thật trên thiết bị khi build kèm `cdylib` (xem comment trong `crates/client/Cargo.toml`). Chưa làm — nếu 20s/lần vẫn chưa đủ nhanh, đây là hướng tiếp theo đáng thử nhưng tốn công hơn hẳn 2 thay đổi ở trên.

### Chế độ "minimal map probe" (debug UI/perf)

Build riêng chỉ có camera + map Sponza + touch controls, không có network/zombie/UI đầy đủ — dùng để cô lập bug input/hiệu năng khỏi độ phức tạp của game thật. **Chỉ bật được qua `devicectl` launch** (icon-tap trên Home Screen không đọc được env var này):

```bash
xcrun devicectl device process launch --device <DEVICE_UUID> \
  -e '{"MINIMAL_MAP_PROBE":"1"}' com.huyvu8051.bevyfpsdemo
```

Bấm lại icon trên máy như bình thường sẽ quay về game đầy đủ.

Trên desktop cũng test được probe này (vẫn dùng đường winit bình thường trên desktop, chỉ iOS đổi sang CADisplayLink):
```bash
MINIMAL_MAP_PROBE=1 TOUCH_UI_PREVIEW=1 cargo run -p client
```

## Đo CPU/GPU trên iOS

Bevy's `SystemInformationDiagnosticsPlugin` **không hỗ trợ iOS** (dùng crate `sysinfo`, bị tắt hẳn trên target này ở source code) — không có plugin Bevy nào báo % CPU/GPU sẵn trên iOS.

Cách thực tế để xem CPU/GPU/FPS trực tiếp trên máy, không cần Instruments:

```bash
xcrun devicectl device process launch --device <DEVICE_UUID> \
  -e '{"MTL_HUD_ENABLED":"1"}' com.huyvu8051.bevyfpsdemo
```

Bật **Metal HUD** (tính năng có sẵn của Apple) — overlay đè lên góc màn hình, hiển thị FPS, frame time, GPU time trực tiếp trên thiết bị. Đây cũng chính là cách Bevy khuyến nghị thay vì dùng `FpsOverlayPlugin` để đo chính xác/ít overhead hơn.

Muốn đo sâu hơn (breakdown theo shader pass, command buffer, memory bandwidth...) thì dùng Instruments với template **"Metal System Trace"**:
```bash
xcrun xctrace record --template "Metal System Trace" --device <DEVICE_UUID_XCTRACE> \
  --time-limit 15s --output trace.trace --attach <PID>
open trace.trace   # mở bằng Instruments GUI để xem track GPU
```
Lưu ý: `<DEVICE_UUID_XCTRACE>` khác format với UUID của `devicectl` — lấy qua `xcrun xctrace list devices`.

Riêng số liệu CPU% tự viết tay (không có plugin Bevy hỗ trợ) — xem ví dụ ở `experiments/cadisplaylink_probe/src/simple_scene.rs` (`process_cpu_seconds()`, dùng `libc::getrusage`).

### Game Mode (iOS 18+)

Đã bật trong `crates/client/gen/apple/iOS/bevy_in_iOS/Info.plist`:
```xml
<key>LSApplicationCategoryType</key>
<string>public.app-category.games</string>
<key>GCSupportsGameMode</key>
<true/>
<key>LSSupportsGameMode</key>
<true/>
```
Chỉ khiến app **đủ điều kiện** — hệ điều hành mới quyết định có thực sự kích hoạt Game Mode hay không (tuỳ trạng thái máy lúc đó: pin, nhiệt độ...). Khi kích hoạt, iOS giảm hoạt động nền + ưu tiên CPU/GPU cho app, giảm độ trễ Bluetooth — kiểm tra bằng icon Game Mode ở Control Center.

## Trạng thái tạm/test (cần dọn lại sau khi test xong)

- `SERVER_ADDR` fallback trong `crates/client/src/lib.rs` đang hardcode IP Tailscale (`100.97.13.68:5031`) thay vì `127.0.0.1:5031`, để icon-tap launch trên điện thoại tự nối vào server chạy local qua Tailscale (icon-tap không đọc được scheme env var của Xcode).
- `run_ai_director` đang bị comment out khỏi `Update` schedule của server (tạm tắt vì chồng chéo với tính năng chỉnh số lượng zombie trong Cheats panel) — để lại 2 warning dead-code khi build server, là chủ ý, chưa fix.
- `ZombieTargetCount` mặc định là `10` (trước đây game spawn tối đa `ZOMBIE_SPAWNS.len()` ~100) — giờ là tham số chỉnh được qua Cheats panel (`+10`/`-10`/`Auto`), không còn hardcode.

## Ghi chú khác

- Xem `RECOIL.md` để biết chi tiết hệ thống recoil súng.
- iOS: cần bật `CADisableMinimumFrameDurationOnPhone` trong Info.plist mới lên được ProMotion 120Hz trên iPhone (đã bật sẵn trong `crates/client/gen/apple/iOS/bevy_in_iOS/Info.plist`).
- Chưa hỗ trợ Android (chưa có scaffold `gen/android`).
- Chuột/bàn phím Bluetooth **có** điều khiển được game trên iOS (đã ghép nối qua Settings > Bluetooth của máy trước) — dùng Apple's GameController framework (`GCKeyboard`/`GCMouse`), bridge qua `crates/client/gen/apple/iOS/base/GameControllerInput.swift` → `cadisplaylink_key_changed`/`cadisplaylink_mouse_moved`/`cadisplaylink_mouse_button_changed` (`crates/client/src/cadisplaylink/ffi.rs`). Cắm chuột vào là look tự khoá luôn (không có con trỏ để bấm-khoá như desktop). Yêu cầu iOS 14+ (gated bằng `#available`, không ảnh hưởng máy chạy iOS 13).

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
- **Menu Cheats**: có nút riêng để mở, cuộn được nếu dài, gộp toàn bộ toggle debug của bản desktop (noclip/god/freeze/hitbox/spawn zombie/...)

## Build & chạy trên iOS

Scaffold nằm ở `crates/client/gen/apple` (dùng `cargo-mobile2` + `xcodegen`).

Sau khi sửa `project.yml`, luôn phải chạy lại:
```bash
cd crates/client/gen/apple
xcodegen generate
```

**Build + cài lên thiết bị thật** (thay UUID bằng thiết bị của bạn, xem qua `xcrun devicectl list devices`):
```bash
cd crates/client/gen/apple
xcodebuild -project client.xcodeproj -scheme client_iOS -sdk iphoneos -configuration debug \
  -allowProvisioningUpdates -destination 'id=<DEVICE_UUID>' build

xcrun devicectl device install app --device <DEVICE_UUID> \
  "$(xcodebuild -project client.xcodeproj -scheme client_iOS -sdk iphoneos -configuration debug -showBuildSettings 2>/dev/null | awk -F'= ' '/ CONFIGURATION_BUILD_DIR/{d=$2} / FULL_PRODUCT_NAME /{n=$2} END{print d"/"n}')"
```

Mở app bằng cách bấm icon trên máy như bình thường.

### Chế độ "minimal map probe" (debug UI/perf)

Build riêng chỉ có camera + map Sponza + touch controls, không có network/zombie/UI đầy đủ — dùng để cô lập bug input/hiệu năng khỏi độ phức tạp của game thật. **Chỉ bật được qua `devicectl` launch** (icon-tap trên Home Screen không đọc được env var này):

```bash
xcrun devicectl device process launch --device <DEVICE_UUID> \
  -e '{"MINIMAL_MAP_PROBE":"1"}' com.huyvu8051.bevyfpsdemo
```

Bấm lại icon trên máy như bình thường sẽ quay về game đầy đủ.

Trên desktop cũng test được probe này:
```bash
MINIMAL_MAP_PROBE=1 TOUCH_UI_PREVIEW=1 cargo run -p client
```

## Trạng thái tạm/test (cần dọn lại sau khi test xong)

- `SERVER_ADDR` fallback trong `crates/client/src/lib.rs` đang hardcode IP Tailscale (`100.97.13.68:5031`) thay vì `127.0.0.1:5031`, để icon-tap launch trên điện thoại tự nối vào server chạy local qua Tailscale (icon-tap không đọc được scheme env var của Xcode).
- `run_ai_director` đang bị comment out khỏi `Update` schedule của server (tạm tắt vì chồng chéo với tính năng chỉnh số lượng zombie trong Cheats panel) — để lại 2 warning dead-code khi build server, là chủ ý, chưa fix.
- `ZombieTargetCount` mặc định là `10` (trước đây game spawn tối đa `ZOMBIE_SPAWNS.len()` ~100) — giờ là tham số chỉnh được qua Cheats panel (`+10`/`-10`/`Auto`), không còn hardcode.

## Ghi chú khác

- Xem `RECOIL.md` để biết chi tiết hệ thống recoil súng.
- iOS: cần bật `CADisableMinimumFrameDurationOnPhone` trong Info.plist mới lên được ProMotion 120Hz trên iPhone (đã bật sẵn trong `project.yml`).
- Chưa hỗ trợ Android (chưa có scaffold `gen/android`).
- Chuột/bàn phím Bluetooth **không** điều khiển được game trên iOS — `winit` (thư viện windowing Bevy dùng) hiện chưa hỗ trợ `GCMouse`/`GCKeyboard` của Apple.

# Cách hệ thống recoil hoạt động

Recoil gồm 3 phần độc lập, tách riêng để không cái nào ghi đè cái nào:

1. **Gun kickback** — súng (mesh) tự lùi ra sau rồi trả lại.
2. **View recoil** — camera bị "giật" lên/lệch qua, tự hồi về theo thời gian.
3. **Bullet spread** — tia raycast lệch khỏi tâm ngắm, càng bắn lâu càng lệch nhiều.

## 1. Vì sao tách `FpsCamera` (góc ngắm) khỏi `ViewRecoil` (độ giật)?

`FpsCamera { yaw, pitch }` là góc ngắm "thật" do chuột điều khiển — đây là nguồn
sự thật duy nhất cho hướng người chơi đang muốn nhìn.

`ViewRecoil { pitch_kick, yaw_kick }` là độ lệch **tạm thời**, cộng thêm vào
góc ngắm khi hiển thị, không sửa `FpsCamera` trực tiếp. Nhờ vậy:

- Recoil không làm "trôi" điểm ngắm gốc của người chơi — nhả chuột ra là camera
  tự về đúng chỗ đang nhắm, không cần người chơi tự kéo chuột bù lại.
- Di chuyển chuột trong lúc recoil đang hồi vẫn hoạt động bình thường — hai giá
  trị cộng dồn (`pitch_kick + cam.pitch`) mỗi frame, không xung đột.

## 2. Luồng dữ liệu mỗi frame (thứ tự system, dùng `.chain()`)

```
grab_cursor        khóa/nhả con trỏ chuột
   ↓
mouse_look         chuột di → cộng dồn vào FpsCamera.yaw/pitch (KHÔNG đụng Transform)
   ↓
shoot              giữ chuột trái + hết cooldown → bắn 1 phát:
                     - gun.recoil = RECOIL_DURATION       (kích hoạt kickback súng)
                     - recoil.pitch_kick += RECOIL_PITCH_KICK, clamp trần   (kích hoạt giật camera)
                     - recoil.yaw_kick += jitter ngẫu nhiên nhỏ
                     - raycast lệch theo "độ nóng" hiện tại (heat = pitch_kick / max)
   ↓
apply_view_recoil  MỖI frame (kể cả không bắn):
                     - pitch_kick, yaw_kick *= (1 - decay)   → hồi dần về 0
                     - Transform.rotation = f(cam.yaw + yaw_kick, cam.pitch + pitch_kick)
   ↓
move_player        di chuyển theo Transform vừa cập nhật (WASD dùng forward/right mới nhất)
   ↓
gun_recoil         mỗi frame: nếu gun.recoil > 0 → lùi mesh súng theo trục Z cục bộ, đếm ngược
```

`.chain()` bắt buộc thứ tự này — nếu không, Bevy có thể chạy các system song
song và `apply_view_recoil` có thể đọc `ViewRecoil` *trước khi* `shoot` kịp ghi
giá trị mới của frame đó (bắn bị "chậm" 1 frame, không rõ ràng khi debug).

## 3. Vì sao "hồi dần" (spring-like decay) thay vì lerp cố định?

```rust
let decay = (RECOIL_RECOVERY_SPEED * time.delta_secs()).min(1.0);
recoil.pitch_kick *= 1.0 - decay;
```

Đây là suy giảm theo cấp số nhân (exponential decay), không phải trừ tuyến
tính. Kết quả: lúc mới bắn giật nhanh, càng về cuối càng chậm lại — giống lò
xo thật, không bị "cắt cụt" đột ngột như trừ thẳng một lượng cố định mỗi
frame. `RECOIL_RECOVERY_SPEED` càng lớn thì hồi càng nhanh.

## 4. "Random" không dùng crate `rand`

```rust
let jitter = ops::sin(time.elapsed_secs() * 977.0);
```

Đây không phải số ngẫu nhiên thật — là hàm sin với tần số vô tri (977, 613,
811...) áp lên thời gian đã trôi qua, cho ra giá trị "trông có vẻ ngẫu nhiên"
đổi liên tục theo từng phát bắn. Đủ dùng cho hiệu ứng thị giác, không cần
thêm dependency `rand` chỉ để lắc súng qua lại.

## 5. Bullet spread ăn theo "độ nóng" của recoil, không phải bộ đếm riêng

```rust
let heat = recoil.pitch_kick / RECOIL_PITCH_KICK_MAX; // 0.0 → 1.0
let spread = BULLET_SPREAD * (0.3 + heat);
```

`heat` tái dùng chính giá trị `pitch_kick` đang có (không cần thêm biến đếm
"số phát đã bắn liên tục" riêng) — vì `pitch_kick` vốn đã tăng khi bắn liên
tục và giảm khi ngừng, đúng nghĩa "đang bắn nóng máy tới đâu" cần cho spread.

## Các hằng số chỉnh nhanh (đầu file `src/main.rs`)

| Hằng số                    | Ảnh hưởng                                          |
|-----------------------------|-----------------------------------------------------|
| `FIRE_RATE`                  | Nhịp bắn khi giữ chuột (giây/phát)                  |
| `RECOIL_PITCH_KICK`          | Camera giật lên bao nhiêu mỗi phát                  |
| `RECOIL_PITCH_KICK_MAX`      | Trần giật — chặn không cho văng lên trời khi bắn lâu|
| `RECOIL_YAW_JITTER`          | Độ lắc ngang mỗi phát                               |
| `RECOIL_RECOVERY_SPEED`      | Tốc độ hồi về 0 (càng lớn hồi càng nhanh)            |
| `BULLET_SPREAD`              | Độ tán xạ đạn tối đa khi "nóng máy" hết cỡ           |
| `RECOIL_KICK` / `RECOIL_DURATION` | Biên độ/thời lượng lùi của **mesh súng** (khác với recoil camera ở trên) |

# Weapon Arsenal — nhiều hơn L4D, vẫn tối ưu animation

Bổ sung cho GDD.md mục 5 (Content Scope, "~12 khẩu, đổi vũ khí giữa
trận") và mục 15 ("~12 khẩu, đủ nhóm vai trò như L4D"). File này nâng
target lên **36 vũ khí** (L4D1+L4D2 gộp lại có ~16 súng + ~10 cận chiến +
4 throwable ≈ 30 item — 36 đã nhỉnh hơn), theo đúng bài học đã rút ra ở
`ZOMBIE_BESTIARY.md`: **phân loại theo cách cầm/bắn/nạp đạn** ("handling
class") để animation dùng chung, còn số lượng SÚNG (model + stat) thì tăng
thoải mái vì rẻ hơn animation nhiều.

## Vì sao tách 2 lớp (giống hệt archetype/animation ở zombie)

- **Handling class** — quyết định **animation player cần** (Hold-idle/
  Fire/Reload/ADS pose, xem `PLAYER_ANIMATION_PLAN.md` mục "Reference —
  what PUBG/L4D ship"). 2 khẩu súng khác model/stat nhưng CÙNG class thì
  dùng chung y hệt bộ clip đó — đúng cách PUBG/L4D thật sự làm ("per-
  weapon Reload" nghĩa là per-CLASS, không phải per khẩu súng thật sự
  riêng biệt trong game gốc).
- **Weapon entry** — 1 khẩu súng cụ thể: model, `MAGAZINE_SIZE`,
  `BULLET_SPREAD`, damage, recoil... (đúng các field đã có trong
  `crates/client/src/lib.rs`, chỉ cần bảng giá trị riêng/khẩu thay vì 1
  bộ hằng số cố định như hiện tại — AK47 đang là khẩu duy nhất).

## 13 handling class (quyết định animation)

| Class | Player cần (Fire/Reload/ADS) | Đặc điểm nạp đạn | Vũ khí dùng class này |
|---|---|---|---|
| **Súng ngắn** | 1 bộ | Rút băng cũ, gạt băng mới vào tay cầm | 3 |
| **SMG** | 1 bộ | Nạp băng thẳng vào ổ dưới | 3 |
| **Shotgun** | 1 bộ | Nhét từng viên qua khe bên hông (chậm, ngắt được giữa chừng) | 3 |
| **Súng trường tấn công (AR)** | 1 bộ | Nạp băng cong tiêu chuẩn (AK47 hiện có thuộc class này) | 4 |
| **Súng trường chiến đấu/DMR** | 1 bộ | Giống AR nhưng chậm hơn, giật mạnh hơn | 2 |
| **Súng bắn tỉa (bolt-action)** | 1 bộ | Kéo/đẩy khoá nòng giữa mỗi phát | 3 |
| **Súng máy hạng nhẹ (LMG)** | 1 bộ | Nạp dây đạn qua nắp trên, chậm nhất trong mọi class | 2 |
| **Nỏ/cung** | 1 bộ | Kéo dây bằng tay, không cần "băng đạn" | 1 |
| **Súng phóng lựu/pháo sáng** | 1 bộ | Bẻ nòng, nhét 1 viên | 2 |
| **Súng phun lửa** | 1 bộ | Đổi bình nhiên liệu sau lưng, không phải "nạp đạn" | 1 |
| **Cận chiến nhẹ** | 1 bộ (vung nhanh, không reload) | — | 3 |
| **Cận chiến nặng** | 1 bộ (vung chậm, không reload) | — | 3 |
| **Cưa máy** | 1 bộ (liên tục, "đạn" = nhiên liệu) | Đổ xăng thay vì nạp đạn | 1 |

**Throwable** (4 khẩu) và **Thực nghiệm** (1 khẩu, xem mục riêng) đứng
ngoài 13 class trên — throwable dùng 1 động tác ném chung (đã có trong
`PLAYER_ANIMATION_PLAN.md`'s reference "throw"), Thực nghiệm tự thiết kế
riêng (chỉ 1 khẩu, xem cuối file).

**Tổng animation player cần: 13 class × 3 clip (Fire/Reload/ADS) + 1
throw + 3 clip riêng cho Thực nghiệm = 43 clip** — cho **36 khẩu súng +
4 throwable + 1 thực nghiệm = 41 vũ khí**, tỉ lệ tái dùng ~1:1 giữa số
class và số clip nhưng **~2.8 khẩu súng/class trung bình** — đúng đòn bẩy
tối ưu giống hệt archetype zombie (nhiều "loại nhìn thấy", ít "animation
phải gen").

---

## Roster đầy đủ 36 khẩu súng + 4 throwable

### Súng ngắn (3)
| Tên | Đặc điểm |
|---|---|
| M9 Sidearm | Cân bằng, súng khởi đầu mặc định |
| Magnum Hạng Nặng | Damage cao nhất trong súng ngắn, băng nhỏ, giật mạnh |
| Súng Ngắn Tự Động | Bắn nhanh nhất, damage/viên thấp nhất |

### SMG (3)
| Tên | Đặc điểm |
|---|---|
| SMG Tiêu Chuẩn | Tốc bắn cao, tầm gần |
| SMG Giảm Thanh | Không gọi thêm zombie khi bắn (tái dùng đúng cơ chế noise/`PLAYER_THREAT_PER_SHOT` — class DUY NHẤT giảm hệ số này) |
| SMG Băng Đôi | Băng lớn hơn, đổi lại giật mạnh hơn hẳn |

### Shotgun (3)
| Tên | Đặc điểm |
|---|---|
| Shotgun Bơm | Damage cao tầm cực gần, chậm |
| Shotgun Tự Động | Bắn nhanh hơn, damage/phát thấp hơn bản bơm |
| Súng Săn Nòng Đôi | 2 phát rồi phải nạp lại từ đầu (nạp chậm nhất Shotgun class) — đổi lại 2 phát đầu gây damage cao nhất toàn game |

### Súng trường tấn công (4)
| Tên | Đặc điểm |
|---|---|
| AK47 (đã có trong code) | Baseline hiện tại |
| M4 Chiến Thuật | Giật nhẹ hơn AK47, damage/viên thấp hơn 1 chút |
| SCAR-L | Đã nhắc trong `PLAYER_ANIMATION_PLAN.md` là "đã trong kế hoạch" — băng lớn, cân bằng |
| AR Bullpup | Ngắn gọn, xoay người nhanh hơn khi ngắm (giảm nhẹ thời gian vào ADS) |

### Súng trường chiến đấu/DMR (2)
| Tên | Đặc điểm |
|---|---|
| DMR Bán Tự Động | Damage/viên cao hơn AR, tốc bắn chậm hơn, tầm xa hơn |
| Battle Rifle Cỡ Nòng Lớn | Damage cao nhất nhóm bán tự động, giật mạnh nhất, băng nhỏ |

### Súng bắn tỉa (3)
| Tên | Đặc điểm |
|---|---|
| Bolt-Action Tiêu Chuẩn | 1 phát 1 lần kéo khoá, damage rất cao |
| Súng Bắn Tỉa Chống Vật Chất | Damage cao nhất toàn game, xuyên được nhiều mục tiêu thẳng hàng |
| Bắn Tỉa Giảm Thanh | Damage thấp hơn 2 khẩu trên, đổi lại không gọi zombie khi bắn (giống SMG Giảm Thanh) |

### Súng máy hạng nhẹ (2)
| Tên | Đặc điểm |
|---|---|
| LMG Dây Đạn | Băng cực lớn, nạp rất chậm, di chuyển chậm khi ngắm |
| LMG Hạng Nhẹ | Băng nhỏ hơn LMG Dây Đạn nhưng di chuyển/ngắm nhanh hơn — "LMG cho người không muốn đứng yên" |

### Nỏ/cung (1)
| Tên | Đặc điểm |
|---|---|
| Nỏ Chiến Thuật | Không tiếng nổ — hoàn toàn không kích hoạt cơ chế noise, đạn (mũi tên) nhặt lại được từ xác sau khi bắn trúng |

### Súng phóng lựu/pháo sáng (2)
| Tên | Đặc điểm |
|---|---|
| Súng Phóng Lựu | AoE diện rộng, băng chỉ 1 viên, nạp chậm — vũ khí "dọn horde" |
| Súng Pháo Sáng | Không sát thương trực tiếp — bắn trúng zombie **Nghẹt Thở (Boomer)** làm nó nổ ngay lập tức từ xa (utility, không phải damage) |

### Súng phun lửa (1)
| Tên | Đặc điểm |
|---|---|
| Súng Phun Lửa Công Nghiệp | DoT diện hẹp liên tục, "đạn" là bình nhiên liệu — hợp Campaign 3 (Vùng công nghiệp), tìm được ở M14 Refinery Loop |

### Cận chiến nhẹ (3)
| Tên | Đặc điểm |
|---|---|
| Dao Chiến Đấu | Nhanh nhất, damage thấp nhất |
| Machete | Cân bằng |
| Gậy Bóng Chày | Có cơ hội hất ngã (stagger) zombie thường khi trúng |

### Cận chiến nặng (3)
| Tên | Đặc điểm |
|---|---|
| Rìu Cứu Hoả | Damage cao, tìm được ở M11 Ironloom Textile Mill (thiết bị cứu hoả nhà máy) |
| Búa Tạ | Chậm nhất, damage cao nhất trong cận chiến (trừ Cưa Máy), gây knockback mạnh |
| Xà Beng | Cân bằng — cũng là công cụ cạy cửa môi trường (nếu sau này có puzzle cạy cửa) |

### Cưa máy (1)
| Tên | Đặc điểm |
|---|---|
| Cưa Máy Công Nghiệp | Sát thương liên tục khi giữ nút, "đạn" là xăng (đồng hồ đếm ngược riêng, không dùng `MAGAZINE_SIZE`) — chậm di chuyển khi đang cưa, tìm được ở khu công nghiệp |

### Throwable (4)
| Tên | Đặc điểm |
|---|---|
| Lựu Đạn Nổ Mảnh | Sát thương AoE trực tiếp |
| Molotov | Đốt diện rộng theo thời gian, chặn đường zombie hiệu quả |
| Bom Ống (Pipe Bomb) | Không sát thương ngay — phát tiếng ồn giả, dụ TOÀN BỘ zombie gần đó dồn tới điểm nổ trước khi nổ thật (tái dùng ý tưởng L4D, đã có trong prose zombie design) |
| Lựu Đạn Khói | Không sát thương — che tầm nhìn zombie thường (chúng "mất dấu" tạm thời), khác Molotov ở chỗ không gây damage |

---

## Vũ khí Thực nghiệm — gắn thẳng vào cốt truyện (1 khẩu, tự thiết kế riêng)

### ⚡ Súng Xung Điện Blackridge (Blackridge Prototype Railgun)

- **Bối cảnh:** chỉ tìm được ở **M18 Subterranean Lab Alpha** (Campaign
  4, đúng nơi "rò rỉ dịch bệnh thật sự bắt đầu" — GDD mục 14.1) hoặc
  **M19 Command Bunker** — vũ khí thử nghiệm của chính BRDF, chưa từng
  đưa vào sản xuất hàng loạt.
- **Cơ chế:** không dùng đạn thường — **sạc điện qua thời gian** (giữ
  nút để sạc, càng sạc lâu damage/AoE càng lớn, tối đa ~2s), bắn ra 1 tia
  xuyên thẳng gây sát thương cực cao xuyên nhiều mục tiêu thẳng hàng +
  gây choáng ngắn cho mọi zombie trúng tia (kể cả Lôi Đình/Tank, dù không
  đủ hạ ngay). Không có "băng đạn" — dùng pin nội bộ hồi chậm theo thời
  gian (giống mana hơn là ammo).
- **Vai trò cốt truyện:** đây là bằng chứng vật lý đầu tiên người chơi
  cầm trên tay cho thấy BRDF không chỉ "cố dọn dẹp" mà **đã và đang phát
  triển vũ khí quân sự thử nghiệm** ngay tại nơi dịch bệnh bắt nguồn —
  củng cố twist Campaign 4 (GDD mục 14.1) bằng gameplay, không chỉ qua
  audio log.
- **Animation:** 3 clip riêng không thuộc class nào (Charge/Windup, Fire,
  Overheat-fail nếu giữ quá lâu) — chấp nhận không tái dùng được vì chỉ 1
  khẩu, xuất hiện muộn, giá trị tường thuật cao hơn giá trị tái dùng.

---

## Attachment system — áp dụng theo class (không phải theo từng khẩu)

GDD mục 17.6 đã chốt khung (2-3 attachment MVP, áp hệ số vào `Gun`
component lúc spawn). Với 36 khẩu, áp dụng attachment **theo class** để
không phải thiết kế riêng 36 lần:

| Attachment | Áp dụng cho class nào | Hệ số |
|---|---|---|
| Ống ngắm (xem bảng zoom tier riêng ngay dưới — không còn 1 tier "4x" chung) | AR, DMR, Sniper, LMG | Giảm `BULLET_SPREAD` khi ADS, tier cao hơn cộng thêm yêu cầu đứng yên |
| Băng đạn mở rộng | Súng ngắn, SMG, AR, DMR, LMG | Cộng thêm vào `MAGAZINE_SIZE` |
| Báng nặng / tay cầm chống giật | AR, DMR, Sniper, LMG, Battle Rifle | Giảm hệ số giật (`RECOIL_PITCH_KICK`/`RECOIL_ROLL_DEG`) |
| Giảm thanh gắn thêm | Súng ngắn, SMG, AR (KHÔNG áp được cho khẩu ĐÃ giảm thanh sẵn — SMG Giảm Thanh/Bắn Tỉa Giảm Thanh không cần) | Giảm hệ số `PLAYER_THREAT_PER_SHOT`, không giảm damage |
| Lưỡi lê | Cận chiến nhẹ + AR/DMR (gắn dưới nòng) | Đòn cận chiến nhanh không cần đổi vũ khí, dame thấp |

Không thiết kế attachment riêng cho Cận chiến/Cưa máy/Súng phun lửa/Thực
nghiệm — không hợp logic (súng cận chiến không gắn ống ngắm).

---

## Zoom tier ống ngắm — dựa theo ngưỡng thật ngoài đời

Nguyên tắc thật (súng trường thật): dưới ~8x cầm tay ngắm vẫn ổn định;
từ ~8x trở lên rung tay bình thường đã bị khuếch đại rõ trong tầm nhìn,
ngoài đời cần tì/kê (bipod, nằm bắn) mới bắn chính xác được. Áp trực
tiếp vào game thành 1 cơ chế thay vì chỉ đổi số `BULLET_SPREAD` phẳng —
**đúng tinh thần "khớp vật lý thật" toàn bộ tài liệu vũ khí/zombie này
đã theo từ đầu**, không phải chỉ đẹp số liệu.

| Tier | Zoom | Class dùng | Cầm tay/di chuyển | Cơ chế thêm |
|---|---|---|---|---|
| 0 — Iron sight/red dot | 1x | Súng ngắn, SMG, Shotgun, Nỏ, Phóng lựu | Bắn/di chuyển tự do, không phạt | — |
| 1 — LPVO thấp | 1-4x | AR, LMG | Bắn/di chuyển tự do, ADS khi đang chạy vẫn được (phạt spread nhẹ như hiện tại) | — |
| 2 — Trung bình | 4-8x | DMR/Battle Rifle | ADS đứng yên ổn định; ADS khi đang di chuyển bị lắc ống ngắm rõ (`SCOPE_SWAY` tăng theo tốc độ di chuyển) — vẫn bắn được, chỉ khó ngắm hơn, không khoá cứng | — |
| 3 — Cao (sniper) | 8-20x tuỳ khẩu (bolt-action rẻ ~8x, khẩu cao cấp cuối game ~16-20x) | Súng bắn tỉa (bolt-action) | ADS khi đang đứng/di chuyển: `SCOPE_SWAY` rất lớn, gần như không ngắm được mục tiêu nhỏ/xa | **Cơ chế "Brace" mới:** đứng yên hẳn ≥ 0.3s (không di chuyển, kể cả strafe nhẹ) → sway giảm dần về gần 0 sau ~1s "nín thở giữ ổn định" (giống Sniper Elite/Escape from Tarkov, không phải cơ chế lạ) — đây chính là "kê súng" ảo, không cần vật lý bipod thật |

**Vì sao không dùng bipod vật lý thật (đặt xuống mặt phẳng cụ thể):**
phức tạp hoá UI/animation không cần thiết cho scope FPS multiplayer nhịp
nhanh — "đứng yên đủ lâu = ổn định" là xấp xỉ đơn giản, đúng cảm giác
("cần kê mới bắn chính xác ở zoom cao") mà không cần dựng cơ chế đặt
bipod/tìm bề mặt riêng.

**Việc cần làm khi code:** thêm field `zoom_tier`/`scope_magnification`
vào `Gun` component (giá trị tĩnh theo loại ống ngắm gắn, không phải
theo khẩu súng — 1 khẩu DMR có thể gắn ống 1 trong tier 1/2 tuỳ
attachment chọn); thêm `SCOPE_SWAY` const + hệ thống tính sway theo
`(zoom_tier, moving, brace_timer)`; brace_timer là 1 component mới
(`ScopeBrace { still_secs: f32 }`) tăng khi player đứng yên trong lúc
ADS, reset khi di chuyển hoặc hết ADS.

## Đối chiếu GDD mục 5/15 — cần cập nhật số liệu

GDD hiện ghi "~12 khẩu" (mục 5) và "~12 khẩu, đủ nhóm vai trò như L4D"
(mục 15) — **cả 2 chỗ nên trỏ về file này** với số liệu mới: 36 súng + 4
throwable + 1 thực nghiệm = 41 vũ khí, 13 handling class. Giữ nguyên
tinh thần GDD mục 16 (chia giai đoạn) — **Giai đoạn 1 vẫn chỉ cần AK47 +
1-2 khẩu khác** (đủ chứng minh attachment system, theo đúng mục 17.6),
KHÔNG cần đủ 41 khẩu ngay — roster đầy đủ này là target Giai đoạn 3 (mục
16, "hoàn thiện toàn bộ ~20 map, ~8 loại địch, ~12 vũ khí"), giờ nâng
"~12" thành "~41" theo đúng quyết định hôm nay.

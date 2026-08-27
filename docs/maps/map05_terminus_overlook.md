# Map 5 — "Điểm hẹn" (Terminus Overlook) — Boss Map

Campaign 1 "Ground Zero", Map 5/5 — **kết thúc campaign**. Xem GDD.md
mục 14.3. Cùng legend với Map 1, thêm:

```
👹  Boss (Tank-style, mục 15) — encounter tái diễn, xem GDD 17.14/14.4
```

Map này **ngắn hơn hẳn** 4 map trước — trọng tâm là boss fight, không
phải hành trình di chuyển dài. Cấu trúc pacing cũng khác: không có
"Peak Fade" truyền thống (boss chết = kết thúc đột ngột, đúng bản chất
climax).

---

## Tầng 1-2 Gara (leo lên) [Build-up]

```
┌──────────────────────────────────────────────────────────┐
│  P P P P  (spawn — tiếp nối Extract Map 4)                   │
│                                                              │
│   ┌──────────────────────┐                                 │
│   │ Tầng 1 gara              │                                 │
│   │  Z          Z             │  A  (đạn — dọn dẹp trước       │
│   │                            │       boss fight)               │
│   └──────────┬───────────┘                                 │
│              ↑                                               │
│   ┌──────────┴───────────┐                                 │
│   │ Tầng 2 gara              │                                 │
│   │  Z                        │  H  (med kit — dọn dẹp cuối     │
│   │                            │       cùng trước boss)          │
│   └──────────┬───────────┘                                 │
└──────────────┼──────────────────────────────────────────────┘
               ↑ Lên bãi đáp trực thăng (nóc gara)
```

- **Mật độ:** 3 zombie lẻ tẻ — dọn dẹp nhanh, không phải cao trào.
- **Item:** 1 ammo + 1 med kit — đây chính là **"safe room" trước boss**
  (không có SR chính thức riêng ở đây, gộp luôn vào tầng gara cuối, đúng
  ghi chú GDD "khu vực ngay trước bãi đáp... đầy đủ đạn/med kit").
- **Vai trò:** cơ hội cuối cùng để nạp đầy tài nguyên trước fight khó nhất
  Campaign 1 — thiết kế có chủ đích để khuyến khích không bỏ sót item.

---

## Bãi đáp trực thăng — BOSS ARENA [SUSTAIN PEAK = Boss Fight]

```
┌──────────────────────────────────────────────────────────┐
│                    BÃI ĐÁP TRỰC THĂNG (nóc gara, ngoài trời)   │
│                                                              │
│        C                              C                     │
│      (cột bê tông — vật cản né đòn boss)                       │
│                                                              │
│                                                              │
│                         👹                                  │
│                   (Boss Tank-style,                           │
│                    xuất hiện NGAY khi                          │
│                    bước ra bãi đáp —                           │
│                    không có cảnh báo                           │
│                    trước, đúng "climax                         │
│                    đột ngột")                                  │
│                                                              │
│        C                              C                     │
│                                                              │
│   KHÔNG có zombie thường xen vào cùng lúc — giữ fight rõ        │
│   ràng, dễ đọc cho co-op 4 người (đúng chốt GDD 14.3)           │
│                                                              │
│   !!! Vệt cháy đen loang lổ trên sàn bãi đáp (deco — dấu vết    │
│       trực thăng cứu hộ từng đáp/cất cánh gần đây)               │
└───────────────────────────────┬────────────────────────────┘
```

- **Mật độ:** 0 zombie thường + **1 Boss** — toàn bộ độ khó dồn vào 1
  encounter, không pha trộn.
- **Kiến trúc:** 4 cột bê tông đối xứng làm vật cản né đòn — bố cục hỗ
  trợ co-op 4 người dàn ra các góc khác nhau thay vì dồn cục 1 chỗ.
- **Không có "Peak Fade":** boss chết → chuyển thẳng sang Relax, đúng
  bản chất climax kết thúc đột ngột (khác 4 map trước đều có giai đoạn
  hạ nhiệt dần).

---

## Kết thúc Campaign 1 [RELAX — Ending]

```
┌──────────────────────────────────────────────────────────┐
│  (boss đã hạ — bãi đáp yên tĩnh)                              │
│                                                              │
│   🔊 Audio log #5 tự động phát khi tới gần — giọng phi công      │
│       cứu hộ, xác nhận trực thăng đã bỏ đi TRƯỚC khi nhóm        │
│       tới (xem GDD 14.3 cho nội dung đầy đủ)                     │
│                                                              │
│                ┌─────────────────┐                          │
│                │  E (Extract Zone) │                          │
│                │  = KẾT THÚC        │                          │
│                │  CAMPAIGN 1        │                          │
│                └─────────────────┘                          │
│                                                              │
│   🔊 Sau extract: radio bắt tín hiệu dân sự tự phát từ         │
│       "Cổng thành" (twist nhẹ cuối campaign, xem GDD 14.3        │
│       và 14.4) — mở đường Campaign 2                             │
└──────────────────────────────────────────────────────────┘
```

- **Không có Safe Room riêng** ở map này — Extract Zone đồng thời là điểm
  kết thúc toàn Campaign 1 (không chỉ 1 map), lưu tiến độ ở cấp cao hơn
  (hoàn thành campaign, không chỉ 1 map — dùng `match_summary`/tương
  đương cấp-campaign, mục 12).

---

## Tổng kết số liệu

| Khu vực | Tag pacing | Zombie | Boss | Ammo | Med kit |
|---|---|---|---|---|---|
| Gara tầng 1-2 | Build-up | 3 | 0 | 1 | 1 |
| Bãi đáp | **Sustain Peak (Boss)** | 0 | **1** | 0 | 0 |
| Kết thúc | Relax | 0 | 0 | 0 | 0 |
| **Tổng** | | **3** | **1 boss** | **1** | **1** |

Map ngắn nhất Campaign 1 về nội dung di chuyển (chỉ 3 khu vực, so với 4-6
ở các map trước) nhưng **độ khó dồn hết vào 1 encounter duy nhất** — đúng
vai trò "climax cuối campaign", không cần độ dài để tạo cường độ.

---

## So sánh tổng thể Campaign 1 (Map 1-5)

| Map | Zombie thường | Đặc biệt | Boss | Ammo | Med kit | Vũ khí mới |
|---|---|---|---|---|---|---|
| 1. Highrise | 19 | 0 | 0 | 3 | 1 | 0 |
| 2. Meridian Plaza | 15 | 0 | 0 | 1 | 0 | 1 (súng ngắn) |
| 3. Meridian Station | 5 | 2 (Boomer) | 0 | 3 | 1 | 0 |
| 4. Route 9 Overpass | 8 | 0 | 0 | 1 | 1 | 1 (sniper) |
| 5. Terminus Overlook | 3 | 0 | 1 (Tank) | 1 | 1 | 0 |
| **Tổng Campaign 1** | **50** | **2** | **1** | **9** | **4** | **2** |

Đường cong độ khó **không tuyến tính tăng dần đều** — dao động theo chủ
đích (Map 1-2 đông về SỐ LƯỢNG, Map 3-4 khó về ĐỊA HÌNH/cơ chế, Map 5 dồn
vào 1 ĐIỂM duy nhất) — tránh nhàm chán do lặp lại cùng 1 kiểu thử thách,
đúng nguyên tắc pacing đa dạng thay vì chỉ tăng số lượng zombie mỗi map.

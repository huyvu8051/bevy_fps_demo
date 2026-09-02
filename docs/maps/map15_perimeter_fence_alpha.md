# Map 15 — "Hàng rào an ninh BRDF" (Perimeter Fence Alpha) — Boss Map

Campaign 3 "Vùng công nghiệp", Map 15/20 (Map 5/5, **kết thúc campaign**).
Xem GDD.md mục 14.5. Cùng legend với Map 5/10 (Boss `👹`, Tank-style tái
xuất lần 3).

⚠️ **v3 — diện tích x4 + zombie thường x12:** xem `map15_dimensions.md`.
**Boss KHÔNG x12** — giữ nguyên 1 con.

---

## Khu A — Dọc hàng rào ngoài [Build-up]

```
┌──────────────────────────────────────────────────────────┐
│  P P P P  (spawn — tiếp nối Extract Map 14)                   │
│                                                              │
│   ┌──┐  ┌──┐  ┌──┐  ┌──┐  (lô cốt dày đặc nhất Campaign 3,       │
│   │🗼│  │🗼│  │🗼│  │🗼│   tháp canh liên tiếp)                    │
│   └──┘  └──┘  └──┘  └──┘                                    │
│                                                              │
│        Z                                                    │
│                                                              │
│   📷 (camera an ninh — KHÔNG hoạt động, chỉ deco)               │
│                                                              │
│   !!! (xác lính BRDF, chết vì đạn — KHÔNG PHẢI zombie hóa,       │
│        gợi ý xung đột nội bộ BRDF hoặc chống lệnh rút lui,         │
│        deco quan trọng)                                          │
└───────────────────────────────┬────────────────────────────┘
                                 ═══ Tới khu safe room trước boss
```

- **Mật độ (v3, x12):** 12 zombie (v1: 1) — Build-up.
- **Ghi chú lore:** xác lính BRDF chết vì đạn (không phải bị cắn) là chi
  tiết quan trọng — gieo ý tưởng "ngay cả BRDF cũng không đồng lòng" cho
  Campaign 4.

---

## Khu B — Khu chuẩn bị (Safe Room mở rộng) [vẫn Build-up]

```
┌──────────────────────────────────────────────────────────┐
│   ┌──────────────────────┐                                 │
│   │  KHU HẬU CẦN LÔ CỐT       │  A  H  ← Đầy đủ trang bị —      │
│   │                          │       "khuyến khích dùng hết      │
│   │  L15 (Audio log #15 —    │       progression tích lũy         │
│   │  bảng hiệu an ninh, cảnh  │       trước khi vào Campaign 4"    │
│   │  báo tự động, giọng điệu  │       theo đúng GDD                │
│   │  đe dọa hơn hẳn mọi cảnh  │                                   │
│   │  báo BRDF trước đó)       │                                   │
│   └──────────────────────┘                                 │
└───────────────────────────────┬────────────────────────────┘
                                ↑ Ra sân giữa lô cốt
```

- **Item:** `L15`, đầy đủ ammo + med kit.

---

## Sân giữa các lô cốt — BOSS ARENA [SUSTAIN PEAK = Boss Fight #3]

```
┌──────────────────────────────────────────────────────────┐
│                    SÂN GIỮA CÁC LÔ CỐT (quy mô LỚN NHẤT           │
│                    trong 3 lần boss fight — công trình quân sự     │
│                    kiên cố nhất tính tới giờ)                       │
│                                                              │
│        C          C          C          C                     │
│      (nhiều vật cản bê tông hơn Map 5/10 — quy mô tăng dần        │
│       qua từng lần boss xuất hiện)                                │
│                                                              │
│                         👹                                  │
│                   (Boss Tank-style, TÁI XUẤT lần 3 —           │
│                    chỉ số tăng nhẹ tiếp theo so với Map 10,       │
│                    tune cụ thể ở Giai đoạn 3)                    │
│                                                              │
│        C          C          C          C                     │
│                                                              │
│   KHÔNG zombie thường xen vào — nhất quán với Map 5/10           │
└───────────────────────────────┬────────────────────────────┘
```

- **Mật độ:** 0 zombie thường + **1 Boss** (KHÔNG x12, giữ nguyên —
  encounter #3).

---

## Cổng chính Campaign 4 [RELAX ngắn — Cliffhanger]

```
┌──────────────────────────────────────────────────────────┐
│  (boss đã hạ)                                                │
│                                                              │
│                ┌─────────────────┐                          │
│                │  E (Extract Zone) │                          │
│                │  = KẾT THÚC        │                          │
│                │  CAMPAIGN 3        │                          │
│                └─────────────────┘                          │
│                                                              │
│   [Cổng chính "Cổng thành" hiện ra ngay phía trước — map        │
│    kết thúc ở ĐÚNG NGƯỠNG CỬA, không có radio twist như          │
│    Campaign 1 — hình ảnh trực quan (cổng) đã đủ tạo              │
│    cliffhanger, không cần audio thêm]                             │
└──────────────────────────────────────────────────────────┘
```

- **Không có Safe Room riêng cuối map này** — Safe Room đã gộp vào Khu B
  (trước boss), map kết thúc ngay sau boss để giữ cảm giác cliffhanger
  liền mạch dẫn sang Campaign 4.

---

## Tổng kết số liệu (v1 → v3, xem `map15_dimensions.md` cho nhịp spawn chi tiết)

| Khu vực | Tag pacing | Zombie (v1→v3) | Boss (không x12) | Ammo | Med kit | Audio log |
|---|---|---|---|---|---|---|
| A. Hàng rào ngoài | Build-up | 1 → **12** | 0 | 0 | 0 | — |
| B. Khu hậu cần | Build-up | 0 → 0 | 0 | 1 | 1 | L15 |
| C. Sân lô cốt (Boss) | **Sustain Peak** | 0 → 0 | **1** | 0 | 0 | — |
| **Tổng** | | **1 → 12** | **1 boss** | **1** | **1** | **1** |

---

## So sánh tổng thể Campaign 3 (Map 11-15, v1 → v3 sau x4 diện tích + x12 zombie thường)

| Map | Zombie thường (v1→v3) | Đặc biệt (không x12) | Boss (không x12) | Ammo | Med kit |
|---|---|---|---|---|---|
| 11. Ironloom Textile Mill | 6 → **72** | 0 | 0 | 1 | 1 |
| 12. Blackridge Freight Yard | 7 → **84** | 0 | 0 | 1 | 1 |
| 13. Cold Storage 7 | 4 → **48** | 1 (Witch) | 0 | 0 | 1 |
| 14. Refinery Loop | 5 → **60** | 0 | 0 | 1 | 1 |
| 15. Perimeter Fence Alpha | 1 → **12** | 0 | 1 (Tank) | 1 | 1 |
| **Tổng Campaign 3** | **23 → 276** | **1** | **1** | **4** | **5** |

Tổng zombie thường tương đương Campaign 2 sau scale (276 vs 264) nhưng
phân bố đều hơn (không có map nào cực đông như Map 1/2 của Campaign 1)
— đúng chiến lược "campaign giữa game" giữ nhịp độ ổn định, dồn kịch
tính cho Campaign 4 (cao trào cuối cùng). Đặc biệt/boss giữ nguyên
tuyệt đối qua v1→v3.

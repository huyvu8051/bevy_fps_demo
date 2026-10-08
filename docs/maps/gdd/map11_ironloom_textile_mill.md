# Map 11 — "Nhà máy dệt cũ" (Ironloom Textile Mill)

Campaign 3 "Vùng công nghiệp", Map 11/20 (Map 1/5 của campaign). Xem
GDD.md mục 14.5. Cùng legend với Map 1.

⚠️ **v3 — diện tích x4 + zombie x12 + spawn theo nhịp:** xem
`map11_dimensions.md` (map này không có địch đặc biệt/boss).

---

## Khu A — Khu văn phòng nhà máy [Build-up]

```
┌──────────────────────────────────────────────────────────┐
│  P P P P  (spawn — tiếp nối Extract Map 10, đầu Campaign 3)   │
│                                                              │
│   ┌──────────┐  ┌──────────┐                               │
│   │ Văn phòng   │  │ Phòng họp   │                               │
│   │  quản lý     │  │             │                               │
│   └──────────┘  └──────────┘                               │
│         Z                                                    │
│                                                              │
│   [Bụi dày phủ khắp — đóng cửa cả năm trước dịch, KHÔNG          │
│    liên quan Meridian, chỉ là nhà máy phá sản]                       │
└───────────────────────────────┬────────────────────────────┘
                                 ═══ Vào khu dây chuyền
```

- **Mật độ (v3, x12):** 12 zombie (v1: 1) — Build-up.

---

## Khu B — Dây chuyền máy dệt [SUSTAIN PEAK]

```
┌──────────────────────────────────────────────────────────┐
│  ⚙️  ⚙️  ⚙️  ⚙️  ⚙️  ⚙️  (máy dệt lớn — vật cản/che khuất,       │
│                             mê cung máy móc)                    │
│                                                              │
│   Z      Z         Z        Z                                │
│   (zombie bao vây từ nhiều hướng giữa các máy — tầm nhìn        │
│    bị chia cắt liên tục bởi khối máy móc lớn)                    │
│                                                              │
│   ┌────────────────────────┐                               │
│   │  Góc trú ẩn mới (không     │  !!! (lửa trại còn ấm, đồ hộp    │
│   │   liên quan chính, tùy       │      mới mở — người sống sót   │
│   │   chọn khám phá)             │      MỚI đã ở đây gần đây,       │
│   │                            │      deco, không gặp trực tiếp)  │
│   └────────────────────────┘                               │
│                                                              │
│  ⚙️  ⚙️  ⚙️  ⚙️  ⚙️  ⚙️                                        │
└───────────────────────────────┬────────────────────────────┘
                                 ═══
```

- **Mật độ (v3, x12):** 48 zombie (v1: 4), dồn trong không gian máy móc
  — Sustain Peak.
- **Ghi chú:** khu vực có dấu hiệu người sống sót mới trú ẩn KHÔNG dẫn
  tới gặp gỡ trực tiếp (giữ đúng nguyên tắc không thêm NPC chiến đấu) —
  chỉ là 1 chi tiết deco tạo cảm giác thế giới sống.

---

## Khu C — Kho vải [Peak Fade]

```
┌──────────────────────────────────────────────────────────┐
│   ┌──────────────────────┐                                 │
│   │  KHO VẢI (cuộn vải lớn    │                                 │
│   │   xếp chồng — cover)      │  L11 (Audio log #11 — sổ trực    │
│   │                          │  ca bảo vệ cũ, chi tiết đời       │
│   │   Z                       │  thường không liên quan dịch)     │
│   └──────────────────────┘                                 │
│                                                              │
│   ┌──────────────────┐                                     │
│   │ Tủ đồ bảo hộ lao      │  A (đạn)                            │
│   │ động                  │                                     │
│   └──────────────────┘                                     │
│   ┌──────────────────┐                                     │
│   │ Phòng y tế nhà máy    │  H (med kit)                        │
│   └──────────────────┘                                     │
└───────────────────────────────┬────────────────────────────┘
                                 ═══
```

- **Mật độ (v3, x12):** 12 zombie (v1: 1), Peak Fade.
- **Item:** `L11`, 1 ammo (tủ đồ bảo hộ), 1 med kit (phòng y tế).

---

## Khu D — Bãi bốc dỡ hàng [Relax + Safe Room + Extract]

```
┌──────────────────────────────────────────────────────────┐
│  (không có zombie)                                           │
│           ┌─────────────────────────┐                       │
│           │   SAFE ROOM (SR)          │                       │
│           └────────────┬────────────┘                       │
│                         ↓                                     │
│                ┌─────────────────┐                          │
│                │  E (Extract Zone) │                          │
│                │  → Map 12 Bãi       │                          │
│                │    container cảng   │                          │
│                └─────────────────┘                          │
└──────────────────────────────────────────────────────────┘
```

---

## Tổng kết số liệu (v1 → v3, xem `map11_dimensions.md` cho nhịp spawn chi tiết)

| Khu vực | Tag pacing | Zombie (v1→v3) | Ammo | Med kit | Audio log |
|---|---|---|---|---|---|
| A. Văn phòng | Build-up | 1 → **12** | 0 | 0 | — |
| B. Dây chuyền | **Sustain Peak** | 4 → **48** | 0 | 0 | — |
| C. Kho vải | Peak Fade | 1 → **12** | 1 | 1 | L11 |
| D. Bãi bốc dỡ | Relax | 0 → 0 | 0 | 0 | — |
| **Tổng** | | **6 → 72** | **1** | **1** | **1** |

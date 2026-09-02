# Map 14 — "Đường ống & Nhà máy lọc" (Refinery Loop)

Campaign 3 "Vùng công nghiệp", Map 14/20 (Map 4/5 của campaign). Xem
GDD.md mục 14.5. Cùng legend với Map 1. **Verticality cao nhất Campaign
3** — rơi khỏi đường đi không chết ngay, chỉ phải vòng lại (mất thời
gian/gặp thêm zombie).

⚠️ **v3 — diện tích x4 + zombie x12 + spawn theo nhịp:** xem
`map14_dimensions.md` (map này không có địch đặc biệt/boss). Chiều cao
giữa các tầng giàn giáo giữ nguyên — chỉ mặt bằng mỗi tầng x2.

---

## Khu A — Mặt đất nhà máy [Build-up]

```
┌──────────────────────────────────────────────────────────┐
│  P P P P  (spawn — tiếp nối Extract Map 13)                   │
│                                                              │
│   🛢️  🛢️  🛢️  (bồn chứa lớn ở mặt đất, chưa lên cao)           │
│                                                              │
│        Z                                                    │
│                                                              │
│   [Hàng rào BRDF mới hơn hẳn phần còn lại của nhà máy —         │
│    quây quanh bồn chứa số 9-12, nhìn thấy từ xa]                 │
└───────────────────────────────┬────────────────────────────┘
                                 ↑ Lên giàn giáo
```

- **Mật độ (v3, x12):** 12 zombie (v1: 1) — Build-up.

---

## Giàn giáo & Đường ống trên cao [SUSTAIN PEAK — verticality]

```
┌──────────────────────────────────────────────────────────┐
│  TẦNG GIÀN GIÁO 1                                             │
│   ═══┬═══┬═══┬═══  (lối đi hẹp trên giàn giáo, không có       │
│      │   │   │      chỗ né rộng)                                │
│      Z       Z                                               │
│                                                              │
│              ↓ [Nếu rơi khỏi đường đi — không chết, phải       │
│                 vòng lại từ dưới, mất thời gian]                 │
│                                                              │
│  TẦNG GIÀN GIÁO 2 (cao hơn)                                    │
│   ═══┬═══┬═══┬═══                                            │
│      │   Z   │                                               │
│                                                              │
│   ┌────────────────────┐                                   │
│   │ 🛢️ Bồn chứa 9-12       │  ← Rào BRDF mới, KHÔNG vào được    │
│   │  (rào chắn BRDF)       │    (chỉ nhìn thấy từ xa, gợi ý       │
│   │                        │    lore, không phải khu vực chơi      │
│   └────────────────────┘    được ở Campaign 3)                  │
│                                                              │
│  TẦNG GIÀN GIÁO 3 (cao nhất)                                   │
│   ═══┬═══┬═══┬═══                                            │
│      Z                                                        │
└───────────────────────────────┬────────────────────────────┘
                                 ↓ Xuống thang cuối
```

- **Mật độ (v3, x12):** 36 zombie (v1: 3) trải đều trên 3 tầng giàn
  giáo (12/tầng) — mật độ vừa nhưng độ khó tăng do lối đi hẹp, không có
  chỗ né rộng như mặt đất.
- **Ghi chú lore quan trọng:** bồn chứa 9-12 (nhắc trong audio log) chỉ
  **nhìn thấy, không vào được** ở campaign này — xây dựng sự tò mò/căng
  thẳng cho Campaign 4 (nơi người chơi cuối cùng vào được khu vực bí mật
  tương tự).

---

## Phòng điều khiển trung tâm [Peak Fade → Relax]

```
┌──────────────────────────────────────────────────────────┐
│   ┌──────────────────────────┐                             │
│   │  PHÒNG ĐIỀU KHIỂN TRUNG TÂM   │                             │
│   │   (nhiều màn hình/bảng          │                             │
│   │    điều khiển cũ)                │                             │
│   │                                │                             │
│   │   Z                             │  A  H  (đạn + med kit,        │
│   │                                │       safe room tự nhiên      │
│   │   L14 (Audio log #14 —          │       trước khi qua map        │
│   │   nhật ký kỹ sư vận hành,        │       cuối campaign)           │
│   │   BRDF thuê khu làm bãi          │                             │
│   │   trung chuyển, cấm vào bồn       │                             │
│   │   9-12)                          │                             │
│   └──────────────────────────┘                             │
│                                                              │
│           ┌─────────────────────────┐                       │
│           │   SAFE ROOM (SR)          │                       │
│           └────────────┬────────────┘                       │
│                         ↓                                     │
│                ┌─────────────────┐                          │
│                │  E (Extract Zone) │                          │
│                │  → Map 15 Hàng     │                          │
│                │    rào an ninh      │                          │
│                │    BRDF (Boss #3)   │                          │
│                └─────────────────┘                          │
└──────────────────────────────────────────────────────────┘
```

- **Mật độ (v3, x12):** 12 zombie (v1: 1), Peak Fade.
- **Item:** `L14`, đầy đủ ammo + med kit tại đây (thay vì tách riêng safe
  room, gộp luôn vào phòng điều khiển — tiết kiệm 1 khu vực riêng).

---

## Tổng kết số liệu (v1 → v3, xem `map14_dimensions.md` cho nhịp spawn chi tiết)

| Khu vực | Tag pacing | Zombie (v1→v3) | Ammo | Med kit | Audio log |
|---|---|---|---|---|---|
| A. Mặt đất | Build-up | 1 → **12** | 0 | 0 | — |
| B. Giàn giáo (3 tầng) | **Sustain Peak** | 3 → **36** | 0 | 0 | — |
| C. Phòng điều khiển | Peak Fade/Relax | 1 → **12** | 1 | 1 | L14 |
| **Tổng** | | **5 → 60** | **1** | **1** | **1** |

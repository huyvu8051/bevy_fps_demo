# Map 3 — "Ga tàu điện ngầm" (Meridian Station)

Campaign 1 "Ground Zero", Map 3/5. Xem GDD.md mục 14.3. Cùng legend với
Map 1. Đây là map **đầu tiên giới thiệu zombie đặc biệt** (Boomer-style,
mục 17.5/17.18) — không gian hẹp/tối được thiết kế chủ đích để tận dụng
đúng điểm mạnh của cơ chế nổ/khói mù trong không gian kín.

⚠️ **v3 — diện tích x4 + zombie thường x12 + spawn theo nhịp:** xem
`map03_dimensions.md`. **Boomer-style KHÔNG x12** — giữ nguyên 2 con
(set-piece cố định, xem ghi chú trong file dimensions).

---

## Khu A — Khu soát vé & Sân ga vào [Build-up]

```
┌──────────────────────────────────────────────────────────┐
│  P P P P  (spawn — tiếp nối Extract Map 2)                   │
│                                                              │
│   ┌──────────────┐                                          │
│   │ Khu soát vé     │  (đèn khẩn cấp còn sáng — ánh sáng vàng   │
│   │  Z               │   yếu, KHÁC hẳn tông tối của khu C)      │
│   └──────────────┘                                          │
│                                                              │
│   ══════════ (xuống cầu thang cuốn, tới sân ga)                │
│                                                              │
│   ┌────────────────────────────┐                            │
│   │  SÂN GA (còn ánh sáng)        │                            │
│   │   Z            A               │                            │
│   └────────────────────────────┘                            │
└───────────────────────────────┬────────────────────────────┘
                                 │ Nhảy xuống đường ray
                                 ↓
```

- **Mật độ (v3, x12):** 24 zombie (v1: 2) — Build-up, vẫn còn đủ sáng
  để quan sát rõ.
- **Item:** 1 ammo (sân ga).
- **Ghi chú:** đây là điểm chuyển tông quan trọng — ánh sáng khẩn cấp
  (vàng, yếu) là tín hiệu thị giác báo trước "sắp tối hơn nhiều".

---

## Khu B — Đường hầm tối [SUSTAIN PEAK — giới thiệu Boomer-style]

```
┌──────────────────────────────────────────────────────────┐
│  ĐƯỜNG HẦM (tối hoàn toàn — chỉ có đèn pin người chơi)         │
│                                                              │
│   ═══════╗                                                  │
│          ║  ← Khúc cua hẹp #1                                │
│          ╚═══════╗                                          │
│                   ║        💥Boo1  ← Boomer #1, đặt NGAY SAU  │
│                   ║              khúc cua — không thấy trước,  │
│                   ╚═══════╗      buộc phản ứng nhanh khi nổ    │
│                            ║                                  │
│                       Z    ║                                  │
│                            ╚═══════╗                          │
│                                     ║  ← Khúc cua hẹp #2        │
│                                     ║                          │
│                                Z    ║      💥Boo2  ← Boomer #2  │
│                                     ╚═══════════                │
│   A  H  A  (đạn/pin đèn xen kẽ dọc đường ray — duy trì nhịp     │
│              di chuyển liên tục thay vì dồn 1 điểm, theo GDD)   │
└───────────────────────────────┬────────────────────────────┘
                                 ═══
```

- **Mật độ (v3, x12 zombie thường):** 24 zombie thường (v1: 2) + **2
  Boomer-style** (`💥`, KHÔNG x12, giữ nguyên) — đây là lần đầu loại
  địch đặc biệt xuất hiện trong toàn game.
- **Item:** 3 điểm pickup xen kẽ (2 ammo + 1 med kit) dọc đường ray, đúng
  ghi chú GDD "duy trì nhịp di chuyển liên tục thay vì dồn vào 1 điểm".
- **Ghi chú thiết kế quan trọng:** cả 2 Boomer đặt **ngay sau khúc cua**
  (không gian hẹp, không thấy trước khi tới gần) — không gian kín khiến
  vùng khói/khí độc khi nó nổ **khó tránh khỏi hơn nhiều** so với không
  gian mở (đúng lý do GDD chọn map này để giới thiệu Boomer: "khói mù
  càng nguy hiểm trong không gian kín, không có chỗ tránh").

---

## Khu C — Ga đến [Peak Fade → Relax]

```
┌──────────────────────────────────────────────────────────┐
│  ═══════ (đường ray dẫn ra, ánh sáng bắt đầu tăng dần)          │
│                                                              │
│   Z  ← 1 zombie sót lại, Peak Fade                            │
│                                                              │
│   ┌────────────────────────────┐                            │
│   │  SÂN GA ĐẾN (sáng sủa hẳn —   │  [Relax — tương phản có     │
│   │   tương phản có chủ đích với  │   chủ đích với đường hầm      │
│   │   đường hầm tối vừa qua)       │   tối vừa trải qua]          │
│   └────────────────────────────┘                            │
│                                                              │
│           ┌─────────────────────────┐                       │
│           │   SAFE ROOM (SR)          │                       │
│           └────────────┬────────────┘                       │
│                         │                                     │
│                         ↓                                     │
│                ┌─────────────────┐                          │
│                │  E (Extract Zone) │                          │
│                │  Lên mặt đất,      │                          │
│                │  hướng cầu vượt →  │                          │
│                │  Map 4             │                          │
│                └─────────────────┘                          │
└──────────────────────────────────────────────────────────┘
```

- **Mật độ (v3, x12):** 12 zombie (v1: 1), tối thiểu.
- **Ghi chú:** ánh sáng sân ga đến được thiết kế **sáng hơn hẳn** ga vào
  (khác biệt trực quan rõ rệt) — nhấn mạnh cảm giác "thoát khỏi bóng tối"
  đúng như GDD mô tả, không chỉ là hết địch mà còn là thay đổi cảm xúc
  qua ánh sáng.

---

## Tổng kết số liệu (v1 → v3, xem `map03_dimensions.md` cho nhịp spawn chi tiết)

| Khu vực | Tag pacing | Zombie thường (v1→v3) | Boomer (không x12) | Ammo | Med kit |
|---|---|---|---|---|---|
| A. Soát vé/sân ga vào | Build-up | 2 → **24** | 0 | 1 | 0 |
| B. Đường hầm | **Sustain Peak** | 2 → **24** | **2** | 2 | 1 |
| C. Ga đến | Peak Fade/Relax | 1 → **12** | 0 | 0 | 0 |
| **Tổng** | | **5 → 60** | **2** | **3** | **1** |

Tổng zombie thường **thấp nhất Campaign 1 tới giờ** (5→60, so với
19→228 ở Map 1 và 15→180 ở Map 2) — bù lại bằng **địa hình khó** (tối,
hẹp) + Boomer (giữ nguyên 2, không x12), đúng triết lý thiết kế "độ khó
không chỉ tới từ số lượng địch".

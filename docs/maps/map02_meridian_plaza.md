# Map 2 — "Phố mua sắm" (Meridian Plaza)

Campaign 1 "Ground Zero", Map 2/5. Xem GDD.md mục 14.3. Cùng legend với
Map 1 (`docs/maps/map01_highrise.md`).

Map này chia theo **khu vực** thay vì tầng (chủ yếu 1 mặt bằng, trừ khu
bãi đỗ xe cuối map) — không gian mở hơn hẳn Map 1.

---

## Khu A — Lối vào quảng trường [Build-up]

```
┌──────────────────────────────────────────────────────────┐
│  P P P P  (spawn — tiếp nối từ Extract Map 1)                │
│                                                              │
│   ══════════════ (đường phố dẫn vào quảng trường)             │
│                                                              │
│           Z                                                 │
│                                                              │
│   ┌───────────┐                                             │
│   │ Cửa hàng     │  A (đạn — kệ hàng còn 1 ít)                  │
│   │ tiện lợi     │                                             │
│   └───────────┘                                             │
└───────────────────────────────┬────────────────────────────┘
                                 ═══
```

- **Mật độ:** 1 zombie — đúng Build-up, cho người chơi quan sát không
  gian mở trước khi vào quảng trường.
- **Item:** 1 ammo (cửa hàng tiện lợi).

---

## Khu B — Quảng trường trung tâm [SUSTAIN PEAK]

```
┌──────────────────────────────────────────────────────────┐
│         ┌──────────┐        ┌──────────┐                  │
│         │ Cửa hàng   │        │ Cửa hàng   │                  │
│         │ quần áo    │        │ đồ điện tử  │  L2 (Audio log   │
│         │  (trống,   │        │             │  #2 — nhóm       │
│         │   cướp phá)│        │             │  sống sót trước) │
│         └────┬─────┘        └────┬─────┘                  │
│              │  Zx2               │  Zx2                    │
│              ↓                    ↓                          │
│   ═══════════▼════════════════════▼═══════════════════      │
│                    QUẢNG TRƯỜNG (không gian mở)               │
│                                                              │
│                    X ← Trigger: đi vào giữa quảng trường       │
│                         → zombie ập ra từ MỌI cửa hàng          │
│                         xung quanh cùng lúc (phục kích          │
│                         theo vị trí, không cần hành động        │
│                         chủ động — khác cơ chế Map 1)           │
│                                                              │
│         Zx3              Zx3              Zx3                │
│    (từ cửa hàng      (từ cửa hàng     (từ cửa hàng            │
│     phía Tây)          phía Nam)        phía Đông)             │
│                                                              │
│         ┌──────────┐        ┌──────────┐                  │
│         │ Cửa hàng   │        │ Cửa hàng   │                  │
│         │ giày dép    │        │ đồ ăn nhanh │                  │
│         └──────────┘        └──────────┘                  │
└───────────────────────────────┬────────────────────────────┘
                                 ═══
```

- **Mật độ:** ~13 zombie (cao nhất Campaign 1 tính tới thời điểm này) —
  spawn đồng loạt từ nhiều hướng khi chạm `X`, đúng thiết kế "phục kích
  vị trí" trong GDD 14.3 (khác Map 1 F3 vốn dùng trigger hành động rõ
  ràng — ở đây trigger là 1 vùng vô hình, không có dấu hiệu báo trước).
- **Item:** `L2` (audio log #2, cửa hàng đồ điện tử).
- **Ghi chú thiết kế:** quảng trường mở khiến người chơi dễ bị bao vây
  360° — khuyến khích chọn 1 góc tường/cửa hàng làm lưng dựa thay vì đứng
  giữa khi combat nổ ra.

---

## Khu C — Khu rào chắn (Nhóm sống sót trước) [Peak Fade]

```
┌──────────────────────────────────────────────────────────┐
│   ┌────────────────────────────────┐                      │
│   │  RÀO CHẮN TẠM (bàn ghế dựng chéo) │                      │
│   │                                    │                      │
│   │   !!! (xác nhóm sống sót trước,    │                      │
│   │        súng đã hết đạn — deco)      │                      │
│   │                                    │                      │
│   │   A2 ← Vũ khí thứ 2 (súng ngắn      │                      │
│   │        dự phòng) — GIỚI THIỆU        │                      │
│   │        hệ thống đổi vũ khí lần đầu   │                      │
│   │        trong toàn game               │                      │
│   └────────────────────────────────┘                      │
│                                                              │
│              Z                                              │
└───────────────────────────────┬────────────────────────────┘
                                 ═══
```

- **Mật độ:** 1 zombie lẻ, giảm mạnh — đúng Peak Fade.
- **Item:** `A2` — vũ khí thứ 2 (không phải ammo thường), điểm mốc gameplay
  quan trọng (đầu tiên người chơi có lựa chọn vũ khí trong toàn Campaign
  1). **Không có ammo/med kit thường ở khu này** — đúng ghi chú GDD "đạn
  khan hiếm hơn ở map này" (nhóm trước đã lấy hết).

---

## Khu D — Bãi đỗ xe nhiều tầng [Relax + Safe Room + Extract]

```
┌──────────────────────────────────────────────────────────┐
│  TẦNG TRÊN BÃI ĐỖ XE (không có zombie)                      │
│                                                              │
│           ┌─────────────────────────┐                       │
│           │   SAFE ROOM (SR)          │                       │
│           │   - Armory / attachment    │                       │
│           │   - Lưu tiến độ            │                       │
│           └────────────┬────────────┘                       │
│                         │                                     │
│                         ↓ Xuống dốc bãi xe                     │
│                                                              │
│                ┌─────────────────┐                          │
│                │  E (Extract Zone) │                          │
│                │  Lối xuống Ga tàu  │                          │
│                │  điện ngầm →       │                          │
│                │  Map 3             │                          │
│                └─────────────────┘                          │
└──────────────────────────────────────────────────────────┘
```

- Không có zombie — `[Relax]`.
- Extract Zone dẫn thẳng xuống Map 3 (đúng lore: ga tàu điện ngầm nằm
  dưới bãi đỗ xe).

---

## Tổng kết số liệu

| Khu vực | Tag pacing | Số zombie | Ammo | Vũ khí | Audio log |
|---|---|---|---|---|---|
| A. Lối vào | Build-up | 1 | 1 | 0 | — |
| B. Quảng trường | **Sustain Peak** | ~13 (đồng loạt) | 0 | 0 | L2 |
| C. Rào chắn | Peak Fade | 1 | 0 | 1 (A2) | — |
| D. Bãi đỗ xe | Relax | 0 | 0 | 0 | — |
| **Tổng** | | **~15 zombie** | **1** | **1** | **1** |

So với Map 1 (~19 zombie, 3 ammo, 1 med kit): tổng zombie giảm nhẹ nhưng
**dồn cực đậm vào 1 điểm** (quảng trường) thay vì rải đều — tạo cảm giác
cường độ cao trào mạnh hơn dù tổng số ít hơn, đúng nguyên tắc "peak" thật
sự gây ấn tượng thay vì chỉ đơn thuần tăng số lượng.

# Map 16 — "Trạm kiểm soát chính" (Main Gatehouse)

Campaign 4 "Cổng thành", Map 16/20 (Map 1/5 của campaign, **bắt đầu twist
chính**). Xem GDD.md mục 14.6. Cùng legend với Map 1, thêm:

```
🪖  Lính BRDF — địch mới, còn sống/có tổ chức (không phải xác chết như
    các campaign trước), đôi khi bắn cả zombie nếu zombie tới gần họ trước
```

Map đầu tiên **lính BRDF thật sự xuất hiện làm địch** — khác hoàn toàn
với các campaign trước, nơi BRDF chỉ tồn tại qua xác chết/dấu vết.

⚠️ **v3 — diện tích x4 + zombie thường x12:** xem `map16_dimensions.md`.
**Lính BRDF KHÔNG x12** — giữ nguyên số lượng gốc (địch có AI/hành vi
riêng, không phải horde).

---

## Khu A — Ngoài hàng rào chính [Build-up — vẫn chỉ có zombie]

```
┌──────────────────────────────────────────────────────────┐
│  P P P P  (spawn — tiếp nối Extract Map 15, qua cổng chính)   │
│                                                              │
│   ═══════ NGOÀI HÀNG RÀO (vẫn bình thường như các map trước) ═ │
│                                                              │
│        Z          Z                                         │
│                                                              │
│   ⚡ (hàng rào điện — chỉ deco, không cản đường chính)          │
└───────────────────────────────┬────────────────────────────┘
                                 ═══ Vào trạm gác
```

- **Mật độ (v3, x12):** 24 zombie thường (v1: 2), KHÔNG có lính BRDF ở
  đây — Build-up giữ nguyên "bình thường" trước khi tạo bất ngờ ở Khu B.

---

## Khu B — Trạm gác chính [SUSTAIN PEAK — lần đầu chạm trán lính BRDF]

```
┌──────────────────────────────────────────────────────────┐
│   ┌──────────────┐                                         │
│   │ Vọng gác          │  🪖 ← Lính BRDF tuần tra, LẦN ĐẦU        │
│   │                  │    xuất hiện còn sống trong toàn game    │
│   └──────────────┘                                         │
│                                                              │
│   ══════════════ SÂN TRẠM GÁC ══════════════                   │
│                                                              │
│        Z         🪖        Z                                 │
│                                                              │
│   (lính đôi khi TỰ BẮN zombie nếu zombie tới gần họ trước —      │
│    tạo hỗn loạn 3 phe: người chơi / zombie / lính BRDF)           │
│                                                              │
│   L16 (Audio log #16 — bộ đàm lính BRDF nhặt từ 1 lính đã       │
│        hạ, hội thoại nội bộ xác nhận coi dân thường là mối        │
│        đe dọa)                                                  │
└───────────────────────────────┬────────────────────────────┘
                                 ═══
```

- **Mật độ (v3, x12 zombie thường):** 24 zombie thường (v1: 2) + **2
  lính BRDF** (KHÔNG x12, giữ nguyên — số lượng nhỏ theo đúng GDD "không
  đông như zombie horde, vai trò khác — nguy hiểm hơn từng cá thể").
- **Item:** `L16`.
- **Cơ chế:** lính rơi lại đạn khi hạ (nguồn đạn mới, khác hẳn pickup dân
  sự) — không cần đặt ammo pickup cố định ở khu này.

---

## Khu C — Sân trong [Peak Fade]

```
┌──────────────────────────────────────────────────────────┐
│   [Biển hiệu chính thức: "CƠ SỞ NGHIÊN CỨU SINH HỌC             │
│    BLACKRIDGE — BRDF" — LẦN ĐẦU tên đầy đủ cơ sở xuất hiện,      │
│    xác nhận trực quan điều audio log #16 vừa hé lộ]               │
│                                                              │
│        Z                                                    │
│                                                              │
│   ┌──────────────┐                                         │
│   │ Phòng chờ          │  📺 (TV vẫn phát loop bản tin          │
│   │                  │      tuyên truyền cũ "tình hình đang     │
│   │                  │      được kiểm soát tốt" — deco mỉa mai)  │
│   └──────────────┘                                         │
└───────────────────────────────┬────────────────────────────┘
                                 ═══
```

- **Mật độ (v3, x12):** 12 zombie (v1: 1), Peak Fade.

---

## Khu D — Trạm gác phụ bỏ trống [Relax + Safe Room + Extract]

```
┌──────────────────────────────────────────────────────────┐
│  (không có địch)                                             │
│           ┌─────────────────────────┐                       │
│           │   SAFE ROOM (SR)          │                       │
│           └────────────┬────────────┘                       │
│                         ↓                                     │
│                ┌─────────────────┐                          │
│                │  E (Extract Zone) │                          │
│                │  → Map 17 Khu       │                          │
│                │    hành chính       │                          │
│                └─────────────────┘                          │
└──────────────────────────────────────────────────────────┘
```

---

## Tổng kết số liệu (v1 → v3, xem `map16_dimensions.md` cho nhịp spawn chi tiết)

| Khu vực | Tag pacing | Zombie (v1→v3) | Lính BRDF (không x12) | Audio log |
|---|---|---|---|---|
| A. Ngoài hàng rào | Build-up | 2 → **24** | 0 | — |
| B. Trạm gác chính | **Sustain Peak** | 2 → **24** | **2** | L16 |
| C. Sân trong | Peak Fade | 1 → **12** | 0 | — |
| D. Trạm gác phụ | Relax | 0 → 0 | 0 | — |
| **Tổng** | | **5 → 60** | **2** | **1** |

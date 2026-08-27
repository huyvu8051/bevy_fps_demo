# Map 17 — "Khu hành chính" (Sector 3 Admin Block)

Campaign 4 "Cổng thành", Map 17/20 (Map 2/5 của campaign). Xem GDD.md
mục 14.6. Cùng legend với Map 16. **Cấu trúc không gian giống Map 1 có
chủ đích** — tạo cảm giác "quay vòng" sau hành trình dài quanh thành phố.

---

## Khu A — Sảnh hành chính [Build-up]

```
┌──────────────────────────────────────────────────────────┐
│  P P P P  (spawn — tiếp nối Extract Map 16)                   │
│                                                              │
│   ┌──────────────────────┐                                 │
│   │  SẢNH HÀNH CHÍNH          │  (bố cục CỐ Ý gợi nhớ sảnh        │
│   │                          │   Map 1 — cùng kiểu không gian     │
│   │  🪖                       │   văn phòng, khác hẳn 15 map        │
│   │                          │   vừa qua)                          │
│   └──────────────────────┘                                 │
└───────────────────────────────┬────────────────────────────┘
                                ↑ Lên khu văn phòng chỉ huy
```

- **Mật độ:** 1 lính BRDF — Build-up.

---

## Khu văn phòng chỉ huy [SUSTAIN PEAK — lính phòng thủ dày hơn Map 16]

```
┌──────────────────────────────────────────────────────────┐
│   ┌────────┐  ┌────────┐  ┌────────┐                     │
│   │ VP nhỏ 1  │  │ VP nhỏ 2  │  │ VP chỉ huy   │  🪖 🪖          │
│   │ Z         │  │           │  │              │  (canh gác      │
│   └────────┘  └────────┘  └────────┘  dày hơn hẳn     │
│                                            Map 16)          │
│   ══════════════ HÀNH LANG ══════════════                     │
│                                                              │
│        Z         🪖                                          │
│  (zombie vẫn xen kẽ ngay cả trong khu vực an ninh cao —        │
│   gợi ý BRDF cũng không kiểm soát hoàn toàn tình hình nội bộ)   │
└───────────────────────────────┬────────────────────────────┘
                                 ═══ Phòng lưu trữ
```

- **Mật độ:** 2 zombie + **3 lính BRDF** — mật độ lính tăng so với Map
  16, đúng ghi chú GDD "bảo vệ khu vực nhạy cảm hơn".

---

## Phòng lưu trữ hồ sơ [Peak Fade — Twist chính được XÁC NHẬN]

```
┌──────────────────────────────────────────────────────────┐
│   ┌──────────────────────────┐                             │
│   │  PHÒNG LƯU TRỮ HỒ SƠ          │                             │
│   │                                │                             │
│   │   🔥 (tủ hồ sơ bị đốt/tiêu     │  🪖                          │
│   │       hủy dở dang — BRDF        │                             │
│   │       đang thực hiện đúng       │                             │
│   │       "khuyến nghị tiêu hủy     │                             │
│   │       bằng chứng" trong log)    │                             │
│   │                                │                             │
│   │   🖨️ (máy hủy tài liệu vẫn      │                             │
│   │       chạy, kẹt giấy quá tải)   │                             │
│   │                                │                             │
│   │   📄 L17 ← TÀI LIỆU VĂN BẢN     │                             │
│   │      (KHÔNG PHẢI audio log —    │                             │
│   │      lần DUY NHẤT trong toàn     │                             │
│   │      game — Báo cáo sự cố       │                             │
│   │      #BR-001, mã "LAZARUS",     │                             │
│   │      containment thất bại,      │                             │
│   │      khuyến nghị tiêu hủy       │                             │
│   │      bằng chứng — XÁC NHẬN       │                             │
│   │      HOÀN TOÀN twist chính)     │                             │
│   └──────────────────────────┘                             │
└───────────────────────────────┬────────────────────────────┘
                                 ═══
```

- **Mật độ:** 1 lính BRDF, Peak Fade.
- **Item:** `L17` — **tài liệu then chốt nhất toàn game** (xem GDD 14.6
  cho nội dung đầy đủ báo cáo #BR-001).

---

## Phòng nghỉ nhân viên [Relax + Safe Room + Extract]

```
┌──────────────────────────────────────────────────────────┐
│  (không có địch)                                             │
│   A  H  (đạn/med kit)                                         │
│           ┌─────────────────────────┐                       │
│           │   SAFE ROOM (SR)          │                       │
│           └────────────┬────────────┘                       │
│                         ↓                                     │
│                ┌─────────────────┐                          │
│                │  E (Extract Zone) │                          │
│                │  → Map 18 Phòng     │                          │
│                │    thí nghiệm       │                          │
│                │    chính (ngầm)      │                          │
│                └─────────────────┘                          │
└──────────────────────────────────────────────────────────┘
```

---

## Tổng kết số liệu

| Khu vực | Tag pacing | Zombie | Lính BRDF | Ammo | Med kit | Tài liệu |
|---|---|---|---|---|---|---|
| A. Sảnh | Build-up | 0 | 1 | 0 | 0 | — |
| B. VP chỉ huy | **Sustain Peak** | 2 | **3** | 0 | 0 | — |
| C. Phòng lưu trữ | Peak Fade | 0 | 1 | 0 | 0 | L17 |
| D. Phòng nghỉ | Relax | 0 | 0 | 1 | 1 | — |
| **Tổng** | | **2** | **5** | **1** | **1** | **1** |

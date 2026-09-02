# Map 19 — "Boong-ke chỉ huy" (Command Bunker)

Campaign 4 "Cổng thành", Map 19/20 (Map 4/5 của campaign). Xem GDD.md
mục 14.6. Cùng legend với Map 16, thêm:

```
👔  Chỉ huy BRDF — encounter đặc biệt, KHÔNG PHẢI Tank-style, là nhân
    vật người thật, chạy trốn/kích hoạt tự hủy thay vì đánh tay đôi
```

⚠️ **Phá lệ có chủ đích:** đây là **lần DUY NHẤT trong toàn game** 1
"nhân vật" thực sự lên tiếng trực tiếp (không qua log/ghi âm) — xem GDD
14.6 để hiểu vì sao đây là ngoại lệ có chủ đích, không phải vi phạm
nguyên tắc "chỉ kể qua log" (mục 14.8).

⚠️ **v3 — diện tích x4 + zombie thường x12:** xem `map19_dimensions.md`.
**Lính BRDF và chỉ huy BRDF KHÔNG x12** — giữ nguyên tuyệt đối, chỉ huy
đặc biệt tuyệt đối không nhân bản (là 1 sự kiện kịch bản, không phải
combat đếm số).

---

## Hành lang ngoài [Build-up → SUSTAIN PEAK — lính phòng thủ dày nhất game]

```
┌──────────────────────────────────────────────────────────┐
│  P P P P  (spawn — tiếp nối Extract Map 18)                   │
│                                                              │
│   ══════════════ HÀNH LANG BOONG-KE (bê tông dày) ══════════   │
│                                                              │
│   🪖  🪖  🪖  🪖  🪖  (lính BRDF DÀY ĐẶC NHẤT toàn game —        │
│                          đây là khu vực họ bảo vệ quyết         │
│                          liệt nhất)                              │
│                                                              │
│        Z              Z                                     │
│  (vẫn có zombie xen kẽ, dù ít hơn hẳn số lính)                  │
└───────────────────────────────┬────────────────────────────┘
                                 ═══ Vào phòng chỉ huy
```

- **Mật độ (v3, x12 zombie):** 24 zombie (v1: 2) + **5 lính BRDF**
  (KHÔNG x12, giữ nguyên) — vẫn cao nhất toàn game về số lượng lính,
  đúng vai trò "khu vực bảo vệ quyết liệt nhất".
- **Không có item pickup** — áp lực thời gian đã bắt đầu ngầm từ đây
  (dù đếm ngược tự hủy chưa chính thức kích hoạt).

---

## Phòng chỉ huy trung tâm [SUSTAIN PEAK — Đối đầu chỉ huy BRDF]

```
┌──────────────────────────────────────────────────────────┐
│   ┌──────────────────────────────┐                         │
│   │  PHÒNG CHỈ HUY TRUNG TÂM           │                         │
│   │                                    │                         │
│   │  📺📺📺📺 (nhiều màn hình giám sát   │  ← Camera khắp cơ sở,   │
│   │            camera an ninh)           │    BAO GỒM CẢ khu vực   │
│   │                                    │    Map 16-18 vừa đi qua  │
│   │                                    │    — xác nhận BRDF đã     │
│   │                                    │    theo dõi nhóm SUỐT     │
│   │              👔                     │    từ đầu Campaign 4       │
│   │      (Chỉ huy BRDF, bị dồn           │                         │
│   │       vào đường cùng)                │                         │
│   │                                    │                         │
│   │  L19 (KHÔNG PHẢI log tìm thấy —       │                         │
│   │  LỜI THOẠI TRỰC TIẾP qua loa           │                         │
│   │  boong-ke: "Các người không hiểu       │                         │
│   │  đâu... Cơ sở sẽ tự hủy trong 5         │                         │
│   │  phút. Lựa chọn của các người:          │                         │
│   │  chạy, hoặc chết cùng sự thật này.")     │                         │
│   └──────────────────────────────┘                         │
│                                                              │
│   👔 → chạy trốn qua cửa phụ (KHÔNG đánh tay đôi) sau khi        │
│         kích hoạt tự hủy — không phải "hạ" theo nghĩa combat      │
│         thông thường, mà là 1 sự kiện kịch bản                     │
└───────────────────────────────┬────────────────────────────┘
                                 X ← TRIGGER: hệ thống tự hủy
                                    kích hoạt, đếm ngược 5 phút
                                    bắt đầu, báo động đỏ khắp nơi
```

- **Mật độ:** 0 zombie thường mới ở đây — toàn bộ tập trung vào encounter
  kịch bản với chỉ huy BRDF.
- **Cơ chế đặc biệt:** đây KHÔNG phải boss fight combat thông thường —
  chỉ huy chạy trốn, kích hoạt tự hủy, rồi map ngay lập tức chuyển tông
  sang tẩu thoát (không có "Peak Fade" hạ nhiệt như các map khác).

---

## Chuyển tiếp khẩn cấp sang Map 20 [Không có Relax — báo động dẫn thẳng]

```
┌──────────────────────────────────────────────────────────┐
│   🚨🚨🚨 BÁO ĐỘNG SƠ TÁN KHẨN CẤP 🚨🚨🚨                       │
│                                                              │
│   Đếm ngược: 5:00 → ... (đồng hồ đếm ngược hiển thị UI,         │
│                            chạy xuyên suốt map 20)                │
│                                                              │
│   Cửa thoát hiểm mở tự động — dẫn thẳng vào Map 20 KHÔNG        │
│   có Safe Room, KHÔNG có Extract Zone riêng ở map này —           │
│   map 19 và map 20 nối liền mạch không ngắt quãng                  │
└──────────────────────────────────────────────────────────┘
```

- **Ghi chú thiết kế:** đây là map DUY NHẤT trong toàn game không kết
  thúc bằng Safe Room + Extract Zone riêng — chuyển tiếp trực tiếp sang
  Map 20 để giữ áp lực thời gian liền mạch, không cho người chơi "nghỉ"
  giữa 2 map như bình thường.

---

## Tổng kết số liệu (v1 → v3, xem `map19_dimensions.md` cho chi tiết)

| Khu vực | Tag pacing | Zombie (v1→v3) | Lính BRDF (không x12) | Chỉ huy (không x12) | Ghi chú |
|---|---|---|---|---|---|
| A. Hành lang ngoài | Build-up/Sustain | 2 → **24** | **5** | 0 | — |
| B. Phòng chỉ huy | **Sustain Peak (kịch bản)** | 0 → 0 | 0 | **1 (thoát)** | L19, kích hoạt tự hủy |
| **Tổng** | | **2 → 24** | **5** | **1** | Không Relax, nối thẳng Map 20 |

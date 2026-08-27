# Map 4 — "Cầu vượt" (Route 9 Overpass)

Campaign 1 "Ground Zero", Map 4/5. Xem GDD.md mục 14.3. Cùng legend với
Map 1. Map ngoài trời, **sightline dài nhất Campaign 1** — thiết kế chủ
đích để giới thiệu vũ khí tầm xa.

---

## Khu A — Đầu cầu (dốc lên) [Build-up]

```
┌──────────────────────────────────────────────────────────┐
│  P P P P  (spawn — tiếp nối Extract Map 3, ra khỏi lòng đất)   │
│                                                              │
│           ╱╱╱╱╱╱╱╱╱╱  (dốc lên cầu, ngoài trời)                │
│                                                              │
│                    Z                                        │
│                                                              │
│   [xe cộ thưa thớt 2 bên đường dốc — chưa kẹt xe dày]          │
└───────────────────────────────┬────────────────────────────┘
                                 ═══ (lên tới mặt cầu chính)
```

- **Mật độ:** 1 zombie — Build-up, mặt cầu còn thoáng.

---

## Khu B — Giữa cầu, kẹt xe dày đặc [SUSTAIN PEAK]

```
┌──────────────────────────────────────────────────────────┐
│   🚗🚙🚕🚐🚗🚙 🚕🚐🚗🚙  (kẹt xe hàng loạt, obstacle course      │
│                             tự nhiên — không đi thẳng được)     │
│                                                              │
│   Z    💤Z   Z    💤Z    Z   💤Z                              │
│        ↑           ↑          ↑                              │
│   Zombie "ẩn" — giả chết trong/dưới gầm xe (💤), kích hoạt       │
│   khi người chơi lại gần — jump-scare nhẹ, xen kẽ zombie         │
│   thường (Z) đứng lộ sẵn                                         │
│                                                              │
│   ┌──────────┐                                              │
│   │ Xe quân sự │  !!! (xe BRDF lật/cháy — gợi ý quân đội         │
│   │ BRDF lật    │       cũng rút chạy hỗn loạn, không trật tự)   │
│   └──────────┘                                              │
│                                                              │
│   🚑 A  H  ← Xe cứu thương bỏ lại, đạn + med kit bên trong        │
└───────────────────────────────┬────────────────────────────┘
                                 ═══
```

- **Mật độ:** 3 zombie lộ sẵn + 3 zombie "ẩn" (giả chết, dùng lại animation
  Idle/Crawl có sẵn theo đúng ghi chú GDD) = 6 tổng, mật độ vừa nhưng bất
  ngờ cao.
- **Item:** 1 ammo + 1 med kit (trong xe cứu thương).
- **Ghi chú thiết kế:** xe cộ dày đặc vừa là **cover** vừa là **nơi giấu
  địch** — buộc người chơi di chuyển chậm/cẩn thận thay vì chạy thẳng qua,
  khác hẳn nhịp độ "chạy và bắn" của Map 1-2.

---

## Khu C — Đoạn cầu thoáng cuối [Peak Fade — Sightline dài]

```
┌──────────────────────────────────────────────────────────┐
│                                                              │
│   ═══════════════════════════════════════ (mặt cầu thoáng,     │
│                                              không có xe che,    │
│                                              nhìn thẳng xa)      │
│                                                              │
│                                            Z  (xa, cuối tầm     │
│                                                nhìn — mục tiêu   │
│                                                tự nhiên cho      │
│                                                vũ khí tầm xa)    │
│                                                              │
│   🎯 SNIPER ← Vị trí đặt vũ khí sniper LẦN ĐẦU trong toàn game,  │
│                tận dụng đúng sightline dài của map này            │
└───────────────────────────────┬────────────────────────────┘
                                 ═══
```

- **Mật độ:** 1 zombie, đặt xa để minh họa tầm bắn — Peak Fade.
- **Item:** vũ khí sniper (điểm mốc gameplay, giống cách Map 2 giới thiệu
  vũ khí thứ 2).
- **Ghi chú:** đoạn này **không có xe che chắn** — đối lập trực tiếp với
  Khu B, để sightline thực sự phát huy tác dụng minh họa vũ khí mới.

---

## Khu D — Cuối cầu, gần điểm hẹn [Relax + Safe Room + Extract]

```
┌──────────────────────────────────────────────────────────┐
│  (không có zombie)                                           │
│                                                              │
│           ┌─────────────────────────┐                       │
│           │   SAFE ROOM (SR)          │                       │
│           └────────────┬────────────┘                       │
│                         │                                     │
│                         ↓                                     │
│                ┌─────────────────┐                          │
│                │  E (Extract Zone) │                          │
│                │  Gara "Điểm hẹn" → │                          │
│                │  Map 5 (Boss)      │                          │
│                └─────────────────┘                          │
└──────────────────────────────────────────────────────────┘
```

---

## Tổng kết số liệu

| Khu vực | Tag pacing | Zombie lộ | Zombie ẩn | Ammo | Med kit | Vũ khí |
|---|---|---|---|---|---|---|
| A. Đầu cầu | Build-up | 1 | 0 | 0 | 0 | 0 |
| B. Giữa cầu | **Sustain Peak** | 3 | 3 | 1 | 1 | 0 |
| C. Cuối cầu thoáng | Peak Fade | 1 | 0 | 0 | 0 | 1 (sniper) |
| D. Cuối cầu/Relax | Relax | 0 | 0 | 0 | 0 | 0 |
| **Tổng** | | **5** | **3** | **1** | **1** | **1** |

Map ngắn hơn Map 1-3 về mật độ tổng (8 zombie), bù lại bằng **địa hình
obstacle course + zombie ẩn** tạo cảm giác nguy hiểm không tương xứng với
con số thô — đúng dụng ý thiết kế đã ghi trong GDD.

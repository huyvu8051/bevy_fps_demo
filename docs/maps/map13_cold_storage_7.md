# Map 13 — "Nhà kho hóa chất" (Cold Storage 7)

Campaign 3 "Vùng công nghiệp", Map 13/20 (Map 3/5 của campaign). Xem
GDD.md mục 14.5. Cùng legend với Map 1, thêm:

```
🧊  Witch-style — đứng yên, chỉ tấn công cực mạnh nếu bị làm phiền (mục 15)
```

⚠️ **Khác biệt cấu trúc pacing:** map này KHÔNG kết thúc bằng Peak
Fade→Relax như thông thường — thay vào đó, "Relax" chính là **khoảnh
khắc căng thẳng tĩnh** khi đi ngang Witch (xem GDD 14.5).

---

## Khu A — Khu nhận hàng [Build-up]

```
┌──────────────────────────────────────────────────────────┐
│  P P P P  (spawn — tiếp nối Extract Map 12)                   │
│                                                              │
│   ┌──────────────────────┐                                 │
│   │ KHU NHẬN HÀNG              │                                 │
│   │  Z                        │                                 │
│   └──────────────────────┘                                 │
│                                                              │
│   [Cảm biến nhiệt độ vẫn hoạt động — hiển thị số trên bảng      │
│    điều khiển, nhắc trong briefing]                              │
└───────────────────────────────┬────────────────────────────┘
                                 ═══ Vào hành lang chính
```

- **Mật độ:** 1 zombie — Build-up.

---

## Khu B — Hành lang chính, dọc phòng lạnh [SUSTAIN PEAK]

```
┌──────────────────────────────────────────────────────────┐
│   🚪Lạnh1  🚪Lạnh2  🚪Lạnh3  🚪Lạnh4  (nhiều cửa phòng lạnh      │
│                                          2 bên hành lang —       │
│                                          mỗi cửa là 1 rủi ro      │
│                                          tiềm ẩn)                 │
│                                                              │
│   ══════════════ HÀNH LANG CHÍNH ══════════════                 │
│                                                              │
│        Z          Z         Z                                │
│                                                              │
│   ❄️ (hơi sương particle — hiệu ứng thị giác, không ảnh          │
│        hưởng gameplay thực)                                     │
└───────────────────────────────┬────────────────────────────┘
                                 ═══ Tới phòng lạnh sâu nhất
```

- **Mật độ:** 3 zombie thường — Sustain Peak thông thường (chưa gặp
  Witch ở đây).
- **Ghi chú:** mật độ zombie thường ở khu này **THẤP hơn** các map khác
  cùng vai trò Sustain Peak — vì đỉnh điểm căng thẳng thật của map nằm ở
  Khu C (Witch), không phải ở đây.

---

## Khu C — Phòng lạnh sâu nhất (Witch) [RELAX kiểu đặc biệt — căng thẳng tĩnh]

```
┌──────────────────────────────────────────────────────────┐
│   ┌────────────────────────────────┐                       │
│   │  PHÒNG LẠNH SÂU NHẤT                │                       │
│   │                                    │                       │
│   │              🧊                     │  ← Witch, đứng yên,     │
│   │        (Witch, đứng im             │    NGỒI GỤC — không       │
│   │         lặng)                      │    tấn công trừ khi bị    │
│   │                                    │    làm phiền (đèn pin     │
│   │   H  ← Med kit ngay TRƯỚC khu       │    chiếu trực tiếp,       │
│   │        vực Witch — phần thưởng      │    tiếng súng gần, hoặc   │
│   │        cho ai CHỌN đối đầu thay      │    chạm phải)             │
│   │        vì né tránh                  │                          │
│   │                                    │                       │
│   │   [Khoang lạnh niêm phong BRDF      │                       │
│   │    đã bị PHÁ TỪ BÊN TRONG —         │                       │
│   │    đây chính là nơi Witch xuất       │                       │
│   │    hiện, liên kết trực tiếp Map 12]  │                       │
│   └────────────────────────────────┘                       │
│                                                              │
│   ┌─────────────┐                                          │
│   │ Lối phụ vòng    │  ← Nếu map có lối phụ, người chơi có thể    │
│   │ tránh Witch      │    đi vòng hoàn toàn né đối đầu             │
│   └─────────────┘                                          │
│                                                              │
│   L13 (Audio log #13 — biên bản kiểm tra kho lạnh,             │
│        khoang lạnh không rõ nguồn gốc, dấu BRDF)                │
└───────────────────────────────┬────────────────────────────┘
                                 ═══
```

- **Mật độ:** 0 zombie thường + **1 Witch** — đây là điểm nhấn chính của
  toàn map, KHÔNG cạnh tranh sự chú ý với zombie thường khác.
- **Item:** `L13`, 1 med kit (thưởng cho lựa chọn đối đầu).
- **Lựa chọn người chơi:** đi vòng an toàn (không phần thưởng) hoặc liều
  lĩnh rón rén qua/đối đầu (rủi ro cao, có med kit bù đắp) — đúng thiết
  kế "Witch-style" cổ điển của thể loại.

---

## Khu D — Sau phòng lạnh [Peak Fade → Extract]

```
┌──────────────────────────────────────────────────────────┐
│  (không có zombie — đã qua điểm căng thẳng chính)              │
│           ┌─────────────────────────┐                       │
│           │   SAFE ROOM (SR)          │                       │
│           └────────────┬────────────┘                       │
│                         ↓                                     │
│                ┌─────────────────┐                          │
│                │  E (Extract Zone) │                          │
│                │  → Map 14 Đường     │                          │
│                │    ống & Nhà máy    │                          │
│                │    lọc              │                          │
│                └─────────────────┘                          │
└──────────────────────────────────────────────────────────┘
```

---

## Tổng kết số liệu

| Khu vực | Tag pacing | Zombie thường | Witch | Ammo | Med kit | Audio log |
|---|---|---|---|---|---|---|
| A. Nhận hàng | Build-up | 1 | 0 | 0 | 0 | — |
| B. Hành lang chính | Sustain Peak (nhẹ) | 3 | 0 | 0 | 0 | — |
| C. Phòng lạnh sâu | **Đặc biệt (căng thẳng tĩnh)** | 0 | **1** | 0 | 1 | L13 |
| D. Sau phòng lạnh | Peak Fade/Relax | 0 | 0 | 0 | 0 | — |
| **Tổng** | | **4** | **1** | **0** | **1** | **1** |

Map có mật độ zombie thường thấp nhất Campaign 3 tính tới giờ (4) — đúng
chủ đích GDD "mật độ zombie thường THẤP nhất map này (Witch là mối đe dọa
chính, không cần đông zombie thường cạnh tranh sự chú ý)".

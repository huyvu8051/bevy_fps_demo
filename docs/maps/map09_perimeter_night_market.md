# Map 9 — "Khu chợ đêm ngoại ô" (Perimeter Night Market)

Campaign 2 "Vành đai", Map 9/20 (Map 4/5 của campaign). Xem GDD.md mục
14.4. Cùng legend với Map 1, thêm:

```
🤏  Jockey-style — kéo tách 1 người chơi khỏi nhóm (mục 15)
```

Map **đầu tiên bối cảnh ban đêm** trong toàn game (8 map trước đều ngày/
chạng vạng) — tương phản có chủ đích. Không gian đông đúc/chật hẹp giữa
các quầy hàng là địa hình lý tưởng để Jockey nguy hiểm thật sự (bị tách
khỏi nhóm giữa mê cung quầy hàng, không thấy đồng đội ngay).

⚠️ **v3 — diện tích x4 + zombie thường x12 + spawn theo nhịp:** xem
`map09_dimensions.md`. **Jockey-style KHÔNG x12** — giữ nguyên 1 con.

---

## Khu A — Lối vào chợ [Build-up — đèn còn sáng 1 phần]

```
┌──────────────────────────────────────────────────────────┐
│  P P P P  (spawn — tiếp nối Extract Map 8, trời đã tối)       │
│                                                              │
│   ✦ ✦ ✦ (dây đèn còn sáng, nhấp nháy chập chờn)                │
│                                                              │
│        Z                                                    │
│                                                              │
│   [Cổng chợ đêm — biển hiệu vẫn còn bật đèn]                   │
└───────────────────────────────┬────────────────────────────┘
                                 ═══ Vào giữa chợ
```

- **Mật độ (v3, x12):** 12 zombie (v1: 1) — Build-up, vẫn còn 1 phần
  ánh sáng.

---

## Khu B — Giữa chợ (mê cung quầy hàng) [SUSTAIN PEAK — giới thiệu Jockey]

```
┌──────────────────────────────────────────────────────────┐
│  🏮   🏮   🏮   🏮   🏮   🏮  (quầy hàng dày đặc, tầm nhìn        │
│                                  bị chia cắt liên tục)          │
│                                                              │
│   Z    Z         🤏        Z    Z                            │
│                (Jockey nhảy lên lưng 1 người chơi, KÉO         │
│                 tách khỏi nhóm qua các lối quầy hàng hẹp —     │
│                 mất tầm nhìn đồng đội trong vài giây, phải       │
│                 tự xoay sở hoặc đồng đội đuổi theo kịp)          │
│                                                              │
│  🏮   🏮   🏮   🏮   🏮   🏮                                    │
│                                                              │
│        Z              Z                                     │
│                                                              │
│   L9 (Audio log #9 — loa quầy hàng, quảng cáo lỗi xen           │
│       lệnh giới nghiêm BRDF)                                    │
└───────────────────────────────┬────────────────────────────┘
                                 ═══
```

- **Mật độ (v3, x12 zombie thường):** 48 zombie thường (v1: 4) + **1
  Jockey** (KHÔNG x12, giữ nguyên) — mật độ cao nhất Campaign 2 tính
  tới giờ, đúng "đêm + chật hẹp = nguy hiểm nhất".
- **Item:** `L9`.
- **Ghi chú thiết kế:** bố cục quầy hàng **cố ý không đối xứng, nhiều lối
  rẽ** — khác các map trước có đường đi rõ ràng, ở đây sự hỗn loạn không
  gian chính là 1 phần độ khó.

---

## Khu C — Khu chợ có dấu hiệu đụng độ [Peak Fade]

```
┌──────────────────────────────────────────────────────────┐
│   🏮   🏮  (quầy hàng thưa dần)                                │
│                                                              │
│        Z                                                    │
│                                                              │
│   !!! (vỏ đạn quân sự lẫn trong đống đổ vỡ — gợi ý ngầm         │
│        BRDF từng dùng vũ lực với dân thường tại đây, deco)       │
└───────────────────────────────┬────────────────────────────┘
                                 ═══
```

- **Mật độ (v3, x12):** 12 zombie (v1: 1), Peak Fade.
- **Ghi chú lore:** chi tiết nhỏ này là 1 trong các gợi ý sớm nhất về bản
  chất thật của BRDF (trước khi twist rõ ràng hơn ở Campaign 3-4).

---

## Khu D — Bãi đậu xe tải giao hàng [Relax + Safe Room + Extract]

```
┌──────────────────────────────────────────────────────────┐
│  (không có zombie)                                           │
│   🚚 A  H  (xe tải giao hàng — đạn + med kit bên trong)         │
│                                                              │
│           ┌─────────────────────────┐                       │
│           │   SAFE ROOM (SR)          │                       │
│           └────────────┬────────────┘                       │
│                         ↓                                     │
│                ┌─────────────────┐                          │
│                │  E (Extract Zone) │                          │
│                │  → Map 10          │                          │
│                │    Checkpoint Delta│                          │
│                │    (Boss #2)        │                          │
│                └─────────────────┘                          │
└──────────────────────────────────────────────────────────┘
```

---

## Tổng kết số liệu (v1 → v3, xem `map09_dimensions.md` cho nhịp spawn chi tiết)

| Khu vực | Tag pacing | Zombie thường (v1→v3) | Jockey (không x12) | Ammo | Med kit | Audio log |
|---|---|---|---|---|---|---|
| A. Lối vào | Build-up | 1 → **12** | 0 | 0 | 0 | — |
| B. Giữa chợ | **Sustain Peak** | 4 → **48** | **1** | 0 | 0 | L9 |
| C. Khu đụng độ | Peak Fade | 1 → **12** | 0 | 0 | 0 | — |
| D. Bãi đậu xe tải | Relax | 0 → 0 | 0 | 1 | 1 | — |
| **Tổng** | | **6 → 72** | **1** | **1** | **1** | **1** |

Mật độ zombie thường cao nhất Campaign 2 (6→72, so với 36/60/60 ở Map
6-8) — đúng chốt GDD "mật độ cao nhất Campaign 2" nhờ kết hợp yếu tố đêm
+ không gian chật hẹp.

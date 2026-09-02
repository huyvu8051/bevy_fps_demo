# Map 6 — "Khu nhà mẫu" (Sundown Gardens)

Campaign 2 "Vành đai", Map 6/20 (Map 1/5 của campaign). Xem GDD.md mục
14.4. Cùng legend với Map 1, thêm:

```
🐆  Hunter-style — nhảy vọt từ mái nhà, tấn công từ trên cao (mục 15)
```

Map đầu tiên **giới thiệu 4 loại zombie đặc biệt còn lại** (mỗi map 1
loại trong Map 6-9) — dùng module nhà lặp lại (tiết kiệm công dựng map,
đúng ghi chú GDD).

⚠️ **v3 — diện tích x4 + zombie thường x12 + spawn theo nhịp:** xem
`map06_dimensions.md`. **Hunter-style KHÔNG x12** — giữ nguyên 1 con.

---

## Khu A — Dãy nhà mẫu đầu khu [Build-up — "yên tĩnh bất thường"]

```
┌──────────────────────────────────────────────────────────┐
│  P P P P  (spawn — tiếp nối Extract Map 5, đầu Campaign 2)    │
│                                                              │
│   ┌────┐  ┌────┐  ┌────┐  ┌────┐  ┌────┐  ┌────┐          │
│   │Nhà 1 │  │Nhà 2 │  │Nhà 3 │  │Nhà 4 │  │Nhà 5 │  │Nhà 6 │          │
│   │      │  │      │  │      │  │      │  │      │  │      │          │
│   └──┬─┘  └────┘  └────┘  └──┬─┘  └────┘  └────┘          │
│      🐆                        │  ← Hunter trên mái nhà 4,      │
│  (Hunter ẩn trên mái nhà 1,      │    chưa lộ diện, chỉ thấy      │
│   quan sát người chơi, chưa       │    khi ngước lên (foreshadow   │
│   tấn công — dạy người chơi        │    trước khi map khác dùng)   │
│   "ngước nhìn lên" lần đầu)                                    │
│                                                              │
│   ═════════════════════════════════ (đường khu dân cư)         │
└───────────────────────────────┬────────────────────────────┘
                                 ═══
```

- **Mật độ:** 0 zombie thường lộ diện, chỉ có Hunter quan sát (chưa tấn
  công) — đúng tag "yên tĩnh bất thường", tạo cảm giác bị theo dõi mà
  không rõ nguồn.
- **Ghi chú thiết kế:** đoạn foreshadow Hunter ở đây **không tấn công**,
  chỉ để lộ thoáng qua (di chuyển giữa 2 mái nhà) — khi nó thực sự tấn
  công (Khu B) người chơi đã có 1 tín hiệu ngầm trước đó.

---

## Khu B — Công viên trung tâm khu [SUSTAIN PEAK — giới thiệu Hunter]

```
┌──────────────────────────────────────────────────────────┐
│         ┌────┐              ┌────┐                          │
│         │Nhà 7 │              │Nhà 8 │                          │
│         └────┘              └────┘                          │
│                                                              │
│   ═══════ CÔNG VIÊN NHỎ TRUNG TÂM (không gian mở) ═══════      │
│                                                              │
│              Z          Z                                    │
│                                                              │
│         🐆 ← Hunter NHẢY VỌT từ mái Nhà 7 xuống giữa công viên  │
│              khi người chơi đi ngang — tấn công bất ngờ từ       │
│              trên cao, KHÁC hẳn zombie thường luôn tấn công       │
│              từ mặt đất (mục 15/17.18)                            │
│                                                              │
│         ┌────┐              ┌────┐                          │
│         │Nhà 9 │              │Nhà 10│                          │
│         └────┘              └────┘                          │
└───────────────────────────────┬────────────────────────────┘
                                 ═══
```

- **Mật độ (v3, x12 zombie thường):** 24 zombie thường (v1: 2) + **1
  Hunter** (KHÔNG x12, giữ nguyên — encounter chính của map).
- **Kiến trúc:** công viên mở + nhà 1 tầng bao quanh = Hunter có nhiều
  điểm nhảy khác nhau, không cố định 1 hướng tấn công.

---

## Khu C — Đường chính ra khỏi khu [Peak Fade]

```
┌──────────────────────────────────────────────────────────┐
│   ┌────────────────────┐                                   │
│   │  "Nhà số 14"           │  !!! (đồ y tế gia đình vứt đầy    │
│   │  (ground-zero dân sự    │       sân — nhiệt kế vỡ, vỏ thuốc  │
│   │   đầu tiên, xem GDD)    │       hạ sốt — deco, không ai       │
│   │                        │       trong nhóm nhận ra ý nghĩa)   │
│   │  L6 (Audio log #6 —     │                                    │
│   │  máy trả lời tự động,   │                                    │
│   │  giọng người mẹ)        │                                    │
│   └────────────────────┘                                   │
│                                                              │
│   ┌────┐  Z  ┌────┐                                        │
│   │Nhà 11│      │Nhà 12│  A (đạn — nhà để xe, theo GDD)          │
│   └────┘      └────┘                                       │
└───────────────────────────────┬────────────────────────────┘
                                 ═══
```

- **Mật độ (v3, x12):** 12 zombie (v1: 1), giảm mạnh — Peak Fade.
- **Item:** `L6` (audio log #6), 1 ammo (nhà để xe).
- **Lore quan trọng:** "Nhà số 14" là địa điểm **ground-zero dân sự thật
  sự** theo timeline ẩn (GDD 14.1) — người chơi đi ngang mà không biết ý
  nghĩa, chỉ người đọc kỹ log + environmental storytelling mới ráp nối
  được.

---

## Khu D — Trạm xe buýt cuối khu [Relax + Safe Room + Extract]

```
┌──────────────────────────────────────────────────────────┐
│  (không có zombie)                                           │
│                                                              │
│           ┌─────────────────────────┐                       │
│           │   SAFE ROOM (SR)          │                       │
│           └────────────┬────────────┘                       │
│                         ↓                                     │
│                ┌─────────────────┐                          │
│                │  E (Extract Zone) │                          │
│                │  → Map 7 Trường    │                          │
│                │    tiểu học         │                          │
│                └─────────────────┘                          │
└──────────────────────────────────────────────────────────┘
```

---

## Tổng kết số liệu (v1 → v3, xem `map06_dimensions.md` cho nhịp spawn chi tiết)

| Khu vực | Tag pacing | Zombie thường (v1→v3) | Hunter (không x12) | Ammo | Audio log |
|---|---|---|---|---|---|
| A. Dãy nhà đầu | Build-up | 0 → 0 | 0 (foreshadow) | 0 | — |
| B. Công viên | **Sustain Peak** | 2 → **24** | **1** | 0 | — |
| C. Đường chính | Peak Fade | 1 → **12** | 0 | 1 | L6 |
| D. Trạm xe buýt | Relax | 0 → 0 | 0 | 0 | — |
| **Tổng** | | **3 → 36** | **1** | **1** | **1** |

Mật độ thấp nhất tính tới thời điểm này trong toàn game (3→36 zombie
thường) — đúng chủ đích GDD "vành đai thưa dân hơn trung tâm", bù lại
bằng độ nguy hiểm của Hunter thay vì số lượng.

# Map 6 — Dimension Spec x4 + Nhịp spawn x12 (bản nháp, chưa implement)

Bổ sung cho `map06_sundown_gardens.md`. Dùng chung thư viện kích thước ở
`map01_dimensions.md` mục 1. Khu dân cư module nhà lặp lại (10 nhà 1
tầng, ~5x6m/nhà).

⚠️ Footprint v1 ước lượng lần đầu. v2 = x2 mỗi cạnh. Zombie thường x12.
**Hunter-style KHÔNG x12** — giữ nguyên 1 con (foreshadow ở Khu A +
encounter thật ở Khu B) đúng nguyên tắc đã lập từ Map 3-5: địch đặc
biệt/boss không nhân theo công thức, chỉ zombie thường.

---

## Khu A — Dãy nhà mẫu đầu khu [Build-up — "yên tĩnh bất thường"]

- **Footprint v1:** dãy 6 nhà dọc đường, tổng ~36x16m → **v2: 72m x 32m**
- Nhà module: v1 5x6 → v2 10x12 mỗi nhà (6 nhà dàn đều dọc trục X)
- Hunter (foreshadow, không tấn công): trên mái Nhà 1 — vị trí giữ
  nguyên tỉ lệ, `(-28, y_mái, 0)`

**Không zombie thường lộ diện** — giữ nguyên đúng thiết kế gốc (0x12=0),
đây là chủ đích "yên tĩnh bất thường", không phải chỗ thiếu spawn.

---

## Khu B — Công viên trung tâm khu [SUSTAIN PEAK — giới thiệu Hunter]

- **Footprint v1:** công viên mở 20x16 + 4 nhà quanh rìa (5x6/nhà) →
  **v2: công viên 40x32, nhà 10x12/nhà**
- Hunter (encounter thật — nhảy từ mái Nhà 7): giữ nguyên 1 con, không
  x12, vị trí nhảy từ mái Nhà 7 `(-16, y_mái, 12)` xuống giữa công viên

**Nhịp spawn (zombie thường, x12):**

| Đợt | Trigger | Số zombie | Vị trí |
|---|---|---|---|
| 1 | Vào công viên (trước khi Hunter nhảy) | 12 (v1: 1, x12) | Rải quanh công viên |
| 2 | Ngay sau khi Hunter tấn công (tăng viện, tận dụng sự chú ý bị chia) | 12 (v1: 1, x12) | Từ 2 nhà còn lại quanh công viên |

Tổng zombie thường 24 (v1: 2, x12). Hunter vẫn 1.

---

## Khu C — Đường chính ra khỏi khu [Peak Fade]

- **Footprint v1:** 24x14 (2 nhà + "Nhà số 14" lore) → **v2: 48m x 28m**
- "Nhà số 14": tâm `(-16, y, -6)`, size 10x12 (x2 từ 5x6)
- `L6` audio log: trong Nhà số 14
- Ammo: nhà để xe cạnh Nhà 12, `(16, y, 6)`

**Nhịp spawn:**

| Đợt | Trigger | Số zombie | Vị trí |
|---|---|---|---|
| 1 | Đi qua khu vực | 12 (v1: 1, x12) | Rải quanh Nhà 11-12, tapering |

---

## Khu D — Trạm xe buýt cuối khu [Relax + Safe Room + Extract]

- **Footprint v1:** 16x12 → **v2: 32m x 24m**
- Không zombie.

---

## Tổng kết số liệu (v1 → v3)

| Khu vực | Tag pacing | Zombie thường (v1→v3) | Hunter (không x12) | Ammo | Audio log |
|---|---|---|---|---|---|
| A. Dãy nhà đầu | Build-up | 0 → 0 | 0 (foreshadow) | 0 | — |
| B. Công viên | **Sustain Peak** | 2 → **24** | **1** | 0 | — |
| C. Đường chính | Peak Fade | 1 → **12** | 0 | 1 | L6 |
| D. Trạm xe buýt | Relax | 0 → 0 | 0 | 0 | — |
| **Tổng** | | **3 → 36** | **1** | **1** | **1** |

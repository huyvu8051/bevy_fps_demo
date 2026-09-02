# Map 7 — Dimension Spec x4 + Nhịp spawn x12 (bản nháp, chưa implement)

Bổ sung cho `map07_fairview_elementary.md`. Dùng chung thư viện kích
thước ở `map01_dimensions.md` mục 1 (phòng học ~ tương đương "văn phòng
riêng" 4x3, x2 thành 8x6).

⚠️ Footprint v1 ước lượng lần đầu. v2 = x2 mỗi cạnh. Zombie thường x12.
**Smoker-style KHÔNG x12** — giữ nguyên 1 con (Lớp 2B), đúng nguyên tắc
chung.

---

## Khu A — Sân trường [Build-up]

- **Footprint v1:** 24x18 → **v2: 48m x 36m**
- Xe phụ huynh (deco): rải giữa sân, x2 khoảng cách

**Nhịp spawn:**

| Đợt | Trigger | Số zombie | Vị trí |
|---|---|---|---|
| 1 | Vào sân trường | 12 (v1: 1, x12) | Rải giữa sân |

---

## Tầng 1 — Hành lang chính & Thư viện [Build-up cuối]

- **Footprint v1:** hành lang 16x3 (rộng 1.8→3.6 v2) + 3 phòng (lớp
  1A/1B, phòng y tế) 4x3 mỗi phòng + thư viện 8x6 → **v2: hành lang
  32x7.2, phòng 8x6/phòng, thư viện 16x12**
- Med kit: phòng y tế, `(10, y, -3)`

**Nhịp spawn:**

| Đợt | Trigger | Số zombie | Vị trí |
|---|---|---|---|
| 1 | Vào hành lang tầng 1 | 12 (v1: 1, x12) | Giữa hành lang, gần thư viện |

---

## Tầng 2 — Hành lang lớp học [SUSTAIN PEAK — giới thiệu Smoker]

- **Footprint v1:** hành lang 20x3.6 (rộng 1.8→3.6) + 4 lớp 4x3/lớp →
  **v2: hành lang 40x7.2, lớp 8x6/lớp**
- Smoker (ẩn Lớp 2B, kéo dây qua cửa sổ vỡ): giữ nguyên 1 con, không
  x12, vị trí Lớp 2B tâm `(-8, y, -8)` (2 phòng từ trái, x2 khoảng cách)

**Nhịp spawn (zombie thường, x12):**

| Đợt | Trigger | Số zombie | Vị trí |
|---|---|---|---|
| 1 | Vào hành lang tầng 2 | 12 (v1: 1, x12) | Đầu hành lang |
| 2 | Sau khi Smoker kéo dây (tăng viện khi đồng đội đang bận cứu) | 12 (v1: 1, x12) | Giữa/cuối hành lang |

Tổng zombie thường 24 (v1: 2, x12). Smoker vẫn 1.

---

## Khu C — Thư viện (đường vòng) [Peak Fade]

- **Footprint v1:** thư viện tầng 2 20x14 + 1 lớp cố thủ 4x3 → **v2:
  40m x 28m, lớp cố thủ 8x6**
- Lớp cố thủ (deco, không xác): góc thư viện, tâm `(-14, y, -10)`

**Nhịp spawn:**

| Đợt | Trigger | Số zombie | Vị trí |
|---|---|---|---|
| 1 | Vào thư viện tầng 2 | 12 (v1: 1, x12) | Rải trong thư viện, tapering |

---

## Khu D — Nhà thể chất [Relax + Safe Room + Extract]

- **Footprint v1:** 24x18 (không gian rộng, sáng) → **v2: 48m x 36m**
- Không zombie.

---

## Tổng kết số liệu (v1 → v3)

| Khu vực | Tag pacing | Zombie thường (v1→v3) | Smoker (không x12) | Ammo | Med kit | Audio log |
|---|---|---|---|---|---|---|
| A. Sân trường | Build-up | 1 → **12** | 0 | 0 | 0 | — |
| B. Hành lang T1/Thư viện | Build-up | 1 → **12** | 0 | 0 | 1 | — |
| C. Hành lang T2 | **Sustain Peak** | 2 → **24** | **1** | 0 | 0 | — |
| D. Thư viện T2 | Peak Fade | 1 → **12** | 0 | 0 | 0 | — |
| E. Nhà thể chất | Relax | 0 → 0 | 0 | 0 | 0 | — |
| **Tổng** | | **5 → 60** | **1** | **0** | **1** | **L7** |

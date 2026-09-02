# Map 14 — Dimension Spec x4 + Nhịp spawn x12 (bản nháp, chưa implement)

Bổ sung cho `map14_refinery_loop.md`. Dùng chung thư viện kích thước ở
`map01_dimensions.md` mục 1. Map verticality cao nhất Campaign 3 (3 tầng
giàn giáo) — chiều cao giữa tầng **giữ nguyên** (không thuộc phạm vi x4
diện tích sàn), chỉ footprint mặt bằng mỗi tầng x2 mỗi cạnh.

---

## Khu A — Mặt đất nhà máy [Build-up]

- **Footprint v1:** 24x18 → **v2: 48m x 36m**
- Bồn chứa lớn (🛢️): 3 bồn, x2 kích thước + khoảng cách

**Nhịp spawn:**

| Đợt | Trigger | Số zombie | Vị trí |
|---|---|---|---|
| 1 | Vào mặt đất nhà máy | 12 (v1: 1, x12) | Quanh bồn chứa |

---

## Giàn giáo & Đường ống trên cao [SUSTAIN PEAK — verticality]

- **Footprint v1 mỗi tầng giàn giáo:** lối đi hẹp ~20m dài, rộng 1.8m →
  **v2: mỗi tầng dài 40m, rộng 3.6m** (vẫn hẹp hơn hành lang thường —
  giữ đúng chủ đích "không có chỗ né rộng", chỉ x2 chứ không mở rộng
  quá mức làm mất tính thử thách verticality)
- Chiều cao giữa 3 tầng: **giữ nguyên** `FLOOR_HEIGHT`-tương-đương (không
  x2) — đây là chiều Y, ngoài phạm vi "rộng x4"
- Bồn chứa 9-12 (rào BRDF, không vào được): vẫn chỉ nhìn thấy từ xa,
  không đổi vai trò dù map rộng hơn

**Nhịp spawn (zombie x12, chia đều 3 tầng):**

| Đợt | Trigger | Số zombie | Vị trí |
|---|---|---|---|
| 1 | Lên tầng giàn giáo 1 | 12 (v1: 1, x12) | Dọc lối đi tầng 1 |
| 2 | Lên tầng giàn giáo 2 | 12 (v1: 1, x12) | Dọc lối đi tầng 2 |
| 3 | Lên tầng giàn giáo 3 (cao nhất) | 12 (v1: 1, x12) | Dọc lối đi tầng 3 |

Tổng 36 (v1: 3, x12).

---

## Phòng điều khiển trung tâm [Peak Fade → Relax]

- **Footprint v1:** 16x12 → **v2: 32m x 24m**
- `L14`, ammo, med kit: theo vị trí gốc trong phòng

**Nhịp spawn:**

| Đợt | Trigger | Số zombie | Vị trí |
|---|---|---|---|
| 1 | Vào phòng điều khiển | 12 (v1: 1, x12) | Tapering |

---

## Tổng kết số liệu (v1 → v3)

| Khu vực | Tag pacing | Zombie (v1→v3) | Ammo | Med kit | Audio log |
|---|---|---|---|---|---|
| A. Mặt đất | Build-up | 1 → **12** | 0 | 0 | — |
| B. Giàn giáo (3 tầng) | **Sustain Peak** | 3 → **36** | 0 | 0 | — |
| C. Phòng điều khiển | Peak Fade/Relax | 1 → **12** | 1 | 1 | L14 |
| **Tổng** | | **5 → 60** | **1** | **1** | **1** |

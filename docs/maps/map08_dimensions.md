# Map 8 — Dimension Spec x4 + Nhịp spawn x12 (bản nháp, chưa implement)

Bổ sung cho `map08_millers_pond.md`. Dùng chung thư viện kích thước ở
`map01_dimensions.md` mục 1. Không gian ngoài trời mở nhất Campaign 2 —
x2 mỗi cạnh càng làm nổi bật đặc điểm này (nhiều chỗ né Spitter hơn).

⚠️ Footprint v1 ước lượng lần đầu. v2 = x2 mỗi cạnh. Zombie thường x12.
**Spitter-style KHÔNG x12** — giữ nguyên 1 con, đúng nguyên tắc chung.

---

## Khu A — Đường chạy bộ vào công viên [Build-up]

- **Footprint v1:** đường chạy dài 30m, rộng 6m → **v2: dài 60m, rộng 12m**

**Nhịp spawn:**

| Đợt | Trigger | Số zombie | Vị trí |
|---|---|---|---|
| 1 | Vào đường chạy | 12 (v1: 1, x12) | Rải dọc đường |

---

## Khu B — Khu mở cạnh hồ [SUSTAIN PEAK — giới thiệu Spitter]

- **Footprint v1:** đường chạy cạnh hồ 30x14 (không vật cản, cần rộng
  để né acid) → **v2: 60m x 28m** — càng rộng càng đúng chủ đích "đủ
  chỗ né vùng nguy hiểm"
- Spitter: đứng xa giữa khu, giữ nguyên 1 con, không x12, vị trí `(0, y,
  10)` (xa đường chạy, x2 khoảng cách)

**Nhịp spawn (zombie thường, x12):**

| Đợt | Trigger | Số zombie | Vị trí |
|---|---|---|---|
| 1 | Vào khu mở cạnh hồ | 12 (v1: 1, x12) | 1 phía đường chạy |
| 2 | Spitter bắt đầu nhả acid (tăng viện khi người chơi bận né) | 12 (v1: 1, x12) | Phía đối diện đường chạy |

Tổng zombie thường 24 (v1: 2, x12). Spitter vẫn 1.

---

## Khu C — Trạm xử lý nước (đường vòng, tùy chọn) [vẫn Sustain Peak]

- **Footprint v1:** 16x12 → **v2: 32m x 24m**
- Xe tuần tra không biển số (deco/lore): cạnh trạm

**Nhịp spawn:**

| Đợt | Trigger | Số zombie | Vị trí |
|---|---|---|---|
| 1 | Vào trạm xử lý nước (nếu chọn đường vòng) | 12 (v1: 1, x12) | Trong trạm |

---

## Khu D — Phía xa hồ [Peak Fade]

- **Footprint v1:** đường chạy 24x8 + gazebo 8x8 → **v2: đường chạy
  48x16, gazebo 16x16**
- `L8`, ammo, med kit: trong gazebo

**Nhịp spawn:**

| Đợt | Trigger | Số zombie | Vị trí |
|---|---|---|---|
| 1 | Đi qua khu vực | 12 (v1: 1, x12) | Rải dọc đường, tapering |

---

## Khu E — Bãi đỗ xe công viên [Relax + Safe Room + Extract]

- **Footprint v1:** 24x16 → **v2: 48m x 32m**
- Không zombie.

---

## Tổng kết số liệu (v1 → v3)

| Khu vực | Tag pacing | Zombie thường (v1→v3) | Spitter (không x12) | Ammo | Med kit | Audio log |
|---|---|---|---|---|---|---|
| A. Đường vào | Build-up | 1 → **12** | 0 | 0 | 0 | — |
| B. Khu mở cạnh hồ | **Sustain Peak** | 2 → **24** | **1** | 0 | 0 | — |
| C. Trạm xử lý nước | Sustain Peak (tùy chọn) | 1 → **12** | 0 | 0 | 0 | — |
| D. Phía xa hồ | Peak Fade | 1 → **12** | 0 | 1 | 1 | L8 |
| E. Bãi đỗ xe | Relax | 0 → 0 | 0 | 0 | 0 | — |
| **Tổng** | | **5 → 60** | **1** | **1** | **1** | **1** |

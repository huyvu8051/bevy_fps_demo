# Map 15 — Dimension Spec x4 + Nhịp spawn x12 (bản nháp, chưa implement) — Boss Map

Bổ sung cho `map15_perimeter_fence_alpha.md`. Dùng chung thư viện kích
thước ở `map01_dimensions.md` mục 1.

⚠️ Footprint v1 ước lượng lần đầu. v2 = x2 mỗi cạnh. Zombie thường x12.
**Boss (Tank-style, tái xuất lần 3) KHÔNG x12** — giữ nguyên 1 con.

---

## Khu A — Dọc hàng rào ngoài [Build-up]

- **Footprint v1:** 24x14 (4 lô cốt/tháp canh liên tiếp) → **v2: 48m x
  28m**
- 4 tháp canh (🗼): x2 kích thước + khoảng cách, giữ mật độ "dày đặc
  nhất Campaign 3"

**Nhịp spawn:**

| Đợt | Trigger | Số zombie | Vị trí |
|---|---|---|---|
| 1 | Vào dọc hàng rào | 12 (v1: 1, x12) | Rải dọc hàng rào |

---

## Khu B — Khu chuẩn bị (Safe Room mở rộng) [vẫn Build-up]

- **Footprint v1:** 16x12 → **v2: 32m x 24m**
- `L15`, ammo, med kit đầy đủ: theo vị trí gốc

**Không zombie** — giữ nguyên đúng bản gốc (0x12=0), đây là "safe room
mở rộng" trước boss, không phải chỗ combat.

---

## Sân giữa các lô cốt — BOSS ARENA [SUSTAIN PEAK = Boss Fight #3]

- **Footprint v1:** 32x32 (lớn nhất trong 3 lần boss, công trình quân
  sự kiên cố nhất) → **v2: 64m x 64m** (half 32x32)
- 8 vật cản bê tông (tăng từ 4 ở Map 5/10 theo đúng chốt gốc "quy mô
  tăng dần"): đối xứng quanh tâm, cách tâm ~20m (x2 từ ~10m)
- Boss (Tank-style, tái xuất lần 3): tâm arena, giữ nguyên 1 con, không
  x12

**Không zombie thường** — nhất quán Map 5/10, không đổi dù rộng hơn.

---

## Tổng kết số liệu (v1 → v3)

| Khu vực | Tag pacing | Zombie (v1→v3) | Boss (không x12) | Ammo | Med kit | Audio log |
|---|---|---|---|---|---|---|
| A. Hàng rào ngoài | Build-up | 1 → **12** | 0 | 0 | 0 | — |
| B. Khu hậu cần | Build-up | 0 → 0 | 0 | 1 | 1 | L15 |
| C. Sân lô cốt (Boss) | **Sustain Peak** | 0 → 0 | **1** | 0 | 0 | — |
| **Tổng** | | **1 → 12** | **1 boss** | **1** | **1** | **1** |

---

## So sánh tổng thể Campaign 3 (Map 11-15, sau x4 diện tích + x12 zombie thường)

| Map | Zombie thường (v1→v3) | Đặc biệt/Boss (không x12) | Ammo | Med kit |
|---|---|---|---|---|
| 11. Ironloom Textile Mill | 6 → **72** | 0 | 1 | 1 |
| 12. Blackridge Freight Yard | 7 → **84** | 0 | 1 | 1 |
| 13. Cold Storage 7 | 4 → **48** | 1 (Witch) | 0 | 1 |
| 14. Refinery Loop | 5 → **60** | 0 | 1 | 1 |
| 15. Perimeter Fence Alpha | 1 → **12** | 1 (Tank) | 1 | 1 |
| **Tổng Campaign 3** | **23 → 276** | **2** | **4** | **5** |

Đúng x12 (23×12=276). Nguyên tắc "chỉ x12 zombie thường, giữ nguyên
đặc biệt/boss" giữ nhất quán qua cả 3 campaign đã làm.

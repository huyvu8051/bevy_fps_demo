# Map 11 — Dimension Spec x4 + Nhịp spawn x12 (bản nháp, chưa implement)

Bổ sung cho `map11_ironloom_textile_mill.md`. Dùng chung thư viện kích
thước ở `map01_dimensions.md` mục 1.

⚠️ Footprint v1 ước lượng lần đầu. v2 = x2 mỗi cạnh. Zombie x12 (map
này không có địch đặc biệt/boss).

---

## Khu A — Khu văn phòng nhà máy [Build-up]

- **Footprint v1:** văn phòng quản lý 4x3 + phòng họp 5x4 + hành lang
  nối 8x2.4 → **v2: văn phòng 8x6, phòng họp 10x8, hành lang 16x4.8**

**Nhịp spawn:**

| Đợt | Trigger | Số zombie | Vị trí |
|---|---|---|---|
| 1 | Vào khu văn phòng | 12 (v1: 1, x12) | Giữa văn phòng/phòng họp |

---

## Khu B — Dây chuyền máy dệt [SUSTAIN PEAK]

- **Footprint v1:** khu máy dệt 24x18 (mê cung máy móc, 12 máy) →
  **v2: 48m x 36m**
- Máy dệt (⚙️): 12 khối, x2 kích thước mỗi khối và khoảng cách, rải đều
  2 hàng ngoài rìa khu

**Nhịp spawn:**

| Đợt | Trigger | Số zombie | Vị trí |
|---|---|---|---|
| 1 | Vào giữa dây chuyền | 24 (v1: 2, x12) | Rải giữa máy dệt, nửa khu đầu |
| 2 | Tiến sâu hơn (bao vây từ nhiều hướng) | 24 (v1: 2, x12) | Rải giữa máy dệt, nửa khu sau |

Tổng 48 (v1: 4, x12).

---

## Khu C — Kho vải [Peak Fade]

- **Footprint v1:** kho vải 10x8 + tủ đồ bảo hộ 3x3 + phòng y tế 4x3 →
  **v2: kho 20x16, tủ đồ 6x6, phòng y tế 8x6**
- `L11`, ammo, med kit: theo đúng vị trí gốc (kho/tủ đồ/phòng y tế)

**Nhịp spawn:**

| Đợt | Trigger | Số zombie | Vị trí |
|---|---|---|---|
| 1 | Vào kho vải | 12 (v1: 1, x12) | Rải trong kho, tapering |

---

## Khu D — Bãi bốc dỡ hàng [Relax + Safe Room + Extract]

- **Footprint v1:** 20x16 → **v2: 40m x 32m**. Không zombie.

---

## Tổng kết số liệu (v1 → v3)

| Khu vực | Tag pacing | Zombie (v1→v3) | Ammo | Med kit | Audio log |
|---|---|---|---|---|---|
| A. Văn phòng | Build-up | 1 → **12** | 0 | 0 | — |
| B. Dây chuyền | **Sustain Peak** | 4 → **48** | 0 | 0 | — |
| C. Kho vải | Peak Fade | 1 → **12** | 1 | 1 | L11 |
| D. Bãi bốc dỡ | Relax | 0 → 0 | 0 | 0 | — |
| **Tổng** | | **6 → 72** | **1** | **1** | **1** |

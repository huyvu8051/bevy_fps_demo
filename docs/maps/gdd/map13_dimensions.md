# Map 13 — Dimension Spec x4 + Nhịp spawn x12 (bản nháp, chưa implement)

Bổ sung cho `map13_cold_storage_7.md`. Dùng chung thư viện kích thước ở
`map01_dimensions.md` mục 1.

⚠️ Footprint v1 ước lượng lần đầu. v2 = x2 mỗi cạnh. Zombie thường x12.
**Witch-style KHÔNG x12** — giữ nguyên 1 con, đúng nguyên tắc chung (số
lượng loại đặc biệt gắn với thiết kế set-piece, không phải mật độ chung).

---

## Khu A — Khu nhận hàng [Build-up]

- **Footprint v1:** 16x12 → **v2: 32m x 24m**

**Nhịp spawn:**

| Đợt | Trigger | Số zombie | Vị trí |
|---|---|---|---|
| 1 | Vào khu nhận hàng | 12 (v1: 1, x12) | Rải trong khu |

---

## Khu B — Hành lang chính, dọc phòng lạnh [SUSTAIN PEAK nhẹ]

- **Footprint v1:** hành lang dài 24m, rộng 3.6 (x2 hành lang chính) →
  **v2: dài 48m, rộng 7.2m**
- 4 cửa phòng lạnh 2 bên: x2 khoảng cách dọc hành lang

**Nhịp spawn (vẫn giữ mật độ THẤP hơn map khác cùng vai trò Sustain
Peak — đỉnh điểm thật nằm ở Khu C):**

| Đợt | Trigger | Số zombie | Vị trí |
|---|---|---|---|
| 1 | Vào hành lang | 18 (v1: 1.5→ làm tròn từ 3, x12 chia 2 đợt) | Nửa đầu hành lang |
| 2 | Giữa hành lang | 18 | Nửa sau hành lang |

Tổng 36 (v1: 3, x12).

---

## Khu C — Phòng lạnh sâu nhất (Witch) [RELAX đặc biệt — căng thẳng tĩnh]

- **Footprint v1:** 14x10 → **v2: 28m x 20m** — rộng hơn cho lối phụ
  vòng tránh Witch rõ ràng hơn, đúng chủ đích "có lựa chọn thật sự"
- Witch: đứng yên giữa phòng, giữ nguyên 1 con, không x12, tâm `(0, y,
  0)`
- Lối phụ vòng tránh: dọc rìa Tây phòng, rộng 4.8m (x2 hành lang chính)
- Med kit, `L13`: ngay trước khu vực Witch

**Không zombie thường** — giữ nguyên nguyên tắc gốc "không cạnh tranh sự
chú ý với Witch", diện tích rộng hơn không đổi quyết định này.

---

## Khu D — Sau phòng lạnh [Peak Fade → Extract]

- **Footprint v1:** 16x12 → **v2: 32m x 24m**. Không zombie.

---

## Tổng kết số liệu (v1 → v3)

| Khu vực | Tag pacing | Zombie thường (v1→v3) | Witch (không x12) | Med kit | Audio log |
|---|---|---|---|---|---|
| A. Nhận hàng | Build-up | 1 → **12** | 0 | 0 | — |
| B. Hành lang chính | Sustain Peak (nhẹ) | 3 → **36** | 0 | 0 | — |
| C. Phòng lạnh sâu | Đặc biệt | 0 → 0 | **1** | 1 | L13 |
| D. Sau phòng lạnh | Peak Fade/Relax | 0 → 0 | 0 | 0 | — |
| **Tổng** | | **4 → 48** | **1** | **1** | **1** |

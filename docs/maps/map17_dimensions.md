# Map 17 — Dimension Spec x4 + Nhịp spawn x12 (bản nháp, chưa implement)

Bổ sung cho `map17_sector3_admin_block.md`. Dùng chung thư viện kích
thước ở `map01_dimensions.md` mục 1 — cấu trúc cố ý giống Map 1 (sảnh
hành chính), nên tái dùng luôn footprint 48x32 đã có ở
`map01_dimensions.md` mục 2 cho cả cụm tầng này thay vì ước lượng mới.

⚠️ v2 = x2 mỗi cạnh (đã áp dụng qua việc tái dùng 48x32). Zombie thường
x12. **Lính BRDF KHÔNG x12** — giữ nguyên, cùng nguyên tắc Map 16.

---

## Khu A — Sảnh hành chính [Build-up]

- **Footprint:** 48m x 32m (tái dùng từ Map 1, "cố ý gợi nhớ")
- Lính BRDF: giữ nguyên **1 con**, không x12

**Không zombie thường ở đây** — giữ nguyên bản gốc (0x12=0), chỉ có
lính Build-up.

---

## Khu văn phòng chỉ huy [SUSTAIN PEAK — lính phòng thủ dày hơn Map 16]

- **Footprint:** 48m x 32m (dùng lại)
- Lính BRDF: giữ nguyên **3 con**, không x12
- Ghi chú gốc "zombie vẫn xen kẽ ngay cả trong khu an ninh cao" — giữ
  đúng chủ đích lore

**Nhịp spawn (zombie thường, x12):**

| Đợt | Trigger | Số zombie | Vị trí |
|---|---|---|---|
| 1 | Vào khu văn phòng chỉ huy | 24 (v1: 2, x12) | Rải giữa các phòng nhỏ + hành lang |

Lính BRDF vẫn 3, không đổi.

---

## Phòng lưu trữ hồ sơ [Peak Fade — Twist chính được XÁC NHẬN]

- **Footprint:** 48m x 32m (dùng lại)
- Lính BRDF: giữ nguyên **1 con**, không x12
- `L17` (tài liệu văn bản, không phải audio log): vị trí không đổi

**Không zombie thường** — giữ nguyên bản gốc.

---

## Phòng nghỉ nhân viên [Relax + Safe Room + Extract]

- **Footprint:** 48m x 32m (dùng lại). Không địch.

---

## Tổng kết số liệu (v1 → v3)

| Khu vực | Tag pacing | Zombie (v1→v3) | Lính BRDF (không x12) | Tài liệu |
|---|---|---|---|---|
| A. Sảnh | Build-up | 0 → 0 | **1** | — |
| B. VP chỉ huy | **Sustain Peak** | 2 → **24** | **3** | — |
| C. Phòng lưu trữ | Peak Fade | 0 → 0 | **1** | L17 |
| D. Phòng nghỉ | Relax | 0 → 0 | 0 | — |
| **Tổng** | | **2 → 24** | **5** | **1** |

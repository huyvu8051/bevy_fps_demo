# Map 16 — Dimension Spec x4 + Nhịp spawn x12 (bản nháp, chưa implement)

Bổ sung cho `map16_main_gatehouse.md`. Dùng chung thư viện kích thước ở
`map01_dimensions.md` mục 1.

⚠️ Footprint v1 ước lượng lần đầu. v2 = x2 mỗi cạnh. Zombie thường x12.
**Lính BRDF KHÔNG x12** — giữ nguyên số lượng gốc, cùng nguyên tắc đã áp
cho Boomer/Hunter/Smoker/Spitter/Jockey/Witch/Tank: địch có AI/hành vi
riêng (không phải zombie horde vô tri) không nhân theo công thức số
lượng chung — đúng ghi chú GDD "lính không đông như horde, nguy hiểm
hơn từng cá thể", tăng số lượng sẽ phá vỡ chủ đích này.

---

## Khu A — Ngoài hàng rào chính [Build-up — vẫn chỉ có zombie]

- **Footprint v1:** 20x14 → **v2: 40m x 28m**

**Nhịp spawn:**

| Đợt | Trigger | Số zombie | Vị trí |
|---|---|---|---|
| 1 | Vào ngoài hàng rào | 24 (v1: 2, x12) | Rải dọc hàng rào |

---

## Khu B — Trạm gác chính [SUSTAIN PEAK — lần đầu chạm trán lính BRDF]

- **Footprint v1:** vọng gác 5x4 + sân trạm gác 20x14 → **v2: vọng gác
  10x8, sân 40m x 28m**
- Lính BRDF: giữ nguyên **2 con**, không x12, rải trong sân trạm gác
- `L16`: nhặt từ 1 lính đã hạ, không cố định vị trí

**Nhịp spawn (zombie thường, x12):**

| Đợt | Trigger | Số zombie | Vị trí |
|---|---|---|---|
| 1 | Vào sân trạm gác | 24 (v1: 2, x12) | Rải quanh sân, tạo hỗn loạn 3 phe với lính |

Lính BRDF vẫn 2, không đổi theo đợt.

---

## Khu C — Sân trong [Peak Fade]

- **Footprint v1:** 16x12 (phòng chờ + sân) → **v2: 32m x 24m**
- Biển hiệu "CƠ SỞ NGHIÊN CỨU SINH HỌC BLACKRIDGE — BRDF": vị trí nổi
  bật, không đổi

**Nhịp spawn:**

| Đợt | Trigger | Số zombie | Vị trí |
|---|---|---|---|
| 1 | Vào sân trong | 12 (v1: 1, x12) | Tapering |

---

## Khu D — Trạm gác phụ bỏ trống [Relax + Safe Room + Extract]

- **Footprint v1:** 16x12 → **v2: 32m x 24m**. Không địch.

---

## Tổng kết số liệu (v1 → v3)

| Khu vực | Tag pacing | Zombie (v1→v3) | Lính BRDF (không x12) | Audio log |
|---|---|---|---|---|
| A. Ngoài hàng rào | Build-up | 2 → **24** | 0 | — |
| B. Trạm gác chính | **Sustain Peak** | 2 → **24** | **2** | L16 |
| C. Sân trong | Peak Fade | 1 → **12** | 0 | — |
| D. Trạm gác phụ | Relax | 0 → 0 | 0 | — |
| **Tổng** | | **5 → 60** | **2** | **1** |

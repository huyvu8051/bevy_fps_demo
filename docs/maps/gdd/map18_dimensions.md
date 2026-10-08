# Map 18 — Dimension Spec x4 + Nhịp spawn x12 (bản nháp, chưa implement)

Bổ sung cho `map18_subterranean_lab_alpha.md`. Dùng chung thư viện kích
thước ở `map01_dimensions.md` mục 1; tái dùng footprint 48x32 (cùng cụm
Map 16-18 theo lore "cùng cơ sở").

⚠️ v2 = x2 mỗi cạnh. **Zombie thường VÀ biến thể phòng thí nghiệm đều
x12** — biến thể lab chỉ là "tái dùng model zombie thường + texture
khác" (không phải địch có AI/cơ chế riêng như Boomer/Hunter...), nên áp
dụng đúng công thức chung, khác với lính Meridian/đặc biệt ở các map khác.

---

## Khu A — Khu tiếp nhận/thay đồ bảo hộ [Build-up]

- **Footprint v1:** 16x12 → **v2: 32m x 24m**

**Nhịp spawn:**

| Đợt | Trigger | Số zombie | Vị trí |
|---|---|---|---|
| 1 | Vào khu thay đồ bảo hộ | 12 (v1: 1, x12) | Rải trong khu |

---

## Khu phòng thí nghiệm chính [SUSTAIN PEAK — mật độ CAO NHẤT TOÀN GAME]

- **Footprint v1:** 4 phòng TN (5x4/phòng) + buồng nuôi cấy (5x4) +
  hành lang trung tâm 20x3.6 → **v2: mỗi phòng 10x8, hành lang 40x7.2**
- Lồng "LAZARUS - Subject 0" (trống, cửa phá từ bên trong): giữa map,
  vị trí không đổi vai trò dù rộng hơn

**Nhịp spawn (zombie thường + biến thể lab, cả 2 đều x12):**

| Đợt | Trigger | Zombie thường | Biến thể lab | Vị trí |
|---|---|---|---|---|
| 1 | Vào khu phòng TN chính | 36 (v1: 3, x12) | 18 (v1: 1.5→ làm tròn, x12 chia 2 đợt) | Nửa đầu khu, từ các phòng TN |
| 2 | Tiến sâu qua lồng "Subject 0" | 36 (v1: 3, x12) | 18 | Nửa sau khu, dọc hành lang trung tâm |

Tổng zombie thường 72 (v1: 6, x12) + biến thể lab 36 (v1: 3, x12) = 108.

- **Item:** vẫn KHÔNG có ammo/med kit ở khu này — giữ nguyên chủ đích
  "khan hiếm có chủ đích", không đổi dù mật độ địch tăng mạnh.

---

## Hành lang thoát hiểm [Peak Fade]

- **Footprint v1:** dài 16m, rộng 3.6 (x2) → **v2: dài 32m, rộng 7.2m**
- `L18`: cuối hành lang, vị trí không đổi

**Nhịp spawn:**

| Đợt | Trigger | Số zombie | Vị trí |
|---|---|---|---|
| 1 | Ra khỏi khu phòng TN | 12 (v1: 1, x12) | Dọc hành lang, tapering |

---

## Trạm an toàn sinh học hỏng [Relax ngắn + Safe Room + Extract]

- **Footprint v1:** 16x12 → **v2: 32m x 24m**
- Ammo/med kit khan hiếm (1 mỗi loại): giữ nguyên, không x12 — nhịp
  game sau đoạn áp lực nặng.

---

## Tổng kết số liệu (v1 → v3)

| Khu vực | Tag pacing | Zombie thường (v1→v3) | Biến thể lab (v1→v3) | Ammo | Med kit | Audio log |
|---|---|---|---|---|---|---|
| A. Thay đồ bảo hộ | Build-up | 1 → **12** | 0 | 0 | 0 | — |
| B. Phòng thí nghiệm | **Sustain Peak** | 6 → **72** | 3 → **36** | 0 | 0 | — |
| C. Hành lang thoát hiểm | Peak Fade | 1 → **12** | 0 | 0 | 0 | L18 |
| D. Trạm an toàn sinh học | Relax | 0 → 0 | 0 | 1 | 1 | — |
| **Tổng** | | **8 → 96** | **3 → 36** | **1** | **1** | **1** |

**Tổng địch (132) vẫn cao nhất toàn game** sau x12 — đúng chốt GDD,
tương xứng tỉ lệ với các map khác đã scale cùng công thức.

# Map 3 — Dimension Spec x4 + Nhịp spawn x12 (bản nháp, chưa implement)

Bổ sung cho `map03_meridian_station.md`. Dùng chung thư viện kích thước
ở `map01_dimensions.md` mục 1. Các khu A-C nối tiếp tuyến tính (soát vé
→ đường hầm → ga đến), không có ràng buộc footprint chung.

⚠️ Footprint v1 là ước lượng lần đầu (chưa từng có số mét). v2 = x2 mỗi
cạnh. Zombie v3 = x12 tổng, chia theo nhịp — **ngoại lệ quan trọng: 2
Boomer-style KHÔNG x12** (đây là zombie đặc biệt, số lượng gắn với thiết
kế set-piece "sau khúc cua", nhân theo công thức chung sẽ phá vỡ nhịp
độ đã tính riêng cho loại này — xem ghi chú Khu B).

---

## Khu A — Khu soát vé & Sân ga vào [Build-up]

- **Footprint v1:** khu soát vé 10x8 + sân ga 16x10 → **v2: soát vé
  20x16, sân ga 32x20**
- Cầu thang cuốn nối 2 khu: rộng 4.8 (x2 hành lang chính), dài ~10m
  (giữ nguyên — gắn độ cao, không thuộc phạm vi x4 diện tích sàn)
- Ammo: trên sân ga, `(10, y, 6)`

**Nhịp spawn:**

| Đợt | Trigger | Số zombie | Vị trí |
|---|---|---|---|
| 1 | Vào khu soát vé | 12 (v1: 1, x12) | Trong khu soát vé |
| 2 | Xuống tới sân ga | 12 (v1: 1, x12) | Rải trên sân ga |

Tổng 24 (v1: 2, x12).

---

## Khu B — Đường hầm tối [SUSTAIN PEAK — giới thiệu Boomer-style]

- **Footprint v1:** đường hầm dài, chia 3 đoạn cua ~12m mỗi đoạn, rộng
  4m (đường ray + lối đi) → **v2: mỗi đoạn ~24m, rộng 8m** (x2 cả 2
  chiều, đường hầm giờ đủ rộng cho 2 người tránh nhau khi combat)
- Khúc cua hẹp #1, #2: giữ vai trò che khuất tầm nhìn, vị trí tại 1/3 và
  2/3 chiều dài đường hầm
- 3 điểm pickup (2 ammo + 1 med kit): xen kẽ dọc đường ray, cách đều
  ~16m (x2 từ ~8m)

**Zombie thường (nhịp spawn, x12 bình thường):**

| Đợt | Trigger | Số zombie | Vị trí |
|---|---|---|---|
| 1 | Vào đường hầm | 12 (v1: 1, x12) | Trước khúc cua #1 |
| 2 | Qua khúc cua #2 | 12 (v1: 1, x12) | Sau khúc cua #2, gần ga đến |

Tổng zombie thường: 24 (v1: 2, x12).

**Boomer-style (KHÔNG x12 — giữ nguyên 2 con):**

⚠️ Boomer là set-piece "ngay sau khúc cua, không thấy trước" — tăng số
lượng loại này phá vỡ đúng cơ chế đã thiết kế (mỗi khúc cua chỉ nên có
đúng 1 bất ngờ, nhiều Boomer cùng lúc = khói/khí độc chồng lấp không
kiểm soát được trong không gian hẹp, có thể gây bất công/không thể né).
**v3 áp dụng x12 chỉ cho zombie thường, giữ nguyên 2 Boomer** (1 sau mỗi
khúc cua, đúng bản gốc) — đây là ngoại lệ có chủ đích, không phải thiếu
sót khi tính x12.

- Boomer #1: ngay sau khúc cua #1
- Boomer #2: ngay sau khúc cua #2

---

## Khu C — Ga đến [Peak Fade → Relax]

- **Footprint v1:** đường ray dẫn ra 16x8 + sân ga đến 20x14 → **v2:
  đường ray 32x16, sân ga đến 40x28**
- Safe Room: góc sân ga đến, tâm `(-14, y, 8)`, size 12x10
- Extract Zone: `(14, 0, 8)`

**Nhịp spawn:**

| Đợt | Trigger | Số zombie | Vị trí |
|---|---|---|---|
| 1 | Ra khỏi đường hầm | 12 (v1: 1, x12) | Trên đường ray dẫn ra, tapering trước Relax |

---

## Tổng kết số liệu (v1 → v3)

| Khu vực | Tag pacing | Zombie thường (v1→v3) | Boomer (không x12) | Ammo | Med kit |
|---|---|---|---|---|---|
| A. Soát vé/sân ga vào | Build-up | 2 → **24** | 0 | 1 | 0 |
| B. Đường hầm | **Sustain Peak** | 2 → **24** | **2** | 2 | 1 |
| C. Ga đến | Peak Fade/Relax | 1 → **12** | 0 | 0 | 0 |
| **Tổng** | | **5 → 60** | **2** | **3** | **1** |

Zombie thường x12 đúng công thức (5×12=60). Boomer giữ nguyên 2 —
ngoại lệ có chủ đích cho loại đặc biệt gắn set-piece cố định, áp dụng
cho mọi map có Boomer/Tank/Witch/Hunter... về sau (xem ghi chú tương tự
ở các map khác).

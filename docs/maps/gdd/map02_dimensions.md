# Map 2 — Dimension Spec x4 + Nhịp spawn x12 (bản nháp, chưa implement)

Bổ sung cho `map02_meridian_plaza.md` (mockup ASCII + pacing). Dùng
chung **thư viện kích thước chuẩn** ở `map01_dimensions.md` mục 1 (xe,
cửa, hành lang, người...) — không định nghĩa lại. Map 2 không có ràng
buộc "cụm tầng xếp chồng" như Map 1 (các khu A-D nối tiếp trên 1 mặt
bằng, trừ khu D bãi xe) nên mỗi khu tính footprint độc lập.

⚠️ Map này **chưa từng có số mét cụ thể** (chỉ ASCII mockup) — footprint
v1 dưới đây là **ước lượng hợp lý lần đầu** (không suy ngược từ code),
dùng mức chính xác blockout/làm tròn giống Map 1. v2 = x2 mỗi cạnh
(diện tích x4). Zombie v3 = x12 tổng (x4 diện tích x x3 tăng cường độ),
chia theo nhịp spawn (wave) từng khu, đúng pattern đã áp cho Map 1.

---

## Khu A — Lối vào quảng trường [Build-up]

- **Footprint v1 (ước lượng):** 20m x 16m → **v2: 40m x 32m** (half 20x16)
- Cửa hàng tiện lợi: v1 6x6 → **v2: 12x12**, đặt góc Đông Bắc, tâm `(12, y, -8)`
- Player spawn: `x=-3,-1,1,3`, `z=-14` (giữ pattern spacing 2.0/người
  như Map 1)
- Ammo (kệ hàng): trong cửa hàng, `(12, y, -8)`

**Nhịp spawn:**

| Đợt | Trigger | Số zombie | Vị trí |
|---|---|---|---|
| 1 | Mission start | 12 (v1: 1, x12) | Rải dọc đường phố dẫn vào, `x∈[-16,16], z∈[-4,4]` |

---

## Khu B — Quảng trường trung tâm [SUSTAIN PEAK]

- **Footprint v1 (ước lượng):** quảng trường mở 30x24 + 5 ô cửa hàng
  quanh rìa (6x6 mỗi ô) → **v2: quảng trường 60m x 48m (half 30x24),
  cửa hàng 12x12 mỗi ô**
- 5 cửa hàng quanh rìa quảng trường (quần áo, đồ điện tử, giày dép, đồ
  ăn nhanh + 1 dự phòng), đặt đối xứng quanh chu vi, cách tâm ~24m
- `X` trigger (vùng vô hình, phục kích đồng loạt): tâm quảng trường `(0,
  y, 0)`, bán kính kích hoạt ~10m (v2, x2 từ ~5m)
- `L2` audio log: trong cửa hàng đồ điện tử

**Nhịp spawn (2 đợt — ambush đồng loạt vẫn giữ tinh thần "phục kích vị
trí", nhưng thêm đợt tăng viện để đạt x12 mà không đông nghẹt cùng 1
lúc):**

| Đợt | Trigger | Số zombie | Vị trí |
|---|---|---|---|
| 1 | `X` kích hoạt (đồng loạt, không báo trước) | 78 (v1: ~7, x12 làm tròn) | Ập ra từ MỌI cửa hàng quanh quảng trường cùng lúc |
| 2 | +8s sau đợt 1 | 78 | Tăng viện tràn từ 3 hướng Tây/Nam/Đông (đúng bố cục gốc), giữ "sustain" không dứt sau cú ambush đầu |

Tổng 78+78 = 156 (v1: ~13, x12). Đợt 1 vẫn giữ đúng bản chất "phục kích
không báo trước" của thiết kế gốc — đợt 2 là lớp mới thêm ở v3 để kéo
dài áp lực thay vì kết thúc ngay sau ambush.

---

## Khu C — Khu rào chắn (Nhóm sống sót trước) [Peak Fade]

- **Footprint v1 (ước lượng):** 14m x 10m → **v2: 28m x 20m** (half 14x10)
- Rào chắn tạm: dọc cạnh Bắc, `z=-8`
- `A2` (vũ khí thứ 2): giữa khu vực rào chắn, `(0, y, -2)`
- Xác nhóm sống sót (deco): rải quanh rào chắn

**Nhịp spawn:**

| Đợt | Trigger | Số zombie | Vị trí |
|---|---|---|---|
| 1 | Vào khu rào chắn | 12 (v1: 1, x12) | Rải quanh rào chắn, `x∈[-12,12], z∈[-6,6]` |

---

## Khu D — Bãi đỗ xe nhiều tầng [Relax + Safe Room + Extract]

- **Footprint v1 (ước lượng):** 30m x 20m mỗi tầng → **v2: 60m x 40m**
  (half 30x20)
- Safe Room: tâm `(-20, y, -14)`, size 12x10 (x2 từ ước lượng 6x5)
- Extract Zone: chân dốc xuống, `(20, 0, -14)`, trigger 8x8

- **Không zombie** — giữ nguyên `[Relax]`, không x12 (0x12=0).

---

## Tổng kết số liệu (v1 → v3)

| Khu vực | Tag pacing | Zombie (v1 → v3) | Số đợt | Ammo | Vũ khí | Audio log |
|---|---|---|---|---|---|---|
| A. Lối vào | Build-up | 1 → **12** | 1 | 1 | 0 | — |
| B. Quảng trường | **Sustain Peak** | ~13 → **156** | 2 | 0 | 0 | L2 |
| C. Rào chắn | Peak Fade | 1 → **12** | 1 | 0 | 1 (A2) | — |
| D. Bãi đỗ xe | Relax | 0 → 0 | — | 0 | 0 | — |
| **Tổng** | | **~15 → 180 zombie** | 4 đợt | **1** | **1** | **1** |

Đúng x12 (~15×12=180). Ammo/vũ khí/audio log giữ nguyên số lượng — nhịp
game, không phụ thuộc diện tích/số zombie.

# Map 4 — Dimension Spec x4 + Nhịp spawn x12 (bản nháp, chưa implement)

Bổ sung cho `map04_route9_overpass.md`. Dùng chung thư viện kích thước ở
`map01_dimensions.md` mục 1 (thêm xe cộ kẹt xe — đã có sẵn trong thư
viện). Khu A-D nối tiếp tuyến tính dọc cây cầu.

⚠️ Footprint v1 là ước lượng lần đầu. v2 = x2 mỗi cạnh. Zombie thường
x12; **zombie "ẩn" (giả chết dưới gầm xe) KHÔNG x12** — số lượng gắn
chặt với số xe làm chỗ ẩn nấp cụ thể trong bố cục kẹt xe, tăng theo công
thức chung sẽ đòi thêm xe mới không có trong thiết kế gốc. Giữ nguyên 3
con, chỉ thêm zombie lộ sẵn thường theo x12.

---

## Khu A — Đầu cầu (dốc lên) [Build-up]

- **Footprint v1:** dốc lên dài 20m, rộng 8m → **v2: dài 40m, rộng 16m**
- Xe cộ thưa 2 bên: giữ nguyên số lượng ít (deco, không phải cover
  chính) nhưng rải xa hơn theo chiều dài x2

**Nhịp spawn:**

| Đợt | Trigger | Số zombie | Vị trí |
|---|---|---|---|
| 1 | Lên dốc | 12 (v1: 1, x12) | Rải dọc dốc, `z∈[-20,0]` |

---

## Khu B — Giữa cầu, kẹt xe dày đặc [SUSTAIN PEAK]

- **Footprint v1:** đoạn cầu kẹt xe dài 30m, rộng 12m (2 làn + lề) →
  **v2: dài 60m, rộng 24m**
- Hàng xe kẹt: mật độ xe giữ nguyên tỉ lệ (dùng thư viện xe mục 1 map01,
  đa dạng loại), rải dày hơn theo chiều dài x2 — vẫn "obstacle course
  liên tục", không thưa ra
- Xe cứu thương (ammo+med kit): cuối đoạn, `(0, y, 20)`
- Xe quân sự Meridian lật (deco): giữa đoạn, `(-8, y, 4)`

**Zombie lộ sẵn (nhịp spawn, x12):**

| Đợt | Trigger | Số zombie | Vị trí |
|---|---|---|---|
| 1 | Vào đầu đoạn kẹt xe | 18 (v1: ~1.5→ làm tròn, x12 của 3 lộ sẵn = 36, chia 2 đợt) | Rải giữa hàng xe, nửa đầu đoạn |
| 2 | Giữa đoạn kẹt xe | 18 | Rải giữa hàng xe, nửa sau đoạn |

Tổng zombie lộ sẵn: 36 (v1: 3, x12).

**Zombie "ẩn" giả chết (KHÔNG x12 — giữ nguyên 3 con):**

⚠️ Mỗi con "ẩn" gắn với 1 gầm xe cụ thể trong bố cục — tăng theo x12 đòi
phải thêm xe mới không có trong ASCII gốc. Giữ nguyên 3 con, rải đều
dọc đoạn kẹt xe (nay dài gấp đôi) để mật độ jump-scare không đổi:
`z = -15, 0, 15` (thay vì cụm gần nhau như bản gốc ngắn hơn).

---

## Khu C — Đoạn cầu thoáng cuối [Peak Fade — Sightline dài]

- **Footprint v1:** 24m x 10m (mặt cầu thoáng) → **v2: 48m x 20m** — vẫn
  giữ **sightline dài nhất Campaign 1**, x2 càng làm nổi bật đặc điểm
  này (tầm nhìn xa hơn nữa)
- Vị trí sniper: cuối đoạn, `(0, y, 20)`

**Nhịp spawn:**

| Đợt | Trigger | Số zombie | Vị trí |
|---|---|---|---|
| 1 | Vào đoạn thoáng | 12 (v1: 1, x12) | Xa, cuối tầm nhìn — vẫn là "mục tiêu minh hoạ tầm bắn", rải dọc `z∈[10,30]` để tận dụng sightline dài hơn |

---

## Khu D — Cuối cầu, gần điểm hẹn [Relax + Safe Room + Extract]

- **Footprint v1:** 20m x 16m → **v2: 40m x 32m**
- Safe Room + Extract Zone: cuối khu, không zombie

---

## Tổng kết số liệu (v1 → v3)

| Khu vực | Tag pacing | Zombie lộ (v1→v3) | Zombie ẩn (không x12) | Ammo | Med kit | Vũ khí |
|---|---|---|---|---|---|---|
| A. Đầu cầu | Build-up | 1 → **12** | 0 | 0 | 0 | 0 |
| B. Giữa cầu | **Sustain Peak** | 3 → **36** | **3** | 1 | 1 | 0 |
| C. Cuối cầu thoáng | Peak Fade | 1 → **12** | 0 | 0 | 0 | 1 (sniper) |
| D. Cuối cầu/Relax | Relax | 0 → 0 | 0 | 0 | 0 | 0 |
| **Tổng** | | **5 → 60** | **3** | **1** | **1** | **1** |

Zombie lộ sẵn x12 đúng công thức. Zombie ẩn giữ nguyên (ngoại lệ set-piece,
giống Boomer ở Map 3).

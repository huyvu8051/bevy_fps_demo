# Map 5 — Dimension Spec x4 + Nhịp spawn x12 (bản nháp, chưa implement) — Boss Map

Bổ sung cho `map05_terminus_overlook.md`. Dùng chung thư viện kích thước
ở `map01_dimensions.md` mục 1. Map ngắn, 2 khu chính + kết thúc.

⚠️ Footprint v1 là ước lượng lần đầu. v2 = x2 mỗi cạnh. Zombie thường
x12. **Boss (Tank-style) KHÔNG x12** — giữ nguyên 1 boss, đúng nguyên
tắc đã lập ở Map 3/4 (địch đặc biệt/boss không nhân theo công thức số
lượng chung, vì độ khó của chúng đến từ chỉ số/cơ chế riêng, không phải
số lượng).

---

## Tầng 1-2 Gara (leo lên) [Build-up]

- **Footprint v1:** mỗi tầng gara 16x12 → **v2: mỗi tầng 32m x 24m**
  (half 16x12, cùng footprint 2 tầng như quy tắc Map 1)
- Ammo (tầng 1): `(10, y, 6)`
- Med kit (tầng 2): `(10, y, 6)` (cùng vị trí tương đối, tầng trên)

**Nhịp spawn:**

| Đợt | Trigger | Số zombie | Vị trí |
|---|---|---|---|
| 1 | Vào tầng 1 | 18 (v1: 2, x12 làm tròn từ 24, chia đều 2 tầng) | Rải tầng 1 |
| 2 | Lên tầng 2 | 18 | Rải tầng 2 |

Tổng 36 (v1: 3, x12).

---

## Bãi đáp trực thăng — BOSS ARENA [SUSTAIN PEAK = Boss Fight]

- **Footprint v1:** 24m x 24m (vuông, đối xứng cho co-op 4 người) →
  **v2: 48m x 48m** (half 24x24) — không gian rộng hơn nhiều giúp
  né đòn boss thoải mái hơn cho 4 người cùng lúc
- 4 cột bê tông (vật cản né đòn): đối xứng quanh tâm, cách tâm ~16m
  (x2 từ ~8m, giữ đối xứng 4 góc)
- Boss (Tank-style): xuất hiện tâm arena `(0, y, 0)` — **giữ nguyên 1
  con**, không x12

**Không zombie thường** ở khu này — giữ nguyên chủ đích GDD "không pha
trộn zombie thường vào boss fight, giữ fight rõ ràng cho co-op". Diện
tích rộng hơn không đổi quyết định thiết kế này.

---

## Kết thúc Campaign 1 [RELAX — Ending]

- **Footprint v1:** khu vực nhỏ quanh Extract, 12x10 → **v2: 24m x 20m**
- Không zombie, không thay đổi khác ngoài diện tích.

---

## Tổng kết số liệu (v1 → v3)

| Khu vực | Tag pacing | Zombie (v1→v3) | Boss (không x12) | Ammo | Med kit |
|---|---|---|---|---|---|
| Gara tầng 1-2 | Build-up | 3 → **36** | 0 | 1 | 1 |
| Bãi đáp | **Sustain Peak (Boss)** | 0 → 0 | **1** | 0 | 0 |
| Kết thúc | Relax | 0 → 0 | 0 | 0 | 0 |
| **Tổng** | | **3 → 36** | **1 boss** | **1** | **1** |

---

## So sánh tổng thể Campaign 1 (Map 1-5, sau x4 diện tích + x12 zombie thường)

| Map | Zombie thường (v1→v3) | Đặc biệt/Boss (không x12) | Ammo | Med kit | Vũ khí mới |
|---|---|---|---|---|---|
| 1. Highrise | 19 → **228** | 0 | 3 | 1 | 0 |
| 2. Meridian Plaza | ~15 → **180** | 0 | 1 | 0 | 1 |
| 3. Meridian Station | 5 → **60** | 2 (Boomer) | 3 | 1 | 0 |
| 4. Route 9 Overpass | 5 → **60** | 3 (ẩn) | 1 | 1 | 1 |
| 5. Terminus Overlook | 3 → **36** | 1 (Tank) | 1 | 1 | 0 |
| **Tổng Campaign 1** | **47 → 564** | **6** | **9** | **4** | **2** |

Đúng x12 zombie thường (47×12=564, lệch nhẹ do làm tròn từng map — chấp
nhận được ở mức blockout). Zombie đặc biệt/boss/ẩn **giữ nguyên tuyệt
đối** qua mọi map — nguyên tắc thống nhất áp dụng xuyên suốt Campaign
2-4 khi làm tiếp: chỉ x12 zombie thường, không đụng vào loại đặc biệt.

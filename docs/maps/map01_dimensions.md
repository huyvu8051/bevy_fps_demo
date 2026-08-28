# Map 1 — Dimension Spec (bản nháp, chưa implement)

Bổ sung cho `map01_highrise.md` (mockup ASCII + pacing) — file đó mô tả
**cái gì** ở đâu, file này quyết **to bao nhiêu mét, ở toạ độ nào**.

⚠️ **Không dựa trên số đã code** (`crates/shared/src/map1/*.rs`) — số ở đó
là số đặt tạm lúc code B1, chưa chắc đúng. Nếu số ở đây lệch code hiện
tại → **code sai, phải sửa code**, không phải ngược lại.

## Phương pháp (áp dụng cho mọi map, không riêng Map 1)

```
1. Chốt thư viện item chuẩn (tra kích thước thực tế — mục 1)
        ↓
2. Với TỪNG khu vực trong mockup: liệt kê mọi item xuất hiện + số lượng
   + kích thước + khoảng đệm đi lại/combat cần thiết (mục 3, bảng "kiểm
   kê" đầu mỗi khu)
        ↓
3. Cộng dồn kiểm kê → ra kích thước khu vực đó (không đoán)
        ↓
4. Khu vực nào đông item nhất trong 1 CỤM TẦNG XẾP CHỒNG (tầng nào cũng
   phải thẳng hàng tường ngoài + trục cầu thang) → khu đó quyết định
   footprint CHUNG cho cả cụm tầng. Các tầng còn lại trong cụm chỉ cần
   kiểm tra "có vừa trong footprint chung không", không tự suy ra kích
   thước riêng (sai kiến trúc nếu mỗi tầng 1 kích thước khác nhau).
        ↓
5. Đặt toạ độ từng item trong footprint đã chốt.
```

**2 loại item — chỉ loại (a) cần liệt kê tay:**

- **(a) Item đặt tay** (furniture, pickup, zombie/player spawn, trigger,
  deco): vị trí là quyết định thiết kế độc lập, không suy ra được từ
  đâu khác → phải liệt kê + gán toạ độ tường minh (mục 3).
- **(b) Item dẫn xuất** (tường, sàn, trần): vị trí là hệ quả tất định
  của phòng/tầng chứa nó — 1 phòng đã có `(center, half_size)` thì 4
  bức tường quanh chỉ còn đúng 1 cách đặt, không có lựa chọn thứ 2.
  **Không liệt kê tay** — sinh tự động từ `(center, half_size)` bằng
  hàm dùng chung (đúng cách code hiện tại đã làm:
  `FloorLevel::walls_ground_floor()` cho viền cả tầng,
  `small_room_walls(center, half_size, ...)` cho phòng nhỏ bên trong).
  Điều kiện duy nhất: mọi phòng/tầng trong mục 3 phải có đủ
  `center + half_size` tường minh (đã có).

Mức chính xác: **blockout game**, không phải bản vẽ kiến trúc — số làm
tròn dễ nhớ là đủ.

---

## 1. Thư viện kích thước chuẩn (dùng chung mọi map)

| Nhóm | Item | Kích thước (Vec3: rộng X, cao Y, sâu Z) | Cơ sở |
|---|---|---|---|
| Player | Capsule | radius 0.35, length 0.9 (đã code, giữ nguyên) | `PLAYER_RADIUS`/`PLAYER_LENGTH` |
| Player | Khoảng đứng chờ spawn | 1.0 x — x 1.0 /người | đủ không đè lên nhau |
| Xe | Sedan (gara) | (1.9, 1.5, 4.6) | sedan thật ~1.8m ngang, ~1.45m cao, ~4.5m dài |
| Xe | 1 ô đậu xe (kể cả biên) | 2.5 x — x 5.0 | tiêu chuẩn VN/US |
| Xe | Aisle xe ra vào (2 chiều) | rộng 6.0 | đủ xe lùi/quay |
| Nội thất | Bàn làm việc đơn | (1.4, 0.75, 0.7) | bàn có màn hình |
| Nội thất | 1 ô cubicle (bàn+vách+lối vào) | (1.6, 1.2, 1.6) | vách cao ngang ngực |
| Nội thất | Lối đi trước dãy cubicle | rộng 2.0 | đủ 1 người + né đạn |
| Nội thất | Quầy lễ tân | (2.2, 1.1, 0.6) | quầy cao, cover đứng |
| Nội thất | Tủ đồ/locker | (0.4, 1.8, 0.5) | 1 ô đơn |
| Kiến trúc | Hành lang chính | rộng 2.4 | 2 người tránh nhau lúc combat |
| Kiến trúc | Hành lang phụ | rộng 1.8 | |
| Kiến trúc | Cửa đơn | rộng 0.9, cao 2.1 | |
| Kiến trúc | Cửa đôi/kính lớn | rộng 1.8, cao 2.4 | phòng họp, sảnh |
| Kiến trúc | Cầu thang xoắn ốc (trục chính) | đường kính 4.0 (cần ô 4x4 clearance) | 1 người đi, chật vừa đủ tạo áp lực |
| Kiến trúc | Tường ngoài (kết cấu, có collider) | dày 1.0 | `ARENA_WALL_THICKNESS` — hằng số toàn cục, giữ nguyên |
| Kiến trúc | Tường/vách nội thất (panel) | dày 0.15 | `LevelPanel` |
| Kiến trúc | Trần | Gara 2.6 / Văn phòng 2.8 / Sảnh chính (thông tầng) 6.0 | |
| Kiến trúc | Tầng-tầng | 3.0 | `FLOOR_HEIGHT` — hằng số toàn cục, giữ nguyên |

---

## 2. Cụm tầng xếp chồng & footprint chung

**G, F2, F3, F4, Mái** thuộc 1 cụm xếp chồng (cùng trục cầu thang xoắn
ốc, cùng viền tường ngoài) → **bắt buộc chung 1 footprint**. B1 là hầm,
không bị ràng buộc thẳng hàng với các tầng trên → tính riêng.

**F2 là tầng đông item nhất trong cụm** (cubicle + 2 phòng riêng + cầu
thang) → dùng F2 để suy ra footprint chung (mục 3.3). Các tầng G/F3/F4/
Mái áp dụng lại đúng footprint đó, chỉ tự kiểm tra "có vừa không" khi bố
trí nội dung riêng.

---

## 3. Chi tiết từng tầng

### 3.1 B1 — Gara (Build-up, 3 zombie, 1 ammo) — tính riêng

Bố cục: **1 hàng 8 xe** cho mỗi khu A/B (không phải lưới 4x4 — xếp 1
hàng dài đông hơn tạo footprint vuông vắn thay vì dài hẹp).

**Kiểm kê Khu A (spawn + hàng xe 1):**

| Item | SL | Kích thước | Đệm | Chiếm dụng |
|---|---|---|---|---|
| Player spawn | 4 | điểm | 1.0/người, xếp hàng ngang | 4.0m (X) x 2.0m (Z) |
| Ô đậu xe (1 hàng) | 8 | 2.5 x 5.0 | liền kề, không đệm giữa các ô | 20.0m (X) x 5.0m (Z) |
| Lề an toàn 2 bên tường | — | — | 1.0m mỗi bên | +2.0m (X) |
| Zombie spawn | 1 | điểm | nằm trong hàng xe | — |

→ **Khu A = 22.0m (X) x 7.0m (Z)** (20+2 ngang; spawn 2.0 + xe 5.0 sâu)

**Aisle nối Khu A/B:** rộng 6.0m (Z) x 22.0m (X) — đúng chuẩn mục 1.

**Kiểm kê Khu B (hàng xe 2, sâu/tối hơn):**

| Item | SL | Kích thước | Đệm | Chiếm dụng |
|---|---|---|---|---|
| Ô đậu xe (1 hàng) | 8 | 2.5 x 5.0 | liền kề | 20.0m (X) x 5.0m (Z) |
| Tủ đồ bảo vệ (ammo) | 1 | 0.4 x 0.5 | góc, cần 1.0m clearance | nằm trong margin |
| Zombie spawn | 2 | điểm | rải trong hàng xe | — |
| Margin sau (tủ đồ + xác deco) | — | — | 1.0m | +1.0m (Z) |

→ **Khu B = 22.0m (X) x 6.0m (Z)**

**Tổng B1 (chưa tính dốc):** X = 22.0m; Z = 7.0 + 6.0 (aisle) + 6.0 =
19.0m. Cộng dốc lên sảnh G (rộng 2.4m hành lang chính, nhô thêm ở cạnh
Bắc) → làm tròn bao trọn thành **22m x 22m**.

→ **B1 half-size: X=11, Z=11** — trần 2.6m.

⚠️ **Khớp biên với G (đã sửa):** G có footprint 24x16 (half Z=8), cạnh
Bắc tại `z=-8`. Nếu đặt B1 tâm (0,0) như bản trước, cạnh Bắc B1 ra tới
`z=-11` — dốc đi lố ra ngoài biên G tới 3m, không khớp. **Fix: dời tâm
B1 theo Z sao cho cạnh Bắc B1 trùng đúng cạnh Bắc G** (`z=-8`, cùng 1
đường tường ngoài toà nhà phía trên) — B1 rộng hơn nên phần dư nằm hết
về phía Nam (hợp lý: hầm to hơn, ăn sâu xuống dưới thêm, không mở rộng
về phía mặt tiền).

→ **B1: north_wall_z = -8, half_x = 11, half_z = 11** (kiểu tham số
giống `FloorLevel::new` hiện có) → span thực tế: `x ∈ [-11,11]`,
`z ∈ [-8, 14]`.

**Toạ độ (đã dời theo Z, +3 so với bản gốc tâm (0,0)):**
- Cột X 8 ô xe (bước 2.5, đối xứng quanh 0 — trục X không đổi): `-8.75,
  -6.25, -3.75, -1.25, 1.25, 3.75, 6.25, 8.75`
- Player spawn (z=-6, 4 điểm cách 1m): `x = -1.5, -0.5, 0.5, 1.5`
- Hàng xe Khu A: `z = -2.5`
- Zombie khu A: `(0, y, -2.5)`
- Aisle: `z ∈ [0, 6]`
- Hàng xe Khu B: `z = 8.5`
- Zombie khu B: `(-5, y, 8.5)` và `(5, y, 8.5)`
- Tủ đồ (ammo): `(9, y, 11.5)`
- Deco xác bảo vệ: `(-9, y, 11.5)`
- Dốc lên G: cạnh Bắc `z = -8` (khớp đúng cạnh Bắc G), tâm `x = 0`,
  rộng 2.4m, dài ~9.4m (leo 3.0m cao, góc ~18°) — đỉnh dốc nối thẳng
  vào sàn G ngay tại đường biên chung.

---

### 3.2 F2 — Văn phòng tầng 2 — tầng quyết định footprint chung

**Kiểm kê:**

| Item | SL | Kích thước | Đệm | Chiếm dụng |
|---|---|---|---|---|
| Cubicle (1 hàng "C C C C C") | 5 | 1.6 x 1.6 | liền kề | 8.0m (X) x 1.6m (Z) |
| Lối đi trước hàng cubicle | 1 | — | rộng 2.0 | +2.0m (Z) |
| Lề 2 bên hàng cubicle | — | — | 1.0m mỗi bên | +2.0m (X) |
| Văn phòng riêng (cửa kính) | 1 phòng | 4.0 x 3.0 | — | 4.0m (X) x 3.0m (Z) |
| Phòng tủ đồ nhân viên | 1 phòng | 3.0 x 3.0 | gap 1.0 với phòng trên | 3.0m (X) x 4.0m (Z, gồm gap) |
| Cầu thang xoắn ốc | 1 | dia 4.0 | clearance quanh 1.0 | 6.0m (X) x 6.0m (Z) |
| Hành lang chính nối cubicle↔phòng | 1 | rộng 2.4 | — | +2.4m (X) |
| Hành lang phụ nối phòng↔cầu thang | 1 | rộng 1.8 | — | +1.8m (X) |
| Med kit | 1 | điểm | cạnh cầu thang | — |
| Zombie spawn | 2 | điểm | trong khu cubicle | — |
| `L1` audio log | 1 | điểm | trong văn phòng riêng | — |
| Ammo | 1 | điểm | trong phòng tủ đồ | — |

**Cộng theo trục X** (dãy song song: cubicle | hành lang chính | phòng
riêng/tủ đồ | hành lang phụ | cầu thang):
`10.0 (cubicle+lề) + 2.4 + 4.0 (phòng, bề rộng lớn nhất trong 2 phòng) +
1.8 + 6.0 = 24.2m ≈ 24.0m`

**Cộng theo trục Z** (chiều sâu lớn nhất trong các nhánh): khu cubicle
`1.6+2.0=3.6m`, cụm 2 phòng xếp dọc `3.0+1.0+3.0=7.0m`, cụm cầu thang
`6.0m` → lấy max + lề trên/dưới `7.0 + 2x4.5 (không gian đi lại chung
quanh, làm tròn) ≈ 16.0m`.

→ **Footprint chung cả cụm (G/F2/F3/F4/Mái): 24m x 16m, half-size 12 x
8**, trần F2 = 2.8m.

**Toạ độ F2** (tâm (0, `FLOOR_HEIGHT*1`)):
- Hàng cubicle: `x = -10.5, -8.5, -6.5, -4.5, -2.5`, `z = -5`
- Lối đi trước cubicle: `z ∈ [-3, -1]`
- Zombie: `(-8.5, y, -5)` và `(-4.5, y, -3)`
- Văn phòng riêng: tâm `(6, y, -5)`, kích thước 4x3
  - Bàn làm việc (1.4x0.7), áp tường Bắc, tâm `(6, y, -6.15)` (lùi vào
    trong phòng đúng nửa chiều sâu bàn tính từ mép tường trong
    `z=-6.5`)
  - `L1` (đặt trên bàn): `(6, y, -6.15)`
  - Cửa kính (1.8m, mở về hành lang phụ phía Nam): tâm `(6, y, -3.5)`
- Phòng tủ đồ nhân viên: tâm `(6, y, 0)`, kích thước 3x3
  - Locker (0.4x1.8x0.5), 4 cái áp tường Tây, xếp cạnh nhau: tâm
    `x = 4.7`, `z = -1.1, -0.5, 0.1, 0.7` (bước 0.6m, tường trong tại
    `x=4.5`)
  - Ammo (giữa phòng, trước dãy locker): `(6, y, 0.5)`
  - Cửa đơn (0.9m, mở về hành lang phụ phía Bắc): tâm `(6, y, -1.5)`
- Cầu thang xoắn ốc (cố định mọi tầng trong cụm): tâm `(8, —, -5)`
- Med kit: `(6.5, y, -3)` (giữa phòng tủ đồ và cầu thang)

---

### 3.3 G — Sảnh chính (Build-up→Sustain, 2 zombie, 1 ammo) — dùng lại footprint 24x16

Tầng này "không gian mở, trần cao" theo chủ đích thiết kế — không tự
suy footprint từ item (ít item), mà **kiểm tra item có vừa footprint đã
chốt (24x16) không**:

| Item | Kích thước | Vừa trong 24x16? |
|---|---|---|
| Quầy lễ tân | 2.2 x 0.6, cần 3m clearance trước quầy | ✅, đặt góc Tây Nam |
| Cầu thang xoắn ốc | dia 4.0 | ✅, đúng vị trí cố định `(8,-5)` |
| 2 zombie spawn | điểm | ✅, giữa sảnh còn dư nhiều |

→ **Footprint: 24x16 (half 12x8), tâm (0,0), trần 6.0m** (thông tầng).

**Toạ độ:**
- Quầy lễ tân: `(-8, 0.55, 5)`, mặt hướng vào giữa sảnh
- Ammo (sau quầy): `(-8, y, 5.5)`
- Cầu thang xoắn ốc: `(8, —, -5)` (giống F2)
- Zombie: `(-2, y, 0)` và `(3, y, -2)`
- Mảng kính vỡ (deco): `(0, y, -1)`
- Biển "ĐIỂM TẬP KẾT" (deco): `(2, y, 2)`

---

### 3.4 F3 — Văn phòng tầng 3 (SUSTAIN PEAK, ~10 zombie) — dùng lại footprint 24x16

**Kiểm kê (kiểm tra vừa footprint, không tự suy):**

| Item | Kích thước | Vừa 24x16? |
|---|---|---|
| Phòng họp kính | 5.0 x 4.0 | ✅ góc Tây Bắc |
| Hành lang chính (nối 2 văn phòng) | rộng 2.4, dài 12.0 | ✅ chạy giữa |
| Văn phòng A | 4.0 x 4.0 | ✅ |
| Văn phòng B | 4.0 x 4.0 | ✅ |
| Cầu thang xoắn ốc | dia 4.0 | ✅ cố định `(8,-5)` |
| Zombie (3 đợt: 3+2+3=~10) | điểm | ✅ rải dọc hành lang + 2 văn phòng |

→ **Footprint: 24x16 (giữ nguyên), tâm (0, `FLOOR_HEIGHT*2`), trần
2.8m.**

**Toạ độ:**
- Phòng họp kính: tâm `(-8, y, -4)`, size 5x4 (span `x[-10.5,-5.5],
  z[-6,-2]`); trigger `X`: `(-8, y, -4)`
  - Cửa kính đôi (1.8m, mở về hành lang phía Nam): tâm `(-8, y, -2)`
- Hành lang: `x ∈ [-6,6], z ∈ [-1,1]`
- Nhóm zombie 1 (từ phòng họp, 3 con): quanh `(-8, y, -2)`
- Nhóm zombie 2 (giữa hành lang, 2 con): quanh `(0, y, 0)`
- Nhóm zombie 3 (từ văn phòng B, 3 con): quanh `(7, y, 3)`
- Văn phòng A: tâm `(-7, y, 4)`, size 4x4 (span `x[-9,-5], z[2,6]`), 1
  zombie tại tâm
  - Cửa đơn (0.9m, mở về hành lang phía Bắc): tâm `(-7, y, 2)`
- Văn phòng B: tâm `(8, y, 4)`, size 4x4 (span `x[6,10], z[2,6]`), 1
  zombie tại tâm
  - Cửa đơn (0.9m, mở về hành lang phía Bắc): tâm `(8, y, 2)`
- Cầu thang xoắn ốc: `(8, —, -5)`

---

### 3.5 F4 — Văn phòng tầng 4 (Peak Fade, 2 zombie, chưa code) — dùng lại footprint 24x16

| Item | Kích thước | Vừa 24x16? |
|---|---|---|
| Phòng lưu trữ tài liệu (trống) | 5.0 x 4.0 | ✅ |
| Hành lang ngắn ra cửa thoát hiểm | rộng 2.4, dài 5.0 | ✅ cạnh Đông |
| Cửa thoát hiểm | 1.8 x 2.4 | ✅ |
| Cầu thang bộ phụ lên Mái (mới, xem ghi chú dưới) | 1.2 x 3.0 | ✅ cạnh cầu thang xoắn ốc |
| Zombie (lẻ tẻ, không đợt mới) | điểm | ✅ |

→ **Footprint: 24x16, tâm (0, `FLOOR_HEIGHT*3`), trần 2.8m.**

⚠️ **Nối lên Mái (thiếu ở bản trước):** cầu thang xoắn ốc dừng hẳn ở F4
(đã ghi rõ trong mockup gốc) nhưng mockup vẫn có `↑ Lên Mái` — cần 1
cầu thang bộ riêng (không xoắn ốc, kiểu cầu thang thoát hiểm nội bộ,
thẳng 1 đợt) đặt cạnh cụm cầu thang xoắn ốc (không phải cùng vị trí).

**Toạ độ:**
- Phòng lưu trữ: tâm `(-6, y, 3)`, size 5x4 (span `x[-8.5,-3.5],
  z[1,5]`)
  - Cửa đơn (0.9m, mở về phía Đông ra khu trung tâm): tâm `(-3.5, y, 3)`
- Hành lang ra cửa thoát hiểm: `x ∈ [6,11], z ∈ [-1,1]`
- Cửa thoát hiểm: `(11.5, y, 0)`
- Zombie: `(-3, y, -3)` và `(3, y, 2)`
- Cầu thang xoắn ốc (dừng ở đây, không lên tiếp): `(8, —, -5)`
- **Cầu thang bộ phụ lên Mái:** tâm `(3, y, -5)`, kích thước 1.2 x 3.0
  (chạy dọc trục Z) — đặt sát phía Tây cụm cầu thang xoắn ốc (cách 3m,
  không đụng), miệng cầu thang trên Mái tại đúng toạ độ `(3, —, -5)`
  (xem mục 3.6).

---

### 3.6 Mái nhà & Lối thoát hiểm (Relax + SR + Extract, chưa code) — dùng lại footprint 24x16

| Item | Kích thước | Vừa 24x16? |
|---|---|---|
| Safe Room (armory + bàn attachment) | 6.0 x 5.0 | ✅ |
| Miệng cầu thang bộ phụ lên từ F4 (mới) | 1.2 x 3.0 | ✅ ngay cạnh Safe Room |
| Thang cứu hoả ngoài trời | rộng 1.0, cao suốt F4→đất | nhô ra ngoài viền, không tính vào footprint mái |
| Extract Zone (chân thang, mặt đất) | 4.0 x 4.0 | riêng, ở cao độ 0 |

→ **Footprint mái: 24x16, tâm (0, `FLOOR_HEIGHT*4`), ngoài trời (không
trần).**

**Toạ độ:**
- Safe Room: tâm `(0, y, -4)`, size 6x5 (span `x[-3,3], z[-6.5,-1.5]`)
  - Cửa vào (1.8m, mở về phía Đông — đúng hướng người chơi vừa lên từ
    cầu thang phụ): tâm `(3, y, -4)`
- Miệng cầu thang bộ phụ lên từ F4 (đúng toạ độ mục 3.5): `(3, —, -5)`
  — trồi lên ngay sát cửa Safe Room, đúng luồng "lên cầu thang → thấy
  Safe Room ngay trước mặt".
- Thang cứu hoả: gắn cạnh Đông toà nhà, `x = 12`, chạy dọc suốt chiều
  cao xuống `y=0`
- Extract Zone: chân thang, mặt đất `(12, 0, 0)`, vùng trigger 4x4

---

## 4. Navmesh bounds (dẫn xuất — không cần quyết định số mới)

Các map khác trong code đã có `NAVMESH_BOUNDS`/`NAVMESH_COLUMN_OBSTACLES`
cho bot đi lại né vật cản; Map 1 hiện B1/G/F2/F3 **chưa có**, và F4/Mái
sắp code mới càng cần ngay từ đầu. Cả 2 đều **dẫn xuất thẳng từ số đã
chốt ở mục 3**, không phải quyết định thiết kế mới:

- `NAVMESH_BOUNDS` mỗi tầng = khối AABB từ `(tâm - half_size, elevation
  y)` đến `(tâm + half_size, elevation y + chiều cao trần)` — dùng
  đúng `half_size`/`elevation`/`trần` đã có ở mục 3 cho từng tầng.
- `NAVMESH_COLUMN_OBSTACLES` = danh sách (x,z) + bán kính của mọi item
  "đặt tay" đã chặn đường đi (xe, bàn, cubicle, phòng kín, quầy lễ tân,
  locker...) — lấy thẳng từ toạ độ mục 3, không cần số mới, chỉ cần khi
  code phải nhớ liệt kê đủ (dễ sót phòng kín/cubicle nếu quên).

→ Việc cần làm lúc code: viết hàm sinh 2 thứ trên từ đúng data mục 3
(tương tự cách map khác đã làm), không phải tự nghĩ số navmesh riêng.

---

## 5. Việc còn lại trước khi code

1. Bạn duyệt/chỉnh số ở mục 1-3 — đặc biệt layout B1 "1 hàng 8 xe",
   footprint chung 24x16, và fix khớp biên B1↔G vừa thêm (mục 3.1).
2. Sau khi chốt, B1/G/F2/F3 cần **sửa lại code hiện có** cho khớp —
   nhiều số khác code hiện tại (VD: B1 đổi từ lưới 4x4/khu sang 1 hàng
   8 xe/khu + dời tâm theo Z; `GARAGE_HALF_SIZE_X` 14→11; office/lobby
   nửa kích thước đổi 12x8→ giữ nguyên nếu code cũ đã đúng số này, cần
   đối chiếu lại).
3. F4 + Mái/Thoát hiểm **chưa có file `.rs`** — code mới hoàn toàn theo
   số ở mục 3.5/3.6, gồm cả cầu thang bộ phụ nối F4↔Mái vừa thêm.
4. Viết hàm sinh `NAVMESH_BOUNDS`/`NAVMESH_COLUMN_OBSTACLES` cho cả 6
   tầng theo mục 4, cùng lúc code từng tầng (không làm sau cùng, dễ
   quên).

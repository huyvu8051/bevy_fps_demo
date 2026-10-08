# Map 1 — Dimension Spec (bản nháp, chưa implement)

Bổ sung cho `map01_highrise.md` (mockup ASCII + pacing) — file đó mô tả
**cái gì** ở đâu, file này quyết **to bao nhiêu mét, ở toạ độ nào**.

⚠️ **Không dựa trên số đã code** (`crates/shared/src/map1/*.rs`) — số ở đó
là số đặt tạm lúc code B1, chưa chắc đúng. Nếu số ở đây lệch code hiện
tại → **code sai, phải sửa code**, không phải ngược lại.

## v2 — Mở rộng diện tích x4 (mỗi cạnh x2)

Bản này thay bản gốc (đã bị ghi đè). **Mọi footprint và toạ độ nhân đôi
so với bản trước** (diện tích sàn mỗi tầng x4). Quy tắc áp dụng:

- **Kích thước vật thể thật giữ nguyên** (không phóng to): xe (mục 1,
  đo từ mesh/tỉ lệ thật), bàn/cubicle/quầy/locker, cửa (0.9/1.8m), người
  chơi, đường kính cầu thang xoắn ốc (4.0m). Phóng to các vật này làm
  sai tỉ lệ asset/mesh.
- **Không gian đi lại/thiết kế nhân đôi**: hành lang, lối đi, aisle,
  clearance quanh cầu thang, khoảng cách giữa các vật lặp lại (xe,
  cubicle) — đây là lựa chọn thiết kế, không phải kích thước vật lý cố
  định, nên tăng để tạo cảm giác thoáng hơn (dễ né đạn/di chuyển hơn).
- **Kích thước phòng (không phải vật thể đơn lẻ) cũng nhân đôi mỗi
  cạnh** — văn phòng riêng, phòng họp, Safe Room... đây là không gian
  sàn, thuộc phạm vi "rộng ra" người dùng yêu cầu.
- **Chiều cao (trần, `FLOOR_HEIGHT`, độ dốc dựa trên chiều cao) giữ
  nguyên** — yêu cầu là "rộng ra" (mặt sàn X/Z), không phải "cao lên"
  (Y). Vì vậy độ dốc (ramp) lên B1→G vẫn dài ~9.4m như cũ dù rộng hơn
  (chiều dốc phụ thuộc độ cao leo 3.0m cố định, không phụ thuộc diện
  tích sàn) — chỉ **bề rộng** dốc nhân đôi theo hành lang chính.
- **Số lượng zombie nhân x4 theo diện tích (v2), rồi x3 nữa để tăng
  cường độ bắn + spawn theo nhịp (v3)** — xem `map01_highrise.md` bảng
  tổng kết: 19→76→228 zombie toàn map, giữ nguyên tỉ lệ giữa các tầng.
  Ammo/med kit/audio log **giữ nguyên số lượng** qua mọi bản — đây là
  nhịp game, độc lập với diện tích sàn/số zombie.
- **Toạ độ ở mục 3 (v3) trình bày theo cụm (tâm + bán kính rải + số
  lượng)** thay vì liệt kê từng điểm — số lượng mỗi đợt spawn (24-36
  con) không còn hợp lý để ghi toạ độ tay từng con; xem bảng "Nhịp
  spawn" tương ứng ở `map01_highrise.md` cho trigger/thời điểm từng đợt.

Mức chính xác vẫn là **blockout game, số làm tròn dễ nhớ**, đúng tinh
thần bản gốc — không truy đến milimet.

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

---

## 1. Thư viện kích thước chuẩn (dùng chung mọi map)

| Nhóm | Item | Kích thước (Vec3: rộng X, cao Y, sâu Z) | Cơ sở |
|---|---|---|---|
| Player | Capsule | radius 0.35, length 0.9 (đã code, giữ nguyên) | `PLAYER_RADIUS`/`PLAYER_LENGTH` |
| Player | Khoảng đứng chờ spawn | **2.0 x — x 2.0 /người** (v2: x2, không gian đi lại) | đủ không đè lên nhau, thoáng hơn |
| Xe | Sedan (gara) | (1.9, 1.5, 4.6) | sedan thật ~1.8m ngang, ~1.45m cao, ~4.5m dài |
| Xe | Cargo Van | (2.0, 2.3, 5.5) | van chở hàng cỡ trung |
| Xe | Pickup Truck | (2.6, 2.2, 5.3) | đo trực tiếp từ bbox mesh (đã đúng tỉ lệ mét thật sẵn) |
| Xe | VF3-inspired EV (compact) | (1.68, 1.6, 3.19) | xe điện mini boxy |
| Xe | SUV | (1.9, 1.8, 4.6) | SUV off-road cỡ trung |
| Xe | Hatchback | (1.7, 1.45, 3.9) | xe nhỏ đô thị |
| Xe | Sports car | (1.85, 1.25, 4.5) | 2 cửa, gầm thấp |
| Xe | Taxi (sedan) | (1.8, 1.5, 4.8) | |
| Xe | Police cruiser | (1.85, 1.45, 5.0) | sedan + light bar |
| Xe | Ambulance | (2.1, 2.6, 6.0) | van boxy + module cứu thương |
| Xe | Delivery box truck | (2.2, 2.5, 6.5) | thùng hộp sau cabin |
| Xe | Tow truck | (2.3, 2.3, 6.5) | flatbed + winch |
| Xe | Station wagon | (1.8, 1.5, 4.8) | |
| Xe | Motorcycle | (0.8, 1.15, 2.2) | tính tới tay lái |
| Xe | Muscle car | (1.9, 1.35, 5.0) | vintage, thân rộng |
| Xe | Minivan | (1.95, 1.75, 5.1) | cửa trượt 2 bên |
| Xe | 1 ô đậu xe (kể cả biên) | 2.5 x — x 5.0 (giữ nguyên, chuẩn thật) | tiêu chuẩn VN/US |
| Xe | Aisle xe ra vào (2 chiều) | rộng **12.0** (v2: x2) | đủ xe lùi/quay, thoáng hơn |
| Nội thất | Bàn làm việc đơn | (1.4, 0.75, 0.7) | bàn có màn hình |
| Nội thất | 1 ô cubicle (bàn+vách+lối vào) | (1.6, 1.2, 1.6) | vách cao ngang ngực, giữ nguyên (vật thể thật) |
| Nội thất | Lối đi trước dãy cubicle | rộng **4.0** (v2: x2) | đủ 1 người + né đạn, thoáng hơn |
| Nội thất | Quầy lễ tân | (2.2, 1.1, 0.6) | quầy cao, cover đứng |
| Nội thất | Tủ đồ/locker | (0.4, 1.8, 0.5) | 1 ô đơn |
| Kiến trúc | Hành lang chính | rộng **4.8** (v2: x2) | 2 người tránh nhau lúc combat, thoáng hơn |
| Kiến trúc | Hành lang phụ | rộng **3.6** (v2: x2) | |
| Kiến trúc | Cửa đơn | rộng 0.9, cao 2.1 (giữ nguyên) | |
| Kiến trúc | Cửa đôi/kính lớn | rộng 1.8, cao 2.4 (giữ nguyên) | phòng họp, sảnh |
| Kiến trúc | Cầu thang xoắn ốc (trục chính) | đường kính 4.0 (giữ nguyên), clearance quanh **8x8** (v2: x2) | 1 người đi, chật vừa đủ tạo áp lực |
| Kiến trúc | Tường ngoài (kết cấu, có collider) | dày 1.0 (giữ nguyên) | `ARENA_WALL_THICKNESS` — hằng số toàn cục |
| Kiến trúc | Tường/vách nội thất (panel) | dày 0.15 (giữ nguyên) | `LevelPanel` |
| Kiến trúc | Trần | Gara 2.6 / Văn phòng 2.8 / Sảnh chính (thông tầng) 6.0 (giữ nguyên — chiều cao không thuộc phạm vi x4) | |
| Kiến trúc | Tầng-tầng | 3.0 (giữ nguyên) | `FLOOR_HEIGHT` — hằng số toàn cục |

Các dòng **Xe** (trừ Sedan/Pickup, đo trực tiếp từ mesh) là số ước lượng
theo xe thật ngoài đời — dùng để điền vào ô **Height** ở dialog Download
của Meshy (bật **Resize** + **Origin: Bottom**) trước khi tải mesh về,
xem quy trình đầy đủ ở `docs/PROP_IMPORT_CHECKLIST.md`.

---

## 2. Cụm tầng xếp chồng & footprint chung

**G, F2, F3, F4, Mái** thuộc 1 cụm xếp chồng (cùng trục cầu thang xoắn
ốc, cùng viền tường ngoài) → **bắt buộc chung 1 footprint**. B1 là hầm,
không bị ràng buộc thẳng hàng với các tầng trên → tính riêng.

**F2 là tầng đông item nhất trong cụm** (cubicle + 2 phòng riêng + cầu
thang) → dùng F2 để suy ra footprint chung (mục 3.3). Các tầng G/F3/F4/
Mái áp dụng lại đúng footprint đó, chỉ tự kiểm tra "có vừa không" khi bố
trí nội dung riêng.

**v2: footprint chung 48m x 32m** (half-size 24 x 16) — x2 mỗi cạnh so
với bản gốc 24x16.

---

## 3. Chi tiết từng tầng

### 3.1 B1 — Gara (Build-up, 36 zombie [v1: 3 → v2: 12 → v3: 36], 1 ammo, 3 đợt spawn) — tính riêng

Bố cục: **1 hàng 8 xe** cho mỗi khu A/B (không phải lưới 4x4 — xếp 1
hàng dài đông hơn tạo footprint vuông vắn thay vì dài hẹp). **v2:**
khoảng cách giữa các xe trong hàng tăng lên (không còn liền kề) để lấp
không gian dôi ra — xe vẫn giữ nguyên kích thước thật. **v3:** zombie
x3 nữa (12→36), chia 3 đợt spawn theo trigger (xem `map01_highrise.md`
mục B1 cho bảng đầy đủ): đợt 1 (8, mission start, Khu A), đợt 2 (10,
băng qua aisle, Khu B), đợt 3 (18, tới gần tủ đồ ammo, tràn từ sau
hàng xe + cửa gara).

**Kiểm kê Khu A (spawn + hàng xe 1):**

| Item | SL | Kích thước | Đệm | Chiếm dụng |
|---|---|---|---|---|
| Player spawn | 4 | điểm | 2.0/người (x2), xếp hàng ngang | 8.0m (X) x 4.0m (Z) |
| Khoảng đệm spawn → hàng xe (mới, v2) | — | — | 5.0m | +5.0m (Z) |
| Ô đậu xe (1 hàng) | 8 | 2.5 x 5.0 (giữ nguyên) | khoảng cách ~2.5m giữa các ô (mới, để lấp độ rộng dôi ra) | 40.0m (X) x 5.0m (Z) |
| Lề an toàn 2 bên tường | — | — | 2.0m mỗi bên (x2) | +4.0m (X) |
| Zombie spawn (đợt 1, mission start) | **8** (v1: 1 → v2: 4 → v3: 8) | điểm | rải giữa hàng xe | — |

→ **Khu A = 44.0m (X) x 14.0m (Z)** (40+4 ngang; spawn 4.0 + đệm 5.0 +
xe 5.0 sâu)

**Aisle nối Khu A/B:** rộng 12.0m (Z, x2) x 44.0m (X)

**Kiểm kê Khu B (hàng xe 2, sâu/tối hơn):**

| Item | SL | Kích thước | Đệm | Chiếm dụng |
|---|---|---|---|---|
| Ô đậu xe (1 hàng) | 8 | 2.5 x 5.0 (giữ nguyên) | khoảng cách ~2.5m giữa các ô | 40.0m (X) x 5.0m (Z) |
| Tủ đồ bảo vệ (ammo) | 1 | 0.4 x 0.5 | góc, cần 2.0m clearance (x2) | nằm trong margin |
| Zombie spawn (đợt 2, băng aisle) | **10** (v1: 1 → v2: 1 → v3: 10) | điểm | rải trong hàng xe | — |
| Zombie spawn (đợt 3, tới ammo) | **18** (mới ở v3) | điểm | tràn từ sau hàng xe + cửa gara | — |
| Margin sau (tủ đồ + xác deco + đệm mở rộng, v2) | — | — | 7.0m | +7.0m (Z) |

→ **Khu B = 44.0m (X) x 12.0m (Z)**

**Tổng B1 (chưa tính dốc):** X = 44.0m; Z = 14.0 + 12.0 (aisle) + 12.0 =
38.0m. Cộng dốc lên sảnh G (rộng 4.8m hành lang chính, nhô thêm ở cạnh
Bắc) → làm tròn bao trọn thành **44m x 44m** (x4 diện tích đúng bản gốc
22x22).

→ **B1 half-size: X=22, Z=22** — trần giữ nguyên 2.6m.

⚠️ **Khớp biên với G:** G có footprint 48x32 (half Z=16), cạnh Bắc tại
`z=-16`. B1 rộng hơn nên dời tâm B1 theo Z sao cho cạnh Bắc B1 trùng
đúng cạnh Bắc G (`z=-16`) — B1 rộng hơn nên phần dư nằm hết về phía Nam
(hầm to hơn, ăn sâu xuống dưới thêm, không mở rộng về phía mặt tiền).

→ **B1: north_wall_z = -16, half_x = 22, half_z = 22** → span thực tế:
`x ∈ [-22,22], z ∈ [-16, 28]`.

**Toạ độ (mọi giá trị nhân đôi so với bản v1):**

- Cột X 8 ô xe (bước 5.0 — xe 2.5 + khoảng cách 2.5, đối xứng quanh 0):
  `-17.5, -12.5, -7.5, -2.5, 2.5, 7.5, 12.5, 17.5`
- Player spawn (z=-12, 4 điểm cách 2m): `x = -3, -1, 1, 3`
- Hàng xe Khu A: `z = -5`
- Zombie đợt 1 (8, mission start): cụm quanh `(0, y, -5)`, rải đều dọc
  trục X trong khoảng `x ∈ [-17, 17]`, bán kính rải mỗi con ~2m quanh
  khe xe gần nhất
- Aisle: `z ∈ [0, 12]`
- Hàng xe Khu B: `z = 17`
- Zombie đợt 2 (10, băng aisle): cụm quanh `(0, y, 17)`, rải dọc `x ∈
  [-17, 17]` dọc hàng xe Khu B
- Zombie đợt 3 (18, tới ammo — mini-peak): cụm quanh `(9, y, 21)` (khu
  tủ đồ), rải rộng `x ∈ [-20, 20], z ∈ [19, 23]`, tràn từ margin sau +
  cửa gara
- Tủ đồ (ammo): `(18, y, 23)`
- Deco xác bảo vệ: `(-18, y, 23)`
- Dốc lên G: cạnh Bắc `z = -16` (khớp đúng cạnh Bắc G), tâm `x = 0`,
  rộng **4.8m** (x2, khớp hành lang chính mới), dài **~9.4m (giữ
  nguyên** — độ dốc phụ thuộc chiều cao leo 3.0m cố định, không đổi
  theo diện tích sàn, góc ~18° giữ nguyên) — đỉnh dốc nối thẳng vào sàn
  G ngay tại đường biên chung.

---

### 3.2 F2 — Văn phòng tầng 2 — tầng quyết định footprint chung

**Kiểm kê:**

| Item | SL | Kích thước | Đệm | Chiếm dụng |
|---|---|---|---|---|
| Cubicle (1 hàng "C C C C C") | 5 | 1.6 x 1.6 (giữ nguyên) | ~3.0m giữa mỗi cubicle (mới, v2) | 20.0m (X) x 1.6m (Z) |
| Lối đi trước hàng cubicle | 1 | — | rộng 4.0 (x2) | +4.0m (Z) |
| Lề 2 bên hàng cubicle | — | — | 2.0m mỗi bên (x2) | +4.0m (X) |
| Văn phòng riêng (cửa kính) | 1 phòng | 8.0 x 6.0 (x2, không gian sàn) | — | 8.0m (X) x 6.0m (Z) |
| Phòng tủ đồ nhân viên | 1 phòng | 6.0 x 6.0 (x2) | gap 2.0 (x2) với phòng trên | 6.0m (X) x 8.0m (Z, gồm gap) |
| Cầu thang xoắn ốc | 1 | dia 4.0 (giữ nguyên) | clearance quanh 2.0 (x2) | 8.0m (X) x 8.0m (Z) |
| Hành lang chính nối cubicle↔phòng | 1 | rộng 4.8 (x2) | — | +4.8m (X) |
| Hành lang phụ nối phòng↔cầu thang | 1 | rộng 3.6 (x2) | — | +3.6m (X) |
| Med kit | 1 | điểm | cạnh cầu thang | — |
| Zombie spawn, 3 đợt | **24** (v1: 2 → v2: 8 → v3: 24) | điểm | trong khu cubicle/lối đi/quanh cầu thang | — |
| `L1` audio log | 1 | điểm | trong văn phòng riêng | — |
| Ammo | 1 | điểm | trong phòng tủ đồ | — |

**Cộng theo trục X** (dãy song song: cubicle | hành lang chính | phòng
riêng/tủ đồ | hành lang phụ | cầu thang):
`20.0 (cubicle+lề) + 4.8 + 8.0 (phòng, bề rộng lớn nhất trong 2 phòng) +
3.6 + 8.0 = 44.4m ≈ 48.0m (làm tròn, khớp x2 bản gốc 24.0)`

**Cộng theo trục Z** (chiều sâu lớn nhất trong các nhánh): khu cubicle
`1.6+4.0=5.6m`, cụm 2 phòng xếp dọc `6.0+2.0+6.0=14.0m`, cụm cầu thang
`8.0m` → lấy max + lề trên/dưới `14.0 + 2x9.0 (không gian đi lại chung
quanh, x2, làm tròn) = 32.0m`

→ **Footprint chung cả cụm (G/F2/F3/F4/Mái): 48m x 32m, half-size 24 x
16**, trần F2 giữ nguyên 2.8m.

**Toạ độ F2** (tâm (0, `FLOOR_HEIGHT*1`)):

- Hàng cubicle (bước 4.6 = module 1.6 + khoảng cách 3.0): `x = -21,
  -17, -13, -9, -5`, `z = -10`
- Lối đi trước cubicle: `z ∈ [-6, -2]`
- Zombie đợt 1 (8, vào khu cubicle): cụm dọc hàng cubicle, `x ∈
  [-21,-5], z ∈ [-10,-2]`
- Zombie đợt 2 (8, mở phòng riêng/tủ đồ): cụm quanh `(12, y, -5)`,
  tràn ra từ văn phòng riêng + phòng tủ đồ
- Zombie đợt 3 (8, tiến gần cầu thang — đợt cuối trước Sustain Peak):
  cụm quanh `(16, y, -10)`, hội tụ quanh chân cầu thang
- Văn phòng riêng: tâm `(12, y, -10)`, kích thước 8x6
  - Bàn làm việc (giữ nguyên 1.4x0.7), áp tường Bắc, tâm `(12, y,
    -12.3)`
  - `L1` (đặt trên bàn): `(12, y, -12.3)`
  - Cửa kính (giữ nguyên 1.8m, mở về hành lang phụ phía Nam): tâm `(12,
    y, -7)`
- Phòng tủ đồ nhân viên: tâm `(12, y, 0)`, kích thước 6x6
  - Locker (giữ nguyên 0.4x1.8x0.5), 4 cái áp tường Tây, xếp cạnh nhau:
    tâm `x = 9.4`, `z = -2.2, -1.0, 0.2, 1.4` (bước 1.2m, x2, tường
    trong tại `x=9.0`)
  - Ammo (giữa phòng, trước dãy locker): `(12, y, 1.0)`
  - Cửa đơn (giữ nguyên 0.9m, mở về hành lang phụ phía Bắc): tâm `(12,
    y, -3)`
- Cầu thang xoắn ốc (cố định mọi tầng trong cụm): tâm `(16, —, -10)`
- Med kit: `(13, y, -6)` (giữa phòng tủ đồ và cầu thang)

---

### 3.3 G — Sảnh chính (Build-up→Sustain, 24 zombie [v1: 2 → v2: 8 → v3: 24], 1 ammo, 3 đợt spawn) — dùng lại footprint 48x32

Tầng này "không gian mở, trần cao" theo chủ đích thiết kế — không tự
suy footprint từ item (ít item), mà **kiểm tra item có vừa footprint đã
chốt (48x32) không**:

| Item | Kích thước | Vừa trong 48x32? |
|---|---|---|
| Quầy lễ tân | 2.2 x 0.6 (giữ nguyên), cần 6m clearance trước quầy (x2) | ✅, đặt góc Tây Nam |
| Cầu thang xoắn ốc | dia 4.0 (giữ nguyên) | ✅, đúng vị trí cố định `(16,-10)` |
| 24 zombie spawn, 3 đợt (v1: 2 → v2: 8 → v3: 24) | điểm | ✅, sảnh 48x32 đủ chỗ dàn trải 3 đợt liên tiếp |

→ **Footprint: 48x32 (half 24x16), tâm (0,0), trần giữ nguyên 6.0m**
(thông tầng).

**Toạ độ:**

- Quầy lễ tân: `(-16, 0.55, 10)`, mặt hướng vào giữa sảnh
- Ammo (sau quầy): `(-16, y, 11)`
- Cầu thang xoắn ốc: `(16, —, -10)` (giống F2)
- Zombie đợt 1 (6, vào sảnh): cụm quanh `(0, y, 0)`, rải rộng khắp
  giữa sảnh, tránh quầy lễ tân `(-16,10)` và cầu thang `(16,-10)`
- Zombie đợt 2 (8, quá nửa sảnh): cụm 2 bên hông, quanh `(-14, y, -8)`
  và `(14, y, 6)`
- Zombie đợt 3 (10, tới quầy lễ tân — mini-peak): cụm quanh `(-16, y,
  8)`, tràn từ sau quầy + mảng kính vỡ, rải `x ∈ [-22,-8], z ∈ [4,14]`
- Mảng kính vỡ (deco): `(0, y, -2)`
- Biển "ĐIỂM TẬP KẾT" (deco): `(4, y, 4)`

---

### 3.4 F3 — Văn phòng tầng 3 (SUSTAIN PEAK, 120 zombie [v1: ~10 → v2: 40 → v3: 120], 5 đợt spawn mỗi 6s) — dùng lại footprint 48x32

**Kiểm kê (kiểm tra vừa footprint, không tự suy):**

| Item | Kích thước | Vừa 48x32? |
|---|---|---|
| Phòng họp kính | 10.0 x 8.0 (x2) | ✅ góc Tây Bắc |
| Hành lang chính (nối 2 văn phòng) | rộng 4.8 (x2), dài 24.0 (x2) | ✅ chạy giữa |
| Văn phòng A | 8.0 x 8.0 (x2) | ✅ |
| Văn phòng B | 8.0 x 8.0 (x2) | ✅ |
| Cầu thang xoắn ốc | dia 4.0 (giữ nguyên) | ✅ cố định `(16,-10)` |
| Zombie (5 đợt x24 = **120**, v1: 3+2+3+1+1=10, v2: 40) | điểm | ✅ rải dọc hành lang + 2 văn phòng, mỗi đợt tự tản trong khu vực của mình |

→ **Footprint: 48x32 (giữ nguyên), tâm (0, `FLOOR_HEIGHT*2`), trần giữ
nguyên 2.8m.**

**Toạ độ:**

- Phòng họp kính: tâm `(-16, y, -8)`, size 10x8 (span `x[-21,-11],
  z[-12,-4]`); trigger `X`: `(-16, y, -8)`
  - Cửa kính đôi (giữ nguyên 1.8m, mở về hành lang phía Nam): tâm `(-16,
    y, -4)`
- Hành lang: `x ∈ [-12,12], z ∈ [-2,2]`
- **Đợt 1 (t=0s, 24 con, phòng họp):** cụm quanh `(-16, y, -4)`, rải
  bán kính ~5m (trong phòng họp + tràn ra cửa)
- **Đợt 2 (t=+6s, 24 con, 2 đầu hành lang):** 2 cụm 12 con, quanh
  `(-10, y, 0)` và `(10, y, 0)`, hội tụ dọc `x ∈ [-12,12], z ∈ [-2,2]`
- **Đợt 3 (t=+12s, 24 con, văn phòng B):** cụm quanh `(16, y, 8)`, rải
  bán kính ~5m (trong văn phòng B + tràn ra cửa)
- **Đợt 4 (t=+18s, 24 con, tăng viện hành lang):** cụm dọc toàn hành
  lang `x ∈ [-12,12]`, xen giữa số zombie đợt 2 còn sống sót
- **Đợt 5 (t=+24s, 24 con, văn phòng A + dồn về hành lang):** cụm quanh
  `(-14, y, 8)`, rải bán kính ~5m, tràn dần về phía hành lang
- Văn phòng A: tâm `(-14, y, 8)`, size 8x8 (span `x[-18,-10], z[4,12]`)
  — vị trí đợt 5
  - Cửa đơn (giữ nguyên 0.9m, mở về hành lang phía Bắc): tâm `(-14, y,
    4)`
- Văn phòng B: tâm `(16, y, 8)`, size 8x8 (span `x[12,20], z[4,12]`) —
  vị trí đợt 3
  - Cửa đơn (giữ nguyên 0.9m, mở về hành lang phía Bắc): tâm `(16, y,
    4)`
- Cầu thang xoắn ốc: `(16, —, -10)`

---

### 3.5 F4 — Văn phòng tầng 4 (Peak Fade, 24 zombie [v1: 2 → v2: 8 → v3: 24], 2 đợt thưa, chưa code) — dùng lại footprint 48x32

| Item | Kích thước | Vừa 48x32? |
|---|---|---|
| Phòng lưu trữ tài liệu (trống) | 10.0 x 8.0 (x2) | ✅ |
| Hành lang ngắn ra cửa thoát hiểm | rộng 4.8 (x2), dài 10.0 (x2) | ✅ cạnh Đông |
| Cửa thoát hiểm | 1.8 x 2.4 (giữ nguyên) | ✅ |
| Cầu thang bộ phụ lên Mái | 2.4 x 6.0 (x2) | ✅ cạnh cầu thang xoắn ốc |
| Zombie (2 đợt thưa, **24** v1: 2 → v2: 8 → v3: 24) | điểm | ✅ |

→ **Footprint: 48x32, tâm (0, `FLOOR_HEIGHT*3`), trần giữ nguyên 2.8m.**

**Toạ độ:**

- Phòng lưu trữ: tâm `(-12, y, 6)`, size 10x8 (span `x[-17,-7],
  z[2,10]`)
  - Cửa đơn (giữ nguyên 0.9m, mở về phía Đông ra khu trung tâm): tâm
    `(-7, y, 6)`
- Hành lang ra cửa thoát hiểm: `x ∈ [12,22], z ∈ [-2,2]`
- Cửa thoát hiểm: `(23, y, 0)`
- Zombie đợt 1 (16, đặt sẵn — stragglers từ F3): rải khắp tầng, `x ∈
  [-16,16], z ∈ [-10,8]`, tránh chồng lên phòng lưu trữ/hành lang thoát
- Zombie đợt 2 (8, t=+10s, trickle từ phòng lưu trữ): cụm quanh `(-12,
  y, 6)`, rải bán kính ~4m quanh cửa phòng lưu trữ
- Cầu thang xoắn ốc (dừng ở đây, không lên tiếp): `(16, —, -10)`
- **Cầu thang bộ phụ lên Mái:** tâm `(6, y, -10)`, kích thước 2.4 x 6.0
  (x2, chạy dọc trục Z) — đặt sát phía Tây cụm cầu thang xoắn ốc (cách
  6m, x2, không đụng), miệng cầu thang trên Mái tại đúng toạ độ `(6, —,
  -10)` (xem mục 3.6).

---

### 3.6 Mái nhà & Lối thoát hiểm (Relax + SR + Extract, chưa code) — dùng lại footprint 48x32

| Item | Kích thước | Vừa 48x32? |
|---|---|---|
| Safe Room (armory + bàn attachment) | 12.0 x 10.0 (x2) | ✅ |
| Miệng cầu thang bộ phụ lên từ F4 | 2.4 x 6.0 (x2) | ✅ ngay cạnh Safe Room |
| Thang cứu hoả ngoài trời | rộng 1.0 (giữ nguyên), cao suốt F4→đất | nhô ra ngoài viền, không tính vào footprint mái |
| Extract Zone (chân thang, mặt đất) | 8.0 x 8.0 (x2) | riêng, ở cao độ 0 |

→ **Footprint mái: 48x32, tâm (0, `FLOOR_HEIGHT*4`), ngoài trời (không
trần).**

**Toạ độ:**

- Safe Room: tâm `(0, y, -8)`, size 12x10 (span `x[-6,6], z[-13,-3]`)
  - Cửa vào (giữ nguyên 1.8m, mở về phía Đông — đúng hướng người chơi
    vừa lên từ cầu thang phụ): tâm `(6, y, -8)`
- Miệng cầu thang bộ phụ lên từ F4 (đúng toạ độ mục 3.5): `(6, —, -10)`
  — trồi lên ngay sát cửa Safe Room, đúng luồng "lên cầu thang → thấy
  Safe Room ngay trước mặt".
- Thang cứu hoả: gắn cạnh Đông toà nhà, `x = 24`, chạy dọc suốt chiều
  cao xuống `y=0`
- Extract Zone: chân thang, mặt đất `(24, 0, 0)`, vùng trigger 8x8

---

## 4. Navmesh bounds (dẫn xuất — không cần quyết định số mới)

Các map khác trong code đã có `NAVMESH_BOUNDS`/`NAVMESH_COLUMN_OBSTACLES`
cho bot đi lại né vật cản; Map 1 hiện B1/G/F2/F3 **chưa có**, và F4/Mái
sắp code mới càng cần ngay từ đầu. Cả 2 đều **dẫn xuất thẳng từ số đã
chốt ở mục 3 (bản v2, đã x2)**, không phải quyết định thiết kế mới:

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

1. Bạn duyệt/chỉnh số ở mục 1-3 (bản v2, đã x2 diện tích) — đặc biệt
   layout B1 "1 hàng 8 xe giãn cách rộng hơn", footprint chung mới
   48x32, và khớp biên B1↔G tại `z=-16`.
2. Sau khi chốt, B1/G/F2/F3 cần **sửa lại code hiện có** cho khớp — số
   đổi nhiều so với cả bản v1 lẫn code gốc (VD: `GARAGE_HALF_SIZE_X`
   14→22; office/lobby nửa kích thước 12x8→24x16 — cần đối chiếu lại
   toàn bộ hằng số trong `crates/shared/src/map1/*.rs`).
3. F4 + Mái/Thoát hiểm **chưa có file `.rs`** — code mới hoàn toàn theo
   số ở mục 3.5/3.6 (bản v2), gồm cả cầu thang bộ phụ nối F4↔Mái.
4. Viết hàm sinh `NAVMESH_BOUNDS`/`NAVMESH_COLUMN_OBSTACLES` cho cả 6
   tầng theo mục 4, cùng lúc code từng tầng (không làm sau cùng, dễ
   quên).
5. Nếu áp dụng x4 diện tích cho Map 2-20 sau này: các map đó hiện mới
   chỉ có mockup ASCII khu vực (`mapNN_*.md`), chưa có dimension spec
   dạng này — cần làm bước "chốt kích thước mét" từ đầu (như mục 1-4 ở
   đây) trước khi có số để nhân đôi, không thể áp trực tiếp quy tắc x2
   vì chưa có số gốc.

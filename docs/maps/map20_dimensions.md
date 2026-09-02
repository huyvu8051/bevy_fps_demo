# Map 20 — Dimension Spec x4 + Nhịp spawn x12 (bản nháp, chưa implement) — Boss cuối, kết thúc game

Bổ sung cho `map20_final_breach.md`. Dùng chung thư viện kích thước ở
`map01_dimensions.md` mục 1. Escape sequence tuyến tính tốc độ cao —
đếm ngược thời gian (⏱️) **giữ nguyên tuyệt đối**, không scale theo diện
tích (thời gian không phải chiều dài/rộng); độ dài quãng đường mỗi đoạn
x2 để khớp footprint nhưng tốc độ chạy của player không đổi → cần điều
chỉnh timer nếu áp thực tế (ghi chú ở cuối).

⚠️ v2 = x2 mỗi cạnh. Zombie thường x12. **Boss cuối KHÔNG x12** — giữ
nguyên 1 con (mạnh nhất trong 4 lần Tank-style, chỉ số không phải số
lượng quyết định độ khó).

---

## Đoạn 1 — Hành lang sập dần [Bắt đầu NGAY trong Sustain Peak]

- **Footprint v1:** dài 20m, rộng 4.8 (x2 hành lang chính) → **v2: dài
  40m, rộng 9.6m**
- ⏱️ 4:30 → **giữ nguyên** (thời gian không x2)

**Nhịp spawn:**

| Đợt | Trigger | Số zombie | Vị trí |
|---|---|---|---|
| 1 | Vào đoạn 1 | 24 (v1: 2, x12) | Rải dọc hành lang, tối thiểu — tốc độ chạy trốn vẫn là trọng tâm |

---

## Đoạn 2 — Qua khu phòng thí nghiệm đang cháy [Vẫn Sustain Peak]

- **Footprint v1:** 16x8 → **v2: 32m x 16m**
- ⏱️ 3:00 → giữ nguyên

**Nhịp spawn:**

| Đợt | Trigger | Số zombie | Vị trí |
|---|---|---|---|
| 1 | Qua khu phòng TN cháy | 12 (v1: 1, x12) | Dọc đường thoát hẹp |

---

## Đoạn 3 — Hành lang chính, chặn bởi Boss cuối [ĐỈNH ĐIỂM]

- **Footprint v1:** 20x14 (hẹp hơn 3 arena boss trước) → **v2: 40m x
  28m** — vẫn hẹp hơn tương đối so với Map 5/10/15 (48x48/56x56/64x64)
  dù cùng tỉ lệ x2, giữ đúng chủ đích "không gian chiến đấu hẹp hơn hẳn"
- ⏱️ 1:30 → giữ nguyên
- Boss cuối: tâm arena, giữ nguyên **1 con**, không x12
- Vật cản (C): rất ít, giữ nguyên số lượng ít — đúng chủ đích "buộc đối
  đầu trực diện"

**Không zombie** — giữ nguyên bản gốc, toàn bộ tập trung vào boss.

---

## Đoạn 4 — Cửa thoát cuối cùng [Peak Fade cực ngắn → Relax/Ending]

- **Footprint v1:** 12x8 → **v2: 24m x 16m**
- ⏱️ 0:20 → giữ nguyên
- `L20` (monologue text): không đổi

**Không zombie.**

---

## Ghi chú timer — ĐÃ CHỐT: buff tốc độ, giữ nguyên đếm ngược

Độ dài quãng đường mỗi đoạn x2 (theo x4 diện tích) nhưng đếm ngược thời
gian giữ nguyên (4:30→3:00→1:30→0:20) — nếu tốc độ di chuyển player
(`PLAYER_SPEED`-tương-đương) không đổi, quãng đường dài gấp đôi sẽ
KHÔNG kịp trong cùng thời gian.

**Quyết định: chọn hướng (a) — buff tốc độ chạy tạm thời khi trigger
escape sequence** (vd `PLAYER_SPEED x2` chỉ trong lúc đếm ngược Map 19→
20 đang chạy), **không đổi số phút đếm ngược**. Lý do chọn (a) thay vì
(b):

- Số phút (5:00 ở Map 19 → 4:30/3:00/1:30/0:20 ở Map 20) đã **gắn với
  lời thoại chỉ huy BRDF** ("Cơ sở sẽ tự hủy trong 5 phút" — L19, xem
  `map19_command_bunker.md`) — đổi số sẽ phải sửa cả câu thoại đã viết,
  lan sang nội dung tường thuật không liên quan tới việc mở rộng bản đồ.
- Buff tốc độ tạm thời trong 1 đoạn escape có kịch bản rõ ràng (nhân vật
  đang chạy thoát khỏi cơ sở sắp nổ) là hợp lý về mặt câu chuyện — không
  cần giải thích gì thêm với người chơi, khác với việc âm thầm đổi số
  phút hiện trên UI đếm ngược mà người chơi có thể để ý và thấy vô lý so
  với lời thoại vừa nghe.
- Không ảnh hưởng tới cân bằng chiến đấu — buff chỉ áp dụng tốc độ di
  chuyển, không đổi damage/health, nên không phá vỡ độ khó đã tune ở
  đoạn 3 (boss).

**Việc cần làm khi code:** thêm 1 buff tốc độ (`SpeedBuff`-tương-đương)
kích hoạt đúng lúc trigger tự hủy ở Map 19 (`X ← TRIGGER` trong
`map19_command_bunker.md`), áp dụng xuyên suốt Map 19 phần cuối + toàn
bộ Map 20, tắt khi qua Extract Zone kết thúc game. Hệ số x2 là điểm bắt
đầu hợp lý (khớp đúng x2 độ dài quãng đường) — cần playtest thật để tune
chính xác, không chỉ tính toán trên giấy.

---

## Tổng kết số liệu (v1 → v3)

| Đoạn | Tag pacing | Zombie (v1→v3) | Boss (không x12) | Ghi chú |
|---|---|---|---|---|
| 1. Hành lang sập | Sustain Peak (tẩu thoát) | 2 → **24** | 0 | — |
| 2. Phòng TN cháy | Sustain Peak | 1 → **12** | 0 | — |
| 3. Boss cuối | **ĐỈNH ĐIỂM** | 0 → 0 | **1 (mạnh nhất)** | — |
| 4. Cửa thoát | Peak Fade cực ngắn | 0 → 0 | 0 | L20 |
| **Tổng** | | **3 → 36** | **1 boss cuối** | |

---

## So sánh tổng thể Campaign 4 (Map 16-20, sau x4 diện tích + x12 zombie thường)

| Map | Zombie thường (v1→v3) | Biến thể lab (v1→v3) | Lính BRDF (không x12) | Chỉ huy | Boss |
|---|---|---|---|---|---|
| 16. Main Gatehouse | 5 → **60** | — | **2** | 0 | 0 |
| 17. Sector 3 Admin Block | 2 → **24** | — | **5** | 0 | 0 |
| 18. Subterranean Lab Alpha | 8 → **96** | 3 → **36** | 0 | 0 | 0 |
| 19. Command Bunker | 2 → **24** | — | **5** | **1** | 0 |
| 20. Final Breach | 3 → **36** | — | 0 | 0 | **1 (mạnh nhất)** |
| **Tổng Campaign 4** | **20 → 240** | **3 → 36** | **12** | **1** | **1** |

---

## So sánh CUỐI CÙNG toàn bộ 4 Campaign (Map 1-20, sau x4 diện tích + x12 zombie thường)

| Campaign | Zombie thường (v1→v3) | Đặc biệt/biến thể (không x12, trừ lab-variant) | Lính/Chỉ huy BRDF | Boss |
|---|---|---|---|---|
| 1. Ground Zero | 47 → **564** | 6 (Boomer 2, ẩn 3, Tank 1) | 0 | 1 |
| 2. Vành đai | 22 → **264** | 5 (Hunter/Smoker/Spitter/Jockey, Tank 1) | 0 | 1 |
| 3. Vùng công nghiệp | 23 → **276** | 2 (Witch 1, Tank 1) | 0 | 1 |
| 4. Cổng thành | 20 → **240** (+lab 3→36) | 1 (Boss cuối) | 13 | 1 (mạnh nhất) |
| **TỔNG TOÀN GAME** | **112 → 1344** (+36 lab) | **14** | **13** | **4 lần Tank + 1 chỉ huy + 1 boss cuối** |

Zombie thường toàn game x12 đúng công thức (112×12=1344, cộng riêng 36
biến thể lab Map 18). Mọi địch đặc biệt/boss/lính BRDF/chỉ huy **giữ
nguyên tuyệt đối số lượng gốc** xuyên suốt cả 20 map — nguyên tắc thống
nhất áp dụng nhất quán từ Map 1 tới Map 20: chỉ zombie thường (và biến
thể cosmetic như lab-zombie) tăng theo diện tích x4 + cường độ x3 = x12
tổng; mọi loại địch có AI/cơ chế/set-piece riêng giữ nguyên để không
phá vỡ thiết kế combat đã tính riêng cho từng loại.

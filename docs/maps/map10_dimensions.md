# Map 10 — Dimension Spec x4 + Nhịp spawn x12 (bản nháp, chưa implement) — Boss Map

Bổ sung cho `map10_checkpoint_delta.md`. Dùng chung thư viện kích thước
ở `map01_dimensions.md` mục 1.

⚠️ Footprint v1 ước lượng lần đầu. v2 = x2 mỗi cạnh. Zombie thường x12.
**Boss (Tank-style, tái xuất lần 2) KHÔNG x12** — giữ nguyên 1 con.

---

## Khu A — Hàng rào ngoài trạm kiểm soát [Build-up]

- **Footprint v1:** 20x14 → **v2: 40m x 28m**
- Tháp canh bỏ hoang (deco): góc

**Nhịp spawn:**

| Đợt | Trigger | Số zombie | Vị trí |
|---|---|---|---|
| 1 | Vào trạm | 24 (v1: 2, x12) | Rải quanh hàng rào |

---

## Khu B — Lô cốt & khu vực chuẩn bị [vẫn Build-up, Safe Room ẩn trong item]

- **Footprint v1:** 16x12 → **v2: 32m x 24m**
- `L10`, ammo, med kit, bản đồ khu vực (lore): trong lô cốt

**Nhịp spawn:**

| Đợt | Trigger | Số zombie | Vị trí |
|---|---|---|---|
| 1 | Vào lô cốt | 12 (v1: 1, x12) | Rải trong lô cốt |

---

## Sân trạm kiểm soát — BOSS ARENA [SUSTAIN PEAK = Boss Fight #2]

- **Footprint v1:** 28x28 (lớn hơn Map 5 — công trình quân sự) → **v2:
  56m x 56m** (half 28x28)
- 4 khối bê tông chắn xe: đối xứng quanh tâm, cách tâm ~18m (x2 từ ~9m)
- Boss (Tank-style, tái xuất lần 2): tâm arena, giữ nguyên 1 con, không
  x12

**Không zombie thường** — giữ nguyên nguyên tắc "giữ fight rõ ràng" từ
Map 5, không đổi dù diện tích rộng hơn.

---

## Tổng kết số liệu (v1 → v3)

| Khu vực | Tag pacing | Zombie (v1→v3) | Boss (không x12) | Ammo | Med kit | Audio log |
|---|---|---|---|---|---|---|
| A. Hàng rào ngoài | Build-up | 2 → **24** | 0 | 0 | 0 | — |
| B. Lô cốt | Build-up | 1 → **12** | 0 | 1 | 1 | L10 |
| C. Sân trạm (Boss) | **Sustain Peak** | 0 → 0 | **1** | 0 | 0 | — |
| **Tổng** | | **3 → 36** | **1 boss** | **1** | **1** | **1** |

---

## So sánh tổng thể Campaign 2 (Map 6-10, sau x4 diện tích + x12 zombie thường)

| Map | Zombie thường (v1→v3) | Đặc biệt/Boss (không x12) | Ammo | Med kit |
|---|---|---|---|---|
| 6. Sundown Gardens | 3 → **36** | 1 (Hunter) | 1 | 0 |
| 7. Fairview Elementary | 5 → **60** | 1 (Smoker) | 0 | 1 |
| 8. Miller's Pond | 5 → **60** | 1 (Spitter) | 1 | 1 |
| 9. Perimeter Night Market | 6 → **72** | 1 (Jockey) | 1 | 1 |
| 10. Checkpoint Delta | 3 → **36** | 1 (Tank) | 1 | 1 |
| **Tổng Campaign 2** | **22 → 264** | **5** | **4** | **4** |

Đúng x12 (22×12=264). Đặc biệt/boss giữ nguyên tuyệt đối, tiếp tục
nguyên tắc thống nhất từ Campaign 1.

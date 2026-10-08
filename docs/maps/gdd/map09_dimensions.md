# Map 9 — Dimension Spec x4 + Nhịp spawn x12 (bản nháp, chưa implement)

Bổ sung cho `map09_perimeter_night_market.md`. Dùng chung thư viện kích
thước ở `map01_dimensions.md` mục 1.

⚠️ Footprint v1 ước lượng lần đầu. v2 = x2 mỗi cạnh. Zombie thường x12.
**Jockey-style KHÔNG x12** — giữ nguyên 1 con, đúng nguyên tắc chung.
Mê cung quầy hàng ở Khu B **càng rộng hơn (x2) càng làm Jockey nguy
hiểm hơn** (tách người chơi xa nhóm hơn khi kéo đi).

---

## Khu A — Lối vào chợ [Build-up]

- **Footprint v1:** 20x14 → **v2: 40m x 28m**

**Nhịp spawn:**

| Đợt | Trigger | Số zombie | Vị trí |
|---|---|---|---|
| 1 | Vào cổng chợ | 12 (v1: 1, x12) | Rải quanh cổng |

---

## Khu B — Giữa chợ (mê cung quầy hàng) [SUSTAIN PEAK — giới thiệu Jockey]

- **Footprint v1:** mê cung quầy hàng 24x18 (không đối xứng, nhiều lối
  rẽ) → **v2: 48m x 36m** — mê cung rộng hơn = lạc nhau xa hơn khi bị
  Jockey kéo đi
- Jockey: giữ nguyên 1 con, không x12, xuất hiện giữa mê cung
- `L9` audio log: gần loa quầy hàng, giữa khu

**Nhịp spawn (zombie thường, x12):**

| Đợt | Trigger | Số zombie | Vị trí |
|---|---|---|---|
| 1 | Vào mê cung quầy hàng | 24 (v1: 2, x12) | Rải quanh 2 dãy quầy đầu |
| 2 | Jockey tách 1 người khỏi nhóm (tăng viện tận dụng lúc nhóm bị chia) | 24 (v1: 2, x12) | Rải quanh 2 dãy quầy sau |

Tổng zombie thường 48 (v1: 4, x12). Jockey vẫn 1.

---

## Khu C — Khu chợ có dấu hiệu đụng độ [Peak Fade]

- **Footprint v1:** 16x10 → **v2: 32m x 20m**
- Vỏ đạn quân sự (deco/lore): rải rác

**Nhịp spawn:**

| Đợt | Trigger | Số zombie | Vị trí |
|---|---|---|---|
| 1 | Đi qua khu vực | 12 (v1: 1, x12) | Rải quanh khu, tapering |

---

## Khu D — Bãi đậu xe tải giao hàng [Relax + Safe Room + Extract]

- **Footprint v1:** 20x16 → **v2: 40m x 32m**
- Không zombie.

---

## Tổng kết số liệu (v1 → v3)

| Khu vực | Tag pacing | Zombie thường (v1→v3) | Jockey (không x12) | Ammo | Med kit | Audio log |
|---|---|---|---|---|---|---|
| A. Lối vào | Build-up | 1 → **12** | 0 | 0 | 0 | — |
| B. Giữa chợ | **Sustain Peak** | 4 → **48** | **1** | 0 | 0 | L9 |
| C. Khu đụng độ | Peak Fade | 1 → **12** | 0 | 0 | 0 | — |
| D. Bãi đậu xe tải | Relax | 0 → 0 | 0 | 1 | 1 | — |
| **Tổng** | | **6 → 72** | **1** | **1** | **1** | **1** |

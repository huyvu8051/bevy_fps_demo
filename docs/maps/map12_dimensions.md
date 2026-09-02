# Map 12 — Dimension Spec x4 + Nhịp spawn x12 (bản nháp, chưa implement)

Bổ sung cho `map12_blackridge_freight_yard.md`. Dùng chung thư viện kích
thước ở `map01_dimensions.md` mục 1 (container ~ tương đương "Xe cargo
van" về scale, dùng riêng kích thước container chuẩn 2.4x2.6x6.1m —
TEU thật).

⚠️ Footprint v1 ước lượng lần đầu. v2 = x2 mỗi cạnh. Zombie x12 (không
có địch đặc biệt ở map này).

---

## Khu A — Lối vào bãi container [Build-up — vẫn thấy toàn cảnh]

- **Footprint v1:** 24x18 → **v2: 48m x 36m**
- Cần trục cảng (mốc thị giác): góc xa, giữ nguyên vai trò định hướng,
  cao hơn hẳn tầm nhìn nên không cần x2 chiều cao

**Nhịp spawn:**

| Đợt | Trigger | Số zombie | Vị trí |
|---|---|---|---|
| 1 | Vào bãi | 12 (v1: 1, x12) | Rải giữa container thưa |

---

## Khu B — Mê cung container [SUSTAIN PEAK]

- **Footprint v1:** 30x24 (mê cung 4x3 khối container 2.4x6.1m) →
  **v2: 60m x 48m**
- Dãy container 447-460 (niêm phong BRDF, lore nối Map 13): góc khu,
  giữ nguyên vị trí tương đối
- Ammo/med kit: trong container thường dọc lối đi chính

**Nhịp spawn:**

| Đợt | Trigger | Số zombie | Vị trí |
|---|---|---|---|
| 1 | Vào mê cung container | 30 (v1: 2.5→3, x12 làm tròn) | Nửa đầu mê cung |
| 2 | Tiến sâu (âm thanh dội khó định vị — tăng viện bất ngờ) | 30 | Nửa sau mê cung |

Tổng 60 (v1: 5, x12).

---

## Khu C — Gần cần trục [Peak Fade]

- **Footprint v1:** 16x12 → **v2: 32m x 24m**
- `L12`: gần chân cần trục

**Nhịp spawn:**

| Đợt | Trigger | Số zombie | Vị trí |
|---|---|---|---|
| 1 | Đi qua khu vực | 12 (v1: 1, x12) | Tapering |

---

## Khu D — Dưới chân cần trục [Relax + Safe Room + Extract]

- **Footprint v1:** 20x16 → **v2: 40m x 32m**. Không zombie.

---

## Tổng kết số liệu (v1 → v3)

| Khu vực | Tag pacing | Zombie (v1→v3) | Ammo | Med kit | Audio log |
|---|---|---|---|---|---|
| A. Lối vào | Build-up | 1 → **12** | 0 | 0 | — |
| B. Mê cung container | **Sustain Peak** | 5 → **60** | 1 | 1 | L12 |
| C. Gần cần trục | Peak Fade | 1 → **12** | 0 | 0 | — |
| D. Dưới cần trục | Relax | 0 → 0 | 0 | 0 | — |
| **Tổng** | | **7 → 84** | **1** | **1** | **1** |

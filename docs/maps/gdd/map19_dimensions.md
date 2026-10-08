# Map 19 — Dimension Spec x4 + Nhịp spawn x12 (bản nháp, chưa implement)

Bổ sung cho `map19_command_bunker.md`. Dùng chung thư viện kích thước ở
`map01_dimensions.md` mục 1.

⚠️ v2 = x2 mỗi cạnh. Zombie thường x12. **Lính Meridian VÀ chỉ huy Meridian
KHÔNG x12** — giữ nguyên, cùng nguyên tắc Map 16/17. Chỉ huy đặc biệt là
"1 sự kiện kịch bản" (chạy trốn, kích hoạt tự hủy), không phải combat
đếm số — tuyệt đối không nhân bản.

---

## Hành lang ngoài [Build-up → SUSTAIN PEAK — lính phòng thủ dày nhất game]

- **Footprint v1:** hành lang bê tông dày, dài 30m, rộng 4.8 (x2 hành
  lang chính) → **v2: dài 60m, rộng 9.6m**
- Lính Meridian: giữ nguyên **5 con**, không x12 — vẫn "dày đặc nhất toàn
  game" đúng vai trò khu vực bảo vệ quyết liệt nhất

**Nhịp spawn (zombie thường, x12):**

| Đợt | Trigger | Số zombie | Vị trí |
|---|---|---|---|
| 1 | Vào hành lang | 24 (v1: 2, x12) | Rải xen kẽ giữa lính, ít hơn hẳn số lính |

Lính Meridian vẫn 5, không đổi.

---

## Phòng chỉ huy trung tâm [SUSTAIN PEAK — Đối đầu chỉ huy Meridian]

- **Footprint v1:** 20x16 → **v2: 40m x 32m**
- Chỉ huy Meridian: giữ nguyên **1 con** (sự kiện kịch bản, chạy trốn + kích
  hoạt tự hủy), không x12, tuyệt đối không nhân bản
- L19 (lời thoại trực tiếp qua loa): không đổi

**Không zombie thường mới ở đây** — giữ nguyên bản gốc, toàn bộ tập
trung vào encounter kịch bản.

---

## Chuyển tiếp khẩn cấp sang Map 20 [Không có Relax]

- Không đổi cấu trúc — vẫn map DUY NHẤT không có Safe Room/Extract
  riêng, nối liền Map 20. Đếm ngược 5 phút giữ nguyên (thời gian, không
  thuộc phạm vi x4 diện tích).

---

## Tổng kết số liệu (v1 → v3)

| Khu vực | Tag pacing | Zombie (v1→v3) | Lính Meridian (không x12) | Chỉ huy (không x12) |
|---|---|---|---|---|
| A. Hành lang ngoài | Build-up/Sustain | 2 → **24** | **5** | 0 |
| B. Phòng chỉ huy | **Sustain Peak (kịch bản)** | 0 → 0 | 0 | **1 (thoát)** |
| **Tổng** | | **2 → 24** | **5** | **1** |

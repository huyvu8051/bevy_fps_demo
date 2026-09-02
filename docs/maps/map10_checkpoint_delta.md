# Map 10 — "Trạm kiểm soát BRDF bỏ hoang" (Checkpoint Delta) — Boss Map

Campaign 2 "Vành đai", Map 10/20 (Map 5/5, **kết thúc campaign**). Xem
GDD.md mục 14.4. Cùng legend với Map 5 (Boss `👹`, Tank-style tái xuất
lần 2 — encounter tái diễn, không phải "chỉ 1 boss duy nhất cả game").

⚠️ **v3 — diện tích x4 + zombie thường x12:** xem `map10_dimensions.md`.
**Boss KHÔNG x12** — giữ nguyên 1 con.

---

## Khu A — Hàng rào ngoài trạm kiểm soát [Build-up]

```
┌──────────────────────────────────────────────────────────┐
│  P P P P  (spawn — tiếp nối Extract Map 9)                   │
│                                                              │
│   ═══════ HÀNG RÀO BÊ TÔNG NGOÀI (cửa mở toang) ═══════         │
│                                                              │
│         Z              Z                                    │
│                                                              │
│   🗼 (tháp canh bỏ hoang, không có lính — chỉ zombie)           │
└───────────────────────────────┬────────────────────────────┘
                                 ═══ Vào trong trạm
```

- **Mật độ (v3, x12):** 24 zombie (v1: 2) — Build-up.

---

## Khu B — Lô cốt & khu vực chuẩn bị [vẫn Build-up, Safe Room ẩn trong item]

```
┌──────────────────────────────────────────────────────────┐
│   ┌──────────────────────┐                                 │
│   │  LÔ CỐT CHỈ HUY           │  A  H  ← Đầy đủ đạn/med kit,   │
│   │                          │       vũ khí quân sự bỏ lại      │
│   │  L10 (Audio log #10 —    │       vội vàng (deco) — safe     │
│   │  sổ trực ca viết tay,     │       room tự nhiên trước boss   │
│   │  lệnh rút gấp từ          │                                 │
│   │  "Cổng thành")            │                                 │
│   │                          │                                 │
│   │  [Bản đồ khu vực dán      │  ← Lần đầu người chơi THẤY TRỰC   │
│   │   tường, đánh dấu rõ       │     QUAN vị trí "Cổng thành"      │
│   │   "Cổng thành" — deco     │     (trước đó chỉ nghe qua lời)   │
│   │   quan trọng]             │                                 │
│   └──────────────────────┘                                 │
│                                                              │
│              Z                                              │
└───────────────────────────────┬────────────────────────────┘
                                ↑ Ra sân trạm kiểm soát
```

- **Mật độ (v3, x12):** 12 zombie (v1: 1).
- **Item:** `L10`, đầy đủ ammo + med kit.
- **Vai trò kép:** vừa là điểm nạp tài nguyên trước boss (giống Map 5),
  vừa là điểm lore quan trọng (bản đồ xác nhận trực quan vị trí "Cổng
  thành" lần đầu).

---

## Sân trạm kiểm soát — BOSS ARENA [SUSTAIN PEAK = Boss Fight #2]

```
┌──────────────────────────────────────────────────────────┐
│                    SÂN TRẠM KIỂM SOÁT (bê tông, ngoài trời)    │
│                                                              │
│        C                              C                     │
│      (khối bê tông chắn xe — vật cản né đòn, quy mô           │
│       LỚN HƠN Map 5 do đây là công trình quân sự thật)          │
│                                                              │
│                         👹                                  │
│                   (Boss Tank-style, TÁI XUẤT lần 2 —           │
│                    chỉ số có thể tăng nhẹ so với Map 5,          │
│                    cụ thể hóa khi tune ở Giai đoạn 3)            │
│                                                              │
│        C                              C                     │
│                                                              │
│   KHÔNG zombie thường xen vào (giữ fight rõ ràng, đúng          │
│   nguyên tắc từ Map 5)                                          │
└───────────────────────────────┬────────────────────────────┘
```

- **Mật độ:** 0 zombie thường + **1 Boss** (KHÔNG x12, giữ nguyên —
  encounter #2 trong game).
- **Kiến trúc:** tương tự bố cục Map 5 (4 vật cản đối xứng) nhưng **quy
  mô lớn hơn** — sân trạm kiểm soát quân sự rộng hơn nóc gara dân sự.

---

## Kết thúc Campaign 2 [RELAX — Ending]

```
┌──────────────────────────────────────────────────────────┐
│  (boss đã hạ — sân trạm kiểm soát yên tĩnh)                    │
│                                                              │
│                ┌─────────────────┐                          │
│                │  E (Extract Zone) │                          │
│                │  = KẾT THÚC        │                          │
│                │  CAMPAIGN 2        │                          │
│                └─────────────────┘                          │
│                                                              │
│   Tiến vào Campaign 3 "Vùng công nghiệp" — không có twist       │
│   radio cuối như Campaign 1 (đã biết hướng đi rõ ràng nhờ        │
│   bản đồ ở Khu B, không cần thêm hook mới)                        │
└──────────────────────────────────────────────────────────┘
```

---

## Tổng kết số liệu (v1 → v3, xem `map10_dimensions.md` cho nhịp spawn chi tiết)

| Khu vực | Tag pacing | Zombie (v1→v3) | Boss (không x12) | Ammo | Med kit | Audio log |
|---|---|---|---|---|---|---|
| A. Hàng rào ngoài | Build-up | 2 → **24** | 0 | 0 | 0 | — |
| B. Lô cốt | Build-up | 1 → **12** | 0 | 1 | 1 | L10 |
| C. Sân trạm (Boss) | **Sustain Peak** | 0 → 0 | **1** | 0 | 0 | — |
| **Tổng** | | **3 → 36** | **1 boss** | **1** | **1** | **1** |

---

## So sánh tổng thể Campaign 2 (Map 6-10, v1 → v3 sau x4 diện tích + x12 zombie thường)

| Map | Zombie thường (v1→v3) | Đặc biệt (không x12) | Boss (không x12) | Ammo | Med kit |
|---|---|---|---|---|---|
| 6. Sundown Gardens | 3 → **36** | 1 (Hunter) | 0 | 1 | 0 |
| 7. Fairview Elementary | 5 → **60** | 1 (Smoker) | 0 | 0 | 1 |
| 8. Miller's Pond | 5 → **60** | 1 (Spitter) | 0 | 1 | 1 |
| 9. Perimeter Night Market | 6 → **72** | 1 (Jockey) | 0 | 1 | 1 |
| 10. Checkpoint Delta | 3 → **36** | 0 | 1 (Tank) | 1 | 1 |
| **Tổng Campaign 2** | **22 → 264** | **4** | **1** | **4** | **4** |

So với Campaign 1 (47→564 zombie thường, 5 đặc biệt, 1 boss): Campaign 2
có **mật độ zombie thường thấp hơn hẳn** (22→264 vs 47→564) nhưng **đa
dạng đặc biệt cao hơn nhiều** (4 loại khác nhau vs 3) — đúng chiến lược
chuyển trọng tâm độ khó từ "số lượng" (Campaign 1, đô thị đông đúc) sang
"đa dạng cơ chế" (Campaign 2, vành đai thưa dân) đã ghi trong GDD. Đặc
biệt/boss giữ nguyên tuyệt đối qua v1→v3.

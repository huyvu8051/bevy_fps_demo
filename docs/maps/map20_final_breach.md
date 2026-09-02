# Map 20 — "Lối thoát cuối cùng" (Final Breach) — Boss Cuối, Kết Thúc Game

Campaign 4 "Cổng thành", Map 20/20 — **kết thúc bản 1.0**. Xem GDD.md mục
14.6. Cùng legend với Map 16, thêm:

```
👹² Boss cuối — Tank-style biến thể mạnh hơn, "LAZARUS Subject" trưởng
    thành, kết quả cuối cùng của thí nghiệm (mục 15/14.6)
```

⚠️ **Khác biệt cấu trúc hoàn toàn:** map này là 1 **escape sequence
tuyến tính tốc độ cao**, KHÔNG có Build-up truyền thống — bắt đầu ngay
trong trạng thái khẩn cấp, tiếp nối trực tiếp từ Map 19 (không có
Safe Room/Extract giữa 2 map, xem map19).

⚠️ **v3 — diện tích x4 + zombie thường x12:** xem `map20_dimensions.md`
— gồm cả quyết định đã chốt về timer đếm ngược (giữ nguyên số phút,
buff tốc độ player thay vì đổi số). **Boss cuối KHÔNG x12** — giữ
nguyên 1 con.

---

## Đoạn 1 — Hành lang sập dần [Bắt đầu NGAY trong Sustain Peak]

```
┌──────────────────────────────────────────────────────────┐
│  P P P P  (tiếp nối trực tiếp từ Map 19 — KHÔNG spawn lại,     │
│             chạy thẳng, không dừng)                              │
│                                                              │
│   ⏱️ 4:30 (đồng hồ đếm ngược tiếp tục từ Map 19)                │
│                                                              │
│   ══🔥══💥══🔥══  (hành lang sập từng đoạn, lửa/khói, phải        │
│                     né mảnh vỡ rơi — không phải combat, mà       │
│                     là chướng ngại vật lý)                       │
│                                                              │
│   📺💥 (màn hình giám sát vỡ nát — NHẮC LẠI hình ảnh phòng        │
│         chỉ huy Map 19, closure trực quan)                        │
│                                                              │
│        Z    Z   (rất ít, không phải trọng tâm)                  │
└───────────────────────────────┬────────────────────────────┘
                                 ═══ Qua khu phòng thí nghiệm cháy
```

- **Mật độ (v3, x12):** 24 zombie (v1: 2) — tối thiểu tương đối, tốc độ
  chạy trốn vẫn là trọng tâm.

---

## Đoạn 2 — Qua khu phòng thí nghiệm đang cháy [Vẫn Sustain Peak]

```
┌──────────────────────────────────────────────────────────┐
│   ⏱️ 3:00                                                    │
│                                                              │
│   🔥🔥🔥 (phòng thí nghiệm — NHẮC LẠI Map 18 — cháy rụi khi       │
│           chạy ngang qua, closure trực quan lần 2)                │
│                                                              │
│        Z                                                    │
│                                                              │
│   ═══════ (đường thoát hẹp, phải chạy liên tục, không dừng      │
│             lại được lâu)                                        │
└───────────────────────────────┬────────────────────────────┘
                                 ═══ Tới hành lang chính dẫn ra ngoài
```

- **Mật độ (v3, x12):** 12 zombie (v1: 1).

---

## Đoạn 3 — Hành lang chính, chặn bởi Boss cuối [ĐỈNH ĐIỂM]

```
┌──────────────────────────────────────────────────────────┐
│   ⏱️ 1:30                                                    │
│                                                              │
│   ══════════════ HÀNH LANG CHÍNH ĐANG SẬP ══════════════       │
│                                                              │
│                         👹²                                 │
│                  (Boss cuối — "LAZARUS Subject" trưởng          │
│                   thành, CHẶN NGANG đường thoát chính,           │
│                   kẹt giữa hành lang đang sập — không gian        │
│                   chiến đấu HẸP HƠN hẳn 3 lần boss trước,          │
│                   không có nhiều chỗ né như Map 5/10/15)          │
│                                                              │
│   C          C  (rất ít vật cản — buộc đối đầu trực diện          │
│                   hơn, đúng tinh thần "không còn lựa chọn         │
│                   nào khác ngoài chiến đấu để sống")                │
└───────────────────────────────┬────────────────────────────┘
                                 ═══ Sau khi hạ boss
```

- **Mật độ:** 0 zombie + **1 Boss cuối** (KHÔNG x12, giữ nguyên — biến
  thể mạnh nhất trong 4 lần xuất hiện Tank-style xuyên suốt game).
- **Ghi chú thiết kế quan trọng:** không gian hẹp hơn hẳn 3 arena boss
  trước (Map 5/10/15 đều có 4 vật cản đối xứng, rộng rãi) — đây là **thử
  thách khó nhất về mặt cơ chế** (ít chỗ né) kết hợp với **áp lực thời
  gian** (hành lang đang sập) làm tổng hòa độ khó cao nhất game, không chỉ
  dựa vào chỉ số boss.

---

## Đoạn 4 — Cửa thoát cuối cùng [Peak Fade cực ngắn → Relax/Ending]

```
┌──────────────────────────────────────────────────────────┐
│  (boss đã hạ — không có thời gian nán lại, cơ sở vẫn sập)       │
│                                                              │
│   ⏱️ 0:20                                                    │
│                                                              │
│   [Biển hiệu "CƠ SỞ NGHIÊN CỨU SINH HỌC BLACKRIDGE" —           │
│    NHẮC LẠI Map 16 — ĐỔ SẬP ngay khi nhóm ra tới cửa ngoài,       │
│    closure trực quan cuối cùng khép vòng lặp toàn Campaign 4]     │
│                                                              │
│   📝 L20 (KHÔNG PHẢI audio log — monologue nội tâm ngắn dạng     │
│           text, không gán nhân vật cụ thể, giữ nguyên tắc         │
│           "người chơi không thoại": "Tụi mình biết sự thật        │
│           rồi. Câu hỏi bây giờ không phải là có tin hay           │
│           không — mà là làm gì với nó.")                          │
│                                                              │
│                ┌─────────────────┐                          │
│                │  E (Extract Zone) │                          │
│                │  = KẾT THÚC GAME   │                          │
│                │    (bản 1.0)        │                          │
│                └─────────────────┘                          │
└───────────────────────────────┬────────────────────────────┘
                                 ═══
```

## Ending [RELAX — Credit/Kết thúc]

```
┌──────────────────────────────────────────────────────────┐
│   Nhóm thoát ra ngoài trời, nhìn cơ sở BRDF sụp đổ phía          │
│   sau — KẾT THÚC MỞ (không quyết định thay người chơi):          │
│                                                              │
│     → Công khai sự thật (rủi ro hoảng loạn diện rộng)            │
│     → Im lặng bỏ đi                                            │
│                                                              │
│   Mở đường DLC/Campaign 5+ khai thác hệ quả lựa chọn nếu          │
│   game thành công (xem GDD 14.6)                                 │
└──────────────────────────────────────────────────────────┘
```

---

## Tổng kết số liệu (v1 → v3, xem `map20_dimensions.md` cho chi tiết)

| Đoạn | Tag pacing | Zombie (v1→v3) | Boss (không x12) | Ghi chú |
|---|---|---|---|---|
| 1. Hành lang sập | Sustain Peak (tẩu thoát) | 2 → **24** | 0 | Chướng ngại vật lý, không combat nặng |
| 2. Phòng TN cháy | Sustain Peak | 1 → **12** | 0 | — |
| 3. Boss cuối | **ĐỈNH ĐIỂM** | 0 → 0 | **1 (mạnh nhất)** | Không gian hẹp, áp lực thời gian |
| 4. Cửa thoát | Peak Fade cực ngắn | 0 → 0 | 0 | L20 (monologue text) |
| **Tổng** | | **3 → 36** | **1 boss cuối** | Không pickup mới toàn map |

---

## So sánh tổng thể Campaign 4 (Map 16-20, v1 → v3 sau x4 diện tích + x12 zombie thường)

| Map | Zombie (v1→v3) | Lính BRDF (không x12) | Chỉ huy (không x12) | Boss (không x12) |
|---|---|---|---|---|
| 16. Main Gatehouse | 5 → **60** | 2 | 0 | 0 |
| 17. Sector 3 Admin Block | 2 → **24** | 5 | 0 | 0 |
| 18. Subterranean Lab Alpha | 8→**96** (+3→**36** biến thể) | 0 | 0 | 0 |
| 19. Command Bunker | 2 → **24** | 5 | 1 (thoát) | 0 |
| 20. Final Breach | 3 → **36** | 0 | 0 | 1 (mạnh nhất) |
| **Tổng Campaign 4** | **20→240** (+3→**36** biến thể) | **12** | **1** | **1** |

---

## So sánh CUỐI CÙNG toàn bộ 4 Campaign (Map 1-20, v1 → v3 sau x4 diện tích + x12 zombie thường)

| Campaign | Zombie thường (v1→v3) | Đặc biệt (không x12) | Lính/Chỉ huy BRDF (không x12) | Boss (không x12) |
|---|---|---|---|---|
| 1. Ground Zero | 47 → **564** | 5 (Boomer 2, ẩn 3) | 0 | 1 |
| 2. Vành đai | 22 → **264** | 4 (Hunter/Smoker/Spitter/Jockey) | 0 | 1 |
| 3. Vùng công nghiệp | 23 → **276** | 1 (Witch) | 0 | 1 |
| 4. Cổng thành | 20 → **240** (+36 biến thể lab) | 0 | 13 | 1 (mạnh nhất) |
| **TỔNG TOÀN GAME** | **112 → 1344** (+36 biến thể lab) | **10** | **13** | **4 lần Tank + 1 chỉ huy + 1 boss cuối** |

**Đường cong thiết kế tổng thể:** Campaign 1 mở màn bằng SỐ LƯỢNG (đô thị
đông đúc, người chơi làm quen combat cơ bản) → Campaign 2 chuyển sang ĐA
DẠNG CƠ CHẾ (giới thiệu toàn bộ hệ thống địch đặc biệt) → Campaign 3 giữ
NHỊP ỔN ĐỊNH (campaign giữa, dồn sức cho cú twist) → Campaign 4 đổi hẳn
BẢN CHẤT ĐỊCH (con người thay vì chỉ zombie) kết hợp ÁP LỰC THỜI GIAN
(escape sequence) làm cao trào kết thúc — đúng nguyên tắc "mỗi campaign 1
bản sắc riêng, không lặp lại công thức", nhất quán xuyên suốt toàn bộ
20 map đã thiết kế.

**v3 (diện tích x4 + zombie thường x12) giữ nguyên đường cong này** —
mọi tỉ lệ giữa campaign/map không đổi, chỉ số tuyệt đối tăng đồng đều.
Toàn bộ địch đặc biệt/boss/lính/chỉ huy BRDF **giữ nguyên số lượng gốc
tuyệt đối** xuyên suốt cả 20 map — chi tiết đầy đủ ở từng
`mapNN_dimensions.md`.

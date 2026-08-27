# Map 1 — "Tòa nhà văn phòng" (Highrise)

Campaign 1 "Ground Zero", Map 1/5. Xem GDD.md mục 14.3 cho bối cảnh/lore.
Đây là bản mockup layout kỹ thuật dùng để dựng blockout thật (mục 17.13
bước 9 trong roadmap) — không phải bản vẽ nghệ thuật cuối cùng.

## Legend (dùng chung mọi tầng)

```
┌─┐  Ranh giới phòng/khu vực           ═══  Hành lang/lối đi chính
P    Player spawn (điểm bắt đầu mission)
Z    Zombie thường (spawn point)        Zx3  = 3 con cùng khu vực
X    Trigger sự kiện (báo cháy, cửa tự động...)
A    Ammo pickup (đạn dự trữ)           H    Med kit (bộ y tế)
L#   Audio log (số thứ tự log toàn game, xem GDD 14.3)
C    Cover/vật cản (bàn, xe, kệ, cột)
!    Environmental storytelling (xác chết, ghi chú, deco — không tương tác)
SR   Safe Room (armory/attachment, mục 14.7 — cuối map trước extract)
E    Extract Zone (điểm kết thúc mission, mục 17.1)
↑/↓  Cầu thang / thang lên-xuống giữa các tầng
     [Build-up] [Sustain Peak] [Peak Fade] [Relax]  = tag nhịp độ (mục 14.2)
```

⚠️ **Lưu ý về "checkpoint":** Map 1 KHÔNG có checkpoint giữa chừng — chết
giữa map chuyển sang trạng thái `Downed` (mục 17.1), không phục hồi tại
1 điểm cố định nào. "Thử lại" sau khi fail (mục 17.14) khởi động lại
TOÀN BỘ mission từ `P` (spawn), không phải từ 1 checkpoint giữa map. Điểm
gần nhất với khái niệm "checkpoint" là **Safe Room (SR)** ở cuối map —
đây là nơi duy nhất lưu tiến độ persistent (cấp-campaign, mục 14.7), qua
được SR nghĩa là map này coi như đã hoàn thành vĩnh viễn cho lần chơi đó.

---

## Tầng B1 — Gara để xe [Build-up]

```
┌──────────────────────────────────────────────────────────┐
│  KHU VỰC A — Chỗ nhóm trú ẩn qua đêm                       │
│                                                              │
│   P P P P        C C C     C C C     C C C                 │
│   (spawn 4        (xe hơi đậu — cover)                      │
│    người chơi)                                               │
│                                                              │
│                         Z                                   │
└───────────────────────────┬────────────────────────────────┘
                             ═══ (lối đi giữa các hàng xe)
┌───────────────────────────┴────────────────────────────────┐
│  KHU VỰC B — Khu đậu xe sâu hơn, tối hơn                     │
│                                                              │
│   C C C     C C C     C C C          A                      │
│                              Z                Z              │
│   !  (1 xác bảo vệ gara, đèn pin vẫn sáng — deco)             │
└───────────────────────────┬────────────────────────────────┘
                             │
                          ↑ Lên Sảnh chính (Ground Floor)
```

- **Mật độ:** 3 zombie thường, rải thưa (đúng tinh thần Build-up — cho
  người chơi làm quen control/aim trước khi vào cao trào).
- **Item:** 1 ammo pickup (khu B, tủ đồ bảo vệ gara).
- **Ghi chú thiết kế:** hàng xe đậu (`C`) làm cover tự nhiên, cũng là nơi
  zombie có thể núp giữa các hàng — line-of-sight bị chặn từng đoạn, phù
  hợp dạy người chơi "peek qua cover" ngay từ đầu.

---

## Tầng G — Sảnh chính (Main Lobby) [Build-up → chuyển Sustain Peak]

```
┌──────────────────────────────────────────────────────────┐
│                    SẢNH CHÍNH (không gian mở, trần cao)      │
│                                                              │
│   !!! (ba lô sơ tán bỏ lại la liệt — deco, không nhặt được)   │
│                                                              │
│         ~~~~~~~~ (mảng kính vỡ lớn, sảnh chính)               │
│                                                              │
│   [BIỂN "ĐIỂM TẬP KẾT" — bị gạch chéo sơn đỏ "ĐỪNG TIN"]      │
│                       (deco, environmental storytelling)      │
│                                                              │
│              Z          Z                                   │
│                                                              │
│   ┌─────────────┐                    ┌──────────────────┐   │
│   │ Quầy lễ tân  │                    │ Cầu thang bộ      │   │
│   │  A (đạn)     │                    │ xoắn ốc (trục     │   │
│   └─────────────┘                    │ chính lên các tầng)│   │
│                                        └─────────┬─────────┘   │
└──────────────────────────────────────────────────┼────────────┘
                                                   ↑ Lên F2
```

- **Mật độ:** 2 zombie, vẫn thưa — đây là đoạn cuối Build-up trước khi
  bước vào cầu thang xoắn ốc (trục chính nối toàn bộ các tầng phía trên).
- **Item:** 1 ammo pickup (quầy lễ tân).
- **Kiến trúc:** cầu thang xoắn ốc là **nút thắt bắt buộc** (chỉ 1 đường
  lên) — mọi tầng phía trên đều nối qua đây, không có đường vòng, đúng
  tinh thần "cầu thang bộ là đường duy nhất" trong briefing.

---

## Tầng F2 — Văn phòng tầng 2 [Build-up cuối, chuẩn bị Sustain Peak]

```
┌──────────────────────────────────────────────────────────┐
│  KHU VỰC BÀN LÀM VIỆC (cubicle mở)                            │
│                                                              │
│   C C C C C (dãy bàn làm việc — cover thấp, dễ bắn qua đầu)   │
│        Z          Z                                         │
│                                                              │
│   ┌───────────────────┐                                    │
│   │ Văn phòng riêng     │   L1 (Audio log #1 —                │
│   │  (cửa kính)         │   giọng bảo vệ, cảnh báo gara B2)     │
│   └───────────────────┘                                    │
│                                                              │
│   ┌───────────────────┐                                    │
│   │ Phòng tủ đồ nhân    │   A (ammo — "tủ đồ nhân viên"          │
│   │ viên                │   theo đúng GDD 14.3)                 │
│   └───────────────────┘                                    │
│                                                              │
│                              H  ← Med kit, NGAY TRƯỚC        │
│                                   khi lên cầu thang xoắn ốc    │
│                                   tiếp tục lên F3 (chuẩn bị    │
│                                   Sustain Peak — theo đúng     │
│                                   GDD 14.3)                    │
└───────────────────────────────────┬─────────────────────────┘
                                    ↑ Lên F3 (qua cầu thang xoắn ốc)
```

- **Mật độ:** 2 zombie giữa các cubicle.
- **Item:** `L1` (audio log #1), 1 ammo (tủ đồ nhân viên), 1 med kit ngay
  trước cầu thang lên F3 — đúng vị trí GDD đã chốt ("ngay trước đoạn cầu
  thang xoắn ốc").
- **Ghi chú:** đây là tầng cuối cùng người chơi có thể "thở" trước khi
  vào đoạn khó nhất map — bố trí đầy đủ đạn + máu tại đây là chủ đích.

---

## Tầng F3 — Văn phòng tầng 3 [SUSTAIN PEAK — cao trào chính]

```
┌──────────────────────────────────────────────────────────┐
│                                                              │
│   ┌──────────────┐                                          │
│   │ Phòng họp kính │  X ← TRIGGER: bước vào đây làm vỡ nốt    │
│   │  (đã nứt sẵn)   │     mảng kính còn lại → kích hoạt        │
│   └──────────────┘      báo cháy tự động toàn tầng           │
│                                                              │
│   ══════════════════ (hành lang chính, dài, mở)               │
│                                                              │
│   Zx3        Zx2         Zx3      ← Horde kéo tới từ NHIỀU    │
│                                     hướng cùng lúc sau khi X    │
│                                     kích hoạt (đây là           │
│                                     "sustain peak" thật —       │
│                                     liên tục, không phải 1 đợt) │
│                                                              │
│   ┌──────────────┐         ┌──────────────┐                 │
│   │ Văn phòng A    │         │ Văn phòng B    │                 │
│   │  Z              │         │  Z              │                 │
│   └──────────────┘         └──────────────┘                 │
└───────────────────────────────────┬─────────────────────────┘
                                    ↑ Lên F4 (qua cầu thang xoắn ốc)
```

- **Mật độ:** ~10 zombie (cao nhất map), spawn theo đợt khi `X` kích hoạt
  — không spawn hết ngay từ đầu tầng, mà dồn vào khi trigger, đúng thiết
  kế "event push" đã ghi trong GDD 14.2.
- **Không có item mới** ở tầng này — người chơi phải sống sót bằng những
  gì đã gom ở F2, tạo áp lực thật cho đúng vai trò "cao trào".
- **Kiến trúc:** hành lang dài + nhiều lối vào từ 2 văn phòng bên hông =
  zombie có thể áp sát từ 3 hướng cùng lúc quanh vị trí `X` — đúng tinh
  thần "bị bao vây" của sustain peak.

---

## Tầng F4 — Văn phòng tầng 4 [PEAK FADE]

```
┌──────────────────────────────────────────────────────────┐
│                                                              │
│   Z                    Z                                    │
│   (zombie lẻ tẻ còn sót lại từ đợt horde F3, không có         │
│    đợt spawn mới — mật độ giảm dần đúng "peak fade")          │
│                                                              │
│   ┌───────────────────────┐                                │
│   │ Phòng lưu trữ tài liệu   │                                │
│   │  (trống, không item)     │                                │
│   └───────────────────────┘                                │
│                                                              │
│   ══════ (hành lang ngắn, dẫn thẳng ra cửa thoát hiểm)         │
└───────────────────────────────────┬─────────────────────────┘
                                    ↑ Lên Mái / Lối thoát hiểm
```

- **Mật độ:** 2 zombie, giảm mạnh so với F3 — không spawn mới.
- **Ghi chú:** tầng ngắn, có chủ đích — vai trò duy nhất là "hạ nhiệt"
  trước khi ra khỏi tòa nhà, không cần nội dung phức tạp.

---

## Mái nhà & Lối thoát hiểm (Fire Escape) [RELAX + Safe Room + Extract]

```
┌──────────────────────────────────────────────────────────┐
│                    MÁI NHÀ (ngoài trời)                       │
│                                                              │
│                    (không có zombie — an toàn)                │
│                                                              │
│              ┌─────────────────────────┐                    │
│              │   SAFE ROOM (SR)          │                    │
│              │   - Armory (chọn vũ khí)   │                    │
│              │   - Bàn gắn attachment      │                    │
│              │   - Lưu tiến độ campaign    │                    │
│              │     (mục 14.7)             │                    │
│              └────────────┬────────────┘                    │
│                            │                                  │
│                     Thang cứu hỏa ngoài tòa nhà                │
│                            │ (đi xuống mặt ngoài                │
│                            │  tòa nhà, ngoài trời)              │
│                            ↓                                  │
└────────────────────────────┼──────────────────────────────────┘
                              │
                    ┌─────────┴─────────┐
                    │   E (Extract Zone) │
                    │   Đường phố —       │
                    │   chuyển sang        │
                    │   Map 2 "Phố mua     │
                    │   sắm"               │
                    └───────────────────┘
```

- **Không có zombie** — đúng tag `[Relax]`, chỉ còn environmental
  storytelling (không vẽ chi tiết ở đây, xem GDD 14.3 cho nội dung).
- **Safe Room:** điểm neo UI cho hệ thống attachment (mục 17.6) — người
  chơi có thể đổi loadout trước khi bước tiếp.
- **Extract Zone:** áp dụng đúng luật "chờ cả nhóm" (mục 17.1) — đếm
  ngược, cần toàn bộ người chơi còn sống (`Alive`, không phải `Dead`)
  đứng trong `E` khi hết giờ mới hoàn thành mission.

---

## Tổng kết số liệu (dùng để implement `ZOMBIE_SPAWNS`-tương-đương cho map)

| Tầng | Tag pacing | Số zombie | Ammo | Med kit | Audio log |
|---|---|---|---|---|---|
| B1 Gara | Build-up | 3 | 1 | 0 | — |
| G Sảnh | Build-up | 2 | 1 | 0 | — |
| F2 Văn phòng | Build-up | 2 | 1 | 1 | L1 |
| F3 Văn phòng | **Sustain Peak** | ~10 (spawn theo đợt) | 0 | 0 | — |
| F4 Văn phòng | Peak Fade | 2 | 0 | 0 | — |
| Mái/Thoát hiểm | Relax | 0 | 0 | 0 | — |
| **Tổng** | | **~19 zombie** | **3** | **1** | **1** |

Mật độ tăng dần theo tầng cho tới đỉnh điểm F3, đúng đường cong pacing đã
thiết kế ở GDD mục 14.2 — có thể dùng bảng này trực tiếp làm input cho
hàm sinh spawn point khi dựng map thật (tương tự cách `zombie_spawn_grid`
hiện có sinh `ZOMBIE_SPAWNS`, nhưng theo cụm/tầng thay vì lưới đều).

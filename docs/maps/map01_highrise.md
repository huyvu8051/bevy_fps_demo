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

⚠️ **v2 — diện tích mỗi tầng x4 (xem `map01_dimensions.md`):** mật độ
zombie mỗi tầng cũng nhân x4 theo đúng số liệu ở đó, để giữ nguyên cảm
giác đông đúc/áp lực dù không gian rộng hơn (không loãng mật độ). Số
trong file này đã cập nhật theo v2 — số cũ (trước x4) không còn hiệu
lực.

⚠️ **v3 — tăng thêm x3 (→ ~228 zombie toàn map) + spawn theo nhịp
(wave) ở MỌI tầng, không chỉ F3:** mỗi tầng giờ có 1 bảng "Nhịp spawn"
riêng — zombie không xuất hiện hết cùng lúc mà chia đợt theo trigger
(vào khu vực / nhặt item / tiến gần cầu thang...), tăng dần độ dồn dập
ngay cả ở tầng Build-up, giữ cảm giác "luôn có gì đó đang tới" xuyên
suốt map thay vì chỉ dồn hết vào F3. Số zombie trong ASCII/bảng kiểm kê
là **tổng cộng dồn tất cả đợt** của tầng đó.

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
│                    Z      Z      Z      Z                   │
└───────────────────────────┬────────────────────────────────┘
                             ═══ (lối đi giữa các hàng xe)
┌───────────────────────────┴────────────────────────────────┐
│  KHU VỰC B — Khu đậu xe sâu hơn, tối hơn                     │
│                                                              │
│   C C C     C C C     C C C          A                      │
│   Z    Z    Z    Z    Z    Z    Z    Z                       │
│   !  (1 xác bảo vệ gara, đèn pin vẫn sáng — deco)             │
└───────────────────────────┬────────────────────────────────┘
                             │
                          ↑ Lên Sảnh chính (Ground Floor)
```

- **Mật độ (v3, x3 nữa so với v2):** **36 zombie** tổng (v2: 12).
- **Nhịp spawn (wave):**

  | Đợt | Trigger | Số zombie | Vị trí |
  |---|---|---|---|
  | 1 | Mission start (spawn ngay, thấy được từ P) | 8 | Khu A, giữa hàng xe |
  | 2 | Người chơi bước qua aisle vào Khu B | 10 | Khu B, đầu hàng xe |
  | 3 | Người chơi tới gần tủ đồ (ammo) cuối Khu B | 18 | Khu B, tràn từ sau hàng xe + cửa gara — mini-peak khép lại B1 trước khi lên Sảnh |

  Tổng 8+10+18 = 36. Đợt 3 cố ý dồn dập hơn 2 đợt trước dù vẫn gắn tag
  `[Build-up]` — tạo cảm giác "chưa kịp thở đã bị vây" ngay từ tầng đầu.
- **Item:** 1 ammo pickup (khu B, tủ đồ bảo vệ gara) — giữ nguyên số
  lượng item, chỉ zombie tăng.
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
│         Z    Z    Z         Z    Z                          │
│                                                              │
│   ┌─────────────┐               Z          ┌──────────────────┐   │
│   │ Quầy lễ tân  │                    │ Cầu thang bộ      │   │
│   │  A (đạn)     │                    │ xoắn ốc (trục     │   │
│   └─────────────┘                    │ chính lên các tầng)│   │
│                                        └─────────┬─────────┘   │
└──────────────────────────────────────────────────┼────────────┘
                                                   ↑ Lên F2
```

- **Mật độ (v3, x3 nữa so với v2):** **24 zombie** tổng (v2: 8).
- **Nhịp spawn (wave):**

  | Đợt | Trigger | Số zombie | Vị trí |
  |---|---|---|---|
  | 1 | Bước vào Sảnh từ dốc B1 | 6 | Rải rác giữa sảnh mở |
  | 2 | Tiến quá nửa sảnh (hướng cầu thang) | 8 | Từ 2 bên hông sảnh |
  | 3 | Tới gần quầy lễ tân nhặt ammo | 10 | Tràn từ sau quầy + mảng kính vỡ — đẩy nhịp lên gần Sustain trước khi vào cầu thang |

  Tổng 6+8+10 = 24. Đúng tinh thần "Build-up → chuyển Sustain Peak" của
  tầng này — đợt 3 là điểm bắt đầu cảm nhận "cao trào sắp tới".
- **Item:** 1 ammo pickup (quầy lễ tân) — giữ nguyên.
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
│   Z   Z   Z   Z   Z   Z   Z   Z (rải dọc hàng cubicle+lối đi) │
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

- **Mật độ (v3, x3 nữa so với v2):** **24 zombie** tổng (v2: 8).
- **Nhịp spawn (wave):**

  | Đợt | Trigger | Số zombie | Vị trí |
  |---|---|---|---|
  | 1 | Bước vào khu cubicle | 8 | Rải giữa hàng cubicle/lối đi |
  | 2 | Mở cửa văn phòng riêng hoặc phòng tủ đồ | 8 | Tràn ra từ phòng vừa mở |
  | 3 | Tiến gần cầu thang lên F3 (sau khi nhặt med kit) | 8 | Hội tụ quanh chân cầu thang — "đợt cuối trước cao trào" |

  Tổng 8+8+8 = 24. Đợt 3 cố ý đặt ngay trước lối lên F3 để người chơi
  bước vào Sustain Peak trong trạng thái vừa đánh xong, không phải nghỉ
  ngơi hoàn toàn.
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
│   Zx24(đợt1)  Zx24+24(đợt2,4) Zx24(đợt3,5) ← Horde 5 đợt LIÊN TỤC │
│                                     (t=0/6/12/18/24s), bao vây từ  │
│                                     3-4 hướng cùng lúc sau khi X   │
│                                     kích hoạt — "sustain peak"     │
│                                     thật, không phải 1 đợt duy nhất│
│                                                              │
│   ┌──────────────┐         ┌──────────────┐                 │
│   │ Văn phòng A    │         │ Văn phòng B    │                 │
│   │  Zx12(đợt5)     │         │  Zx24(đợt3)     │                 │
│   └──────────────┘         └──────────────┘                 │
└───────────────────────────────────┬─────────────────────────┘
                                    ↑ Lên F4 (qua cầu thang xoắn ốc)
```

- **Mật độ (v3, x3 nữa so với v2):** **120 zombie** tổng (v2: 40) — cao
  nhất map, đúng vai trò Sustain Peak.
- **Nhịp spawn (wave) — 5 đợt liên tục, cách nhau 6 giây, tổng ~24-30
  giây áp lực không ngắt quãng:**

  | Đợt | Thời điểm | Số zombie | Vị trí |
  |---|---|---|---|
  | 1 | `X` kích hoạt (t=0s) | 24 | Từ phòng họp kính (Tây) |
  | 2 | t=+6s | 24 | Từ 2 đầu hành lang, hội tụ vào giữa |
  | 3 | t=+12s | 24 | Tràn từ Văn phòng B (Đông) |
  | 4 | t=+18s | 24 | Tăng viện đợt 2 dọc hành lang (đúng nghĩa "sustain" — không phải 1 đợt duy nhất) |
  | 5 | t=+24s | 24 | Từ Văn phòng A + số còn lại dồn về hành lang trước khi tầng chuyển Peak Fade |

  Tổng 24x5 = 120. Không gian 48x32 đủ chỗ dàn trải mà không đặc nghẹt,
  nhưng vẫn giữ được thế "bị bao vây từ nhiều hướng" đúng thiết kế gốc
  — chỉ kéo dài thời gian áp lực thay vì dồn 1 cục.
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
│   Z    Z    Z       Z    Z    Z    Z    Z                    │
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

- **Mật độ (v3, x3 nữa so với v2):** **24 zombie** tổng (v2: 8), vẫn
  giảm mạnh so với F3 (120) — tỉ lệ giảm giữ nguyên đúng bản gốc (~1/5).
- **Nhịp spawn (wave) — vẫn tapering, KHÔNG dồn dập như các tầng khác
  (đúng vai trò Peak Fade):**

  | Đợt | Trigger | Số zombie | Vị trí |
  |---|---|---|---|
  | 1 | Đặt sẵn khi vào tầng (stragglers từ horde F3) | 16 | Rải khắp tầng |
  | 2 | +10s sau khi vào tầng (trickle nhẹ, không phải combat mới) | 8 | Từ phòng lưu trữ tài liệu |

  Tổng 16+8 = 24. Đợt 2 cố ý thưa/chậm — vai trò duy nhất của tầng này
  là hạ nhiệt, không tạo thêm áp lực thật.
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

## Tổng kết số liệu (v3 — x3 nữa trên v2 + spawn theo nhịp mọi tầng, dùng để implement `ZOMBIE_SPAWNS`-tương-đương cho map)

| Tầng | Tag pacing | Số zombie (v1 → v2 → v3) | Số đợt spawn | Ammo | Med kit | Audio log |
|---|---|---|---|---|---|---|
| B1 Gara | Build-up | 3 → 12 → **36** | 3 đợt | 1 | 0 | — |
| G Sảnh | Build-up | 2 → 8 → **24** | 3 đợt | 1 | 0 | — |
| F2 Văn phòng | Build-up | 2 → 8 → **24** | 3 đợt | 1 | 1 | L1 |
| F3 Văn phòng | **Sustain Peak** | ~10 → 40 → **120** | 5 đợt, mỗi 6s | 0 | 0 | — |
| F4 Văn phòng | Peak Fade | 2 → 8 → **24** | 2 đợt (thưa) | 0 | 0 | — |
| Mái/Thoát hiểm | Relax | 0 → 0 → 0 | — | 0 | 0 | — |
| **Tổng** | | **~19 → 76 → 228 zombie** | 16 đợt spawn | **3** | **1** | **1** |

Mật độ tăng dần theo tầng cho tới đỉnh điểm F3, đúng đường cong pacing đã
thiết kế ở GDD mục 14.2 — tỉ lệ giữa các tầng giữ nguyên qua cả 3 bản
(v1→v2 x4 theo diện tích, v2→v3 x3 để tăng cường độ bắn). Ammo/med
kit/audio log **giữ nguyên số lượng** qua mọi bản — đây là item đặt tay
theo nhịp game, không phụ thuộc diện tích/số zombie.

**Spawn theo nhịp (mới ở v3):** mỗi tầng giờ chia 2-5 đợt gắn trigger
riêng (xem bảng "Nhịp spawn" trong từng mục tầng ở trên) thay vì spawn
hết 1 lần khi vào tầng — kể cả các tầng Build-up cũng có đợt cuối dồn
dập hơn ngay trước khi sang tầng tiếp theo, giữ cảm giác "luôn có áp
lực mới" xuyên suốt map, không chỉ dồn hết vào F3. Toạ độ spawn chi
tiết (theo cụm, không liệt kê từng điểm ở số lượng lớn): xem
`map01_dimensions.md` mục 3. Có thể dùng bảng "Nhịp spawn" trực tiếp
làm input cho hệ thống spawn-theo-thời-gian/trigger khi dựng map thật
(khác cách `zombie_spawn_grid` hiện có — spawn 1 lần theo lưới đều —
cần thêm cơ chế đợt/timer mới để hỗ trợ bảng này).

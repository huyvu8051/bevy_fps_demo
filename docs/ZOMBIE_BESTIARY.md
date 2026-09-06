# Zombie Bestiary — thường + đặc biệt + archetype môi trường

Bổ sung cho GDD.md mục 15 (content scope) và `docs/characters/zombie.md`
(hiện đang là stub). File này là **nguồn tham chiếu duy nhất** cho "map
này có những loại zombie gì" — dùng khi viết brief AI-gen model và khi
code spawn table cho từng map.

## 3 tầng zombie (không nhầm lẫn khi đọc bảng dưới)

1. **Zombie thường (đã có, code hiện tại)** — 1 AI đuổi + cận chiến duy
   nhất (`crates/shared/src/zombie.rs`), 2 bộ mesh (`ZombieKind::Classic`/
   `Meshy`) không mang ý nghĩa gì ngoài đa dạng hình ảnh.
2. **Archetype môi trường (MỚI, file này)** — vẫn dùng CHUNG khâu di
   chuyển/đuổi (`move_zombies`) của zombie thường, KHÔNG viết AI framework
   mới. Khác nhau ở:
   - **Ngoại hình/trang phục** theo bối cảnh map (công nhân, nhân viên
     văn phòng, lính Meridian...) — mỗi archetype có **2-3 variant** (đổi màu
     trang phục/dáng người qua prompt AI-gen khác nhau, cùng 1 rig).
   - **Hệ số** `Health`/tốc độ so với baseline (vd lính gác dày hơn,
     người già yếu hơn) — dùng lại đúng field `Health`/`ZOMBIE_WALK_
     SPEED`/`ZOMBIE_RUN_SPEED` đã có, chỉ nhân hệ số theo archetype lúc
     spawn.
   - **1 "behavior trait"** phù hợp tính cách (hung hãn/lỳ đòn/bầy đàn/
     lang thang/đờ đẫn/cảnh giác cao — xem mục "Hành vi riêng cho
     archetype" dưới) — vẫn là tham số chỉnh trên `move_zombies` có sẵn,
     KHÔNG phải system AI riêng cho từng archetype.
   - Mục đích: mỗi map trông có "dân cư riêng" (cả hình ảnh lẫn cảm giác
     hành vi) thay vì cùng 1 hình ảnh + hành vi lặp lại xuyên suốt 20 map.
3. **Zombie đặc biệt đặt tên (đã có khung, GDD mục 15/17.5/17.18)** —
   Bloater/Stalker/Strangler/Spewer/Leech (kỹ năng riêng, cần AI mới) + Brute
   (boss) + Wailer-style (special encounter). KHÔNG lặp lại ở đây, xem GDD
   trực tiếp. Archetype (tầng 2) không thay thế tầng này — 1 map có thể có
   cả archetype VÀ 1 con đặc biệt đặt tên (vd Map 3 vừa có "Hành khách
   tàu điện" archetype vừa là map giới thiệu Bloater).

## Animation: dùng chung vs. riêng (quyết định 2026-09-01 — tối ưu, không
## generate thừa)

Animation là chi phí AI-gen thật (mỗi clip = 1 lượt gen + chỉnh), khác
model/texture (rẻ hơn nhiều) — nên **tối đa hoá dùng chung**, chỉ thêm clip
mới khi cơ chế BẮT BUỘC cần, không thêm vì "cho đẹp".

**Vì sao dùng chung được:** mọi zombie trong game (archetype lẫn đặc
biệt) đều **hình người, cùng 1 trong 2 khung xương** (`Classic`/`Meshy`)
— animation clip gắn theo TÊN BONE (kiểu rig Mixamo, xem
`CHARACTER_IMPORT_CHECKLIST.md` mục 1 — hệ hitbox `wire_zombie_hitbox_
bones` hiện tại cũng đang dựa đúng nguyên tắc này), không gắn theo model
— cùng tên bone thì 1 clip chạy đúng trên MỌI model dùng chung khung
xương đó, bất kể hình dạng/texture khác nhau thế nào. Điều kiện duy nhất:
model archetype/đặc biệt mới phải **export đúng tên bone Mixamo-style**
(hoặc rename lại trong Blender) khớp `Classic`/`Meshy` đã có — không phải
tự nhiên miễn phí, nhưng là 1 bước rẻ (đổi tên bone), không phải gen lại
animation.

- **33 archetype (tầng 2) — animation dùng chung 100%; hành vi dùng chung
  khâu di chuyển + 1 trait/archetype (11 trait tổng, xem mục "Hành vi
  riêng cho archetype" dưới).** `move_zombies` vẫn là hàm duy nhất mọi
  archetype
  chạy qua — trait chỉ là tham số (chase range/tốc độ/độ trễ) đọc theo
  archetype lúc chạy, không phải system/AI riêng từng loại. **0 clip
  animation mới cho toàn bộ 96 model archetype** — trait không cần
  animation riêng (hung hãn/lỳ đòn/bầy đàn... đều thể hiện qua khoảng
  cách/thời điểm aggro, không qua 1 clip mới).
- **10 loại đặc biệt/boss (tầng 3, gồm cả 3 loại tham khảo ngoài L4D) —
  hành vi CHUNG (di chuyển/aggro, tái dùng `move_zombies`) + hành vi
  RIÊNG (đúng 1 system/loại — xem bảng kỹ năng ở mục "Thiết kế đầy đủ 4
  loại đặc biệt còn lại"/"3 loại đặc biệt mới" trên: Tongue/Spit/Pounce/
  Latch/Charge/Scream, riêng Kẻ Rình dùng lại nguyên cơ chế phát hiện âm
  thanh — không cần clip tấn công riêng, chỉ cần Bite như zombie thường),
  animation cũng theo đúng tỉ lệ đó — dùng chung bộ 5 clip nền, chỉ thêm
  TỐI ĐA 1 clip riêng/loại** (không phải 1 bộ animation lẫn 1 bộ AI riêng
  từ đầu):

| Loại | Dùng chung (Idle/Walk/Run/Crawl) | Clip riêng cần thêm | Tận dụng lại clip nền cho phần còn lại |
|---|---|---|---|
| Nghẹt Thở (Bloater) | ✅ | **0 mới** — nổ là VFX + despawn, không phải animation | — |
| Dây Thối (Strangler) | ✅ | 1: **Tongue Attack** (thay Bite khi ở xa) | Bite vẫn dùng khi bị kéo tới gần |
| Kẻ Phun Độc (Spewer) | ✅ | 1: **Spit Attack** (thay Bite khi ở xa) | — |
| Kẻ Rình Rập (Stalker) | ✅ | 1: **Pounce** (one-shot nhảy tới) | Sau khi ghim xuống đất, dùng lại **Bite** loop luôn — không cần clip "cắn khi ghim" riêng |
| Kẻ Cưỡi (Leech) | ✅ | 1: **Latch** (one-shot bám lên lưng) | Lúc đang cưỡi không cần clip riêng — camera/controller là phần hiện, không phải animation nó |
| Lôi Đình (Brute) | ✅ | **0 mới** — dùng lại `Bite` làm đòn đấm (đổi hitbox/dame trong code, không đổi clip) | Walk/Run cũng dùng thẳng bản zombie thường, chỉ tăng scale model |
| Người Than Khóc (Wailer) | ❌ (không Walk/Run — đứng yên) | 2: **Cower** (idle loop khi yên) + **Charge** (one-shot khi nổi giận, có thể tái dùng Run tốc độ cao thay vì clip mới nếu chấp nhận bớt kịch tính) | — |
| Kẻ Húc (Rammer) | ✅ | 1: **Slam** (one-shot đập nạn nhân xuống đất, lặp lại) | Pha lao tới dùng lại **Run** nguyên bản (chỉ nhanh hơn qua code, không cần clip riêng) |
| Kẻ Rình (Stray) | ✅ | **0 mới** — mù nên không cần tư thế "ngắm"/"rình" đặc biệt, chỉ đi tới nguồn âm rồi Bite như thường | — |
| Kẻ Hú (Howler) | ✅ | 1: **Scream** (one-shot, ngửa đầu hú) | Bỏ chạy sau khi hú dùng lại **Run** nguyên bản |

**Tổng clip mới cần gen cho TOÀN BỘ hệ thống đặc biệt (10 loại): 7**
(Tongue, Spit, Pounce, Latch, Cower, Slam, Scream — Wailer's Charge vẫn để
tuỳ chọn/bỏ được) — không phải 10 loại × 1 bộ animation riêng, và **3
trong 10 loại (Nghẹt Thở, Lôi Đình, Kẻ Rình) cần đúng 0 clip mới**. Mỗi
clip mới chỉ cần gen **1 lần trên rig `zombie_meshy.glb`**
(đúng khuyến nghị hiện có của `ZOMBIE_ANIMATION_PLAN.md`), dùng chung cho
mọi variant của loại đó (vd cả 2 variant "Người bán hàng đêm"/"Bảo vệ
chợ" của Dây Thối đều phát chung 1 clip Tongue Attack, chỉ khác model).

## Nguyên tắc thiết kế archetype (để không vượt ngân sách solo dev)

- **Tái dùng archetype giữa các map cùng bối cảnh** thay vì tạo mới cho
  mỗi map — vd "Nhân viên bảo vệ/Lính Meridian" xuất hiện lặp lại ở nhiều map
  khác nhau (Map 1, 5, 10, 16, 19, 20) vì hợp lý với lore (Meridian đóng quân
  khắp thành phố) và tiết kiệm asset. Chỉ tạo archetype thật mới khi bối
  cảnh map đủ khác biệt.
- **Không tạo archetype trẻ em** — theo đúng convention mọi game zombie
  chủ lưu (L4D, Dying Light...) tránh nội dung nhạy cảm; Map 7 (trường
  tiểu học) dùng "học sinh cấp 3/giáo viên", không dùng trẻ nhỏ.
- Mỗi map: **4 archetype** (tăng gấp đôi từ mức 2 ban đầu — quyết định
  2026-09-01 — 2 archetype riêng theo bối cảnh map + 2 archetype dùng
  chung xuyên suốt campaign, xem mục "Archetype xuyên-campaign" dưới).
- Mỗi archetype: **2-3 variant** (palette/trang phục khác nhau qua prompt
  — variant KHÔNG đổi AI/stat, chỉ đổi model).

---

## Roster archetype (dùng chung nhiều map)

| Archetype | Base kind | Hệ số Health/tốc độ | Variant (2-3) | Xuất hiện ở map |
|---|---|---|---|---|
| **Nhân viên văn phòng** | Classic | 1.0 / 1.0 (baseline) | Nam vest xộc xệch · Nữ áo sơ mi rách · Thực tập sinh đeo thẻ | M1, M17 |
| **Bảo vệ / Lính Meridian** | Meshy | 1.3 / 1.0 (dày hơn, không nhanh hơn) | Bảo vệ toà nhà · Lính Meridian thường · Sĩ quan Meridian (huy hiệu) | M1, M5, M10, M14, M16, M19, M20 |
| **Khách mua sắm** | Classic | 1.0 / 1.0 | Gia đình (túi đồ) · Thiếu niên (tai nghe còn đeo) · Người già (xe đẩy) | M2 |
| **Hành khách tàu điện** | Meshy | 0.9 / 1.1 (gầy hơn, nhanh hơn chút — kẹt dưới hầm lâu) | Hành khách thường · Nhân viên soát vé · Công nhân vệ sinh đường ray | M3 |
| **Người vô gia cư** | Classic | 0.9 / 1.0 | Trú ẩn dưới ga tàu · Nhặt ve chai quanh sân ga | M3 |
| **Nạn nhân giao thông** | Classic | 1.0 / 0.9 (chậm hơn — chấn thương) | Tài xế cháy sém · Hành khách xe khách · Lính cứu hộ giao thông | M4 |
| **Cảnh sát giao thông** | Meshy | 1.2 / 1.0 | CSGT tuần tra (áo phản quang) · CSGT chỉ huy điểm ùn tắc | M4 |
| **Người tị nạn** | Classic | 0.8 / 1.0 (kiệt sức, chậm hơn hẳn) | Chờ tại điểm hẹn (M5) · Bị từ chối ở checkpoint (M10) · Cố vượt hàng rào (M15) · Xếp hàng chờ vào cổng chính (M16) | M5, M10, M15, M16 |
| **Cư dân khu mẫu** | Meshy | 1.0 / 1.0 | Chủ nhà (áo choàng tắm) · Người giúp việc · Bảo vệ khu dân cư | M6 |
| **Nhân viên bất động sản** | Classic | 1.0 / 1.0 | Môi giới nam (com-lê) · Môi giới nữ (bảng tên còn đeo) | M6 |
| **Giáo viên/Học sinh cấp 3** | Classic | 0.9 / 1.1 | Giáo viên · Học sinh cấp 3 · Bảo vệ trường | M7 |
| **Phụ huynh đón con** | Meshy | 1.0 / 1.0 | Phụ huynh lo lắng (vẫn cầm ô/dù) · Tài xế xe buýt trường | M7 |
| **Người đi công viên** | Meshy | 1.0 / 1.0 | Người chạy bộ · Người câu cá · Người dắt chó (dây xích đứt) | M8 |
| **Nhân viên công viên** | Classic | 1.0 / 0.9 | Kiểm lâm viên · Nhân viên vệ sinh công viên | M8 |
| **Người chợ đêm** | Classic | 1.0 / 1.0 | Người bán hàng (tạp dề) · Khách mua sắm đêm · An ninh chợ | M9 |
| **Người giao hàng đêm** | Meshy | 1.0 / 1.2 (nhanh hơn — vẫn theo quán tính giao hàng gấp) | Shipper xe máy (mũ bảo hiểm còn đội) · Người bán rong dạo | M9 |
| **Công nhân dệt** | Meshy | 1.1 / 0.9 | Công nhân dệt (khẩu trang bụi) · Quản đốc · Thợ sửa máy | M11 |
| **Công nhân cảng** | Classic | 1.2 / 0.9 (áo phao/đồ bảo hộ nặng) | Công nhân bốc dỡ (áo phản quang) · Lái cần cẩu · Bảo vệ cảng | M12 |
| **Đội an ninh tư nhân** | Meshy | 1.2 / 1.0 | Bảo vệ nhà máy dệt (M11) · Bảo vệ cảng (M12) · Bảo vệ kho lạnh (M13) | M11, M12, M13 |
| **Nạn nhân hoá chất** | Meshy | 0.8 / 1.0 (rữa da nặng, nhìn kinh dị nhất map) | Công nhân kho lạnh (đồ bảo hộ rách) · Kỹ thuật viên hoá chất · Không rõ danh tính (biến dạng nặng) | M13 |
| **Kỹ thuật viên nhà máy lọc** | Classic | 1.0 / 1.0 | Kỹ thuật viên (đồ liền thân) · Giám sát ca · Lính cứu hoả nhà máy | M14 |
| **Nhà khoa học/Nhân viên lab** | Meshy | 0.9 / 1.0 | Nhà khoa học (áo blouse) · Kỹ thuật viên lab · An ninh phòng lab | M17 (khu hành chính), M18 |
| **Đối tượng thí nghiệm** | Meshy | 0.7 / 1.1 (gầy yếu nhưng lết nhanh bất thường) | Bị trói trên bàn (dây đứt lủng lẳng) · Mặc đồ bệnh nhân lem máu | M18 |
| **Nhân viên vận hành chỉ huy** | Classic | 1.0 / 1.0 | Sĩ quan trực chỉ huy · Kỹ thuật viên liên lạc (tai nghe còn gắn) | M19 |
| **Người sống sót cuối cùng** | Classic | 0.9 / 1.1 | Mang theo balo trốn chạy dở dang · Bị thương băng bó dở dang | M20 |

### Archetype xuyên-campaign (mới, tăng gấp đôi mật độ/map — quyết định 2026-09-01)

Thay vì tạo mới 40 archetype lẻ (không đủ ngân sách), thêm **đúng 2
archetype dùng chung cho cả 5 map/campaign**, mỗi archetype 5 variant
(1 variant/map trong campaign đó, giữ lý do xuất hiện hợp bối cảnh từng
map) — cách này nhân đôi mật độ archetype/map (2 → 4) mà chỉ cần thêm 8
archetype mới cho toàn bộ 20 map, đúng tinh thần "Người tị nạn" đã làm.

| Archetype | Base kind | Hệ số | Variant theo map (5) | Campaign |
|---|---|---|---|---|
| **Người đi đường** | Classic | 1.0 / 1.0 | Trước sảnh toà nhà (M1) · Trước cửa hàng (M2) · Trước cửa ga tàu (M3) · Lề đường cao tốc (M4) · Quảng trường điểm hẹn (M5) | 1 Ground Zero |
| **Nhân viên vệ sinh đô thị** | Meshy | 0.9 / 1.0 | Lao công toà nhà (M1) · Lao công trung tâm mua sắm (M2) · Công nhân vệ sinh ga tàu (M3) · Đội dọn tai nạn cao tốc (M4) · Lao công quảng trường (M5) | 1 Ground Zero |
| **Cư dân vành đai** | Classic | 1.0 / 1.0 | Cư dân khu mẫu khác (M6) · Phụ huynh sống gần trường (M7) · Người dân quanh hồ (M8) · Cư dân gần chợ đêm (M9) · Cư dân gần checkpoint (M10) | 2 Vành đai |
| **Nhân viên dịch vụ công cộng** | Meshy | 1.0 / 1.0 | Bưu tá khu mẫu (M6) · Nhân viên y tế học đường (M7) · Nhân viên quản lý công viên cấp cao (M8) · Nhân viên thuế chợ đêm (M9) · Nhân viên hành chính checkpoint (M10) | 2 Vành đai |
| **Công nhân thời vụ** | Classic | 1.0 / 0.9 | Thời vụ nhà máy dệt (M11) · Thời vụ bốc dỡ cảng (M12) · Thời vụ kho lạnh (M13) · Thời vụ nhà máy lọc (M14) · Thời vụ hàng rào an ninh (M15) | 3 Vùng công nghiệp |
| **Nhân viên vận tải/logistics** | Meshy | 1.1 / 1.0 | Lái xe tải nhà máy dệt (M11) · Điều phối container (M12) · Lái xe đông lạnh (M13) · Lái xe bồn nhà máy lọc (M14) · Lái xe tuần tra hàng rào (M15) | 3 Vùng công nghiệp |
| **Nhân viên hậu cần Meridian** | Meshy | 1.1 / 1.0 | Hậu cần cổng chính (M16) · Hậu cần khu hành chính (M17) · Hậu cần phòng lab (M18) · Hậu cần boong-ke (M19) · Hậu cần điểm cuối (M20) | 4 Cổng thành |
| **Người bị giam giữ** | Classic | 0.7 / 0.9 (yếu, từng bị giam) | Bị giam ở cổng chính (M16) · Bị thẩm vấn khu hành chính (M17) · Đối tượng theo dõi ở lab (M18) · Bị giam ở boong-ke (M19) · Bị giam ở điểm cuối (M20) | 4 Cổng thành |

**Tổng mới:** 25 + 8 = **33 archetype**, ~96 model zombie riêng biệt (variant
tổng) cho toàn bộ 20 map — cộng **10 loại đặc biệt/boss** (7 kiểu L4D +
3 loại tham khảo ngoài, xem mục riêng dưới) = **~120 loại địch phân biệt
bằng mắt**. Mọi map giờ đều có **đúng 4** archetype (gấp đôi
mức 2 trước đó), vẫn nằm trong khả năng AI-gen 1 người làm — mỗi variant
chỉ là 1 biến thể prompt của 2 rig `zombie.glb`/`zombie_meshy.glb` đã có,
không phải rig mới. "Người tị nạn"/8 archetype xuyên-campaign ở trên là
các ngoại lệ có chủ đích với 4-5 variant thay vì 2-3 — mỗi cái đều có 1
mạch lý do xuyên suốt (Meridian bỏ mặc dân thường, hoặc đơn giản là "vai trò
tồn tại ở mọi map trong campaign" như lao công/bưu tá/tài xế), không
phải chỉ tạo cho đủ số.

---

## Hành vi riêng cho archetype (quyết định 2026-09-01 — sửa lại: KHÔNG
## còn 100% dùng chung nữa)

Trước đó file này ghi archetype "dùng chung hành vi 100%" — sửa lại:
archetype vẫn dùng CHUNG khâu di chuyển/đuổi bên dưới (`move_zombies`,
đúng 1 hàm cho mọi zombie), nhưng thêm **1 "behavior trait"** cho mỗi
archetype, chọn theo đúng ngoại hình/tính cách của nó. Trait KHÔNG phải
system AI mới — chỉ là **tham số chỉnh trên cùng hệ thống có sẵn** (aggro
range, tốc độ, độ trễ phản ứng...), giữ đúng tinh thần "tối ưu, không
design/code quá nhiều" đã thống nhất: 33 archetype chỉ cần **11 trait
dùng chung** (không phải 33 hành vi riêng biệt).

### 11 trait (áp dụng qua tham số, không phải code AI mới)

| Trait | Cơ chế | Field/hệ thống tái dùng |
|---|---|---|
| **Bình thường** | Không đổi gì — baseline | — |
| **Hung hãn** | Phát hiện từ xa hơn (`ZOMBIE_CHASE_RANGE` × 1.5) + tốc lên +50% trong 1s ngay lúc vừa aggro | `ZOMBIE_CHASE_RANGE` nhân hệ số + 1 timer nhỏ kiểu `KnockbackVelocity` đã có |
| **Lỳ đòn** | Khó phát hiện hơn (chase range × 0.6) nhưng đã aggro thì KHÔNG bỏ đuổi dù người chơi thoát khỏi tầm | Bỏ qua bước "hết tầm thì huỷ `ChaseTarget`" đang có sẵn |
| **Bầy đàn** | Khi 1 con aggro, các con **cùng archetype** trong bán kính gần đó cũng aggro theo | 1 mini-mechanic mới duy nhất — gắn `ChaseTarget` lan sang hàng xóm, tái dùng nguyên component |
| **Lang thang** | Lúc `Idle` không đứng yên — dạo quanh vị trí spawn trong bán kính nhỏ, tốc độ rất chậm | Tái dùng pose `Walk` sẵn có, chỉ đổi mục tiêu di chuyển thành điểm ngẫu nhiên gần spawn |
| **Đờ đẫn** | Trễ 1-2s giữa lúc phát hiện và lúc thực sự bắt đầu đuổi (vẫn quay mặt về hướng người chơi trong lúc trễ) | 1 timer nhỏ chèn trước khi gắn `ChaseTarget` |
| **Cảnh giác cao** | Phát hiện xa hơn (chase range × 1.3), không bị vật cản mỏng che khuất tầm nhìn | `ZOMBIE_CHASE_RANGE` nhân hệ số (khác Hung hãn ở chỗ không có tốc bùng lên) |
| **Hoảng loạn** *(mới)* | Máu xuống dưới 50% → khựng lại/né tránh ~0.5s (không tiến) trước khi tiếp tục đuổi bình thường | Cùng kiểu check ngưỡng máu như `ZOMBIE_CRAWL_HEALTH_FRACTION` đã có, chỉ đổi ngưỡng + hành vi (lùi thay vì bò) |
| **Phản ứng âm thanh** *(mới)* | Dễ bị hút về phía tiếng súng hơn hẳn — kể cả đang ngoài `ZOMBIE_CHASE_RANGE` bình thường | Tái dùng đúng cơ chế "tiếng súng là noise, kéo chú ý" đã có (`PLAYER_THREAT_PER_SHOT`), chỉ tăng hệ số riêng cho trait này |
| **Ẩn nấp** *(mới)* | Chase range giảm như Lỳ đòn (khó phát hiện), NHƯNG 1 khi đã aggro thì có 1 lần tốc bùng như Hung hãn — kết hợp 2 cơ chế có sẵn, không phải cơ chế thứ 3 | Ghép 2 field đã dùng ở Lỳ đòn + Hung hãn, không thêm gì mới |
| **Bám trụ** *(mới)* | Không đuổi quá xa khỏi vị trí spawn (bán kính "leash") — thoát khỏi bán kính đó thì quay lại vị trí cũ thay vì đuổi tiếp | 1 so sánh khoảng cách với `spawn_pos` đã lưu sẵn lúc `spawn_zombies`, không cần lưu thêm state mới |

### Archetype → trait (theo đúng tính cách/ngoại hình đã thiết kế)

| Trait | Archetype |
|---|---|
| Bình thường | Hành khách tàu điện · Người đi đường — 2 archetype trung tính nhất, không thiên hẳn về tính cách nào |
| Hung hãn | Bảo vệ/Lính Meridian · Cảnh sát giao thông · Công nhân cảng · Đội an ninh tư nhân — có huấn luyện/phản xạ nhanh trước khi biến đổi |
| Lỳ đòn | Người tị nạn · Người sống sót cuối cùng — đã quen sợ hãi/cảnh giác từ trước, 1 khi đã chú ý tới ai thì không rời mắt |
| Bầy đàn | Khách mua sắm · Giáo viên/Học sinh cấp 3 · Phụ huynh đón con · Người chợ đêm — vốn hay tụ tập theo nhóm khi còn sống |
| Lang thang | Người đi công viên · Nhân viên công viên · Người giao hàng đêm · Nhân viên vệ sinh đô thị · Nhân viên dịch vụ công cộng · Nhân viên vận tải/logistics — nghề nghiệp vốn di chuyển liên tục |
| Đờ đẫn | Nạn nhân giao thông · Nạn nhân hoá chất — chấn thương cơ thể nặng, phản xạ chậm |
| Cảnh giác cao | Nhà khoa học/Nhân viên lab · Nhân viên vận hành chỉ huy · Nhân viên hậu cần Meridian — công việc đòi hỏi quan sát liên tục |
| **Hoảng loạn** *(mới)* | Nhân viên bất động sản · Cư dân vành đai — dân thường thuần, chưa từng va chạm bạo lực, dễ chững lại khi bị dồn |
| **Phản ứng âm thanh** *(mới)* | Người vô gia cư · Kỹ thuật viên nhà máy lọc — quen dựa vào thính giác (sống lang thang / quen tiếng máy móc lớn nên nhạy với âm thanh lạ) |
| **Ẩn nấp** *(mới)* | Người bị giam giữ · Công nhân thời vụ — phản xạ như thú bị dồn ép (từng bị giam) / quen trốn việc ở góc khuất nhà xưởng |
| **Bám trụ** *(mới)* | Nhân viên văn phòng · Cư dân khu mẫu · Công nhân dệt · Đối tượng thí nghiệm — gắn chặt với 1 chỗ (bàn làm việc/nhà/máy dệt/bàn thí nghiệm) tới mức không rời xa dù đang đuổi |

---

## Bảng map → archetype (tra nhanh khi code spawn table)

| Map | Campaign | Archetype xuất hiện (đúng 4 mọi map) | Đặc biệt/boss trong map |
|---|---|---|---|
| M1 Highrise | 1 Ground Zero | Nhân viên văn phòng, Bảo vệ/Lính Meridian, Người đi đường, Nhân viên vệ sinh đô thị | — |
| M2 Meridian Plaza | 1 | Khách mua sắm, Bảo vệ/Lính Meridian, Người đi đường, Nhân viên vệ sinh đô thị | — |
| M3 Meridian Station | 1 | Hành khách tàu điện, Người vô gia cư, Người đi đường, Nhân viên vệ sinh đô thị | **Nghẹt Thở** (Bloater, giới thiệu) |
| M4 Route 9 Overpass | 1 | Nạn nhân giao thông, Cảnh sát giao thông, Người đi đường, Nhân viên vệ sinh đô thị | **Kẻ Húc** (Rammer, giới thiệu) |
| M5 Terminus Overlook | 1 | Bảo vệ/Lính Meridian, Người tị nạn, Người đi đường, Nhân viên vệ sinh đô thị | **Lôi Đình** (Brute, boss, lần 1) |
| M6 Sundown Gardens | 2 Vành đai | Cư dân khu mẫu, Nhân viên bất động sản, Cư dân vành đai, Nhân viên dịch vụ công cộng | — |
| M7 Fairview Elementary | 2 | Giáo viên/Học sinh cấp 3, Phụ huynh đón con, Cư dân vành đai, Nhân viên dịch vụ công cộng | — |
| M8 Miller's Pond | 2 | Người đi công viên, Nhân viên công viên, Cư dân vành đai, Nhân viên dịch vụ công cộng | — |
| M9 Perimeter Night Market | 2 | Người chợ đêm, Người giao hàng đêm, Cư dân vành đai, Nhân viên dịch vụ công cộng | **Dây Thối** (Strangler, giới thiệu) |
| M10 Checkpoint Delta | 2 | Bảo vệ/Lính Meridian, Người tị nạn, Cư dân vành đai, Nhân viên dịch vụ công cộng | **Lôi Đình** (lần 2) |
| M11 Ironloom Textile Mill | 3 Vùng công nghiệp | Công nhân dệt, Đội an ninh tư nhân, Công nhân thời vụ, Nhân viên vận tải/logistics | **Kẻ Hú** (Howler, giới thiệu) |
| M12 Cát Lái Freight Yard | 3 | Công nhân cảng, Đội an ninh tư nhân, Công nhân thời vụ, Nhân viên vận tải/logistics | — |
| M13 Cold Storage 7 | 3 | Nạn nhân hoá chất, Đội an ninh tư nhân, Công nhân thời vụ, Nhân viên vận tải/logistics | **Kẻ Phun Độc** (Spewer, giới thiệu) |
| M14 Refinery Loop | 3 | Kỹ thuật viên nhà máy lọc, Bảo vệ/Lính Meridian, Công nhân thời vụ, Nhân viên vận tải/logistics | **Kẻ Rình Rập** (Stalker, giới thiệu) |
| M15 Perimeter Fence Alpha | 3 | Bảo vệ/Lính Meridian, Người tị nạn, Công nhân thời vụ, Nhân viên vận tải/logistics | **Lôi Đình** (lần 3) |
| M16 Main Gatehouse | 4 Cổng thành | Bảo vệ/Lính Meridian, Người tị nạn, Nhân viên hậu cần Meridian, Người bị giam giữ | — |
| M17 Sector 3 Admin Block | 4 | Nhân viên văn phòng, Nhà khoa học/Nhân viên lab, Nhân viên hậu cần Meridian, Người bị giam giữ | **Kẻ Rình** (Stray, giới thiệu) |
| M18 Subterranean Lab Alpha | 4 | Nhà khoa học/Nhân viên lab, Đối tượng thí nghiệm, Nhân viên hậu cần Meridian, Người bị giam giữ | **Kẻ Cưỡi** (Leech, giới thiệu) |
| M19 Command Bunker | 4 | Bảo vệ/Lính Meridian, Nhân viên vận hành chỉ huy, Nhân viên hậu cần Meridian, Người bị giam giữ | tái xuất Dây Thối + Kẻ Phun Độc |
| M20 Final Breach | 4 | Bảo vệ/Lính Meridian, Người sống sót cuối cùng, Nhân viên hậu cần Meridian, Người bị giam giữ | **Lôi Đình** (boss cuối) + toàn bộ 9 loại đặc biệt/encounter khác cùng lúc |

---

## Thiết kế đầy đủ 4 loại đặc biệt còn lại (quyết định 2026-09-01)

Đặt tên tiếng Việt riêng cho từng loại (GDD trước đó chỉ gọi "kiểu
Stalker/Strangler/Spewer/Leech" để tham chiếu nhanh theo công năng — giờ
mỗi loại có **2 tên chính thức song song cho 2 ngôn ngữ chính của game**:
tên Việt (chính) + tên Anh riêng trong ngoặc (không dùng lại tên gốc từ
game khác, đặt mới theo đúng công năng — vd "Stalker" cho loại nhảy vồ
rình rập). Mỗi loại: tái dùng `move_zombies`/pathing/aggro
hiện có cho di chuyển (đúng nguyên tắc GDD mục 17.18 — không viết lại AI
framework), chỉ thêm **1 system hành vi riêng** xử lý phần khác biệt.

### Lore chung — "4 mức độ đột biến"

Audio log rải rác (đúng cơ chế environmental storytelling ở GDD 14.3)
dần hé lộ: 4 loại đặc biệt không phải 4 "loài" tách biệt — là **4 mức độ
đột biến khác nhau của cùng 1 chủng Đỏ**, tuỳ thuộc **người đó
phơi nhiễm ở đâu/bao lâu/thể trạng gốc** trước khi biến đổi. Đây là lý do
kể chuyện tự nhiên cho việc chúng xuất hiện đúng nơi hợp bối cảnh (Kẻ
Phun Độc chỉ xuất hiện quanh khu vực có hoá chất, Kẻ Rình Rập từng là vận
động viên/công nhân leo trèo...) — không cần giải thích bằng cutscene,
chỉ cần 1-2 audio log mỗi loại khi giới thiệu lần đầu.

### 🫧 Dây Thối (Strangler) — M9 Night Market, tái xuất M19

- **Ngoại hình:** gầy khô, cổ họng phình to bất thường, da xám xanh rỉ
  dịch nhớt. Thường đứng yên trong bóng tối/sau quầy hàng, chỉ lộ ra khi
  đã ngắm trúng.
- **Kỹ năng:** bắn 1 "dây lưỡi" dài (tầm bắn đề xuất 15m, tương đương
  `ZOMBIE_CHASE_RANGE / 4`) về phía người chơi có line-of-sight — trúng
  thì **kéo liên tục** người chơi lại gần nó (giống Downed nhẹ: không tự
  di chuyển được, vẫn bắn được nhưng tầm bắn giảm) cho tới khi: (a) dây
  bị bắn đứt (HP dây riêng, thấp — vài phát súng lục là đứt) hoặc (b) bị
  kéo tới sát nó, bị cắn 1 đòn nặng rồi thả ra.
- **Vai trò chiến thuật:** tách 1 người chơi ra khỏi nhóm — nếu không ai
  bắn đứt dây kịp, nạn nhân bị kéo ra xa khỏi tầm yểm trợ, dễ bị zombie
  thường vây quanh trong lúc không di chuyển được.
- **Counter-play:** bất kỳ đồng đội nào bắn trúng "dây" (hitbox riêng dọc
  theo đường dây, không phải bắn thân nó) đều ngắt ngay — khuyến khích
  phối hợp, đúng tinh thần co-op.
- **Variant (2):** Người bán hàng đêm (tạp dề còn dính) · Bảo vệ chợ
  (đồng phục rách).

### 🧪 Kẻ Phun Độc (Spewer) — M13 Cold Storage 7, tái xuất M19

- **Ngoại hình:** da phồng rộp/loang lổ hoá chất, miệng rỉ chất lỏng
  xanh lục phát sáng nhẹ trong bóng tối (hợp bối cảnh kho lạnh tối).
  Không cận chiến — luôn giữ khoảng cách, lùi lại nếu người chơi áp sát.
- **Kỹ năng:** đứng yên tại chỗ ngắm rồi phun 1 vũng acid tầm xa (giống
  đạn cầu vồng, có thể né bằng cách di chuyển ngang) — vũng tồn tại N
  giây (đề xuất 6s), gây DoT cho bất kỳ ai đứng trong đó (tái dùng đúng
  cơ chế "vùng khói/khí độc" đã thiết kế cho Nghẹt Thở/Bloater ở GDD 17.5,
  chỉ đổi particle màu xanh acid thay vì khói xám) + phá huỷ dần item
  pickup nếu vũng phủ lên (buộc người chơi phải chọn: lấy đồ liều ăn DoT,
  hay bỏ qua).
- **Vai trò chiến thuật:** area denial — ép người chơi di chuyển khỏi vị
  trí đang cố thủ, đặc biệt nguy hiểm khi kết hợp zombie thường đang dồn
  tới từ hướng khác (buộc chọn giữa 2 rủi ro).
- **Counter-play:** tầm bắn xa nhưng máu thấp (tương đương zombie thường)
  — ưu tiên bắn hạ trước khi nó kịp phun, hoặc đơn giản là di chuyển khỏi
  vũng acid đã phun (không né được auto).
- **Variant (2):** Công nhân kho lạnh (đồ bảo hộ ăn mòn) · Kỹ thuật viên
  hoá chất (kính bảo hộ vỡ, mặt biến dạng nặng nhất trong 2 variant).

### 🐆 Kẻ Rình Rập (Stalker) — M14 Refinery Loop, tái xuất M20

- **Ngoại hình:** cơ bắp co rút lộ gân, dáng khom người như thú săn mồi,
  di chuyển bằng cả 2 tay 2 chân trên bề mặt dốc/ống dẫn (hợp M14 —
  "verticality cao nhất Campaign 3" theo đúng doc map đã ghi).
  Im lặng gần như tuyệt đối cho tới lúc phóng tới — không gầm gừ báo
  trước như zombie thường.
- **Kỹ năng:** nấp trên cao/khuất tầm nhìn, khi người chơi vào tầm (đề
  xuất 10m + có line-of-sight) → nhảy vọt (arc nhảy dài hơn tầm nhảy
  bình thường của player) tới, nếu trúng thì **ghim người chơi xuống đất**
  (không tự đứng dậy/bắn được, khác Downed thường — đây là trạng thái
  tạm, không tính vào `bleed_out_timer` của Downed) và liên tục cắn gây
  sát thương theo thời gian cho tới khi: (a) đồng đội bắn nó rớt ra (vài
  phát trúng là ngắt) hoặc (b) máu người chơi về 0 → chuyển `Downed` thật
  theo luật thường.
- **Vai trò chiến thuật:** trừng phạt việc tách đoàn/không để ý phía
  trên — đúng tinh thần "ambush predator", ép người chơi phải quan sát
  không gian 3 chiều (trần/gờ tường), không chỉ nhìn ngang như đấu với
  zombie thường.
- **Counter-play:** có 1 khoảnh khắc "rướn người chuẩn bị nhảy" rõ ràng
  (telegraph ~0.4s) trước khi phóng tới — người chơi tinh ý né được bằng
  cách lăn/tránh sang bên; nếu đã bị ghim, chỉ cần đồng đội bắn trúng là
  cứu được ngay, không cần giữ nút như Revive.
- **Variant (2):** Công nhân bảo trì (dây an toàn đứt lủng lẳng) · Giám
  sát ca (mũ bảo hộ vỡ).

### 🎯 Kẻ Cưỡi (Leech) — M18 Subterranean Lab Alpha, tái xuất M20

- **Ngoại hình:** nhỏ/gầy bất thường so với zombie khác (tương phản có
  chủ đích — trông "yếu" nhưng nguy hiểm nhất nếu chủ quan), 2 tay dài
  bất cân đối, móng tay dài như móc.
- **Kỹ năng:** lao tới nhanh (tốc độ cao hơn hẳn baseline, đề xuất
  `ZOMBIE_RUN_SPEED * 1.8`), nếu chạm được người chơi thì **nhảy lên
  lưng, chiếm quyền lái** — người chơi vẫn di chuyển được nhưng input di
  chuyển bị "kéo lệch" dần về phía nó muốn (server-side, giống cách
  `move_player_physics` đã nhận input rồi mô phỏng — Kẻ Cưỡi chỉ cộng
  thêm 1 vector lệch hướng vào input trước khi simulate), cố lái người
  chơi ra khỏi nhóm/vào khu vực nguy hiểm (hố sâu, phòng đầy zombie khác,
  bẫy thí nghiệm trong lab).
- **Vai trò chiến thuật:** khác Dây Thối (kéo về 1 điểm cố định) — Kẻ
  Cưỡi **chủ động lái theo địa hình nguy hiểm nhất gần đó**, biến chính
  người chơi thành mối đe doạ cho bản thân, đúng tinh thần "hài hước
  nhưng nguy hiểm" của Leech gốc trong L4D.
- **Counter-play:** không tự gỡ được (như Dây Thối/Kẻ Rình Rập) — bắt
  buộc đồng đội can thiệp (bắn nó rớt khỏi lưng, sát thương thấp là đủ
  vì mục tiêu là làm nó rớt chứ không cần giết). Solo chơi 1 mình: bị
  cưỡi = mất kiểm soát cho tới khi máu nó về 0 hoặc hết thời gian bám tự
  nhiên (đề xuất cap cứng 8s để không kẹt game vô thời hạn khi chơi solo).
- **Variant (2):** Nhà khoa học (áo blouse rách bươm, dáng co quắp) · Vật
  thí nghiệm không rõ danh tính (không mặc đồ, biến dạng nặng nhất toàn
  bestiary — chỉ xuất hiện ở M18/M20, đúng tinh thần "nơi rò rỉ dịch bệnh
  thật sự bắt đầu" của M18).

## 3 loại đặc biệt mới — tham khảo ngoài L4D (quyết định 2026-09-01)

L4D1/2 đã dùng hết ở trên (Bloater/Stalker/Strangler/Spewer/Leech/Brute/
Wailer) — 3 loại dưới đây lấy cảm hứng từ franchise zombie khác, lấp vào
những map hiện chưa có đặc biệt riêng (M4, M11, M17), giữ đúng nguyên
tắc cũ: tái dùng `move_zombies`, chỉ thêm đúng 1 system hành vi + 1 clip
animation mới mỗi loại.

### 🚌 Kẻ Húc (Rammer — lấy cảm hứng từ L4D2) — M4 Route 9 Overpass

- **Ngoại hình:** 1 cánh tay phải phình to dị dạng gấp 3 tay trái, thân
  hình lệch hẳn sang 1 bên, dáng đi khập khiễng khi không charge.
- **Kỹ năng:** khi thấy người chơi trong đường thẳng đủ xa (hợp M4 —
  "sightline dài nhất Campaign 1"), **lao thẳng 1 mạch** (charge) theo
  hướng đó. Trúng người chơi → **tóm kéo đi** trong lúc vẫn đang chạy,
  đập liên tục xuống đất (sát thương theo thời gian, khác Kẻ Rình Rập —
  Rammer DI CHUYỂN trong lúc khống chế, không đứng yên 1 chỗ) cho tới
  khi đồng đội bắn nó rớt ra. Charge trượt (không trúng ai, hoặc đâm phải
  tường/vật cản) → **choáng 2-3s tại chỗ** — cửa sổ phản công rõ ràng.
- **Vai trò chiến thuật:** biến chính "sightline dài" (ưu điểm của người
  chơi có súng tầm xa) thành rủi ro — đứng dàn hàng ngang giữa đường
  thẳng dễ bị 1 con Rammer gom cả nhóm.
- **Counter-play:** né sang ngang (charge chỉ theo đường thẳng ban đầu,
  không bẻ lái) — hoặc chủ động đứng nép sau cột/xe hỏng để nó đâm trượt
  và tự choáng.
- **Variant (2):** Tài xế xe tải (charge từ thói quen "cầm lái"), Lính
  cứu hộ giao thông (tay dị dạng như đang "kéo nạn nhân").

### 🔇 Kẻ Rình (Stray — lấy cảm hứng từ Clicker, *The Last of Us*) — M17 Sector 3 Admin Block

- **Ngoại hình:** mắt phủ màng trắng đục (mù hẳn), lỗ tai/mũi phình bất
  thường — không gầm gừ, gần như im lặng tuyệt đối.
- **Kỹ năng:** **KHÔNG dùng line-of-sight để phát hiện** (khác MỌI loại
  zombie khác trong game, kể cả archetype) — chỉ phát hiện qua **âm
  thanh** (tiếng súng, tiếng chạy, tiếng nói) trong 1 bán kính lớn, tái
  dùng đúng cơ chế "gunfire = noise" (`PLAYER_THREAT_PER_SHOT`) đã có,
  nhưng đây là điều kiện phát hiện DUY NHẤT của nó, không cộng thêm vào
  chase range như archetype trait "Phản ứng âm thanh". Đi bộ chậm/không
  bắn súng qua khu vực có Kẻ Rình gần như vô hình với nó.
- **Vai trò chiến thuật:** đảo ngược hoàn toàn nhịp game — mọi loại khác
  đều thưởng cho phản xạ/hoả lực, riêng Kẻ Rình thưởng cho **sự im lặng
  và kiên nhẫn** (hợp M17 — "cấu trúc giống Map 1 có chủ đích, tạo cảm
  giác quay vòng" — vẫn không gian quen thuộc nhưng luật chơi khác hẳn).
- **Counter-play:** đi chậm (không sprint), tránh nổ súng gần nó, dùng
  dao/cận chiến thay súng nếu buộc phải hạ nó. 1 khi đã phát hiện, di
  chuyển như zombie thường (đuổi bằng cảm nhận rung động, không cần thấy).
- **Variant (2):** Nhân viên văn phòng (đã mù do phơi nhiễm hoá chất
  trong toà nhà), Nhà khoa học (kính bảo hộ vỡ, mắt cháy do thí nghiệm).

### 📢 Kẻ Hú (Howler — lấy cảm hứng từ *State of Decay*/*Dying Light*) — M11 Ironloom Textile Mill

- **Ngoại hình:** hàm dưới banh rộng bất thường gần 90°, cổ họng phồng
  to liên tục rung.
- **Kỹ năng:** **không tấn công trực tiếp bao giờ** — khi thấy người
  chơi, đứng yên **hú vang** (tăng vọt "PLAYER_THREAT" toàn map tạm thời
  + gọi TOÀN BỘ zombie thường trong bán kính lớn, rộng hơn hẳn archetype
  trait "Bầy đàn" chỉ gọi cùng archetype gần đó) rồi **bỏ chạy ngay lập
  tức** khỏi giao tranh (không đấu tay đôi). Máu rất thấp (ngang zombie
  thường) — chết nhanh nếu bắn kịp TRƯỚC khi nó hú xong.
  hợp M11 — nhà máy dệt ồn ào, giải thích tự nhiên vì sao 1 sinh vật
  "cần hú thật to mới gọi được đồng loại qua tiếng ồn máy dệt".
- **Vai trò chiến thuật:** bài test phản xạ — thấy là phải ưu tiên bắn
  hạ NGAY, khác nhịp "từ từ quan sát" của phần lớn loại khác trong game.
  Bỏ lỡ = phải chống đỡ 1 đợt horde bất ngờ giữa map, không phải lúc
  trigger đã định sẵn như F3 Highrise.
- **Counter-play:** luôn quét mắt tìm nó đầu tiên khi vào khu vực rộng —
  máu thấp nên 1 phát súng lục chính xác là đủ hạ trước khi kịp hú.
- **Variant (2):** Công nhân dệt (họng rách vì hít bụi vải lâu ngày),
  Quản đốc (còi báo động vẫn đeo quanh cổ — chi tiết môi trường, không
  tương tác).

---

### Ghi chú phân bổ theo campaign (đúng nhịp "giới thiệu dần" của L4D)

| Campaign | Đặc biệt giới thiệu mới | Lý do chọn map đó |
|---|---|---|
| 1 Ground Zero | Nghẹt Thở (Bloater, M3) · Kẻ Húc (Rammer, M4) | Bloater đã chốt GDD 17.5 — không gian hẹp M3; Rammer tận dụng sightline dài nhất Campaign 1 ở M4 |
| 2 Vành đai | Dây Thối (Strangler) | M9 chợ đêm nhiều góc khuất/quầy hàng để núp |
| 3 Vùng công nghiệp | Kẻ Phun Độc + Kẻ Rình Rập + Kẻ Hú | M13 có sẵn hoá chất, M14 có sẵn verticality, M11 nhà máy ồn ào hợp "phải hú to" — 3 loại cùng campaign vì địa hình công nghiệp đa dạng nhất, đủ chỗ cho cả 3 phát huy đúng sở trường |
| 4 Cổng thành | Kẻ Cưỡi + Kẻ Rình | M18 phòng lab — hành lang hẹp hợp "lái người chơi vào bẫy thí nghiệm"; Kẻ Rình ở M17 (trước M18) — twist luật chơi ngay khi bước vào campaign cuối, chuẩn bị tâm lý trước không gian tối tăm nhất game ở M18 |

Tới M20 (final breach): **cả 10 loại đặc biệt/boss/encounter** xuất hiện cùng
lúc — đúng vai trò "tổng kết toàn bộ những gì người chơi đã học" của map
cuối, không giới thiệu gì mới nữa.

---

## Boss & special encounter — thiết kế đầy đủ

GDD mục 15 mới ghi 1 dòng ("Brute-style, máu/dame cao" / "Wailer-style,
đứng yên trừ khi bị làm phiền") — đặt tên và điền chi tiết ở đây.

### 👹 Lôi Đình (Brute) — boss, tái xuất M5 → M10 → M15 → M20

- **Ngoại hình:** to gấp ~3 lần zombie thường, cơ bắp phình bất thường
  (mức đột biến cực đoan nhất trong "4 mức độ đột biến" — phơi nhiễm liều
  cao nhất/gần nguồn rò rỉ nhất, hé lộ dần qua log ở Campaign 4), da nứt
  nẻ đỏ như bị bỏng từ bên trong. Không có variant trang phục (khỏa thân
  phần thân trên, quần rách) — cùng 1 model tái xuất cả 4 lần, chỉ đổi
  **màu da** theo từng lần gặp để báo hiệu "mạnh hơn lần trước": xám
  (M5) → đỏ sẫm (M10) → đỏ tươi nứt sáng (M15) → đen cháy phát sáng khe
  nứt (M20, mạnh nhất).
- **Kỹ năng:** không có kỹ năng tầm xa — thuần cận chiến nhưng cực nặng:
  1 đòn đấm gây knockback mạnh (tái dùng `KNOCKBACK_FORCE` đã có, nhân hệ
  số cao hơn hẳn) + sát thương đủ hạ gục 1 người chơi máu đầy chỉ sau
  2-3 đòn nếu không né. Máu rất cao (đề xuất ~40x `ZOMBIE_MAX_HEALTH`),
  không chạy nhanh hơn player đi bộ thường (đề xuất bằng `ZOMBIE_RUN_SPEED`
  — đuổi kịp người đứng yên/bắn, không đuổi kịp người đang chạy lùi có
  kiểm soát) — buộc chiến thuật "vừa lùi vừa bắn", không phải chạy trốn
  hoàn toàn.
- **Vai trò chiến thuật:** đúng vai trò Brute gốc — 1 "sự kiện" riêng biệt
  giữa dòng zombie thường, cả nhóm phải tập trung hoả lực + kiểm soát vị
  trí (không để ai bị dồn vào góc cụt) trong lúc zombie thường vẫn tiếp
  tục quấy nhiễu xung quanh.
- **Counter-play:** không né được hoàn toàn đòn cận chiến (hitbox lớn),
  nhưng có thể kite bằng cách lùi liên tục trong lúc bắn — chết vì Lôi
  Đình gần như luôn là do đứng yên/kẹt góc, không phải do máu nó quá cao.
- 4 lần gặp không lặp lại y hệt — **địa hình xung quanh khác nhau** theo
  từng map (M5 sân thượng hẹp, M10 sân checkpoint mở, M15 hàng rào an
  ninh nhiều chướng ngại, M20 không gian cuối cùng) là đủ tạo cảm giác
  mới mà không cần đổi kỹ năng.

### 🕯️ Người Than Khóc (Wailer-style) — special encounter, không phải boss

- **Xuất hiện:** rải rác nhiều map (không cố định — đề xuất mọi map
  Campaign 2 trở đi, mỗi map tối đa 1 con, vị trí cố định theo map giống
  hệt cách zombie thường đặt qua data map, KHÔNG random spawn) — luôn
  đặt ở 1 góc khuất/phòng riêng, không nằm trên lối đi chính.
- **Ngoại hình:** ngồi co ro ôm gối, tóc che mặt, run rẩy liên tục
  (animation loop riêng, không phải Idle zombie thường) — thân hình gầy
  yếu, KHÔNG trông nguy hiểm khi nhìn thoáng qua (đúng chủ đích lừa người
  chơi chủ quan).
- **Kỹ năng:** đứng yên, phát ra tiếng khóc nhỏ liên tục (audio cue duy
  nhất người chơi có để nhận biết trước khi thấy hình). Không tấn công
  trừ khi: (a) người chơi lại gần trong 1 bán kính nhỏ (đề xuất 3m) HOẶC
  (b) bị trúng đạn/ánh đèn pin chiếu thẳng vào quá N giây liên tục → nổi
  giận, đứng dậy lao tới với tốc độ RẤT cao (đề xuất `ZOMBIE_RUN_SPEED *
  2.5`) và sát thương 1 đòn gần bằng Lôi Đình — nhưng máu thấp (ngang
  zombie thường), hạ được nhanh nếu đã chuẩn bị.
- **Vai trò chiến thuật:** bài test "kỷ luật" cho cả nhóm — phần thưởng
  (1 item hiếm đặt cạnh nó) chỉ đáng lấy nếu nhóm đủ bình tĩnh đi vòng/rón
  rén qua, không phải lúc nào cũng nên chọc nó dậy.
- **Counter-play:** cách an toàn nhất là **tránh hoàn toàn** (đi đường
  vòng nếu map cho phép) — nếu buộc phải đi qua, tắt đèn pin, đi chậm,
  giữ khoảng cách > 3m. Nếu đã chọc dậy: tập trung hoả lực ngay lập tức,
  đừng chạy (tốc độ nó nhanh hơn player).

---

## Kho dự trữ — 20 loại bổ sung (quyết định 2026-09-01, dành cho HẬU
## LAUNCH, không phải 20 map ban đầu)

10 loại đặc biệt ở trên (7 kiểu L4D + 3 tham khảo ngoài) đã đủ cho toàn
bộ 4 campaign/20 map. 20 loại dưới đây (#1-10 rồi #11-20, thêm sau) là
**kho ý tưởng dự trữ** — thiết kế sẵn cho rẻ (chỉ tốn giấy), nhưng KHÔNG
gán map/campaign nào cả, đúng
tinh thần GDD mục 16 Giai đoạn 3 "bổ sung dần theo bản cập nhật hậu ra
mắt" — tránh lặp lại đúng cái bẫy đã tự cảnh báo ở mục 15 (scope 20 map +
8 loại địch đã là khối lượng "cả 1 studio nhỏ, không phải 1 người trong 3
tháng"). Khi cần dùng: chọn 1-2 loại phù hợp map mới, gán vào đúng mẫu ở
trên (map, animation, trait), không cần thiết kế lại từ đầu.

| # | Tên | Cảm hứng | Kỹ năng cốt lõi | Vì sao khác mọi loại đã có |
|---|---|---|---|---|
| 1 | **Kẻ Tái Sinh** | Regenerator (*Resident Evil 4*) | Tự hồi máu liên tục — chỉ 1 điểm yếu (vết nứt phát sáng ở ngực) mới gây sát thương thật, bắn chỗ khác gần vô dụng | Duy nhất loại buộc **bắn chính xác 1 điểm**, không phải bắn nhiều/né |
| 2 | **Kẻ Bám Trần** | Licker (*Resident Evil*) | Bám tường/trần, rơi xuống đánh 1 đòn mạnh rồi bò đi tái định vị ngay (hit-and-run) | Khác Kẻ Rình Rập (Stalker) ở chỗ KHÔNG ghim/pin — đánh xong rút liền |
| 3 | **Kẻ Cuồng Nộ** | Berserker (*Dead Island*) | Miễn sát thương khi đang gầm gừ lao tới; phải bắn dồn đủ số phát trong thời gian ngắn mới làm choáng, lộ điểm yếu vài giây | Duy nhất loại có "giáp tạm thời theo trạng thái", không phải giáp cố định |
| 4 | **Kẻ Ném Đá** | Behemoth (*Dying Light 2*) | Bản tank tầm xa — ném mảnh vỡ/vật cản gây sát thương diện rộng, chậm nhưng nguy hiểm ở không gian mở | Brute duy nhất có tấn công TẦM XA (Lôi Đình thuần cận chiến) |
| 5 | **Kẻ Ấp Trứng** | Carrier (*World War Z*) | Chết → nổ ra 2-3 zombie con nhỏ/yếu/nhanh | Buộc cân nhắc hạ nó TỪ XA thay vì cận chiến như mọi loại khác |
| 6 | **Kẻ Giáp Sắt** | Lính chống bạo động (nguyên bản, cảm hứng chung nhiều game) | Giáp trước gần miễn sát thương — chỉ hiệu quả khi bắn từ sau lưng/2 bên | Duy nhất loại buộc **bọc lườn/đi vòng** giữa combat, không phải bắn thẳng mặt |
| 7 | **Đầu Ký Sinh** | Headcrab (*Half-Life*) | Sau khi hạ cơ thể chính, 1 ký sinh tách khỏi cổ, nhảy loạn xạ tấn công yếu nhưng rất nhanh | Duy nhất loại có **2 pha chiến đấu** trong 1 lần hạ gục |
| 8 | **Kẻ Giả Chết** | "Xác giả chết bật dậy" (trope kinh điển RE/L4D) | Nằm bất động lẫn giữa xác trang trí môi trường (`!` deco có sẵn trong map design, xem `map01_highrise.md`'s legend) — chỉ lộ diện khi người chơi đi sát qua | Duy nhất loại tái dùng thẳng prop môi trường đã có sẵn làm ngụy trang, không cần asset mới |
| 9 | **Kẻ Nhiễm Độc Ngược** | Gasbag (*World War Z*) | Chết phát tán khí khiến ĐẠN BẮN QUA VÙNG KHÍ mất độ chính xác (debuff vũ khí, không phải debuff tầm nhìn như Nghẹt Thở) | Khác Nghẹt Thở (Bloater) ở đúng chỗ này — debuff súng thay vì debuff mắt |
| 10 | **Kẻ Rình Bóng Tối** | "Chỉ di chuyển khi không bị nhìn" (trope kiểu *SCP-173*/Weeping Angel) | Đứng im khi trong tầm nhìn/crosshair người chơi, chỉ di chuyển khi ngoài góc nhìn trực tiếp | Duy nhất loại biến **hành động "ngoái nhìn lại"** thành cơ chế sinh tồn, không loại nào khác trong bestiary có |
| 11 | **Kẻ Săn Bầy** | Lycan (*Resident Evil Village*) | Luôn xuất hiện theo nhóm 2-3, chủ động tách ra bao vây từ nhiều hướng cùng lúc thay vì dồn từ 1 phía | Duy nhất loại có **spawn theo cụm bắt buộc + phối hợp bao vây**, không phải 1 con đơn lẻ |
| 12 | **Kẻ Phân Mảnh** | Necromorph (*Dead Space*) | Bắn trúng thân chỉ làm chậm — phải bắn đứt tay/chân mới hạ hẳn; chi đứt rời còn bò/lết tấn công riêng vài giây | Duy nhất loại buộc đổi mục tiêu ngắm (tay/chân) thay vì luôn nhắm giữa thân |
| 13 | **Kẻ Trong Ống Thông Gió** | Xenomorph (*Alien: Isolation*) | Di chuyển qua hệ ống dẫn/trần treo (map công nghiệp có sẵn), bất ngờ xuất hiện từ 1 trong nhiều cửa thoát khác nhau mỗi lần | Duy nhất loại có **nhiều điểm xuất hiện không đoán trước**, không phải 1 vị trí spawn cố định |
| 14 | **Kẻ Không Ngừng** | "Never stops, always walking toward you" (*It Follows*) | Luôn biết chính xác vị trí người chơi (không cần phát hiện), đi thẳng tới KHÔNG BAO GIỜ ngừng — nhưng chậm hơn cả đi bộ player | Duy nhất loại gây áp lực **tâm lý dài hạn** thay vì nguy hiểm tức thời — không nguy nếu di chuyển, nguy nếu mải làm việc quá lâu 1 chỗ |
| 15 | **Kẻ Giả Dạng** | Mimic (*The Thing*) | Model/texture giống HỆT 1 archetype dân thường đang đứng lẫn trong map, không phân biệt được bằng mắt — chỉ lộ diện khi bất ngờ tấn công | Duy nhất loại gây **nghi ngờ mọi archetype khác** xung quanh, đổi hẳn cách người chơi quan sát map |
| 16 | **Kẻ Sợ Ánh Sáng** | đảo ngược khái niệm Wailer — ánh sáng thành vũ khí | Bị đẩy lùi/choáng tạm thời khi đèn pin chiếu thẳng vào; hung hãn hơn hẳn trong bóng tối | Duy nhất loại biến **đèn pin thành vũ khí phòng thủ thật sự**, không chỉ để nhìn đường |
| 17 | **Kẻ Đào Hang** | côn trùng/loài đào hang (nguyên bản) | Chui qua khe hẹp/lỗ thông gió nhỏ mà player không lọt qua được — luôn có thể vòng ra sau lưng trong không gian kín | Duy nhất loại "bỏ qua" layout tường thông thường của map |
| 18 | **Kẻ Câm** | 28 Days Later-style rage infected, phiên bản im lặng | Không phát tiếng bước chân dù đang chạy — mọi loại khác đều có tiếng động làm cảnh báo tự nhiên, riêng loại này thì không | Duy nhất loại tước mất **tín hiệu cảnh báo âm thanh** người chơi quen dựa vào |
| 19 | **Kẻ Thôi Miên** | Siren-song (thần thoại Hy Lạp) | Phát ra âm thanh khiến zombie THƯỜNG gần đó tạm ngừng đuổi người chơi, quay sang cắn lẫn nhau vài giây | Duy nhất loại **gây rối loạn hàng ngũ địch** thay vì tấn công/triệu tập thêm quân |
| 20 | **Kẻ Đóng Băng** | zombie hàn đới (nguyên bản, khác hẳn Cold Storage/Kẻ Phun Độc đã dùng) | Cắn trúng gây **giảm tốc độ di chuyển tạm thời** cho người chơi thay vì sát thương | Duy nhất loại crowd-control thuần, không gây dame trực tiếp |

Ghi chú kỹ thuật (khi thật sự cần dùng 1 loại trong số này): #1/#3/#6/#12
cần 1 khái niệm mới hoàn toàn — "điểm yếu"/"trạng thái miễn sát thương
tạm thời"/"hướng bắn"/"chi có thể tách rời" — vốn `zombie_hit_damage_
bones`/`ZombieHitboxBones` (đã có, dùng cho hitbox theo từng vùng thân)
là nền tảng gần nhất cho cả 4, chỉ cần mở rộng field `damage_multiplier`/
`detachable` theo bone thay vì cố định — **rẻ hơn viết mới** vì hệ
per-bone hitbox đã tồn tại đúng mục đích này. #10/#14 rẻ bất ngờ: chỉ cần
so `Transform` hướng camera hoặc khoảng cách người chơi mỗi frame, không
cần render/AI phức tạp gì thêm. #11/#13/#17 đều là biến thể của "nhiều
điểm spawn/xuất hiện" — có thể dùng chung 1 hệ thống spawn-point-pool
thay vì viết riêng 3 lần.

### Animation cho 20 loại dự trữ — soát lại theo đúng nguyên tắc cũ

Chưa xét lúc thiết kế cơ chế ở trên — soát lại ngay, giữ đúng kỷ luật
"tái dùng tối đa" đã áp dụng cho 10 loại chính. Kết quả: **20 loại chỉ
cần thêm ĐÚNG 4 clip mới**, còn lại toàn bộ tái dùng bộ 5 clip nền
(`Idle/Walk/Run/Crawl/Bite`) + 7 clip đặc biệt đã có (Tongue/Spit/Pounce/
Latch/Cower/Slam/Scream).

| # | Loại | Animation | Lý do |
|---|---|---|---|
| 1 | Kẻ Tái Sinh | Bite (0 mới) | Điểm yếu là hitbox/code, không phải dáng đứng |
| 2 | Kẻ Bám Trần | **1 mới: Drop Strike** | Không loại nào có tư thế "rơi từ trên xuống tấn công" |
| 3 | Kẻ Cuồng Nộ | Bite (0 mới) | Miễn sát thương là trạng thái code/VFX phát sáng, không đổi dáng đánh |
| 4 | Kẻ Ném Đá | **1 mới: Throw** | Chưa loại nào có động tác ném |
| 5 | Kẻ Ấp Trứng | Bite (0 mới) | Nổ ra zombie con là code trigger lúc chết, không phải animation |
| 6 | Kẻ Giáp Sắt | Bite (0 mới) | Giáp là giảm dame theo hướng, thuần code |
| 7 | Đầu Ký Sinh | **Pounce** (tái dùng từ Kẻ Rình Rập, model thu nhỏ) | Đúng dáng "nhảy chồm liên tục" đã có sẵn |
| 8 | Kẻ Giả Chết | **1 mới: Lie Still** (idle nằm, không phải Idle đứng) | Cần nằm lẫn giữa xác trang trí — Idle đứng không hợp bối cảnh |
| 9 | Kẻ Nhiễm Độc Ngược | Bite (0 mới) | Khí độc là VFX+code lúc chết |
| 10 | Kẻ Rình Bóng Tối | Idle/Walk (0 mới) | Đứng im/di chuyển đều là pose có sẵn, chỉ đổi ĐIỀU KIỆN chạy pose nào |
| 11 | Kẻ Săn Bầy | Run/Bite (0 mới) | Phối hợp bao vây là logic spawn/target, không phải animation riêng |
| 12 | Kẻ Phân Mảnh | **Crawl** (tái dùng, gắn lên chi đã tách rời) | Đúng dáng "lết trên mặt đất" có sẵn — chỉ gắn lên entity chi rời thay vì cả cơ thể |
| 13 | Kẻ Trong Ống Thông Gió | **Pounce** (tái dùng, cho pha "bật ra khỏi ống") | Cùng dáng bung người tới như Stalker |
| 14 | Kẻ Không Ngừng | Walk/Bite (0 mới) | Chỉ là Walk vĩnh viễn, không có Run |
| 15 | Kẻ Giả Dạng | **Dùng nguyên animation của archetype đang giả dạng** (0 mới) | Đúng định nghĩa — nó LÀ archetype đó về mặt hình ảnh cho tới lúc lộ diện |
| 16 | Kẻ Sợ Ánh Sáng | **Hit reaction/flinch** (tái dùng từ Tier 3 `ZOMBIE_ANIMATION_PLAN.md`, đã có kế hoạch) | Đúng dáng "giật lùi vì tổn thương" có sẵn, chỉ đổi tác nhân kích hoạt (ánh sáng thay vì đạn) |
| 17 | Kẻ Đào Hang | **1 mới: Burrow** (chui xuống/trồi lên, dùng ngược chiều cho cả 2 chiều) | Chưa loại nào có động tác đào/chui |
| 18 | Kẻ Câm | Walk/Run/Bite (0 mới) | Chỉ tắt audio bước chân, animation y hệt zombie thường |
| 19 | Kẻ Thôi Miên | **Scream** (tái dùng từ Kẻ Hú) | Cùng dáng "ngửa đầu hú" — hiệu ứng code khác (gây loạn thay vì gọi bầy) nhưng animation giống hệt |
| 20 | Kẻ Đóng Băng | Bite (0 mới) | Hiệu ứng đóng băng là status effect gắn lúc cắn trúng, không đổi animation cắn |

**Tổng clip mới cho 20 loại dự trữ: 4** (Drop Strike, Throw, Lie Still,
Burrow) — 16/20 loại dùng **0 clip mới**, nhờ tái dùng chéo cả bộ clip
đặc biệt đã gen cho 10 loại chính (Pounce dùng lại 2 lần, Scream dùng lại
1 lần) lẫn Tier 3 (Hit reaction) đã có kế hoạch sẵn trong
`ZOMBIE_ANIMATION_PLAN.md`.

### Sửa lại: 1 số loại KHÔNG thể tái dùng Walk/Run 2 chân (đã bỏ sót)

Soát lần 2 phát hiện lỗi: mấy bảng trên gán "Walk/Run (0 mới)" cho vài
loại mà chính phần thiết kế cơ chế đã mô tả **di chuyển 4 chân/leo trèo**
— dáng đó không thể ra từ animation 2 chân đứng thẳng có sẵn, dù chỉ đổi
tốc độ qua code cũng không cứu được (sai cả bộ xương chuyển động, không
phải sai tốc độ).

| Loại | Mô tả gốc | Animation ĐÚNG cần có |
|---|---|---|
| Kẻ Rình Rập (Stalker, tầng 10 loại chính) | "di chuyển bằng cả 2 tay 2 chân trên bề mặt dốc/ống dẫn" | Cần **Quadruped Prowl** (bò rình 4 chân) thay Walk, **Quadruped Climb** thay Run — KHÔNG phải Walk/Run người thường như đã ngầm định trước đó |
| Kẻ Bám Trần (Licker) | bám tường/trần | Cần **Wall Cling** (bám, thay Idle) + **Quadruped Climb** (di chuyển trên tường/trần, thay Walk/Run) — không chỉ mỗi "Drop Strike" lúc tấn công |
| Kẻ Trong Ống Thông Gió | di chuyển qua ống dẫn | Cần **Quadruped Climb** (chui/bò trong ống hẹp) — không phải Walk/Run đứng thẳng |
| Kẻ Săn Bầy (Lycan) | cảm hứng sói *RE Village*, vốn là sinh vật 4 chân | Cần **Quadruped Prowl/Run** — nếu giữ hoàn toàn dáng người thì mất đúng chất "Lycan" |

**Fix bằng 1 bộ locomotion 4 chân DÙNG CHUNG** (đúng tinh thần tái dùng
xuyên suốt file này) — chỉ **2 clip mới** (`Quadruped Prowl` đi chậm rình
mồi, `Quadruped Climb` di chuyển nhanh/leo trèo), dùng chung cho cả 4
loại trên thay vì 4 bộ riêng — vẫn đúng 1 khung xương `Meshy` (rig vẫn
hình người, chỉ đổi TƯ THẾ khi chạy 2 clip này, không phải đổi rig).

**Kẻ Ấp Trứng** không nằm trong danh sách sửa — đám zombie con nó nổ ra
LÀ zombie thường thu nhỏ, vẫn 2 chân bình thường, Walk/Run gốc đúng rồi.

**Riêng Đầu Ký Sinh — ngoại lệ thật sự, không tái dùng được:** đây là
sinh vật NHỎ, tách khỏi cơ thể chính, tỉ lệ cơ thể khác hẳn zombie —
không thể gắn clip `Meshy` (dành cho khung xương người lớn) lên 1 model
nhỏ khác tỉ lệ mà không méo mó. Cần **1 rig nhỏ riêng + bộ 3 clip riêng**
(Idle, Move/Skitter, Leap-bite) — khoản duy nhất trong toàn bộ 30 loại
KHÔNG tái dùng được gì, chấp nhận vì chỉ 1 con/1 lần dùng (pha 2 sau khi
hạ cơ thể chính), không lặp lại ở loại nào khác.

**Tổng clip mới đã sửa lại (trước khi tính thêm phần dưới)** — cộng dồn
TOÀN BỘ hệ thống (10 loại chính + 20 loại dự trữ): 7 (10 loại chính) + 4
(dự trữ #1-10, không đổi) + 2 (bộ Quadruped dùng chung cho 4 loại) + 3
(rig riêng Đầu Ký Sinh) = **16 clip mới cho 30 loại**.

### Đúng — mỗi loại nên có ≥2-3 animation tấn công, không phải 1 (sửa lần 3)

2 việc cần tách bạch trước khi thêm:

1. **Loại chỉ tái dùng `Bite`** (11 loại: Kẻ Tái Sinh, Kẻ Cuồng Nộ, Kẻ Ấp
   Trứng, Kẻ Giáp Sắt, Kẻ Nhiễm Độc Ngược, Kẻ Không Ngừng, Kẻ Câm, Kẻ
   Đóng Băng, Kẻ Săn Bầy phần cắn, zombie con của Kẻ Ấp Trứng...) —
   **ĐÃ có sẵn 3-6 biến thể** qua Tier 4 của `ZOMBIE_ANIMATION_PLAN.md`
   (Bite/Claw trái/Claw phải/Double grab/Overhead smash/Lunge bite, chọn
   ngẫu nhiên mỗi lần vào tầm đánh) — **không cần thêm gì**, đúng "2-3
   animation riêng" bạn nói, chỉ là đã có kế hoạch từ trước, không phải
   thiếu.
2. **Loại có kỹ năng tấn công riêng thật sự** (Tongue/Spit/Pounce/Latch/
   Slam/Throw/Drop Strike — 7 loại) — đây mới là chỗ ĐÚNG LÀ đang thiếu:
   mỗi loại này hiện chỉ có 1 clip cho CẢ cụm "chuẩn bị → ra đòn", trong
   khi phần mô tả cơ chế đã tự nhắc tới "telegraph" (vd Stalker "rướn
   người chuẩn bị nhảy ~0.4s") mà animation chưa có clip riêng cho đúng
   khoảnh khắc đó.

**Fix — thêm 4 clip windup/recovery DÙNG CHUNG theo nhóm kỹu tấn công**
(không phải 1 bộ riêng/loại — vẫn giữ tinh thần tái dùng):

| Clip mới | Dùng khi nào | Dùng chung cho |
|---|---|---|
| **Lunge Windup** | Rướn người ngay trước khi lao/nhảy/chồm tới | Kẻ Rình Rập, Kẻ Húc, Kẻ Bám Trần, Kẻ Cuồng Nộ, Kẻ Săn Bầy, Kẻ Cưỡi (6 loại) |
| **Ranged Windup** | Kéo tay/ngậm miệng ngay trước khi bắn/phun/ném | Dây Thối, Kẻ Phun Độc, Kẻ Ném Đá (3 loại) |
| **Miss Recovery** | Loạng choạng ~0.5-1s sau khi lao/nhảy trượt (không trúng ai) | Kẻ Rình Rập, Kẻ Húc, Kẻ Bám Trần, Kẻ Cuồng Nộ, Kẻ Săn Bầy (5 loại) — đúng cửa sổ phản công đã mô tả ở phần counter-play mỗi loại |
| **Gather Breath** | Hít vào/ngửa cổ chuẩn bị hú, ngay trước clip Scream đã có | Kẻ Hú, Kẻ Thôi Miên (2 loại, cùng dùng lại Scream cho phần "ra đòn") |

Kết quả: **7 loại kỹ năng riêng giờ đều có đúng 3 clip cho 1 lần tấn công**
(Windup → Strike → Recovery-nếu-trượt), thay vì 1 clip duy nhất — đúng
yêu cầu "2-3 animation riêng" — mà chỉ tốn thêm **4 clip mới**, nhờ mỗi
clip windup/recovery dùng chung cho 3-6 loại cùng họ tấn công thay vì
generate riêng từng loại.

**Tổng clip mới cho tới đây: 16 + 4 = 20** — cộng thêm Tier 4's tối đa 6
biến thể Bite (dùng chung cho MỌI loại còn lại + zombie thường).

### 10 đòn tấn công đặc trưng mới (quyết định 2026-09-01) — cho 10 loại
### đang chỉ dựa vào Bite/clip dùng chung, thêm dấu ấn riêng

10 clip dưới đây KHÔNG phải windup/recovery (đã có ở trên) — là **đòn ra
tay (Strike) thật sự riêng**, thay thế Bite cho đúng 10 loại đang "mượn"
animation nhiều nhất, để chúng không đánh giống hệt nhau dù cùng dựa vào
Tier 4. Mỗi clip vẫn CHỈ 1 lần gen, không nhân theo variant.

| Clip mới | Gán cho | Khác Bite/Tier 4 ở điểm nào |
|---|---|---|
| **Shield Bash** | Kẻ Giáp Sắt | Đẩy bằng khiên/tay giáp — đòn ĐẨY LÙI, không phải cắn/cào |
| **Tentacle Lash** | Kẻ Tái Sinh | Quất chi biến dị tầm xa hơn Bite 1 chút — khớp hình tượng Regenerator vốn có xúc tu |
| **Double Slash Combo** | Kẻ Săn Bầy | 2 nhát cào liên tiếp nhanh — hợp lối đánh phối hợp theo nhóm |
| **Ground Slam Shockwave** | Kẻ Ném Đá | Đòn cận chiến khi bị áp sát (đập đất, sóng xung kích lan ra) — bổ sung cho Throw vốn chỉ đánh xa |
| **Neck Grapple Bite** | Kẻ Rình | Cắn giữ chặt lâu (DoT) thay vì 1 phát nhanh — hợp việc nó định vị bằng cách bám chặt do bị mù |
| **Tail Whip** | Kẻ Đào Hang | Quật đuôi khi bị lộ trên mặt đất (đuôi côn trùng, hợp ngoại hình đào hang) |
| **Spore Burst Melee** | Kẻ Nhiễm Độc Ngược | Xì độc tại chỗ mỗi lần cận chiến — không chỉ lúc chết như thiết kế gốc |
| **Piercing Lunge** | Đầu Ký Sinh | Đòn riêng cho rig nhỏ (không tái dùng Bite người lớn được — đúng lý do nó đã cần rig riêng) |
| **Freezing Grasp** | Kẻ Đóng Băng | 2 tay ôm giữ + toả hơi lạnh — trực quan hoá hiệu ứng đóng băng thay vì giấu trong 1 cú cắn thường |
| **Suffocating Grip** | Kẻ Không Ngừng | Bóp cổ nghẹt thở khi cuối cùng bắt kịp — hợp chất "kinh dị tâm lý" của loại này hơn 1 cú cắn thường |

**Tổng clip mới sau đợt này: 20 + 10 = 30 clip mới cho 30 loại đặc
biệt/boss/encounter** — đúng 1 clip attack riêng/loại tính trung bình,
nhưng vẫn rẻ hơn "30 bộ animation đầy đủ" rất nhiều vì Idle/Walk/Run/
Crawl + windup/recovery vẫn 100% dùng chung như các mục trên — chỉ phần
"đòn ra tay" mới được cá nhân hoá lần này.

## Việc còn lại trước khi code

1. Điền `docs/characters/zombie.md` (story/lore/visual notes đang TODO)
   cho zombie thường trước — làm nền tảng style trước khi viết brief cho
   96 archetype variant + 10 loại đặc biệt/boss/encounter.
2. ~~Quyết định map nào nhận Stalker/Strangler/Spewer/Leech~~ — **đã xong**,
   xem bảng "Ghi chú phân bổ theo campaign" ở trên.
3. **Kỹ thuật — archetype (code, chưa làm ở giai đoạn hiện tại):** cần 1
   bảng tra map_id → danh sách (model_path, health_mult, speed_mult) khi
   spawn — tái dùng đúng pattern `ZombieKind`/`zombie_kind_for_phase` đã
   có trong `crates/shared/src/zombie.rs`, chỉ mở rộng từ 2 kind cố định
   thành 1 bảng theo map. Chưa cần làm ở Giai đoạn 1 (GDD mục 17.19) —
   map nghệ thuật thật là bước cuối cùng, archetype chỉ thật sự cần khi
   bắt đầu dựng map 2 trở đi.
4. **Kỹ thuật — 4 loại đặc biệt (code, thứ tự đề xuất theo mục 17.18/
   17.19 của GDD):**
   - `SpecialZombieKind` enum (`Bloater` đã có khung ở GDD 17.5, thêm
     `Strangler`/`Spewer`/`Stalker`/`Leech`) gắn thêm lên entity `Zombie` đã
     có, giống hệt cách archetype dự kiến làm ở bước 3 — không tạo entity
     loại mới, chỉ thêm component.
   - Mỗi loại 1 system hành vi riêng (`smoker_tongue_behavior`,
     `spitter_ranged_behavior`, `hunter_pounce_behavior`,
     `jockey_hijack_behavior`) chạy `With<SpecialZombieKind::X>`, không
     đụng vào `move_zombies` của zombie thường.
   - Cơ chế dùng chung có thể tái dùng ngay: vũng AoE của Kẻ Phun Độc tái
     dùng particle/damage-over-time đã thiết kế cho Nghẹt Thở (GDD 17.5);
     trạng thái "bị ghim" của Kẻ Rình Rập/Dây Thối tái dùng khung
     `PlayerLifeState` (GDD 17.1) làm 1 sub-state tạm thời thay vì tạo
     state máy mới.
   - Lôi Đình (Brute) chỉ cần tái dùng zombie thường với multiplier cực
     lớn trên `Health`/`KNOCKBACK_FORCE` — không cần AI mới, chỉ cần
     spawn 1 lần/map tại vị trí cố định thay vì theo lưới `ZOMBIE_SPAWNS`.
   - Người Than Khóc cần 1 `PassiveUntilProvoked` component riêng (aggro
     range nhỏ + trigger ánh sáng/tiếng súng) — gần giống cơ chế
     `ChaseTarget` hiện có nhưng đảo ngược điều kiện kích hoạt.

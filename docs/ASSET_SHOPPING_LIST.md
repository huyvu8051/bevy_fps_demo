# Asset shopping list — candidate paid packs (not bought yet)

Packs found while researching the Unreal-blockout-to-Bevy pipeline
(see chat history around 2026-09-06 for the full pipeline discussion:
PCG in Unreal → export mesh+layout → Blender → glTF → RON → Bevy spawn,
same generic pattern `item_gallery_cars.rs`'s `spawn_item_gallery_test_props`
already uses). These are industrial/warehouse/container-yard kits meant
for Map 12 (Cát Lái Freight Yard) and Map 13 (Cold Storage 7) — see
`docs/maps/map12_cat_lai_freight_yard.md` / `map13_cold_storage_7.md`.

Only the first one below is confirmed free; the rest need buying —
noted here to reconsider later, not decided/purchased yet.

## Free (use this one first)

- **Free Factory Environment Collection** (ArtCore Studios) —
  https://www.unrealengine.com/marketplace/en-US/product/factory-environment-collection
  850+ meshes (LOD + collision included), 430+ materials, factory/
  warehouse/office/storage sections, includes a crane + cargo lift +
  drone. Confirmed free via Epic's own blog announcement.

## Paid — reconsider later if the free pack isn't enough

- **Container Yard + Warehouse** —
  https://www.fab.com/listings/073c1efd-2196-4ce7-a9bc-bf0001a4a39a
  3 container sizes (10ft/20ft/40ft), open/double-open variants — most
  directly matches Map 12's container-maze need. Price not confirmed
  (Fab blocks automated fetches, checked manually in chat — listed as
  paid).
- **Container Yard Environment Set** —
  https://www.fab.com/listings/3a882c55-de05-4790-8d03-b1adc99da4bc
  Alternative to the above, not compared side-by-side yet.
- **Modular Warehouse** —
  https://www.unrealengine.com/marketplace/en-US/product/modular-warehouse-01
  Modular wall/roof/beam/floor kit + cranes/pallets/tools/clutter —
  most relevant for Map 13's cold-storage corridor if the free pack's
  warehouse pieces don't fit well enough.
- **Industrial Factory** —
  https://www.fab.com/listings/02708474-5331-4c47-b55c-3cbadc51fb4f
  Fully modular industrial kit, includes photo-scanned foliage + VFX
  library (birds, light beams, fog).

## Free CC0 alternative (skips Unreal export entirely for these props)

Kenney.nl ships `.glb`/`.obj` directly — usable straight in Blender →
Bevy without round-tripping through Unreal at all, only worth going
through the Unreal/PCG pipeline for these if you specifically want
PCG's scatter/layout automation rather than the meshes themselves:

- **Factory Kit** — https://kenney.nl/assets/factory-kit (140 assets)
- **City Kit (Industrial)** — https://kenney.nl/assets/city-kit-industrial (40 assets)
- **Conveyor Kit** — https://kenney.nl/assets/conveyor-kit

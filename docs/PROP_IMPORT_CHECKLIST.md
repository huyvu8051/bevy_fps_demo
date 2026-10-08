# Prop/item import checklist (meshy.ai)

Companion to [`CHARACTER_IMPORT_CHECKLIST.md`](CHARACTER_IMPORT_CHECKLIST.md)
(that one's for rigged/animated characters — zombies, survivors). This one
is for everything else: furniture, vehicles, pickups, architectural props
— static or simple-transform meshes with no skeleton.

Item list + progress tracking lives in a separate checklist artifact
(ask in chat for the link if you don't have it) — this doc is the
*process* (how to generate + import correctly), not the *inventory*.

## 1. Generation workflow (meshy.ai)

Confirmed by hands-on testing in the Meshy workspace (`Model`/`Animate`
tabs) — this is the actual sequence, not a guess. Full end-to-end test
done on a sedan car (see mục 5 for what that run actually cost/found).

1. **Image tab** — generate the concept image *inside Meshy itself*,
   don't feed it an external photo. Testing found this gives
   noticeably better texture results on the resulting mesh than
   Image-to-3D from an outside image. 1 image, 1:1 aspect ≈ 9 credits.
2. **Model tab** — Image-to-3D from that generated image.
   - **Topology: use `Smart Topology`, not `High Detail`, for every
     prop.** `High Detail` is what produced a 116MB / ~3.1M-triangle
     glb in testing — completely unusable as a game asset (Meshy's own
     `Faces`/`Vertices` counter in the viewer confirms the count before
     you even download; if it reads in the millions, stop and switch
     topology mode rather than downloading). `Smart Topology` gives a
     retopologized quad mesh sized for real-time use — this is the
     actual purpose-built control for exactly this problem, not a
     manual decimation step to do afterward.
   - Rigged characters need `Rigged` on + `A-Pose`; static props don't.
   - Base geometry generation ≈ 25 credits (High Detail, Ultra Mode).
   - **Texture is a separate pass from the base model**, even with the
     panel's `Texture` toggle on — after the base mesh finishes, click
     the viewer's own `Texture` button and run that step too (uses the
     concept image as the texture reference automatically). ≈ 20
     credits more.
3. **Animate tab** — characters only. `Animate Model` auto-rigs (if not
   already rigged in step 2), then attach whichever clips are needed
   from the Library or the model-specific AI Motion suggestions.
4. **Download** — format `glb`, "All Added" animations bundled, "Single
   file" on. One `.glb` per item, mesh + textures + (if a character)
   every animation clip embedded.
   - **Turn on `Resize` and set `Height` to the item's real-world size
     in cm, `Origin: Bottom`** — this bakes the correct real-world
     scale AND a ground-level pivot into the exported file itself, so
     the in-engine `Transform` can just be `scale 1.0` / `y = 0` with
     no per-item correction needed. Missed this on the first test (car
     downloaded at whatever arbitrary scale Meshy generated at,
     needed `scale ≈ 2.2` fixed up by hand afterward, see mục 3) — use
     it going forward, it's strictly better than fixing scale in code.
     Take the height number straight from `docs/maps/gdd/map01_dimensions.md`
     section 1's item library (e.g. sedan = 150cm) so every item stays
     consistent with the sizes already decided there.

**Already generated something at `High Detail` (like the 116MB sedan
from the first test)?** Don't regenerate from scratch — the viewer's
own toolbar has a **`Remesh`** button ("Rebuild the mesh with a target
polycount and cleaner topology", ≈ 20 credits) that fixes an
already-generated mesh in place. Cheaper than a full re-generation and
doesn't lose the texture/concept-image work already paid for.

### Real cost per item (measured, not estimated)

~54 credits and ~4-5 minutes wall-clock for 1 static prop (image 9 +
model 25 + texture 20, `High Detail`), most of it spent waiting on the
2 generation steps rather than active work. **Budget accordingly before
generating the full item list**: at this rate the checklist's ~85
items would cost roughly 4,500 credits — check your actual balance
before committing to Tier 2/3, and prioritize Tier 1 first regardless.
`Smart Topology` is untested for cost yet (likely cheaper, simpler
mesh) — re-measure once a few props go through it and update this
number.

### Prompt templates

Style anchor — keep this exact phrase at the end of every prompt so all
generated assets read as one consistent game, not a mismatched pile:
`gritty desaturated apocalyptic survival-horror style`.

**Prop/furniture:**
```
[Item name], three-quarter front view, isolated on a plain light gray
background, realistic proportions, worn office/industrial materials
with subtle dirt and wear, game-ready PBR asset, neutral studio
lighting, no cast shadow, gritty desaturated apocalyptic survival-horror
style
```

**Vehicle/large object:**
```
[Vehicle/object name], three-quarter front-side view, parked and
stationary, isolated on a plain light gray background, realistic
proportions, dusty and slightly weathered, game-ready PBR asset,
neutral studio lighting, no cast shadow, gritty desaturated
apocalyptic survival-horror style
```

**Character (zombie/survivor):** see `CHARACTER_IMPORT_CHECKLIST.md`'s
own prompt notes — A-pose is mandatory there for auto-rig to map
correctly; not relevant for props.

Tips: plain gray isolated background (Image-to-3D separates the object
from a clean background far more reliably than a busy one); full object
in frame, not cropped; 3/4 angle for anything non-symmetric or with a
distinct front/back (vehicles, cabinets), straight-on is fine for flat
symmetric things (doors, desks, panels).

## 2. Where files go

No metadata-file convention for props yet (unlike characters'
`character.ron`) — just a folder convention, mirroring the characters
layout:

```
crates/client/assets/props/<slug>/
  mesh/<slug>.glb        <- the imported file, as downloaded
  mesh/source/           <- optional: original Meshy project export, if kept
  concept/               <- optional: the Image-tab concept art, for reference
```

## 3. Import gotchas (props, no rig)

Much shorter list than characters — no bone names, no animation clip
indices to match, no per-bone hitbox wiring.

- **Set `Resize` at download time (mục 1 step 4) and this mostly goes
  away.** Meshy's raw mesh scale is NOT normalized to real-world
  meters — confirmed by testing, a car downloaded without `Resize` set
  spawned at roughly 55% of its correct height (needed `scale ≈ 2.2`
  fixed up by hand, next to a 1.8m reference `Cuboid`, to read as a
  real sedan) and floated above the floor until the Y offset was also
  corrected (origin wasn't at the mesh's own base either). If you
  forgot to set `Resize`/`Origin: Bottom` before downloading, that
  same manual process still works as a fallback: spawn at `scale 1.0`,
  drop a known-height reference `Cuboid` next to it, screenshot,
  eyeball the ratio, adjust `scale` + Y offset, repeat — but re-
  downloading with `Resize` set correctly is less error-prone than
  doing this by eye every time.
- **Mesh data**: same `ATTRIBUTE_POSITION`/`ATTRIBUTE_NORMAL`/
  `ATTRIBUTE_UV_0` + indexed-mesh expectation as characters (see
  `CHARACTER_IMPORT_CHECKLIST.md` mục 4) — applies identically here,
  Meshy's own glb export satisfies this by default so it's usually a
  non-issue, but check if a prop ever renders untextured/blank.
- **Both render paths** (desktop `bevy_render` vs. iOS native Metal
  probe) still apply if this prop is meant to ship on-device — test both
  before calling a prop done, same as characters.
- **Replacing a `LevelBox` placeholder**: every Item Gallery entry and
  every Map 1 piece (`GARAGE_CAR_SIZE`, `LOBBY_DESK_SIZE`, etc.) is
  currently a plain colored cuboid built from
  `shared::map1_level_pieces()`/`item_gallery_level_pieces()` (see
  `crates/shared/src/level_piece.rs`'s module doc comment for why —
  geometry is baked into both client and server at compile time, no
  network dependency). Swapping one of these for a real mesh means
  spawning a `SceneRoot` from the new glb (client-only, alongside the
  existing procedural piece or replacing it) rather than editing the
  `BoxPiece`/`PanelPiece` data — those stay describing physics-collider
  bounds even once a real mesh is drawn over them.

# Character import checklist

Run through this every time a new or replacement character mesh (e.g. from
meshy.ai) is about to be dropped into `crates/client/assets/characters/`.
The game hardcodes several assumptions against the two shipped meshes
(`zombie.glb`, `soldier.glb`) — an AI-generated mesh that doesn't conform
to these will load, but animate/hit-detect wrong in ways that are easy to
miss until you're already mid-playtest.

## 1. Skeleton / bone names

Only needed for characters that use per-bone hitboxes (currently just
zombies — see `wire_zombie_hitbox_bones`, `crates/shared/src/lib.rs`).
That system matches bone `Name`s exactly (Mixamo-style rig naming):

- `Head`
- `Spine2` (chest)
- `Spine` (belly)
- `LeftArm` / `RightArm` / `LeftForeArm` / `RightForeArm` / `LeftHand` / `RightHand`
- `LeftUpLeg` / `RightUpLeg` / `LeftLeg` / `RightLeg` / `LeftFoot` / `RightFoot`

meshy.ai's default rig almost certainly won't use these names — either
rename bones in Blender before export, or (cheaper) keep meshy.ai's names
and update the `matches!` arms in `wire_zombie_hitbox_bones` to match
instead. Don't skip this silently: a mesh with no matching bones just
means every hit falls back to whatever hitbox logic exists without
per-bone precision — no error, no crash, just wrong damage.

## 2. Animation clips

`skinned_gltf::compute_joint_matrices` (`crates/client/src/cadisplaylink/
skinned_gltf.rs`) takes a **clip index**, not a name. The game's own
consts (`ZOMBIE_ANIM_*`/`PLAYER_MODEL_ANIM_*`, `crates/client/src/lib.rs`)
hardcode which index means idle/walk/run/etc. for the *current* two
meshes — a new mesh's clip order will almost certainly differ.

Before wiring a new mesh in:
1. Load it in Blender (or `gltf-viewer`) and note which animation index
   is which pose.
2. Update the `animations:` block in that character's `character.ron` to
   match (this file isn't parsed by the game yet — treat it as the
   documented source of truth until it is, and update the corresponding
   `*_ANIM_*` consts in `lib.rs` to match by hand).

## 3. Scale and origin

The game does NOT trust a mesh's native scale/pivot — every character has
a hand-tuned correction applied on a *child* entity (not the mesh's own
Transform, which network code overwrites every tick):
- Zombie: `ZOMBIE_SCALE`, `ZOMBIE_MODEL_Y_OFFSET` (feet-to-capsule-center).
- Soldier: `PLAYER_MODEL_SCALE`, `PLAYER_MODEL_Y_OFFSET`,
  `PLAYER_MODEL_FACING_CORRECTION` (yaw offset — soldier.glb's forward
  axis didn't match the game's own "+Z is forward" convention).

A new mesh needs these re-measured by eye (spawn it, compare against a
known-good reference like the player capsule) — there's no automatic way
to detect a wrong scale/offset/facing besides looking at it in-game.

## 4. Mesh data the game actually reads

Both the desktop (bevy_render) and native Metal renderer paths read
`ATTRIBUTE_POSITION`/`ATTRIBUTE_NORMAL`/`ATTRIBUTE_UV_0` and expect an
indexed mesh — see `ExtractedMesh`'s own doc comment
(`crates/client/src/cadisplaylink/native_game_extract.rs`) for exactly
what's tolerated missing (UVs can be empty, falls back to `[0,0]`) vs. what
silently produces a blank/wrong mesh (no `ATTRIBUTE_POSITION` = skipped
entirely). Textures: only common RGBA8 formats decode on the native Metal
path — anything else (compressed KTX2/Basis, etc.) renders untextured.

## 5. Both render paths, not just one

A mesh that looks right on desktop can still be broken on iOS
(`USE_REAL_GAME_NATIVE_RENDERER`) or vice versa — the two paths extract/
draw meshes through completely separate code (`native_metal_probe.rs`
never touches `bevy_render` at all). Test both before calling a new
character done.

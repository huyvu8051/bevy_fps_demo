# Implementation plan: real bone-based zombie hitboxes (Option C)

Grounded in the actual codebase (see `HITBOX_RESEARCH.md` for the
"why"). This plan replaces the single-capsule + fixed-Y-threshold guess
(`zombie_hit_damage_geometric`) with real per-bone hit detection, server
side, using the same headless-animation approach Valorant/Overwatch use
(rewind pose, not just root position, then test real per-part shapes).

## 0. Baseline facts this plan depends on

- Server (`crates/server/src/main.rs`) uses `MinimalPlugins` +
  `ScheduleRunnerPlugin::run_loop` at a fixed `SERVER_TICK_HZ = 30.0`
  (line 55) — not Bevy's `FixedUpdate`. It already has `AssetPlugin`,
  `bevy::mesh::MeshPlugin`, `ScenePlugin`. It has **no** `AnimationPlugin`
  and the `bevy` dependency is `default-features = false, features =
  ["bevy_log", "bevy_render"]` — `bevy_gltf`/`bevy_animation` are not
  compiled in at all today.
- Lag-comp history today is root-only:
  `ZombieHistory(VecDeque<(f32 timestamp, Vec3 translation, Quat rotation)>)`,
  written every tick by `record_zombie_history`, pruned to
  `LAG_COMPENSATION_MAX_SECS = 0.3s` (~9 entries/zombie at 30Hz), read by
  `zombie_history_at` in `handle_fire_shot`.
- Server zombies are spawned with a bare `avian3d::Collider::capsule` (no
  `RigidBody`, no mesh, no skeleton) — see `spawn_zombies`.
- Client already does 90% of the "attach entities to bone names" work:
  `ZombieHitboxBones` component + `wire_zombie_hitbox_bones` system
  (`crates/client/src/lib.rs`) match glTF node `Name`s (`Head`, `Spine2`,
  `Spine`, arm/leg bones) and walk `ChildOf` up to the zombie root. This
  logic is directly reusable server-side almost as-is.
- Asset: `models/zombie.glb`, one skinned mesh, Mixamo-style humanoid
  rig, referenced via `GltfAssetLabel::Scene(0)` / `GltfAssetLabel::
  Animation(n)`.

## Phase 1 — Server can load and animate the skeleton (no gameplay change yet) — ✅ DONE

**Goal:** prove a headless zombie skeleton can exist and animate on the
server with zero rendering, before touching damage logic at all.

**Confirmed working**: spawned one zombie with a real `SceneRoot` +
looping idle animation, headless, and logged the `Head` bone's live
`GlobalTransform.y` once/sec — it visibly oscillated (~5.87 to ~6.55)
purely from server-side animation sampling, no window/render ever
created. The spike code itself was temporary and has been removed;
everything below is the *permanent* infra it required, which stays.

**What it actually took** (more than the original estimate — each step
below was only discovered by running the server and reading the next
panic, not knowable up front):

1. `crates/server/Cargo.toml` — `bevy` feature list needed, in order of
   discovery:
   - `"bevy_gltf"`, `"bevy_animation"` — the obvious ones.
   - `"gltf_animation"` — **distinct from `"bevy_animation"`!** Without
     it, `zombie.glb` loaded with zero `AnimationN` labeled sub-assets
     even though `bevy_animation` alone was already enabled — this is
     the actual switch that makes `bevy_gltf`'s loader convert animation
     curves into `AnimationClip` assets at all.
   - `"png"` — `zombie.glb` embeds PNG textures; the loader tries to
     decode every embedded image regardless of whether anything renders
     it, and aborts the whole scene load (skeleton included) without a
     decoder compiled in.
   - `"bevy_pbr"` — needed for the `StandardMaterial` type to exist at
     all (see `init_asset` below).
   - `"debug"` — temporarily, to unmask reflection type names in panic
     messages (normally elided as `<Enable the debug feature to see the
     name>`); kept since it costs nothing at runtime and Phase 2's own
     debugging will hit the same class of error again.
2. `crates/server/src/main.rs` plugin list — added, in this order,
   right after the existing `AssetPlugin`/`MeshPlugin`/`ScenePlugin`:
   - `AnimationPlugin`
   - `GltfPlugin::default()` — **required even with the `"bevy_gltf"`
     cargo feature on**: the feature only compiles the crate in, it's
     `GltfPlugin` that actually registers the `AssetLoader` for `.glb`/
     `.gltf` paths with `AssetServer`. Without it: "Could not find an
     asset loader matching ... zombie.glb".
   - `ImagePlugin::default()` — registers `Assets<Image>`, normally
     pulled in transitively by `RenderPlugin` (never added here).
     Without it: `bevy_gltf`'s loader panics trying to allocate an
     `Image` handle for every embedded texture ("Cannot allocate an
     Asset Handle of type ... Image because the asset type has not been
     initialized").
   - `AssetPlugin { file_path: "../client/assets", .. }` (overriding the
     default `AssetPlugin::default()` already there) — the server has no
     `assets/` dir of its own; reuses the client's. **Gotcha**: this path
     is relative to this crate's own `CARGO_MANIFEST_DIR`
     (`crates/server/`) in dev builds, **not** the process's actual CWD
     — confirmed by first trying `"crates/client/assets"` (assuming CWD
     = workspace root per `cargo run -p server`'s README-documented
     invocation) and getting `.../crates/server/crates/client/assets/...`.
3. `.init_asset::<StandardMaterial>()` on the `App` builder — registers
   the asset type directly rather than adding the full `PbrPlugin`
   (which expects a `RenderPlugin`/render-world to extract into and
   isn't meant to run without one). `bevy_gltf` allocates a
   `StandardMaterial` handle per material regardless of rendering.
4. A `register_type::<T>()` call **per component type actually present
   in the spawned glTF scene hierarchy** — `bevy_scene`'s spawner clones
   entities from a loaded `Scene` using reflection-based cloning for any
   component without a registered `ComponentCloneBehavior`, and panics
   ("scene contains the unregistered type ...") one type at a time,
   discovered by iterating build→run→read-panic→register→repeat. Full
   list that was actually needed, in the order they surfaced:
   `Transform`, `GlobalTransform`,
   `bevy::transform::components::TransformTreeChanged`, `Visibility`,
   `InheritedVisibility`, `ViewVisibility`, `Name`, `Mesh3d`,
   `MeshMaterial3d<StandardMaterial>`,
   `bevy::ecs::hierarchy::Children`, `bevy::ecs::hierarchy::ChildOf`,
   `bevy::animation::AnimatedBy` (**not** `AnimationTarget` — that type
   doesn't exist by that name in Bevy 0.18; this is the actual
   per-bone-animation-curve-target component), `AnimationPlayer`,
   `bevy::camera::primitives::Aabb`, `bevy::gltf::GltfExtras`,
   `GltfSceneExtras`, `GltfMeshExtras`, `GltfMaterialExtras`,
   `GltfMeshName`, `GltfMaterialName`,
   `bevy::mesh::skinning::SkinnedMesh`. Notably even core
   ECS-relationship types (`Children`/`ChildOf`) needed explicit
   registration — `MinimalPlugins` doesn't register them, only
   `TypeRegistrationPlugin`'s own baseline set.
5. Confirmed no regression: ran the server normally afterward (no spike
   code) — client connects, zombies spawn/replicate as before, only a
   benign `bevy_gltf: CompressedImageFormatSupport resource not found`
   warning (expected — that resource is a `RenderPlugin` thing this
   server correctly doesn't have).

**Exit criterion met.** This phase's changes (Cargo features, plugins,
`init_asset`, `register_type` calls) are permanent — Phase 2 builds on
top of them rather than redoing this discovery process.

## Phase 2 — Real zombies carry a live skeleton + bone hitbox entities — ✅ DONE

**Confirmed working**: every real server-spawned zombie now carries a
resolved `Head` bone entity — verified with a temporary per-second log
(`[hitbox-phase2] head bone resolved: N/N zombies`, always 1:1 across
fluctuating zombie counts as some got killed/respawned during the test),
then removed once confirmed.

What actually landed (close to the original plan, with one
simplification):

1. Moved `ZombieHitboxBones` + `wire_zombie_hitbox_bones` from
   `crates/client/src/lib.rs` to `crates/shared/src/lib.rs` as `pub`,
   with client re-importing both from `shared` — no call-site changes
   needed beyond the `use` list.
2. `crates/server/src/main.rs`: added a new `on_zombie_spawned` observer
   (`On<Add, Zombie>`, mirroring the client's existing
   `on_zombie_replicated`) that spawns the same `zombie.glb` `SceneRoot`
   as a child the instant any zombie entity is created — decoupled from
   `spawn_zombies` itself (which has 6 call sites across different
   systems; adding the observer avoided threading `Res<AssetServer>`
   through all of them). `spawn_zombies`'s own bundle just gained
   `ZombieHitboxBones::default()` + `Visibility::default()` (the latter
   not in the original plan — needed to silence Bevy's B0004 warning:
   the child scene's `InheritedVisibility` requires the *parent* to have
   `Visibility` too, not just itself).
3. Animation: **simplified from the plan's "match ZombieMotion's
   moving/sprinting state" to always Idle** for this phase — a
   `ServerZombieAnim` resource (one shared `AnimationGraph`, built once
   at Startup, same one-graph-many-players shape as Phase 1's spike and
   the client's own `ZombieAnim`) plus `wire_server_zombie_animations`
   (watches `Added<AnimationPlayer>`, attaches the graph, plays Idle on
   loop). Real walk/run/attack-state-matched animation selection is
   deferred to whenever Phase 3's pose-level rewind is being built —
   there's no point picking the "correct" clip per zombie until the
   history buffer can actually rewind to it; matching today would be
   thrown away work.
4. Registered `wire_zombie_hitbox_bones` (now `pub` in `shared`) in the
   server's `Update` schedule alongside the new `wire_server_zombie_animations`.
5. Verified no regression beyond Phase 1's own check: server starts,
   client connects, zombies spawn/replicate/die/respawn normally.

**Exit criterion met** — every real zombie resolves its hitbox bones
within a couple frames of spawning, confirmed at fluctuating population
sizes (6-10 in the test run), not just the single spike zombie from
Phase 1.

## Phase 3 — Pose-level lag compensation (the Valorant/Overwatch part) — ✅ DONE (3a)

Implemented as **3a** (below) — 3b remains a documented option, not
needed unless playtesting later shows head hitboxes visibly lagging a
fast animation.

**What landed** (`crates/server/src/main.rs`):
1. `ZombieHistory`'s entry type grew from `(f32, Vec3, Quat)` to
   `(f32, Vec3, Quat, BoneLocalOffsets)`, where `BoneLocalOffsets` stores
   each resolved bone's position **relative to the root, in the root's
   own local frame** (`rotation.inverse() * (bone_world - root_world)`)
   — not a raw world position. This is the detail that makes rewinding
   correct across rotation, not just translation: reapplying a *past*
   rotation to a *local* offset reconstructs the bone's true past world
   position even after the zombie has since turned to face a different
   way, whereas a stored world position would silently assume orientation
   never changed.
2. `record_zombie_history` now also reads `ZombieHitboxBones` + a
   `Query<&GlobalTransform>` each tick to compute those offsets — cheap,
   since `GlobalTransform` is already computed for free by Bevy's own
   transform propagation; no new animation sampling cost.
3. `zombie_history_at`'s return type changed from `Option<(Vec3, Quat)>`
   to `Option<(Vec3, Quat, BoneWorldPositions)>` — it now reconstructs
   full bone world positions at the target instant internally, so callers
   never need to know the local-offset trick exists at all.
4. `handle_fire_shot`'s rewound path now gets fully rewound bones
   straight from `zombie_history_at` — no live `ZombieHitboxBones`/
   `GlobalTransform` query needed there anymore, simplifying that
   function's own signature back down. The live-hit fallback path
   (no history young enough to rewind) and `debug_bot_fire` (deliberately
   never rewinds at all — no client round-trip to compensate for) still
   resolve bones live via the Phase 4 `resolve_bone_world_positions`
   helper.
5. `shared::zombie_hit_damage_bones` was refactored to take an
   already-resolved `BoneWorldPositions` struct instead of a
   `&ZombieHitboxBones` + entity-lookup closure — decouples the damage
   *decision* from *how the positions were obtained* (live query vs.
   history rewind), so Phase 3 slotted in without changing that
   function's own logic, only its callers.

**Verified:**
- A new permanent regression test
  (`crates/server`, `hitbox_phase3_tests::
  rewind_uses_the_rotation_recorded_at_that_instant_not_the_latest_one`)
  directly proves the rotation-correctness claim: a bone offset recorded
  while facing one way, rewound after the zombie has since turned 90°,
  reconstructs the ORIGINAL world position, not the current-orientation
  one. Passed.
- All 5 Phase 4 tests still pass after the `BoneWorldPositions` refactor
  (`cargo test -p server -p shared`: 6 passed, 3 suites).
- Full workspace build (`server`+`client`+`shared`) still 0 errors;
  server ran cleanly with no panics afterward.

## Phase 4 — Replace the damage decision — ✅ DONE (skipped ahead of Phase 3)

**Deliberately done before Phase 3** (pose-level rewind) at the user's
own call: real per-bone hit detection using each bone's *current* world
position (not yet rewound to the shot's timestamp) is still strictly
better than the old fixed-height-band guess for the common case
(idle/walking zombies), and ships the actually-requested improvement
sooner. The known gap — a moving/attacking zombie's bones tested
slightly out of sync with what the shooter's screen showed — is
explicitly documented in `zombie_hit_damage_bones`' own doc comment and
is exactly what Phase 3 would close later, not a surprise regression.

**What landed** (`crates/shared/src/lib.rs`):
1. `zombie_hit_damage_bones(hit_point, zombie_transform, bones:
   &ZombieHitboxBones, bone_position: impl Fn(Entity) -> Option<Vec3>)`
   — spheres (not capsules; simpler, and a bone is a point not a
   segment) centered on each resolved bone's world position, tested
   head-first then limbs then chest then belly (same precedence order
   `zombie_hit_damage_geometric` used, preserving "head always wins if
   it's in range" behavior), falling back to the geometric guess for any
   region whose bone hasn't resolved AND for the case where the hit point
   is genuinely outside every bone's sphere — never a silent 0-damage
   miss on a raycast that already confirmed a hit.
2. New radius constants (`HEAD_HITBOX_RADIUS` etc.) as fractions of
   `ZOMBIE_RADIUS` rather than fresh `ZOMBIE_SCALE_FACTOR` multiples, so
   the two hitbox systems stay tunable relative to each other.
3. `bone_position` is a plain closure, not a `Query` type, specifically
   so the same function is callable from both `crates/server`
   (`handle_fire_shot`'s two paths + `debug_bot_fire`, all newly querying
   `&ZombieHitboxBones` + a separate `Query<&GlobalTransform>`) and
   `crates/client` (`shoot()`'s local prediction, bundled into the
   already-overflowing `VfxAssets` param struct — same "no room at the
   top level" pattern used for `stance`/`recently_hit` earlier).
4. `zombie_hit_damage_geometric` and the `HITBOX_*_MIN_Y`/lateral-distance
   constants were **kept**, not deleted — they're the fallback path, not
   dead code, per point 1 above.
5. Verified with 5 unit tests (`crates/shared`, `#[cfg(test)] mod
   hitbox_phase4_tests`, temporary — see the module's own note) covering:
   headshot on the head bone, a limb hit at head-height resolving to
   *limb* damage (the exact case the old lateral-distance check existed
   to catch — bone-based detection gets it for free), a chest hit, and
   both fallback paths (unresolved bones; resolved bones but hit point
   outside every sphere) matching `zombie_hit_damage_geometric` exactly.
   All 5 passed. Full build (`server`+`client`+`shared`) still 0 errors;
   server ran cleanly afterward with no panics.

## Phase 5 — Performance guardrails — ✅ DONE (measured: no guardrail needed)

**Measured, then decided** — per the plan's own instructions, not a
fixed amount of upfront optimization work.

**Method:** temporarily bumped `ZombieTargetCount`'s default from 10 to
100 (the full `ZOMBIE_SPAWNS` grid — the worst case this plan flagged)
and added a per-second logging probe measuring (a) real wall-clock tick
duration (`Time::delta_secs()` — the actual cost of everything that
frame: physics, networking, `AnimationPlugin`'s own internal curve
sampling, our own bone bookkeeping) against the `SERVER_TICK_HZ` budget,
and (b) the isolated cost of just reading every zombie's resolved bone
`GlobalTransform`s (the same per-tick work `record_zombie_history` does),
to separate "our own new bookkeeping" from everything else. Ran twice —
once with the Phase 2+ skeleton-spawning observer active, once with it
disabled (a true apples-to-apples baseline, both at the same 100-zombie
population) — all temporary, removed after.

**Results:**
- Isolated bone-read cost: **~0.01ms** per tick across 100 zombies —
  utterly negligible, confirms the research doc's own prediction that
  per-bone `GlobalTransform` reads are effectively free.
- Real tick duration: **avg ~37-39ms, max up to ~98ms**, against a
  33.33ms budget — a genuine overrun at 100 zombies.
- **But this overrun is identical with the skeleton-spawning observer
  disabled** (avg ~37-39ms, max up to ~65ms, same range) — i.e., it's
  **pre-existing** (physics/AI/navmesh cost at 100 zombies, unrelated to
  this plan) and was already there before Phase 1 touched anything.

**Conclusion:** the bone-hitbox work (Phases 1-4) adds no measurable
per-tick cost at any zombie population tested, including the
theoretical maximum. The 100-zombie tick overrun is real but is a
separate, pre-existing server performance characteristic — out of scope
for this plan and not something the guardrails below (staggering,
capping bone-tracking by distance) would even address, since it isn't
the bone tracking that's expensive. No Phase 5 code changes were kept;
if the pre-existing 100-zombie overrun needs fixing later, that's its
own investigation (likely `move_zombies`' navmesh/steering cost or
avian3d's spatial queries at that entity count, going by the same
overrun existing with zero skeletons in play).

## Suggested order of work / shippable checkpoints

1. Phase 1 alone is a safe, low-risk spike — merge it independently once
   it proves headless glTF+animation works, even with no gameplay change.
2. Phase 4's *client-only* half (swap `shoot()`'s local prediction to use
   real bones it already has) can ship on its own, independent of the
   server work, as a quick visible win for the shooter's own hit
   feedback — though the server remains authoritative and would still
   correct/override with the old geometric guess until Phases 2-4's
   server half lands.
3. Phases 2-3-4 (server side) are the real fix and should land together
   — there's no useful intermediate state where the server has bones but
   still uses the Y-threshold guess, or vice versa.
4. Phase 5 is measure-then-decide, not a fixed amount of upfront work.

## Open questions to confirm before starting Phase 1

- Does `crates/server`'s `bevy` dependency with `bevy_gltf`+
  `bevy_animation` added actually compile cleanly with `default-features
  = false` and no render backend? (Bevy's `bevy_gltf` has historically
  had some coupling to `bevy_pbr` material types for full material
  support — likely fine since we only need the scene graph + skinning
  data, not materials, but worth confirming in Phase 1's own exit
  criterion before assuming Phases 2+ are unblocked.)
- Confirm target zombie count under real play conditions (not just the
  spawn grid's theoretical max) to size Phase 5's guardrails realistically
  instead of guessing.

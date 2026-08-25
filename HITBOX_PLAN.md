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

## Phase 2 — Real zombies carry a live skeleton + bone hitbox entities

1. Move `ZombieHitboxBones` (component) and `wire_zombie_hitbox_bones`
   (system) from `crates/client/src/lib.rs` to `crates/shared/src/lib.rs`
   so both crates register the same component/system instead of
   duplicating it. Re-export from client to avoid breaking existing call
   sites.
2. `crates/server/src/main.rs`'s `spawn_zombies`: alongside the existing
   `Collider::capsule` (kept as-is — the broad-phase shape, see Phase 3),
   attach `ZombieHitboxBones::default()` to the zombie root and spawn the
   `SceneRoot` glTF as a child, exactly mirroring the client's
   `zombie_cosmetic_bundle()`/`zombie_mesh_bundle()` split. Also spawn/
   attach whatever `AnimationPlayer` + the zombie's current animation
   clip (idle/walk/run — the server already tracks zombie motion state
   for `ZombieMotion`, reuse that to pick which clip plays, matching
   whatever the client shows for visual consistency, even though the
   server doesn't need to *render* it — it needs its `AnimationPlayer` to
   be actually advancing so bone transforms are correct at query time).
3. Register `wire_zombie_hitbox_bones` in the server's `Update` schedule
   (same system, now living in `shared`).
4. **Exit criterion:** every server-spawned zombie has non-`None`
   `ZombieHitboxBones.head`/`chest`/`belly`/at least one limb within a
   couple frames of spawning, verified via a temporary debug log/assert
   in a test build.

## Phase 3 — Pose-level lag compensation (the Valorant/Overwatch part)

The current `ZombieHistory` only rewinds the **root** transform. A moving/
attacking zombie's bones move relative to that root too (arm swing,
lunge), so rewinding only the root and then reading the bones' *current*
(non-rewound) local offsets would still be slightly wrong during a lunge/
attack animation — acceptable for a first cut, but worth being explicit
about the gap:

- **3a (cheaper, recommended first cut):** keep bone shapes rigid
  relative to the root at each instant — i.e., when rewinding for lag
  comp, snapshot each zombie's **per-bone local offsets from root**
  alongside the root transform, not just root translation/rotation.
  Extend `ZombieHistory`'s entry type from `(f32, Vec3, Quat)` to include
  a small fixed-size array of bone-local offsets (head/chest/belly + a
  representative limb point is enough — you don't need all 12 limb
  bones in history, just whichever ones matter for damage regions).
  This makes `record_zombie_history` read each bone's `GlobalTransform`
  relative to the zombie root once per tick (cheap — it's already
  computed by Bevy's transform propagation, no new animation sampling
  cost, just extra reads) and store it. `zombie_history_at` returns the
  full rewound bone snapshot instead of just root pos/rot.
- **3b (full fidelity, do only if 3a proves visibly wrong in testing):**
  also snapshot/rewind the `AnimationPlayer`'s play-head time and
  re-sample the animation curve at the exact rewound instant, rather than
  relying on "whatever bone-local offset happened to be recorded that
  tick." This is strictly more accurate but meaningfully more complex
  (needs access to the `AnimationClip` curves directly, bypassing the
  live `AnimationPlayer`, to evaluate a pose at an arbitrary past time
  without disturbing the actually-playing animation). Given
  `LAG_COMPENSATION_MAX_SECS` is only 0.3s and zombie attack/lunge
  animations are relatively slow, 3a's "nearest recorded tick's bone
  offsets" is very likely close enough — don't build 3b until profiling/
  playtesting shows head hitboxes visibly lagging behind a fast animation.

## Phase 4 — Replace the damage decision

1. New function in `crates/shared/src/lib.rs`, replacing
   `zombie_hit_damage_geometric` as the thing `handle_fire_shot` calls:
   given the rewound bone snapshot (Phase 3's output) and the shot ray,
   test small shapes per region instead of one Y-threshold comparison:
   - Head: a sphere at the head bone's rewound world position, radius
     tuned to the model (start from `HITBOX_HEAD_MIN_Y`'s existing
     "roughly head-sized" intuition, just as a sphere radius instead of a
     Y cutoff).
   - Chest/belly: capsule or sphere at the corresponding bone.
   - Limbs: small capsules/spheres at representative limb bones (elbow/
     knee midpoints are enough — no need for one shape per finger bone).
   - Keep it as a **prioritized raycast-vs-shapes test in a fixed order**
     (head first, so an overlapping head+chest shape still resolves to
     headshot, matching today's `if head else if limb else if chest...`
     precedence) rather than "closest shape wins," to preserve the
     existing head-priority behavior players are used to.
2. `handle_fire_shot` (both the rewind path and the live-hit fallback
   path) and `debug_bot_fire`: replace the
   `zombie_hit_damage_geometric(hit_point, &rewound_transform)` call with
   the new per-bone-shape test, passing the rewound bone snapshot instead
   of just `rewound_transform`.
3. Client's `shoot()` local-prediction path: same swap, but note the
   client already has real live (non-rewound) bones via its own
   `ZombieHitboxBones` — it doesn't need Phase 1-3's server-side skeleton
   work at all, it just needs the new shared per-bone-shape test function
   instead of the old geometric one. This is a smaller, independent
   change and could ship before the server-side phases are done (client
   prediction would then be more accurate than the server's authoritative
   answer during the transition, which is fine — prediction has always
   been "best effort, corrected by server" here).
4. Delete `zombie_hit_damage_geometric` and the `HITBOX_*_MIN_Y` /
   lateral-distance constants once nothing calls them.

## Phase 5 — Performance guardrails

The research doc flagged this: 30Hz × N zombies × per-bone
`GlobalTransform` reads is bounded and cheap (transform propagation
already happens for free), but N zombies each running a live
`AnimationPlayer` server-side is new CPU work that didn't exist before.

1. Reuse the existing `entity_phase`-based staggering pattern (already
   used for `path_recalc_timer`) if animation advancement turns out to be
   measurably expensive — e.g. only advance/re-sample animation state for
   zombies within some distance of a player, or at a lower rate for
   off-screen/far zombies, mirroring `update_zombie_lod`'s existing
   distance-based cost-cutting on the client.
2. Add a quick before/after measurement: server tick duration with N
   zombies at rest vs. Phase 2 enabled, using whatever profiling the
   project already has (check for existing perf-measurement systems
   before adding new ones — `PerfTimings` exists client-side per the
   earlier codebase read; confirm whether an equivalent exists or is
   worth adding server-side).
3. **Decision point:** if per-tick cost is unacceptable at the target
   zombie count (the `ZOMBIE_SPAWNS` grid suggests up to ~100), consider
   capping how many zombies get real bone-tracking (e.g. only zombies
   within `MAX_SHOT_DISTANCE` of any player, falling back to the old
   capsule+geometric guess for the rest — they're not shootable anyway at
   that range) rather than abandoning the approach.

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

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

## Phase 1 — Server can load and animate the skeleton (no gameplay change yet)

**Goal:** prove a headless zombie skeleton can exist and animate on the
server with zero rendering, before touching damage logic at all.

1. `crates/server/Cargo.toml`: add `"bevy_gltf"` and `"bevy_animation"`
   to the `bevy` feature list (currently `["bevy_log", "bevy_render"]`).
   Do NOT add `bevy_pbr`/`bevy_winit`/`bevy_render`'s GPU backends — the
   server never draws anything, it only needs the *data* (scene graph +
   animation curves), so `AnimationPlugin` + `GltfPlugin` should attach
   without pulling in a renderer. Verify this compiles and the server
   still starts headless (no window, no wgpu init) — if `bevy_gltf`
   transitively drags in image-decoding or render-target types that
   don't build without a `RenderPlugin`, that surfaces here first.
2. `crates/server/src/main.rs`: add `AnimationPlugin` to the plugin list
   next to `ScenePlugin`.
3. Spawn ONE test zombie's `SceneRoot` + `AnimationPlayer` server-side,
   reusing the client's own asset path/label
   (`GltfAssetLabel::Scene(0).from_asset("models/zombie.glb")`,
   `GltfAssetLabel::Animation(n)`), gated behind a temporary debug flag or
   just a Startup system you delete once this phase is verified.
4. Reuse `wire_zombie_hitbox_bones` verbatim (move it to `shared` so both
   client and server can call it — it doesn't touch anything
   client-only) to confirm bone entities resolve and their
   `GlobalTransform` updates as the animation plays, by logging the
   `Head` bone's world Y once per second for a few seconds.
5. **Exit criterion:** server-side log shows the head bone's height
   oscillating/changing as the idle or walk animation plays, with no
   window ever opened and no render errors.

This phase touches nothing gameplay-visible and can be reverted/disabled
trivially if it turns out `bevy_gltf`/`bevy_animation` pull in more than
expected for a headless build.

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

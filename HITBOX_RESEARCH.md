# Hitbox research: mesh/bone-based hit detection vs. current height-guessing

## 1. What the game does today

Every hit check (server real shots, server debug-bot shots, and the client's
own local prediction in `shoot()`) does the exact same two-step thing:

1. **One raycast against one capsule per zombie** —
   `Collider::capsule(ZOMBIE_RADIUS, ZOMBIE_LENGTH)` in
   `crates/server/src/main.rs` (real shots + debug bots) and
   `crates/client/src/lib.rs` (`shoot()`'s local prediction). This capsule
   is the *entire* zombie body approximated as one cylinder — no limbs, no
   head, just a single blob sized from the mesh's measured bounding-box
   height (`ZOMBIE_MESH_REFERENCE_HEIGHT`).

2. **Classify the hit point by height** —
   `zombie_hit_damage_geometric()` in `crates/shared/src/lib.rs` takes the
   raycast's hit point, transforms it into the zombie's local (yaw-undone)
   space, and buckets it by fixed Y thresholds:

   ```rust
   pub fn zombie_hit_damage_geometric(hit_point: Vec3, zombie_transform: &Transform) -> f32 {
       let relative = hit_point - zombie_transform.translation;
       let local = zombie_transform.rotation.inverse() * relative;
       let lateral = local.x.hypot(local.z);

       if local.y >= HITBOX_HEAD_MIN_Y { HEAD_DAMAGE }
       else if lateral >= HITBOX_LIMB_LATERAL_MIN { LIMB_DAMAGE }
       else if local.y >= HITBOX_CHEST_MIN_Y { CHEST_DAMAGE }
       else if local.y >= HITBOX_BELLY_MIN_Y { BELLY_DAMAGE }
       else { LIMB_DAMAGE }
   }
   ```

   The code's own comment admits it: *"Rough estimates ... not visually
   calibrated"* and *"the server has no skeleton/bone data at all — this
   geometric approximation is now the only method."* `HEAD_DAMAGE` is a
   guaranteed one-shot kill, so this height threshold is literally deciding
   headshot-or-not for the entire game.

**The irony:** the client already resolves and tracks the zombie's real
`Head` bone (and chest/belly/limb bones) every frame —
`ZombieHitboxBones` + `wire_zombie_hitbox_bones()` in
`crates/client/src/lib.rs` — but that data is wired *only* into a debug
gizmo (`draw_zombie_hitboxes`) that draws colored spheres for visualization.
It's never consulted for damage. Several comments near it are stale,
still referencing a `zombie_hit_damage` function that no longer exists
(replaced by the geometric version once damage authority moved
server-side).

**Why it ended up this way:** the server is headless — it never loads the
`zombie.glb` scene, so it has no skeleton, no bones, no skinned mesh at
all. It only knows each zombie's root `Transform` (+ a short position/
rotation history for lag-comp rewind, see `zombie_history_at`). A real
bone-based hitbox needs *some* representation of "where the head is right
now, given the current animation pose," and the server currently has zero
way to answer that question — hence "guess from a fixed height fraction
of the root capsule" as the only option left.

## 2. How other games do this

### Multiple simple shapes per bone, not the render mesh
No shipped game hit-tests the actual skinned render mesh (triangle-level
raycasts against thousands of animated verts, every shot, for every
player) — it's needlessly expensive and gives a jagged, animation-jittery
hit volume. Every source I found uses a **small number of simple
primitives** (spheres/capsules/boxes) individually attached to specific
bones, sized to loosely cover that body part:

- **CS2**: capsule-based hitboxes for every body part, standardized to
  identical proportions across all player models (CS:GO's hitboxes varied
  up to ~16.75% by skin/model, which was a genuine competitive-fairness
  bug they fixed by standardizing this in CS2).
- **Unreal-style skeletal hit detection**: a capsule (or a `PhysicsAsset`
  full of per-bone shapes) parented to a bone via a socket, offset in
  bone/component space, so it moves with that bone's animated transform
  every frame.
- **Godot-style "weakspot" pattern**: a small `Area3D` hitbox parented
  under a `BoneAttachment3D` node for headshot detection specifically —
  same idea, different engine vocabulary.

### The hard part isn't the shapes, it's *when* they're evaluated
This is the actually interesting research finding, and it's the part
directly relevant to this codebase's headless-server constraint:

- **Valorant** (per their own dev blog on hit registration): the server
  does a coarse **bounding-box sphere-cast first** against every entity's
  root, and only for entities that pass that broad-phase does it pay the
  cost of **animation rollback** — resimulating that entity's skeleton
  back to the exact timestamp the shot was fired at server-side, to get
  the real bone poses at that instant — before doing the precise per-bone
  raycast. This two-phase (cheap broad-phase, expensive narrow-phase only
  on candidates) is what keeps it affordable with 10 players' worth of
  skeletons.
- **Overwatch** (GDC 2017): same shape — every entity carries hitbox
  colliders, and the server does a rewind ("favor the shooter" lag comp)
  before testing them, not just a rewind of root position like this
  codebase already does for zombies.
- This is the direct generalization of what `zombie_history_at` /
  `rewind_time` already do here today: they rewind the zombie's **root**
  transform to the shot's timestamp. Real games do the same rewind, just
  one level deeper — they also rewind the **skeleton pose**, not only the
  root, before hit-testing individual bones.

### Where the "head = instant kill" idea itself is not unusual
Treating a head-hitbox hit as a guaranteed kill (independent of the
weapon's normal per-body-part damage multiplier) is exactly how this
codebase already models `HEAD_DAMAGE = ZOMBIE_MAX_HEALTH`. Nothing to
change there conceptually — only *how a hit is classified as "head" or
not* is the actual gap.

## 3. Why "just port `ZombieHitboxBones` to the server" is the crux, not a detail

The client's bone tracking works today because the client *renders* the
`zombie.glb` scene — Bevy's glTF loader spawns real bone entities with
`Name`/`ChildOf` and its animation system drives their `Transform` every
frame, and `TransformSystems::Propagate` (Bevy core) fills in
`GlobalTransform` automatically. That's it — no special work, it's a side
effect of the mesh being loaded and animated for rendering.

The server has none of that because it never renders, but there's no
requirement that only rendering code can load/animate a glTF scene. The
same skeleton + `AnimationPlayer` machinery is a purely-ECS `Transform`
graph; you can run it in a scene with no camera, no `Mesh3d`, no render
plugins at all. This is the standard "headless Bevy app" pattern the
server already uses for other things (`bevy::app::App` with `MinimalPlugins`
+ whatever specific plugins you need, e.g. `TransformPlugin`,
`AnimationPlugin`, `AssetPlugin` to load the `.glb`, `ScenePlugin` to
spawn it) — no `RenderPlugin`/`WinitPlugin`/`WgpuPlugin` needed since
nothing needs to draw to a screen.

## 4. Options, roughly cheapest → most accurate

| # | Approach | What changes | Cost | Accuracy |
|---|---|---|---|---|
| A | **Tune the height bands** (current approach, just better-calibrated numbers) | Only `HITBOX_*_MIN_Y` constants; measure against the actual mesh instead of eyeballing | ~free | Still fundamentally guessing; a crouching/leaning/prone zombie pose breaks any fixed-height assumption |
| B | **Static per-part capsules in the zombie's *rest pose*, moved as a group with the root** | Replace the single body capsule with a small fixed set (head sphere + torso capsule + 2 limb capsules) positioned relative to the root, still rigid (no per-frame animation) | Small — a few more raycasts per shot, all against simple shapes | Fixes today's "one blob" approximation, but still wrong whenever the zombie's *pose* changes (arms out, lunging, crawling) since the shapes don't animate |
| C | **Server loads + animates the real skeleton (headless), attaches hitbox colliders to real bones, does the rewind at the pose level like Valorant/Overwatch** | Server gains a lightweight non-rendering `zombie.glb` + `AnimationPlayer` instance per zombie (or per zombie archetype+pose, reusable), hitbox colliders become children of real bones and get `GlobalTransform` for free from Bevy's own propagation, `zombie_history_at` extended to also snapshot pose (or animation-time) so the rewind is a real animation rewind, not just a position/rotation rewind | Real work — new server-side asset loading, an animation-time history buffer per zombie, more entities/colliders to raycast (bounded, e.g. 4-6 shapes per zombie, not the render mesh) | Matches how CS2/Valorant/Overwatch actually do it; fixes the "guessing" problem at its root, and headshots stay correct as the zombie animates |

**C is the direct fix for what the user is asking about** ("làm sao để khi
bắn biết trúng địch thay vì đoán height") — B is a reasonable *incremental*
step if C is too large a lift right now (still an improvement: real
per-part shapes instead of Y-bands, at near-zero extra cost, while leaving
the door open to animate them later), and A is not really worth doing on
its own since it doesn't fix the underlying guess, just makes the guess
slightly less wrong for the idle pose specifically.

## 5. Suggested next step

Given the size of C, recommend scoping it as its own follow-up plan
(headless server animation loading is a nontrivial new piece of
infrastructure — asset loading path, per-zombie animation state, extending
the lag-comp history buffer to carry pose data, and a decision on how many
zombies' skeletons the server can afford to animate every tick before it
becomes the new perf bottleneck). Happy to turn section 4's option C into
a concrete step-by-step implementation plan next if that's the direction
you want to go — or start with B as a smaller, shippable-today
improvement while C is designed properly.

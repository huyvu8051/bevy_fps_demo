# Zombie animation plan

Every animation clip the zombie models need, and the exact game state that
plays it. The player equivalent is [`PLAYER_ANIMATION_PLAN.md`](PLAYER_ANIMATION_PLAN.md);
shared meshy.ai / root-motion / duration guidance is in
[`CHARACTER_ANIMATION_PLAN.md`](CHARACTER_ANIMATION_PLAN.md). The mechanics
of wiring a finished `.glb` in (clip index order, bone names, scale) are in
[`CHARACTER_IMPORT_CHECKLIST.md`](CHARACTER_IMPORT_CHECKLIST.md).

## How zombie animation works (so the clip list makes sense)

- State machine is `ZombiePose` (`crates/shared/src/zombie.rs`): `Idle`,
  `Walk`, `Run`, `Crawl`, `Bite`. `zombie_pose_for(distance,
  health_fraction, moving)` picks one; `update_server_zombie_pose` (server)
  and `update_zombie_animation` (client) both play the matching clip with a
  hard cut (`stop_all()` then `play(node).repeat()`), no blend.
- Per-kind clip mapping is `zombie_kind_pose_clips(kind) -> [idle, walk,
  run, crawl, bite]` — clip **indices** into that kind's `.glb`. Classic
  and Meshy have different files with different clip orders, so this
  indirection is the only thing that has to match a regenerated file.
- **The zombie always turns to face its movement direction**
  (`turn_towards` in `move_zombies`). It never strafes, backpedals, or
  moves diagonally relative to its own body — so there is **no** 8-way /
  directional locomotion set. One forward Walk + one forward Run is all
  the locomotion it can ever use.
- **Root motion: OFF, in place.** Position is 100% code-driven
  (`move_zombies` velocity → `Transform`). A clip that travels the root
  makes the mesh foot-slide / drift off its collider. Say "in place, no
  root motion" in every prompt.

## Current state

| Kind | File | Clips it ships |
|---|---|---|
| Classic | `assets/characters/zombie/mesh/zombie.glb` | 5: BITE=0, CRAWL=1, IDLE=2, RUN=3, WALK=4 — all wired |
| Meshy | `assets/characters/zombie_meshy/mesh/zombie_meshy.glb` | 7: 0–4 = placeholder Text-to-Motion, 5 = Running, 6 = Walking. Only Walk/Run are usable; `zombie_kind_pose_clips` currently falls Idle/Crawl/Bite back to clip 0 |

Everything below is what to generate on the **Meshy** rig (and optionally
regenerate on Classic later for parity).

## Tier 1 — core, required

The pose machine already dispatches all 5. Meshy is missing usable Idle,
Crawl and at least one attack.

| # | Clip | Plays when | Loop | Length |
|---|---|---|---|---|
| 1 | **Idle** | `moving == false` (no target / out of range) | loop | 2–3 s |
| 2 | **Walk** | moving, target farther than `ZOMBIE_RUN_DISTANCE` (20 m) | loop | ~1–1.2 s (one L-R-L step cycle) |
| 3 | **Run** | moving, within 20 m, health > 35 % | loop | ~0.8–1 s |
| 4 | **Crawl** | health ≤ `ZOMBIE_CRAWL_HEALTH_FRACTION` (35 %) — dragging, can't stand | loop | ~1.5–2 s |
| 5 | **Attack** (see Tier 4 for variants) | within `ZOMBIE_STOP_DISTANCE` (1.2 m) of the target | loop | ~0.8–1.2 s |

## Tier 2 — movement extras (need a small code hook)

| # | Clip | Plays when | Loop | Length | Hook needed |
|---|---|---|---|---|---|
| 6 | **Fall** | airborne — `!motion.grounded` and falling | loop | ~0.6–1 s | add `ZombiePose::Fall`; pass `grounded` / `vertical_velocity` into `zombie_pose_for` |
| 7 | **Land / get-up** | first frames after touching down from a real fall | one-shot | ~0.8–1.2 s | same hook + a short "just landed" timer |

## Tier 3 — damage reactions (need a code hook)

| # | Clip | Plays when | Loop | Length | Hook needed |
|---|---|---|---|---|---|
| 8 | **Hit reaction / flinch** | took bullet damage, not killed | one-shot, interrupts current pose | ~0.3–0.5 s | "recently damaged" timer set in `handle_fire_shot` (server already applies the damage there) |
| 9 | **Stumble / knockback** | `KnockbackVelocity` magnitude over a threshold | one-shot | ~0.6–1 s | read `knockback.0.length()` in the pose dispatch (`KnockbackVelocity` component already exists) |

## Tier 4 — attack variety (replaces the single Bite)

Right now `ZombiePose::Bite` is one clip. Generate several and pick one at
random each time a zombie enters attack range (wiring: an `attack_variant:
u8` rolled in `update_server_zombie_pose` / replicated, or rolled per
zombie at spawn for a consistent "this one's a biter" feel — TBD). All
loop-able (the zombie holds in range and keeps attacking), all in place.

| # | Clip | Motion | Length |
|---|---|---|---|
| 5a | **Bite** | head snaps forward, jaws clamp, shoulders hunch in | ~0.8–1 s |
| 5b | **Claw swipe — right** | right arm rakes down-across the body | ~0.7–0.9 s |
| 5c | **Claw swipe — left** | mirror of 5b, left arm | ~0.7–0.9 s |
| 5d | **Double grab** | both arms shoot out, grab and pull the victim inward | ~0.9–1.1 s |
| 5e | **Overhead smash** | both fists raised overhead, slam straight down | ~0.9–1.2 s |
| 5f | **Lunge bite** | tiny forward weight-shift + bite (any real closing distance is code-driven) | ~0.8–1 s |

Minimum useful set: **5a + 5b + 5c** (bite + two claw sides). 5d–5f are
gravy.

## Tier 5 — optional flavour (L4D / WWZ)

| # | Clip | Plays when | Loop | Length | Notes |
|---|---|---|---|---|---|
| 10 | **Death / collapse** | killed | one-shot | ~1.5–2 s | Currently **omitted on purpose** — a dead zombie despawns immediately server-side. Adding it means holding the despawn until the clip ends, which touches `ZOMBIE_RESPAWN_DELAY` and the hit-cleanup path. Decide before generating. |
| 11 | **Alert / scream** | just acquired a chase target (Idle/wander → chase) | one-shot | ~1–1.5 s | needs a big-brain state-transition hook (`ChaseAction` start) |
| 12 | **Wander / shamble** | has no target — slow aimless walk instead of frozen Idle | loop | ~1.5–2 s | needs a wander AI state; low priority |

## Not needed — and why

- **Backpedal / strafe / diagonal walk-run, 8-way locomotion** — the
  zombie rotates to face its heading, so it's always going "forward".
- **Climb / vault / mantle obstacles** (L4D common infected, WWZ) — this
  game's zombies path *around* obstacles (navmesh); there's no climb AI to
  drive such a clip.
- **Pile-up / climbing on each other** (WWZ signature) — a large AI
  feature, not an animation task.
- **Jump up onto ledges** — Fall + Land cover the only vertical motion the
  movement code produces (gravity down slopes / off edges).

## Recommended target clip order for a regenerated Meshy `.glb`

If you regenerate the whole set, produce them in this index order so the
mapping is obvious. `zombie_kind_pose_clips(ZombieKind::Meshy)` then just
becomes `[0, 1, 2, 3, 4]` for the core poses; the extras get their own
constants as they're wired.

```
0  Idle
1  Walk
2  Run
3  Crawl
4  Attack — Bite            (5a)
5  Attack — Claw right      (5b)
6  Attack — Claw left       (5c)
7  Attack — Double grab     (5d, optional)
8  Attack — Overhead smash  (5e, optional)
9  Attack — Lunge bite      (5f, optional)
10 Fall
11 Land / get-up
12 Hit reaction / flinch
13 Stumble / knockback
14 Death                    (optional)
15 Alert / scream           (optional)
16 Wander / shamble         (optional)
```

## Text-to-Motion prompts (copy-paste)

Target: the rigged zombie mesh, Animate tab. Check **Library** / **AI-Motions**
first (see `CHARACTER_ANIMATION_PLAN.md` §4). Every prompt ends "in place,
no root motion" — keep it.

**Zombie — Idle** (loop, ~2–3 s)
```
Zombie stands hunched in place, weight shifting slowly side to side, head
lolling and twitching, arms hanging limp, occasional shoulder spasm and
jaw movement. In place, no root motion.
```

**Zombie — Walk** (loop, ~1–1.2 s)
```
Zombie shambles forward in place, stiff uneven gait, one leg dragging
slightly, torso swaying, arms loose at the sides. One full step cycle,
hips stay centered, in place, no forward root motion.
```

**Zombie — Run** (loop, ~0.8–1 s)
```
Zombie sprints forward in place, aggressive lurching run, arms flailing
forward and reaching, head low, fast desperate gait. In place, no forward
root motion.
```

**Zombie — Crawl** (loop, ~1.5–2 s)
```
Badly wounded zombie drags itself forward flat on its belly, pulling with
both forearms and elbows, legs limp and trailing behind, slow relentless
low crawl staying close to the ground. In place, no root motion.
```

**Zombie — Attack: Bite** (loop, ~0.8–1 s)
```
Zombie attacks a target directly in front, lunging its head forward with
jaws snapping shut, shoulders hunching in, both hands clutching toward the
victim. Standing, in place, no root motion.
```

**Zombie — Attack: Claw swipe right** (loop, ~0.7–0.9 s)
```
Zombie rakes its right arm down and across its body in a savage clawing
swipe at a target in front, torso twisting with the swing, then resets.
Standing, in place, no root motion.
```

**Zombie — Attack: Claw swipe left** (loop, ~0.7–0.9 s)
```
Zombie rakes its left arm down and across its body in a savage clawing
swipe at a target in front, torso twisting with the swing, then resets.
Standing, in place, no root motion.
```

**Zombie — Attack: Double grab** (loop, ~0.9–1.1 s)
```
Zombie thrusts both arms straight forward, grabs a victim by the
shoulders and yanks them inward toward its mouth, then releases. Standing,
in place, no root motion.
```

**Zombie — Attack: Overhead smash** (loop, ~0.9–1.2 s)
```
Zombie raises both clenched fists high overhead and slams them straight
down onto a target in front, whole body dropping into the blow, then
straightens back up. Standing, in place, no root motion.
```

**Zombie — Attack: Lunge bite** (loop, ~0.8–1 s)
```
Zombie shifts its weight sharply forward and bites at a target just in
front, head thrusting out, arms sweeping in, then rocks back to a ready
crouch. In place, no root motion — any real forward movement is
code-driven.
```

**Zombie — Fall** (loop, ~0.6–1 s)
```
Zombie falls through the air, body limp and tumbling slightly, arms and
legs loose and flailing, making no attempt to brace or recover. Airborne
pose, in place, no root motion.
```

**Zombie — Land / get-up** (one-shot, ~0.8–1.2 s)
```
Zombie crashes to the ground on its front, then shoves itself up
unsteadily onto its feet, staggering upright and hunching back into a
ready stance. One-shot recovery from prone to standing, in place, no root
motion.
```

**Zombie — Hit reaction / flinch** (one-shot, ~0.3–0.5 s)
```
Zombie jerks and flinches from a bullet impact to the torso, upper body
twisting sharply away from the hit, a half-step stagger, then snaps back
to its stance. Very short one-shot, in place, no root motion.
```

**Zombie — Stumble / knockback** (one-shot, ~0.6–1 s)
```
Zombie recoils from a heavy impact to the chest, upper body thrown
backward, feet stutter-stepping to keep balance, arms flailing outward,
then catches itself and leans forward again. In place, no root motion —
the knockback push is code-driven.
```

**Zombie — Death / collapse** (one-shot, ~1.5–2 s; only if adopting it)
```
Zombie is shot and loses all control, folding at the knees and pitching
limply to the ground with no attempt to break the fall, then goes
completely still. One-shot, in place, no root motion.
```

**Zombie — Alert / scream** (one-shot, ~1–1.5 s)
```
Zombie snaps its head up as it spots prey, throws its head back and lets
out a screaming roar with arms raised and mouth wide, then lurches its
weight forward ready to charge. One-shot, in place, no root motion.
```

**Zombie — Wander / shamble** (loop, ~1.5–2 s)
```
Zombie drifts forward in place with an aimless slow shamble, head
drooping, feet shuffling and half-dragging, arms swinging loosely,
directionless. In place, no forward root motion.
```

## Suggested generation order

1. **Idle, Crawl** + one **Attack: Bite** — finishes the core set Meshy
   is missing; just remap `zombie_kind_pose_clips`, no new code.
2. **Attack: Claw right / Claw left** — cheap variety, needs the random
   attack-pick wiring.
3. **Hit reaction** — highest feel-per-effort; needs the "recently
   damaged" hook.
4. **Fall + Land** — needs the `ZombiePose::Fall` hook.
5. **Stumble / knockback**, then the optional attacks (5d–5f).
6. **Death, Alert, Wander** — each needs its own gameplay decision first.

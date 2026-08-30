# Player (soldier) animation plan

Every animation clip the player's 3rd-person soldier model needs, grounded
in the real state machine (`PlayerStance`, `ClientInput`,
`PlayerModelPose`) — not a wishlist. The zombie equivalent is
[`ZOMBIE_ANIMATION_PLAN.md`](ZOMBIE_ANIMATION_PLAN.md); shared meshy.ai /
root-motion / duration guidance is in
[`CHARACTER_ANIMATION_PLAN.md`](CHARACTER_ANIMATION_PLAN.md). Wiring a
finished `.glb` in (clip index order, bone names, scale) is
[`CHARACTER_IMPORT_CHECKLIST.md`](CHARACTER_IMPORT_CHECKLIST.md).

Rig to reuse: `crates/client/assets/characters/soldier/mesh/soldier.glb`
(already has the 5 shipped clips baked in, referenced by index). Upload it
to Meshy's Animate tab and add new motion clips on the same skeleton —
don't regenerate the mesh. Update `PLAYER_MODEL_ANIM_*`
(`crates/client/src/lib.rs`) to the new index order afterwards.

## Clip list — 13 clips, 5 done, 8 missing

Currently wired (`PLAYER_MODEL_ANIM_*` in `crates/client/src/lib.rs`,
`PlayerModelPose` in `crates/client/src/zombie_client.rs`): Idle, Walk,
Run, Shoot, Death.

| # | Clip | Status | Why it's real (not guessed) |
|---|---|---|---|
| 1 | Idle (standing) | done | — |
| 2 | Walk (standing) | done | — |
| 3 | Run/Sprint (standing) | done | — |
| 4 | Crouch Idle | missing | `PlayerStance::Crouching` exists and changes eye height/capsule length (`CROUCH_LENGTH`, `movement.rs`) — model still shows the standing pose |
| 5 | Crouch Walk | missing | `CROUCH_SPEED_MULTIPLIER` is a real, distinct movement speed — crouch-moving is a real state, not just a static pose |
| 6 | Prone Idle | missing | `PlayerStance::Prone` exists, own `PRONE_LENGTH` |
| 7 | Prone Crawl | missing | `PRONE_SPEED_MULTIPLIER` — moving while prone is a real mechanic, confirmed in `simulate_player_step` |
| 8 | Shoot | done | one-shot, triggered on fire |
| 9 | Reload | missing | reload only animates the local 1st-person view-model gun (procedural dip/tilt in `vfx.rs`) — other players see no reload motion on the 3rd-person body |
| 10 | Aim/ADS pose | missing | `AIM_MOVE_SPEED_MULTIPLIER` is real and distinct from normal walk speed, but the 3rd-person body never changes pose when aiming |
| 11 | Jump/Fall | missing (optional) | jump changes `vertical_velocity` only, no pose change — doesn't look broken, just stiff |
| 12 | Death | done | one-shot, triggered on `PlayerDiedFx` |
| 13 | Hit reaction (flinch) | missing, **needs a code hook first** | damage is pure HP subtraction (`health.0 -= damage`) with no event a client could react to — the clip alone does nothing until a trigger exists |

Crouch-run and prone-run aren't in this list on purpose: `send_player_input`
only allows sprint while `stance == Standing`, so those states can't happen.

## Reference — what PUBG / Left 4 Dead ship for the player

Scope-matching notes, not a checklist to copy.

**PUBG (3rd-person)**: Idle/Walk/Run × Standing/Crouch/Prone, Jump,
vault/climb, swim, **lean left/right**, **per-weapon** Aim + Reload poses
(rifle/pistol/sniper/shotgun held differently), weapon switch/holster,
grenade throw, melee, item-use (bandage/medkit — long, stationary),
vehicle enter/exit/drive, parachute, **Knocked/Downed** (crawling,
revivable), physics ragdoll death.

**Left 4 Dead — Survivor**: Idle/Walk/Run only (no crouch/prone),
per-weapon Shoot + Reload, melee push/shove, throw (Molotov/pipe bomb),
**Incapacitated** (kneeling, one-handed pistol, revivable), give/receive
item (2-player synced animation), per-special-infected capture reactions.

**Worth adopting, scope-appropriate:**
- Lean left/right — cheap, slots into the existing cover-shooting feel.
- Per-weapon Reload — worth planning now since more guns (SCAR-L) are
  already in the pipeline; cheaper to design for than to retrofit.

**Not worth it now:** vehicle animations, parachute, per-item use poses,
Incapacitated/revive (a real co-op feature, bigger than an animation task).

## Durations

| Clip | Loop or one-shot | Suggested length | Why |
|---|---|---|---|
| Crouch Idle | loop | 2–3 s | idle poses need a full breathing cycle before the loop seam |
| Crouch Walk | loop | 1–1.2 s | one step cycle (L-R-L) — matches the existing Walk clip so playback-speed scaling stays consistent |
| Prone Idle | loop | 2–3 s | same as Crouch Idle |
| Prone Crawl | loop | 1.5–2 s | crawl cycles read slower than a walk cycle; too short looks like a jittery loop |
| Reload | one-shot | 2–2.5 s | must cover mag-out → mag-in → charging-handle; cutting it short truncates the motion |
| Aim/ADS pose | loop | 1.5–2 s | mostly a held pose — short loop with subtle sway/breathing |
| Jump/Fall | one-shot | 0.8–1.2 s | matches actual airtime at this game's jump height |
| Lean left/right | one-shot (hold last frame) | 0.6–1 s | fast transition into the pose; the pose itself is held by code |
| Hit reaction | one-shot | 0.3–0.5 s | a quick flinch, must not interrupt aiming for long |

## Text-to-Motion prompts (copy-paste)

Every prompt keeps root motion **off** — see
[`CHARACTER_ANIMATION_PLAN.md`](CHARACTER_ANIMATION_PLAN.md) for why.

**Player — Crouch Idle** (loop, ~2–3 s)
```
Character crouches down and holds a stationary crouching idle pose,
weight balanced on both feet, knees bent, torso upright, alert ready
stance, holding a rifle at the chest.
```

**Player — Crouch Walk** (loop, ~1–1.2 s, in place — no root motion)
```
Character walks in place while crouching low, knees bent, staying low
to the ground, cautious tactical crouch-walk cycle, holding a rifle
raised at the chest. In place, hips stay centered, no forward travel —
only legs and arms cycle.
```

**Player — Prone Idle** (loop, ~2–3 s)
```
Character lies flat on the stomach in a prone position, elbows propped
forward on the ground, holding a rifle aimed forward, stationary prone
ready pose.
```

**Player — Prone Crawl** (loop, ~1.5–2 s, in place — no root motion)
```
Character crawls in place while lying prone on the stomach, pulling
with elbows and forearms, legs dragging behind, military low crawl,
holding a rifle in front. In place, torso stays centered, no forward
travel — only arms and legs cycle.
```

**Player — Reload** (one-shot, ~2–2.5 s)
```
Character reloads a rifle: pulls out the empty magazine downward with
one hand, drops it, inserts a new magazine up into the weapon, pulls
the charging handle back and releases it, quick tactical reload, weapon
held at chest height.
```

**Player — Aim/ADS pose** (loop, ~1.5–2 s)
```
Character raises a rifle to shoulder height and aims down the sights,
steady standing aiming stance, weapon held close to the face, elbows
tucked in, minimal sway.
```

**Player — Jump/Fall** (one-shot, ~0.8–1.2 s, in place — no vertical or
forward root travel; the height is already driven by `vertical_velocity`,
and jump is the clip most likely to get baked-in root motion by default)
```
Character jumps in place from a standing position, arms swinging up
for momentum, then transitions into a mid-air falling pose with knees
slightly bent, landing in a soft recovery crouch. In place, no forward
or vertical root travel — pose only.
```

**Player — Lean left/right** (one-shot, hold last frame, ~0.6–1 s)
```
Character leans sideways from the waist while standing still, upper
body tilts out to peek around a corner, feet planted, weapon still
raised and aimed.
```

**Player — Hit reaction / flinch** (one-shot, ~0.3–0.5 s; needs a code
hook — no damage event exists yet)
```
Character flinches from a bullet impact, upper body jerking back and to
one side, a short stagger step, weapon dipping briefly, then recovers to
the ready stance. Very short one-shot, in place, no root motion.
```

## Suggested generation order

1. **Crouch Idle/Walk, Prone Idle/Crawl** — 4 clips, the gameplay state
   already exists; purely a visual gap.
2. **Reload, Aim/ADS pose** — needed before adding more weapons (SCAR-L is
   in the pipeline).
3. **Jump/Fall, Lean** — lower priority, doesn't look broken.
4. **Hit reaction** — needs a damage-event code hook first (shared with
   the zombie's — see `ZOMBIE_ANIMATION_PLAN.md` Tier 3).

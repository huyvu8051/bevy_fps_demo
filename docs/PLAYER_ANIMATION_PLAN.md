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

## 8-directional strafe locomotion set (rifle stance)

Directional expansion of clips #2 (Walk), #5 (Crouch Walk), and #3 (Run)
above — not new poses, new *travel directions* for those same three. This
is the standard strafe-locomotion shape for a shooter where aim is
decoupled from movement (`shared::movement::simulate_player_step` already
takes `move_axis` independent of facing) — the upper body and rifle stay
aimed forward the whole time in every clip; only the direction the hips/
feet actually travel changes. **Say this explicitly in every prompt** —
Meshy's default read of "walking forward" is a normal walk that turns to
face where it's going, which is wrong here.

Directions are named by travel angle relative to where the character is
aiming (0° = straight ahead), clockwise — not compass directions, and not
which way the character *faces* (that's always 0°/forward in every clip).

19 clips total: 8 standing walk + 8 crouch walk + 3 run (run only covers
forward + the two forward diagonals — full 8-way running isn't requested/
needed here, matching this game's sprint being a forward-biased action).

| # | Clip | Stance | Angle | Duration (loop) |
|---|---|---|---|---|
| 1 | Walk Forward | Standing | 0° | 1–1.2 s |
| 2 | Walk Forward-Right | Standing | 45° | 1–1.2 s |
| 3 | Walk Right (strafe) | Standing | 90° | 1–1.2 s |
| 4 | Walk Backward-Right | Standing | 135° | 1–1.2 s |
| 5 | Walk Backward | Standing | 180° | 1–1.2 s |
| 6 | Walk Backward-Left | Standing | 225° | 1–1.2 s |
| 7 | Walk Left (strafe) | Standing | 270° | 1–1.2 s |
| 8 | Walk Forward-Left | Standing | 315° | 1–1.2 s |
| 9 | Crouch Walk Forward | Crouching | 0° | 1–1.2 s |
| 10 | Crouch Walk Forward-Right | Crouching | 45° | 1–1.2 s |
| 11 | Crouch Walk Right (strafe) | Crouching | 90° | 1–1.2 s |
| 12 | Crouch Walk Backward-Right | Crouching | 135° | 1–1.2 s |
| 13 | Crouch Walk Backward | Crouching | 180° | 1–1.2 s |
| 14 | Crouch Walk Backward-Left | Crouching | 225° | 1–1.2 s |
| 15 | Crouch Walk Left (strafe) | Crouching | 270° | 1–1.2 s |
| 16 | Crouch Walk Forward-Left | Crouching | 315° | 1–1.2 s |
| 17 | Run Forward | Standing | 0° | 0.8–1 s |
| 18 | Run Forward-Right (diagonal) | Standing | 45° | 0.8–1 s |
| 19 | Run Forward-Left (diagonal) | Standing | 315° | 0.8–1 s |

### Text-to-Motion prompts (copy-paste, ≤400 chars each)

All 19 are loops, all **in place / no root motion**, all rifle held
**right-handed** (right hand on trigger/grip, left hand on handguard),
torso/rifle always aimed forward regardless of travel direction. Kept
under Meshy's 400-character Text-to-Motion prompt limit.

**1. Walk Forward** (standing, 0°, loop, ~1–1.2 s, in place — no root motion)
```
Character strafe-walks, moving straight forward. Torso and rifle stay aimed straight forward the whole time, upper body does not turn toward travel direction. Rifle held right-handed at chest (right hand on trigger/grip, left hand on handguard). In place, no root motion, only limbs cycle.
```

**2. Walk Forward-Right** (standing, 45°, loop, ~1–1.2 s, in place — no root motion)
```
Character strafe-walks, moving diagonally forward-right. Torso and rifle stay aimed straight forward the whole time, upper body does not turn toward travel direction. Rifle held right-handed at chest (right hand on trigger/grip, left hand on handguard). In place, no root motion, only limbs cycle.
```

**3. Walk Right (strafe)** (standing, 90°, loop, ~1–1.2 s, in place — no root motion)
```
Character strafe-walks, moving directly right (sideways strafe). Torso and rifle stay aimed straight forward the whole time, upper body does not turn toward travel direction. Rifle held right-handed at chest (right hand on trigger/grip, left hand on handguard). In place, no root motion, only limbs cycle.
```

**4. Walk Backward-Right** (standing, 135°, loop, ~1–1.2 s, in place — no root motion)
```
Character strafe-walks, moving diagonally backward-right. Torso and rifle stay aimed straight forward the whole time, upper body does not turn toward travel direction. Rifle held right-handed at chest (right hand on trigger/grip, left hand on handguard). In place, no root motion, only limbs cycle.
```

**5. Walk Backward** (standing, 180°, loop, ~1–1.2 s, in place — no root motion)
```
Character strafe-walks, moving straight backward. Torso and rifle stay aimed straight forward the whole time, upper body does not turn toward travel direction. Rifle held right-handed at chest (right hand on trigger/grip, left hand on handguard). In place, no root motion, only limbs cycle.
```

**6. Walk Backward-Left** (standing, 225°, loop, ~1–1.2 s, in place — no root motion)
```
Character strafe-walks, moving diagonally backward-left. Torso and rifle stay aimed straight forward the whole time, upper body does not turn toward travel direction. Rifle held right-handed at chest (right hand on trigger/grip, left hand on handguard). In place, no root motion, only limbs cycle.
```

**7. Walk Left (strafe)** (standing, 270°, loop, ~1–1.2 s, in place — no root motion)
```
Character strafe-walks, moving directly left (sideways strafe). Torso and rifle stay aimed straight forward the whole time, upper body does not turn toward travel direction. Rifle held right-handed at chest (right hand on trigger/grip, left hand on handguard). In place, no root motion, only limbs cycle.
```

**8. Walk Forward-Left** (standing, 315°, loop, ~1–1.2 s, in place — no root motion)
```
Character strafe-walks, moving diagonally forward-left. Torso and rifle stay aimed straight forward the whole time, upper body does not turn toward travel direction. Rifle held right-handed at chest (right hand on trigger/grip, left hand on handguard). In place, no root motion, only limbs cycle.
```

**9. Crouch Walk Forward** (crouching, 0°, loop, ~1–1.2 s, in place — no root motion)
```
Character crouch-walks low to the ground, knees bent, moving straight forward. Torso and rifle stay aimed straight forward the whole time, upper body does not turn toward travel direction. Rifle held right-handed at chest (right hand on trigger/grip, left hand on handguard). In place, no root motion, only limbs cycle.
```

**10. Crouch Walk Forward-Right** (crouching, 45°, loop, ~1–1.2 s, in place — no root motion)
```
Character crouch-walks low to the ground, knees bent, moving diagonally forward-right. Torso and rifle stay aimed straight forward the whole time, upper body does not turn toward travel direction. Rifle held right-handed at chest (right hand on trigger/grip, left hand on handguard). In place, no root motion, only limbs cycle.
```

**11. Crouch Walk Right (strafe)** (crouching, 90°, loop, ~1–1.2 s, in place — no root motion)
```
Character crouch-walks low to the ground, knees bent, moving directly right (sideways strafe). Torso and rifle stay aimed straight forward the whole time, upper body does not turn toward travel direction. Rifle held right-handed at chest (right hand on trigger/grip, left hand on handguard). In place, no root motion, only limbs cycle.
```

**12. Crouch Walk Backward-Right** (crouching, 135°, loop, ~1–1.2 s, in place — no root motion)
```
Character crouch-walks low to the ground, knees bent, moving diagonally backward-right. Torso and rifle stay aimed straight forward the whole time, upper body does not turn toward travel direction. Rifle held right-handed at chest (right hand on trigger/grip, left hand on handguard). In place, no root motion, only limbs cycle.
```

**13. Crouch Walk Backward** (crouching, 180°, loop, ~1–1.2 s, in place — no root motion)
```
Character crouch-walks low to the ground, knees bent, moving straight backward. Torso and rifle stay aimed straight forward the whole time, upper body does not turn toward travel direction. Rifle held right-handed at chest (right hand on trigger/grip, left hand on handguard). In place, no root motion, only limbs cycle.
```

**14. Crouch Walk Backward-Left** (crouching, 225°, loop, ~1–1.2 s, in place — no root motion)
```
Character crouch-walks low to the ground, knees bent, moving diagonally backward-left. Torso and rifle stay aimed straight forward the whole time, upper body does not turn toward travel direction. Rifle held right-handed at chest (right hand on trigger/grip, left hand on handguard). In place, no root motion, only limbs cycle.
```

**15. Crouch Walk Left (strafe)** (crouching, 270°, loop, ~1–1.2 s, in place — no root motion)
```
Character crouch-walks low to the ground, knees bent, moving directly left (sideways strafe). Torso and rifle stay aimed straight forward the whole time, upper body does not turn toward travel direction. Rifle held right-handed at chest (right hand on trigger/grip, left hand on handguard). In place, no root motion, only limbs cycle.
```

**16. Crouch Walk Forward-Left** (crouching, 315°, loop, ~1–1.2 s, in place — no root motion)
```
Character crouch-walks low to the ground, knees bent, moving diagonally forward-left. Torso and rifle stay aimed straight forward the whole time, upper body does not turn toward travel direction. Rifle held right-handed at chest (right hand on trigger/grip, left hand on handguard). In place, no root motion, only limbs cycle.
```

**17. Run Forward** (standing, 0°, loop, ~0.8–1 s, in place — no root motion)
```
Character sprints, moving straight forward, leaning into the direction of travel from the hips. Torso and rifle stay aimed/oriented forward, not fully turning to face travel direction. Rifle held right-handed at chest/firing-ready height (right hand on trigger/grip, left hand on handguard). In place, no root motion, only limbs cycle.
```

**18. Run Forward-Right (diagonal)** (standing, 45°, loop, ~0.8–1 s, in place — no root motion)
```
Character sprints, moving diagonally forward-right, leaning into the direction of travel from the hips. Torso and rifle stay aimed/oriented forward, not fully turning to face travel direction. Rifle held right-handed at chest/firing-ready height (right hand on trigger/grip, left hand on handguard). In place, no root motion, only limbs cycle.
```

**19. Run Forward-Left (diagonal)** (standing, 315°, loop, ~0.8–1 s, in place — no root motion)
```
Character sprints, moving diagonally forward-left, leaning into the direction of travel from the hips. Torso and rifle stay aimed/oriented forward, not fully turning to face travel direction. Rifle held right-handed at chest/firing-ready height (right hand on trigger/grip, left hand on handguard). In place, no root motion, only limbs cycle.
```

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

## Text-to-Motion prompts (copy-paste, ≤400 chars each)

Every prompt keeps root motion **off** — see
[`CHARACTER_ANIMATION_PLAN.md`](CHARACTER_ANIMATION_PLAN.md) for why. Same
right-handed grip convention as the 8-directional set above (right hand
on trigger/grip, left hand on handguard), and same 400-character Meshy
Text-to-Motion limit.

**Player — Crouch Idle** (loop, ~2–3 s)
```
Character holds a stationary crouching idle pose, weight balanced, knees bent, torso upright, alert ready stance, holding a rifle at chest right-handed (right hand on trigger/grip, left hand on handguard).
```

**Player — Crouch Walk** (loop, ~1–1.2 s, in place — no root motion)
```
Character crouch-walks low to the ground, knees bent, moving straight forward. Torso and rifle stay aimed straight forward the whole time, upper body does not turn toward travel direction. Rifle held right-handed at chest (right hand on trigger/grip, left hand on handguard). In place, no root motion, only limbs cycle.
```

**Player — Prone Idle** (loop, ~2–3 s)
```
Character lies flat prone, elbows propped on the ground, holding a rifle aimed forward right-handed (right hand on trigger/grip, left hand on handguard), stationary prone ready pose.
```

**Player — Prone Crawl** (loop, ~1.5–2 s, in place — no root motion)
```
Character crawls in place lying prone, pulling with elbows and forearms, legs dragging behind, military low crawl, holding a rifle in front right-handed (right hand on trigger/grip, left hand on handguard). In place, no root motion, only arms/legs cycle.
```

**Player — Reload** (one-shot, ~2–2.5 s)
```
Character reloads a rifle held right-handed at chest height (right hand stays on trigger/grip throughout): left hand pulls the empty magazine down, drops it, inserts a new magazine, pulls the charging handle back and releases it. Quick tactical reload, one-shot.
```

**Player — Aim/ADS pose** (loop, ~1.5–2 s)
```
Character raises a rifle to shoulder height and aims down the sights, right-handed - right cheek on the stock, right hand on trigger/grip, left hand on handguard. Steady standing aiming stance, weapon close to the face, elbows tucked in, minimal sway.
```

**Player — Jump/Fall** (one-shot, ~0.8–1.2 s, in place — no vertical or
forward root travel; the height is already driven by `vertical_velocity`,
and jump is the clip most likely to get baked-in root motion by default)
```
Character jumps in place from standing, holding a rifle right-handed (right hand on trigger/grip, left hand on handguard), arms swinging up without releasing the weapon, then a mid-air falling pose, knees bent, landing in a soft recovery crouch. In place, no root travel, pose only.
```

**Player — Lean left/right** (one-shot, hold last frame, ~0.6–1 s)
```
Character leans sideways from the waist while standing still, upper body tilts out to peek around a corner, feet planted, rifle still raised and aimed right-handed (right hand on trigger/grip, left hand on handguard).
```

**Player — Hit reaction / flinch** (one-shot, ~0.3–0.5 s; needs a code
hook — no damage event exists yet)
```
Character flinches from a bullet impact, upper body jerks back and to one side, a short stagger step, rifle (held right-handed, right hand on trigger/grip, left hand on handguard) dips briefly, then recovers to ready stance. Very short one-shot, in place, no root motion.
```

## Suggested generation order

1. **Crouch Idle/Walk, Prone Idle/Crawl** — 4 clips, the gameplay state
   already exists; purely a visual gap.
2. **Reload, Aim/ADS pose** — needed before adding more weapons (SCAR-L is
   in the pipeline).
3. **Jump/Fall, Lean** — lower priority, doesn't look broken.
4. **Hit reaction** — needs a damage-event code hook first (shared with
   the zombie's — see `ZOMBIE_ANIMATION_PLAN.md` Tier 3).

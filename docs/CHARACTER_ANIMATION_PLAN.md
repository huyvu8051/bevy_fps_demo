# Character animation plan

What animation clips the player (soldier) and zombie models actually need,
grounded in the game's real state machine (`PlayerStance`, `ClientInput`,
`ZombiePose`) — not a wishlist. Companion to
[`CHARACTER_IMPORT_CHECKLIST.md`](CHARACTER_IMPORT_CHECKLIST.md) (that one's
the *mechanics* of wiring a new mesh in — clip index, bone names, scale;
this one's *which clips to make in the first place*).

## 1. Player (soldier) — 13 clips, 5 done, 8 missing

Currently wired (`PLAYER_MODEL_ANIM_*` in `crates/client/src/lib.rs`,
`PlayerModelPose` in `crates/client/src/zombie_client.rs`): Idle, Walk, Run,
Shoot, Death.

| # | Clip | Status | Why it's real (not guessed) |
|---|---|---|---|
| 1 | Idle (standing) | ✅ done | — |
| 2 | Walk (standing) | ✅ done | — |
| 3 | Run/Sprint (standing) | ✅ done | — |
| 4 | Crouch Idle | ❌ missing | `PlayerStance::Crouching` exists and changes eye height/capsule length (`CROUCH_LENGTH`, `movement.rs`) — model still shows standing pose |
| 5 | Crouch Walk | ❌ missing | `CROUCH_SPEED_MULTIPLIER` is a real, distinct movement speed — crouch-moving is a real state, not just a static pose |
| 6 | Prone Idle | ❌ missing | `PlayerStance::Prone` exists, own `PRONE_LENGTH` |
| 7 | Prone Crawl | ❌ missing | `PRONE_SPEED_MULTIPLIER` — moving while prone is a real mechanic, confirmed in `simulate_player_step` |
| 8 | Shoot | ✅ done | one-shot, triggered on fire |
| 9 | Reload | ❌ missing | reload only animates the local 1st-person view-model gun (procedural dip/tilt in `vfx.rs`) — other players see no reload motion on the 3rd-person body at all |
| 10 | Aim/ADS pose | ❌ missing | `AIM_MOVE_SPEED_MULTIPLIER` is real and distinct from normal walk speed, but the 3rd-person body never changes pose when aiming — only the local view-model/scope UI react |
| 11 | Jump/Fall | ❌ missing (optional) | jump changes `vertical_velocity` only, no pose change — lower priority, doesn't look broken, just stiff |
| 12 | Death | ✅ done | one-shot, triggered on `PlayerDiedFx` |
| 13 | Hit reaction (flinch) | ❌ missing, **needs a code hook first** | damage is currently pure HP subtraction (`health.0 -= damage`) with no event a client could react to — adding this clip alone does nothing until a trigger exists |

Crouch-run and prone-run aren't in this list on purpose: `send_player_input`
only allows sprint while `stance == Standing`, so those states can't happen.

## 2. Zombie — 5 clips, all done; 1 deliberate gap

Currently wired (`ZOMBIE_ANIM_*` in `crates/shared/src/zombie.rs`,
`ZombiePose` dispatch in `crates/client/src/zombie_client.rs`): Idle, Walk,
Run, Crawl, Bite.

**No Death clip** — a killed zombie despawns immediately server-side
(`crates/server/src/main.rs`), only blood/spark VFX plays. This is a real
design choice, not an oversight: adding a death animation means the
despawn has to wait for the clip to finish, which touches respawn timing
(`ZOMBIE_RESPAWN_DELAY`) and the hit-registration cleanup path. Decide
explicitly before generating one — don't add the clip and assume wiring it
in is free.

## 3. Reference: what PUBG / Left 4 Dead actually ship

Not a checklist to copy wholesale — scope-matching notes for what's
actually relevant here.

**PUBG (player, 3rd-person)**: full Idle/Walk/Run × Standing/Crouch/Prone,
Jump, vault/climb, swim, **lean left/right**, **per-weapon** Aim + Reload
poses (rifle/pistol/sniper/shotgun all held differently), weapon
switch/holster, grenade throw, melee, item-use (bandage/medkit — long,
stationary), vehicle enter/exit/drive, parachute, **Knocked/Downed**
(crawling, revivable), physics ragdoll death.

**Left 4 Dead — Survivor**: Idle/Walk/Run only (no crouch/prone — simpler
than PUBG), per-weapon Shoot + Reload, melee push/shove, throw
(Molotov/pipe bomb), **Incapacitated** (kneeling, one-handed pistol,
revivable by teammate), give/receive item (2-player synced animation),
and per-special-infected **capture reactions** (dragged by Smoker's tongue,
pinned by Hunter, slammed by Charger, ridden by Jockey, blinded/stumbling
from Boomer vomit).

**Left 4 Dead — Common Infected** (closest reference to this game's
zombies): Idle, Walk, Run/Sprint, Claw attack, Death (ragdoll), climb over
obstacles, scream/alert on spotting a player. Notably **no crawl pose** —
opposite tradeoff from this game (which has Crawl but no Death).

**Worth adopting, scope-appropriate:**
- Lean left/right — cheap, slots into existing cover-shooting feel
- Per-weapon Reload — worth planning now since more guns (SCAR-L) are
  already in the pipeline, cheaper to design for than retrofit later

**Not worth it right now:** vehicle animations, parachute, per-item use
poses, Incapacitated/revive (real multiplayer co-op feature, bigger than
an animation task) — out of current scope.

## 4. Generating clips with meshy.ai

Existing rigged asset to reuse: `crates/client/assets/characters/soldier/mesh/soldier.glb`
(already has the 5 shipped clips baked in, referenced by index — see
`CHARACTER_IMPORT_CHECKLIST.md` §2 for why index order matters).

Workflow (Animate tab, confirmed via hands-on use earlier this project):
1. Upload the existing rigged `soldier.glb` (don't regenerate the mesh —
   only need new motion clips on the same skeleton).
2. **Library** — try first for generic clips: Idle/Walk/Run variants,
   Crouch, Prone, Jump. Free-est option if the stock motion already fits.
3. **AI-Motions** — shape-aware suggestions Meshy proposes for this rig;
   worth checking before hand-writing a Text-to-Motion prompt.
4. **Text-to-Motion** — needed for game-specific motions the library won't
   have verbatim: Reload (this game's specific timing/pose), Aim/ADS pose,
   Lean.
5. **Video-to-Motion** — fallback if Text-to-Motion output doesn't read
   right; feed a short reference clip instead.

After generating: re-check clip **index order** in the downloaded file
(Blender or a gltf viewer) and update `PLAYER_MODEL_ANIM_*`
(`crates/client/src/lib.rs`) to match — the game reads clips by index, not
name (see `CHARACTER_IMPORT_CHECKLIST.md` §2).

## 5. Text-to-Motion prompts (copy-paste ready)

One prompt per missing clip (§1/§2) — paste directly into the Animate
tab's Text-to-Motion box once a rigged model (soldier or zombie) is
selected as the target. Check **Library** first (§4 step 2) for each one
before spending a Text-to-Motion generation — a close-enough stock motion
is free, these prompts are the fallback for the ones the library doesn't
have verbatim (mainly Reload, Aim, and anything weapon-specific).

**Player — Crouch Idle**
```
Character crouches down and holds a stationary crouching idle pose,
weight balanced on both feet, knees bent, torso upright, alert ready
stance, holding a rifle at the chest.
```

**Player — Crouch Walk**
```
Character walks forward while crouching low, knees bent, staying low
to the ground, cautious tactical crouch-walk cycle, holding a rifle
raised at the chest.
```

**Player — Prone Idle**
```
Character lies flat on the stomach in a prone position, elbows propped
forward on the ground, holding a rifle aimed forward, stationary prone
ready pose.
```

**Player — Prone Crawl**
```
Character crawls forward slowly while lying prone on the stomach,
pulling with elbows and forearms, legs dragging behind, military low
crawl, holding a rifle in front.
```

**Player — Reload**
```
Character reloads a rifle: pulls out the empty magazine downward with
one hand, drops it, inserts a new magazine up into the weapon, pulls
the charging handle back and releases it, quick tactical reload, weapon
held at chest height.
```

**Player — Aim/ADS pose**
```
Character raises a rifle to shoulder height and aims down the sights,
steady standing aiming stance, weapon held close to the face, elbows
tucked in, minimal sway.
```

**Player — Jump/Fall**
```
Character jumps upward from a standing position, arms swinging up for
momentum, then transitions into a mid-air falling pose with knees
slightly bent, landing in a soft recovery crouch.
```

**Player — Lean left/right** (bonus, from the PUBG/L4D notes in §3)
```
Character leans sideways from the waist while standing still, upper
body tilts out to peek around a corner, feet planted, weapon still
raised and aimed.
```

**Zombie — Death** (only if the design decision in §2 is "yes, add it")
```
Zombie is shot and collapses backward, losing balance and falling
limply to the ground, ragdoll-like uncontrolled death fall, no
graceful landing.
```

## 6. Suggested order

1. Zombie Death — single decision (do we want it) + single clip, small
   blast radius once decided.
2. Player Crouch Idle/Walk, Prone Idle/Crawl — 4 clips, state already
   exists in gameplay code, purely a visual gap to close.
3. Player Reload, Aim/ADS pose — needed before adding more weapons
   (SCAR-L already in the pipeline).
4. Jump/Fall, Lean, Hit reaction — lower priority / needs a code hook
   first (Hit reaction specifically).

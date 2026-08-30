# Character animation plan — shared guidance + index

Which clips each character needs is split by character:

- **[`PLAYER_ANIMATION_PLAN.md`](PLAYER_ANIMATION_PLAN.md)** — the soldier
  (3rd-person model): 13 clips, 5 done.
- **[`ZOMBIE_ANIMATION_PLAN.md`](ZOMBIE_ANIMATION_PLAN.md)** — the zombie
  (Classic + Meshy rigs): core 5 + fall / reactions / attack variety.

This file holds only the guidance that applies to **both**. The
*mechanics* of wiring a finished `.glb` in (clip index order, bone names,
scale) live in
[`CHARACTER_IMPORT_CHECKLIST.md`](CHARACTER_IMPORT_CHECKLIST.md).

## Root motion: must be OFF (in place only)

Character position is driven entirely by server-side movement code
(velocity → `Transform` in `movement_combat.rs` / `move_zombies`). The
skinning path only computes joint matrices to pose the mesh — it never
reads the root bone's translation back onto the entity.

If a generated clip has baked-in root motion (the hips/pelvis actually
translating through world space — e.g. a walk cycle that "travels"), the
mesh double-moves: code moves the entity via velocity *and* the animation
drags the root bone forward inside it, causing foot sliding or the visible
mesh drifting off its collider. This bites hardest on the clips that
involve real movement (Walk, Run, Crouch Walk, Prone Crawl, Zombie Crawl,
Jump/Fall) — exactly the ones a generator defaults to giving root motion.

- When exporting from Meshy, use the **in-place** / **no root motion**
  option if offered.
- In every Text-to-Motion prompt, add "in place, no root motion"
  explicitly so the hips stay centered over one spot and only the limbs
  cycle.

## Duration is a generation setting, not a code constraint

The game plays whichever clip is at a given index, looping or one-shot per
the pose dispatch (`PlayerModelPose` / `ZombiePose`). Length is purely
Meshy's duration slider. Rules of thumb:

- **Idle / held poses** — 2–3 s, so a full breathing cycle passes before
  the loop seam repeats.
- **Walk / step cycles** — ~1–1.2 s for one L-R-L cycle; match the
  existing Walk clip so playback-speed scaling stays consistent.
- **Run cycles** — ~0.8–1 s.
- **Crawl cycles** — ~1.5–2 s (reads slower than a walk; too short looks
  jittery).
- **One-shot actions** (Reload, Death, Land) — long enough to cover the
  whole described motion, no longer.
- **Flinch / quick reactions** — 0.3–0.5 s.

Per-clip suggestions are in each character's own doc.

## Generating clips with meshy.ai

Reuse the existing rigged asset — don't regenerate the mesh, only add
motion clips on the same skeleton:

- Player: `crates/client/assets/characters/soldier/mesh/soldier.glb`
- Zombie: `crates/client/assets/characters/zombie/mesh/zombie.glb`
  (Classic) or `.../zombie_meshy/mesh/zombie_meshy.glb` (Meshy)

Workflow (Animate tab):

1. Upload the rigged `.glb`.
2. **Library** — try first for generic clips (Idle/Walk/Run variants,
   Crouch, Prone, Jump). Free-est option if the stock motion fits.
3. **AI-Motions** — shape-aware suggestions Meshy proposes for this rig;
   check before hand-writing a prompt.
4. **Text-to-Motion** — for game-specific motions the library won't have
   verbatim (Reload timing, ADS pose, Lean, specific zombie attacks). The
   per-character docs have copy-paste prompts.
5. **Video-to-Motion** — fallback if Text-to-Motion output doesn't read
   right; feed a short reference clip.

After generating: re-check clip **index order** in the downloaded file
(Blender or a glTF viewer) and update the matching constants —
`PLAYER_MODEL_ANIM_*` (`crates/client/src/lib.rs`) for the soldier, or
`zombie_kind_pose_clips` (`crates/shared/src/zombie.rs`) plus the
`ZOMBIE_ANIM_*` constants for the zombie. The game reads clips by index,
not name.

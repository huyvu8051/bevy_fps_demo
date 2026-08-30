// --- Zombies (server-authoritative AI/movement, see crates/server) ---

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::arena::ARENA_HALF_SIZE;
use crate::network::HITBOX_TEST_ROW_X;

// Marks the root of a zombie's spawned scene, so a raycast hit anywhere in
// its (deep, multi-node) hierarchy can be traced back up to something we can
// despawn, and so move_zombies can find every zombie each frame. Replicated
// (server spawns/AI-drives these; client reacts to Add<Zombie> to spawn the
// cosmetic GLTF scene — see crates/client's spawn_zombie).
#[derive(Component, Default, Serialize, Deserialize)]
#[require(Transform)]
pub struct Zombie;

// Gravity/ground state, same idea as Player but tracked per-zombie.
// path_recalc_timer/cached_path_dir throttle move_zombies' navmesh
// pathfinding AND local obstacle-avoidance raycasts instead of recomputing
// both every single frame for every chasing zombie — cached_path_dir holds
// the final, already-steered movement direction.
#[derive(Component, Default)]
pub struct ZombieMotion {
    pub vertical_velocity: f32,
    pub grounded: bool,
    pub path_recalc_timer: f32,
    pub cached_path_dir: Vec3,
    // Set by move_zombies each frame (distance > ZOMBIE_LOD_DISTANCE) —
    // freeze_zombie_animations pauses animation/skinning for these, and
    // update_zombie_health_bars hides their bar, since neither is worth
    // paying for on a zombie too far away to be scrutinized.
    pub far: bool,
}

// Horizontal impulse from a bullet hit, added on top of chase movement in
// move_zombies and exponentially decayed back to zero — a hit knocks a
// zombie back for a moment instead of its position being unaffected by
// anything but its own chase logic.
#[derive(Component, Default)]
pub struct KnockbackVelocity(pub Vec3);

// Bone entities used for real per-bone hit detection (see HITBOX_PLAN.md
// Phase 4 — this replaces zombie_hit_damage_geometric's fixed-Y-threshold
// guess once wired into damage). zombie.glb's rig names its nodes after
// standard humanoid bones; wire_zombie_hitbox_bones (below) matches
// against those names as they show up while the glTF scene loads.
// Fields are individually optional/incrementally filled rather than an
// all-or-nothing bundle, so a hit lands correctly even mid-load using
// whichever bones have resolved so far. Moved here from crates/client
// (Phase 2) so the server's own zombie skeletons (Phase 2's real point)
// can populate the exact same component the client already did —
// one system, two callers, instead of two copies drifting apart.
//
// left_arm/right_arm/left_leg/right_leg are ordered joint chains
// ([shoulder/hip, elbow/knee, hand/foot]) rather than an unordered bag —
// zombie_hit_damage_bones tests each CONSECUTIVE pair as a capsule segment
// (shoulder->elbow, elbow->hand) instead of a lone point-sphere at each
// joint. A point-sphere-per-joint approach left real "dead zones" along
// the visible forearm/shin — a clean hit partway between two joints landed
// outside every sphere and scored 0 damage even though it clearly hit the
// mesh. Capsule segments have no such gap. Same reasoning applies to
// belly/chest below (see zombie_hit_damage_bones).
#[derive(Component, Default)]
pub struct ZombieHitboxBones {
    pub head: Option<Entity>,
    pub belly: Option<Entity>,
    pub chest: Option<Entity>,
    pub left_arm: [Option<Entity>; 3],
    pub right_arm: [Option<Entity>; 3],
    pub left_leg: [Option<Entity>; 3],
    pub right_leg: [Option<Entity>; 3],
}

// Matches newly-spawned bone entities (glTF nodes get a Name component
// from their node name) against zombie.glb's known rig names, and
// records whichever ones matter on the zombie root's ZombieHitboxBones.
// Registered in both crates/client (cosmetic gizmo today) and
// crates/server (Phase 2 onward — real hit detection) — same system,
// same component, so there's exactly one place this bone-name mapping
// can drift out of sync with the actual rig.
// Each entry is (rig node name, setter) — the setter both picks which field
// this bone belongs on and, for the limb chains, which position in the
// [shoulder/hip, elbow/knee, hand/foot] ordering, so zombie_hit_damage_bones
// can always build its capsule segments in the right order regardless of
// what order glTF nodes happen to load in.
type HitboxSetter = fn(&mut ZombieHitboxBones, Entity);

const CLASSIC_HITBOX_SLOTS: &[(&str, HitboxSetter)] = &[
    ("Head", |b, e| b.head = Some(e)),
    ("Spine", |b, e| b.belly = Some(e)),
    ("Spine2", |b, e| b.chest = Some(e)),
    ("LeftArm", |b, e| b.left_arm[0] = Some(e)),
    ("LeftForeArm", |b, e| b.left_arm[1] = Some(e)),
    ("LeftHand", |b, e| b.left_arm[2] = Some(e)),
    ("RightArm", |b, e| b.right_arm[0] = Some(e)),
    ("RightForeArm", |b, e| b.right_arm[1] = Some(e)),
    ("RightHand", |b, e| b.right_arm[2] = Some(e)),
    ("LeftUpLeg", |b, e| b.left_leg[0] = Some(e)),
    ("LeftLeg", |b, e| b.left_leg[1] = Some(e)),
    ("LeftFoot", |b, e| b.left_leg[2] = Some(e)),
    ("RightUpLeg", |b, e| b.right_leg[0] = Some(e)),
    ("RightLeg", |b, e| b.right_leg[1] = Some(e)),
    ("RightFoot", |b, e| b.right_leg[2] = Some(e)),
];

// zombie_meshy's rig is Mixamo-named too — identical to CLASSIC except the
// chest bone is "Spine02" (it also carries Spine01 / LeftShoulder /
// RightShoulder that the classic rig lacks; the game's arm chains still
// start at LeftArm/RightArm, so those aren't mapped).
const MESHY_HITBOX_SLOTS: &[(&str, HitboxSetter)] = &[
    ("Head", |b, e| b.head = Some(e)),
    ("Spine", |b, e| b.belly = Some(e)),
    ("Spine02", |b, e| b.chest = Some(e)),
    ("LeftArm", |b, e| b.left_arm[0] = Some(e)),
    ("LeftForeArm", |b, e| b.left_arm[1] = Some(e)),
    ("LeftHand", |b, e| b.left_arm[2] = Some(e)),
    ("RightArm", |b, e| b.right_arm[0] = Some(e)),
    ("RightForeArm", |b, e| b.right_arm[1] = Some(e)),
    ("RightHand", |b, e| b.right_arm[2] = Some(e)),
    ("LeftUpLeg", |b, e| b.left_leg[0] = Some(e)),
    ("LeftLeg", |b, e| b.left_leg[1] = Some(e)),
    ("LeftFoot", |b, e| b.left_leg[2] = Some(e)),
    ("RightUpLeg", |b, e| b.right_leg[0] = Some(e)),
    ("RightLeg", |b, e| b.right_leg[1] = Some(e)),
    ("RightFoot", |b, e| b.right_leg[2] = Some(e)),
];

pub fn zombie_hitbox_bone_slots(kind: ZombieKind) -> &'static [(&'static str, HitboxSetter)] {
    match kind {
        ZombieKind::Classic => CLASSIC_HITBOX_SLOTS,
        ZombieKind::Meshy => MESHY_HITBOX_SLOTS,
    }
}

pub fn wire_zombie_hitbox_bones(
    child_of: Query<&ChildOf>,
    mut zombies: Query<(&mut ZombieHitboxBones, Option<&ZombieKind>), With<Zombie>>,
    new_names: Query<(Entity, &Name), Added<Name>>,
) {
    for (bone_entity, name) in &new_names {
        let n = name.as_str();
        // Cheap reject before the ChildOf walk: not a bone name in either rig.
        if n != "Spine02" && !CLASSIC_HITBOX_SLOTS.iter().any(|&(s, _)| s == n) {
            continue;
        }

        let mut current = bone_entity;
        for _ in 0..20 {
            if let Ok((mut bones, kind)) = zombies.get_mut(current) {
                let slots = zombie_hitbox_bone_slots(kind.copied().unwrap_or_default());
                if let Some(&(_, set)) = slots.iter().find(|&&(s, _)| s == n) {
                    set(&mut bones, bone_entity);
                }
                break;
            }
            let Ok(parent) = child_of.get(current) else {
                break;
            };
            current = parent.0;
        }
    }
}

// How often a zombie in melee range can bite — see ZombieAttackTimer.
pub const ZOMBIE_ATTACK_INTERVAL: f32 = 1.0;

// Counts down to the next bite — only ticks while the zombie is actually in
// range (zombies_damage_player), so wandering out of melee range and back
// doesn't let an attack "bank" while it was away. Starts at
// ZOMBIE_ATTACK_INTERVAL rather than 0.0 so a zombie that just caught up
// gets a beat before its first bite instead of landing one the instant it
// arrives.
#[derive(Component)]
pub struct ZombieAttackTimer(pub f32);

impl Default for ZombieAttackTimer {
    fn default() -> Self {
        Self(ZOMBIE_ATTACK_INTERVAL)
    }
}

// Scenario::HitboxTest only — tags a zombie with its own fixed row
// position (server-only, never replicated) so a kill respawns a fresh
// zombie back at the same slot instead of the normal cycling-
// ZOMBIE_SPAWNS-grid behavior. See PendingRespawn's own doc comment
// (crates/server/src/main.rs) for how this gets read at death time.
#[derive(Component, Clone, Copy)]
pub struct HitboxTestSlot(pub Vec3);

// Driven by big-brain's Thinker/Scorer/Action (NearPlayer scorer picks
// ChaseAction once in range — both server-only, see crates/server).
// ChaseAction's Requested/Cancelled handlers insert/remove this component —
// move_zombies reads its presence to know which zombies should be walking
// at all. No `target` field: with more than one player connected, the
// nearest one can change moment to moment, so move_zombies just re-picks
// the nearest connected player fresh every frame instead of chasing
// whichever one happened to be nearest when this was first inserted.
#[derive(Component, Clone)]
#[component(storage = "SparseSet")]
pub struct Chase;

pub const ZOMBIE_AI_ENABLED: bool = true;
// Large enough that zombies start chasing basically as soon as they spawn —
// this is meant as target practice, not a stealth encounter.
pub const ZOMBIE_CHASE_RANGE: f32 = 60.0;
// Was a single ZOMBIE_SPEED (1.0) regardless of pose — split so the Run
// pose (see ZOMBIE_RUN_DISTANCE/zombie_pose_for) actually moves faster
// than Walk instead of just looking like it does. 1.5x/3x the old value.
pub const ZOMBIE_WALK_SPEED: f32 = 1.5;
pub const ZOMBIE_RUN_SPEED: f32 = 3.0;

// Single formula for "how fast is a zombie moving at this distance",
// mirroring zombie_pose_for's own ZOMBIE_RUN_DISTANCE threshold so the
// Run pose and the actual speed boost always kick in at the same range —
// see move_zombies (crates/server) for the caller.
pub fn zombie_speed_for(distance: f32) -> f32 {
    if distance <= ZOMBIE_RUN_DISTANCE {
        ZOMBIE_RUN_SPEED
    } else {
        ZOMBIE_WALK_SPEED
    }
}

// Visual mesh scale multiplier for zombie.glb (crates/client's ZOMBIE_PATH)
// — lives here rather than client-only because the physics/hit-detection
// capsule below needs to stay sized to match it. Keeping them in the same
// crate is what stops them silently drifting apart: previously ZOMBIE_SCALE
// was bumped 0.1 -> 0.25 directly in the client without resizing the
// capsule to match, so the visual head grew well above where the actual
// collider ended and headshots stopped registering as hits at all (the
// capsule/hitbox constants below are now all *derived* from this value
// instead of separately hardcoded, specifically so that can't happen again).
pub const ZOMBIE_SCALE: f32 = 0.25;
// Raw mesh height in zombie.glb's own unscaled local units, actually
// measured from the file (not guessed): its POSITION accessor's Y
// bounding box is [-0.0000837, 0.0760498] (height 0.0761335), but the
// glTF scene's "Zombie" node — the one crates/client's SceneRoot load
// actually renders through — bakes in its own 107.006576 scale on top of
// that raw vertex data. 0.0761335 * 107.006576 = 8.1468, i.e. this
// constant already includes that node scale, so ZOMBIE_WORLD_HEIGHT below
// only needs to multiply by ZOMBIE_SCALE, not by both. (Two earlier
// values here were both wrong: 20.0 was a guess never checked against the
// file, and 1.0 made ZOMBIE_LENGTH go negative — see ZOMBIE_LENGTH's own
// comment on why that's a real bug, not just a bad guess.)
const ZOMBIE_MESH_REFERENCE_HEIGHT: f32 = 8.1468;
const ZOMBIE_WORLD_HEIGHT: f32 = ZOMBIE_MESH_REFERENCE_HEIGHT * ZOMBIE_SCALE;
// Relative to the original 0.1 calibration (factor 1.0 at ZOMBIE_SCALE =
// 0.1) — KNOCKBACK_FORCE and the old debug-gizmo-only HITBOX_*_MIN_Y ring
// constants (crate::hitbox) still scale off this so they stay proportional
// to whatever ZOMBIE_SCALE currently is, instead of separately hardcoded
// numbers. ZOMBIE_RADIUS itself no longer uses this — see its own doc
// comment for why. pub(crate): crate::hitbox needs it too.
pub(crate) const ZOMBIE_SCALE_FACTOR: f32 = ZOMBIE_SCALE / 0.1;
// Raw mesh X-axis extent (torso depth) in zombie.glb's own unscaled local
// units, measured the same way as ZOMBIE_MESH_REFERENCE_HEIGHT above: its
// POSITION accessor's X bounding box is [-0.00656874990090728,
// 0.0093571599572897] (extent 0.0159259099), times the same 107.006576
// node scale baked into that accessor. Deliberately the X axis, not Z
// (extent ~7.65 at this same scale) — Z is the mesh's T-pose left/right ARM
// SPAN, which massively overstates how wide the character actually reads
// standing normally; X is front-to-back torso depth, unaffected by the
// arms being spread out in the bind pose. See ZOMBIE_RADIUS below.
const ZOMBIE_MESH_REFERENCE_TORSO_WIDTH: f32 = 1.7042;
// Used to be a guessed multiple of the old PLAYER_RADIUS-aliased value
// (0.35 * ZOMBIE_SCALE_FACTOR = 0.875 at the current ZOMBIE_SCALE) — that
// number turned out to be close to the mesh's full T-pose arm span, not its
// actual torso width, so the raycast capsule ended up roughly 2x wider than
// the visible character: shots that clearly missed the model still
// registered as a capsule hit (see spawn_zombie_hit_feedback's own doc
// comment in crates/client for the other half of that bug — those phantom
// hits landing outside every real bone hitbox still showed blood/floating
// damage before that fix). Now derived from the mesh's actual measured
// torso width instead of a guess, same treatment ZOMBIE_MESH_REFERENCE_HEIGHT
// already got. Halved for radius since TORSO_WIDTH above is a full extent.
// This stays the ACCURATE value — HEAD_HITBOX_RADIUS/TORSO_HITBOX_RADIUS/
// LIMB_HITBOX_RADIUS (the real per-bone damage regions, crate::hitbox) all
// derive from this, so damage decisions keep matching the real mesh. See
// ZOMBIE_OUTER_RADIUS below for the (deliberately wider) broad-phase
// capsule instead.
pub const ZOMBIE_RADIUS: f32 = ZOMBIE_MESH_REFERENCE_TORSO_WIDTH * ZOMBIE_SCALE / 2.0;
// Capsule height (2*radius + length) is set to exactly ZOMBIE_WORLD_HEIGHT
// so a raycast/capsule-cast covers the mesh's full height, including the
// very top of the head — no gap left uncovered like before. ZOMBIE_RADIUS
// is derived from a different axis of the same mesh (torso width vs.
// height) rather than from ZOMBIE_WORLD_HEIGHT itself, so this can in
// principle still go negative if ZOMBIE_MESH_REFERENCE_HEIGHT is ever set
// too small relative to 2*ZOMBIE_RADIUS — avian3d's capsule
// collider doesn't panic on that (SharedShape::capsule just takes two
// endpoints, so a negative length silently flips them and produces a
// capsule sized off |length| instead — see the client's draw_zombie_hitboxes
// for why that made the debug capsule gizmo visibly lie about the real
// hitbox size). A negative value here is always a bug, not a valid "smaller
// zombie" — shrink ZOMBIE_SCALE instead, which scales both terms together.
pub const ZOMBIE_LENGTH: f32 = ZOMBIE_WORLD_HEIGHT - 2.0 * ZOMBIE_RADIUS;

// The broad-phase shape ("did this shot even reach the zombie AT ALL" —
// handle_fire_shot's first pass, debug_bot_fire's own fire branch,
// shoot()'s local prediction, and the zombie's own attached
// physics Collider used for generic SpatialQuery casts like handle_fire_
// shot's live_hit fallback) — deliberately WIDER than ZOMBIE_RADIUS,
// requested to make that first pass more forgiving. Was a capsule (rounded
// hemispherical end caps); now a plain cylinder (Collider::cylinder/
// Cylinder gizmo — flat top/bottom, no caps), per request. A cylinder's
// own "height" parameter is the full straight height with no cap
// contribution to subtract (unlike a capsule, where total height =
// length + 2*radius) — so ZOMBIE_OUTER_HEIGHT is just ZOMBIE_WORLD_HEIGHT
// directly, keeping the same "still covers the mesh's real height"
// invariant ZOMBIE_LENGTH/ZOMBIE_OUTER_LENGTH (its capsule-era equivalent)
// kept. Was `ZOMBIE_RADIUS * 4.0` (~0.85, itself a 2x widen doubled);
// pinned to a flat 0.7 so the zombie's move footprint (this is also the
// MoveAndSlide cylinder in move_zombies) is small enough to fit gaps a
// player clears. Does NOT affect the real per-bone damage regions
// (HEAD/TORSO/LIMB_HITBOX_RADIUS still derive from the accurate
// ZOMBIE_RADIUS above) — zombie_hit_damage_ray still decides how much (if
// any) damage actually lands from where the ray actually goes; a shot that
// only reaches this outer shell and no real bone region is a genuine miss
// (see zombie_hit_damage_ray's own doc comment), not a graze.
pub const ZOMBIE_OUTER_RADIUS: f32 = 0.7;
pub const ZOMBIE_OUTER_HEIGHT: f32 = ZOMBIE_WORLD_HEIGHT;

// How far below the zombie root's own Transform/NetTransform.translation
// (capsule center) the mesh's own local origin needs to sit so its feet
// land at the capsule's bottom — capsule center to feet. Applied to the
// mesh/skeleton CHILD's own Transform (in world units — the child's
// parent, the zombie entity itself, is never scaled, only this child is),
// alongside a ZOMBIE_SCALE scale-down, not baked into the scaled
// mesh-local space. Both crates/client (the visible mesh, zombie_mesh_
// bundle) and crates/server (the real skeleton used for hit detection,
// on_zombie_spawned) need this exact same transform — a skeleton spawned
// without it sits at the raw unscaled mesh size (~4x too big at the
// current ZOMBIE_SCALE) and offset from the root, so its bones end up
// nowhere near the outer capsule/visible mesh a shot is actually aimed at.
pub const ZOMBIE_MODEL_Y_OFFSET: f32 = -(ZOMBIE_RADIUS + ZOMBIE_LENGTH / 2.0);

// Was two separately-declared consts (ZOMBIE_PATH in crates/client,
// SERVER_ZOMBIE_PATH in crates/server) both pointing at the same file —
// same "shouldn't silently drift" reasoning as ZOMBIE_MODEL_Y_OFFSET
// above. "Animated Zombie" by Quaternius (poly.pizza, CC-BY 3.0) —
// https://poly.pizza/m/jkrEvQZb8J — keep this attribution if shared/
// published.
pub const ZOMBIE_PATH: &str = "characters/zombie/mesh/zombie.glb";
// zombie.glb's animation order (read from the file's own animations list):
// 0=Bite, 1=Crawl, 2=Idle, 3=Run, 4=Walk. Was two separately-declared sets
// of these five indices (crates/client's ZOMBIE_ANIM_*, crates/server's
// SERVER_ZOMBIE_ANIM_* — the latter's own doc comment even called out the
// "a drift between them would silently desync which clip each name means"
// risk this now closes) — a single set here instead, used by both.
pub const ZOMBIE_ANIM_BITE: usize = 0;
pub const ZOMBIE_ANIM_CRAWL: usize = 1;
pub const ZOMBIE_ANIM_IDLE: usize = 2;
pub const ZOMBIE_ANIM_RUN: usize = 3;
pub const ZOMBIE_ANIM_WALK: usize = 4;

// --- Second zombie mesh (Scenario::ItemGallery animation-review rig only) ---
//
// A fully separate Meshy-generated zombie — different 24-joint rig, 7 clips
// (0..4 Text-to-Motion, 5=Running, 6=Walking) — NOT wired into gameplay:
// only spawn_item_gallery_zombie_showcase (crates/client) uses it, to stand
// one instance per clip next to the classic zombie's own row so the two
// rigs' animations can be eyeballed side by side. See commit bf3614c.
pub const ZOMBIE_MESHY_PATH: &str = "characters/zombie_meshy/mesh/zombie_meshy.glb";

// Eyeball-tuned placement for the meshy rig — its glTF ships lying on its
// back (bind pose ~flat in Y, long in Z) at an unknown scale, so the right
// numbers can only be found by eye (F2 editor). Stored in a RON file rather
// than compiled in so F9 can save edits back — see zombie_meshy_transform()
// and crates/client's save_zombie_meshy_showcase_transform.
//
// Tuple layout: (scale, mesh_y, rot_x_deg, rot_y_deg, rot_z_deg)
//   scale    — uniform scale of the mesh child
//   mesh_y   — Y offset of the mesh child relative to the (physics-settled)
//              root, to line the meshy rig's feet up with the floor — the
//              classic rig's equivalent is the baked-in ZOMBIE_MODEL_Y_OFFSET
//   rot_*    — X/Y/Z euler degrees (EulerRot::YXZ); X pitch stands it up,
//              Y yaw faces it at the player

fn zombie_meshy_transform_raw() -> (f32, f32, f32, f32, f32) {
    static DATA: std::sync::OnceLock<(f32, f32, f32, f32, f32)> = std::sync::OnceLock::new();
    *DATA.get_or_init(|| {
        // Same CARGO_MANIFEST_DIR-relative resolution as the item-gallery
        // car physics RON — see its own comment for why not CWD.
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../client/assets/data/zombie_meshy_transform.ron");
        let text = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("failed to read meshy zombie transform at {path}: {e}"));
        ron::de::from_str(&text)
            .unwrap_or_else(|e| panic!("failed to parse meshy zombie transform at {path}: {e}"))
    })
}

pub fn zombie_meshy_scale() -> f32 {
    zombie_meshy_transform_raw().0
}
/// Y offset of the meshy mesh child relative to the zombie root.
pub fn zombie_meshy_mesh_y() -> f32 {
    zombie_meshy_transform_raw().1
}
/// X/Y/Z euler degrees.
pub fn zombie_meshy_rot_deg() -> Vec3 {
    let (_, _, x, y, z) = zombie_meshy_transform_raw();
    Vec3::new(x, y, z)
}

/// Which of the two zombie meshes an entity uses. Only ItemGallery's
/// showcase rig ever sets it to `Meshy`; every networked/gameplay zombie is
/// `Classic` (and usually carries no `ZombieKind` at all — absence == Classic,
/// see `zombie_hitbox_bone_slots` / client's `zombie_mesh_bundle`).
#[derive(Component, Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum ZombieKind {
    #[default]
    Classic,
    Meshy,
}

impl std::fmt::Display for ZombieKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            ZombieKind::Classic => "classic",
            ZombieKind::Meshy => "meshy",
        })
    }
}

/// Marks a showcase zombie (ItemGallery) whose animation is pinned to this
/// clip index for its whole life — client's `wire_zombie_animations` plays
/// it once and `update_zombie_animation` skips the entity, so no pose logic
/// ever runs. Never replicated; these entities are client-local.
#[derive(Component, Clone, Copy)]
pub struct ZombieShowcaseClip(pub usize);

pub fn zombie_kind_mesh_path(kind: ZombieKind) -> &'static str {
    match kind {
        ZombieKind::Classic => ZOMBIE_PATH,
        ZombieKind::Meshy => ZOMBIE_MESHY_PATH,
    }
}

/// How many animation clips the kind's glTF file ships — one showcase
/// zombie is spawned per clip.
pub fn zombie_kind_clip_count(kind: ZombieKind) -> usize {
    match kind {
        ZombieKind::Classic => 5,
        ZombieKind::Meshy => 7,
    }
}

/// Local transform for the mesh child under a zombie root, per kind. Classic
/// keeps the exact transform on_zombie_spawned / zombie_mesh_bundle already
/// used (`ZOMBIE_MODEL_Y_OFFSET` + `ZOMBIE_SCALE`); meshy uses its own
/// eyeball-tuned consts above (Y placement is on the root instead — see
/// `zombie_kind_showcase_root_y`).
pub fn zombie_kind_mesh_transform(kind: ZombieKind) -> Transform {
    match kind {
        ZombieKind::Classic => {
            Transform::from_xyz(0.0, ZOMBIE_MODEL_Y_OFFSET, 0.0).with_scale(Vec3::splat(ZOMBIE_SCALE))
        }
        ZombieKind::Meshy => {
            let r = zombie_meshy_rot_deg();
            Transform::from_xyz(0.0, zombie_meshy_mesh_y(), 0.0)
                .with_rotation(Quat::from_euler(
                    EulerRot::YXZ,
                    r.y.to_radians(),
                    r.x.to_radians(),
                    r.z.to_radians(),
                ))
                .with_scale(Vec3::splat(zombie_meshy_scale()))
        }
    }
}

/// World Y for an ItemGallery showcase zombie's root — they have no
/// physics (walk-through, like every zombie: bare Collider, no RigidBody)
/// so the floor is a fixed known height. Classic: cancel the capsule-center
/// `ZOMBIE_MODEL_Y_OFFSET` baked into its mesh child so the feet land on
/// Y=0. Meshy: 0 — its mesh child's own `mesh_y` carries the whole offset.
pub fn zombie_kind_showcase_root_y(kind: ZombieKind) -> f32 {
    match kind {
        ZombieKind::Classic => -ZOMBIE_MODEL_Y_OFFSET,
        ZombieKind::Meshy => 0.0,
    }
}

pub const ZOMBIE_PATH_RECALC_INTERVAL: f32 = 0.8;
// How long a zombie sticks with its current chase target before re-picking
// the nearest connected player. Without this, two players standing close
// together make a zombie flicker between them every frame (recomputed
// "nearest" changes sign-flips with tiny position noise) — committing to a
// target for a while and only re-evaluating periodically (or immediately if
// that target disconnects) fixes the flicker while still adapting when
// someone new gets closer.
pub const ZOMBIE_RETARGET_INTERVAL: f32 = 1.5;
// Threat/aggro scoring (server-only state, see crates/server's
// PlayerThreat): recent gunfire makes a player "louder" to nearby zombies
// than pure distance alone would — a player shooting from further away can
// still pull a zombie's attention off someone standing closer but quiet.
// PLAYER_THREAT_PER_SHOT is added per shot fired (hit or miss — the noise is
// what matters); PLAYER_THREAT_DECAY_RATE is the fraction of remaining
// threat lost per second; ZOMBIE_THREAT_DISTANCE_BONUS converts 1.0 threat
// into meters subtracted from that player's effective distance when a
// zombie compares candidates.
pub const PLAYER_THREAT_PER_SHOT: f32 = 1.0;
pub const PLAYER_THREAT_DECAY_RATE: f32 = 0.5;
pub const ZOMBIE_THREAT_DISTANCE_BONUS: f32 = 8.0;
// Meters added to a candidate's effective distance per zombie already
// chasing them — a small nudge so re-evaluating zombies lean toward
// under-targeted players instead of everyone piling onto whoever's already
// getting the most attention. Deliberately small relative to
// ZOMBIE_THREAT_DISTANCE_BONUS: it should only break close ties, not
// override a real difference in distance or threat.
pub const ZOMBIE_LOAD_BALANCE_PENALTY: f32 = 1.0;

// --- AI Director (server-only, see crates/server) ---
// Paces bonus zombie hordes on top of the fixed baseline population
// (ZOMBIE_SPAWNS + 1:1 respawn-on-kill): if the server-visible "pressure"
// (how many zombies are currently in melee range of a player) stays low for
// a while, players are due for a scare — spawn a burst near one of them.
// Player Health/damage is purely client-local (see PlayerDied's own doc
// comment), so pressure is measured from something the server can actually
// see instead: zombies close enough to bite. Director-spawned zombies don't
// get 1:1 respawned on death (see crates/server's DirectorSpawned) so a
// sustained-calm streak doesn't permanently grow the population every time
// it fires.
pub const DIRECTOR_MELEE_RANGE: f32 = 2.5;
// How fast the smoothed pressure value chases the instantaneous melee-range
// zombie count — higher reacts faster, lower is steadier/less twitchy.
pub const DIRECTOR_PRESSURE_SMOOTHING: f32 = 0.5;
pub const DIRECTOR_CALM_PRESSURE_THRESHOLD: f32 = 1.0;
pub const DIRECTOR_CALM_DURATION_SECS: f32 = 20.0;
pub const DIRECTOR_HORDE_SIZE: usize = 10;
pub const DIRECTOR_HORDE_SPAWN_RADIUS: f32 = 6.0;
pub const ZOMBIE_STOP_DISTANCE: f32 = 1.2;

// Which animation clip a zombie should be playing right now — moved here
// (was client-only) so crates/server can drive its OWN AnimationPlayer
// through the exact same pose transitions instead of sitting frozen on
// Idle forever. That mismatch mattered: crates/server's zombie_hit_damage_
// bones tests hit points against THIS skeleton's live bone positions, not
// the client's — a server skeleton stuck in Idle meant every hit test was
// checked against arms/legs in their idle sway, never the real Walk/Run/
// Bite pose the shooter was actually looking at, for the entire time a
// zombie was doing anything but standing still. zombie_pose_for below is
// the single formula both sides now call, so there's exactly one place
// this priority order (Bite > Crawl > Run > Walk > Idle) can drift between
// client and server. Getting the EXACT same animation frame/phase on both
// sides isn't the goal (and isn't achievable — network latency alone rules
// that out); getting the same POSE CATEGORY is, since crates/server's
// existing lag-compensated rewind (ZombieHistory/zombie_history_at) already
// records real per-tick bone positions and reconstructs whatever the
// shooter's own screen was likely showing at the time they fired — once
// the server's own skeleton is actually cycling through the right poses at
// the right (server-authoritative) moments, that existing mechanism closes
// most of the remaining gap for free, no new sync channel needed.
#[derive(Component, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum ZombiePose {
    #[default]
    Idle,
    Walk,
    Run,
    Crawl,
    Bite,
}

// Health fraction at/below which a zombie switches to the Crawl pose
// (dragging itself, can't stand) instead of Walk/Run — checked before
// Run/Walk so a badly hurt zombie always crawls even while still in chase
// range, but after Bite so point-blank still always reads as an attack.
pub const ZOMBIE_CRAWL_HEALTH_FRACTION: f32 = 0.35;
// Distance (same units as ZOMBIE_STOP_DISTANCE/ZOMBIE_CHASE_RANGE) inside
// which a moving zombie reads as sprinting toward you (Run) rather than
// the ordinary Walk — deliberately well short of ZOMBIE_STOP_DISTANCE
// (which gates the Bite pose/actual damage tick), so the visual "closing
// in fast" cue reads before the attack itself lands. Was 10.0.
pub const ZOMBIE_RUN_DISTANCE: f32 = 20.0;

// The single formula deciding ZombiePose — see its own doc comment for why
// this lives here instead of being duplicated per-crate. `distance` is to
// whichever player this zombie is tracking, `health_fraction` is
// Health/ZOMBIE_MAX_HEALTH, `moving` mirrors NetTransform.moving.
// Points a zombie's (or any animated character's) root entity at the
// specific descendant entity that carries its AnimationPlayer, recorded
// once wire_zombie_animations/wire_server_zombie_animations finds it (the
// glTF scene loads asynchronously, so it doesn't exist yet at spawn time).
// Public field so both crates' own update_*_pose systems can read link.0
// directly.
#[derive(Component)]
pub struct ZombieAnimationPlayer(pub Entity);

// Walks up from a newly-spawned AnimationPlayer through ChildOf looking for
// an ancestor matching `is_root`, up to 20 hops (glTF scenes nest deeply).
// Shared by crates/client's wire_zombie_animations/wire_player_model_animations
// and crates/server's wire_server_zombie_animations so each only has to say
// what root type it's looking for, not re-implement the walk.
pub fn find_animation_root(entity: Entity, child_of: &Query<&ChildOf>, is_root: impl Fn(Entity) -> bool) -> Option<Entity> {
    let mut current = entity;
    for _ in 0..20 {
        if is_root(current) {
            return Some(current);
        }
        current = child_of.get(current).ok()?.0;
    }
    None
}

pub fn zombie_pose_for(distance: f32, health_fraction: f32, moving: bool) -> ZombiePose {
    if distance <= ZOMBIE_STOP_DISTANCE {
        ZombiePose::Bite
    } else if health_fraction <= ZOMBIE_CRAWL_HEALTH_FRACTION {
        ZombiePose::Crawl
    } else if moving && distance <= ZOMBIE_RUN_DISTANCE {
        ZombiePose::Run
    } else if moving {
        ZombiePose::Walk
    } else {
        ZombiePose::Idle
    }
}

pub const ZOMBIE_LOD_DISTANCE: f32 = 15.0;
// The Y a far (motion.far, beyond ZOMBIE_LOD_DISTANCE) zombie is pinned to
// each frame instead of running full physics — has to match how high the
// capsule naturally rests when grounded (radius + length/2), or a far
// zombie visibly sinks/floats relative to the capsule-height everyone else
// uses. Was a hardcoded 0.9 tuned for the old fixed-size capsule, derived
// now so it can't drift out of sync with ZOMBIE_RADIUS/ZOMBIE_LENGTH again
// (exactly what just happened when ZOMBIE_SCALE grew the capsule but this
// stayed put — far zombies sank into the ground).
pub const ZOMBIE_LOD_REST_HEIGHT: f32 = ZOMBIE_RADIUS + ZOMBIE_LENGTH / 2.0;
pub const ZOMBIE_AVOID_PROBE_DISTANCE: f32 = 2.5;
pub const ZOMBIE_AVOID_ANGLES_DEG: [f32; 6] = [25.0, -25.0, 50.0, -50.0, 80.0, -80.0];
pub const ZOMBIE_SEPARATION_RADIUS: f32 = 1.1;
pub const ZOMBIE_SEPARATION_STRENGTH: f32 = 1.2;
pub const KNOCKBACK_DECAY_RATE: f32 = 6.0;
// Max degrees/sec a zombie's facing can turn — move_zombies rotates toward
// its movement direction gradually instead of snapping instantly whenever
// that direction changes (every path_recalc_timer tick, or a full turn on
// retarget). Turning much faster than a real jog-turn would still be
// visible as a near-instant snap, so this is deliberately not huge.
pub const ZOMBIE_TURN_RATE_DEG: f32 = 180.0;

// A 10x10 grid spanning -half_x..half_x / -half_z..half_z at height y —
// used to generate a large ZOMBIE_SPAWNS (100 entries) without writing them
// out by hand. const fn, so this still runs at compile time.
pub const fn zombie_spawn_grid(half_x: f32, half_z: f32, y: f32) -> [Vec3; 100] {
    let mut spawns = [Vec3::ZERO; 100];
    let mut i = 0usize;
    while i < 100 {
        let row = (i / 10) as f32;
        let col = (i % 10) as f32;
        let x = -half_x + col * (2.0 * half_x / 9.0);
        let z = -half_z + row * (2.0 * half_z / 9.0);
        spawns[i] = Vec3::new(x, y, z);
        i += 1;
    }
    spawns
}

// Inset a few meters from the arena walls so nothing spawns stuck inside
// geometry. Spawn Y matches the capsule's natural grounded height (radius +
// length/2) — was hardcoded 0.9 for the old fixed-size capsule, which after
// ZOMBIE_RADIUS/ZOMBIE_LENGTH grew with ZOMBIE_SCALE left every zombie
// spawning already deeply embedded in the floor (bottom of the capsule
// ended up well below y=0), which move_and_slide couldn't fully resolve
// back out of on its own — that's what was still showing as "sunk into the
// ground" even after fixing ZOMBIE_LOD_REST_HEIGHT.
pub const ZOMBIE_SPAWNS: [Vec3; 100] =
    zombie_spawn_grid(ARENA_HALF_SIZE - 3.0, ARENA_HALF_SIZE - 3.0, ZOMBIE_RADIUS + ZOMBIE_LENGTH / 2.0);

// Scatters zombie fill-spawns (spawn_initial_zombies/on_set_zombie_count/
// tick_respawns, crates/server) across the whole arena instead of always
// walking ZOMBIE_SPAWNS' fixed grid in the same row-major order — same
// "no rand crate, deterministic per-index hash" approach entity_phase
// (crates/shared/src/hitbox.rs) already uses for per-entity staggering,
// extended to 2D via the R2 low-discrepancy sequence (successive `seed`s
// land at well-spread, non-repeating points instead of clumping the way a
// naive single-multiplier hash would for two correlated axes). `seed` is
// just NextZombieSpawnIndex's own ever-incrementing counter — same role
// it already played indexing into ZOMBIE_SPAWNS, just no longer tied to
// that grid's cell order.
pub fn random_zombie_spawn_pos(seed: usize) -> Vec3 {
    const INV_G: f32 = 0.754_877_7; // 1/g, g = plastic number (R2 sequence)
    const INV_G2: f32 = 0.569_840_3; // 1/g^2
    let half = ARENA_HALF_SIZE - 3.0;
    let x = (-1.0 + 2.0 * (seed as f32 * INV_G).fract()) * half;
    let z = (-1.0 + 2.0 * (seed as f32 * INV_G2).fract()) * half;
    Vec3::new(x, ZOMBIE_RADIUS + ZOMBIE_LENGTH / 2.0, z)
}

// Scenario::HitboxTest's 5-zombie row — same grounded-Y convention as
// ZOMBIE_SPAWNS above, at z=0 (arena center), spread across
// HITBOX_TEST_ROW_X (crate::network).
pub const HITBOX_TEST_ZOMBIE_POSITIONS: [Vec3; 5] = [
    Vec3::new(HITBOX_TEST_ROW_X[0], ZOMBIE_RADIUS + ZOMBIE_LENGTH / 2.0, 0.0),
    Vec3::new(HITBOX_TEST_ROW_X[1], ZOMBIE_RADIUS + ZOMBIE_LENGTH / 2.0, 0.0),
    Vec3::new(HITBOX_TEST_ROW_X[2], ZOMBIE_RADIUS + ZOMBIE_LENGTH / 2.0, 0.0),
    Vec3::new(HITBOX_TEST_ROW_X[3], ZOMBIE_RADIUS + ZOMBIE_LENGTH / 2.0, 0.0),
    Vec3::new(HITBOX_TEST_ROW_X[4], ZOMBIE_RADIUS + ZOMBIE_LENGTH / 2.0, 0.0),
];

// Scenario::RenderCompare's 3-zombie lineup — one entry per target pose
// (Idle/Bite/Crawl). Positions alone don't force a pose — the server's
// spawn_render_compare_zombies (crates/server) also has to give each
// entity the right components (see its own doc comment) — but the Bite
// entry's position does matter: it has to land within ZOMBIE_STOP_DISTANCE
// of RENDER_COMPARE_PLAYER_SPAWN (crate::network) once that zombie's
// ChaseTarget is pointed at the connected player, or update_server_
// zombie_pose will never actually pick Bite over Idle. Idle/Crawl's x=+-2.2
// keeps both within about 24 degrees of dead-ahead from RENDER_COMPARE_
// PLAYER_SPAWN (0, 3, 5) — narrow enough to stay in frame even at the
// desktop client's narrower (~1.5:1) window aspect, not just the phone's
// wide landscape one (an earlier +-3.0 spread cleared the phone's FOV but
// clipped off-screen on desktop, discovered by literally comparing the two
// — exactly the kind of render difference this map exists to catch). Bite
// is offset to the same side as Idle (x=-0.9) rather than dead-center or
// toward Crawl, so its up-close bulk doesn't screen-overlap Crawl on the
// other side — distance from RENDER_COMPARE_PLAYER_SPAWN is
// sqrt(0.9^2 + 0.7^2), about 1.14, comfortably under the 1.2 stop distance.
pub const RENDER_COMPARE_ZOMBIE_POSITIONS: [Vec3; 3] = [
    Vec3::new(-2.2, ZOMBIE_RADIUS + ZOMBIE_LENGTH / 2.0, 0.0), // Idle
    Vec3::new(-0.9, ZOMBIE_RADIUS + ZOMBIE_LENGTH / 2.0, 4.3), // Bite
    Vec3::new(2.2, ZOMBIE_RADIUS + ZOMBIE_LENGTH / 2.0, 0.0),  // Crawl
];

// Sent by the client's "spawn 100 more" debug button — spawning must be a
// server decision now, since zombies are server-authoritative.
#[derive(Event, Clone, Copy, Serialize, Deserialize, Default)]
pub struct SpawnMoreZombies;

// Mobile cheat panel's zombie population controls (crates/server's
// maintain_zombie_population/on_clear_all_zombies). Absolute target, not a
// delta — the client tracks its own last-set value locally (see
// ZombieCountUi) and sends the new total each tap, simplest way to stay in
// sync without the server needing to echo the current target back.
#[derive(Event, Clone, Copy, Serialize, Deserialize)]
pub struct SetZombieCount(pub u32);

// Despawns every zombie server-side (baseline AND director-spawned) and
// clears pending respawns — unlike a client just despawning its own local
// replicated copy (what the old desktop-only "Digit2: kill all" debug
// action does), this is authoritative, so they don't just reappear on the
// next replication update.
#[derive(Event, Clone, Copy, Serialize, Deserialize, Default)]
pub struct ClearAllZombies;

// Toggles whether tick_respawns actually replaces a killed baseline zombie
// — off lets the population drain down through normal play instead of
// staying pinned at the target count.
#[derive(Event, Clone, Copy, Serialize, Deserialize)]
pub struct SetAutoMaintainZombies(pub bool);

// Debug-panel "Freeze zombies" (KeyF / mobile CheatFreezeButton) — unlike
// most of the other panel toggles (ShowHitboxes, ZombieAnimDisabled, ...),
// which are purely cosmetic client-local state, freeze needs the server to
// actually stop moving zombies: their real position is server-authoritative
// (see move_zombies in crates/server), so a client-only flag can pause the
// client's own AnimationPlayer all it wants but the zombie keeps walking
// underneath it — every frame the still-moving server position gets
// replicated back down and re-interpolated into Transform regardless,
// which also kept re-triggering update_zombie_animation's pose-change
// detection and undoing the animation pause. One global bool, not per-
// player: zombies are shared world entities (this server process is
// already scoped to one map/room — see Scenario), so there's no
// "freeze only for me" that would make sense here the way noclip/autopilot
// (per-player state) do.
#[derive(Event, Clone, Copy, Serialize, Deserialize)]
pub struct SetFreezeZombies(pub bool);

// A dead zombie is replaced after this long, keeping the population topped
// up instead of thinning out.
pub const ZOMBIE_RESPAWN_DELAY: f32 = 1.0;
// Horizontal-only impulse applied to a zombie that survives a hit, along
// the bullet's own travel direction. Scaled by ZOMBIE_SCALE_FACTOR (see
// ZOMBIE_RADIUS's own comment) — an unscaled push displaces the same
// absolute world distance regardless of the zombie's visual size, which
// reads as "barely any knockback at all" once the zombie is several times
// bigger than the original ~2m calibration.
pub const KNOCKBACK_FORCE: f32 = 3.5 * ZOMBIE_SCALE_FACTOR;

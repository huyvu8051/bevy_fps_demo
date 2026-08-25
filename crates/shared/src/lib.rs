use avian3d::prelude::*;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

pub mod movement;
pub mod sim_latency;

// Plain gameplay data shared between the (future) authoritative server and
// the client — no rendering/audio handles, so this crate can compile
// headless (server side). Doc comments trimmed to what's still true from
// this crate's point of view; see the systems that use these in
// crates/client/src/main.rs / crates/server/src/main.rs for full context.

// The physics body: a capsule moved each frame via avian3d's MoveAndSlide.
// Separate from the camera entity (which just gets its translation copied
// from this each frame) so look rotation and physics movement don't fight
// over the same Transform.
#[derive(Component)]
pub struct Player {
    pub vertical_velocity: f32,
    pub grounded: bool,
}

// KeyC toggles Standing<->Crouching, KeyX toggles Standing<->Prone. Affects
// move speed, the physics capsule's length, and camera eye height.
#[derive(Component, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlayerStance {
    #[default]
    Standing,
    Crouching,
    Prone,
}

#[derive(Component, Clone, Copy, Serialize, Deserialize)]
pub struct Health(pub f32);

// Global game-over flag. Checked (early-return) at the top of the systems
// that should freeze once the player dies: move_player_physics, shoot,
// move_zombies. Reset by restart_game.
#[derive(Resource, Default)]
pub struct GameOver(pub bool);

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
pub fn wire_zombie_hitbox_bones(
    child_of: Query<&ChildOf>,
    mut zombies: Query<&mut ZombieHitboxBones, With<Zombie>>,
    new_names: Query<(Entity, &Name), Added<Name>>,
) {
    // Each entry is (rig node name, setter) — the setter both picks which
    // field this bone belongs on and, for the limb chains, which position
    // in the [shoulder/hip, elbow/knee, hand/foot] ordering, so
    // zombie_hit_damage_bones can always build its capsule segments in the
    // right order regardless of what order glTF nodes happen to load in.
    type Setter = fn(&mut ZombieHitboxBones, Entity);
    const SLOTS: &[(&str, Setter)] = &[
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

    for (bone_entity, name) in &new_names {
        let Some(&(_, set)) = SLOTS.iter().find(|&&(slot_name, _)| slot_name == name.as_str()) else {
            continue;
        };

        let mut current = bone_entity;
        for _ in 0..20 {
            if let Ok(mut bones) = zombies.get_mut(current) {
                set(&mut bones, bone_entity);
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
pub struct Chase {
    pub speed: f32,
}

// --- Networking (client <-> server) ---
// Both sides need to agree on arena geometry and player movement tuning so
// the server's authoritative physics and the client's rendering line up.

pub const ARENA_HALF_SIZE: f32 = 18.0;
pub const ARENA_WALL_HEIGHT: f32 = 5.0;
pub const ARENA_WALL_THICKNESS: f32 = 1.0;

pub const PLAYER_SPAWN: Vec3 = Vec3::new(0.0, 3.0, ARENA_HALF_SIZE - 4.0);
pub const PLAYER_MAX_HEALTH: f32 = 100.0;

// --- Test scenarios (map-select menu, iOS icon-tap) -----------------------
//
// Each scenario is its own dedicated-server process on its own port —
// bevy_replicon has no "rooms" concept, so this is the simplest way to get
// genuinely separate worlds (map = port = room; connecting to the same
// port means seeing each other, same as normal play today). The client
// picks the port from the scenario it selected (see MAP_ID in
// crates/client/src/lib.rs); the server picks both its bind port and its
// spawn preset from the same MAP_ID env var it's started with. Kept in
// `shared` so client and server can't drift on the port <-> scenario
// mapping.
pub const SERVER_PORT_BASE: u16 = 5031;

// Was two separately-declared `const PROTOCOL_ID: u64 = 0;` (one in each of
// crates/client and crates/server) — renet2 rejects a connection whose
// client and server protocol_id don't match, so a drift here fails loudly
// (connection refused) rather than silently, but there's still no reason to
// keep it as two copies that could disagree.
pub const PROTOCOL_ID: u64 = 0;

/// Which map-select preset a server process (and the client connecting to
/// it) is running. `Normal` is today's actual game — everything else is a
/// fixed test rig, not part of normal play.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Scenario {
    #[default]
    Normal,
    /// Map 1: 5 zombies stationary in a row (no AI — they never chase),
    /// player spawned facing them, for testing hit registration in
    /// isolation. A killed zombie respawns at its own row slot after
    /// HITBOX_TEST_RESPAWN_DELAY, not the normal cycling-zombie-grid
    /// behavior.
    HitboxTest,
    /// Map 2: STRESS_TEST_ZOMBIE_COUNT zombies with normal chase AI,
    /// player spawned high above the arena (noclip auto-enabled
    /// client-side, see crate::client's map-select handling) to observe
    /// FPS/behavior with a full horde active without being in melee range.
    StressTest,
}

impl Scenario {
    pub fn from_map_id(id: u8) -> Self {
        match id {
            1 => Scenario::HitboxTest,
            2 => Scenario::StressTest,
            _ => Scenario::Normal,
        }
    }

    pub fn map_id(self) -> u8 {
        match self {
            Scenario::Normal => 0,
            Scenario::HitboxTest => 1,
            Scenario::StressTest => 2,
        }
    }

    pub fn port(self) -> u16 {
        SERVER_PORT_BASE + self.map_id() as u16
    }
}

pub const HITBOX_TEST_PLAYER_SPAWN: Vec3 = PLAYER_SPAWN;
pub const HITBOX_TEST_RESPAWN_DELAY: f32 = 2.0;
// A row across the arena's center (z=0, well clear of PLAYER_SPAWN's own
// z=ARENA_HALF_SIZE-4 corner) — same grounded-height convention
// ZOMBIE_SPAWNS itself uses (radius + length/2), computed further down
// this file once ZOMBIE_RADIUS/ZOMBIE_LENGTH are in scope.
pub const HITBOX_TEST_ROW_X: [f32; 5] = [-6.0, -3.0, 0.0, 3.0, 6.0];

pub const STRESS_TEST_ZOMBIE_COUNT: usize = 50;
pub const STRESS_TEST_PLAYER_SPAWN: Vec3 = Vec3::new(0.0, 15.0, 0.0);

pub const MOVE_SPEED: f32 = 4.0;
pub const AIM_MOVE_SPEED_MULTIPLIER: f32 = 0.5;
pub const SPRINT_MULTIPLIER: f32 = 1.6;
pub const CROUCH_SPEED_MULTIPLIER: f32 = 0.5;
pub const PRONE_SPEED_MULTIPLIER: f32 = 0.25;

pub const PLAYER_RADIUS: f32 = 0.35;
pub const PLAYER_LENGTH: f32 = 0.9;
pub const CROUCH_LENGTH: f32 = PLAYER_LENGTH * 0.5;
pub const PRONE_LENGTH: f32 = PLAYER_RADIUS * 0.3;

pub const GRAVITY: f32 = -20.0;
pub const JUMP_SPEED: f32 = 6.5;
pub const GROUND_NORMAL_MIN_Y: f32 = 0.6;

// How far in the past (relative to local received-snapshot time) the
// client renders everyone else's replicated position at — see the client's
// interpolate_net_transform for the full explanation. Also used by the
// server's lag-compensated hit detection (handle_fire_shot): a shooter's
// screen reflects the world from roughly (their network latency +
// this render delay) ago, so that's how far back to rewind zombie
// positions when checking whether their shot looked like a hit to them.
pub const INTERP_DELAY_SECS: f32 = 0.1;
// Upper bound on how far handle_fire_shot will rewind zombie positions for
// lag compensation, regardless of a shooter's actual measured RTT — caps
// how far back a laggy/malicious client could shift hit detection in its
// favor (a real server would care about this for fairness/anti-cheat; here
// it's mostly just to keep the rewind sane if RTT ever reports something
// absurd).
pub const LAG_COMPENSATION_MAX_SECS: f32 = 0.3;

// Debug flight mode (KeyN) — no gravity/collision, flies in whatever
// direction move_axis+yaw points horizontally, jump/crouch to ascend/
// descend. See ClientInput.noclip and simulate_player_step.
pub const NOCLIP_SPEED: f32 = MOVE_SPEED * 3.0;

// Sent by the client every frame (Channel::Unreliable — the next one
// supersedes a dropped one, no need to retransmit). move_axis is
// pre-rotation: x = right(+)/left(-), y = forward(+)/back(-), matching the
// WASD accumulation the client used to do locally before this move; the
// server rotates it by `yaw` to get a world-space direction. stance is
// decided (and animated) client-side and just trusted here — good enough
// for a debug-scope server with no anti-cheat goals.
#[derive(Event, Component, Clone, Copy, Serialize, Deserialize, Default)]
pub struct ClientInput {
    // Increments by 1 every time the client sends an input — lets the
    // client-side predicted-movement buffer (see the client's
    // send_player_input/apply_server_player_transform) match each
    // replicated LastProcessedInput back to the exact input it was for, so
    // it knows which of its own predictions to discard vs. replay.
    pub sequence: u32,
    pub move_axis: Vec2,
    pub yaw: f32,
    pub jump: bool,
    pub sprint: bool,
    pub aiming: bool,
    pub stance: PlayerStance,
    // Client-decided debug flight toggle (KeyN) — same "trust the client,
    // no anti-cheat goals" stance as `stance` above. Server and client run
    // the exact same simulate_player_step, so simulating noclip there too
    // (instead of only client-side) keeps both sides in agreement instead
    // of the server fighting/reverting a client-only fly-through-walls
    // override every tick.
    pub noclip: bool,
}

// Replicated pose (server -> client) for any networked entity — smaller and
// serde-friendly unlike Transform, which isn't a plain `Serialize` type in
// this Bevy version. `yaw` is enough to orient a capsule body; full look
// pitch stays purely client-side (each client's own camera).
#[derive(Component, Clone, Copy, Serialize, Deserialize, Default)]
pub struct NetTransform {
    pub translation: Vec3,
    pub yaw: f32,
    // Only meaningful for Zombie entities — drives the client's Walk/Idle
    // animation pick (see crates/client's update_zombie_animation). Chase
    // itself isn't replicated (its `target: Entity` would need cross-network
    // entity mapping for no real benefit), so this is the cheap stand-in.
    pub moving: bool,
    // Only meaningful for NetPlayer entities — mirrors the shooting
    // client's own ClientInput.sprint (already sent every input tick, just
    // not previously copied into the replicated pose), so other clients
    // can show a Run pose for a sprinting remote player instead of the
    // same Walk pose regardless of speed. Always false for Zombie/DebugBot
    // entities (they don't sprint).
    pub sprinting: bool,
}

// Replicated marker for a connected player's entity (spawned by the server
// directly on the connection entity — see crates/server). Requires
// Transform so replicated NetPlayer entities have somewhere for
// mirror_net_transform to write the pose into on the client.
#[derive(Component, Clone, Copy, Default, Serialize, Deserialize)]
#[require(Transform)]
pub struct NetPlayer;

// Replicated alongside NetTransform on a player's entity — the sequence
// number of the newest ClientInput the server has actually simulated (see
// movement::simulate_player_step and crates/server's move_players). The
// client compares this against its own predicted-input buffer to know
// which predictions are now confirmed (safe to discard) vs. still ahead of
// the server and need replaying on top of the confirmed position — see
// crates/client's apply_server_player_transform.
#[derive(Component, Clone, Copy, Default, Serialize, Deserialize)]
pub struct LastProcessedInput(pub u32);

// --- Zombies (server-authoritative AI/movement, see crates/server) ---

pub const ZOMBIE_AI_ENABLED: bool = true;
// Large enough that zombies start chasing basically as soon as they spawn —
// this is meant as target practice, not a stealth encounter.
pub const ZOMBIE_CHASE_RANGE: f32 = 60.0;
pub const ZOMBIE_SPEED: f32 = 1.0;

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
// constants below still scale off this so they stay proportional to
// whatever ZOMBIE_SCALE currently is, instead of separately hardcoded
// numbers. ZOMBIE_RADIUS itself no longer uses this — see its own doc
// comment for why.
const ZOMBIE_SCALE_FACTOR: f32 = ZOMBIE_SCALE / 0.1;
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
// LIMB_HITBOX_RADIUS (the real per-bone damage regions) all derive from
// this, so damage decisions keep matching the real mesh. See
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
// kept. Radius doubled again on top of its prior 2x widen (now 4x the
// accurate ZOMBIE_RADIUS). Does NOT affect the real per-bone damage
// regions (HEAD/TORSO/LIMB_HITBOX_RADIUS still derive from the accurate
// ZOMBIE_RADIUS above) — zombie_hit_damage_ray still decides how much (if
// any) damage actually lands from where the ray actually goes; a shot that
// only reaches this outer shell and no real bone region is a genuine miss
// (see zombie_hit_damage_ray's own doc comment), not a graze.
pub const ZOMBIE_OUTER_RADIUS: f32 = ZOMBIE_RADIUS * 4.0;
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

// Debug-bot combat AI (crates/server): a debug bot with a zombie in range
// walks into DEBUG_BOT_ENGAGE_RANGE and fires on it instead of just standing
// wherever it was manually placed — see spawn_debug_bot's own doc comment.
// DETECT_RANGE mirrors ZOMBIE_CHASE_RANGE's role (how far a bot bothers
// looking for a fight); ENGAGE_RANGE is deliberately inside MAX_SHOT_DISTANCE
// so a bot always stops well within its own weapon's range before shooting.
pub const DEBUG_BOT_DETECT_RANGE: f32 = 40.0;
pub const DEBUG_BOT_ENGAGE_RANGE: f32 = 10.0;
pub const DEBUG_BOT_RETARGET_INTERVAL: f32 = 1.5;
pub const DEBUG_BOT_FIRE_INTERVAL_SECS: f32 = 0.4;
// Mirrors the client's own EYE_HEIGHT_OFFSET (standing) — a bot has no real
// camera, this just picks a plausible shot origin/height for its raycast.
pub const DEBUG_BOT_EYE_HEIGHT: f32 = 0.65;

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
// in fast" cue reads before the attack itself lands.
pub const ZOMBIE_RUN_DISTANCE: f32 = 10.0;

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
// uses. Was a hardcoded 0.9 tuned for the old fixed-size capsule; derived
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

pub const NAVMESH_HEIGHT: f32 = 1.0;
pub const NAVMESH_BOUNDS: [Vec2; 4] = [
    Vec2::new(-(ARENA_HALF_SIZE - 1.0), -(ARENA_HALF_SIZE - 1.0)),
    Vec2::new(ARENA_HALF_SIZE - 1.0, -(ARENA_HALF_SIZE - 1.0)),
    Vec2::new(ARENA_HALF_SIZE - 1.0, ARENA_HALF_SIZE - 1.0),
    Vec2::new(-(ARENA_HALF_SIZE - 1.0), ARENA_HALF_SIZE - 1.0),
];
// Radius of each USE_TEST_ARENA pillar (both the physical collider and the
// navmesh cutout column_footprint carves out of NAVMESH_BOUNDS below).
pub const PILLAR_RADIUS: f32 = 0.6;
// Scattered (x, z) positions for a handful of free-standing pillars inside
// the test arena — added as cover/obstacles to test navigation and
// line-of-sight around (autopilot targeting, zombie pathfinding) rather
// than the previously wide-open flat room. Kept clear of PLAYER_SPAWN
// (0, ARENA_HALF_SIZE - 4 = 14) and the arena center. Both crates/client's
// spawn_test_arena (visual mesh + collider) and crates/server's
// spawn_arena_colliders (collider only) iterate this same array, so the
// two processes' physical worlds can't drift out of sync with each other.
pub const NAVMESH_COLUMN_OBSTACLES: [(f32, f32); 8] = [
    (-8.0, -6.0),
    (8.0, -6.0),
    (-8.0, 6.0),
    (8.0, 6.0),
    (-4.0, 0.0),
    (4.0, 0.0),
    (0.0, -10.0),
    (0.0, 10.0),
];

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

// Scenario::HitboxTest's 5-zombie row — same grounded-Y convention as
// ZOMBIE_SPAWNS above, at z=0 (arena center), spread across
// HITBOX_TEST_ROW_X. Defined here (not next to that const) since it needs
// ZOMBIE_RADIUS/ZOMBIE_LENGTH, declared further down this file.
pub const HITBOX_TEST_ZOMBIE_POSITIONS: [Vec3; 5] = [
    Vec3::new(HITBOX_TEST_ROW_X[0], ZOMBIE_RADIUS + ZOMBIE_LENGTH / 2.0, 0.0),
    Vec3::new(HITBOX_TEST_ROW_X[1], ZOMBIE_RADIUS + ZOMBIE_LENGTH / 2.0, 0.0),
    Vec3::new(HITBOX_TEST_ROW_X[2], ZOMBIE_RADIUS + ZOMBIE_LENGTH / 2.0, 0.0),
    Vec3::new(HITBOX_TEST_ROW_X[3], ZOMBIE_RADIUS + ZOMBIE_LENGTH / 2.0, 0.0),
    Vec3::new(HITBOX_TEST_ROW_X[4], ZOMBIE_RADIUS + ZOMBIE_LENGTH / 2.0, 0.0),
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

// --- Debug bots (client debug panel, see crates/client) ---
// Fake NetPlayer stand-ins so multi-player zombie targeting can be tested
// without launching a second real client. Identified by a small server-
// assigned id rather than Entity, since client->server events don't need
// cross-network entity mapping this way.
#[derive(Component, Clone, Copy, Serialize, Deserialize)]
pub struct DebugBot(pub u32);

#[derive(Event, Clone, Copy, Serialize, Deserialize)]
pub struct SpawnDebugBot {
    pub at: Vec3,
}

#[derive(Event, Clone, Copy, Serialize, Deserialize)]
pub struct MoveDebugBot {
    pub id: u32,
    pub target: Vec3,
}

#[derive(Event, Clone, Copy, Serialize, Deserialize)]
pub struct DespawnDebugBot {
    pub id: u32,
}

// --- Shooting/damage (server-authoritative, see crates/server) ---

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
// Same distance avian3d's physics raycast in shoot() used to go — far past
// anything reachable inside the arena.
pub const MAX_SHOT_DISTANCE: f32 = 200.0;

// Body-shot kills still take this many hits; a headshot always kills in one
// (see zombie_hit_damage_bones).
pub const ZOMBIE_MAX_HEALTH: f32 = 8.0;

// Per-region damage, picked by zombie_hit_damage_bones from real bone
// hitbox spheres (HITBOX_PLAN.md Phase 4) — see that function's own doc
// comment.
pub const HEAD_DAMAGE: f32 = ZOMBIE_MAX_HEALTH; // always a one-shot kill
pub const CHEST_DAMAGE: f32 = 1.5;
pub const BELLY_DAMAGE: f32 = 1.0;
pub const LIMB_DAMAGE: f32 = 0.5;

// Height bands (world-space meters, relative to the zombie's root) and a
// lateral distance from the vertical axis through the root — the ORIGINAL
// fixed-height-band guess this project used before real bone hitboxes
// existed (see HITBOX_PLAN.md). No damage function reads these anymore
// (zombie_hit_damage_bones has no fallback to them — an unresolved region
// just scores 0 rather than falling back to a completely different
// detection method); kept only because draw_zombie_hitboxes (client-side
// debug gizmo) still draws rings at these heights for visual comparison
// against the real bone spheres.
pub const HITBOX_HEAD_MIN_Y: f32 = 0.9 * ZOMBIE_SCALE_FACTOR;
pub const HITBOX_CHEST_MIN_Y: f32 = 0.3 * ZOMBIE_SCALE_FACTOR;
pub const HITBOX_BELLY_MIN_Y: f32 = -0.2 * ZOMBIE_SCALE_FACTOR;
pub const HITBOX_LIMB_LATERAL_MIN: f32 = 0.35 * ZOMBIE_SCALE_FACTOR;

// Sphere/capsule radii (world-space meters) tested against each resolved
// bone's live world position — same "rough estimate, playtest-tunable"
// spirit as HITBOX_HEAD_MIN_Y et al. above (this codebase has never had the
// actual mesh's per-part dimensions to calibrate against), just centered on
// the real bone instead of a fixed height band. Fractions of ZOMBIE_RADIUS
// (itself derived from the real measured mesh width — see its own doc
// comment) rather than fresh ZOMBIE_SCALE_FACTOR multiples, so these two
// hitbox systems can be tuned relative to each other by eye without a unit
// conversion.
// Back to a single head sphere at the Head bone's own raw resolved position
// (no Y/forward offset, no separate neck sphere) — reverted per request.
// Sized to match ZOMBIE_RADIUS itself (1x, up from the old 0.5x) rather than
// a smaller fraction of it, so the sphere reads as "as big as the zombie's
// own body radius", not a noticeably tighter headshot window.
pub const HEAD_HITBOX_RADIUS: f32 = 1.0 * ZOMBIE_RADIUS;
// One capsule spanning belly->chest replaces the old two separate point
// spheres (see zombie_hit_damage_bones) — a single radius for both ends,
// picked as the larger of the two former sphere radii so the capsule still
// generously covers the wider (chest) end.
pub const TORSO_HITBOX_RADIUS: f32 = 0.85 * ZOMBIE_RADIUS;
pub const LIMB_HITBOX_RADIUS: f32 = 0.4 * ZOMBIE_RADIUS;

// Already-resolved world positions for a hit test — the output of either
// "read each bone's live GlobalTransform right now" (Phase 4, still used
// by the client's own prediction and as the live-hit fallback server-side)
// or "rewind ZombieHistory's stored per-bone offsets to some past instant"
// (Phase 3, zombie_history_at below). zombie_hit_damage_bones doesn't care
// which one produced it — decoupling the damage decision itself from
// "how did we get these positions" is what let Phase 3 slot in without
// changing zombie_hit_damage_bones' own logic at all, only its callers.
// Field shape mirrors ZombieHitboxBones exactly — see its own doc comment
// for why the limb chains are ordered arrays, not an unordered bag.
#[derive(Default, Clone)]
pub struct BoneWorldPositions {
    pub head: Option<Vec3>,
    pub belly: Option<Vec3>,
    pub chest: Option<Vec3>,
    pub left_arm: [Option<Vec3>; 3],
    pub right_arm: [Option<Vec3>; 3],
    pub left_leg: [Option<Vec3>; 3],
    pub right_leg: [Option<Vec3>; 3],
}

// Resolves a live ZombieHitboxBones (real Entity references) into a
// BoneWorldPositions snapshot via the given lookup — the Phase 4 (no
// rewind) path, used for the client's own local prediction and the
// server's live-hit fallback (no ZombieHistory entry recent enough to
// rewind to, e.g. a zombie that just spawned). `bone_position` is a
// callback rather than a `Query` directly so this stays callable from
// both crates/server and crates/client without depending on either
// crate's exact query-building code.
pub fn resolve_bone_world_positions(bones: &ZombieHitboxBones, bone_position: impl Fn(Entity) -> Option<Vec3>) -> BoneWorldPositions {
    let resolve_chain = |chain: &[Option<Entity>; 3]| chain.map(|joint| joint.and_then(&bone_position));
    BoneWorldPositions {
        head: bones.head.and_then(&bone_position),
        belly: bones.belly.and_then(&bone_position),
        chest: bones.chest.and_then(&bone_position),
        left_arm: resolve_chain(&bones.left_arm),
        right_arm: resolve_chain(&bones.right_arm),
        left_leg: resolve_chain(&bones.left_leg),
        right_leg: resolve_chain(&bones.right_leg),
    }
}

// Closest distance from `point` to the line segment a->b — lets a hit test
// against a capsule spanning two joints (shoulder->elbow, belly->chest)
// instead of just a point-sphere at one end. Clamped to the segment (not an
// infinite line), so a hit well past either end still measures against
// that end, same as a capsule's rounded cap.
fn point_segment_distance(point: Vec3, a: Vec3, b: Vec3) -> f32 {
    let ab = b - a;
    let len_sq = ab.length_squared();
    if len_sq <= f32::EPSILON {
        return point.distance(a);
    }
    let t = ((point - a).dot(ab) / len_sq).clamp(0.0, 1.0);
    point.distance(a + ab * t)
}

// Same t as point_segment_distance's own internal computation, exposed on
// its own for zombie_hit_damage_ray's chest-vs-belly split (it already has
// a real hit point on the torso capsule's surface, not just a distance, so
// it needs to know WHERE along the segment that landed).
fn point_segment_t(point: Vec3, a: Vec3, b: Vec3) -> f32 {
    let ab = b - a;
    let len_sq = ab.length_squared();
    if len_sq <= f32::EPSILON {
        return 0.0;
    }
    ((point - a).dot(ab) / len_sq).clamp(0.0, 1.0)
}

// Position + rotation + full length for a capsule spanning a->b, or None
// for a degenerate (zero-length) segment — Capsule3d/avian3d's Collider::
// capsule are both built along the local Y axis, so this is the same
// from_rotation_arc trick draw_bone_capsule (crates/client) uses to orient
// one along an arbitrary segment.
fn capsule_transform(a: Vec3, b: Vec3) -> Option<(Vec3, Quat, f32)> {
    let segment = b - a;
    let direction = Dir3::new(segment).ok()?;
    Some((a.midpoint(b), Quat::from_rotation_arc(Vec3::Y, *direction), segment.length()))
}

// Deterministic per-entity pseudo-randomness via a golden-ratio hash (this
// codebase avoids a `rand` dependency — same spirit as the sin-based jitter
// used for particle directions elsewhere) — returns a value in [0, 1) that
// stays stable for a given entity but doesn't correlate across entities
// spawned at the same time, unlike hashing spawn position directly (which
// aliases badly against ZOMBIE_SPAWNS' regular grid — see this fn's
// original introduction for the full story). Used both for staggering
// per-zombie timers (move_zombies, bot_pick_target) and for
// random_aim_point below.
pub fn entity_phase(entity: Entity) -> f32 {
    (entity.index().index() as f32 * 0.618_034).fract()
}

// Which body region to aim at, and by how much, for random_aim_point below
// — deliberately weighted toward center-mass (Body highest), so autopilot
// reads as "aiming like a person," not a perfect aimbot. Head is excluded
// entirely (not just given a low weight) per request — see random_aim_point,
// which never rolls it. Playtest-tunable like every other hitbox constant
// in this file.
pub const AIM_REGION_WEIGHT_FOREARM: f32 = 2.0;
pub const AIM_REGION_WEIGHT_UPPER_ARM: f32 = 3.0;
pub const AIM_REGION_WEIGHT_THIGH: f32 = 4.0;
pub const AIM_REGION_WEIGHT_SHIN: f32 = 5.0;
pub const AIM_REGION_WEIGHT_BODY: f32 = 10.0;

// Weighted-random point on `bones` to aim at (autopilot_think — see
// AIM_REGION_WEIGHT_* above for the bias). The pick is a pure function of
// `target` (via entity_phase), not re-rolled every frame — the same
// committed target keeps aiming at the same body part instead of jittering
// between regions every tick, and naturally re-rolls whenever autopilot
// commits to a different target (see AutopilotState's own retarget-timer
// doc comment).
// Returns None if the picked region's bones haven't resolved yet (mid-load
// window, same "eat the brief gap, don't silently substitute a different
// method" stance zombie_hit_damage_bones takes) — callers just keep
// whatever aim point they already had for that one frame.
// The `&'static str` half of the return is purely a debug/HUD label (e.g.
// "showing which body part autopilot is currently aiming at" on screen) —
// never fed back into damage/collision math, so it's fine as a plain name.
pub fn random_aim_point(bones: &BoneWorldPositions, target: Entity) -> Option<(Vec3, &'static str)> {
    const TOTAL_WEIGHT: f32 =
        AIM_REGION_WEIGHT_FOREARM + AIM_REGION_WEIGHT_UPPER_ARM + AIM_REGION_WEIGHT_THIGH + AIM_REGION_WEIGHT_SHIN + AIM_REGION_WEIGHT_BODY;
    let base = entity_phase(target);
    let mut roll = base * TOTAL_WEIGHT;
    // A second, decorrelated-enough draw off the same base for picking a
    // side — doesn't need to be statistically perfect, just not visibly
    // tied to the category roll (e.g. always left when category is high).
    let left = (base * 97.0).fract() < 0.5;

    if roll < AIM_REGION_WEIGHT_FOREARM {
        let chain = if left { &bones.left_arm } else { &bones.right_arm };
        return Option::zip(chain[1], chain[2]).map(|(a, b)| (a.midpoint(b), "Forearm"));
    }
    roll -= AIM_REGION_WEIGHT_FOREARM;
    if roll < AIM_REGION_WEIGHT_UPPER_ARM {
        let chain = if left { &bones.left_arm } else { &bones.right_arm };
        return Option::zip(chain[0], chain[1]).map(|(a, b)| (a.midpoint(b), "Upper arm"));
    }
    roll -= AIM_REGION_WEIGHT_UPPER_ARM;
    if roll < AIM_REGION_WEIGHT_THIGH {
        let chain = if left { &bones.left_leg } else { &bones.right_leg };
        return Option::zip(chain[0], chain[1]).map(|(a, b)| (a.midpoint(b), "Thigh"));
    }
    roll -= AIM_REGION_WEIGHT_THIGH;
    if roll < AIM_REGION_WEIGHT_SHIN {
        let chain = if left { &bones.left_leg } else { &bones.right_leg };
        return Option::zip(chain[1], chain[2]).map(|(a, b)| (a.midpoint(b), "Shin"));
    }
    // Body (the remaining/highest-weight bucket).
    match (bones.belly, bones.chest) {
        (Some(belly), Some(chest)) => Some((belly.midpoint(chest), "Body")),
        (Some(p), None) | (None, Some(p)) => Some((p, "Body")),
        (None, None) => None,
    }
}

// Real per-bone hit detection (HITBOX_PLAN.md Phase 4, positions
// optionally lag-compensated as of Phase 3) — checked in the same
// head-first priority order the old height-band guess used (so an
// overlapping head+torso region still resolves to a headshot, matching the
// behavior players already know). No fallback to the old geometric guess
// anymore: a region whose bones haven't resolved yet (or a hit that lands
// outside every capsule/sphere) simply scores 0 damage — bones resolve
// within a couple frames of a zombie spawning (see wire_zombie_hitbox_bones),
// so this window is brief, and mixing in a completely different
// hit-detection method as a silent fallback was judged worse than just
// eating that brief no-damage window.
//
// Head is a single point-sphere at the Head bone's own raw resolved
// position (no offset, no separate neck sphere — reverted per request).
// Limbs and torso are tested as capsule SEGMENTS between consecutive joints
// (shoulder->elbow->hand, hip->knee->foot, belly->chest) instead of a
// point-sphere at each joint — a point-sphere-per-joint approach left real
// dead zones along the visible forearm/shin/torso where a clean hit
// partway between two joints landed outside every sphere and scored 0
// damage even though it clearly hit the mesh.
pub fn zombie_hit_damage_bones(hit_point: Vec3, bones: &BoneWorldPositions, _forward: Vec3) -> f32 {
    if let Some(pos) = bones.head {
        if hit_point.distance(pos) <= HEAD_HITBOX_RADIUS {
            return HEAD_DAMAGE;
        }
    }
    for chain in [&bones.left_arm, &bones.right_arm, &bones.left_leg, &bones.right_leg] {
        for pair in chain.windows(2) {
            if let (Some(a), Some(b)) = (pair[0], pair[1]) {
                if point_segment_distance(hit_point, a, b) <= LIMB_HITBOX_RADIUS {
                    return LIMB_DAMAGE;
                }
            }
        }
    }
    if let (Some(belly), Some(chest)) = (bones.belly, bones.chest) {
        let ab = chest - belly;
        let len_sq = ab.length_squared();
        if len_sq > f32::EPSILON {
            let t = ((hit_point - belly).dot(ab) / len_sq).clamp(0.0, 1.0);
            let closest = belly + ab * t;
            if hit_point.distance(closest) <= TORSO_HITBOX_RADIUS {
                // Which half of the capsule the hit landed closer to
                // decides chest vs. belly damage — t=0 is belly's end,
                // t=1 is chest's.
                return if t >= 0.5 { CHEST_DAMAGE } else { BELLY_DAMAGE };
            }
        }
    }
    // Only one of belly/chest resolved (mid-load, see wire_zombie_hitbox_bones)
    // — no capsule to build yet, fall back to a plain point-sphere on
    // whichever one exists rather than scoring 0 for the whole torso.
    if let Some(pos) = bones.chest {
        if hit_point.distance(pos) <= TORSO_HITBOX_RADIUS {
            return CHEST_DAMAGE;
        }
    }
    if let Some(pos) = bones.belly {
        if hit_point.distance(pos) <= TORSO_HITBOX_RADIUS {
            return BELLY_DAMAGE;
        }
    }
    0.0
}

// Diagnostic only — never used by the actual damage decision. Finds
// whichever real hit region (head/torso/each limb segment) `hit_point`
// came CLOSEST to, along with that region's own radius, so a 0-damage hit
// can be logged as "missed the torso capsule by 6cm" instead of just
// "miss" — lets a near-miss (radius/timing precision issue) be told apart
// from a wildly-off one (an actual bug: wrong bone, wrong coordinate
// space, stale positions) from the log alone. Mirrors zombie_hit_damage_
// bones' own region set exactly, so "which region is nearest" always means
// the same thing in both places.
pub fn zombie_hit_nearest_region(hit_point: Vec3, bones: &BoneWorldPositions, _forward: Vec3) -> Option<(&'static str, f32, f32)> {
    let mut nearest: Option<(&'static str, f32, f32)> = None;
    let mut consider = |name: &'static str, distance: f32, radius: f32| {
        if nearest.is_none_or(|(_, best, _)| distance < best) {
            nearest = Some((name, distance, radius));
        }
    };
    if let Some(pos) = bones.head {
        consider("head", hit_point.distance(pos), HEAD_HITBOX_RADIUS);
    }
    for (name, chain) in [
        ("left_arm", &bones.left_arm),
        ("right_arm", &bones.right_arm),
        ("left_leg", &bones.left_leg),
        ("right_leg", &bones.right_leg),
    ] {
        for pair in chain.windows(2) {
            if let (Some(a), Some(b)) = (pair[0], pair[1]) {
                consider(name, point_segment_distance(hit_point, a, b), LIMB_HITBOX_RADIUS);
            }
        }
    }
    if let (Some(belly), Some(chest)) = (bones.belly, bones.chest) {
        consider("torso", point_segment_distance(hit_point, belly, chest), TORSO_HITBOX_RADIUS);
    }
    nearest
}

// Real ray-vs-shape hit test against the ACTUAL per-bone regions (head
// sphere, torso capsule, 4 limb capsule chains) — unlike
// zombie_hit_damage_bones (tests one already-known point's DISTANCE to
// each region), this casts the real ray directly into each region's own
// capsule/sphere shape, exactly the same way the outer broad-phase capsule
// itself is tested. Needed because a hit_point sampled from the outer
// capsule's own surface (its entry point, facing the shooter — see
// ZOMBIE_OUTER_RADIUS's own doc comment) can land meaningfully outside
// every real region even on a shot dead-center on the mesh, once the outer
// shell is wider than the real body — that surface point sits on the wide
// shell, not the thin real bone surface, and the ray was never actually
// tested any farther in. Whichever region the ray reaches FIRST wins (not
// the old head>limb>torso priority order — a real ray naturally hits
// whatever's physically closest along its path, which is the correct
// answer once every region is a real shape instead of a point-distance
// check). Returns None if the ray misses every region entirely.
pub fn zombie_hit_damage_ray(origin: Vec3, direction: Dir3, max_distance: f32, bones: &BoneWorldPositions, _forward: Vec3) -> Option<(Vec3, Vec3, f32)> {
    let mut best: Option<(f32, Vec3, f32)> = None;
    let mut consider = |hit: Option<(f32, Vec3)>, damage: f32| {
        if let Some((toi, normal)) = hit {
            if best.is_none_or(|(best_toi, ..)| toi < best_toi) {
                best = Some((toi, normal, damage));
            }
        }
    };

    if let Some(pos) = bones.head {
        consider(Collider::sphere(HEAD_HITBOX_RADIUS).cast_ray(pos, Quat::IDENTITY, origin, *direction, max_distance, true), HEAD_DAMAGE);
    }
    for chain in [&bones.left_arm, &bones.right_arm, &bones.left_leg, &bones.right_leg] {
        for pair in chain.windows(2) {
            if let (Some(a), Some(b)) = (pair[0], pair[1]) {
                if let Some((position, rotation, length)) = capsule_transform(a, b) {
                    consider(Collider::capsule(LIMB_HITBOX_RADIUS, length).cast_ray(position, rotation, origin, *direction, max_distance, true), LIMB_DAMAGE);
                }
            }
        }
    }
    if let (Some(belly), Some(chest)) = (bones.belly, bones.chest) {
        if let Some((position, rotation, length)) = capsule_transform(belly, chest) {
            if let Some((toi, normal)) = Collider::capsule(TORSO_HITBOX_RADIUS, length).cast_ray(position, rotation, origin, *direction, max_distance, true) {
                // Which half of the capsule the real hit landed closer to
                // decides chest vs. belly damage, same split
                // zombie_hit_damage_bones uses.
                let t = point_segment_t(origin + *direction * toi, belly, chest);
                let damage = if t >= 0.5 { CHEST_DAMAGE } else { BELLY_DAMAGE };
                if best.is_none_or(|(best_toi, ..)| toi < best_toi) {
                    best = Some((toi, normal, damage));
                }
            }
        }
    }

    best.map(|(toi, normal, damage)| (origin + *direction * toi, normal, damage))
}

// Regression tests for HITBOX_PLAN.md Phase 4 — zombie_hit_damage_bones'
// priority order and fallback behavior in isolation, without needing a
// live client/bot to actually fire a shot. Kept permanently.
#[cfg(test)]
mod hitbox_phase4_tests {
    use super::*;

    fn bones_at(head: Vec3) -> (ZombieHitboxBones, std::collections::HashMap<Entity, Vec3>) {
        let head_entity = Entity::from_raw_u32(1).unwrap();
        let chest_entity = Entity::from_raw_u32(2).unwrap();
        let belly_entity = Entity::from_raw_u32(3).unwrap();
        let shoulder_entity = Entity::from_raw_u32(4).unwrap();
        let elbow_entity = Entity::from_raw_u32(5).unwrap();
        let hand_entity = Entity::from_raw_u32(6).unwrap();
        let bones = ZombieHitboxBones {
            head: Some(head_entity),
            belly: Some(belly_entity),
            chest: Some(chest_entity),
            left_arm: [Some(shoulder_entity), Some(elbow_entity), Some(hand_entity)],
            right_arm: [None, None, None],
            left_leg: [None, None, None],
            right_leg: [None, None, None],
        };
        let mut positions = std::collections::HashMap::new();
        positions.insert(head_entity, head);
        positions.insert(chest_entity, head - Vec3::Y * 0.6);
        positions.insert(belly_entity, head - Vec3::Y * 1.1);
        positions.insert(shoulder_entity, head + Vec3::X * 0.6);
        positions.insert(elbow_entity, head + Vec3::X * 1.0);
        positions.insert(hand_entity, head + Vec3::X * 1.4);
        (bones, positions)
    }

    #[test]
    fn hit_exactly_on_head_bone_is_a_headshot() {
        let (bones, positions) = bones_at(Vec3::new(0.0, 1.9, 0.0));
        let resolved = resolve_bone_world_positions(&bones, |e| positions.get(&e).copied());
        let head_pos = resolved.head.unwrap();
        let damage = zombie_hit_damage_bones(head_pos, &resolved, Vec3::NEG_Z);
        assert_eq!(damage, HEAD_DAMAGE);
    }

    #[test]
    fn hit_near_limb_joint_is_a_limb_hit_even_though_closer_to_head_height() {
        let (bones, positions) = bones_at(Vec3::new(0.0, 1.9, 0.0));
        let resolved = resolve_bone_world_positions(&bones, |e| positions.get(&e).copied());
        let shoulder_pos = resolved.left_arm[0].unwrap();
        // Same world Y as the head, but far enough laterally that only the
        // limb capsule (not the head sphere) should contain it — this is
        // exactly the case the old height-band guess handled via a
        // separate lateral-distance check; bone-based detection gets it
        // for free from the limb joints' own real positions.
        let damage = zombie_hit_damage_bones(shoulder_pos, &resolved, Vec3::NEG_Z);
        assert_eq!(damage, LIMB_DAMAGE);
    }

    #[test]
    fn hit_midway_between_limb_joints_is_still_a_limb_hit() {
        // The actual dead-zone bug this capsule-segment approach fixes: a
        // point-sphere-per-joint design scored 0 damage for a hit landing
        // cleanly on the visible forearm mesh but equidistant from both the
        // shoulder and elbow spheres. A capsule spanning the two joints has
        // no such gap.
        let (bones, positions) = bones_at(Vec3::new(0.0, 1.9, 0.0));
        let resolved = resolve_bone_world_positions(&bones, |e| positions.get(&e).copied());
        let shoulder = resolved.left_arm[0].unwrap();
        let elbow = resolved.left_arm[1].unwrap();
        let midpoint = shoulder.lerp(elbow, 0.5);
        assert_eq!(zombie_hit_damage_bones(midpoint, &resolved, Vec3::NEG_Z), LIMB_DAMAGE);
    }

    #[test]
    fn hit_near_chest_bone_is_a_chest_hit() {
        let (bones, positions) = bones_at(Vec3::new(0.0, 1.9, 0.0));
        let resolved = resolve_bone_world_positions(&bones, |e| positions.get(&e).copied());
        let chest_pos = resolved.chest.unwrap();
        let damage = zombie_hit_damage_bones(chest_pos, &resolved, Vec3::NEG_Z);
        assert_eq!(damage, CHEST_DAMAGE);
    }

    #[test]
    fn hit_midway_on_torso_scores_belly_or_chest_not_zero() {
        // Same dead-zone fix as the limb test above, applied to the
        // belly<->chest capsule.
        let (bones, positions) = bones_at(Vec3::new(0.0, 1.9, 0.0));
        let resolved = resolve_bone_world_positions(&bones, |e| positions.get(&e).copied());
        let belly = resolved.belly.unwrap();
        let chest = resolved.chest.unwrap();
        let midpoint = belly.lerp(chest, 0.5);
        let damage = zombie_hit_damage_bones(midpoint, &resolved, Vec3::NEG_Z);
        assert!(damage == CHEST_DAMAGE || damage == BELLY_DAMAGE, "expected torso damage, got {damage}");
    }

    #[test]
    fn unresolved_bones_score_zero_damage_not_a_fallback_guess() {
        let empty = BoneWorldPositions::default();
        let hit_point = Vec3::new(0.0, 1.9, 0.0);
        // No bone resolved at all (e.g. scene still loading) — must score
        // 0, not silently fall back to a different detection method.
        assert_eq!(zombie_hit_damage_bones(hit_point, &empty, Vec3::NEG_Z), 0.0);
    }

    #[test]
    fn far_from_every_bone_scores_zero_damage() {
        let (bones, positions) = bones_at(Vec3::new(0.0, 1.9, 0.0));
        let resolved = resolve_bone_world_positions(&bones, |e| positions.get(&e).copied());
        // Nowhere near any bone's capsule/sphere — confirms this scores 0
        // rather than crediting some other region by accident.
        let hit_point = Vec3::new(5.0, 1.0, 0.0);
        assert_eq!(zombie_hit_damage_bones(hit_point, &resolved, Vec3::NEG_Z), 0.0);
    }
}

// Sent by the client every time it fires (Channel::Ordered — unlike
// per-frame movement input, a dropped shot shouldn't just be superseded).
// The client computes the exact ray (including its own spread/recoil
// jitter) and the server just casts it — trusting the client's aim, same
// "no anti-cheat goals for a debug server" stance as ClientInput's stance
// field.
#[derive(Event, Clone, Copy, Serialize, Deserialize)]
pub struct FireShot {
    pub origin: Vec3,
    pub direction: Vec3,
}

// Broadcast by the server alongside a FireShot's own ShotFx, so every OTHER
// client (not just the shooter, who already knows locally) can play a
// brief Gun_Shoot pose on that specific remote player's model — see
// crate::client's update_remote_player_model_animation. No prior signal
// like this existed (ShotFx itself carries only the hit point/damage, not
// who fired). `u64` instead of `bevy_replicon`'s own `NetworkId` type
// purely to avoid adding a `bevy_replicon` dependency to this crate just
// for one field — convert via `NetworkId::new(id)`/`.get()` at the two
// call sites that actually need the real type (server broadcasting,
// client matching against a NetPlayer's own `NetworkId` component).
#[derive(Event, Clone, Copy, Serialize, Deserialize)]
pub struct PlayerFiredFx {
    pub shooter_network_id: u64,
}

// Broadcast by the server the instant a specific player's client reports
// PlayerDied — previously the only signal was the global GameStateEvent::
// GameOver, sent once *every* connected player is dead with no per-entity
// distinction, so other clients had no way to tell which remote player(s)
// had already died. Lets crate::client play a one-shot Death pose on that
// specific player's model instead of everyone just freezing mid-animation
// once the whole match ends.
#[derive(Event, Clone, Copy, Serialize, Deserialize)]
pub struct PlayerDiedFx {
    pub who_network_id: u64,
}

// Sent by the server after resolving a FireShot, to play hitmarker/blood/
// spark/floating-damage VFX. SurfaceHit uses SendMode::Broadcast (every
// client, including the shooter — nothing was predicted for it locally).
// ZombieHit uses SendMode::BroadcastExcept(shooter) instead: the shooter
// already predicted and showed their own hitmarker locally the instant
// they fired (see crate::client's shoot()), so echoing this back to them
// too would double it up. The shooter's own floating-damage-number/blood-
// or-spark choice instead waits on ShotConfirmed below, not this event.
// `origin` (the shooter's own FireShot.origin, echoed back) exists purely so
// receiving clients can draw a bullet tracer for someone ELSE'S shot —
// there's no other way to know where a remote shot came from (ShotFx never
// carried who fired, and NetTransform only has yaw, not the pitch/eye-height
// a real muzzle position would need).
#[derive(Event, Clone, Copy, Serialize, Deserialize)]
pub enum ShotFx {
    ZombieHit { origin: Vec3, point: Vec3, normal: Vec3, damage: f32, killed: bool },
    SurfaceHit { origin: Vec3, point: Vec3, normal: Vec3 },
}

// Sent by the server directly to the shooter only (SendMode::Direct), right
// alongside the ZombieHit broadcast everyone else gets — carries the
// server's own authoritative point/damage for that same shot. The floating
// damage number and the blood-vs-spark choice (both depend on knowing the
// real damage, which a local guess could get wrong — see
// spawn_zombie_hit_feedback's own doc comment for the bug that caused) are
// deliberately gated on THIS arriving, not on the client's own local
// capsule-vs-zombie prediction in shoot(). The hitmarker itself still shows
// immediately from that local prediction (an honest "the raycast physically
// stopped here" signal that doesn't depend on knowing damage), so firing
// still feels responsive — only the damage-dependent half of the feedback
// waits on the server's real answer.
#[derive(Event, Clone, Copy, Serialize, Deserialize)]
pub struct ShotConfirmed {
    pub point: Vec3,
    pub normal: Vec3,
    pub damage: f32,
    pub killed: bool,
}

// --- Game state (server-authoritative, see crates/server) ---

// Sent once by a client the moment its own local Health hits 0 — the
// server tracks which connected players have reported dead and declares
// GameOver once all of them have (see GameStateEvent).
#[derive(Event, Clone, Copy, Serialize, Deserialize, Default)]
pub struct PlayerDied;

// Sent by a client's restart keybind. Only meaningful once the server has
// broadcast GameStateEvent::GameOver, but the server itself is the one that
// actually decides whether/how to reset (despawn+respawn zombies, revive
// all players) — same "server owns the shared world" principle as
// everything else moved there in earlier phases.
#[derive(Event, Clone, Copy, Serialize, Deserialize, Default)]
pub struct RequestRestart;

// Broadcast by the server on the two game-state transitions clients need to
// react to.
#[derive(Event, Clone, Copy, Serialize, Deserialize)]
pub enum GameStateEvent {
    GameOver,
    Restarted,
}

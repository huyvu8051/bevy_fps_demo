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
#[derive(Component, Default)]
pub struct ZombieHitboxBones {
    pub head: Option<Entity>,
    pub chest: Option<Entity>,
    pub belly: Option<Entity>,
    pub limbs: Vec<Entity>,
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
    for (bone_entity, name) in &new_names {
        let is_head = name.as_str() == "Head";
        let is_chest = name.as_str() == "Spine2";
        let is_belly = name.as_str() == "Spine";
        // Upper arm/leg alone left forearm/hand/lower-leg/foot hits with no
        // nearby marker of their own, misattributing to whatever tracked
        // bone happened to be geometrically closest — covering the full
        // chain fixes that.
        let is_limb = matches!(
            name.as_str(),
            "LeftArm"
                | "RightArm"
                | "LeftForeArm"
                | "RightForeArm"
                | "LeftHand"
                | "RightHand"
                | "LeftUpLeg"
                | "RightUpLeg"
                | "LeftLeg"
                | "RightLeg"
                | "LeftFoot"
                | "RightFoot"
        );
        if !(is_head || is_chest || is_belly || is_limb) {
            continue;
        }

        let mut current = bone_entity;
        for _ in 0..20 {
            if let Ok(mut bones) = zombies.get_mut(current) {
                if is_head {
                    bones.head = Some(bone_entity);
                } else if is_chest {
                    bones.chest = Some(bone_entity);
                } else if is_belly {
                    bones.belly = Some(bone_entity);
                } else {
                    bones.limbs.push(bone_entity);
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
// 0.1) — everything below scales off this so the capsule and per-region
// damage thresholds stay proportional to whatever ZOMBIE_SCALE currently
// is, instead of separately hardcoded numbers.
const ZOMBIE_SCALE_FACTOR: f32 = ZOMBIE_SCALE / 0.1;
// No longer aliased to the player's capsule (PLAYER_RADIUS/PLAYER_LENGTH) —
// a zombie 2.5x the player's visual size should have a matching hitbox, not
// a human-sized one buried inside it.
pub const ZOMBIE_RADIUS: f32 = 0.35 * ZOMBIE_SCALE_FACTOR;
// Capsule height (2*radius + length) is set to exactly ZOMBIE_WORLD_HEIGHT
// so a raycast/capsule-cast covers the mesh's full height, including the
// very top of the head — no gap left uncovered like before. ZOMBIE_RADIUS
// isn't derived from ZOMBIE_WORLD_HEIGHT (it scales off ZOMBIE_SCALE_FACTOR
// independently), so this can go negative if ZOMBIE_MESH_REFERENCE_HEIGHT
// is ever set too small relative to 2*ZOMBIE_RADIUS — avian3d's capsule
// collider doesn't panic on that (SharedShape::capsule just takes two
// endpoints, so a negative length silently flips them and produces a
// capsule sized off |length| instead — see the client's draw_zombie_hitboxes
// for why that made the debug capsule gizmo visibly lie about the real
// hitbox size). A negative value here is always a bug, not a valid "smaller
// zombie" — shrink ZOMBIE_SCALE instead, which scales both terms together.
pub const ZOMBIE_LENGTH: f32 = ZOMBIE_WORLD_HEIGHT - 2.0 * ZOMBIE_RADIUS;
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
pub const NAVMESH_COLUMN_OBSTACLES: [(f32, f32); 0] = [];

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
// (see zombie_hit_damage_geometric).
pub const ZOMBIE_MAX_HEALTH: f32 = 8.0;

// Per-region damage. Historically the ONLY way to decide "head vs chest vs
// belly vs limb" was zombie_hit_damage_geometric's fixed-height-band guess
// (see its own doc comment below), because the server had no skeleton/bone
// data at all. As of HITBOX_PLAN.md Phase 2, real bone entities ARE
// available (ZombieHitboxBones, populated server-side too, not just
// client-side) — zombie_hit_damage_bones (below) uses them directly and is
// what handle_fire_shot/shoot() actually call now;
// zombie_hit_damage_geometric stays only as its fallback for the brief
// window before a freshly-spawned zombie's glTF scene finishes loading and
// its bones resolve.
pub const HEAD_DAMAGE: f32 = ZOMBIE_MAX_HEALTH; // always a one-shot kill
pub const CHEST_DAMAGE: f32 = 1.5;
pub const BELLY_DAMAGE: f32 = 1.0;
pub const LIMB_DAMAGE: f32 = 0.5;

// Height bands (world-space meters, relative to the zombie's root, which
// sits around the physics capsule's center — roughly hip/waist height, not
// the feet) and a lateral distance from the vertical axis through the root,
// used to tell an out-flung arm from the torso at similar heights. Rough
// estimates, originally tuned for the ~2m-tall zombie at ZOMBIE_SCALE = 0.1
// — not visually calibrated. Scaled by ZOMBIE_SCALE_FACTOR (see
// ZOMBIE_RADIUS's own comment) so a bigger/smaller zombie's head/chest/
// belly bands grow or shrink along with it instead of staying fixed at the
// original size while the mesh around them changes.
pub const HITBOX_HEAD_MIN_Y: f32 = 0.9 * ZOMBIE_SCALE_FACTOR;
pub const HITBOX_CHEST_MIN_Y: f32 = 0.3 * ZOMBIE_SCALE_FACTOR;
pub const HITBOX_BELLY_MIN_Y: f32 = -0.2 * ZOMBIE_SCALE_FACTOR;
pub const HITBOX_LIMB_LATERAL_MIN: f32 = 0.35 * ZOMBIE_SCALE_FACTOR;

// Approximates head/chest/belly/limb from the hit point's height and
// lateral distance from the zombie's own vertical axis. Works in the
// zombie's own local space (undoes its current yaw) so the lateral-distance
// check means the same thing regardless of which way the zombie is
// currently facing.
pub fn zombie_hit_damage_geometric(hit_point: Vec3, zombie_transform: &Transform) -> f32 {
    let relative = hit_point - zombie_transform.translation;
    let local = zombie_transform.rotation.inverse() * relative;
    let lateral = local.x.hypot(local.z);

    if local.y >= HITBOX_HEAD_MIN_Y {
        HEAD_DAMAGE
    } else if lateral >= HITBOX_LIMB_LATERAL_MIN {
        LIMB_DAMAGE
    } else if local.y >= HITBOX_CHEST_MIN_Y {
        CHEST_DAMAGE
    } else if local.y >= HITBOX_BELLY_MIN_Y {
        BELLY_DAMAGE
    } else {
        LIMB_DAMAGE
    }
}

// Sphere radii (world-space meters) tested against each resolved bone's
// live world position — same "rough estimate, playtest-tunable" spirit as
// HITBOX_HEAD_MIN_Y et al. above (this codebase has never had the actual
// mesh's per-part dimensions to calibrate against), just centered on the
// real bone instead of a fixed height band. Fractions of ZOMBIE_RADIUS
// (itself derived from the real measured mesh height — see its own doc
// comment) rather than fresh ZOMBIE_SCALE_FACTOR multiples, so these two
// hitbox systems can be tuned relative to each other by eye without a unit
// conversion.
pub const HEAD_HITBOX_RADIUS: f32 = 0.5 * ZOMBIE_RADIUS;
pub const CHEST_HITBOX_RADIUS: f32 = 0.85 * ZOMBIE_RADIUS;
pub const BELLY_HITBOX_RADIUS: f32 = 0.8 * ZOMBIE_RADIUS;
pub const LIMB_HITBOX_RADIUS: f32 = 0.4 * ZOMBIE_RADIUS;

// Real per-bone hit detection (HITBOX_PLAN.md Phase 4) — tests the hit
// point against a small sphere centered on each resolved bone's live
// world position, checked in the same head-first priority order
// zombie_hit_damage_geometric used (so an overlapping head+chest sphere
// still resolves to a headshot, matching the behavior players already
// know), falling back to the old height-band guess for any bone that
// hasn't resolved yet (e.g. a zombie whose glTF scene is still loading —
// see wire_zombie_hitbox_bones) rather than silently treating a genuine
// hit as a miss.
//
// `bone_position` is a callback rather than a `Query` directly so this
// stays callable from both crates/server (real per-bone hit detection)
// and crates/client (shoot()'s local prediction, which already has live
// bones via its own ZombieHitboxBones) without this function depending on
// either crate's exact query-building code — the caller just answers "if
// this bone entity resolved, where is it in world space right now?".
//
// Note: uses each bone's CURRENT world position, not a lag-compensated
// rewind of it — HITBOX_PLAN.md Phase 3 (not done yet) covers rewinding
// per-bone offsets the same way zombie_history_at already rewinds the
// root; until then, a moving/attacking zombie's bones can be tested
// slightly out of sync with where the shooter's screen actually showed
// them, same class of imprecision the root-only rewind already accepted
// for capsule tests before this.
pub fn zombie_hit_damage_bones(
    hit_point: Vec3,
    zombie_transform: &Transform,
    bones: &ZombieHitboxBones,
    bone_position: impl Fn(Entity) -> Option<Vec3>,
) -> f32 {
    if let Some(pos) = bones.head.and_then(&bone_position) {
        if hit_point.distance(pos) <= HEAD_HITBOX_RADIUS {
            return HEAD_DAMAGE;
        }
    }
    for &limb in &bones.limbs {
        if let Some(pos) = bone_position(limb) {
            if hit_point.distance(pos) <= LIMB_HITBOX_RADIUS {
                return LIMB_DAMAGE;
            }
        }
    }
    if let Some(pos) = bones.chest.and_then(&bone_position) {
        if hit_point.distance(pos) <= CHEST_HITBOX_RADIUS {
            return CHEST_DAMAGE;
        }
    }
    if let Some(pos) = bones.belly.and_then(&bone_position) {
        if hit_point.distance(pos) <= BELLY_HITBOX_RADIUS {
            return BELLY_DAMAGE;
        }
    }
    zombie_hit_damage_geometric(hit_point, zombie_transform)
}

// TEMPORARY (HITBOX_PLAN.md Phase 4 verification) — sanity-checks
// zombie_hit_damage_bones' priority order and fallback in isolation,
// without needing a live client/bot to actually fire a shot. Removed
// once confirmed.
#[cfg(test)]
mod hitbox_phase4_tests {
    use super::*;

    fn bones_at(head: Vec3) -> (ZombieHitboxBones, std::collections::HashMap<Entity, Vec3>) {
        let head_entity = Entity::from_raw_u32(1).unwrap();
        let chest_entity = Entity::from_raw_u32(2).unwrap();
        let belly_entity = Entity::from_raw_u32(3).unwrap();
        let limb_entity = Entity::from_raw_u32(4).unwrap();
        let bones = ZombieHitboxBones {
            head: Some(head_entity),
            chest: Some(chest_entity),
            belly: Some(belly_entity),
            limbs: vec![limb_entity],
        };
        let mut positions = std::collections::HashMap::new();
        positions.insert(head_entity, head);
        positions.insert(chest_entity, head - Vec3::Y * 0.6);
        positions.insert(belly_entity, head - Vec3::Y * 1.1);
        positions.insert(limb_entity, head + Vec3::X * 0.6);
        (bones, positions)
    }

    #[test]
    fn hit_exactly_on_head_bone_is_a_headshot() {
        let root = Transform::from_xyz(0.0, 1.0, 0.0);
        let (bones, positions) = bones_at(Vec3::new(0.0, 1.9, 0.0));
        let head_pos = positions[&bones.head.unwrap()];
        let damage = zombie_hit_damage_bones(head_pos, &root, &bones, |e| positions.get(&e).copied());
        assert_eq!(damage, HEAD_DAMAGE);
    }

    #[test]
    fn hit_near_limb_bone_is_a_limb_hit_even_though_closer_to_head_height() {
        let root = Transform::from_xyz(0.0, 1.0, 0.0);
        let (bones, positions) = bones_at(Vec3::new(0.0, 1.9, 0.0));
        let limb_pos = positions[&bones.limbs[0]];
        // Same world Y as the head, but far enough laterally that only the
        // limb sphere (not the head sphere) should contain it — this is
        // exactly the case the old height-band guess handled via a
        // separate lateral-distance check; bone-based detection gets it
        // for free from the limb bone's own real position.
        let damage = zombie_hit_damage_bones(limb_pos, &root, &bones, |e| positions.get(&e).copied());
        assert_eq!(damage, LIMB_DAMAGE);
    }

    #[test]
    fn hit_near_chest_bone_is_a_chest_hit() {
        let root = Transform::from_xyz(0.0, 1.0, 0.0);
        let (bones, positions) = bones_at(Vec3::new(0.0, 1.9, 0.0));
        let chest_pos = positions[&bones.chest.unwrap()];
        let damage = zombie_hit_damage_bones(chest_pos, &root, &bones, |e| positions.get(&e).copied());
        assert_eq!(damage, CHEST_DAMAGE);
    }

    #[test]
    fn unresolved_bones_fall_back_to_geometric_guess_instead_of_a_silent_miss() {
        let root = Transform::from_xyz(0.0, 1.0, 0.0);
        let empty_bones = ZombieHitboxBones::default();
        let hit_point = Vec3::new(0.0, 1.9, 0.0);
        let damage = zombie_hit_damage_bones(hit_point, &root, &empty_bones, |_| None);
        // No bone resolved at all (e.g. scene still loading) — must fall
        // back to the geometric guess rather than silently scoring a miss.
        assert_eq!(damage, zombie_hit_damage_geometric(hit_point, &root));
    }

    #[test]
    fn far_from_every_bone_falls_back_to_geometric_guess() {
        let root = Transform::from_xyz(0.0, 1.0, 0.0);
        let (bones, positions) = bones_at(Vec3::new(0.0, 1.9, 0.0));
        // Nowhere near any bone's sphere — confirms the fallback still
        // fires (rather than returning 0 damage) when bone spheres all
        // miss, same as the empty-bones case above but with bones that
        // resolved, just not close enough to this particular hit point.
        let hit_point = Vec3::new(5.0, 1.0, 0.0);
        let damage = zombie_hit_damage_bones(hit_point, &root, &bones, |e| positions.get(&e).copied());
        assert_eq!(damage, zombie_hit_damage_geometric(hit_point, &root));
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

// Broadcast by the server (ServerEventAppExt, SendMode::Broadcast) after
// resolving a FireShot, so every client — not just the shooter — plays the
// same hitmarker/blood/spark/floating-damage VFX at the same place.
#[derive(Event, Clone, Copy, Serialize, Deserialize)]
pub enum ShotFx {
    ZombieHit { point: Vec3, normal: Vec3, damage: f32, killed: bool },
    SurfaceHit { point: Vec3, normal: Vec3 },
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

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
// Reuses the player's capsule dimensions (roughly human-sized).
pub const ZOMBIE_RADIUS: f32 = PLAYER_RADIUS;
pub const ZOMBIE_LENGTH: f32 = PLAYER_LENGTH;
pub const ZOMBIE_PATH_RECALC_INTERVAL: f32 = 0.8;
pub const ZOMBIE_STOP_DISTANCE: f32 = 1.2;
pub const ZOMBIE_LOD_DISTANCE: f32 = 15.0;
pub const ZOMBIE_LOD_REST_HEIGHT: f32 = 0.9;
pub const ZOMBIE_AVOID_PROBE_DISTANCE: f32 = 2.5;
pub const ZOMBIE_AVOID_ANGLES_DEG: [f32; 6] = [25.0, -25.0, 50.0, -50.0, 80.0, -80.0];
pub const ZOMBIE_SEPARATION_RADIUS: f32 = 1.1;
pub const ZOMBIE_SEPARATION_STRENGTH: f32 = 1.2;
pub const KNOCKBACK_DECAY_RATE: f32 = 6.0;

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
// geometry.
pub const ZOMBIE_SPAWNS: [Vec3; 100] = zombie_spawn_grid(ARENA_HALF_SIZE - 3.0, ARENA_HALF_SIZE - 3.0, 0.9);

// Sent by the client's "spawn 100 more" debug button — spawning must be a
// server decision now, since zombies are server-authoritative.
#[derive(Event, Clone, Copy, Serialize, Deserialize, Default)]
pub struct SpawnMoreZombies;

// --- Shooting/damage (server-authoritative, see crates/server) ---

// A dead zombie is replaced after this long, keeping the population topped
// up instead of thinning out.
pub const ZOMBIE_RESPAWN_DELAY: f32 = 1.0;
// Horizontal-only impulse applied to a zombie that survives a hit, along
// the bullet's own travel direction.
pub const KNOCKBACK_FORCE: f32 = 3.5;
// Same distance avian3d's physics raycast in shoot() used to go — far past
// anything reachable inside the arena.
pub const MAX_SHOT_DISTANCE: f32 = 200.0;

// Body-shot kills still take this many hits; a headshot always kills in one
// (see zombie_hit_damage_geometric).
pub const ZOMBIE_MAX_HEALTH: f32 = 8.0;

// Per-region damage, picked by zombie_hit_damage_geometric from where the
// raycast hit lands relative to the zombie's root. The zombie.glb rig has
// no separate per-body-part meshes to hit-test against directly (single
// "Zombie_Cylinder" mesh) — bones exist for skinning/animation only, not as
// distinct raycast targets — so regions are approximated geometrically from
// the hit point instead of a real hitbox lookup. This used to have a
// nearest-bone-preferred refinement client-side, but the server (which now
// decides damage authoritatively) has no skeleton/bone data at all — this
// geometric approximation, previously just a fallback, is now the only
// method.
pub const HEAD_DAMAGE: f32 = ZOMBIE_MAX_HEALTH; // always a one-shot kill
pub const CHEST_DAMAGE: f32 = 1.5;
pub const BELLY_DAMAGE: f32 = 1.0;
pub const LIMB_DAMAGE: f32 = 0.5;

// Height bands (world-space meters, relative to the zombie's root, which
// sits around the physics capsule's center — roughly hip/waist height, not
// the feet) and a lateral distance from the vertical axis through the root,
// used to tell an out-flung arm from the torso at similar heights. Rough
// estimates for a ~2m-tall zombie — not visually calibrated.
pub const HITBOX_HEAD_MIN_Y: f32 = 0.9;
pub const HITBOX_CHEST_MIN_Y: f32 = 0.3;
pub const HITBOX_BELLY_MIN_Y: f32 = -0.2;
pub const HITBOX_LIMB_LATERAL_MIN: f32 = 0.35;

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

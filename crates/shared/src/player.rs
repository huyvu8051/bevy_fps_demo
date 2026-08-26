// --- Player (server-authoritative physics/state, see crates/server) ---

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::arena::ARENA_HALF_SIZE;

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

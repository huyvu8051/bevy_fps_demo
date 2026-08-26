// --- Shooting/damage events (server-authoritative, see crates/server) ---

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

// Same distance avian3d's physics raycast in shoot() used to go — far past
// anything reachable inside the arena.
pub const MAX_SHOT_DISTANCE: f32 = 200.0;

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

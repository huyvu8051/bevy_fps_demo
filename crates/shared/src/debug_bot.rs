// --- Debug bots (client debug panel, see crates/client) ---
// Fake NetPlayer stand-ins so multi-player zombie targeting can be tested
// without launching a second real client. Identified by a small server-
// assigned id rather than Entity, since client->server events don't need
// cross-network entity mapping this way.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

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

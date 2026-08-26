// --- Game state (server-authoritative, see crates/server) ---

use bevy::prelude::Event;
use serde::{Deserialize, Serialize};

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

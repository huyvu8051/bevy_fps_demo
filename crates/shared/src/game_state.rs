
use bevy::prelude::Event;
use serde::{Deserialize, Serialize};

#[derive(Event, Clone, Copy, Serialize, Deserialize, Default)]
pub struct PlayerDied;

#[derive(Event, Clone, Copy, Serialize, Deserialize, Default)]
pub struct RequestRestart;

#[derive(Event, Clone, Copy, Serialize, Deserialize)]
pub enum GameStateEvent {
    GameOver,
    Restarted,
}

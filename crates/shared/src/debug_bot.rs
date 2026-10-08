
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

pub const DEBUG_BOT_DETECT_RANGE: f32 = 40.0;
pub const DEBUG_BOT_ENGAGE_RANGE: f32 = 10.0;
pub const DEBUG_BOT_RETARGET_INTERVAL: f32 = 1.5;
pub const DEBUG_BOT_FIRE_INTERVAL_SECS: f32 = 0.4;

pub const DEBUG_BOT_EYE_HEIGHT: f32 = 0.65;

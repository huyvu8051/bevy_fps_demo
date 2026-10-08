
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

pub const MAX_SHOT_DISTANCE: f32 = 200.0;

#[derive(Event, Clone, Copy, Serialize, Deserialize)]
pub struct FireShot {
    pub origin: Vec3,
    pub direction: Vec3,
}

#[derive(Event, Clone, Copy, Serialize, Deserialize)]
pub struct PlayerFiredFx {
    pub shooter_network_id: u64,
}

#[derive(Event, Clone, Copy, Serialize, Deserialize)]
pub struct PlayerDiedFx {
    pub who_network_id: u64,
}

#[derive(Event, Clone, Copy, Serialize, Deserialize)]
pub enum ShotFx {
    ZombieHit { origin: Vec3, point: Vec3, normal: Vec3, damage: f32, killed: bool },
    SurfaceHit { origin: Vec3, point: Vec3, normal: Vec3 },
}

#[derive(Event, Clone, Copy, Serialize, Deserialize)]
pub struct ShotConfirmed {
    pub point: Vec3,
    pub normal: Vec3,
    pub damage: f32,
    pub killed: bool,
}

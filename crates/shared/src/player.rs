
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Component)]
pub struct Player {
    pub vertical_velocity: f32,
    pub grounded: bool,
}

#[derive(Component, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlayerStance {
    #[default]
    Standing,
    Crouching,
    Prone,
}

#[derive(Component, Clone, Copy, Serialize, Deserialize)]
pub struct Health(pub f32);

#[derive(Resource, Default)]
pub struct GameOver(pub bool);

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
pub const MAX_WALKABLE_SLOPE_DEGREES: f32 = 60.0;

pub fn ground_normal_min_y() -> f32 {
    MAX_WALKABLE_SLOPE_DEGREES.to_radians().cos()
}

pub const NOCLIP_SPEED: f32 = MOVE_SPEED * 3.0;

#[derive(Event, Component, Clone, Copy, Serialize, Deserialize, Default)]
pub struct ClientInput {

    pub sequence: u32,
    pub move_axis: Vec2,
    pub yaw: f32,
    pub jump: bool,
    pub sprint: bool,
    pub aiming: bool,
    pub stance: PlayerStance,

    pub noclip: bool,
}

#[derive(Component, Clone, Copy, Serialize, Deserialize, Default)]
pub struct NetTransform {
    pub translation: Vec3,
    pub yaw: f32,

    pub moving: bool,

    pub sprinting: bool,
}

#[derive(Component, Clone, Copy, Default, Serialize, Deserialize)]
#[require(Transform)]
pub struct NetPlayer;

#[derive(Component, Clone, Copy, Default, Serialize, Deserialize)]
pub struct LastProcessedInput(pub u32);

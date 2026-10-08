
use bevy::prelude::*;

use crate::player::PLAYER_SPAWN;

pub const SERVER_PORT_BASE: u16 = 5031;

pub const PROTOCOL_ID: u64 = 0;

#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Scenario {
    #[default]
    StressTest,
}

impl Scenario {
    pub fn from_map_id(id: u8) -> Self {
        match id {
            2 => Scenario::StressTest,
            _ => Scenario::default(),
        }
    }

    pub fn map_id(self) -> u8 {
        match self {
            Scenario::StressTest => 2,
        }
    }

    pub fn port(self) -> u16 {
        SERVER_PORT_BASE + self.map_id() as u16
    }

    pub fn map_kind(self) -> MapKind {
        match self {
            Scenario::StressTest => MapKind::Arena,
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Scenario::StressTest => "ARENA",
        }
    }

    pub fn subtitle(self) -> &'static str {
        match self {
            Scenario::StressTest => "OPEN FLAT ARENA, CHASE-AI ZOMBIES",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MapKind {
    Arena,
}

pub fn scenario_player_spawn(scenario: Scenario) -> Vec3 {
    match scenario {
        Scenario::StressTest => STRESS_TEST_PLAYER_SPAWN,
    }
}

pub const STRESS_TEST_ZOMBIE_COUNT: usize = 10;

pub const STRESS_TEST_PLAYER_SPAWN: Vec3 = PLAYER_SPAWN;

pub const INTERP_DELAY_SECS: f32 = 0.1;

pub const LAG_COMPENSATION_MAX_SECS: f32 = 1.0;

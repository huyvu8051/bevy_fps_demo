
use bevy::prelude::*;

use crate::player::PLAYER_SPAWN;

pub const SERVER_PORT_BASE: u16 = 5031;

pub const PROTOCOL_ID: u64 = 0;

#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Scenario {
    Normal,

    #[default]
    StressTest,

    ItemGallery,
}

impl Scenario {
    pub fn from_map_id(id: u8) -> Self {
        match id {
            1 => Scenario::Normal,
            2 => Scenario::StressTest,
            3 => Scenario::ItemGallery,
            _ => Scenario::default(),
        }
    }

    pub fn map_id(self) -> u8 {
        match self {
            Scenario::Normal => 1,
            Scenario::StressTest => 2,
            Scenario::ItemGallery => 3,
        }
    }

    pub fn port(self) -> u16 {
        SERVER_PORT_BASE + self.map_id() as u16
    }

    pub fn map_kind(self) -> MapKind {
        match self {
            Scenario::ItemGallery => MapKind::ItemGallery,
            Scenario::StressTest => MapKind::Arena,
            Scenario::Normal => MapKind::Campaign(1),
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Scenario::Normal => "GAME MAP",
            Scenario::StressTest => "ARENA",
            Scenario::ItemGallery => "ITEM GALLERY",
        }
    }

    pub fn subtitle(self) -> &'static str {
        match self {
            Scenario::Normal => "OFFICE BUILDING (B1-F3)",
            Scenario::StressTest => "OPEN FLAT ARENA, CHASE-AI ZOMBIES",
            Scenario::ItemGallery => "ALL STANDARD ITEM SIZES ON FLAT FLOOR",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MapKind {

    Campaign(u8),

    Arena,

    ItemGallery,
}

pub fn scenario_player_spawn(scenario: Scenario) -> Vec3 {
    match scenario {
        Scenario::Normal => crate::map1::map1_player_spawn(),
        Scenario::StressTest => STRESS_TEST_PLAYER_SPAWN,
        Scenario::ItemGallery => crate::item_gallery::item_gallery_player_spawn(),
    }
}

pub const STRESS_TEST_ZOMBIE_COUNT: usize = 10;

pub const STRESS_TEST_PLAYER_SPAWN: Vec3 = PLAYER_SPAWN;

pub const INTERP_DELAY_SECS: f32 = 0.1;

pub const LAG_COMPENSATION_MAX_SECS: f32 = 1.0;

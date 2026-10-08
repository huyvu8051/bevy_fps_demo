
use bevy::prelude::*;

use crate::player::PLAYER_SPAWN;

pub const SERVER_PORT_BASE: u16 = 5031;

pub const PROTOCOL_ID: u64 = 0;

#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Scenario {
    Desert,

    #[default]
    StressTest,
}

impl Scenario {
    pub fn from_map_id(id: u8) -> Self {
        match id {
            1 => Scenario::Desert,
            2 => Scenario::StressTest,
            _ => Scenario::default(),
        }
    }

    pub fn map_id(self) -> u8 {
        match self {
            Scenario::Desert => 1,
            Scenario::StressTest => 2,
        }
    }

    pub fn port(self) -> u16 {
        SERVER_PORT_BASE + self.map_id() as u16
    }

    pub fn map_kind(self) -> MapKind {
        match self {
            Scenario::Desert => MapKind::Desert,
            Scenario::StressTest => MapKind::Arena,
        }
    }

    /// Navmeshes are baked offline by the `navmesh_baker` binary and shipped
    /// as assets, so a town-sized map never has to voxelize at startup.
    pub fn baked_navmesh_path(self) -> &'static str {
        match self {
            Scenario::Desert => crate::desert::DESERT_NAVMESH_PATH,
            Scenario::StressTest => ARENA_NAVMESH_PATH,
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Scenario::Desert => "DESERT CITY",
            Scenario::StressTest => "ARENA",
        }
    }

    pub fn subtitle(self) -> &'static str {
        match self {
            Scenario::Desert => "OPEN DESERT TOWN, BAKED NAVMESH",
            Scenario::StressTest => "OPEN FLAT ARENA, CHASE-AI ZOMBIES",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MapKind {
    Desert,
    Arena,
}

pub fn scenario_player_spawn(scenario: Scenario) -> Vec3 {
    match scenario {
        Scenario::Desert => crate::desert::DESERT_PLAYER_SPAWN,
        Scenario::StressTest => STRESS_TEST_PLAYER_SPAWN,
    }
}

pub const ARENA_NAVMESH_PATH: &str = "maps/arena/navmesh.nav";

pub const STRESS_TEST_ZOMBIE_COUNT: usize = 10;

pub const STRESS_TEST_PLAYER_SPAWN: Vec3 = PLAYER_SPAWN;

pub const INTERP_DELAY_SECS: f32 = 0.1;

pub const LAG_COMPENSATION_MAX_SECS: f32 = 1.0;

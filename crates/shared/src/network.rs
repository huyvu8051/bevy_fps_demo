// --- Networking (client <-> server) ---
// Ports, protocol id, and the map-select Scenario enum — both sides need
// to agree on these so a client actually connects to the right server
// process and process/interpret its replicated state consistently.

use bevy::prelude::*;

use crate::player::PLAYER_SPAWN;

// --- Test scenarios (map-select menu, iOS icon-tap) -----------------------
//
// Each scenario is its own dedicated-server process on its own port —
// bevy_replicon has no "rooms" concept, so this is the simplest way to get
// genuinely separate worlds (map = port = room; connecting to the same
// port means seeing each other, same as normal play today). The client
// picks the port from the scenario it selected (see MAP_ID in
// crates/client/src/lib.rs); the server picks both its bind port and its
// spawn preset from the same MAP_ID env var it's started with. Kept in
// `shared` so client and server can't drift on the port <-> scenario
// mapping.
pub const SERVER_PORT_BASE: u16 = 5031;
pub const WS_PORT_OFFSET: u16 = 1000;

// Was two separately-declared `const PROTOCOL_ID: u64 = 0;` (one in each of
// crates/client and crates/server) — renet2 rejects a connection whose
// client and server protocol_id don't match, so a drift here fails loudly
// (connection refused) rather than silently, but there's still no reason to
// keep it as two copies that could disagree.
pub const PROTOCOL_ID: u64 = 0;

/// Which map-select preset a server process (and the client connecting to
/// it) is running. `Normal` is today's actual game — everything else is a
/// fixed test rig, not part of normal play.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Scenario {
    #[default]
    Normal,
    /// Map 1: 5 zombies stationary in a row (no AI — they never chase),
    /// player spawned facing them, for testing hit registration in
    /// isolation. A killed zombie respawns at its own row slot after
    /// HITBOX_TEST_RESPAWN_DELAY, not the normal cycling-zombie-grid
    /// behavior.
    HitboxTest,
    /// Map 2: the open flat arena (MapKind::Arena — same geometry as
    /// LegacyArena) with STRESS_TEST_ZOMBIE_COUNT chase-AI zombies. Plain
    /// walkable combat — noclip off, ground-level spawn — kept as its own
    /// map-select entry / dedicated port.
    StressTest,
    /// Map 3: 3 stationary zombies, each parked in a different ZombiePose
    /// (Idle/Crawl/Bite — see RENDER_COMPARE_ZOMBIE_POSITIONS' own doc
    /// comment for how each pose is forced without AI movement), player
    /// spawned facing all three and not moving (Autopilot auto-disabled
    /// client-side for this scenario). A pure visual-inspection rig for
    /// comparing the
    /// desktop and iOS/Metal renderers side by side — not a combat test,
    /// so unlike HitboxTest these zombies don't respawn once killed.
    RenderCompare,
    /// Map 4: the old flat test arena (floor + pillars, crates/shared's
    /// arena.rs) — normal chase-AI zombie population, same as `Normal`,
    /// just on MapKind::Arena instead of the Map 1 office building. Kept
    /// around now that `Normal` means the real Map 1 building, so the old
    /// arena is still reachable for quick combat/AI testing without the
    /// building's geometry involved.
    LegacyArena,
    /// Map 5: flat floor with ONE of every standard map-item size laid
    /// out in a grid (cars, desks, cubicles, lockers, reception counter,
    /// doors, stairwell footprint — see `crate::item_gallery` and
    /// `docs/maps/map01_dimensions.md` section 1, its source of truth).
    /// No real room/floor design here, purely a scale-reference rig: open
    /// it in the dev editor (`EDITOR=1`, F2) to eyeball whether each
    /// item's real-world-derived size actually reads right in-engine
    /// before committing it into an actual map floor.
    ItemGallery,
}

impl Scenario {
    pub fn from_map_id(id: u8) -> Self {
        match id {
            1 => Scenario::HitboxTest,
            2 => Scenario::StressTest,
            3 => Scenario::RenderCompare,
            4 => Scenario::LegacyArena,
            5 => Scenario::ItemGallery,
            _ => Scenario::Normal,
        }
    }

    pub fn map_id(self) -> u8 {
        match self {
            Scenario::Normal => 0,
            Scenario::HitboxTest => 1,
            Scenario::StressTest => 2,
            Scenario::RenderCompare => 3,
            Scenario::LegacyArena => 4,
            Scenario::ItemGallery => 5,
        }
    }

    pub fn port(self) -> u16 {
        SERVER_PORT_BASE + self.map_id() as u16
    }

    /// The server's WebSocket listener port for this scenario — a real UDP
    /// socket and a WebSocket (TCP) listener are different address spaces,
    /// so they can't share `port()`'s number even though nothing else
    /// about them conflicts. Native clients (`native_transport`, real UDP)
    /// use `port()`; a wasm32 web client can't open a raw UDP socket at
    /// all (browser sandbox), so it connects here instead via
    /// `ws_client_transport` — see network_setup.rs's wasm
    /// spawn_client_connection and server/main.rs's start_server (the
    /// server runs both sockets at once, socket_id 0 = native, 1 = ws, so
    /// one dedicated server instance serves both native and web clients).
    /// +1000 keeps it visually distinct from the UDP port in logs/configs
    /// without any real collision risk (SERVER_PORT_BASE..+6 is nowhere
    /// near it).
    pub fn ws_port(self) -> u16 {
        self.port() + WS_PORT_OFFSET
    }

    /// The one place that decides which level geometry a scenario runs on —
    /// both the server (colliders) and the client (visuals + local
    /// prediction colliders) match on this instead of the old scattered
    /// `USE_MAP1_B1_GARAGE` / `USE_TEST_ARENA` bools + per-scenario
    /// `!= LegacyArena` special-casing.
    pub fn map_kind(self) -> MapKind {
        match self {
            Scenario::ItemGallery => MapKind::ItemGallery,
            Scenario::LegacyArena | Scenario::StressTest => MapKind::Arena,
            Scenario::Normal | Scenario::HitboxTest | Scenario::RenderCompare => MapKind::Building,
        }
    }
}

/// Which level geometry a `Scenario` runs on — see `Scenario::map_kind`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MapKind {
    /// Map 1 office building (garage / lobby / F2 / F3) — `shared::map1`.
    Building,
    /// Flat walled arena + pillars — `shared::arena` / client's spawn_test_arena.
    Arena,
    /// Flat scale-reference room with the item-size grid — `shared::item_gallery`.
    ItemGallery,
}

/// Player spawn point for a scenario — the server's authoritative spawn
/// (spawn_player / the reset handler) AND the client's first-frame camera
/// position, so the two can't disagree. ItemGallery reads an env override,
/// hence a fn rather than a const table.
pub fn scenario_player_spawn(scenario: Scenario) -> Vec3 {
    match scenario {
        Scenario::Normal => crate::map1::GARAGE_PLAYER_SPAWN,
        Scenario::HitboxTest => HITBOX_TEST_PLAYER_SPAWN,
        Scenario::StressTest => STRESS_TEST_PLAYER_SPAWN,
        Scenario::RenderCompare => RENDER_COMPARE_PLAYER_SPAWN,
        Scenario::LegacyArena => PLAYER_SPAWN,
        Scenario::ItemGallery => crate::item_gallery::item_gallery_player_spawn(),
    }
}

pub const HITBOX_TEST_PLAYER_SPAWN: Vec3 = PLAYER_SPAWN;
pub const HITBOX_TEST_RESPAWN_DELAY: f32 = 2.0;
// A row across the arena's center (z=0, well clear of PLAYER_SPAWN's own
// z=ARENA_HALF_SIZE-4 corner) — same grounded-height convention
// ZOMBIE_SPAWNS itself uses (radius + length/2), computed in crate::zombie
// once ZOMBIE_RADIUS/ZOMBIE_LENGTH are in scope.
pub const HITBOX_TEST_ROW_X: [f32; 5] = [-6.0, -3.0, 0.0, 3.0, 6.0];

pub const STRESS_TEST_ZOMBIE_COUNT: usize = 10;
// Arena centre, ground level — same as LegacyArena's PLAYER_SPAWN (was
// y=15 back when Map 2 auto-enabled noclip and you floated above the horde).
pub const STRESS_TEST_PLAYER_SPAWN: Vec3 = PLAYER_SPAWN;

// Scenario::RenderCompare — set back far enough that the Idle/Crawl
// zombies (RENDER_COMPARE_ZOMBIE_POSITIONS' 1st/3rd entries) both fit in
// view on either side, while the Bite-pose zombie (2nd entry) is offset to
// one side rather than dead-center so its close-range bulk doesn't block
// the other two — see that const's own doc comment for the exact math.
pub const RENDER_COMPARE_PLAYER_SPAWN: Vec3 = Vec3::new(0.0, 3.0, 5.0);

// How far in the past (relative to local received-snapshot time) the
// client renders everyone else's replicated position at — see the client's
// interpolate_net_transform for the full explanation. Also used by the
// server's lag-compensated hit detection (handle_fire_shot): a shooter's
// screen reflects the world from roughly (their network latency +
// this render delay) ago, so that's how far back to rewind zombie
// positions when checking whether their shot looked like a hit to them.
pub const INTERP_DELAY_SECS: f32 = 0.1;
// Upper bound on how far handle_fire_shot will rewind zombie positions for
// lag compensation, regardless of a shooter's actual measured RTT — caps
// how far back a laggy/malicious client could shift hit detection in its
// favor. 1.0s matches CS:GO's sv_maxunlag; ZombieHistory is pruned to this
// window, so bumping it just costs ~1s * tick-rate entries per zombie.
pub const LAG_COMPENSATION_MAX_SECS: f32 = 1.0;

//! Phase 6 follow-up: the real game's settings menu is a `bevy_ui` Node
//! hierarchy (`spawn_settings_menu` in `crate::lib`), and our custom Metal
//! renderer never draws `bevy_ui` content at all (`native_metal_probe.rs`
//! only issues `Mesh3d`/`MeshMaterial3d` draw calls) — confirmed via a
//! temporary self-test that `SettingsMenuVisible` itself flips correctly on
//! F1/MENU-button press, so this is a missing-renderer-feature, not a logic
//! bug. Per the plan's own Phase 4 guidance ("recommend not reimplementing
//! bevy_ui... drive native UIKit views instead"), the fix is a native Swift
//! settings menu overlay, not a bevy_ui-in-Metal renderer. This module is
//! the read/write seam: plain `SystemState` queries against the real
//! headless `World`, exactly like `native_game_extract.rs`'s
//! `extract_render_snapshot`, so the Swift-facing FFI functions in `ffi.rs`
//! stay one-liners.
//!
//! Cheat state (GodMode/Noclip/FreezeZombies/kill-all/spawn-more) is gated
//! behind `#[cfg(debug_assertions)]` here too, mirroring the same gate just
//! added around `spawn_settings_menu`'s cheat buttons in `crate::lib` — a
//! release build's native menu simply won't expose these, matching the
//! bevy_ui version's own (now-gated) behavior instead of reopening the same
//! "cheats reachable in production" gap through a different door.

use bevy::app::App;
use bevy::ecs::system::{Commands, Query, SystemState};
use bevy::ecs::world::FromWorld;
#[cfg(debug_assertions)]
use bevy::transform::components::Transform;
use shared::Health;
#[cfg(debug_assertions)]
use shared::{Zombie, ZOMBIE_MAX_HEALTH};

// Mirrors ffi.rs's own `#[repr(C)]`-free style (no repr(C) structs anywhere
// in this codebase's FFI boundary yet) — instead we expose one getter per
// field, called individually from Swift, same shape as the existing
// `native_metal_probe_*` input functions. Simpler than hand-writing a
// matching Swift struct layout, and each value is cheap enough (a handful
// of SystemState queries per menu-open frame, not per-render-frame) that
// the extra FFI call overhead doesn't matter.

// Ping — same real value the bevy_ui HUD's own PingUi text reads
// (`update_ping_ui` in crate::lib), just via renet2's `RenetClient::rtt()`
// directly instead of a resource, since there's no dedicated ping
// resource to read (the bevy_ui system computes the formatted string
// straight from RenetClient each tick). -1.0 sentinel for "no client yet"
// (matches player_hp's own "-1 = no data" convention below), which
// distinguishes "not connected" from a genuine 0ms reading.
pub fn ping_ms(app: &mut App) -> f32 {
    app.world().get_resource::<bevy_replicon_renet2::renet2::RenetClient>().map(|c| c.rtt() as f32 * 1000.0).unwrap_or(-1.0)
}

pub fn menu_visible(app: &mut App) -> bool {
    app.world().get_resource::<crate::SettingsMenuVisible>().map(|v| v.0).unwrap_or(false)
}

pub fn set_menu_visible(app: &mut App, visible: bool) {
    if let Some(mut v) = app.world_mut().get_resource_mut::<crate::SettingsMenuVisible>() {
        v.0 = visible;
    }
}

pub fn sensitivity(app: &mut App) -> f32 {
    app.world().get_resource::<crate::SensitivityScale>().map(|v| v.0).unwrap_or(1.0)
}

// Same clamp bounds as the real game's own `sensitivity_buttons` system
// (crate::lib) — duplicated here rather than made `pub(crate)` there, since
// this is the one place outside that system doing an unclamped external
// write and the constants are a fixed, unlikely-to-drift pair of numbers.
const SENSITIVITY_SCALE_MIN: f32 = 0.25;
const SENSITIVITY_SCALE_MAX: f32 = 3.0;

pub fn set_sensitivity(app: &mut App, value: f32) {
    if let Some(mut v) = app.world_mut().get_resource_mut::<crate::SensitivityScale>() {
        v.0 = value.clamp(SENSITIVITY_SCALE_MIN, SENSITIVITY_SCALE_MAX);
    }
}

// Player HP for the menu's own display — best-effort: `Query::single()`
// finds nothing before the local player has spawned (e.g. still connecting
// to the server), so this returns -1.0 as a sentinel Swift treats as
// "no data yet" rather than misreporting 0 HP (which would look like the
// player just died).
pub fn player_hp(app: &mut App) -> f32 {
    let mut state: SystemState<Query<&Health, bevy::ecs::query::With<crate::Player>>> =
        SystemState::from_world(app.world_mut());
    let query = state.get(app.world());
    query.single().map(|h| h.0).unwrap_or(-1.0)
}

// Player death — like the settings menu, the real game's own "GAME OVER" /
// "Press R to Restart" overlay (`check_game_over`'s GameOverUi spawn,
// crate::lib) is a bevy_ui Node hierarchy our custom Metal renderer never
// draws, so death currently shows nothing at all on iOS. `GameOver` is a
// plain replicated-but-locally-readable resource (`shared::GameOver`) that
// `check_game_over` already sets the instant local Health hits 0 — no
// FFI-side health check needed, just read the same flag. Not gated behind
// `#[cfg(debug_assertions)]`: unlike the Cheats page, dying and restarting
// is normal gameplay every player needs, in every build.
pub fn game_over(app: &mut App) -> bool {
    app.world().get_resource::<shared::GameOver>().map(|v| v.0).unwrap_or(false)
}

// Same server-authoritative-trigger shape as trigger_kill_all_zombies
// below (`commands.client_trigger(...)`, needs a live server — restart is
// the server's decision, same as every other piece of shared world state;
// see `restart_game`'s own doc comment in crate::lib) — just not gated to
// debug builds, since restarting after death is core gameplay.
pub fn trigger_restart(app: &mut App) {
    use bevy_replicon::prelude::ClientTriggerExt;
    let mut state: SystemState<Commands> = SystemState::from_world(app.world_mut());
    state.get_mut(app.world_mut()).client_trigger(shared::RequestRestart);
    state.apply(app.world_mut());
}

#[cfg(debug_assertions)]
pub fn god_mode(app: &mut App) -> bool {
    app.world().get_resource::<crate::GodMode>().map(|v| v.0).unwrap_or(false)
}

#[cfg(debug_assertions)]
pub fn set_god_mode(app: &mut App, on: bool) {
    if let Some(mut v) = app.world_mut().get_resource_mut::<crate::GodMode>() {
        v.0 = on;
    }
}

#[cfg(debug_assertions)]
pub fn noclip(app: &mut App) -> bool {
    app.world().get_resource::<crate::Noclip>().map(|v| v.0).unwrap_or(false)
}

#[cfg(debug_assertions)]
pub fn set_noclip(app: &mut App, on: bool) {
    if let Some(mut v) = app.world_mut().get_resource_mut::<crate::Noclip>() {
        v.0 = on;
    }
}

#[cfg(debug_assertions)]
pub fn freeze_zombies(app: &mut App) -> bool {
    app.world().get_resource::<crate::FreezeZombies>().map(|v| v.0).unwrap_or(false)
}

#[cfg(debug_assertions)]
pub fn set_freeze_zombies(app: &mut App, on: bool) {
    if let Some(mut v) = app.world_mut().get_resource_mut::<crate::FreezeZombies>() {
        v.0 = on;
    }
}

// The remaining debug-panel toggles (crate::lib's Cheats page 1 second/
// third/fourth rows: Gun/MSAA/Shadow, Hitboxes/AimBlur/PlayerMdl, NoZAnim/
// NoHPBar/FullLOD) are all the exact same "read/write a local bool
// Resource, no networking involved" shape as GodMode/Noclip/FreezeZombies
// above — this macro generates one getter/setter pair per resource instead
// of hand-copying the same 8 lines 9 times. `$default` matches each
// resource's own `impl Default`/`#[derive(Default)]` in crate::lib exactly
// (see that file for which ones default to `true`).
macro_rules! bool_toggle {
    ($get:ident, $set:ident, $resource:ty, $default:expr) => {
        #[cfg(debug_assertions)]
        pub fn $get(app: &mut App) -> bool {
            app.world().get_resource::<$resource>().map(|v| v.0).unwrap_or($default)
        }

        #[cfg(debug_assertions)]
        pub fn $set(app: &mut App, on: bool) {
            if let Some(mut v) = app.world_mut().get_resource_mut::<$resource>() {
                v.0 = on;
            }
        }
    };
}

bool_toggle!(gun_visible, set_gun_visible, crate::GunVisible, true);
bool_toggle!(map_msaa, set_map_msaa, crate::MapMsaaOn, false);
bool_toggle!(map_shadows, set_map_shadows, crate::MapShadowsOn, true);
bool_toggle!(show_hitboxes, set_show_hitboxes, crate::ShowHitboxes, false);
bool_toggle!(aim_blur_enabled, set_aim_blur_enabled, crate::AimBlurEnabled, true);
bool_toggle!(zombie_anim_disabled, set_zombie_anim_disabled, crate::ZombieAnimDisabled, false);
bool_toggle!(zombie_health_bars_hidden, set_zombie_health_bars_hidden, crate::ZombieHealthBarsHidden, false);
bool_toggle!(zombie_lod_disabled, set_zombie_lod_disabled, crate::ZombieLodDisabled, false);
bool_toggle!(player_model_visible, set_player_model_visible, crate::PlayerModelVisible, false);

// Zombie count +/-10 (crate::lib's `zombie_count_buttons`) — like Kill All/
// Spawn +100, `SetZombieCount` is a server-authoritative replicon client
// event (zombie count is server-owned), so this needs the same live server
// connection. `ZombieCountUi` itself is a plain local "client's own best
// guess at the current TARGET" mirror (crate::lib's own comment on that
// resource) — it's what the auto-maintain system aims to keep the
// population at, not a live headcount. Reported as confusing/wrong: it
// never moves when Spawn +100 adds a one-off batch (that's a separate
// action, deliberately not tied to the target), and nothing anywhere
// tracks/replicates the ACTUAL current zombie count to clients. Fixed by
// adding `live_zombie_count` below (a real Query<With<Zombie>> count
// against this client's own already-replicated world — accurate, no new
// replication needed) and showing both in the native menu instead of just
// the target alone.
#[cfg(debug_assertions)]
pub fn zombie_target_count(app: &mut App) -> u32 {
    app.world().get_resource::<crate::ZombieCountUi>().map(|v| v.0).unwrap_or(10)
}

// Real, current count of Zombie entities this client has actually
// replicated from the server right now — unlike zombie_target_count,
// reflects Spawn +100/Kill All/natural deaths immediately, not just
// +/-10 taps.
pub fn live_zombie_count(app: &mut App) -> u32 {
    let mut state: SystemState<Query<(), bevy::ecs::query::With<shared::Zombie>>> = SystemState::from_world(app.world_mut());
    state.get(app.world()).iter().count() as u32
}

// Watch companion app's kill counter (see crate::AUTOPILOT... no, see the
// Apple Watch feature: shows a notification + running kill count on the
// wrist). `crate::KillCount` is incremented by `crate::count_local_kills`
// whenever a zombie this client personally damaged (tracked via
// `crate::RecentlyHitZombies`, populated in `shoot()`'s own local hit
// prediction) disappears from the world shortly after — see that system's
// doc comment for why a bare despawn alone can't be trusted as "I killed
// it" (respawns/disconnects despawn zombies too).
pub fn kill_count(app: &mut App) -> u32 {
    app.world().get_resource::<crate::KillCount>().map(|v| v.0).unwrap_or(0)
}

#[cfg(debug_assertions)]
fn set_zombie_count_and_trigger(app: &mut App, new_count: u32) {
    use bevy_replicon::prelude::ClientTriggerExt;
    if let Some(mut count) = app.world_mut().get_resource_mut::<crate::ZombieCountUi>() {
        count.0 = new_count;
    }
    let mut state: SystemState<Commands> = SystemState::from_world(app.world_mut());
    state.get_mut(app.world_mut()).client_trigger(shared::SetZombieCount(new_count));
    state.apply(app.world_mut());
}

#[cfg(debug_assertions)]
pub fn zombie_count_dec(app: &mut App) {
    let new_count = zombie_target_count(app).saturating_sub(10);
    set_zombie_count_and_trigger(app, new_count);
}

// Same cap as `zombie_count_buttons`'s own `.min(ZOMBIE_SPAWNS.len() as
// u32)` — `ZOMBIE_SPAWNS` is private to crate::lib, so the cap is
// duplicated here as a plain literal (100 preset spawn points, see
// shared::SpawnMoreZombies' own doc comment) rather than exposed just for
// this one read.
#[cfg(debug_assertions)]
const ZOMBIE_SPAWN_PRESET_COUNT: u32 = 100;

#[cfg(debug_assertions)]
pub fn zombie_count_inc(app: &mut App) {
    let new_count = (zombie_target_count(app) + 10).min(ZOMBIE_SPAWN_PRESET_COUNT);
    set_zombie_count_and_trigger(app, new_count);
}

// Heal Nearest / 1HP Nearest (crate::lib's `cheat_zombie_health_buttons` /
// `set_nearest_zombie_health`) — unlike Kill All/Spawn/SetZombieCount,
// there's no server-side event for this at all; the real game's own
// version is already a client-side, cosmetic-only `Health` write (see its
// doc comment at crate::lib ~2988: "Health is server-authoritative/
// replicated... gets overwritten by the next replication update" — a
// pre-existing limitation of the feature itself, not something this FFI
// path adds). Replicated here as the same direct write against the
// nearest-to-local-player `Zombie`.
#[cfg(debug_assertions)]
fn set_nearest_zombie_health(app: &mut App, value: f32) {
    let mut state: SystemState<(Query<&Transform, bevy::ecs::query::With<crate::Player>>, Query<(&Transform, &mut Health), bevy::ecs::query::With<Zombie>>)> =
        SystemState::from_world(app.world_mut());
    let (player, mut zombies) = state.get_mut(app.world_mut());
    let Ok(player_transform) = player.single() else {
        return;
    };
    let from = player_transform.translation;
    let nearest = zombies
        .iter_mut()
        .min_by(|(a, _), (b, _)| a.translation.distance_squared(from).total_cmp(&b.translation.distance_squared(from)));
    if let Some((_, mut health)) = nearest {
        health.0 = value;
    }
}

#[cfg(debug_assertions)]
pub fn heal_nearest_zombie(app: &mut App) {
    set_nearest_zombie_health(app, ZOMBIE_MAX_HEALTH);
}

#[cfg(debug_assertions)]
pub fn near_death_nearest_zombie(app: &mut App) {
    set_nearest_zombie_health(app, 1.0);
}

// Kill All / Spawn +100 are server-authoritative replicon client events
// (`shared::ClearAllZombies`/`shared::SpawnMoreZombies`, see their own doc
// comments — zombies are server-owned so a purely client-side despawn
// would just reappear on the next replication update). Triggering them
// here works exactly like the real game's own cheat buttons
// (`cheat_kill_all_button`/`cheat_zombie_health_buttons` in crate::lib) —
// both go through `commands.client_trigger(...)`, which needs the same
// live server connection normal play already requires; nothing about
// running headless changes that, since `RepliconPlugins`/
// `RepliconRenetPlugins` are part of the headless plugin set unchanged.
#[cfg(debug_assertions)]
pub fn trigger_kill_all_zombies(app: &mut App) {
    use bevy_replicon::prelude::ClientTriggerExt;
    let mut state: SystemState<Commands> = SystemState::from_world(app.world_mut());
    state.get_mut(app.world_mut()).client_trigger(shared::ClearAllZombies);
    state.apply(app.world_mut());
}

#[cfg(debug_assertions)]
pub fn trigger_spawn_more_zombies(app: &mut App) {
    use bevy_replicon::prelude::ClientTriggerExt;
    let mut state: SystemState<Commands> = SystemState::from_world(app.world_mut());
    state.get_mut(app.world_mut()).client_trigger(shared::SpawnMoreZombies);
    state.apply(app.world_mut());
}

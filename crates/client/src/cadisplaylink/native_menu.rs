
use bevy::app::App;
use bevy::ecs::system::{Commands, Query, SystemState};
use bevy::ecs::world::FromWorld;
#[cfg(debug_assertions)]
use bevy::transform::components::Transform;
use shared::Health;
#[cfg(debug_assertions)]
use shared::{Zombie, ZOMBIE_MAX_HEALTH};

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

pub fn fps_cap(app: &mut App) -> f32 {
    app.world().get_resource::<crate::FpsCap>().map(|c| c.0 as f32).unwrap_or(120.0)
}

const SENSITIVITY_SCALE_MIN: f32 = 0.25;
const SENSITIVITY_SCALE_MAX: f32 = 3.0;

pub fn set_sensitivity(app: &mut App, value: f32) {
    if let Some(mut v) = app.world_mut().get_resource_mut::<crate::SensitivityScale>() {
        v.0 = value.clamp(SENSITIVITY_SCALE_MIN, SENSITIVITY_SCALE_MAX);
    }
}

pub fn player_hp(app: &mut App) -> f32 {
    let mut state: SystemState<Query<&Health, bevy::ecs::query::With<crate::Player>>> =
        SystemState::from_world(app.world_mut());
    let query = state.get(app.world()).expect("system param validation failed");
    query.single().map(|h| h.0).unwrap_or(-1.0)
}

pub fn game_over(app: &mut App) -> bool {
    app.world().get_resource::<shared::GameOver>().map(|v| v.0).unwrap_or(false)
}

pub fn trigger_restart(app: &mut App) {
    use bevy_replicon::prelude::ClientTriggerExt;
    let mut state: SystemState<Commands> = SystemState::from_world(app.world_mut());
    state.get_mut(app.world_mut()).expect("system param validation failed").client_trigger(shared::RequestRestart);
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

pub fn autopilot(app: &mut App) -> bool {
    app.world().get_resource::<crate::autopilot::Autopilot>().map(|v| v.0).unwrap_or(true)
}

pub fn set_autopilot(app: &mut App, on: bool) {
    if let Some(mut v) = app.world_mut().get_resource_mut::<crate::autopilot::Autopilot>() {
        v.0 = on;
    }
}

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
bool_toggle!(show_hitboxes, set_show_hitboxes, crate::ShowHitboxes, false);
bool_toggle!(aim_blur_enabled, set_aim_blur_enabled, crate::AimBlurEnabled, true);
bool_toggle!(zombie_anim_disabled, set_zombie_anim_disabled, crate::ZombieAnimDisabled, false);
bool_toggle!(zombie_health_bars_hidden, set_zombie_health_bars_hidden, crate::ZombieHealthBarsHidden, false);
bool_toggle!(player_model_visible, set_player_model_visible, crate::PlayerModelVisible, false);
bool_toggle!(show_autopilot_target, set_show_autopilot_target, crate::ShowAutopilotTarget, false);
bool_toggle!(show_hit_markers, set_show_hit_markers, crate::ShowHitMarkers, true);

#[cfg(debug_assertions)]
pub fn zombie_target_count(app: &mut App) -> u32 {
    app.world().get_resource::<crate::ZombieCountUi>().map(|v| v.0).unwrap_or(10)
}

pub fn live_zombie_count(app: &mut App) -> u32 {
    let mut state: SystemState<Query<(), bevy::ecs::query::With<shared::Zombie>>> = SystemState::from_world(app.world_mut());
    state.get(app.world()).expect("system param validation failed").iter().count() as u32
}

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
    state.get_mut(app.world_mut()).expect("system param validation failed").client_trigger(shared::SetZombieCount(new_count));
    state.apply(app.world_mut());
}

#[cfg(debug_assertions)]
pub fn zombie_count_dec(app: &mut App) {
    let new_count = zombie_target_count(app).saturating_sub(10);
    set_zombie_count_and_trigger(app, new_count);
}

#[cfg(debug_assertions)]
const ZOMBIE_SPAWN_PRESET_COUNT: u32 = 100;

#[cfg(debug_assertions)]
pub fn zombie_count_inc(app: &mut App) {
    let new_count = (zombie_target_count(app) + 10).min(ZOMBIE_SPAWN_PRESET_COUNT);
    set_zombie_count_and_trigger(app, new_count);
}

#[cfg(debug_assertions)]
fn set_nearest_zombie_health(app: &mut App, value: f32) {
    let mut state: SystemState<(Query<&Transform, bevy::ecs::query::With<crate::Player>>, Query<(&Transform, &mut Health), bevy::ecs::query::With<Zombie>>)> =
        SystemState::from_world(app.world_mut());
    let (player, mut zombies) = state.get_mut(app.world_mut()).expect("system param validation failed");
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

#[cfg(debug_assertions)]
pub fn trigger_kill_all_zombies(app: &mut App) {
    use bevy_replicon::prelude::ClientTriggerExt;
    let mut state: SystemState<Commands> = SystemState::from_world(app.world_mut());
    state.get_mut(app.world_mut()).expect("system param validation failed").client_trigger(shared::ClearAllZombies);
    state.apply(app.world_mut());
}

#[cfg(debug_assertions)]
pub fn trigger_spawn_more_zombies(app: &mut App) {
    use bevy_replicon::prelude::ClientTriggerExt;
    let mut state: SystemState<Commands> = SystemState::from_world(app.world_mut());
    state.get_mut(app.world_mut()).expect("system param validation failed").client_trigger(shared::SpawnMoreZombies);
    state.apply(app.world_mut());
}

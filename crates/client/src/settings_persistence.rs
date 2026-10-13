
use bevy::prelude::*;
use crate::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default, Clone)]
#[serde(default)]
pub(crate) struct PersistedSettings {
    pub(crate) light_quality: Option<u8>,
    pub(crate) shadow_quality: Option<u8>,
    pub(crate) point_light_quality: Option<u8>,
    pub(crate) render_resolution_scale: Option<f32>,
    pub(crate) fps_cap: Option<u32>,
    pub(crate) map_msaa_on: Option<bool>,
    pub(crate) scope_camera_enabled: Option<bool>,
    pub(crate) sensitivity_scale: Option<f32>,
    pub(crate) noclip: Option<bool>,
    pub(crate) god_mode: Option<bool>,
    pub(crate) freeze_zombies: Option<bool>,
    pub(crate) show_hitboxes: Option<bool>,
    pub(crate) aim_blur_enabled: Option<bool>,
    pub(crate) zombie_anim_disabled: Option<bool>,
    pub(crate) zombie_health_bars_hidden: Option<bool>,
    pub(crate) player_model_visible: Option<bool>,
    pub(crate) gun_visible: Option<bool>,
    pub(crate) aim_dof_focal_distance_alt: Option<bool>,
    pub(crate) autopilot_look_only: Option<bool>,
    pub(crate) show_autopilot_target: Option<bool>,
    pub(crate) show_hit_markers: Option<bool>,
    pub(crate) show_floating_damage: Option<bool>,
    pub(crate) show_bullet_tracers: Option<bool>,
    pub(crate) autopilot: Option<bool>,
    pub(crate) inspect_mode: Option<bool>,
    pub(crate) nav_debug_visible: Option<bool>,
}

#[cfg(target_os = "ios")]
fn ios_home_directory() -> Option<String> {
    use objc::{runtime::Object, *};
    unsafe extern "C" {
        fn NSHomeDirectory() -> *mut Object;
    }
    unsafe {
        let path = NSHomeDirectory();
        if path.is_null() {
            return None;
        }
        let utf8: *const std::os::raw::c_char = msg_send![path, UTF8String];
        if utf8.is_null() {
            return None;
        }
        Some(std::ffi::CStr::from_ptr(utf8).to_string_lossy().into_owned())
    }
}

fn settings_file_path() -> std::path::PathBuf {

    #[cfg(target_os = "ios")]
    {
        let home = ios_home_directory().unwrap_or_else(|| ".".to_string());
        std::path::Path::new(&home).join("Library/Application Support/bevy_fps_demo_settings.ron")
    }
    #[cfg(not(target_os = "ios"))]
    {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        std::path::Path::new(&home).join(".bevy_fps_demo_settings.ron")
    }
}

fn read_settings_text() -> Option<String> {
    std::fs::read_to_string(settings_file_path()).ok()
}

fn write_settings_text(text: &str) {
    let path = settings_file_path();
    if let Some(parent) = path.parent() {

        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::write(&path, text) {
        Ok(()) => info!("[settings] saved to {path:?}"),
        Err(e) => warn!("[settings] failed to save to {path:?}: {e}"),
    }
}

fn delete_settings_text() {
    let _ = std::fs::remove_file(settings_file_path());
}

fn load_ship_defaults() -> PersistedSettings {
    #[cfg(target_os = "ios")]
    let text = shared::read_ron_data_file!("default_settings_mobile.ron");
    #[cfg(not(target_os = "ios"))]
    let text = shared::read_ron_data_file!("default_settings_desktop.ron");
    ron::from_str(&text).unwrap_or_else(|e| {
        warn!("failed to parse bundled default settings, falling back to compiled-in Default: {e}");
        PersistedSettings::default()
    })
}

fn reset_config_env_requested() -> bool {
    std::env::var("RESET_CONFIG").is_ok_and(|v| v == "1" || v.to_lowercase() == "true")
}

fn apply_settings_to_world(world: &mut World, settings: &PersistedSettings) {
    macro_rules! apply {

        ($field:ident, $resource:ty) => {
            if let Some(v) = settings.$field
                && let Some(mut r) = world.get_resource_mut::<$resource>()
            {
                r.0 = v;
            }
        };
    }
    apply!(light_quality, LightQuality);
    apply!(shadow_quality, ShadowQuality);
    apply!(point_light_quality, PointLightQuality);
    apply!(render_resolution_scale, RenderResolutionScale);
    apply!(fps_cap, FpsCap);
    apply!(map_msaa_on, MapMsaaOn);
    apply!(scope_camera_enabled, ScopeCameraEnabled);
    apply!(sensitivity_scale, SensitivityScale);
    apply!(noclip, Noclip);
    apply!(god_mode, GodMode);
    apply!(freeze_zombies, FreezeZombies);
    apply!(show_hitboxes, ShowHitboxes);
    apply!(aim_blur_enabled, AimBlurEnabled);
    apply!(zombie_anim_disabled, ZombieAnimDisabled);
    apply!(zombie_health_bars_hidden, ZombieHealthBarsHidden);
    apply!(player_model_visible, PlayerModelVisible);
    apply!(gun_visible, GunVisible);
    apply!(aim_dof_focal_distance_alt, AimDofFocalDistanceAlt);
    apply!(autopilot_look_only, AutopilotLookOnly);
    apply!(show_autopilot_target, ShowAutopilotTarget);
    apply!(show_hit_markers, ShowHitMarkers);
    apply!(show_floating_damage, ShowFloatingDamage);
    apply!(show_bullet_tracers, ShowBulletTracers);
    apply!(autopilot, Autopilot);
    apply!(inspect_mode, InspectMode);
    apply!(nav_debug_visible, NavDebugVisible);
}

pub(crate) fn apply_persisted_settings(world: &mut World) {
    apply_settings_to_world(world, &load_ship_defaults());
    if reset_config_env_requested() {
        delete_settings_text();
        return;
    }
    info!("[settings] looking for local custom settings at {:?}", settings_file_path());
    let Some(text) = read_settings_text() else {
        info!("[settings] no local custom settings found, staying at ship defaults");
        return;
    };
    let settings: PersistedSettings = match ron::from_str(&text) {
        Ok(s) => s,
        Err(e) => {
            warn!("failed to parse saved settings, ignoring: {e}");
            return;
        }
    };
    info!("[settings] loaded local custom settings, applying on top of ship defaults");
    apply_settings_to_world(world, &settings);
}

pub(crate) fn persist_settings_on_change(world: &mut World) {
    macro_rules! changed {
        ($resource:ty) => {
            world.get_resource_ref::<$resource>().is_some_and(|r| r.is_changed())
        };
    }
    let any_changed = changed!(LightQuality)
        || changed!(ShadowQuality)
        || changed!(PointLightQuality)
        || changed!(RenderResolutionScale)
        || changed!(FpsCap)
        || changed!(MapMsaaOn)
        || changed!(ScopeCameraEnabled)
        || changed!(SensitivityScale)
        || changed!(Noclip)
        || changed!(GodMode)
        || changed!(FreezeZombies)
        || changed!(ShowHitboxes)
        || changed!(AimBlurEnabled)
        || changed!(ZombieAnimDisabled)
        || changed!(ZombieHealthBarsHidden)
        || changed!(PlayerModelVisible)
        || changed!(GunVisible)
        || changed!(AimDofFocalDistanceAlt)
        || changed!(AutopilotLookOnly)
        || changed!(ShowAutopilotTarget)
        || changed!(ShowHitMarkers)
        || changed!(ShowFloatingDamage)
        || changed!(ShowBulletTracers)
        || changed!(Autopilot)
        || changed!(InspectMode)
        || changed!(NavDebugVisible);
    if !any_changed {
        return;
    }
    match ron::to_string(&read_settings_from_world(world)) {
        Ok(text) => write_settings_text(&text),
        Err(e) => warn!("failed to serialize settings: {e}"),
    }
}

fn read_settings_from_world(world: &World) -> PersistedSettings {
    PersistedSettings {
        light_quality: world.get_resource::<LightQuality>().map(|r| r.0),
        shadow_quality: world.get_resource::<ShadowQuality>().map(|r| r.0),
        point_light_quality: world.get_resource::<PointLightQuality>().map(|r| r.0),
        render_resolution_scale: world.get_resource::<RenderResolutionScale>().map(|r| r.0),
        fps_cap: world.get_resource::<FpsCap>().map(|r| r.0),
        map_msaa_on: world.get_resource::<MapMsaaOn>().map(|r| r.0),
        scope_camera_enabled: world.get_resource::<ScopeCameraEnabled>().map(|r| r.0),
        sensitivity_scale: world.get_resource::<SensitivityScale>().map(|r| r.0),
        noclip: world.get_resource::<Noclip>().map(|r| r.0),
        god_mode: world.get_resource::<GodMode>().map(|r| r.0),
        freeze_zombies: world.get_resource::<FreezeZombies>().map(|r| r.0),
        show_hitboxes: world.get_resource::<ShowHitboxes>().map(|r| r.0),
        aim_blur_enabled: world.get_resource::<AimBlurEnabled>().map(|r| r.0),
        zombie_anim_disabled: world.get_resource::<ZombieAnimDisabled>().map(|r| r.0),
        zombie_health_bars_hidden: world.get_resource::<ZombieHealthBarsHidden>().map(|r| r.0),
        player_model_visible: world.get_resource::<PlayerModelVisible>().map(|r| r.0),
        gun_visible: world.get_resource::<GunVisible>().map(|r| r.0),
        aim_dof_focal_distance_alt: world.get_resource::<AimDofFocalDistanceAlt>().map(|r| r.0),
        autopilot_look_only: world.get_resource::<AutopilotLookOnly>().map(|r| r.0),
        show_autopilot_target: world.get_resource::<ShowAutopilotTarget>().map(|r| r.0),
        show_hit_markers: world.get_resource::<ShowHitMarkers>().map(|r| r.0),
        show_floating_damage: world.get_resource::<ShowFloatingDamage>().map(|r| r.0),
        show_bullet_tracers: world.get_resource::<ShowBulletTracers>().map(|r| r.0),
        autopilot: world.get_resource::<Autopilot>().map(|r| r.0),
        inspect_mode: world.get_resource::<InspectMode>().map(|r| r.0),
        nav_debug_visible: world.get_resource::<NavDebugVisible>().map(|r| r.0),
    }
}

pub(crate) fn reset_config_button(world: &mut World) {
    let tapped = world
        .query_filtered::<&Interaction, (With<ResetConfigButton>, Changed<Interaction>)>()
        .iter(world)
        .any(|i| *i == Interaction::Pressed);
    if !tapped {
        return;
    }
    apply_settings_to_world(world, &load_ship_defaults());
    match ron::to_string(&read_settings_from_world(world)) {
        Ok(text) => write_settings_text(&text),
        Err(e) => warn!("failed to serialize settings: {e}"),
    }
}

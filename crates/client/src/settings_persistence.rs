// --- Settings persistence -------------------------------------------
//
// Every value adjustable from the Settings menu (quality sliders,
// sensitivity, resolution scale, every cheat toggle, distance-LOD
// config...) gets saved locally so relaunching the app doesn't lose it —
// "mở lên khỏi phải config lại" was the actual ask, not just the "real"
// player-facing settings, so cheats are included too.
//
// Storage is one RON-encoded `PersistedSettings` blob:
//   - native (desktop + iOS/Android): a dotfile under `$HOME` (see
//     settings_file_path — iOS sets HOME to the app's own sandboxed
//     container, so this is a normal writable per-app location there,
//     not a shared/global path).
//   - wasm32: `window.localStorage`, since a browser tab has no real
//     filesystem to write to (same constraint noted on
//     shared::read_ron_data_file!'s own doc comment).
//
// Every field is `Option<T>` — `None` means "not pinned, let the
// resource's own `Default` decide" (which for a handful of fields is
// genuinely dynamic — RENDER_SCALE/LIGHTS/SHADOWS env vars, or
// Autopilot's per-scenario default — logic this system has no business
// duplicating). `#[serde(default)]` on the struct means an older file
// missing newer fields still deserializes fine (they come back `None`).
//
// Two SEPARATE sources feed this, not one:
//   - Shipped defaults (assets/data/default_settings_{desktop,mobile}.ron,
//     load_ship_defaults) — read-only, bundled with the app, one file per
//     platform (mobile = iOS/Android sharing the same lower-tier numbers,
//     matching every existing `cfg!(any(target_os = "ios", target_os =
//     "android"))` split elsewhere in this crate). This is what an actual
//     human can open and edit to retune "what does a fresh install ship
//     at" without touching Rust code or recompiling.
//   - The device's own local custom settings (a dotfile under `$HOME` on
//     native — see settings_file_path, ios_home_directory for why a plain
//     env var isn't enough on iOS; `window.localStorage` on wasm32) —
//     written every time any tracked resource actually changes, and wins
//     over the shipped file wherever it has its own `Some` value.
//
// Flow:
//   - apply_persisted_settings (Startup, exclusive `&mut World` — the
//     only way to touch ~35 different resource types without blowing
//     past Bevy's per-system param-count ceiling, see
//     cheat_toggle_buttons_2's own doc comment in settings_menu.rs for
//     the same wall hit a different way) applies the shipped defaults
//     first, then layers the device's local custom file on top if one
//     exists. RESET_CONFIG=1 (or any truthy value) skips the local file
//     entirely and deletes it — same env-var-driven-reset shape as
//     TOUCH_UI_PREVIEW/SHOW_HIT_MARKERS elsewhere in this crate.
//   - persist_settings_on_change (Update, also exclusive — same reason)
//     checks `is_changed()` on every tracked resource every frame; the
//     instant ANY of them changed (a settings button tapped, a cheat
//     toggled, ...) it re-serializes the whole struct and writes it to
//     the device's local file. Whole-file rewrite rather than a partial
//     diff — these change on a button tap, not every frame, so there's
//     no real cost to keep this simple.
//   - reset_config_button (settings_menu.rs's "Reset Config" button,
//     map-select screen) re-applies the shipped-defaults file to every
//     live resource AND overwrites the device's local custom file with
//     that same fully-resolved state — "reset" means "go back to exactly
//     what a fresh install would have", not just "whatever Default::
//     default() happens to compute right now".

use bevy::prelude::*;
use crate::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default, Clone)]
#[serde(default)]
pub(crate) struct PersistedSettings {
    pub(crate) texture_quality: Option<u8>,
    pub(crate) poly_quality: Option<u8>,
    pub(crate) zombie_texture_quality: Option<u8>,
    pub(crate) gun_texture_quality: Option<u8>,
    pub(crate) gun_poly_quality: Option<u8>,
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
    pub(crate) zombie_lod_disabled: Option<bool>,
    pub(crate) player_model_visible: Option<bool>,
    pub(crate) gun_visible: Option<bool>,
    pub(crate) cars_visible: Option<bool>,
    pub(crate) aim_dof_focal_distance_alt: Option<bool>,
    pub(crate) autopilot_look_only: Option<bool>,
    pub(crate) show_autopilot_target: Option<bool>,
    pub(crate) show_hit_markers: Option<bool>,
    pub(crate) show_floating_damage: Option<bool>,
    pub(crate) show_bullet_tracers: Option<bool>,
    pub(crate) autopilot: Option<bool>,
    pub(crate) inspect_mode: Option<bool>,
    pub(crate) nav_debug_visible: Option<bool>,
    pub(crate) show_distance_rings: Option<bool>,
    pub(crate) distance_lod_enabled: Option<bool>,
    pub(crate) lod_band_distance: Option<[f32; 3]>,
    pub(crate) lod_car_texture_level: Option<[u8; 3]>,
    pub(crate) lod_car_poly_level: Option<[u8; 3]>,
    pub(crate) lod_zombie_texture_level: Option<[u8; 3]>,
}

// A real iOS app launched by the OS (as opposed to a command-line tool
// exec'd from a shell) has no guarantee `HOME` is actually populated in
// its process environment — confirmed the hard way: settings silently
// never persisted on-device, and `std::env::var("HOME")` was the
// suspect (its fallback of "." resolves to whatever the current
// directory happens to be at process start, which for an app bundle
// launched by iOS is not guaranteed writable or stable across
// relaunches). `NSHomeDirectory()` is the actual Foundation API for
// this — a plain C function (not an Objective-C message), called the
// same way native_metal_probe.rs's own bundle_resource_path() already
// calls NSBundle for the read-only asset path, just without needing
// msg_send since this one isn't a method call.
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

#[cfg(not(target_arch = "wasm32"))]
fn settings_file_path() -> std::path::PathBuf {
    // iOS sandbox rule confirmed the hard way on a real device (syslog:
    // "Operation not permitted (os error 1)"): NSHomeDirectory() is a
    // real, writable-by-this-app directory, but you can't write files
    // directly at ITS root — only inside specific subdirectories
    // (Documents/, Library/, tmp/, ...). "Library/Application Support" is
    // Apple's own recommended spot for exactly this kind of "app-managed
    // data the user never browses directly" file (Documents is for
    // user-visible/iCloud-backed/Files-app-exposed content instead). See
    // write_settings_text's create_dir_all — a fresh container has no
    // Application Support folder yet, has to be created before first
    // write.
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

#[cfg(not(target_arch = "wasm32"))]
fn read_settings_text() -> Option<String> {
    std::fs::read_to_string(settings_file_path()).ok()
}

#[cfg(not(target_arch = "wasm32"))]
fn write_settings_text(text: &str) {
    let path = settings_file_path();
    if let Some(parent) = path.parent() {
        // No-op on desktop ($HOME always exists already); on iOS a fresh
        // container has no "Library/Application Support" subfolder until
        // something creates it.
        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::write(&path, text) {
        Ok(()) => info!("[settings] saved to {path:?}"),
        Err(e) => warn!("[settings] failed to save to {path:?}: {e}"),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn delete_settings_text() {
    let _ = std::fs::remove_file(settings_file_path());
}

const WEB_STORAGE_KEY: &str = "bevy_fps_demo_settings";

#[cfg(target_arch = "wasm32")]
fn read_settings_text() -> Option<String> {
    web_sys::window()?.local_storage().ok()??.get_item(WEB_STORAGE_KEY).ok()?
}

#[cfg(target_arch = "wasm32")]
fn write_settings_text(text: &str) {
    if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok()).flatten() {
        let _ = storage.set_item(WEB_STORAGE_KEY, text);
    }
}

#[cfg(target_arch = "wasm32")]
fn delete_settings_text() {
    if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok()).flatten() {
        let _ = storage.remove_item(WEB_STORAGE_KEY);
    }
}

// The bundled, read-only "what does a fresh install ship at" reference —
// completely separate from the device's own writable custom-settings
// file above. One RON file per platform bucket (mobile = iOS/Android,
// matching every other `cfg!(any(target_os = "ios", target_os =
// "android"))` split in this crate); `shared::read_ron_data_file!`
// already handles reading it correctly per-platform (NSBundle.
// resourcePath on iOS, CARGO_MANIFEST_DIR on desktop, include_str! on
// wasm32 — see that macro's own doc comment), so this needs no bespoke
// path logic of its own the way the writable file above does.
fn load_ship_defaults() -> PersistedSettings {
    #[cfg(any(target_os = "ios", target_os = "android"))]
    let text = shared::read_ron_data_file!("default_settings_mobile.ron");
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    let text = shared::read_ron_data_file!("default_settings_desktop.ron");
    ron::from_str(&text).unwrap_or_else(|e| {
        warn!("failed to parse bundled default settings, falling back to compiled-in Default: {e}");
        PersistedSettings::default()
    })
}

// Same truthy-string convention as every other env-var toggle in this
// crate (TOUCH_UI_PREVIEW, SHOW_HIT_MARKERS, ...). wasm32 has no env vars
// at all (std::env::var always Err there — see shared::read_ron_data_
// file!'s own doc comment), so this is native-only; reset on web goes
// through the settings-menu button instead.
fn reset_config_env_requested() -> bool {
    std::env::var("RESET_CONFIG").is_ok_and(|v| v == "1" || v.to_lowercase() == "true")
}

// Shared by apply_persisted_settings (ship defaults, then local custom on
// top) and reset_config_button (ship defaults only) — every `Some` field
// in `settings` overwrites the matching resource; `None` fields are left
// exactly as they already are, whatever set them.
fn apply_settings_to_world(world: &mut World, settings: &PersistedSettings) {
    macro_rules! apply {
        // get_resource_mut, not resource_mut: NavDebugVisible (and
        // anything else similarly gated) doesn't exist at all in release
        // builds (NavDebugPlugin::build returns early — see its own doc
        // comment), so the panicking accessor would crash every release
        // launch the instant a saved settings file had that field set.
        ($field:ident, $resource:ty) => {
            if let Some(v) = settings.$field
                && let Some(mut r) = world.get_resource_mut::<$resource>()
            {
                r.0 = v;
            }
        };
    }
    apply!(texture_quality, TextureQuality);
    apply!(poly_quality, PolyQuality);
    apply!(zombie_texture_quality, ZombieTextureQuality);
    apply!(gun_texture_quality, GunTextureQuality);
    apply!(gun_poly_quality, GunPolyQuality);
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
    apply!(zombie_lod_disabled, ZombieLodDisabled);
    apply!(player_model_visible, PlayerModelVisible);
    apply!(gun_visible, GunVisible);
    apply!(cars_visible, CarsVisible);
    apply!(aim_dof_focal_distance_alt, AimDofFocalDistanceAlt);
    apply!(autopilot_look_only, AutopilotLookOnly);
    apply!(show_autopilot_target, ShowAutopilotTarget);
    apply!(show_hit_markers, ShowHitMarkers);
    apply!(show_floating_damage, ShowFloatingDamage);
    apply!(show_bullet_tracers, ShowBulletTracers);
    apply!(autopilot, Autopilot);
    apply!(inspect_mode, InspectMode);
    apply!(nav_debug_visible, NavDebugVisible);
    apply!(show_distance_rings, ShowDistanceRings);
    apply!(distance_lod_enabled, DistanceLodEnabled);
    apply!(lod_band_distance, LodBandDistance);
    apply!(lod_car_texture_level, LodCarTextureLevel);
    apply!(lod_car_poly_level, LodCarPolyLevel);
    apply!(lod_zombie_texture_level, LodZombieTextureLevel);
}

pub(crate) fn apply_persisted_settings(world: &mut World) {
    apply_settings_to_world(world, &load_ship_defaults());
    if reset_config_env_requested() {
        delete_settings_text();
        return;
    }
    #[cfg(not(target_arch = "wasm32"))]
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
    let any_changed = changed!(TextureQuality)
        || changed!(PolyQuality)
        || changed!(ZombieTextureQuality)
        || changed!(GunTextureQuality)
        || changed!(GunPolyQuality)
        || changed!(LightQuality)
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
        || changed!(ZombieLodDisabled)
        || changed!(PlayerModelVisible)
        || changed!(GunVisible)
        || changed!(CarsVisible)
        || changed!(AimDofFocalDistanceAlt)
        || changed!(AutopilotLookOnly)
        || changed!(ShowAutopilotTarget)
        || changed!(ShowHitMarkers)
        || changed!(ShowFloatingDamage)
        || changed!(ShowBulletTracers)
        || changed!(Autopilot)
        || changed!(InspectMode)
        || changed!(NavDebugVisible)
        || changed!(ShowDistanceRings)
        || changed!(DistanceLodEnabled)
        || changed!(LodBandDistance)
        || changed!(LodCarTextureLevel)
        || changed!(LodCarPolyLevel)
        || changed!(LodZombieTextureLevel);
    if !any_changed {
        return;
    }
    match ron::to_string(&read_settings_from_world(world)) {
        Ok(text) => write_settings_text(&text),
        Err(e) => warn!("failed to serialize settings: {e}"),
    }
}

// Shared by persist_settings_on_change (snapshot to write to the local
// file) and reset_config_button (snapshot AFTER ship defaults have been
// applied, so what gets written back is the fully-resolved reset state,
// not just whichever fields the ship-defaults file happened to pin).
fn read_settings_from_world(world: &World) -> PersistedSettings {
    PersistedSettings {
        texture_quality: world.get_resource::<TextureQuality>().map(|r| r.0),
        poly_quality: world.get_resource::<PolyQuality>().map(|r| r.0),
        zombie_texture_quality: world.get_resource::<ZombieTextureQuality>().map(|r| r.0),
        gun_texture_quality: world.get_resource::<GunTextureQuality>().map(|r| r.0),
        gun_poly_quality: world.get_resource::<GunPolyQuality>().map(|r| r.0),
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
        zombie_lod_disabled: world.get_resource::<ZombieLodDisabled>().map(|r| r.0),
        player_model_visible: world.get_resource::<PlayerModelVisible>().map(|r| r.0),
        gun_visible: world.get_resource::<GunVisible>().map(|r| r.0),
        cars_visible: world.get_resource::<CarsVisible>().map(|r| r.0),
        aim_dof_focal_distance_alt: world.get_resource::<AimDofFocalDistanceAlt>().map(|r| r.0),
        autopilot_look_only: world.get_resource::<AutopilotLookOnly>().map(|r| r.0),
        show_autopilot_target: world.get_resource::<ShowAutopilotTarget>().map(|r| r.0),
        show_hit_markers: world.get_resource::<ShowHitMarkers>().map(|r| r.0),
        show_floating_damage: world.get_resource::<ShowFloatingDamage>().map(|r| r.0),
        show_bullet_tracers: world.get_resource::<ShowBulletTracers>().map(|r| r.0),
        autopilot: world.get_resource::<Autopilot>().map(|r| r.0),
        inspect_mode: world.get_resource::<InspectMode>().map(|r| r.0),
        nav_debug_visible: world.get_resource::<NavDebugVisible>().map(|r| r.0),
        show_distance_rings: world.get_resource::<ShowDistanceRings>().map(|r| r.0),
        distance_lod_enabled: world.get_resource::<DistanceLodEnabled>().map(|r| r.0),
        lod_band_distance: world.get_resource::<LodBandDistance>().map(|r| r.0),
        lod_car_texture_level: world.get_resource::<LodCarTextureLevel>().map(|r| r.0),
        lod_car_poly_level: world.get_resource::<LodCarPolyLevel>().map(|r| r.0),
        lod_zombie_texture_level: world.get_resource::<LodZombieTextureLevel>().map(|r| r.0),
    }
}

// "Reset Config" button (map-select screen, network_setup.rs) — re-applies
// the SHIPPED defaults (assets/data/default_settings_{desktop,mobile}.ron,
// same file apply_persisted_settings reads at Startup), not just whatever
// Default::default() happens to compute right now — "reset" means "go
// back to exactly what a fresh install would have", including the couple
// of fields (autopilot, render_resolution_scale, shadow/light/point-light
// quality on desktop) that file deliberately leaves `None` for the
// existing env-var/scenario-aware Default to keep deciding. The
// fully-resolved result (every field now concretely whatever it landed
// on) gets written back as the device's new local custom-settings file
// too, so it doesn't just revert in memory this session and drift back on
// next launch. Exclusive `&mut World`, not plain Commands, because it
// needs to both apply AND read back ~35 resources — Commands can insert
// blindly with no such limit, but reading current values back for the
// write-out needs real World access.
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

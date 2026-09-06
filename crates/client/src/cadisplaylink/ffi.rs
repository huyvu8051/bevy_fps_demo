//! extern "C" bridge Swift calls into. Unlike jinleili/bevy-in-app's
//! original (which hardcodes TouchInput.id = 0, so only ever tracks one
//! finger), touch_id here is a real per-finger identifier passed in from
//! Swift — our probe's joystick (left half) and look-drag (right half)
//! need two simultaneous touches to both work at once.

use super::native_metal_renderer::NativeMetalRenderer;
use bevy::app::App;
use bevy::ecs::{entity::Entity, system::Query, world::FromWorld};
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyCode, KeyboardInput, NativeKey};
use bevy::input::mouse::{MouseButton, MouseButtonInput, MouseMotion};
use bevy::input::touch::{TouchInput, TouchPhase};
use bevy::math::Vec2;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow, Window};

// Draws the REAL game's own level (spawn_test_arena's walls+floor, the
// ak47_scope gun), camera driven by the real game's FpsCamera instead of
// touch input — see native_metal_renderer.rs's `real_game_app` field and
// native_game_app.rs/native_game_extract.rs for the headless App +
// extraction it reads from. Used to have a sibling `native_metal_probe_
// create` (real_game=false) drawing this probe's own standalone Sponza/
// box-arena/patrol demo World instead — removed once that path was
// confirmed unreachable in the shipped app (ViewController.swift hardcodes
// USE_REAL_GAME_NATIVE_RENDERER = true); see git history if a standalone
// demo/probe entry point is ever worth resurrecting.
#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_create_real_game(
    view: *mut objc::runtime::Object,
    _scale_factor: f32,
) -> *mut std::ffi::c_void {
    let probe = unsafe { NativeMetalRenderer::new(view) };
    Box::into_raw(Box::new(probe)) as *mut std::ffi::c_void
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_render_frame(obj: *mut std::ffi::c_void) {
    let probe = unsafe { &mut *(obj as *mut NativeMetalRenderer) };
    probe.render_frame();
}

// Swift tracks the raw touches (left half = move, right half = look, same
// split as the original Swift-only demo) and calls this once per frame
// with the resulting deltas instead of moving its own camera fields.
#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_update_camera(
    obj: *mut std::ffi::c_void,
    forward: f32,
    strafe: f32,
    look_dx: f32,
    look_dy: f32,
    delta_time: f32,
) {
    let probe = unsafe { &mut *(obj as *mut NativeMetalRenderer) };
    probe.update_camera(forward, strafe, look_dx, look_dy, delta_time);
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_release(obj: *mut std::ffi::c_void) {
    let probe: Box<NativeMetalRenderer> = unsafe { Box::from_raw(obj as *mut NativeMetalRenderer) };
    drop(probe);
}

// Shared with `native_metal_probe_touch_*` below (real-game App).
pub(super) fn touched(app: &mut App, touch_id: u64, phase: TouchPhase, position: Vec2) {
    let mut windows_state: bevy::ecs::system::SystemState<Query<Entity, bevy::ecs::query::With<Window>>> =
        bevy::ecs::system::SystemState::from_world(app.world_mut());
    let windows = windows_state.get(app.world_mut());
    let Ok(entity) = windows.single() else {
        return;
    };
    let input = TouchInput {
        window: entity,
        phase,
        position,
        force: None,
        id: touch_id,
    };
    // Also feed bevy_picking's own touch backend (touch_pick_events, which
    // reads WindowEvent::TouchInput specifically, not the raw TouchInput
    // message above — see native_game_app.rs's own `app.add_message::
    // <WindowEvent>()` comment, which registered this message type
    // expecting it to stay forever empty/a no-op since there's no real
    // window/winit here to ever populate it). Writing it ourselves is
    // enough: UiPickingPlugin (bundled inside UiPlugin, already added) and
    // the rest of DefaultPickingPlugins (already added) don't need a real
    // window, only this message and the synthetic PrimaryWindow entity
    // `entity` already resolves to — so HoverMap starts reflecting real,
    // Z-order-respecting topmost hits instead of staying permanently
    // empty. multi_touch_button_interaction's own hand-rolled per-Button
    // rect loop (needed for touches after the first, which bevy_ui's own
    // focus system never looks at) can now check HoverMap instead of a
    // raw rect-contains sweep across every Button on screen regardless of
    // what's drawn on top of it.
    app.world_mut().write_message(bevy::window::WindowEvent::TouchInput(input));
    app.world_mut().write_message(input);
}

fn primary_window_entity(app: &mut App) -> Option<Entity> {
    let mut windows_state: bevy::ecs::system::SystemState<Query<Entity, bevy::ecs::query::With<Window>>> =
        bevy::ecs::system::SystemState::from_world(app.world_mut());
    windows_state.get(app.world_mut()).single().ok()
}

// GCKeyCode has no stable numeric identity we want to depend on directly
// (its rawValue happens to line up with USB HID usage codes today, but
// that's undocumented) — GameControllerInput.swift's own switch converts
// each GCKeyCode it cares about into one of these small, explicit IDs
// instead, so the Rust<->Swift contract is this file's own enum, not an
// Apple implementation detail.
fn map_key_id(id: u16) -> Option<KeyCode> {
    const LETTERS: [KeyCode; 26] = [
        KeyCode::KeyA,
        KeyCode::KeyB,
        KeyCode::KeyC,
        KeyCode::KeyD,
        KeyCode::KeyE,
        KeyCode::KeyF,
        KeyCode::KeyG,
        KeyCode::KeyH,
        KeyCode::KeyI,
        KeyCode::KeyJ,
        KeyCode::KeyK,
        KeyCode::KeyL,
        KeyCode::KeyM,
        KeyCode::KeyN,
        KeyCode::KeyO,
        KeyCode::KeyP,
        KeyCode::KeyQ,
        KeyCode::KeyR,
        KeyCode::KeyS,
        KeyCode::KeyT,
        KeyCode::KeyU,
        KeyCode::KeyV,
        KeyCode::KeyW,
        KeyCode::KeyX,
        KeyCode::KeyY,
        KeyCode::KeyZ,
    ];
    const DIGITS: [KeyCode; 10] = [
        KeyCode::Digit0,
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
        KeyCode::Digit7,
        KeyCode::Digit8,
        KeyCode::Digit9,
    ];
    Some(match id {
        0..=25 => LETTERS[id as usize],
        26..=35 => DIGITS[(id - 26) as usize],
        36 => KeyCode::Space,
        37 => KeyCode::Escape,
        38 => KeyCode::ShiftLeft,
        39 => KeyCode::ShiftRight,
        40 => KeyCode::ControlLeft,
        41 => KeyCode::ControlRight,
        42 => KeyCode::AltLeft,
        43 => KeyCode::AltRight,
        44 => KeyCode::BracketLeft,
        45 => KeyCode::BracketRight,
        46 => KeyCode::F1,
        47 => KeyCode::Tab,
        48 => KeyCode::Enter,
        49 => KeyCode::ArrowUp,
        50 => KeyCode::ArrowDown,
        51 => KeyCode::ArrowLeft,
        52 => KeyCode::ArrowRight,
        _ => return None,
    })
}

// Shared with `native_metal_probe_key_changed` below — GameControllerInput.
// swift calls that from GCKeyboardInput's keyChangedHandler, one call per
// key transition, translated from Apple's GCKeyCode to `key_id` via that
// file's own switch (see map_key_id above for the shared numbering both
// sides agree on).
pub(super) fn key_changed(app: &mut App, key_id: u16, pressed: bool) {
    let Some(key_code) = map_key_id(key_id) else {
        return;
    };
    let Some(window) = primary_window_entity(app) else {
        return;
    };
    app.world_mut().write_message(KeyboardInput {
        key_code,
        logical_key: Key::Unidentified(NativeKey::Unidentified),
        state: if pressed { ButtonState::Pressed } else { ButtonState::Released },
        text: None,
        repeat: false,
        window,
    });
}

// Shared with `native_metal_probe_mouse_connected` below —
// GameControllerInput.swift calls that once when a GCMouse connects/
// disconnects — mouse_look (see crates/client/src/lib.rs) only reads
// MouseMotion while CursorOptions.grab_mode is Locked, same gate desktop's
// click-to-lock uses; there's no on-screen cursor to click on iOS, so a
// connected hardware mouse just locks look immediately instead.
pub(super) fn mouse_connected(app: &mut App, connected: bool) {
    let mut cursor_state: bevy::ecs::system::SystemState<Query<&mut CursorOptions, bevy::ecs::query::With<PrimaryWindow>>> =
        bevy::ecs::system::SystemState::from_world(app.world_mut());
    let mut cursor = cursor_state.get_mut(app.world_mut());
    let Ok(mut cursor) = cursor.single_mut() else {
        return;
    };
    cursor.grab_mode = if connected { CursorGrabMode::Locked } else { CursorGrabMode::None };
}

// Shared with `native_metal_probe_mouse_moved` below.
pub(super) fn mouse_moved(app: &mut App, delta_x: f32, delta_y: f32) {
    app.world_mut().write_message(MouseMotion {
        delta: Vec2::new(delta_x, delta_y),
    });
}

// Shared with `native_metal_probe_mouse_button_changed` below. button: 0 =
// left, 1 = right, 2 = middle — matches GameControllerInput.swift's own
// leftButton/rightButton/middleButton wiring.
pub(super) fn mouse_button_changed(app: &mut App, button: u8, pressed: bool) {
    let button = match button {
        0 => MouseButton::Left,
        1 => MouseButton::Right,
        2 => MouseButton::Middle,
        _ => return,
    };
    let Some(window) = primary_window_entity(app) else {
        return;
    };
    app.world_mut().write_message(MouseButtonInput {
        button,
        state: if pressed { ButtonState::Pressed } else { ButtonState::Released },
        window,
    });
}

// Phase 6 Step 3: the shared `touched`/`key_changed`/`mouse_*` helpers
// above, re-targeted at the real headless game App a `NativeMetalRenderer`
// constructed with `real_game: true` owns (see
// `NativeMetalRenderer::real_game_app_mut`) — the only entry points Swift
// actually calls (see ViewController.swift/GameControllerInput.swift) for
// BOTH touch (joystick/look-drag) and Bluetooth keyboard/mouse, so either
// input source drives the real game's own `send_player_input`/`mouse_look`/
// `touch_look` systems unmodified. No-ops if this probe isn't in real-game
// mode.
#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_touch_started(obj: *mut std::ffi::c_void, touch_id: u64, x: f32, y: f32) {
    let probe = unsafe { &mut *(obj as *mut NativeMetalRenderer) };
    if let Some(app) = probe.real_game_app_mut() {
        touched(app, touch_id, TouchPhase::Started, Vec2::new(x, y));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_touch_moved(obj: *mut std::ffi::c_void, touch_id: u64, x: f32, y: f32) {
    let probe = unsafe { &mut *(obj as *mut NativeMetalRenderer) };
    if let Some(app) = probe.real_game_app_mut() {
        touched(app, touch_id, TouchPhase::Moved, Vec2::new(x, y));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_touch_ended(obj: *mut std::ffi::c_void, touch_id: u64, x: f32, y: f32) {
    let probe = unsafe { &mut *(obj as *mut NativeMetalRenderer) };
    if let Some(app) = probe.real_game_app_mut() {
        touched(app, touch_id, TouchPhase::Ended, Vec2::new(x, y));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_touch_cancelled(obj: *mut std::ffi::c_void, touch_id: u64, x: f32, y: f32) {
    let probe = unsafe { &mut *(obj as *mut NativeMetalRenderer) };
    if let Some(app) = probe.real_game_app_mut() {
        touched(app, touch_id, TouchPhase::Canceled, Vec2::new(x, y));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_key_changed(obj: *mut std::ffi::c_void, key_id: u16, pressed: bool) {
    let probe = unsafe { &mut *(obj as *mut NativeMetalRenderer) };
    if let Some(app) = probe.real_game_app_mut() {
        key_changed(app, key_id, pressed);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_mouse_connected(obj: *mut std::ffi::c_void, connected: bool) {
    let probe = unsafe { &mut *(obj as *mut NativeMetalRenderer) };
    if let Some(app) = probe.real_game_app_mut() {
        mouse_connected(app, connected);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_mouse_moved(obj: *mut std::ffi::c_void, delta_x: f32, delta_y: f32) {
    let probe = unsafe { &mut *(obj as *mut NativeMetalRenderer) };
    if let Some(app) = probe.real_game_app_mut() {
        mouse_moved(app, delta_x, delta_y);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_mouse_button_changed(obj: *mut std::ffi::c_void, button: u8, pressed: bool) {
    let probe = unsafe { &mut *(obj as *mut NativeMetalRenderer) };
    if let Some(app) = probe.real_game_app_mut() {
        mouse_button_changed(app, button, pressed);
    }
}

// Native Swift settings-menu overlay (see native_menu.rs's doc comment for
// why: our custom Metal renderer never draws any `bevy_ui` content, so the
// real game's own `spawn_settings_menu` UI is invisible no matter how
// correctly `SettingsMenuVisible` itself toggles). All of these read/write
// the same headless real-game `World` `native_metal_probe_*` input
// functions already target — no-ops if this probe isn't in real-game mode.
#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_menu_visible(obj: *mut std::ffi::c_void) -> bool {
    let probe = unsafe { &mut *(obj as *mut NativeMetalRenderer) };
    probe.real_game_app_mut().map(super::native_menu::menu_visible).unwrap_or(false)
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_set_menu_visible(obj: *mut std::ffi::c_void, visible: bool) {
    let probe = unsafe { &mut *(obj as *mut NativeMetalRenderer) };
    if let Some(app) = probe.real_game_app_mut() {
        super::native_menu::set_menu_visible(app, visible);
    }
}

// Death/restart — the real game's own "GAME OVER" bevy_ui overlay is
// invisible on iOS for the same reason the settings menu was (see
// native_menu.rs's own doc comment on game_over/trigger_restart). Not
// debug-gated: dying and restarting is normal gameplay in every build.
#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_game_over(obj: *mut std::ffi::c_void) -> bool {
    let probe = unsafe { &mut *(obj as *mut NativeMetalRenderer) };
    probe.real_game_app_mut().map(super::native_menu::game_over).unwrap_or(false)
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_trigger_restart(obj: *mut std::ffi::c_void) {
    let probe = unsafe { &mut *(obj as *mut NativeMetalRenderer) };
    if let Some(app) = probe.real_game_app_mut() {
        super::native_menu::trigger_restart(app);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_get_sensitivity(obj: *mut std::ffi::c_void) -> f32 {
    let probe = unsafe { &mut *(obj as *mut NativeMetalRenderer) };
    probe.real_game_app_mut().map(super::native_menu::sensitivity).unwrap_or(1.0)
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_set_sensitivity(obj: *mut std::ffi::c_void, value: f32) {
    let probe = unsafe { &mut *(obj as *mut NativeMetalRenderer) };
    if let Some(app) = probe.real_game_app_mut() {
        super::native_menu::set_sensitivity(app, value);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_get_fps_cap(obj: *mut std::ffi::c_void) -> f32 {
    let probe = unsafe { &mut *(obj as *mut NativeMetalRenderer) };
    probe.real_game_app_mut().map(super::native_menu::fps_cap).unwrap_or(120.0)
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_get_player_hp(obj: *mut std::ffi::c_void) -> f32 {
    let probe = unsafe { &mut *(obj as *mut NativeMetalRenderer) };
    probe.real_game_app_mut().map(super::native_menu::player_hp).unwrap_or(-1.0)
}

// Ping (ms) — same bevy_ui HUD readout that's invisible on iOS; see
// native_menu::ping_ms's own doc comment. -1.0 = not connected yet.
#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_get_ping_ms(obj: *mut std::ffi::c_void) -> f32 {
    let probe = unsafe { &mut *(obj as *mut NativeMetalRenderer) };
    probe.real_game_app_mut().map(super::native_menu::ping_ms).unwrap_or(-1.0)
}

// Cheat controls — debug builds only, mirroring the same
// `#[cfg(debug_assertions)]` gate just added around `spawn_settings_menu`'s
// cheat buttons in crate::lib, so a release build's native menu can't
// reach these through this door either. In release builds these still
// link (Swift always calls the same symbol names) but are no-ops, since a
// release `ViewController` build should also compile out the cheat UI
// entirely via its own `#if DEBUG` gate — see ViewController.swift.
#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_get_god_mode(obj: *mut std::ffi::c_void) -> bool {
    let probe = unsafe { &mut *(obj as *mut NativeMetalRenderer) };
    #[cfg(debug_assertions)]
    return probe.real_game_app_mut().map(super::native_menu::god_mode).unwrap_or(false);
    #[cfg(not(debug_assertions))]
    { let _ = probe; false }
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_set_god_mode(obj: *mut std::ffi::c_void, on: bool) {
    let probe = unsafe { &mut *(obj as *mut NativeMetalRenderer) };
    #[cfg(debug_assertions)]
    if let Some(app) = probe.real_game_app_mut() {
        super::native_menu::set_god_mode(app, on);
    }
    #[cfg(not(debug_assertions))]
    { let _ = (probe, on); }
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_get_noclip(obj: *mut std::ffi::c_void) -> bool {
    let probe = unsafe { &mut *(obj as *mut NativeMetalRenderer) };
    #[cfg(debug_assertions)]
    return probe.real_game_app_mut().map(super::native_menu::noclip).unwrap_or(false);
    #[cfg(not(debug_assertions))]
    { let _ = probe; false }
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_set_noclip(obj: *mut std::ffi::c_void, on: bool) {
    let probe = unsafe { &mut *(obj as *mut NativeMetalRenderer) };
    #[cfg(debug_assertions)]
    if let Some(app) = probe.real_game_app_mut() {
        super::native_menu::set_noclip(app, on);
    }
    #[cfg(not(debug_assertions))]
    { let _ = (probe, on); }
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_get_freeze_zombies(obj: *mut std::ffi::c_void) -> bool {
    let probe = unsafe { &mut *(obj as *mut NativeMetalRenderer) };
    #[cfg(debug_assertions)]
    return probe.real_game_app_mut().map(super::native_menu::freeze_zombies).unwrap_or(false);
    #[cfg(not(debug_assertions))]
    { let _ = probe; false }
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_set_freeze_zombies(obj: *mut std::ffi::c_void, on: bool) {
    let probe = unsafe { &mut *(obj as *mut NativeMetalRenderer) };
    #[cfg(debug_assertions)]
    if let Some(app) = probe.real_game_app_mut() {
        super::native_menu::set_freeze_zombies(app, on);
    }
    #[cfg(not(debug_assertions))]
    { let _ = (probe, on); }
}

// Unlike every other toggle pair on this page, not debug-gated — Autopilot
// is a real player-facing feature (see crate::autopilot's own module doc
// comment), not a cheat, so it must work in release builds too.
#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_get_autopilot(obj: *mut std::ffi::c_void) -> bool {
    let probe = unsafe { &mut *(obj as *mut NativeMetalRenderer) };
    probe.real_game_app_mut().map(super::native_menu::autopilot).unwrap_or(true)
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_set_autopilot(obj: *mut std::ffi::c_void, on: bool) {
    let probe = unsafe { &mut *(obj as *mut NativeMetalRenderer) };
    if let Some(app) = probe.real_game_app_mut() {
        super::native_menu::set_autopilot(app, on);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_trigger_kill_all_zombies(obj: *mut std::ffi::c_void) {
    let probe = unsafe { &mut *(obj as *mut NativeMetalRenderer) };
    #[cfg(debug_assertions)]
    if let Some(app) = probe.real_game_app_mut() {
        super::native_menu::trigger_kill_all_zombies(app);
    }
    #[cfg(not(debug_assertions))]
    { let _ = probe; }
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_trigger_spawn_more_zombies(obj: *mut std::ffi::c_void) {
    let probe = unsafe { &mut *(obj as *mut NativeMetalRenderer) };
    #[cfg(debug_assertions)]
    if let Some(app) = probe.real_game_app_mut() {
        super::native_menu::trigger_spawn_more_zombies(app);
    }
    #[cfg(not(debug_assertions))]
    { let _ = probe; }
}

// The remaining Cheats-page toggles (Gun/MSAA/Shadow, Hitboxes/AimBlur/
// PlayerMdl, NoZAnim/NoHPBar/FullLOD) are all the exact same "no-op unless
// debug, read/write a plain local bool" shape as god_mode/noclip/
// freeze_zombies above, so a macro generates the extern "C" get/set pair
// per resource rather than hand-copying the same 12 lines 9 times more.
macro_rules! bool_toggle_ffi {
    ($get_ffi:ident, $set_ffi:ident, $get:path, $set:path) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn $get_ffi(obj: *mut std::ffi::c_void) -> bool {
            let probe = unsafe { &mut *(obj as *mut NativeMetalRenderer) };
            #[cfg(debug_assertions)]
            return probe.real_game_app_mut().map($get).unwrap_or(false);
            #[cfg(not(debug_assertions))]
            {
                let _ = probe;
                false
            }
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn $set_ffi(obj: *mut std::ffi::c_void, on: bool) {
            let probe = unsafe { &mut *(obj as *mut NativeMetalRenderer) };
            #[cfg(debug_assertions)]
            if let Some(app) = probe.real_game_app_mut() {
                $set(app, on);
            }
            #[cfg(not(debug_assertions))]
            {
                let _ = (probe, on);
            }
        }
    };
}

bool_toggle_ffi!(
    native_metal_probe_get_gun_visible,
    native_metal_probe_set_gun_visible,
    super::native_menu::gun_visible,
    super::native_menu::set_gun_visible
);
bool_toggle_ffi!(
    native_metal_probe_get_map_msaa,
    native_metal_probe_set_map_msaa,
    super::native_menu::map_msaa,
    super::native_menu::set_map_msaa
);
bool_toggle_ffi!(
    native_metal_probe_get_show_hitboxes,
    native_metal_probe_set_show_hitboxes,
    super::native_menu::show_hitboxes,
    super::native_menu::set_show_hitboxes
);
bool_toggle_ffi!(
    native_metal_probe_get_show_autopilot_target,
    native_metal_probe_set_show_autopilot_target,
    super::native_menu::show_autopilot_target,
    super::native_menu::set_show_autopilot_target
);
bool_toggle_ffi!(
    native_metal_probe_get_show_hit_markers,
    native_metal_probe_set_show_hit_markers,
    super::native_menu::show_hit_markers,
    super::native_menu::set_show_hit_markers
);
bool_toggle_ffi!(
    native_metal_probe_get_aim_blur_enabled,
    native_metal_probe_set_aim_blur_enabled,
    super::native_menu::aim_blur_enabled,
    super::native_menu::set_aim_blur_enabled
);
bool_toggle_ffi!(
    native_metal_probe_get_zombie_anim_disabled,
    native_metal_probe_set_zombie_anim_disabled,
    super::native_menu::zombie_anim_disabled,
    super::native_menu::set_zombie_anim_disabled
);
bool_toggle_ffi!(
    native_metal_probe_get_zombie_health_bars_hidden,
    native_metal_probe_set_zombie_health_bars_hidden,
    super::native_menu::zombie_health_bars_hidden,
    super::native_menu::set_zombie_health_bars_hidden
);
bool_toggle_ffi!(
    native_metal_probe_get_zombie_lod_disabled,
    native_metal_probe_set_zombie_lod_disabled,
    super::native_menu::zombie_lod_disabled,
    super::native_menu::set_zombie_lod_disabled
);
bool_toggle_ffi!(
    native_metal_probe_get_player_model_visible,
    native_metal_probe_set_player_model_visible,
    super::native_menu::player_model_visible,
    super::native_menu::set_player_model_visible
);

// Zombie count +/-10 and Heal Nearest/1HP Nearest — same debug-only gate,
// same no-op-in-release shape as trigger_kill_all_zombies above.
#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_get_zombie_target_count(obj: *mut std::ffi::c_void) -> u32 {
    let probe = unsafe { &mut *(obj as *mut NativeMetalRenderer) };
    #[cfg(debug_assertions)]
    return probe.real_game_app_mut().map(super::native_menu::zombie_target_count).unwrap_or(10);
    #[cfg(not(debug_assertions))]
    {
        let _ = probe;
        10
    }
}

// Real, current headcount — not debug-gated (it's just a display value,
// not a cheat), unlike the target count above. See live_zombie_count's own
// doc comment for why the target alone was reported as a confusing/wrong
// "counter".
#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_get_live_zombie_count(obj: *mut std::ffi::c_void) -> u32 {
    let probe = unsafe { &mut *(obj as *mut NativeMetalRenderer) };
    probe.real_game_app_mut().map(super::native_menu::live_zombie_count).unwrap_or(0)
}

// Total zombies this local player has personally killed this session — see
// native_menu::kill_count's own doc comment for how kills are detected.
// Not debug-gated: like live_zombie_count, this is a normal-gameplay
// display value (the Watch companion app's kill counter), not a cheat.
#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_get_kill_count(obj: *mut std::ffi::c_void) -> u32 {
    let probe = unsafe { &mut *(obj as *mut NativeMetalRenderer) };
    probe.real_game_app_mut().map(super::native_menu::kill_count).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_zombie_count_dec(obj: *mut std::ffi::c_void) {
    let probe = unsafe { &mut *(obj as *mut NativeMetalRenderer) };
    #[cfg(debug_assertions)]
    if let Some(app) = probe.real_game_app_mut() {
        super::native_menu::zombie_count_dec(app);
    }
    #[cfg(not(debug_assertions))]
    {
        let _ = probe;
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_zombie_count_inc(obj: *mut std::ffi::c_void) {
    let probe = unsafe { &mut *(obj as *mut NativeMetalRenderer) };
    #[cfg(debug_assertions)]
    if let Some(app) = probe.real_game_app_mut() {
        super::native_menu::zombie_count_inc(app);
    }
    #[cfg(not(debug_assertions))]
    {
        let _ = probe;
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_heal_nearest_zombie(obj: *mut std::ffi::c_void) {
    let probe = unsafe { &mut *(obj as *mut NativeMetalRenderer) };
    #[cfg(debug_assertions)]
    if let Some(app) = probe.real_game_app_mut() {
        super::native_menu::heal_nearest_zombie(app);
    }
    #[cfg(not(debug_assertions))]
    {
        let _ = probe;
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_near_death_nearest_zombie(obj: *mut std::ffi::c_void) {
    let probe = unsafe { &mut *(obj as *mut NativeMetalRenderer) };
    #[cfg(debug_assertions)]
    if let Some(app) = probe.real_game_app_mut() {
        super::native_menu::near_death_nearest_zombie(app);
    }
    #[cfg(not(debug_assertions))]
    {
        let _ = probe;
    }
}


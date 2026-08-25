//! extern "C" bridge Swift calls into. Unlike jinleili/bevy-in-app's
//! original (which hardcodes TouchInput.id = 0, so only ever tracks one
//! finger), touch_id here is a real per-finger identifier passed in from
//! Swift — our probe's joystick (left half) and look-drag (right half)
//! need two simultaneous touches to both work at once.

use super::native_game_app::build_headless_game_ecs_app;
use super::native_game_extract::extract_render_snapshot;
use super::native_metal_probe::NativeMetalProbe;
use super::{IosViewObj, build_cadisplaylink_fps_probe_app, build_cadisplaylink_game_app};
use bevy::app::App;
use bevy::ecs::{entity::Entity, system::Query, world::FromWorld};
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyCode, KeyboardInput, NativeKey};
use bevy::input::mouse::{MouseButton, MouseButtonInput, MouseMotion};
use bevy::input::touch::{TouchInput, TouchPhase};
use bevy::math::Vec2;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow, Window};

// Bevy UI text silently renders nothing (no error) if it's extracted for
// the first time before its glyph atlas/layout has settled — a known Bevy
// gap (see https://github.com/bevyengine/bevy/issues/18205 for the same
// "invalid/not-yet-ready span renders blank" shape). Startup also kicks off
// a bunch of async asset loads (scene gltf, textures, sounds) on background
// task-pool threads — those need real wall-clock time to make progress, not
// just Bevy update() ticks, so a tight loop of update() calls alone doesn't
// reliably give them time to finish.
//
// warm_up_app() below runs update() repeatedly for a minimum real-time
// window (WARMUP_MIN_DURATION), not a fixed tick count, so the background
// loader threads get actual wall-clock time to run alongside it — before
// Swift's CADisplayLink starts presenting real frames, so the first frame
// the user actually sees isn't also the first frame each piece of UI text
// was ever drawn or the first frame of a still-loading model appearing.
const WARMUP_MIN_DURATION: std::time::Duration = std::time::Duration::from_millis(1500);
const WARMUP_MIN_TICKS: u32 = 5;

fn warm_up_app(app: &mut App) {
    let start = std::time::Instant::now();
    let mut ticks = 0;
    while ticks < WARMUP_MIN_TICKS || start.elapsed() < WARMUP_MIN_DURATION {
        app.update();
        ticks += 1;
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn cadisplaylink_create_game_app(view: *mut objc::runtime::Object, scale_factor: f32) -> *mut std::ffi::c_void {
    let view_obj = IosViewObj { view, scale_factor };
    let mut app = build_cadisplaylink_game_app(view_obj);
    warm_up_app(&mut app);
    Box::into_raw(Box::new(app)) as *mut std::ffi::c_void
}

// crate::fps_probe's App — camera + static room, no game logic — for
// measuring raw achievable FPS/CPU. Skips warm_up_app on purpose: there's
// no UI/async asset loading here to warm up for.
#[unsafe(no_mangle)]
pub extern "C" fn cadisplaylink_create_fps_probe_app(view: *mut objc::runtime::Object, scale_factor: f32) -> *mut std::ffi::c_void {
    let view_obj = IosViewObj { view, scale_factor };
    let app = build_cadisplaylink_fps_probe_app(view_obj);
    Box::into_raw(Box::new(app)) as *mut std::ffi::c_void
}

// Control experiment: same 5-cuboid arena as fps_probe.rs/the Swift native
// demo, but rendered from Rust via the `metal` crate directly — no Bevy
// App, no bevy_render, no wgpu at all — to check whether wgpu's "high CPU
// encoding cost" overhead is specific to wgpu's abstraction layer or
// something about doing this from Rust in general. See
// native_metal_probe.rs's own doc comment.
#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_create(view: *mut objc::runtime::Object, _scale_factor: f32) -> *mut std::ffi::c_void {
    let probe = unsafe { NativeMetalProbe::new(view, false) };
    Box::into_raw(Box::new(probe)) as *mut std::ffi::c_void
}

// Phase 6 Step 4: same probe/renderer, but drawing the REAL game's own
// level (spawn_test_arena's walls+floor, the ak47_scope gun) instead of
// the probe's own Sponza/box-arena/patrol World, with the camera driven by
// the real game's FpsCamera instead of touch input. See
// native_metal_probe.rs's `real_game_app` field and native_game_app.rs/
// native_game_extract.rs for the headless App + extraction it reads from.
#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_create_real_game(
    view: *mut objc::runtime::Object,
    _scale_factor: f32,
) -> *mut std::ffi::c_void {
    let probe = unsafe { NativeMetalProbe::new(view, true) };
    Box::into_raw(Box::new(probe)) as *mut std::ffi::c_void
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_render_frame(obj: *mut std::ffi::c_void) {
    let probe = unsafe { &mut *(obj as *mut NativeMetalProbe) };
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
    let probe = unsafe { &mut *(obj as *mut NativeMetalProbe) };
    probe.update_camera(forward, strafe, look_dx, look_dy, delta_time);
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_release(obj: *mut std::ffi::c_void) {
    let probe: Box<NativeMetalProbe> = unsafe { Box::from_raw(obj as *mut NativeMetalProbe) };
    drop(probe);
}

#[unsafe(no_mangle)]
pub extern "C" fn cadisplaylink_enter_frame(obj: *mut std::ffi::c_void) {
    let app = unsafe { &mut *(obj as *mut App) };
    app.update();
}

#[unsafe(no_mangle)]
pub extern "C" fn cadisplaylink_touch_started(obj: *mut std::ffi::c_void, touch_id: u64, x: f32, y: f32) {
    let app = unsafe { &mut *(obj as *mut App) };
    touched(app, touch_id, TouchPhase::Started, Vec2::new(x, y));
}

#[unsafe(no_mangle)]
pub extern "C" fn cadisplaylink_touch_moved(obj: *mut std::ffi::c_void, touch_id: u64, x: f32, y: f32) {
    let app = unsafe { &mut *(obj as *mut App) };
    touched(app, touch_id, TouchPhase::Moved, Vec2::new(x, y));
}

#[unsafe(no_mangle)]
pub extern "C" fn cadisplaylink_touch_ended(obj: *mut std::ffi::c_void, touch_id: u64, x: f32, y: f32) {
    let app = unsafe { &mut *(obj as *mut App) };
    touched(app, touch_id, TouchPhase::Ended, Vec2::new(x, y));
}

#[unsafe(no_mangle)]
pub extern "C" fn cadisplaylink_touch_cancelled(obj: *mut std::ffi::c_void, touch_id: u64, x: f32, y: f32) {
    let app = unsafe { &mut *(obj as *mut App) };
    touched(app, touch_id, TouchPhase::Canceled, Vec2::new(x, y));
}

// Shared by both the toy `cadisplaylink_*` App and Phase 6 Step 3's
// `native_metal_probe_*` real-game App (see below) — same message-writing
// logic either way, only which `App`'s `World` it targets differs.
pub(super) fn touched(app: &mut App, touch_id: u64, phase: TouchPhase, position: Vec2) {
    let mut windows_state: bevy::ecs::system::SystemState<Query<Entity, bevy::ecs::query::With<Window>>> =
        bevy::ecs::system::SystemState::from_world(app.world_mut());
    let windows = windows_state.get(app.world_mut());
    let Ok(entity) = windows.single() else {
        return;
    };
    app.world_mut().write_message(TouchInput {
        window: entity,
        phase,
        position,
        force: None,
        id: touch_id,
    });
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

// GameControllerInput.swift calls this from GCKeyboardInput's
// keyChangedHandler — one call per key transition, translated from Apple's
// GCKeyCode to `key_id` via that file's own switch (see map_key_id above
// for the shared numbering both sides agree on).
#[unsafe(no_mangle)]
pub extern "C" fn cadisplaylink_key_changed(obj: *mut std::ffi::c_void, key_id: u16, pressed: bool) {
    let app = unsafe { &mut *(obj as *mut App) };
    key_changed(app, key_id, pressed);
}

// Shared by both `cadisplaylink_key_changed` and Step 3's
// `native_metal_probe_key_changed`.
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

// GameControllerInput.swift calls this once when a GCMouse connects/
// disconnects — mouse_look (see crates/client/src/lib.rs) only reads
// MouseMotion while CursorOptions.grab_mode is Locked, same gate desktop's
// click-to-lock uses; there's no on-screen cursor to click on iOS, so a
// connected hardware mouse just locks look immediately instead.
#[unsafe(no_mangle)]
pub extern "C" fn cadisplaylink_mouse_connected(obj: *mut std::ffi::c_void, connected: bool) {
    let app = unsafe { &mut *(obj as *mut App) };
    mouse_connected(app, connected);
}

// Shared by both `cadisplaylink_mouse_connected` and Step 3's
// `native_metal_probe_mouse_connected`.
pub(super) fn mouse_connected(app: &mut App, connected: bool) {
    let mut cursor_state: bevy::ecs::system::SystemState<Query<&mut CursorOptions, bevy::ecs::query::With<PrimaryWindow>>> =
        bevy::ecs::system::SystemState::from_world(app.world_mut());
    let mut cursor = cursor_state.get_mut(app.world_mut());
    let Ok(mut cursor) = cursor.single_mut() else {
        return;
    };
    cursor.grab_mode = if connected { CursorGrabMode::Locked } else { CursorGrabMode::None };
}

#[unsafe(no_mangle)]
pub extern "C" fn cadisplaylink_mouse_moved(obj: *mut std::ffi::c_void, delta_x: f32, delta_y: f32) {
    let app = unsafe { &mut *(obj as *mut App) };
    mouse_moved(app, delta_x, delta_y);
}

// Shared by both `cadisplaylink_mouse_moved` and Step 3's
// `native_metal_probe_mouse_moved`.
pub(super) fn mouse_moved(app: &mut App, delta_x: f32, delta_y: f32) {
    app.world_mut().write_message(MouseMotion {
        delta: Vec2::new(delta_x, delta_y),
    });
}

// button: 0 = left, 1 = right, 2 = middle — matches GameControllerInput.swift's own leftButton/rightButton/middleButton wiring.
#[unsafe(no_mangle)]
pub extern "C" fn cadisplaylink_mouse_button_changed(obj: *mut std::ffi::c_void, button: u8, pressed: bool) {
    let app = unsafe { &mut *(obj as *mut App) };
    mouse_button_changed(app, button, pressed);
}

// Shared by both `cadisplaylink_mouse_button_changed` and Step 3's
// `native_metal_probe_mouse_button_changed`.
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

// Phase 6 Step 3: same input plumbing as the `cadisplaylink_*` functions
// above, re-targeted at the real headless game App a `NativeMetalProbe`
// constructed with `real_game: true` owns (see
// `NativeMetalProbe::real_game_app_mut`) — Swift calls these instead of the
// `cadisplaylink_*` ones when `USE_REAL_GAME_NATIVE_RENDERER` is set, for
// BOTH touch (joystick/look-drag) and Bluetooth keyboard/mouse
// (GameControllerInput.swift), so either input source drives the real
// game's own `send_player_input`/`mouse_look`/`touch_look` systems
// unmodified. No-ops if this probe isn't in real-game mode.
#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_touch_started(obj: *mut std::ffi::c_void, touch_id: u64, x: f32, y: f32) {
    let probe = unsafe { &mut *(obj as *mut NativeMetalProbe) };
    if let Some(app) = probe.real_game_app_mut() {
        touched(app, touch_id, TouchPhase::Started, Vec2::new(x, y));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_touch_moved(obj: *mut std::ffi::c_void, touch_id: u64, x: f32, y: f32) {
    let probe = unsafe { &mut *(obj as *mut NativeMetalProbe) };
    if let Some(app) = probe.real_game_app_mut() {
        touched(app, touch_id, TouchPhase::Moved, Vec2::new(x, y));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_touch_ended(obj: *mut std::ffi::c_void, touch_id: u64, x: f32, y: f32) {
    let probe = unsafe { &mut *(obj as *mut NativeMetalProbe) };
    if let Some(app) = probe.real_game_app_mut() {
        touched(app, touch_id, TouchPhase::Ended, Vec2::new(x, y));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_touch_cancelled(obj: *mut std::ffi::c_void, touch_id: u64, x: f32, y: f32) {
    let probe = unsafe { &mut *(obj as *mut NativeMetalProbe) };
    if let Some(app) = probe.real_game_app_mut() {
        touched(app, touch_id, TouchPhase::Canceled, Vec2::new(x, y));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_key_changed(obj: *mut std::ffi::c_void, key_id: u16, pressed: bool) {
    let probe = unsafe { &mut *(obj as *mut NativeMetalProbe) };
    if let Some(app) = probe.real_game_app_mut() {
        key_changed(app, key_id, pressed);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_mouse_connected(obj: *mut std::ffi::c_void, connected: bool) {
    let probe = unsafe { &mut *(obj as *mut NativeMetalProbe) };
    if let Some(app) = probe.real_game_app_mut() {
        mouse_connected(app, connected);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_mouse_moved(obj: *mut std::ffi::c_void, delta_x: f32, delta_y: f32) {
    let probe = unsafe { &mut *(obj as *mut NativeMetalProbe) };
    if let Some(app) = probe.real_game_app_mut() {
        mouse_moved(app, delta_x, delta_y);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_mouse_button_changed(obj: *mut std::ffi::c_void, button: u8, pressed: bool) {
    let probe = unsafe { &mut *(obj as *mut NativeMetalProbe) };
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
    let probe = unsafe { &mut *(obj as *mut NativeMetalProbe) };
    probe.real_game_app_mut().map(super::native_menu::menu_visible).unwrap_or(false)
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_set_menu_visible(obj: *mut std::ffi::c_void, visible: bool) {
    let probe = unsafe { &mut *(obj as *mut NativeMetalProbe) };
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
    let probe = unsafe { &mut *(obj as *mut NativeMetalProbe) };
    probe.real_game_app_mut().map(super::native_menu::game_over).unwrap_or(false)
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_trigger_restart(obj: *mut std::ffi::c_void) {
    let probe = unsafe { &mut *(obj as *mut NativeMetalProbe) };
    if let Some(app) = probe.real_game_app_mut() {
        super::native_menu::trigger_restart(app);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_get_sensitivity(obj: *mut std::ffi::c_void) -> f32 {
    let probe = unsafe { &mut *(obj as *mut NativeMetalProbe) };
    probe.real_game_app_mut().map(super::native_menu::sensitivity).unwrap_or(1.0)
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_set_sensitivity(obj: *mut std::ffi::c_void, value: f32) {
    let probe = unsafe { &mut *(obj as *mut NativeMetalProbe) };
    if let Some(app) = probe.real_game_app_mut() {
        super::native_menu::set_sensitivity(app, value);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_get_player_hp(obj: *mut std::ffi::c_void) -> f32 {
    let probe = unsafe { &mut *(obj as *mut NativeMetalProbe) };
    probe.real_game_app_mut().map(super::native_menu::player_hp).unwrap_or(-1.0)
}

// Ping (ms) — same bevy_ui HUD readout that's invisible on iOS; see
// native_menu::ping_ms's own doc comment. -1.0 = not connected yet.
#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_get_ping_ms(obj: *mut std::ffi::c_void) -> f32 {
    let probe = unsafe { &mut *(obj as *mut NativeMetalProbe) };
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
    let probe = unsafe { &mut *(obj as *mut NativeMetalProbe) };
    #[cfg(debug_assertions)]
    return probe.real_game_app_mut().map(super::native_menu::god_mode).unwrap_or(false);
    #[cfg(not(debug_assertions))]
    { let _ = probe; false }
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_set_god_mode(obj: *mut std::ffi::c_void, on: bool) {
    let probe = unsafe { &mut *(obj as *mut NativeMetalProbe) };
    #[cfg(debug_assertions)]
    if let Some(app) = probe.real_game_app_mut() {
        super::native_menu::set_god_mode(app, on);
    }
    #[cfg(not(debug_assertions))]
    { let _ = (probe, on); }
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_get_noclip(obj: *mut std::ffi::c_void) -> bool {
    let probe = unsafe { &mut *(obj as *mut NativeMetalProbe) };
    #[cfg(debug_assertions)]
    return probe.real_game_app_mut().map(super::native_menu::noclip).unwrap_or(false);
    #[cfg(not(debug_assertions))]
    { let _ = probe; false }
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_set_noclip(obj: *mut std::ffi::c_void, on: bool) {
    let probe = unsafe { &mut *(obj as *mut NativeMetalProbe) };
    #[cfg(debug_assertions)]
    if let Some(app) = probe.real_game_app_mut() {
        super::native_menu::set_noclip(app, on);
    }
    #[cfg(not(debug_assertions))]
    { let _ = (probe, on); }
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_get_freeze_zombies(obj: *mut std::ffi::c_void) -> bool {
    let probe = unsafe { &mut *(obj as *mut NativeMetalProbe) };
    #[cfg(debug_assertions)]
    return probe.real_game_app_mut().map(super::native_menu::freeze_zombies).unwrap_or(false);
    #[cfg(not(debug_assertions))]
    { let _ = probe; false }
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_set_freeze_zombies(obj: *mut std::ffi::c_void, on: bool) {
    let probe = unsafe { &mut *(obj as *mut NativeMetalProbe) };
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
    let probe = unsafe { &mut *(obj as *mut NativeMetalProbe) };
    probe.real_game_app_mut().map(super::native_menu::autopilot).unwrap_or(true)
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_set_autopilot(obj: *mut std::ffi::c_void, on: bool) {
    let probe = unsafe { &mut *(obj as *mut NativeMetalProbe) };
    if let Some(app) = probe.real_game_app_mut() {
        super::native_menu::set_autopilot(app, on);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_trigger_kill_all_zombies(obj: *mut std::ffi::c_void) {
    let probe = unsafe { &mut *(obj as *mut NativeMetalProbe) };
    #[cfg(debug_assertions)]
    if let Some(app) = probe.real_game_app_mut() {
        super::native_menu::trigger_kill_all_zombies(app);
    }
    #[cfg(not(debug_assertions))]
    { let _ = probe; }
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_trigger_spawn_more_zombies(obj: *mut std::ffi::c_void) {
    let probe = unsafe { &mut *(obj as *mut NativeMetalProbe) };
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
            let probe = unsafe { &mut *(obj as *mut NativeMetalProbe) };
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
            let probe = unsafe { &mut *(obj as *mut NativeMetalProbe) };
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
    native_metal_probe_get_map_shadows,
    native_metal_probe_set_map_shadows,
    super::native_menu::map_shadows,
    super::native_menu::set_map_shadows
);
bool_toggle_ffi!(
    native_metal_probe_get_show_hitboxes,
    native_metal_probe_set_show_hitboxes,
    super::native_menu::show_hitboxes,
    super::native_menu::set_show_hitboxes
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
    let probe = unsafe { &mut *(obj as *mut NativeMetalProbe) };
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
    let probe = unsafe { &mut *(obj as *mut NativeMetalProbe) };
    probe.real_game_app_mut().map(super::native_menu::live_zombie_count).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_zombie_count_dec(obj: *mut std::ffi::c_void) {
    let probe = unsafe { &mut *(obj as *mut NativeMetalProbe) };
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
    let probe = unsafe { &mut *(obj as *mut NativeMetalProbe) };
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
    let probe = unsafe { &mut *(obj as *mut NativeMetalProbe) };
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
    let probe = unsafe { &mut *(obj as *mut NativeMetalProbe) };
    #[cfg(debug_assertions)]
    if let Some(app) = probe.real_game_app_mut() {
        super::native_menu::near_death_nearest_zombie(app);
    }
    #[cfg(not(debug_assertions))]
    {
        let _ = probe;
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn cadisplaylink_release_app(obj: *mut std::ffi::c_void) {
    let app: Box<App> = unsafe { Box::from_raw(obj as *mut App) };
    drop(app);
}

// Phase 6 Step 1 feasibility spike: the real game's gameplay App
// (physics/networking/AI/prediction from build_game_common) with no
// window/render backend at all. Not wired into any renderer yet — this
// just proves the headless App itself runs without panicking and that
// networking/physics/AI actually progress. See native_game_app.rs.
#[unsafe(no_mangle)]
pub extern "C" fn native_game_app_create() -> *mut std::ffi::c_void {
    let app = build_headless_game_ecs_app();
    Box::into_raw(Box::new(app)) as *mut std::ffi::c_void
}

#[unsafe(no_mangle)]
pub extern "C" fn native_game_app_tick(obj: *mut std::ffi::c_void) {
    let app = unsafe { &mut *(obj as *mut App) };
    app.update();

    // Phase 6 Step 2 (extraction): pull a plain, owned snapshot out of the
    // headless World — same query shapes the plan specified (Zombie/
    // NetPlayer transforms, FpsCamera pose, static Mesh3d geometry for the
    // arena+gun). Not wired into any renderer yet (Step 4); this just
    // proves the extraction queries themselves resolve correctly against
    // the real game's live entities.
    let snapshot = extract_render_snapshot(app.world_mut());
    let mesh_vert_total: usize = snapshot.static_meshes.iter().map(|m| m.positions.len()).sum();
    match &snapshot.camera {
        Some(cam) => println!(
            "[native_game_app_spike] zombies={} players={} static_meshes={} verts={} camera_eye={:?} yaw={:.2} pitch={:.2}",
            snapshot.zombies.len(),
            snapshot.remote_players.len(),
            snapshot.static_meshes.len(),
            mesh_vert_total,
            cam.eye,
            cam.yaw,
            cam.pitch,
        ),
        None => println!(
            "[native_game_app_spike] zombies={} players={} static_meshes={} verts={} camera=<none yet>",
            snapshot.zombies.len(),
            snapshot.remote_players.len(),
            snapshot.static_meshes.len(),
            mesh_vert_total,
        ),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn native_game_app_release(obj: *mut std::ffi::c_void) {
    let app: Box<App> = unsafe { Box::from_raw(obj as *mut App) };
    drop(app);
}

//! extern "C" bridge Swift calls into. Unlike jinleili/bevy-in-app's
//! original (which hardcodes TouchInput.id = 0, so only ever tracks one
//! finger), touch_id here is a real per-finger identifier passed in from
//! Swift — our probe's joystick (left half) and look-drag (right half)
//! need two simultaneous touches to both work at once.

use super::{IosViewObj, build_cadisplaylink_game_app, build_cadisplaylink_probe_app};
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
pub extern "C" fn cadisplaylink_create_app(view: *mut objc::runtime::Object, scale_factor: f32) -> *mut std::ffi::c_void {
    let view_obj = IosViewObj { view, scale_factor };
    let mut app = build_cadisplaylink_probe_app(view_obj);
    warm_up_app(&mut app);
    Box::into_raw(Box::new(app)) as *mut std::ffi::c_void
}

// Real game (networking + UI + zombies + all debug tooling), same App
// enter_frame/touch_*/release_app below operate on regardless of which of
// these two constructors built it.
#[unsafe(no_mangle)]
pub extern "C" fn cadisplaylink_create_game_app(view: *mut objc::runtime::Object, scale_factor: f32) -> *mut std::ffi::c_void {
    let view_obj = IosViewObj { view, scale_factor };
    let mut app = build_cadisplaylink_game_app(view_obj);
    warm_up_app(&mut app);
    Box::into_raw(Box::new(app)) as *mut std::ffi::c_void
}

#[unsafe(no_mangle)]
pub extern "C" fn cadisplaylink_enter_frame(obj: *mut std::ffi::c_void) {
    let app = unsafe { &mut *(obj as *mut App) };
    app.update();
}

#[unsafe(no_mangle)]
pub extern "C" fn cadisplaylink_touch_started(obj: *mut std::ffi::c_void, touch_id: u64, x: f32, y: f32) {
    touched(obj, touch_id, TouchPhase::Started, Vec2::new(x, y));
}

#[unsafe(no_mangle)]
pub extern "C" fn cadisplaylink_touch_moved(obj: *mut std::ffi::c_void, touch_id: u64, x: f32, y: f32) {
    touched(obj, touch_id, TouchPhase::Moved, Vec2::new(x, y));
}

#[unsafe(no_mangle)]
pub extern "C" fn cadisplaylink_touch_ended(obj: *mut std::ffi::c_void, touch_id: u64, x: f32, y: f32) {
    touched(obj, touch_id, TouchPhase::Ended, Vec2::new(x, y));
}

#[unsafe(no_mangle)]
pub extern "C" fn cadisplaylink_touch_cancelled(obj: *mut std::ffi::c_void, touch_id: u64, x: f32, y: f32) {
    touched(obj, touch_id, TouchPhase::Canceled, Vec2::new(x, y));
}

fn touched(obj: *mut std::ffi::c_void, touch_id: u64, phase: TouchPhase, position: Vec2) {
    let app = unsafe { &mut *(obj as *mut App) };
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
    app.world_mut().write_message(MouseMotion {
        delta: Vec2::new(delta_x, delta_y),
    });
}

// button: 0 = left, 1 = right, 2 = middle — matches GameControllerInput.swift's own leftButton/rightButton/middleButton wiring.
#[unsafe(no_mangle)]
pub extern "C" fn cadisplaylink_mouse_button_changed(obj: *mut std::ffi::c_void, button: u8, pressed: bool) {
    let app = unsafe { &mut *(obj as *mut App) };
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

#[unsafe(no_mangle)]
pub extern "C" fn cadisplaylink_release_app(obj: *mut std::ffi::c_void) {
    let app: Box<App> = unsafe { Box::from_raw(obj as *mut App) };
    drop(app);
}

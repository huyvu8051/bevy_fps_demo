//! extern "C" bridge Swift calls into. Unlike jinleili/bevy-in-app's
//! original (which hardcodes TouchInput.id = 0, so only ever tracks one
//! finger), touch_id here is a real per-finger identifier passed in from
//! Swift — our probe's joystick (left half) and look-drag (right half)
//! need two simultaneous touches to both work at once.

use super::{IosViewObj, build_cadisplaylink_game_app, build_cadisplaylink_probe_app};
use bevy::app::App;
use bevy::ecs::{entity::Entity, system::Query, world::FromWorld};
use bevy::input::touch::{TouchInput, TouchPhase};
use bevy::math::Vec2;
use bevy::window::Window;

#[unsafe(no_mangle)]
pub extern "C" fn cadisplaylink_create_app(view: *mut objc::runtime::Object, scale_factor: f32) -> *mut std::ffi::c_void {
    let view_obj = IosViewObj { view, scale_factor };
    let app = build_cadisplaylink_probe_app(view_obj);
    Box::into_raw(Box::new(app)) as *mut std::ffi::c_void
}

// Real game (networking + UI + zombies + all debug tooling), same App
// enter_frame/touch_*/release_app below operate on regardless of which of
// these two constructors built it.
#[unsafe(no_mangle)]
pub extern "C" fn cadisplaylink_create_game_app(view: *mut objc::runtime::Object, scale_factor: f32) -> *mut std::ffi::c_void {
    let view_obj = IosViewObj { view, scale_factor };
    let app = build_cadisplaylink_game_app(view_obj);
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

#[unsafe(no_mangle)]
pub extern "C" fn cadisplaylink_release_app(obj: *mut std::ffi::c_void) {
    let app: Box<App> = unsafe { Box::from_raw(obj as *mut App) };
    drop(app);
}

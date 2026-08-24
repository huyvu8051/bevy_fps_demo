//
//  libbevy_in_app.h
//  client_iOS_CADisplayLink
//
//  FFI bridge into crates/client's cadisplaylink module (Rust side:
//  crates/client/src/cadisplaylink/ffi.rs).
//

#ifndef libbevy_in_app_h
#define libbevy_in_app_h

#include <stdbool.h>
#include <stdint.h>

// Opaque pointer to the Rust-side bevy::app::App.
struct bevy_app;

struct bevy_app *cadisplaylink_create_game_app(void *view, float scale_factor);
// Camera + a small static primitive-built room, no game logic at all — see
// crates/client/src/fps_probe.rs.
struct bevy_app *cadisplaylink_create_fps_probe_app(void *view, float scale_factor);

void cadisplaylink_enter_frame(struct bevy_app *app);
void cadisplaylink_release_app(struct bevy_app *app);

// Rust + `metal` crate control experiment (no Bevy App at all) — see
// crates/client/src/cadisplaylink/native_metal_probe.rs.
struct native_metal_probe;
struct native_metal_probe *native_metal_probe_create(void *view, float scale_factor);
void native_metal_probe_render_frame(struct native_metal_probe *probe);
void native_metal_probe_update_camera(struct native_metal_probe *probe, float forward, float strafe, float look_dx, float look_dy, float delta_time);
void native_metal_probe_release(struct native_metal_probe *probe);

// touch_id is a stable per-finger identifier for the duration of one touch
// (we pass in the UITouch's own pointer address, cast to an integer) — the
// probe needs two simultaneous touches (joystick + look-drag) to both work,
// unlike upstream bevy-in-app's single hardcoded id=0.
void cadisplaylink_touch_started(struct bevy_app *app, uint64_t touch_id, float x, float y);
void cadisplaylink_touch_moved(struct bevy_app *app, uint64_t touch_id, float x, float y);
void cadisplaylink_touch_ended(struct bevy_app *app, uint64_t touch_id, float x, float y);
void cadisplaylink_touch_cancelled(struct bevy_app *app, uint64_t touch_id, float x, float y);

// Bluetooth keyboard/mouse support (GameController framework, see
// GameControllerInput.swift) — key_id is our own small numbering (letters
// 0-25, digits 26-35, then a handful of named keys), NOT a raw GCKeyCode,
// so the Rust<->Swift contract doesn't depend on an Apple implementation
// detail (see map_key_id in crates/client/src/cadisplaylink/ffi.rs for the
// authoritative list both sides agree on).
void cadisplaylink_key_changed(struct bevy_app *app, uint16_t key_id, bool pressed);
// Locks mouse-look the instant a hardware mouse connects/disconnects —
// there's no on-screen cursor to click-to-lock on iOS the way desktop does.
void cadisplaylink_mouse_connected(struct bevy_app *app, bool connected);
void cadisplaylink_mouse_moved(struct bevy_app *app, float delta_x, float delta_y);
// button: 0 = left, 1 = right, 2 = middle.
void cadisplaylink_mouse_button_changed(struct bevy_app *app, uint8_t button, bool pressed);

#endif /* libbevy_in_app_h */

//
//  libbevy_in_app.h
//  client_iOS_CADisplayLink
//
//  FFI bridge into crates/client's cadisplaylink module (Rust side:
//  crates/client/src/cadisplaylink/ffi.rs).
//

#ifndef libbevy_in_app_h
#define libbevy_in_app_h

#include <stdint.h>

// Opaque pointer to the Rust-side bevy::app::App.
struct bevy_app;

struct bevy_app *cadisplaylink_create_app(void *view, float scale_factor);
// Full real game (networking/UI/zombies/debug tooling) instead of the
// stripped-down probe scene.
struct bevy_app *cadisplaylink_create_game_app(void *view, float scale_factor);

void cadisplaylink_enter_frame(struct bevy_app *app);
void cadisplaylink_release_app(struct bevy_app *app);

// touch_id is a stable per-finger identifier for the duration of one touch
// (we pass in the UITouch's own pointer address, cast to an integer) — the
// probe needs two simultaneous touches (joystick + look-drag) to both work,
// unlike upstream bevy-in-app's single hardcoded id=0.
void cadisplaylink_touch_started(struct bevy_app *app, uint64_t touch_id, float x, float y);
void cadisplaylink_touch_moved(struct bevy_app *app, uint64_t touch_id, float x, float y);
void cadisplaylink_touch_ended(struct bevy_app *app, uint64_t touch_id, float x, float y);
void cadisplaylink_touch_cancelled(struct bevy_app *app, uint64_t touch_id, float x, float y);

#endif /* libbevy_in_app_h */

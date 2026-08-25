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
// Phase 6 Step 4: same probe, but draws the REAL game's own level
// (arena+gun) with the camera driven by the real FpsCamera instead of
// touch input — see native_metal_probe.rs's `real_game_app` field.
struct native_metal_probe *native_metal_probe_create_real_game(void *view, float scale_factor);
void native_metal_probe_render_frame(struct native_metal_probe *probe);
void native_metal_probe_update_camera(struct native_metal_probe *probe, float forward, float strafe, float look_dx, float look_dy, float delta_time);
void native_metal_probe_release(struct native_metal_probe *probe);

// Phase 6 Step 3: same input plumbing as the cadisplaylink_* functions
// below, re-targeted at a real-game-mode probe's own headless App — call
// these instead of the cadisplaylink_* ones when
// USE_REAL_GAME_NATIVE_RENDERER is set, for BOTH touch and Bluetooth
// keyboard/mouse. No-ops if the probe isn't in real-game mode.
void native_metal_probe_touch_started(struct native_metal_probe *probe, uint64_t touch_id, float x, float y);
void native_metal_probe_touch_moved(struct native_metal_probe *probe, uint64_t touch_id, float x, float y);
void native_metal_probe_touch_ended(struct native_metal_probe *probe, uint64_t touch_id, float x, float y);
void native_metal_probe_touch_cancelled(struct native_metal_probe *probe, uint64_t touch_id, float x, float y);
void native_metal_probe_key_changed(struct native_metal_probe *probe, uint16_t key_id, bool pressed);
void native_metal_probe_mouse_connected(struct native_metal_probe *probe, bool connected);
void native_metal_probe_mouse_moved(struct native_metal_probe *probe, float delta_x, float delta_y);
void native_metal_probe_mouse_button_changed(struct native_metal_probe *probe, uint8_t button, bool pressed);

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

// Native Swift settings-menu overlay — our custom Metal renderer never
// draws bevy_ui content, so the real game's own settings menu is invisible
// no matter how correctly SettingsMenuVisible itself toggles (F1/MENU
// button). These read/write that real headless World directly instead —
// see crates/client/src/cadisplaylink/native_menu.rs. No-ops (return
// false/0/-1) if the probe isn't in real-game mode.
bool native_metal_probe_menu_visible(struct native_metal_probe *probe);
void native_metal_probe_set_menu_visible(struct native_metal_probe *probe, bool visible);
float native_metal_probe_get_sensitivity(struct native_metal_probe *probe);
void native_metal_probe_set_sensitivity(struct native_metal_probe *probe, float value);
float native_metal_probe_get_player_hp(struct native_metal_probe *probe);
// Ping (ms). -1.0 = not connected yet.
float native_metal_probe_get_ping_ms(struct native_metal_probe *probe);
// Cheat controls — debug builds only (compiled out / no-op in release,
// mirroring the #[cfg(debug_assertions)] gate on spawn_settings_menu's own
// cheat buttons). Kill All / Spawn +100 need a live server connection
// (server-authoritative replicon events) — same requirement normal play
// already has.
bool native_metal_probe_get_god_mode(struct native_metal_probe *probe);
void native_metal_probe_set_god_mode(struct native_metal_probe *probe, bool on);
bool native_metal_probe_get_noclip(struct native_metal_probe *probe);
void native_metal_probe_set_noclip(struct native_metal_probe *probe, bool on);
bool native_metal_probe_get_freeze_zombies(struct native_metal_probe *probe);
void native_metal_probe_set_freeze_zombies(struct native_metal_probe *probe, bool on);
void native_metal_probe_trigger_kill_all_zombies(struct native_metal_probe *probe);
void native_metal_probe_trigger_spawn_more_zombies(struct native_metal_probe *probe);
// Remaining Cheats-page toggles (page 1's Gun/MSAA/Shadow, Hitboxes/
// AimBlur/PlayerMdl, NoZAnim/NoHPBar/FullLOD rows) — same debug-only,
// no-op-in-release contract as the toggles above.
bool native_metal_probe_get_gun_visible(struct native_metal_probe *probe);
void native_metal_probe_set_gun_visible(struct native_metal_probe *probe, bool on);
bool native_metal_probe_get_map_msaa(struct native_metal_probe *probe);
void native_metal_probe_set_map_msaa(struct native_metal_probe *probe, bool on);
bool native_metal_probe_get_map_shadows(struct native_metal_probe *probe);
void native_metal_probe_set_map_shadows(struct native_metal_probe *probe, bool on);
bool native_metal_probe_get_show_hitboxes(struct native_metal_probe *probe);
void native_metal_probe_set_show_hitboxes(struct native_metal_probe *probe, bool on);
bool native_metal_probe_get_aim_blur_enabled(struct native_metal_probe *probe);
void native_metal_probe_set_aim_blur_enabled(struct native_metal_probe *probe, bool on);
bool native_metal_probe_get_zombie_anim_disabled(struct native_metal_probe *probe);
void native_metal_probe_set_zombie_anim_disabled(struct native_metal_probe *probe, bool on);
bool native_metal_probe_get_zombie_health_bars_hidden(struct native_metal_probe *probe);
void native_metal_probe_set_zombie_health_bars_hidden(struct native_metal_probe *probe, bool on);
bool native_metal_probe_get_zombie_lod_disabled(struct native_metal_probe *probe);
void native_metal_probe_set_zombie_lod_disabled(struct native_metal_probe *probe, bool on);
bool native_metal_probe_get_player_model_visible(struct native_metal_probe *probe);
void native_metal_probe_set_player_model_visible(struct native_metal_probe *probe, bool on);
// Zombie count +/-10 (needs a live server, same as Kill All/Spawn +100)
// and Heal Nearest/1HP Nearest (client-side cosmetic write only — gets
// overwritten by the next server replication update, same pre-existing
// limitation the real game's own version has).
uint32_t native_metal_probe_get_zombie_target_count(struct native_metal_probe *probe);
// Real, current headcount (not debug-gated, just a display value) — the
// target count above never reflects Spawn +100/Kill All/natural deaths.
uint32_t native_metal_probe_get_live_zombie_count(struct native_metal_probe *probe);
void native_metal_probe_zombie_count_dec(struct native_metal_probe *probe);
void native_metal_probe_zombie_count_inc(struct native_metal_probe *probe);
void native_metal_probe_heal_nearest_zombie(struct native_metal_probe *probe);
void native_metal_probe_near_death_nearest_zombie(struct native_metal_probe *probe);
// Death/restart — normal gameplay, not debug-gated. The real game's own
// "GAME OVER" bevy_ui overlay never renders on iOS (same reason the
// settings menu didn't), so a native equivalent is needed to see it at
// all and to restart. Restart needs a live server (server-authoritative,
// same as Kill All/Spawn +100).
bool native_metal_probe_game_over(struct native_metal_probe *probe);
void native_metal_probe_trigger_restart(struct native_metal_probe *probe);

// Phase 6 Step 1 feasibility spike: the real game's gameplay App
// (physics/networking/AI/prediction) with no window/render backend at all
// — see crates/client/src/cadisplaylink/native_game_app.rs. Not wired into
// any renderer yet; native_game_app_tick just prints zombie/player counts.
struct bevy_app *native_game_app_create(void);
void native_game_app_tick(struct bevy_app *app);
void native_game_app_release(struct bevy_app *app);

#endif /* libbevy_in_app_h */

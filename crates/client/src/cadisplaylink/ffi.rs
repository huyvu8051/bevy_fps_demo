
use metal_renderer::native_metal_renderer::{NativeMetalRenderer, SkinnedModelSpec};
use bevy::app::App;
use bevy::ecs::{entity::Entity, system::Query, world::FromWorld};
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyCode, KeyboardInput, NativeKey};
use bevy::input::mouse::{MouseButton, MouseButtonInput, MouseMotion};
use bevy::input::touch::{TouchInput, TouchPhase};
use bevy::math::Vec2;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow, Window};

/// Owns both halves of the iOS entry point: the game's headless ECS app and
/// the generic Metal renderer that draws whatever the app extracts.
pub struct GameHost {
    app: App,
    renderer: NativeMetalRenderer,
}

fn zombie_model_specs() -> Vec<SkinnedModelSpec> {
    let mut specs = vec![SkinnedModelSpec {
        name: "soldier",
        asset_path: "characters/soldier_meshy/mesh/soldier_meshy.glb",
        scale: 1.04,
        y_offset: -0.76,
        facing_radians: 0.0,
    }];
    for kind in [shared::ZombieKind::Classic, shared::ZombieKind::Meshy, shared::ZombieKind::Soldier] {
        specs.push(SkinnedModelSpec {
            name: shared::zombie_kind_metal_model_name(kind),
            asset_path: shared::zombie_kind_mesh_path(kind),
            scale: 1.0,
            y_offset: 0.0,
            facing_radians: 0.0,
        });
    }
    specs
}

impl GameHost {
    fn real_game_app_mut(&mut self) -> Option<&mut App> {
        Some(&mut self.app)
    }

    fn update_camera(&mut self, forward: f32, strafe: f32, look_dx: f32, look_dy: f32, dt: f32) {
        self.renderer.update_camera(forward, strafe, look_dx, look_dy, dt);
    }

    fn render_frame(&mut self) {
        let (width_pts, height_pts) = self.renderer.view_size();
        super::native_game_extract::prime_camera_viewport(
            self.app.world_mut(),
            width_pts,
            height_pts,
            self.renderer.contents_scale(),
        );
        self.app.update();
        let snapshot = super::native_game_extract::extract_render_snapshot(self.app.world_mut(), |id| {
            self.renderer.has_uploaded_mesh(id)
        });
        self.renderer.render_frame(snapshot);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_create_real_game(
    view: *mut objc::runtime::Object,
    scale_factor: f32,
) -> *mut std::ffi::c_void {
    let renderer = unsafe { NativeMetalRenderer::new(view, &zombie_model_specs(), Default::default()) };
    let (width_pts, height_pts) = renderer.view_size();
    let scale = if scale_factor > 0.0 { scale_factor } else { renderer.contents_scale() };
    let app = super::native_game_app::build_headless_game_ecs_app(
        (width_pts * scale) as u32,
        (height_pts * scale) as u32,
        scale,
    );
    Box::into_raw(Box::new(GameHost { app, renderer })) as *mut std::ffi::c_void
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_render_frame(obj: *mut std::ffi::c_void) {
    let probe = unsafe { &mut *(obj as *mut GameHost) };
    probe.render_frame();
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_update_camera(
    obj: *mut std::ffi::c_void,
    forward: f32,
    strafe: f32,
    look_dx: f32,
    look_dy: f32,
    delta_time: f32,
) {
    let probe = unsafe { &mut *(obj as *mut GameHost) };
    probe.update_camera(forward, strafe, look_dx, look_dy, delta_time);
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_release(obj: *mut std::ffi::c_void) {
    let probe: Box<GameHost> = unsafe { Box::from_raw(obj as *mut GameHost) };
    drop(probe);
}

pub(super) fn touched(app: &mut App, touch_id: u64, phase: TouchPhase, position: Vec2) {
    let mut windows_state: bevy::ecs::system::SystemState<Query<Entity, bevy::ecs::query::With<Window>>> =
        bevy::ecs::system::SystemState::from_world(app.world_mut());
    let windows = windows_state.get(app.world_mut()).expect("system param validation failed");
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

    app.world_mut().write_message(bevy::window::WindowEvent::TouchInput(input));
    app.world_mut().write_message(input);
}

fn primary_window_entity(app: &mut App) -> Option<Entity> {
    let mut windows_state: bevy::ecs::system::SystemState<Query<Entity, bevy::ecs::query::With<Window>>> =
        bevy::ecs::system::SystemState::from_world(app.world_mut());
    windows_state.get(app.world_mut()).expect("system param validation failed").single().ok()
}

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

pub(super) fn mouse_connected(app: &mut App, connected: bool) {
    let mut cursor_state: bevy::ecs::system::SystemState<Query<&mut CursorOptions, bevy::ecs::query::With<PrimaryWindow>>> =
        bevy::ecs::system::SystemState::from_world(app.world_mut());
    let mut cursor = cursor_state.get_mut(app.world_mut()).expect("system param validation failed");
    let Ok(mut cursor) = cursor.single_mut() else {
        return;
    };
    cursor.grab_mode = if connected { CursorGrabMode::Locked } else { CursorGrabMode::None };
}

pub(super) fn mouse_moved(app: &mut App, delta_x: f32, delta_y: f32) {
    app.world_mut().write_message(MouseMotion {
        delta: Vec2::new(delta_x, delta_y),
    });
}

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

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_touch_started(obj: *mut std::ffi::c_void, touch_id: u64, x: f32, y: f32) {
    let probe = unsafe { &mut *(obj as *mut GameHost) };
    if let Some(app) = probe.real_game_app_mut() {
        touched(app, touch_id, TouchPhase::Started, Vec2::new(x, y));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_touch_moved(obj: *mut std::ffi::c_void, touch_id: u64, x: f32, y: f32) {
    let probe = unsafe { &mut *(obj as *mut GameHost) };
    if let Some(app) = probe.real_game_app_mut() {
        touched(app, touch_id, TouchPhase::Moved, Vec2::new(x, y));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_touch_ended(obj: *mut std::ffi::c_void, touch_id: u64, x: f32, y: f32) {
    let probe = unsafe { &mut *(obj as *mut GameHost) };
    if let Some(app) = probe.real_game_app_mut() {
        touched(app, touch_id, TouchPhase::Ended, Vec2::new(x, y));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_touch_cancelled(obj: *mut std::ffi::c_void, touch_id: u64, x: f32, y: f32) {
    let probe = unsafe { &mut *(obj as *mut GameHost) };
    if let Some(app) = probe.real_game_app_mut() {
        touched(app, touch_id, TouchPhase::Canceled, Vec2::new(x, y));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_key_changed(obj: *mut std::ffi::c_void, key_id: u16, pressed: bool) {
    let probe = unsafe { &mut *(obj as *mut GameHost) };
    if let Some(app) = probe.real_game_app_mut() {
        key_changed(app, key_id, pressed);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_mouse_connected(obj: *mut std::ffi::c_void, connected: bool) {
    let probe = unsafe { &mut *(obj as *mut GameHost) };
    if let Some(app) = probe.real_game_app_mut() {
        mouse_connected(app, connected);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_mouse_moved(obj: *mut std::ffi::c_void, delta_x: f32, delta_y: f32) {
    let probe = unsafe { &mut *(obj as *mut GameHost) };
    if let Some(app) = probe.real_game_app_mut() {
        mouse_moved(app, delta_x, delta_y);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_mouse_button_changed(obj: *mut std::ffi::c_void, button: u8, pressed: bool) {
    let probe = unsafe { &mut *(obj as *mut GameHost) };
    if let Some(app) = probe.real_game_app_mut() {
        mouse_button_changed(app, button, pressed);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_menu_visible(obj: *mut std::ffi::c_void) -> bool {
    let probe = unsafe { &mut *(obj as *mut GameHost) };
    probe.real_game_app_mut().map(super::native_menu::menu_visible).unwrap_or(false)
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_set_menu_visible(obj: *mut std::ffi::c_void, visible: bool) {
    let probe = unsafe { &mut *(obj as *mut GameHost) };
    if let Some(app) = probe.real_game_app_mut() {
        super::native_menu::set_menu_visible(app, visible);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_game_over(obj: *mut std::ffi::c_void) -> bool {
    let probe = unsafe { &mut *(obj as *mut GameHost) };
    probe.real_game_app_mut().map(super::native_menu::game_over).unwrap_or(false)
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_trigger_restart(obj: *mut std::ffi::c_void) {
    let probe = unsafe { &mut *(obj as *mut GameHost) };
    if let Some(app) = probe.real_game_app_mut() {
        super::native_menu::trigger_restart(app);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_get_sensitivity(obj: *mut std::ffi::c_void) -> f32 {
    let probe = unsafe { &mut *(obj as *mut GameHost) };
    probe.real_game_app_mut().map(super::native_menu::sensitivity).unwrap_or(1.0)
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_set_sensitivity(obj: *mut std::ffi::c_void, value: f32) {
    let probe = unsafe { &mut *(obj as *mut GameHost) };
    if let Some(app) = probe.real_game_app_mut() {
        super::native_menu::set_sensitivity(app, value);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_get_fps_cap(obj: *mut std::ffi::c_void) -> f32 {
    let probe = unsafe { &mut *(obj as *mut GameHost) };
    probe.real_game_app_mut().map(super::native_menu::fps_cap).unwrap_or(120.0)
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_get_player_hp(obj: *mut std::ffi::c_void) -> f32 {
    let probe = unsafe { &mut *(obj as *mut GameHost) };
    probe.real_game_app_mut().map(super::native_menu::player_hp).unwrap_or(-1.0)
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_get_ping_ms(obj: *mut std::ffi::c_void) -> f32 {
    let probe = unsafe { &mut *(obj as *mut GameHost) };
    probe.real_game_app_mut().map(super::native_menu::ping_ms).unwrap_or(-1.0)
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_get_god_mode(obj: *mut std::ffi::c_void) -> bool {
    let probe = unsafe { &mut *(obj as *mut GameHost) };
    #[cfg(debug_assertions)]
    return probe.real_game_app_mut().map(super::native_menu::god_mode).unwrap_or(false);
    #[cfg(not(debug_assertions))]
    { let _ = probe; false }
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_set_god_mode(obj: *mut std::ffi::c_void, on: bool) {
    let probe = unsafe { &mut *(obj as *mut GameHost) };
    #[cfg(debug_assertions)]
    if let Some(app) = probe.real_game_app_mut() {
        super::native_menu::set_god_mode(app, on);
    }
    #[cfg(not(debug_assertions))]
    { let _ = (probe, on); }
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_get_noclip(obj: *mut std::ffi::c_void) -> bool {
    let probe = unsafe { &mut *(obj as *mut GameHost) };
    #[cfg(debug_assertions)]
    return probe.real_game_app_mut().map(super::native_menu::noclip).unwrap_or(false);
    #[cfg(not(debug_assertions))]
    { let _ = probe; false }
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_set_noclip(obj: *mut std::ffi::c_void, on: bool) {
    let probe = unsafe { &mut *(obj as *mut GameHost) };
    #[cfg(debug_assertions)]
    if let Some(app) = probe.real_game_app_mut() {
        super::native_menu::set_noclip(app, on);
    }
    #[cfg(not(debug_assertions))]
    { let _ = (probe, on); }
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_get_freeze_zombies(obj: *mut std::ffi::c_void) -> bool {
    let probe = unsafe { &mut *(obj as *mut GameHost) };
    #[cfg(debug_assertions)]
    return probe.real_game_app_mut().map(super::native_menu::freeze_zombies).unwrap_or(false);
    #[cfg(not(debug_assertions))]
    { let _ = probe; false }
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_set_freeze_zombies(obj: *mut std::ffi::c_void, on: bool) {
    let probe = unsafe { &mut *(obj as *mut GameHost) };
    #[cfg(debug_assertions)]
    if let Some(app) = probe.real_game_app_mut() {
        super::native_menu::set_freeze_zombies(app, on);
    }
    #[cfg(not(debug_assertions))]
    { let _ = (probe, on); }
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_get_autopilot(obj: *mut std::ffi::c_void) -> bool {
    let probe = unsafe { &mut *(obj as *mut GameHost) };
    probe.real_game_app_mut().map(super::native_menu::autopilot).unwrap_or(true)
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_set_autopilot(obj: *mut std::ffi::c_void, on: bool) {
    let probe = unsafe { &mut *(obj as *mut GameHost) };
    if let Some(app) = probe.real_game_app_mut() {
        super::native_menu::set_autopilot(app, on);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_trigger_kill_all_zombies(obj: *mut std::ffi::c_void) {
    let probe = unsafe { &mut *(obj as *mut GameHost) };
    #[cfg(debug_assertions)]
    if let Some(app) = probe.real_game_app_mut() {
        super::native_menu::trigger_kill_all_zombies(app);
    }
    #[cfg(not(debug_assertions))]
    { let _ = probe; }
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_trigger_spawn_more_zombies(obj: *mut std::ffi::c_void) {
    let probe = unsafe { &mut *(obj as *mut GameHost) };
    #[cfg(debug_assertions)]
    if let Some(app) = probe.real_game_app_mut() {
        super::native_menu::trigger_spawn_more_zombies(app);
    }
    #[cfg(not(debug_assertions))]
    { let _ = probe; }
}

macro_rules! bool_toggle_ffi {
    ($get_ffi:ident, $set_ffi:ident, $get:path, $set:path) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn $get_ffi(obj: *mut std::ffi::c_void) -> bool {
            let probe = unsafe { &mut *(obj as *mut GameHost) };
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
            let probe = unsafe { &mut *(obj as *mut GameHost) };
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
    native_metal_probe_get_player_model_visible,
    native_metal_probe_set_player_model_visible,
    super::native_menu::player_model_visible,
    super::native_menu::set_player_model_visible
);

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_get_zombie_target_count(obj: *mut std::ffi::c_void) -> u32 {
    let probe = unsafe { &mut *(obj as *mut GameHost) };
    #[cfg(debug_assertions)]
    return probe.real_game_app_mut().map(super::native_menu::zombie_target_count).unwrap_or(10);
    #[cfg(not(debug_assertions))]
    {
        let _ = probe;
        10
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_get_live_zombie_count(obj: *mut std::ffi::c_void) -> u32 {
    let probe = unsafe { &mut *(obj as *mut GameHost) };
    probe.real_game_app_mut().map(super::native_menu::live_zombie_count).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_get_kill_count(obj: *mut std::ffi::c_void) -> u32 {
    let probe = unsafe { &mut *(obj as *mut GameHost) };
    probe.real_game_app_mut().map(super::native_menu::kill_count).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn native_metal_probe_zombie_count_dec(obj: *mut std::ffi::c_void) {
    let probe = unsafe { &mut *(obj as *mut GameHost) };
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
    let probe = unsafe { &mut *(obj as *mut GameHost) };
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
    let probe = unsafe { &mut *(obj as *mut GameHost) };
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
    let probe = unsafe { &mut *(obj as *mut GameHost) };
    #[cfg(debug_assertions)]
    if let Some(app) = probe.real_game_app_mut() {
        super::native_menu::near_death_nearest_zombie(app);
    }
    #[cfg(not(debug_assertions))]
    {
        let _ = probe;
    }
}

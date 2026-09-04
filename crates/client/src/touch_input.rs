// --- Touch controls (iOS/Android) ---
// Extracted from lib.rs (Phase 1 of the gameplay-code-organization pass).
// Every item here is `pub(crate)` so lib.rs's single master `.chain()`'d
// Update system list (unchanged by this pass — pure code motion, no
// execution-order change) can still reference these by name.
//
// `use crate::*;` below pulls in every crate-root item this module needs
// (constants, other components/resources, helper fns) — root-private
// items ARE visible to a descendant module like this one per Rust's
// normal privacy rules, just not name-resolved without an explicit
// `use`/path, hence the glob rather than hunting down each one by hand.

use bevy::input::touch::Touches;
use bevy::prelude::*;
use crate::*;

// Auto-on for iOS/Android builds; TOUCH_UI_PREVIEW=1 forces it on under a
// desktop build too, since this dev environment has no physical
// device/simulator to build+run the real mobile targets on — this is the
// only way to visually iterate on the touch UI/systems from here.
#[derive(Resource)]
pub(crate) struct TouchControlsEnabled(pub(crate) bool);

#[derive(Component)]
pub(crate) struct TouchJoystickBase;
#[derive(Component)]
pub(crate) struct TouchJoystickKnob;
#[derive(Component)]
pub(crate) struct TouchScopeZoomSliderTrack;
#[derive(Component)]
pub(crate) struct TouchScopeZoomSliderHandle;
#[derive(Component)]
pub(crate) struct TouchFireButton;
#[derive(Component)]
pub(crate) struct TouchAimButton;

#[derive(Component)]
pub(crate) struct TouchReloadButton;
#[derive(Component)]
pub(crate) struct TouchJumpButton;
#[derive(Component)]
pub(crate) struct TouchCrouchButton;
#[derive(Component)]
pub(crate) struct TouchSprintButton;
#[derive(Component)]
pub(crate) struct TouchProneButton;
#[derive(Component)]
pub(crate) struct TouchLeanLeftButton;
#[derive(Component)]
pub(crate) struct TouchLeanRightButton;

/// Marks every real touch-control button (plus the settings-menu toggle) with
/// its own on-screen label, purely so the cadisplaylink Metal renderer
/// (native_game_extract.rs) can extract "draw this button, with this text"
/// generically — one query over `TouchControlButton` instead of a separate
/// per-marker-type query for each of the 10 buttons. The label here is a
/// second source of truth for the same text already spawned as a `Text`
/// child below (kept in sync by hand at each spawn site) rather than the
/// renderer re-reading that child `Text` component, since these labels are
/// static and never change — an empty string (the fire button) means "draw
/// the button shape, no text".
// `.0` is only ever read by the cadisplaylink Metal renderer
// (native_game_extract.rs, iOS-only) — desktop/normal-wgpu builds never
// read it (bevy_ui already draws real Text children there), hence the
// allow: this is a real cross-platform consumer, not dead code.
#[derive(Component)]
pub(crate) struct TouchControlButton(#[allow(dead_code)] pub(crate) &'static str);

// Which touch (Bevy's own per-touch id) is currently driving the joystick,
// and where it started — drag-from-start is the whole joystick model, no
// separate "return to center" animation needed since releasing just drops
// the tracked id (see touch_move_axis).
#[derive(Resource, Default)]
pub(crate) struct TouchMoveState {
    touch_id: Option<u64>,
    origin: Vec2,
}

// Computed fresh each frame by touch_move_axis, read by send_player_input
// exactly like the WASD-derived Vec2 it already builds — same units (x =
// right, y = forward), so send_player_input just adds the two together.
#[derive(Resource, Default)]
pub(crate) struct TouchMoveAxis(pub(crate) Vec2);

#[derive(Resource, Default)]
pub(crate) struct TouchLookState {
    touch_id: Option<u64>,
    last_pos: Vec2,
}

// Which touch is currently dragging the scope-zoom slider — same
// commit-a-touch-id-until-release shape as TouchMoveState, but there's no
// separate "axis" resource since this system writes straight into the
// real ScopeZoom.0 (the slider's whole value range already IS the zoom
// range, unlike movement which composes a drag delta with WASD).
#[derive(Resource, Default)]
pub(crate) struct TouchScopeZoomState {
    touch_id: Option<u64>,
}

#[derive(Resource, Default)]
pub(crate) struct TouchFiring(pub(crate) bool);

// Toggled by TouchAimButton (touch_aim_button) — unlike TouchFiring (held),
// this is a toggle since there's no natural "hold to aim" gesture that
// doesn't also fight with the fire button under a thumb. Read by every
// is_aiming() call site the same way TouchFiring is read by shoot().
#[derive(Resource, Default)]
pub(crate) struct TouchAiming(pub(crate) bool);

// One-frame pulse, not a held state: touch_jump_button resets this to false
// every tick and only sets it true on the exact frame TouchJumpButton's
// Interaction changes to Pressed, matching keys.just_pressed(KeyCode::Space)
// (the real game's jump gate — see send_player_input) rather than
// keys.pressed (which would jump repeatedly every frame the button stays
// down). Generic across the real game and minimal_map_probe_main, which
// each consume it differently (physics jump vs. a simple bounce).
#[derive(Resource, Default)]
pub(crate) struct TouchJump(pub(crate) bool);

// Same one-frame-pulse shape as TouchJump — reload_gun ORs this in
// alongside keys.just_pressed(KeyCode::KeyR).
#[derive(Resource, Default)]
pub(crate) struct TouchReload(pub(crate) bool);

// Held level (not a pulse) — same "no Changed<> gate, read every tick"
// shape as update_noclip_fly_input's own jump/crouch reads, since sprint is
// a continuous hold, not a toggle. send_player_input ORs this in alongside
// keys.pressed(KeyCode::ShiftLeft).
#[derive(Resource, Default)]
pub(crate) struct TouchSprintHeld(pub(crate) bool);

// Same held-level shape as TouchSprintHeld — apply_lean_input ORs each side
// in alongside keys.pressed(KeyCode::KeyQ)/KeyE.
#[derive(Resource, Default)]
pub(crate) struct TouchLeanHeld {
    pub(crate) left: bool,
    pub(crate) right: bool,
}

// Touch's up/down hold state for noclip flight (Jump = fly up, Crouch =
// fly down) — bundled into one resource/system (update_noclip_fly_input)
// rather than two separate ones so send_player_input only needs one extra
// param (it's already at Bevy's 16-param system function limit; see
// HitMarkerAssets' own doc comment for the same constraint hit elsewhere).
// Unlike TouchJump (one-frame pulse matching keys.just_pressed, for a real
// jump), these track whether the button is CURRENTLY held, matching
// keys.pressed(KeyCode::Space)-style continuous hold — touch has no
// keyboard hold state to read otherwise. touch_crouch_button (the normal-
// gameplay stance toggle) skips its own toggle while noclipping, so
// tapping Crouch during noclip doesn't leave PlayerStance stuck on
// Crouching once noclip is turned back off.
#[derive(Resource, Default)]
pub(crate) struct NoclipFlyInput {
    pub(crate) up: bool,
    pub(crate) down: bool,
}

// Bundles 3 touch-input resource reads send_player_input needs into one
// param — that fn was already at Bevy's 16-param system function limit
// (see HitMarkerAssets' own doc comment for the same constraint hit
// elsewhere), and NoclipFlyInput would have pushed it over.
#[derive(SystemParam)]
pub(crate) struct TouchGameplayInputs<'w> {
    pub(crate) aiming: Res<'w, TouchAiming>,
    pub(crate) jump: Res<'w, TouchJump>,
    pub(crate) sprint: Res<'w, TouchSprintHeld>,
    pub(crate) noclip_fly: Res<'w, NoclipFlyInput>,
}

// bevy_ui's own ui_focus_system (the thing that normally drives Interaction)
// only ever looks at Touches::first_pressed_position() — a single,
// arbitrarily-chosen touch, full stop. On a touchscreen with two fingers
// down at once (e.g. holding the movement joystick while tapping Fire),
// whichever touch ui_focus_system picked as "first" gets normal hover/press
// handling, but every OTHER simultaneous touch is invisible to it — any
// button under a second finger silently never becomes Interaction::Pressed.
// This is a known community pain point, not a CADisplayLink-specific bug: a
// popular touch-joystick crate (SergioRibera/virtual_joystick) works around
// the exact same limitation by hand-testing Touches against node rects
// instead of relying on Interaction at all. This system does the same,
// narrower: it only forces Interaction::Pressed for actual Button entities
// hit by a touch OTHER than the "first" one ui_focus_system already
// manages — so it never fights ui_focus_system over the same touch, and
// never touches non-Button nodes (the joystick base/knob, which aren't
// Button, are naturally excluded by the With<Button> filter below).
// Clearing back to None on release is already handled for free: bevy_ui's
// own mouse_released check reacts to touches_input.any_just_released() —
// ANY touch ending, not just the first — and resets every Pressed node to
// None, so this system only ever needs to set, never clear.
// Runs first (see both call sites) so button-tap systems reading
// Changed<Interaction> later in the same Update tick see this frame's
// change immediately, not one frame late.
pub(crate) fn multi_touch_button_interaction(
    touches: Res<Touches>,
    primary_window: Query<&Window, With<PrimaryWindow>>,
    mut buttons: Query<(&ComputedNode, &UiGlobalTransform, &mut Interaction), With<Button>>,
) {
    let Some(first_id) = touches.iter().next().map(|t| t.id()) else {
        return;
    };
    let Ok(window) = primary_window.single() else {
        return;
    };
    // node.contains_point expects physical-pixel coordinates, same as
    // window.physical_cursor_position() — touch.position() is logical, so
    // needs the same scale-factor conversion ui_focus_system itself applies
    // to touches_input.first_pressed_position().
    let scale_factor = window.scale_factor();
    let other_touch_positions: Vec<Vec2> = touches
        .iter()
        .filter(|touch| touch.id() != first_id)
        .map(|touch| touch.position() * scale_factor)
        .collect();
    if other_touch_positions.is_empty() {
        return;
    }
    for (node, transform, mut interaction) in &mut buttons {
        if other_touch_positions.iter().any(|&pos| node.contains_point(*transform, pos)) {
            interaction.set_if_neq(Interaction::Pressed);
        }
    }
}

// Auto-hides touch controls (same set hide_ui_when_locked hides) after
// TOUCH_CONTROLS_IDLE_HIDE_SECS with no touch on screen — decluttering for
// screenshots/spectating, same idea as most mobile games' HUD auto-hide.
// Suppressed while the settings menu is open (an idle player reading the
// menu shouldn't watch its own buttons vanish out from under them) and
// reset instantly by any touch, anywhere on screen (not just a button —
// there'd be nothing to tap if the controls are already hidden).
#[derive(Resource, Default)]
pub(crate) struct TouchControlsIdleHidden(pub(crate) bool);

pub(crate) fn update_touch_controls_idle_hidden(
    time: Res<Time>,
    touches: Res<Touches>,
    settings_visible: Res<SettingsMenuVisible>,
    keys: Res<ButtonInput<KeyCode>>,
    mut idle_elapsed: Local<f32>,
    mut hidden: ResMut<TouchControlsIdleHidden>,
) {
    if settings_visible.0 || touches.iter().next().is_some() || keys.just_pressed(KeyCode::Backquote) {
        *idle_elapsed = 0.0;
        if hidden.0 {
            hidden.0 = false;
        }
        return;
    }
    *idle_elapsed += time.delta_secs();
    let should_hide = *idle_elapsed >= TOUCH_CONTROLS_IDLE_HIDE_SECS;
    if should_hide != hidden.0 {
        hidden.0 = should_hide;
    }
}

// Touch equivalent of scroll_settings_menu — drags ScrollPosition.y to
// follow a finger, native-scroll-view style (content moves with the
// finger: dragging up reveals more content below). Same MouseUiLocked
// gate as touch_move_axis/touch_look (see MouseUiLocked's own doc comment)
// so an AssistiveTouch-synthesized "touch" from a mouse click doesn't also
// drag the menu while a hardware mouse is meant to be the only pointer.
pub(crate) fn touch_scroll_settings_menu(
    touches: Res<Touches>,
    visible: Res<SettingsMenuVisible>,
    mouse_ui_locked: Res<MouseUiLocked>,
    windows: Query<&Window, With<PrimaryWindow>>,
    menu: Query<(&ComputedNode, &UiGlobalTransform), With<SettingsMenuRoot>>,
    mut state: ResMut<SettingsMenuScrollState>,
    mut scroll_pos: Query<&mut ScrollPosition, With<SettingsMenuRoot>>,
) {
    if !visible.0 || mouse_ui_locked.0 {
        state.touch_id = None;
        return;
    }
    let Ok(window) = windows.single() else {
        return;
    };
    let scale_factor = window.scale_factor();
    let Ok((node, transform)) = menu.single() else {
        return;
    };

    if let Some(id) = state.touch_id {
        if touches.get_pressed(id).is_none() {
            state.touch_id = None;
        }
    }
    if state.touch_id.is_none() {
        if let Some(touch) = touches
            .iter_just_pressed()
            .find(|touch| node.contains_point(*transform, touch.position() * scale_factor))
        {
            state.touch_id = Some(touch.id());
            state.last_pos_y = touch.position().y;
        }
    }

    let Some(touch) = state.touch_id.and_then(|id| touches.get_pressed(id)) else {
        return;
    };
    let delta_y = touch.position().y - state.last_pos_y;
    state.last_pos_y = touch.position().y;
    if delta_y == 0.0 {
        return;
    }
    let Ok(mut scroll_pos) = scroll_pos.single_mut() else {
        return;
    };
    scroll_pos.y = (scroll_pos.y - delta_y).max(0.0);
}

// Bottom-left joystick base+knob, bottom-right fire button — same
// absolute-Node/BackgroundColor style spawn_crosshair/spawn_health_ui
// already use elsewhere in setup(). The look region (right half of the
// screen) deliberately has no visible Node — see touch_look, which just
// compares touch x-position against window width/2 instead of hit-testing
// a UI element, so there's nothing to draw for it.
pub(crate) fn spawn_touch_controls(commands: &mut Commands) {
    commands
        .spawn((
            Node {
                width: Val::Px(TOUCH_JOYSTICK_BASE_PX),
                height: Val::Px(TOUCH_JOYSTICK_BASE_PX),
                position_type: PositionType::Absolute,
                left: Val::Px(32.0),
                bottom: Val::Px(32.0),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(TOUCH_JOYSTICK_BASE_PX / 2.0)),
                ..default()
            },
            // Alpha bumped from the original 0.15/0.35 (barely visible
            // against a bright in-game background — reported as "can't see
            // the buttons at all" on-device) plus an opaque-ish white border
            // outline so the button's outline reads even over a
            // similarly-light patch of scene behind it.
            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.3)),
            BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.6)),
            GlobalZIndex(STATIC_UI_Z_INDEX),
            TouchJoystickBase,
        ))
        .with_children(|base| {
            base.spawn((
                Node {
                    width: Val::Px(TOUCH_JOYSTICK_KNOB_PX),
                    height: Val::Px(TOUCH_JOYSTICK_KNOB_PX),
                    position_type: PositionType::Absolute,
                    left: Val::Px((TOUCH_JOYSTICK_BASE_PX - TOUCH_JOYSTICK_KNOB_PX) / 2.0),
                    top: Val::Px((TOUCH_JOYSTICK_BASE_PX - TOUCH_JOYSTICK_KNOB_PX) / 2.0),
                    border_radius: BorderRadius::all(Val::Px(TOUCH_JOYSTICK_KNOB_PX / 2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.55)),
                TouchJoystickKnob,
            ));
        });

    commands.spawn((
        Node {
            width: Val::Px(TOUCH_FIRE_BUTTON_PX),
            height: Val::Px(TOUCH_FIRE_BUTTON_PX),
            position_type: PositionType::Absolute,
            right: Val::Px(32.0),
            bottom: Val::Px(48.0),
            border: UiRect::all(Val::Px(2.0)),
            border_radius: BorderRadius::all(Val::Px(TOUCH_FIRE_BUTTON_PX / 2.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(1.0, 0.3, 0.3, 0.55)),
        BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.6)),
        Button,
        Interaction::default(),
        GlobalZIndex(STATIC_UI_Z_INDEX),
        TouchFireButton,
        TouchControlButton(""),
    ));

    // Toggle (not hold) — see TouchAiming's own doc comment for why. Sits
    // just above the fire button, out of the way of a firing thumb.
    commands
        .spawn((
            Node {
                width: Val::Px(TOUCH_FIRE_BUTTON_PX * 0.7),
                height: Val::Px(TOUCH_FIRE_BUTTON_PX * 0.7),
                position_type: PositionType::Absolute,
                right: Val::Px(32.0 + (TOUCH_FIRE_BUTTON_PX - TOUCH_FIRE_BUTTON_PX * 0.7) / 2.0),
                bottom: Val::Px(48.0 + TOUCH_FIRE_BUTTON_PX + 16.0),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(TOUCH_FIRE_BUTTON_PX * 0.35)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.3, 0.5, 1.0, 0.4)),
            BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.6)),
            Button,
            Interaction::default(),
            GlobalZIndex(STATIC_UI_Z_INDEX),
            TouchAimButton,
            TouchControlButton("ADS"),
        ))
        .with_children(|btn| {
            btn.spawn((
                Text::new("ADS"),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });

    // Reload — to the left of Fire (same row/height), out of the way of
    // ADS above and a firing thumb still resting on Fire.
    commands
        .spawn((
            Node {
                width: Val::Px(TOUCH_FIRE_BUTTON_PX * 0.7),
                height: Val::Px(TOUCH_FIRE_BUTTON_PX * 0.7),
                position_type: PositionType::Absolute,
                right: Val::Px(32.0 + TOUCH_FIRE_BUTTON_PX + 16.0),
                bottom: Val::Px(48.0 + (TOUCH_FIRE_BUTTON_PX - TOUCH_FIRE_BUTTON_PX * 0.7) / 2.0),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(TOUCH_FIRE_BUTTON_PX * 0.35)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(1.0, 0.8, 0.2, 0.4)),
            BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.6)),
            Button,
            Interaction::default(),
            GlobalZIndex(STATIC_UI_Z_INDEX),
            TouchReloadButton,
            TouchControlButton("RELOAD"),
        ))
        .with_children(|btn| {
            btn.spawn((
                Text::new("RELOAD"),
                TextFont {
                    font_size: 10.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });

    // Jump/Crouch/Prone (stance row) sit just above the joystick, out of
    // the thumb's drag path (stays within TOUCH_JOYSTICK_RADIUS_PX of the
    // base) — 3-across rather than the original 2-across/3-down grid: on
    // a landscape phone the whole column's real height (joystick + every
    // row) was landing close enough to the top edge to overlap HealthUi's
    // own top:16 label (both are real ComputedNode rects now that both
    // draw via the Metal renderer — see native_metal_probe.rs's
    // touch-control/HUD draw loops — so this was actually visible, not
    // just theoretical). One fewer row buys back a full row's height.
    let stance_btn_px = TOUCH_FIRE_BUTTON_PX * 0.6;
    let stance_row_gap = 12.0;
    let stance_row_bottom = 32.0 + TOUCH_JOYSTICK_BASE_PX + 16.0;
    spawn_touch_round_button(commands, "JUMP", 32.0, stance_row_bottom, stance_btn_px, Color::srgba(0.3, 1.0, 0.4, 0.4), TouchJumpButton);
    spawn_touch_round_button(
        commands,
        "CROUCH",
        32.0 + (stance_btn_px + stance_row_gap),
        stance_row_bottom,
        stance_btn_px,
        Color::srgba(1.0, 0.8, 0.3, 0.4),
        TouchCrouchButton,
    );
    spawn_touch_round_button(
        commands,
        "PRONE",
        32.0 + (stance_btn_px + stance_row_gap) * 2.0,
        stance_row_bottom,
        stance_btn_px,
        Color::srgba(0.6, 0.5, 1.0, 0.4),
        TouchProneButton,
    );

    // Sprint/Lean (movement-modifier row) stacked once above the stance
    // row, same left-side column, out of the joystick's own drag path
    // (radius-limited — see TOUCH_JOYSTICK_RADIUS_PX) — same reasoning as
    // the stance row's own placement comment.
    let modifier_row_bottom = stance_row_bottom + stance_btn_px + stance_row_gap;
    spawn_touch_round_button(
        commands,
        "SPRINT",
        32.0,
        modifier_row_bottom,
        stance_btn_px,
        Color::srgba(1.0, 0.55, 0.15, 0.4),
        TouchSprintButton,
    );
    spawn_touch_round_button(
        commands,
        "LEAN <",
        32.0 + (stance_btn_px + stance_row_gap),
        modifier_row_bottom,
        stance_btn_px,
        Color::srgba(0.4, 0.85, 0.5, 0.4),
        TouchLeanLeftButton,
    );
    spawn_touch_round_button(
        commands,
        "LEAN >",
        32.0 + (stance_btn_px + stance_row_gap) * 2.0,
        modifier_row_bottom,
        stance_btn_px,
        Color::srgba(0.4, 0.85, 0.5, 0.4),
        TouchLeanRightButton,
    );

    // Scope-zoom slider — right edge, well above the Fire/ADS/Reload
    // cluster (bottom:48 + TOUCH_FIRE_BUTTON_PX + margin), so a thumb
    // reaching up from Fire doesn't cross it. Hidden by default
    // (Display::None) — only relevant while ADS'd, toggled to Flex by
    // update_scope_zoom_slider_visibility the same way update_crosshair
    // toggles CrosshairRoot off while aiming (just inverted: this one
    // shows on aim instead of hiding).
    commands
        .spawn((
            Node {
                width: Val::Px(TOUCH_SCOPE_ZOOM_TRACK_WIDTH_PX),
                height: Val::Px(TOUCH_SCOPE_ZOOM_TRACK_HEIGHT_PX),
                position_type: PositionType::Absolute,
                right: Val::Px(24.0),
                bottom: Val::Px(48.0 + TOUCH_FIRE_BUTTON_PX + 48.0),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(TOUCH_SCOPE_ZOOM_TRACK_WIDTH_PX / 2.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.3)),
            BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.6)),
            GlobalZIndex(STATIC_UI_Z_INDEX),
            TouchScopeZoomSliderTrack,
        ))
        .with_children(|track| {
            track.spawn((
                Node {
                    width: Val::Px(TOUCH_SCOPE_ZOOM_HANDLE_PX),
                    height: Val::Px(TOUCH_SCOPE_ZOOM_HANDLE_PX),
                    position_type: PositionType::Absolute,
                    left: Val::Px((TOUCH_SCOPE_ZOOM_TRACK_WIDTH_PX - TOUCH_SCOPE_ZOOM_HANDLE_PX) / 2.0),
                    top: Val::Px(0.0),
                    border_radius: BorderRadius::all(Val::Px(TOUCH_SCOPE_ZOOM_HANDLE_PX / 2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.55)),
                TouchScopeZoomSliderHandle,
            ));
        });
}

// Shared helper for the small round left/right/right-labeled touch buttons
// (Jump, Crouch) — mirrors the TouchAimButton node built by hand just above,
// factored out since there are now two more of these with only position/
// label/color/marker differing.
pub(crate) fn spawn_touch_round_button(commands: &mut Commands, label: &'static str, left_px: f32, bottom_px: f32, size_px: f32, color: Color, marker: impl Component) {
    commands
        .spawn((
            Node {
                width: Val::Px(size_px),
                height: Val::Px(size_px),
                position_type: PositionType::Absolute,
                left: Val::Px(left_px),
                bottom: Val::Px(bottom_px),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(size_px / 2.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(color),
            BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.6)),
            Button,
            Interaction::default(),
            GlobalZIndex(STATIC_UI_Z_INDEX),
            marker,
            TouchControlButton(label),
        ))
        .with_children(|btn| {
            btn.spawn((
                Text::new(label),
                TextFont {
                    font_size: 10.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });
}

// Toggles TouchAiming on the instant a tap lands (Changed<Interaction>, same
// debounce pattern as toggle_settings_menu — without it, holding the button
// down would flip the toggle every single frame instead of once per tap).
pub(crate) fn touch_aim_button(mut aiming: ResMut<TouchAiming>, button: Query<&Interaction, (With<TouchAimButton>, Changed<Interaction>)>) {
    for interaction in &button {
        if *interaction == Interaction::Pressed {
            aiming.0 = !aiming.0;
        }
    }
}

// Resets to false every tick and only goes true on the exact frame the
// button's Interaction changes to Pressed — see TouchJump's own doc comment
// for why this has to be an edge, not a held level. Shared by the real game
// and minimal_map_probe_main; each has its own consumer of TouchJump.
pub(crate) fn touch_jump_button(mut jump: ResMut<TouchJump>, button: Query<&Interaction, (With<TouchJumpButton>, Changed<Interaction>)>) {
    jump.0 = button.iter().any(|interaction| *interaction == Interaction::Pressed);
}

// Same one-frame-pulse shape as touch_jump_button.
pub(crate) fn touch_reload_button(mut reload: ResMut<TouchReload>, button: Query<&Interaction, (With<TouchReloadButton>, Changed<Interaction>)>) {
    reload.0 = button.iter().any(|interaction| *interaction == Interaction::Pressed);
}

// Same tap-edge toggle as touch_aim_button, mirroring toggle_player_stance's
// own KeyC branch (Standing<->Crouching, switches straight over from Prone
// too) so a touch tap and a desktop KeyC press behave identically. Skipped
// entirely while noclipping — send_player_input reads TouchCrouchHeld
// directly for fly-down instead, so tapping Crouch during noclip doesn't
// leave PlayerStance stuck on Crouching once noclip is turned back off.
pub(crate) fn touch_crouch_button(
    noclip: Res<Noclip>,
    button: Query<&Interaction, (With<TouchCrouchButton>, Changed<Interaction>)>,
    mut player: Query<&mut PlayerStance, With<Player>>,
) {
    if noclip.0 {
        return;
    }
    for interaction in &button {
        if *interaction == Interaction::Pressed {
            let Ok(mut stance) = player.single_mut() else {
                return;
            };
            *stance = if *stance == PlayerStance::Crouching {
                PlayerStance::Standing
            } else {
                PlayerStance::Crouching
            };
        }
    }
}

// Same tap-edge toggle as touch_crouch_button, mirroring toggle_player_
// stance's own KeyX branch (Standing<->Prone, switches straight over from
// Crouching too) so a touch tap and a desktop KeyX press behave
// identically. No noclip skip needed — unlike Crouch, Prone has no second
// job driving noclip fly-down.
pub(crate) fn touch_prone_button(button: Query<&Interaction, (With<TouchProneButton>, Changed<Interaction>)>, mut player: Query<&mut PlayerStance, With<Player>>) {
    for interaction in &button {
        if *interaction == Interaction::Pressed {
            let Ok(mut stance) = player.single_mut() else {
                return;
            };
            *stance = if *stance == PlayerStance::Prone { PlayerStance::Standing } else { PlayerStance::Prone };
        }
    }
}

// Held level, same "no Changed<> gate, read every tick" shape as
// update_noclip_fly_input's own jump/crouch reads — sprint needs a
// continuous held state, not an edge/toggle.
pub(crate) fn touch_sprint_button(mut sprint: ResMut<TouchSprintHeld>, button: Query<&Interaction, With<TouchSprintButton>>) {
    sprint.0 = button.iter().any(|interaction| *interaction == Interaction::Pressed);
}

// Same held-level shape as touch_sprint_button, one bool per side.
pub(crate) fn touch_lean_buttons(mut lean: ResMut<TouchLeanHeld>, left_button: Query<&Interaction, With<TouchLeanLeftButton>>, right_button: Query<&Interaction, With<TouchLeanRightButton>>) {
    lean.left = left_button.iter().any(|interaction| *interaction == Interaction::Pressed);
    lean.right = right_button.iter().any(|interaction| *interaction == Interaction::Pressed);
}

// Claims whichever touch starts on the left half of the screen as the
// movement touch, tracks it while held, and writes the drag-from-origin
// vector (clamped/normalized to TOUCH_JOYSTICK_RADIUS_PX) into
// TouchMoveAxis — send_player_input adds this straight into the same
// move_axis WASD builds. Dropping the touch (finger lifted) snaps back to
// zero; there's no separate "spring back to center" tween.
// True if this touch is currently hovering/pressing any UI element (a
// settings-menu button, in practice — nothing else in this game's UI sits
// over the movement/look halves of the screen). Without this check,
// touch_move_axis/touch_look would ALSO claim the exact same touch a
// button tap uses as a gameplay input (a settings-menu button lives on the
// left half, right where the joystick claims any new touch) — the button's
// own Interaction still fires correctly either way, but the joystick
// snapping to a touch that's really tapping a menu button (and staying
// claimed after release, since the touch never started as a "real"
// movement drag) was the actual cause of the settings menu closing itself
// right back up on the very next tap.
// Only counts a hover as "real UI" if the hovered entity also has
// Interaction (i.e. an actual Button) — every Node is hoverable by default
// in bevy_picking regardless of whether it's interactive, so without this
// filter, touching purely decorative UI (crosshair, health bar text, even
// the joystick's own base/knob visuals, which have no Button either) would
// ALSO count as "over UI" and block touch_move_axis/touch_look, including
// the joystick blocking its own drag.
// Real replacement for what used to be a `touch_over_ui` check built on
// bevy_picking's `HoverMap` — that map only ever gets populated by
// bevy_picking's own `touch_pick_events` system, which reads
// `WindowEvent::TouchInput`, and this app only ever emits the raw
// `TouchInput` message (see ffi.rs's `touched()`), so `HoverMap` stayed
// permanently empty and that check always returned false. Concretely this
// meant touch_move_axis/touch_look claimed a touch that started ON TOP of
// a real Button — the Fire button, the scope-zoom slider's own handle
// area, Jump/Crouch/Lean/etc — as a joystick/look-drag too, on top of that
// button's own tap/hold handling: holding Fire and dragging the same
// finger fired AND spun the camera, same for the zoom slider. Same
// ComputedNode::contains_point rect-check technique
// multi_touch_button_interaction/touch_over_settings_menu already use,
// just generalized to "any real Button", not one specific node.
pub(crate) fn touch_over_touch_button(pos_physical: Vec2, buttons: &Query<(&ComputedNode, &UiGlobalTransform), With<Button>>) -> bool {
    buttons.iter().any(|(node, transform)| node.contains_point(*transform, pos_physical))
}

// The scope-zoom slider (TouchScopeZoomSliderTrack) needs its own check
// alongside touch_over_touch_button above — it's deliberately NOT a
// `Button` (touch_scope_zoom_drag reads raw Touches/rect-contains itself,
// not Interaction), so the generic Button-based check above can't see it.
// Gated on `touch_aiming` (same condition touch_scope_zoom_drag itself
// checks before doing anything) rather than trusting the track's
// Display::None-when-hidden state alone — bevy_ui doesn't recompute
// layout for a Display::None node, so its ComputedNode can retain a
// stale, real-looking rect from the last frame it WAS visible instead of
// collapsing to zero size.
pub(crate) fn touch_over_scope_zoom_slider(
    pos_physical: Vec2,
    touch_aiming: bool,
    track: &Query<(&ComputedNode, &UiGlobalTransform), With<TouchScopeZoomSliderTrack>>,
) -> bool {
    touch_aiming && track.single().is_ok_and(|(node, transform)| node.contains_point(*transform, pos_physical))
}

pub(crate) fn touch_over_settings_menu(
    pos_physical: Vec2,
    visible: &SettingsMenuVisible,
    menu: &Query<(&ComputedNode, &UiGlobalTransform), With<SettingsMenuRoot>>,
) -> bool {
    visible.0 && menu.single().is_ok_and(|(node, transform)| node.contains_point(*transform, pos_physical))
}

pub(crate) fn touch_move_axis(
    touches: Res<Touches>,
    windows: Query<&Window, With<PrimaryWindow>>,
    buttons: Query<(&ComputedNode, &UiGlobalTransform), With<Button>>,
    mouse_ui_locked: Res<MouseUiLocked>,
    settings_visible: Res<SettingsMenuVisible>,
    settings_menu: Query<(&ComputedNode, &UiGlobalTransform), With<SettingsMenuRoot>>,
    mut state: ResMut<TouchMoveState>,
    mut axis: ResMut<TouchMoveAxis>,
) {
    // AssistiveTouch (the only way to use a Bluetooth mouse on iPhone —
    // see GameControllerInput.swift) synthesizes real UITouch events at the
    // pointer's location on click, which this system sees exactly like a
    // finger tap — MouseUiLocked exists so a mouse click can't drive
    // movement via the joystick zone this way.
    if mouse_ui_locked.0 {
        state.touch_id = None;
        axis.0 = Vec2::ZERO;
        return;
    }
    let Ok(window) = windows.single() else {
        axis.0 = Vec2::ZERO;
        return;
    };
    let half_width = window.width() / 2.0;
    let scale_factor = window.scale_factor();

    if let Some(id) = state.touch_id {
        if touches.get_pressed(id).is_none() {
            state.touch_id = None;
        }
    }
    if state.touch_id.is_none() {
        if let Some(touch) = touches.iter_just_pressed().find(|touch| {
            touch.position().x < half_width
                && !touch_over_touch_button(touch.position() * scale_factor, &buttons)
                && !touch_over_settings_menu(touch.position() * scale_factor, &settings_visible, &settings_menu)
        }) {
            state.touch_id = Some(touch.id());
            state.origin = touch.position();
        }
    }

    axis.0 = match state.touch_id.and_then(|id| touches.get_pressed(id)) {
        Some(touch) => {
            let drag = (touch.position() - state.origin).clamp_length_max(TOUCH_JOYSTICK_RADIUS_PX) / TOUCH_JOYSTICK_RADIUS_PX;
            // Screen Y grows downward; move_axis.y is "forward" (matches
            // WASD's KeyW = +1.0), so this flips the sign.
            Vec2::new(drag.x, -drag.y)
        }
        None => Vec2::ZERO,
    };
}

// Purely cosmetic: slides the joystick knob to follow the tracked drag so
// there's visual feedback, independent of the actual TouchMoveAxis math
// above.
pub(crate) fn update_joystick_knob_visual(state: Res<TouchMoveState>, touches: Res<Touches>, mut knob: Query<&mut Node, With<TouchJoystickKnob>>) {
    let Ok(mut node) = knob.single_mut() else {
        return;
    };
    let offset = match state.touch_id.and_then(|id| touches.get_pressed(id)) {
        Some(touch) => (touch.position() - state.origin).clamp_length_max(TOUCH_JOYSTICK_RADIUS_PX),
        None => Vec2::ZERO,
    };
    let center = (TOUCH_JOYSTICK_BASE_PX - TOUCH_JOYSTICK_KNOB_PX) / 2.0;
    node.left = Val::Px(center + offset.x);
    node.top = Val::Px(center + offset.y);
}

// Drags ScopeZoom directly between SCOPE_MIN_ZOOM (bottom of the track)
// and SCOPE_MAX_ZOOM (top) — the touch equivalent of desktop's scroll
// wheel (adjust_scope_zoom). Deliberately a separate system/resource
// rather than folded into adjust_scope_zoom itself: that system's own
// early-return (not aiming) and autopilot auto-zoom branch already gate
// out everything this needs to skip, and since this only ever WRITES
// zoom.0 (never reads it), running before or after adjust_scope_zoom in
// the frame doesn't matter — whichever runs last simply wins, same as
// scroll-wheel vs. touch-drag would on any platform that had both.
pub(crate) fn touch_scope_zoom_drag(
    touches: Res<Touches>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mouse_ui_locked: Res<MouseUiLocked>,
    touch_aiming: Res<TouchAiming>,
    autopilot: Res<Autopilot>,
    track: Query<(&ComputedNode, &UiGlobalTransform), With<TouchScopeZoomSliderTrack>>,
    mut state: ResMut<TouchScopeZoomState>,
    mut zoom: ResMut<ScopeZoom>,
) {
    // Only while manually ADS'd on touch — the slider is hidden
    // (Display::None, see update_scope_zoom_slider_visibility) whenever
    // not aiming anyway, and autopilot drives its own auto-zoom in
    // adjust_scope_zoom (see that fn's own doc comment on why it checks
    // Autopilot.0 rather than trusting a possibly-stale target).
    // AssistiveTouch guard (MouseUiLocked) mirrors touch_move_axis's own.
    if !touch_aiming.0 || autopilot.0 || mouse_ui_locked.0 {
        state.touch_id = None;
        return;
    }
    let (Ok(window), Ok((node, transform))) = (windows.single(), track.single()) else {
        return;
    };
    let scale_factor = window.scale_factor();

    if let Some(id) = state.touch_id {
        if touches.get_pressed(id).is_none() {
            state.touch_id = None;
        }
    }
    if state.touch_id.is_none() {
        if let Some(touch) = touches
            .iter_just_pressed()
            .find(|touch| node.contains_point(*transform, touch.position() * scale_factor))
        {
            state.touch_id = Some(touch.id());
        }
    }
    let Some(touch) = state.touch_id.and_then(|id| touches.get_pressed(id)) else {
        return;
    };
    // ComputedNode::size()/UiGlobalTransform's translation are both
    // physical pixels (see multi_touch_button_interaction's own doc
    // comment); touch.position() is logical — inverse_scale_factor
    // converts the node's own rect to logical to compare directly,
    // same technique native_game_extract.rs's logical_rect uses.
    let logical_height = node.size().y * node.inverse_scale_factor();
    let logical_top = transform.affine().translation.y * node.inverse_scale_factor() - logical_height / 2.0;
    let t = ((touch.position().y - logical_top) / logical_height).clamp(0.0, 1.0);
    // t=0 (touch at the track's top) -> SCOPE_MAX_ZOOM; t=1 (bottom) ->
    // SCOPE_MIN_ZOOM — dragging UP zooms IN, matching a real optic's zoom
    // ring (twist/slide away from you = further/more magnified) better
    // than the opposite would.
    zoom.0 = SCOPE_MAX_ZOOM + (SCOPE_MIN_ZOOM - SCOPE_MAX_ZOOM) * t;
}

// Purely cosmetic, mirrors update_joystick_knob_visual — slides the
// handle to reflect the CURRENT ScopeZoom.0 regardless of what's driving
// it (touch drag above, desktop scroll wheel, or autopilot's own
// auto-zoom in adjust_scope_zoom), so the slider always reads correctly
// even on frames this touch isn't actively dragging it.
pub(crate) fn update_scope_zoom_slider_visual(zoom: Res<ScopeZoom>, mut handle: Query<&mut Node, With<TouchScopeZoomSliderHandle>>) {
    let Ok(mut node) = handle.single_mut() else {
        return;
    };
    let t = ((zoom.0 - SCOPE_MIN_ZOOM) / (SCOPE_MAX_ZOOM - SCOPE_MIN_ZOOM)).clamp(0.0, 1.0);
    // Inverse of touch_scope_zoom_drag's own t (max zoom = top of track);
    // handle top position ranges from 0 (track top) to track_height -
    // handle_height (track bottom).
    let travel = TOUCH_SCOPE_ZOOM_TRACK_HEIGHT_PX - TOUCH_SCOPE_ZOOM_HANDLE_PX;
    node.top = Val::Px(travel * (1.0 - t));
}

// Mirror of mouse_look for the right half of the screen — writes into the
// exact same FpsCamera.yaw/pitch and MouseLookDelta, so everything
// downstream (apply_view_recoil composing the final camera rotation,
// recoil kick, etc.) doesn't need to know or care whether the drag came
// from a mouse or a finger.
pub(crate) fn touch_look(
    touches: Res<Touches>,
    windows: Query<&Window, With<PrimaryWindow>>,
    buttons: Query<(&ComputedNode, &UiGlobalTransform), With<Button>>,
    touch_aiming: Res<TouchAiming>,
    zoom_slider_track: Query<(&ComputedNode, &UiGlobalTransform), With<TouchScopeZoomSliderTrack>>,
    mouse_ui_locked: Res<MouseUiLocked>,
    settings_visible: Res<SettingsMenuVisible>,
    settings_menu: Query<(&ComputedNode, &UiGlobalTransform), With<SettingsMenuRoot>>,
    mut state: ResMut<TouchLookState>,
    mut look_delta: ResMut<MouseLookDelta>,
    sensitivity_scale: Res<SensitivityScale>,
    mut query: Query<&mut FpsCamera>,
    autopilot: Res<Autopilot>,
    look_only: Res<AutopilotLookOnly>,
) {
    // See mouse_look's own doc comment — same reasoning applies here: real
    // touch-look input is ignored the whole time Autopilot (or its look-
    // only subset, AutopilotLookOnly) is on.
    if autopilot.0 || look_only.0 {
        return;
    }
    // See touch_move_axis's own comment on MouseUiLocked/AssistiveTouch —
    // same reasoning here so a mouse click on the right half doesn't also
    // yank the camera via the touch look-drag zone while GCMouse deltas
    // (mouse_look) are already driving it.
    if mouse_ui_locked.0 {
        state.touch_id = None;
        return;
    }
    let Ok(window) = windows.single() else {
        return;
    };
    let half_width = window.width() / 2.0;
    let scale_factor = window.scale_factor();

    if let Some(id) = state.touch_id {
        if touches.get_pressed(id).is_none() {
            state.touch_id = None;
        }
    }
    if state.touch_id.is_none() {
        if let Some(touch) = touches.iter_just_pressed().find(|touch| {
            let pos_physical = touch.position() * scale_factor;
            touch.position().x >= half_width
                && !touch_over_touch_button(pos_physical, &buttons)
                && !touch_over_scope_zoom_slider(pos_physical, touch_aiming.0, &zoom_slider_track)
                && !touch_over_settings_menu(pos_physical, &settings_visible, &settings_menu)
        }) {
            state.touch_id = Some(touch.id());
            state.last_pos = touch.position();
        }
    }

    let Some(touch) = state.touch_id.and_then(|id| touches.get_pressed(id)) else {
        return;
    };
    let delta = touch.position() - state.last_pos;
    state.last_pos = touch.position();
    if delta == Vec2::ZERO {
        return;
    }

    look_delta.0 = delta;
    let sensitivity = MOUSE_SENSITIVITY * TOUCH_LOOK_SENSITIVITY_MULTIPLIER * sensitivity_scale.0;
    for mut cam in &mut query {
        cam.yaw -= delta.x * sensitivity;
        cam.pitch = (cam.pitch - delta.y * sensitivity).clamp(-PITCH_CLAMP, PITCH_CLAMP);
    }
}

// Held (not just tapped) → auto-fires the same way holding left-click does
// on desktop, via shoot()'s own FIRE_RATE cooldown.
pub(crate) fn touch_fire_button(mut firing: ResMut<TouchFiring>, buttons: Query<&Interaction, With<TouchFireButton>>) {
    firing.0 = buttons.iter().any(|interaction| matches!(interaction, Interaction::Pressed));
}


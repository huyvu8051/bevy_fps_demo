
use bevy::input::touch::Touches;
use bevy::picking::hover::HoverMap;
use bevy::picking::pointer::PointerId;
use bevy::prelude::*;
use crate::*;

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

#[derive(Component)]
pub(crate) struct TouchControlButton;

/// The button's un-pressed `BackgroundColor`, so the press-brighten effect
/// can always compute from a stable base instead of compounding onto itself.
#[derive(Component)]
pub(crate) struct TouchButtonBaseColor(pub(crate) Color);

pub(crate) fn update_touch_button_press_visual(mut buttons: Query<(&Interaction, &TouchButtonBaseColor, &mut BackgroundColor), With<TouchControlButton>>) {
    for (interaction, base, mut bg) in &mut buttons {
        let c = base.0.to_srgba();
        let alpha = if *interaction == Interaction::Pressed { (c.alpha * 1.6).min(1.0) } else { c.alpha };
        bg.0 = Color::srgba(c.red, c.green, c.blue, alpha);
    }
}

#[derive(Resource, Default)]
pub(crate) struct TouchMoveState {
    touch_id: Option<u64>,
    origin: Vec2,
}

#[derive(Resource, Default)]
pub(crate) struct TouchMoveAxis(pub(crate) Vec2);

#[derive(Resource, Default)]
pub(crate) struct TouchLookState {
    touch_id: Option<u64>,
    last_pos: Vec2,
}

#[derive(Resource, Default)]
pub(crate) struct TouchScopeZoomState {
    touch_id: Option<u64>,
}

#[derive(Resource, Default)]
pub(crate) struct TouchFiring(pub(crate) bool);

#[derive(Resource, Default)]
pub(crate) struct TouchAiming(pub(crate) bool);

#[derive(Resource, Default)]
pub(crate) struct TouchJump(pub(crate) bool);

#[derive(Resource, Default)]
pub(crate) struct TouchReload(pub(crate) bool);

#[derive(Resource, Default)]
pub(crate) struct TouchSprintHeld(pub(crate) bool);

#[derive(Resource, Default)]
pub(crate) struct TouchLeanHeld {
    pub(crate) left: bool,
    pub(crate) right: bool,
}

#[derive(Resource, Default)]
pub(crate) struct NoclipFlyInput {
    pub(crate) up: bool,
    pub(crate) down: bool,
}

pub(crate) fn update_noclip_fly_input(
    mut fly: ResMut<NoclipFlyInput>,
    jump_button: Query<&Interaction, With<TouchJumpButton>>,
    crouch_button: Query<&Interaction, With<TouchCrouchButton>>,
) {
    fly.up = jump_button.iter().any(|interaction| *interaction == Interaction::Pressed);
    fly.down = crouch_button.iter().any(|interaction| *interaction == Interaction::Pressed);
}

#[derive(SystemParam)]
pub(crate) struct TouchGameplayInputs<'w> {
    pub(crate) aiming: Res<'w, TouchAiming>,
    pub(crate) jump: Res<'w, TouchJump>,
    pub(crate) sprint: Res<'w, TouchSprintHeld>,
    pub(crate) noclip_fly: Res<'w, NoclipFlyInput>,
}

pub(crate) fn suppress_touch_buttons_while_menu_open(
    menu_visible: Res<SettingsMenuVisible>,
    mut buttons: Query<&mut Interaction, (With<TouchControlButton>, Without<SettingsToggleButton>)>,
) {
    if !menu_visible.0 {
        return;
    }
    for mut interaction in &mut buttons {
        interaction.set_if_neq(Interaction::None);
    }
}

pub(crate) fn multi_touch_button_interaction(touches: Res<Touches>, hover_map: Res<HoverMap>, mut buttons: Query<&mut Interaction, With<Button>>) {
    let Some(first_id) = touches.iter().next().map(|t| t.id()) else {
        return;
    };
    for touch in touches.iter() {
        if touch.id() == first_id {
            continue;
        }
        let Some(hits) = hover_map.get(&PointerId::Touch(touch.id())) else {
            continue;
        };
        for &entity in hits.keys() {
            if let Ok(mut interaction) = buttons.get_mut(entity) {
                interaction.set_if_neq(Interaction::Pressed);
            }
        }
    }
}

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

            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.3)),
            BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.6)),
            GlobalZIndex(STATIC_UI_Z_INDEX),
            TouchJoystickBase,
            NativeUiRoot,
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
        TouchButtonBaseColor(Color::srgba(1.0, 0.3, 0.3, 0.55)),
        BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.6)),
        Button,
        Interaction::default(),
        GlobalZIndex(STATIC_UI_Z_INDEX),
        TouchFireButton,
        TouchControlButton,
        NativeUiRoot,
    ));

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
            TouchButtonBaseColor(Color::srgba(0.3, 0.5, 1.0, 0.4)),
            BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.6)),
            Button,
            Interaction::default(),
            GlobalZIndex(STATIC_UI_Z_INDEX),
            TouchAimButton,
            TouchControlButton,
            NativeUiRoot,
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
            TouchButtonBaseColor(Color::srgba(1.0, 0.8, 0.2, 0.4)),
            BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.6)),
            Button,
            Interaction::default(),
            GlobalZIndex(STATIC_UI_Z_INDEX),
            TouchReloadButton,
            TouchControlButton,
            NativeUiRoot,
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
            NativeUiRoot,
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
            TouchButtonBaseColor(color),
            BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.6)),
            Button,
            Interaction::default(),
            GlobalZIndex(STATIC_UI_Z_INDEX),
            marker,
            TouchControlButton,
            NativeUiRoot,
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

pub(crate) fn touch_aim_button(mut aiming: ResMut<TouchAiming>, button: Query<&Interaction, (With<TouchAimButton>, Changed<Interaction>)>) {
    for interaction in &button {
        if *interaction == Interaction::Pressed {
            aiming.0 = !aiming.0;
        }
    }
}

pub(crate) fn touch_jump_button(mut jump: ResMut<TouchJump>, button: Query<&Interaction, (With<TouchJumpButton>, Changed<Interaction>)>) {
    jump.0 = button.iter().any(|interaction| *interaction == Interaction::Pressed);
}

pub(crate) fn touch_reload_button(mut reload: ResMut<TouchReload>, button: Query<&Interaction, (With<TouchReloadButton>, Changed<Interaction>)>) {
    reload.0 = button.iter().any(|interaction| *interaction == Interaction::Pressed);
}

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

pub(crate) fn touch_sprint_button(mut sprint: ResMut<TouchSprintHeld>, button: Query<&Interaction, With<TouchSprintButton>>) {
    sprint.0 = button.iter().any(|interaction| *interaction == Interaction::Pressed);
}

pub(crate) fn touch_lean_buttons(mut lean: ResMut<TouchLeanHeld>, left_button: Query<&Interaction, With<TouchLeanLeftButton>>, right_button: Query<&Interaction, With<TouchLeanRightButton>>) {
    lean.left = left_button.iter().any(|interaction| *interaction == Interaction::Pressed);
    lean.right = right_button.iter().any(|interaction| *interaction == Interaction::Pressed);
}

fn touch_over_node<F: bevy::ecs::query::QueryFilter>(pos_physical: Vec2, query: &Query<(&ComputedNode, &UiGlobalTransform), F>) -> bool {
    query.single().is_ok_and(|(node, transform)| node.contains_point(*transform, pos_physical))
}

pub(crate) fn touch_over_scope_zoom_slider(
    pos_physical: Vec2,
    touch_aiming: bool,
    track: &Query<(&ComputedNode, &UiGlobalTransform), With<TouchScopeZoomSliderTrack>>,
) -> bool {
    touch_aiming && touch_over_node(pos_physical, track)
}

pub(crate) fn touch_over_settings_menu(
    pos_physical: Vec2,
    visible: &SettingsMenuVisible,
    menu: &Query<(&ComputedNode, &UiGlobalTransform), With<SettingsMenuRoot>>,
) -> bool {
    visible.0 && touch_over_node(pos_physical, menu)
}

pub(crate) fn touch_move_axis(
    touches: Res<Touches>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mouse_ui_locked: Res<MouseUiLocked>,
    settings_visible: Res<SettingsMenuVisible>,
    settings_menu: Query<(&ComputedNode, &UiGlobalTransform), With<SettingsMenuRoot>>,
    mut state: ResMut<TouchMoveState>,
    mut axis: ResMut<TouchMoveAxis>,
) {

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
            touch.position().x < half_width && !touch_over_settings_menu(touch.position() * scale_factor, &settings_visible, &settings_menu)
        }) {
            state.touch_id = Some(touch.id());
            state.origin = touch.position();
        }
    }

    axis.0 = match state.touch_id.and_then(|id| touches.get_pressed(id)) {
        Some(touch) => {
            let drag = (touch.position() - state.origin).clamp_length_max(TOUCH_JOYSTICK_RADIUS_PX) / TOUCH_JOYSTICK_RADIUS_PX;

            Vec2::new(drag.x, -drag.y)
        }
        None => Vec2::ZERO,
    };
}

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

    let logical_height = node.size().y * node.inverse_scale_factor();
    let logical_top = transform.affine().translation.y * node.inverse_scale_factor() - logical_height / 2.0;
    let t = ((touch.position().y - logical_top) / logical_height).clamp(0.0, 1.0);

    zoom.0 = SCOPE_MAX_ZOOM + (SCOPE_MIN_ZOOM - SCOPE_MAX_ZOOM) * t;
}

pub(crate) fn update_scope_zoom_slider_visual(zoom: Res<ScopeZoom>, mut handle: Query<&mut Node, With<TouchScopeZoomSliderHandle>>) {
    let Ok(mut node) = handle.single_mut() else {
        return;
    };
    let t = ((zoom.0 - SCOPE_MIN_ZOOM) / (SCOPE_MAX_ZOOM - SCOPE_MIN_ZOOM)).clamp(0.0, 1.0);

    let travel = TOUCH_SCOPE_ZOOM_TRACK_HEIGHT_PX - TOUCH_SCOPE_ZOOM_HANDLE_PX;
    node.top = Val::Px(travel * (1.0 - t));
}

pub(crate) fn touch_look(
    touches: Res<Touches>,
    windows: Query<&Window, With<PrimaryWindow>>,
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

    if autopilot.0 || look_only.0 {
        return;
    }

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

pub(crate) fn touch_fire_button(mut firing: ResMut<TouchFiring>, buttons: Query<&Interaction, With<TouchFireButton>>) {
    firing.0 = buttons.iter().any(|interaction| matches!(interaction, Interaction::Pressed));
}

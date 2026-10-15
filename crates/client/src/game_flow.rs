
use bevy::prelude::*;
use crate::*;

#[derive(Component)]
pub(crate) struct GameOverUi;

#[derive(Component)]
pub(crate) struct RestartButton;

#[derive(States, Clone, Copy, Eq, PartialEq, Hash, Debug, Default)]
pub(crate) enum AppState {
    #[default]
    Init,
    Menu,
    InGame,
}

#[derive(Resource)]
pub(crate) struct MapSelectionPending(pub(crate) bool);

static MAP_ID_OVERRIDE: std::sync::Mutex<Option<u8>> = std::sync::Mutex::new(None);

pub(crate) fn set_map_id(id: u8) {
    *MAP_ID_OVERRIDE.lock().unwrap() = Some(id);
}

pub(crate) fn map_id_override() -> Option<u8> {
    MAP_ID_OVERRIDE
        .lock()
        .unwrap()
        .or_else(|| std::env::var("MAP_ID").ok().and_then(|s| s.parse().ok()))
}

pub(crate) fn current_scenario() -> Scenario {
    map_id_override().map(Scenario::from_map_id).unwrap_or_default()
}

pub(crate) fn autopilot_enabled_by_default(_scenario: Scenario) -> bool {
    true
}

pub(crate) fn check_game_over(
    mut commands: Commands,
    mut game_over: ResMut<GameOver>,
    player: Query<&Health, With<Player>>,
    mut cursor: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    if game_over.0 {
        return;
    }
    let Ok(health) = player.single() else {
        return;
    };
    if health.0 > 0.0 {
        return;
    }
    game_over.0 = true;

    commands.trigger(PlayerDied);

    if let Ok(mut cursor) = cursor.single_mut() {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    }

    spawn_game_over_ui(&mut commands);
}

pub(crate) fn spawn_game_over_ui(commands: &mut Commands) {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(12.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
            GlobalZIndex(STATIC_UI_Z_INDEX + 25),
            GameOverUi,
            NativeUiRoot,
        ))
        .with_children(|root| {
            root.spawn((
                Text::new("GAME OVER"),
                TextFont {
                    font_size: FontSize::Px(64.0),
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.2, 0.2)),
            ));
            root.spawn((
                Text::new("Press R to Restart"),
                TextFont {
                    font_size: FontSize::Px(24.0),
                    ..default()
                },
                TextColor(Color::WHITE),
            ));

            root.spawn((
                Node {
                    width: Val::Px(180.0),
                    height: Val::Px(56.0),
                    margin: UiRect::top(Val::Px(12.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(1.0, 0.2, 0.2, 0.5)),
                BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.7)),
                Button,
                Interaction::default(),
                RestartButton,
            ))
            .with_children(|btn| {
                btn.spawn((
                    Text::new("RESTART"),
                    TextFont {
                        font_size: FontSize::Px(20.0),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
            });
        });
}

pub(crate) fn force_game_over_on_start(mut commands: Commands, mut game_over: ResMut<GameOver>) {
    if std::env::var("GAME_OVER_ON_START").is_err() {
        return;
    }
    game_over.0 = true;
    spawn_game_over_ui(&mut commands);
}

#[derive(Component)]
pub(crate) struct MapSelectRoot;

#[derive(Component)]
pub(crate) struct MapSelectButton(u8);

#[derive(Component)]
pub(crate) struct ResetConfigButton;

pub(crate) fn spawn_map_select_ui(commands: &mut Commands) {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(16.0),
                ..default()
            },
            BackgroundColor(Color::BLACK),
            GlobalZIndex(STATIC_UI_Z_INDEX + 100),
            MapSelectRoot,
            NativeUiRoot,
        ))
        .with_children(|root| {
            root.spawn((
                Text::new("SELECT MAP"),
                TextFont {
                    font_size: FontSize::Px(40.0),
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
            for scenario in Scenario::ALL {
                let (id, title, subtitle) = (scenario.map_id(), scenario.title(), scenario.subtitle());
                root.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(4.0),
                    margin: UiRect::top(Val::Px(8.0)),
                    ..default()
                })
                .with_children(|group| {
                    group
                        .spawn((
                            Node {
                                width: Val::Px(300.0),
                                height: Val::Px(56.0),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                border: UiRect::all(Val::Px(2.0)),
                                border_radius: BorderRadius::all(Val::Px(8.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.2, 0.3, 0.45, 1.0)),
                            BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.6)),
                            Button,
                            Interaction::default(),
                            MapSelectButton(id),
                        ))
                        .with_children(|btn| {
                            btn.spawn((
                                Text::new(title),
                                TextFont {
                                    font_size: FontSize::Px(20.0),
                                    ..default()
                                },
                                TextColor(Color::WHITE),
                            ));
                        });
                    group.spawn((
                        Text::new(subtitle),
                        TextFont {
                            font_size: FontSize::Px(13.0),
                            ..default()
                        },
                        TextColor(Color::srgba(0.8, 0.8, 0.8, 1.0)),
                    ));
                });
            }
            root.spawn((
                Node {
                    width: Val::Px(220.0),
                    height: Val::Px(40.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(8.0)),
                    margin: UiRect::top(Val::Px(24.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.45, 0.15, 0.15, 1.0)),
                BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.6)),
                Button,
                Interaction::default(),
                ResetConfigButton,
            ))
            .with_children(|btn| {
                btn.spawn((
                    Text::new("Reset Config"),
                    TextFont {
                        font_size: FontSize::Px(16.0),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
            });
        });
}

pub(crate) fn enter_menu(mut commands: Commands, mut pending: ResMut<MapSelectionPending>) {
    pending.0 = true;
    spawn_map_select_ui(&mut commands);
}

pub(crate) fn exit_menu(
    mut commands: Commands,
    root: Query<Entity, With<MapSelectRoot>>,
    mut pending: ResMut<MapSelectionPending>,
) {
    pending.0 = false;
    for entity in &root {
        commands.entity(entity).despawn();
    }
}

pub(crate) fn map_select_button(
    buttons: Query<(&Interaction, &MapSelectButton), Changed<Interaction>>,
    mut next_state: ResMut<NextState<AppState>>,
    mut noclip: ResMut<Noclip>,
    mut autopilot: ResMut<Autopilot>,
) {
    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        set_map_id(button.0);
        let scenario = Scenario::from_map_id(button.0);
        noclip.0 = false;
        autopilot.0 = autopilot_enabled_by_default(scenario);
        next_state.set(AppState::InGame);
    }
}

pub(crate) fn restart_game(mut commands: Commands, keys: Res<ButtonInput<KeyCode>>, game_over: Res<GameOver>) {
    if !game_over.0 || !keys.just_pressed(KeyCode::KeyR) {
        return;
    }
    commands.trigger(RequestRestart);
}

pub(crate) fn restart_button(mut commands: Commands, buttons: Query<&Interaction, (With<RestartButton>, Changed<Interaction>)>) {
    for interaction in &buttons {
        if *interaction == Interaction::Pressed {
            commands.trigger(RequestRestart);
        }
    }
}

pub(crate) fn on_game_state_event(
    trigger: On<GameStateEvent>,
    mut commands: Commands,
    mut game_over: ResMut<GameOver>,
    mut player: Query<(&mut Transform, &mut Player, &mut Health)>,
    game_over_ui: Query<Entity, With<GameOverUi>>,
) {
    match trigger.event() {
        GameStateEvent::GameOver => {
            game_over.0 = true;
        }
        GameStateEvent::Restarted => {
            game_over.0 = false;
            if let Ok((mut transform, mut player, mut health)) = player.single_mut() {
                transform.translation = current_scenario().player_spawn();
                player.vertical_velocity = 0.0;
                player.grounded = false;
                health.0 = PLAYER_MAX_HEALTH;
            }
            for entity in &game_over_ui {
                commands.entity(entity).try_despawn();
            }
        }
    }
}

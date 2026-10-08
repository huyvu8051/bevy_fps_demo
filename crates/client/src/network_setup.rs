
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

pub(crate) fn connect_to_server(mut commands: Commands, channels: Res<RepliconChannels>) {
    spawn_client_connection(&mut commands, &channels);
}

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

pub(crate) fn auto_reconnect(
    mut commands: Commands,
    time: Res<Time>,
    state: Res<State<ClientState>>,
    channels: Res<RepliconChannels>,
    mut disconnected_for: Local<f32>,
    pending: Res<MapSelectionPending>,
) {
    if pending.0 {
        *disconnected_for = 0.0;
        return;
    }
    if *state.get() != ClientState::Disconnected {
        *disconnected_for = 0.0;
        return;
    }
    *disconnected_for += time.delta_secs();
    if *disconnected_for < RECONNECT_INTERVAL {
        return;
    }
    *disconnected_for = 0.0;

    commands.remove_resource::<RenetClient>();
    commands.remove_resource::<NetcodeClientTransport>();
    info!("connection lost — retrying");
    spawn_client_connection(&mut commands, &channels);
}

pub(crate) fn despawn_replicated_on_disconnect(mut commands: Commands, entities: Query<Entity, Or<(With<Zombie>, With<NetPlayer>)>>) {
    for entity in &entities {
        commands.entity(entity).despawn();
    }
}

pub(crate) fn current_scenario() -> Scenario {
    map_id_override().map(Scenario::from_map_id).unwrap_or_default()
}

pub(crate) fn autopilot_enabled_by_default(scenario: Scenario) -> bool {
    scenario != Scenario::ItemGallery
}

pub(crate) fn spawn_client_connection(commands: &mut Commands, channels: &RepliconChannels) {

    let server_addr_str = std::env::var("SERVER_ADDR").unwrap_or_else(|_| {
        let host = if std::env::var("LOCAL").is_ok() {
            "127.0.0.1".to_string()
        } else {
            std::env::var("SERVER_HOST").unwrap_or_else(|_| "100.97.13.68".to_string())
        };
        format!("{host}:{}", current_scenario().port())
    });
    let server_addr: std::net::SocketAddr = std::net::ToSocketAddrs::to_socket_addrs(&server_addr_str)
        .unwrap_or_else(|_| panic!("SERVER_ADDR must be a resolvable host:port (DNS lookup failed for \"{server_addr_str}\"), e.g. 127.0.0.1:5031 or foo.tun.ply.gg:5031"))
        .next()
        .unwrap_or_else(|| panic!("SERVER_ADDR \"{server_addr_str}\" resolved to no addresses"));

    let client = RenetClient::new(
        ConnectionConfig::from_channels(channels.server_configs(), channels.client_configs()),
        false,
    );

    let current_time = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .expect("system clock should be after the unix epoch");
    let client_id = current_time.as_millis() as u64;
    let socket = std::net::UdpSocket::bind((std::net::Ipv4Addr::UNSPECIFIED, 0))
        .expect("failed to bind client UDP socket");
    let authentication = ClientAuthentication::Unsecure {
        client_id,
        protocol_id: PROTOCOL_ID,
        socket_id: 0,
        server_addr,
        user_data: None,
    };
    let socket = LaggedSocket::new(NativeSocket::new(socket).unwrap(), sim_latency_from_env());
    let transport = NetcodeClientTransport::new(current_time, authentication, socket)
        .expect("failed to set up netcode client transport");

    commands.insert_resource(client);
    commands.insert_resource(transport);
    commands.insert_resource(MyClientId(client_id));
    info!("connecting to server at {server_addr}");
}

#[derive(Resource)]
pub(crate) struct MyClientId(u64);

#[derive(Component)]
pub(crate) struct LocalPlayer;

pub(crate) fn mark_local_player(
    trigger: On<Add, NetworkId>,
    mut commands: Commands,
    my_id: Res<MyClientId>,
    players: Query<&NetworkId, With<NetPlayer>>,
) {
    let entity = trigger.event().entity;
    let Ok(network_id) = players.get(entity) else {
        return;
    };
    if network_id.get() == my_id.0 {
        commands.entity(entity).insert(LocalPlayer);
    }
}

pub(crate) fn on_net_player_replicated(
    trigger: On<Add, NetPlayer>,
    mut commands: Commands,
    my_id: Res<MyClientId>,
    network_ids: Query<&NetworkId>,
    asset_server: Res<AssetServer>,
    tuning: Res<PlayerModelTuning>,
) {
    let entity = trigger.event().entity;
    if network_ids.get(entity).is_ok_and(|id| id.get() == my_id.0) {
        return;
    }

    commands.entity(entity).insert(Visibility::default());
    commands.entity(entity).with_children(|parent| {
        spawn_remote_player_model(parent, &asset_server, &tuning);
    });
}

pub(crate) fn spawn_remote_player_model(parent: &mut ChildSpawnerCommands, asset_server: &AssetServer, tuning: &PlayerModelTuning) {
    parent.spawn((
        Name::new("PlayerModel (remote NetPlayer)"),
        SceneRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(tuning.mesh.clone()))),

        Transform::from_xyz(0.0, tuning.y_offset, 0.0)
            .with_rotation(Quat::from_rotation_y(tuning.facing_deg.to_radians()))
            .with_scale(Vec3::splat(tuning.scale)),
        PlayerModelAnimRoot,

        PlayerModelSkeletonBones::default(),
    ));
}

pub(crate) fn mirror_local_player_net_transform(
    mut players: Query<(&NetTransform, &mut Transform), (Changed<NetTransform>, With<LocalPlayer>)>,
) {
    for (net_transform, mut transform) in &mut players {
        transform.translation = net_transform.translation;
        transform.rotation = Quat::from_rotation_y(net_transform.yaw);
    }
}

#[derive(Component, Default)]
pub(crate) struct NetTransformBuffer {
    snapshots: std::collections::VecDeque<(f32, Vec3, f32)>,
}

pub(crate) fn on_net_transform_replicated(trigger: On<Add, NetTransform>, mut commands: Commands) {
    commands.entity(trigger.event().entity).insert(NetTransformBuffer::default());
}

pub(crate) fn buffer_net_transform(
    time: Res<Time>,
    mut entities: Query<(&NetTransform, &mut NetTransformBuffer), (Changed<NetTransform>, Without<LocalPlayer>)>,
) {
    let now = time.elapsed_secs();
    for (net_transform, mut buffer) in &mut entities {
        buffer.snapshots.push_back((now, net_transform.translation, net_transform.yaw));
        while buffer.snapshots.len() > 2 && now - buffer.snapshots[1].0 > INTERP_BUFFER_MAX_AGE_SECS {
            buffer.snapshots.pop_front();
        }
    }
}

pub(crate) fn interpolate_net_transform(time: Res<Time>, mut entities: Query<(&NetTransformBuffer, &mut Transform), Without<LocalPlayer>>) {
    let render_time = time.elapsed_secs() - INTERP_DELAY_SECS;
    for (buffer, mut transform) in &mut entities {
        let Some(&(_, latest_pos, latest_yaw)) = buffer.snapshots.back() else {
            continue;
        };
        if buffer.snapshots.len() < 2 || render_time >= buffer.snapshots.back().unwrap().0 {
            transform.translation = latest_pos;
            transform.rotation = Quat::from_rotation_y(latest_yaw);
            continue;
        }

        let bracket = buffer.snapshots.iter().zip(buffer.snapshots.iter().skip(1)).find(|(a, b)| render_time >= a.0 && render_time <= b.0);
        let Some((&(t0, pos0, yaw0), &(t1, pos1, yaw1))) = bracket else {

            let &(_, pos, yaw) = buffer.snapshots.front().unwrap();
            transform.translation = pos;
            transform.rotation = Quat::from_rotation_y(yaw);
            continue;
        };

        let span = (t1 - t0).max(f32::EPSILON);
        let t = ((render_time - t0) / span).clamp(0.0, 1.0);
        transform.translation = pos0.lerp(pos1, t);
        transform.rotation = Quat::from_rotation_y(yaw0).slerp(Quat::from_rotation_y(yaw1), t);
    }
}

pub(crate) fn log_client_connection_state(state: Res<State<ClientState>>) {
    if state.is_changed() {
        info!("client connection state: {:?}", state.get());
    }
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

    commands.client_trigger(PlayerDied);

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
                    font_size: 64.0,
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.2, 0.2)),
            ));
            root.spawn((
                Text::new("Press R to Restart"),
                TextFont {
                    font_size: 24.0,
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
                        font_size: 20.0,
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
                    font_size: 40.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
            for scenario in [Scenario::Normal, Scenario::StressTest, Scenario::ItemGallery] {
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
                                    font_size: 20.0,
                                    ..default()
                                },
                                TextColor(Color::WHITE),
                            ));
                        });
                    group.spawn((
                        Text::new(subtitle),
                        TextFont {
                            font_size: 13.0,
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
                        font_size: 16.0,
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
    commands.client_trigger(RequestRestart);
}

pub(crate) fn restart_button(mut commands: Commands, buttons: Query<&Interaction, (With<RestartButton>, Changed<Interaction>)>) {
    for interaction in &buttons {
        if *interaction == Interaction::Pressed {
            commands.client_trigger(RequestRestart);
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
                transform.translation = scenario_player_spawn(current_scenario());
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

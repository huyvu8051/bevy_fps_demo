// --- Networking setup: connection, replication mirroring, game-over/
// map-select/restart flow ---
// Extracted from lib.rs (Phase 7, the last of the gameplay-code-
// organization pass — see chat/commit history). Every item here is
// `pub(crate)` so lib.rs's single master `.chain()`'d Update system
// list (unchanged — pure code motion, no execution-order change) can
// still reference these by name.
//
// `use crate::*;` pulls in every crate-root item this module needs
// (constants, other components/resources, helper fns) — root-private
// items ARE visible to a descendant module like this one per Rust's
// normal privacy rules, just not name-resolved without an explicit
// `use`/path, hence the glob rather than hunting down each one by hand.

use bevy::prelude::*;
use crate::*;

// Marks the "GAME OVER" overlay so restart_game can find and despawn it.
#[derive(Component)]
pub(crate) struct GameOverUi;

#[derive(Component)]
pub(crate) struct RestartButton;

// True until a map's been picked (see spawn_map_select_ui/map_select_button)
// — mirrors what used to be entirely native-Swift-side state
// (ViewController's own `mapSelectionPending`, set from the exact same
// `ProcessInfo.environment["MAP_ID"] == nil` check): a plain icon-tap
// launch has no MAP_ID yet, so this stays true and connect_to_server (see
// its own `.run_if`) holds off connecting until map_select_button sets
// MAP_ID and flips this false. A `MAP_ID` env var already present (e.g. a
// devicectl/xcrun launch with `-e MAP_ID=1`) skips the menu entirely, same
// as the old Swift behavior.
#[derive(Resource)]
pub(crate) struct MapSelectionPending(pub(crate) bool);

pub(crate) fn connect_to_server(mut commands: Commands, channels: Res<RepliconChannels>) {
    spawn_client_connection(&mut commands, &channels);
}

// Runs every frame; only actually does anything once ClientState has been
// Disconnected for RECONNECT_INTERVAL seconds straight (the timer resets
// otherwise, including while Connecting/Connected). The stale RenetClient/
// NetcodeClientTransport left over from the dropped connection is removed
// first — resource_added::<RenetClient> (see bevy_replicon_renet2's own
// set_connecting) is what drives the Connecting state transition, and that
// only fires on a genuine remove-then-insert, not an insert over an
// already-present resource.
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

// bevy_replicon's own OnExit(ClientState::Connected) cleanup only clears its
// internal server-entity-id bookkeeping (ServerEntityMap etc.) — it never
// despawns the actual client-side entities that were spawned for
// replication. Left alone, a reconnect (see auto_reconnect) to a fresh
// server session would leave every zombie/player from the old session
// sitting around frozen forever (no longer receiving updates, since the
// new server has no idea they exist) while a whole new set spawns in on
// top of them. Despawning Zombie/NetPlayer entities here — not the local
// Player puppet, which isn't replicated — clears them out; things that
// reference a zombie by Entity (health bars) already self-heal once their
// target is gone (see update_zombie_health_bars).
pub(crate) fn despawn_replicated_on_disconnect(mut commands: Commands, entities: Query<Entity, Or<(With<Zombie>, With<NetPlayer>)>>) {
    for entity in &entities {
        commands.entity(entity).despawn();
    }
}

// Which map-select scenario this run is using — read once from the MAP_ID
// env var (0/1/2, default 0/Normal). Set either by the iOS map-select menu
// (via setenv() before creating the game — see ViewController.swift's own
// map-select handling) or by a devicectl launch -e '{"MAP_ID":"1"}' to
// skip the menu entirely. Also drives Noclip's scenario-specific default
// (see its own init below) — both call sites need the exact same value
// this run, hence a plain function (re-reads the env var, cheap) instead
// of stashing it in a resource only one of them would see first.
pub(crate) fn current_scenario() -> Scenario {
    let map_id: u8 = std::env::var("MAP_ID").ok().and_then(|s| s.parse().ok()).unwrap_or(0);
    Scenario::from_map_id(map_id)
}

// Single source of truth for Autopilot's scenario-specific default — used
// both at app-build time (direct MAP_ID=N launch, menu skipped entirely)
// and from the map-select menu's own button handler (see AutopilotButton's
// call site) — previously only the LATTER existed, so a direct MAP_ID
// launch (no menu) always got Autopilot::default()'s plain `true`
// regardless of scenario. Off for any scenario where an AI target-seeking,
// auto-firing player would fight the reason that scenario exists:
// RenderCompare (static pose comparison, needs the camera to hold still)
// and ItemGallery (no zombies to target at all — walk-around scale check,
// autopilot has nothing to do but still eats manual WASD/mouse input).
pub(crate) fn autopilot_enabled_by_default(scenario: Scenario) -> bool {
    !matches!(scenario, Scenario::RenderCompare | Scenario::ItemGallery)
}

pub(crate) fn spawn_client_connection(commands: &mut Commands, channels: &RepliconChannels) {
    // to_socket_addrs (not parse::<SocketAddr>) so this also accepts a DNS
    // hostname, not just a literal IP — needed for e.g. playit.gg's
    // assigned tunnel domain (foo.tun.ply.gg:PORT), which a plain
    // SocketAddr parse rejects outright since it never resolves DNS.
    // Host resolution, in precedence order:
    //   SERVER_ADDR=host:port  — full override, host AND port (skips the
    //                            scenario-port routing); the escape hatch.
    //   LOCAL=1                — host 127.0.0.1 (local dev on this machine).
    //   SERVER_HOST=host       — custom host, scenario port.
    //   (nothing)              — this Mac's Tailscale IP, for the on-device
    //                            test build (only reachable on that network).
    // Port for the last three comes from current_scenario().port() — see
    // its own doc comment and shared::Scenario's for the map-select-menu ->
    // port-per-scenario routing.
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

// The renet2 netcode client_id chosen in connect_to_server — also what the
// server surfaces as NetworkId on the connection entity it spawns, so
// comparing the two is how the client recognizes which replicated NetPlayer
// entity is its own (see mark_local_player).
#[derive(Resource)]
pub(crate) struct MyClientId(u64);

// Client-only marker (never sent over the network) for whichever replicated
// NetPlayer entity turned out to be this client's own — see MyClientId.
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

// Every non-local NetPlayer entity (a real second client OR a debug bot —
// both look identical from here, they're just replicated NetPlayer/
// NetTransform data) currently has zero visual mesh, since only the local
// player has the optional T-toggle debug model. Attaches PLAYER_MODEL_PATH
// as a child so it inherits the parent's yaw automatically from
// mirror_local_player_net_transform/interpolate_net_transform — no extra
// rotation code needed here. NetworkId is already present by the time
// NetPlayer replicates in (it's assigned at connection time, before
// spawn_player ever inserts NetPlayer server-side), so a bot — which has no
// NetworkId at all — always falls through to "spawn a visual" below.
pub(crate) fn on_net_player_replicated(
    trigger: On<Add, NetPlayer>,
    mut commands: Commands,
    my_id: Res<MyClientId>,
    network_ids: Query<&NetworkId>,
    asset_server: Res<AssetServer>,
) {
    let entity = trigger.event().entity;
    if network_ids.get(entity).is_ok_and(|id| id.get() == my_id.0) {
        return;
    }
    // Visibility: NetPlayer never had one before (see zombie_cosmetic_bundle's
    // matching comment) — needed now that this entity has a child with a
    // SceneRoot, or Bevy's visibility-propagation hook warns (B0004).
    commands.entity(entity).insert(Visibility::default());
    commands.entity(entity).with_children(|parent| {
        parent.spawn((
            SceneRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(PLAYER_MODEL_PATH))),
            Transform::from_xyz(0.0, PLAYER_MODEL_Y_OFFSET, 0.0)
                .with_rotation(Quat::from_rotation_y(PLAYER_MODEL_FACING_CORRECTION))
                .with_scale(Vec3::splat(PLAYER_MODEL_SCALE)),
            PlayerModelAnimRoot,
        ));
    });
}

// The local player's own replicated pose is copied straight through,
// uninterpolated — this is just the confirmed baseline predict_player_movement
// replays unconfirmed inputs on top of, not what's actually rendered.
// Interpolation's fixed rendering delay would only add latency here, since
// there's no "other player" to smooth relative to.
pub(crate) fn mirror_local_player_net_transform(
    mut players: Query<(&NetTransform, &mut Transform), (Changed<NetTransform>, With<LocalPlayer>)>,
) {
    for (net_transform, mut transform) in &mut players {
        transform.translation = net_transform.translation;
        transform.rotation = Quat::from_rotation_y(net_transform.yaw);
    }
}

// Timestamped position/yaw history for interpolate_net_transform — every
// NetTransform-carrying entity gets one (see on_net_transform_replicated),
// including the local player's, but mirror_local_player_net_transform
// bypasses it for that one (Without<LocalPlayer> everywhere below).
#[derive(Component, Default)]
pub(crate) struct NetTransformBuffer {
    snapshots: std::collections::VecDeque<(f32, Vec3, f32)>,
}

pub(crate) fn on_net_transform_replicated(trigger: On<Add, NetTransform>, mut commands: Commands) {
    commands.entity(trigger.event().entity).insert(NetTransformBuffer::default());
}

// Appends a timestamped snapshot every time replication actually updates
// NetTransform — this is the "actually received" data interpolate_net_transform
// draws its two bracketing points from.
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

// Renders every non-local NetTransform entity (zombies today; other
// players once they have a visual — see mirror_local_player_net_transform's
// comment on why the local player skips this) at INTERP_DELAY_SECS behind
// its own snapshot buffer, lerped between whichever two received snapshots
// bracket that render time. Falls back to the newest/oldest snapshot
// (still no visible snap, just holds position) when render_time falls
// outside the buffered range — before the first snapshot arrives, or once
// the buffer catches up to real time between replication ticks.
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
            // render_time is older than every buffered snapshot — hold at
            // the oldest one rather than extrapolate backwards.
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
    // Tells the server this player is down — it declares GameOver once
    // every connected player has reported the same (see on_player_died on
    // the server, and this client's own on_game_state_event observer).
    commands.client_trigger(PlayerDied);

    if let Ok(mut cursor) = cursor.single_mut() {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    }

    spawn_game_over_ui(&mut commands);
}

// Factored out of check_game_over so GAME_OVER_ON_START (see its own doc
// comment) can spawn the exact same overlay without needing a real death
// first — same launch-time-verification idea as OPEN_SETTINGS_MENU/
// TOUCH_UI_PREVIEW.
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
            GameOverUi,
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
            // Touch/click equivalent of the KeyR restart — mobile has no
            // keyboard, so the text-only hint above was unreachable there.
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

// GAME_OVER_ON_START=1 shows the GAME OVER overlay immediately at launch —
// same launch-time-override idea as OPEN_SETTINGS_MENU/TOUCH_UI_PREVIEW, so
// it (and, on iOS, the cadisplaylink Metal renderer's own draw of it) can
// be visually verified without needing a real death first. Runs on Startup,
// after setup() has already spawned everything else — GameOverUi is a
// dynamic overlay (unlike SettingsMenuRoot, always spawned but hidden),
// so this is the one env-var override that adds a new entity rather than
// just flipping a resource.
pub(crate) fn force_game_over_on_start(mut commands: Commands, mut game_over: ResMut<GameOver>) {
    if std::env::var("GAME_OVER_ON_START").is_err() {
        return;
    }
    game_over.0 = true;
    spawn_game_over_ui(&mut commands);
}

// Marks the map-select overlay so map_select_button can find and despawn
// it — was MapSelectView.swift's whole reason to exist (the one native-
// only piece left after touch controls/HUD/settings menu/game over all
// moved to bevy_ui) before that file was deleted in favor of this.
#[derive(Component)]
pub(crate) struct MapSelectRoot;

// `.0` is the MAP_ID this button picks — see MAP_SELECT_ENTRIES.
#[derive(Component)]
pub(crate) struct MapSelectButton(u8);

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
            for &(id, title, subtitle) in &MAP_SELECT_ENTRIES {
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
        });
}

pub(crate) fn spawn_map_select_ui_if_pending(mut commands: Commands, pending: Res<MapSelectionPending>) {
    if pending.0 {
        spawn_map_select_ui(&mut commands);
    }
}

// Touch/click handler for the map-select buttons — sets MAP_ID (the exact
// same env var the old Swift `setenv()` flow used, and current_scenario()
// still reads every time it's called), flips MapSelectionPending so
// connect_to_server's own `.run_if` would let it through on a future
// Startup, and connects right away via the same spawn_client_connection
// auto_reconnect already calls to recover from a dropped connection — no
// separate "first connect" code path needed.
pub(crate) fn map_select_button(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    buttons: Query<(&Interaction, &MapSelectButton), Changed<Interaction>>,
    root: Query<Entity, With<MapSelectRoot>>,
    mut pending: ResMut<MapSelectionPending>,
    mut noclip: ResMut<Noclip>,
    mut autopilot: ResMut<Autopilot>,
    channels: Res<RepliconChannels>,
    // Every currently-spawned level-geometry entity (+ the ItemGallery car
    // test props, same reasoning) — despawned + rebuilt below for whichever
    // scenario was just picked. Needed because level geometry is now built
    // LOCALLY at Startup (see setup()) instead of replicated from the
    // server (see level_piece.rs's own doc comment) — Startup only runs
    // once, at the DEFAULT scenario (MAP_ID unset), so picking a different
    // map from this in-game menu used to leave the stale default geometry
    // on screen forever (reported bug: picking ITEM GALLERY still showed
    // the B1 garage) unless re-spawned here. The car props hit the same
    // bug from the other direction too: picking ItemGallery from this menu
    // never ran setup()'s Startup-only spawn, so they just never appeared.
    existing_geometry: Query<
        Entity,
        Or<(
            With<shared::LevelBox>,
            With<shared::LevelPanel>,
            With<ItemGalleryTestProp>,
            With<ArenaGeometry>,
        )>,
    >,
) {
    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        // SAFETY: called from this one Bevy system, on the main thread, in
        // response to a UI tap — not concurrently with any other env var
        // read/write, same assumption current_scenario()'s own callers
        // already rely on.
        unsafe {
            std::env::set_var("MAP_ID", button.0.to_string());
        }
        if let Ok(root) = root.single() {
            commands.entity(root).despawn();
        }
        pending.0 = false;
        let scenario = Scenario::from_map_id(button.0);
        // No scenario auto-enables noclip anymore — reset it off in case a
        // previous pick / manual toggle left it on.
        noclip.0 = false;
        // Same reasoning — Autopilot::default() also baked in before any
        // map was picked (see autopilot_enabled_by_default's own doc
        // comment), needs re-applying now that the choice is real.
        autopilot.0 = autopilot_enabled_by_default(scenario);

        for entity in &existing_geometry {
            commands.entity(entity).despawn();
        }
        // Rebuild geometry for the picked scenario (Startup's setup() only
        // ran for the default). Same match as setup()'s own, via
        // Scenario::map_kind(). Static lights (spawn_map1_lights / the
        // arena's overhead) aren't re-spawned/cleared here — they're
        // default-off anyway and already up from Startup; a menu switch
        // leaving a stale light is a known cosmetic gap, not a blocker.
        match scenario.map_kind() {
            MapKind::ItemGallery => {
                let (pieces, panels) = shared::item_gallery_level_pieces();
                spawn_level_pieces(&mut commands, pieces, panels);
                spawn_item_gallery_test_props(&mut commands, &asset_server);
                spawn_item_gallery_zombie_showcase(&mut commands, &asset_server);
            }
            MapKind::Building => {
                let (pieces, panels) = shared::map1_level_pieces();
                spawn_level_pieces(&mut commands, pieces, panels);
            }
            MapKind::Arena => {
                spawn_test_arena(&mut commands, &mut meshes, &mut materials);
                spawn_item_gallery_test_props(&mut commands, &asset_server);
            }
        }
        spawn_client_connection(&mut commands, &channels);
    }
}

// Just asks the server to restart — it's the one that actually decides how
// (despawn+respawn zombies, revive every connected player), same as every
// other piece of shared world state moved there in earlier phases. Any
// client whose own GameOver is set can trigger it; see on_game_state_event
// for the actual local reset once the server confirms via GameStateEvent.
pub(crate) fn restart_game(mut commands: Commands, keys: Res<ButtonInput<KeyCode>>, game_over: Res<GameOver>) {
    if !game_over.0 || !keys.just_pressed(KeyCode::KeyR) {
        return;
    }
    commands.client_trigger(RequestRestart);
}

// Touch/click path for the same request — see RestartButton's own spawn
// site. Changed<Interaction>-gated like the other touch buttons, so a tap
// only fires once instead of every frame it's held.
pub(crate) fn restart_button(mut commands: Commands, buttons: Query<&Interaction, (With<RestartButton>, Changed<Interaction>)>) {
    for interaction in &buttons {
        if *interaction == Interaction::Pressed {
            commands.client_trigger(RequestRestart);
        }
    }
}

// Reacts to the server's two game-state broadcasts. GameOver mainly matters
// for a client that somehow hasn't already noticed locally (see
// check_game_over); Restarted is what actually resets this client's player
// and tears down the GAME OVER overlay — deliberately the only place that
// does so, whether this client was the one who pressed Restart or not.
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


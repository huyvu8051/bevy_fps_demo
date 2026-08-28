// --- HUD: health/ammo/ping/compass/crosshair/debug panel/floating damage ---
// Extracted from lib.rs (Phase 6 of the gameplay-code-organization pass —
// see chat/commit history). Every item here is `pub(crate)` so lib.rs's
// single master `.chain()`'d Update system list (unchanged — pure code
// motion, no execution-order change) can still reference these by name.
//
// `use crate::*;` pulls in every crate-root item this module needs
// (constants, other components/resources, helper fns) — root-private
// items ARE visible to a descendant module like this one per Rust's
// normal privacy rules, just not name-resolved without an explicit
// `use`/path, hence the glob rather than hunting down each one by hand.

use bevy::prelude::*;
use crate::*;

// Debug/dev: toggled by KeyL. A step further than plain Noclip — this is a
// pure spectator camera for checking a map's own geometry/scale (walls,
// ramps, gaps lining up between floors, etc.) without any player-shaped
// interference: turning it on forces Noclip on and the gun invisible in the
// same frame (see toggle_inspect_mode), and shoot() early-returns while it's
// on so flying through a zombie's aim cone can't burn ammo or trigger a
// reload. Turning it back off restores both to their prior state. Exists
// because manually combining Noclip + hide-gun + "don't fire" by hand every
// time you just want to eyeball a newly-built floor is exactly the kind of
// bookkeeping a dedicated mode should absorb.
#[derive(Resource, Default)]
pub(crate) struct InspectMode(pub(crate) bool);

// Debug/dev: toggled by KeyP. Shows debug_panel_actions' hotkeys plus
// current state of every other debug toggle, via update_debug_panel.
#[derive(Resource, Default)]
pub(crate) struct DebugPanelVisible(bool);

// Marks the debug panel's text node so update_debug_panel can find it.
#[derive(Component)]
pub(crate) struct DebugPanelText;

// Marks the always-visible health readout so update_health_ui can find it.
#[derive(Component)]
pub(crate) struct HealthUi;

// Marks the InspectMode coordinate readout so update_inspect_ui can find
// it — only shown while InspectMode is on (see that system). Exists so
// checking a newly-built floor's geometry against its own FloorLevel
// constants (elevation, wall lines, ramp gaps) is "fly there and read the
// numbers off screen" instead of guessing from the render alone.
#[derive(Component)]
pub(crate) struct InspectUi;

// A damage number that pops up at the hit point and floats/fades out.
// Tracks its own world position (drifting upward each frame) rather than a
// zombie Entity, since it should keep floating even if the zombie that took
// the hit dies and despawns a moment later.
#[derive(Component)]
pub(crate) struct FloatingDamageText {
    world_pos: Vec3,
    remaining: f32,
}

// Marks the ammo counter text so update_ammo_ui can find it.
#[derive(Component)]
pub(crate) struct AmmoUi;

pub(crate) fn spawn_floating_damage(commands: &mut Commands, world_pos: Vec3, damage: f32) {
    let critical = damage >= HEAD_DAMAGE;
    commands.spawn((
        Text::new(format!("{damage:.1}")),
        TextFont {
            font_size: if critical {
                FLOATING_DAMAGE_CRIT_FONT_SIZE
            } else {
                FLOATING_DAMAGE_FONT_SIZE
            },
            ..default()
        },
        TextColor(if critical {
            Color::srgb(1.0, 0.2, 0.15)
        } else {
            Color::WHITE
        }),
        Node {
            position_type: PositionType::Absolute,
            // Positioned/shown by update_floating_damage_texts once it's
            // projected — same one-frame-flash avoidance as the health bar.
            display: Display::None,
            ..default()
        },
        FloatingDamageText {
            world_pos,
            remaining: FLOATING_DAMAGE_LIFETIME,
        },
    ));
}

pub(crate) fn update_floating_damage_texts(
    mut commands: Commands,
    time: Res<Time>,
    camera: Query<(&Camera, &GlobalTransform), With<FpsCamera>>,
    mut texts: Query<(Entity, &mut FloatingDamageText, &mut Node, &mut TextColor)>,
) {
    let Ok((camera, camera_transform)) = camera.single() else {
        return;
    };
    for (entity, mut floating, mut node, mut color) in &mut texts {
        floating.remaining -= time.delta_secs();
        if floating.remaining <= 0.0 {
            commands.entity(entity).try_despawn();
            continue;
        }
        floating.world_pos += Vec3::Y * FLOATING_DAMAGE_RISE_SPEED * time.delta_secs();
        color.0.set_alpha((floating.remaining / FLOATING_DAMAGE_LIFETIME).clamp(0.0, 1.0));

        let Ok(screen_pos) = camera.world_to_viewport(camera_transform, floating.world_pos) else {
            node.display = Display::None;
            continue;
        };
        node.display = Display::Flex;
        node.left = Val::Px(screen_pos.x);
        node.top = Val::Px(screen_pos.y);
    }
}

pub(crate) fn spawn_health_ui(commands: &mut Commands) {
    commands.spawn((
        Text::new("HP: 100"),
        TextFont {
            font_size: 24.0,
            ..default()
        },
        TextColor(Color::srgb(1.0, 0.3, 0.3)),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(16.0),
            top: Val::Px(16.0),
            ..default()
        },
        GlobalZIndex(STATIC_UI_Z_INDEX),
        HealthUi,
    ));
}

pub(crate) fn update_health_ui(
    player: Query<&Health, (With<Player>, Changed<Health>)>,
    mut text: Query<&mut Text, With<HealthUi>>,
) {
    let Ok(health) = player.single() else {
        return;
    };
    let Ok(mut text) = text.single_mut() else {
        return;
    };
    text.0 = format!("HP: {}", health.0.max(0.0).round() as i32);
}

// Bottom-center so it doesn't collide with health (top-left)/ammo
// (bottom-right)/ping (top-right) — hidden by default, only meaningful
// while InspectMode is on.
pub(crate) fn spawn_inspect_ui(commands: &mut Commands) {
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: 18.0,
            ..default()
        },
        TextColor(Color::srgb(0.4, 1.0, 0.6)),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(16.0),
            bottom: Val::Px(16.0),
            ..default()
        },
        GlobalZIndex(STATIC_UI_Z_INDEX),
        Visibility::Hidden,
        InspectUi,
    ));
}

// World position + yaw/pitch of the spectator camera, refreshed every
// frame InspectMode is on — the whole point of the mode is comparing this
// against a floor's own FloorLevel constants (elevation, north_wall_z,
// etc.) to confirm the built geometry actually lines up with the design.
pub(crate) fn update_inspect_ui(
    inspect: Res<InspectMode>,
    camera: Query<(&Transform, &FpsCamera)>,
    mut text: Query<(&mut Text, &mut Visibility), With<InspectUi>>,
) {
    let Ok((mut text, mut visibility)) = text.single_mut() else {
        return;
    };
    if !inspect.0 {
        *visibility = Visibility::Hidden;
        return;
    }
    *visibility = Visibility::Visible;
    let Ok((transform, cam)) = camera.single() else {
        return;
    };
    let p = transform.translation;
    text.0 = format!(
        "INSPECT  x={:.2} y={:.2} z={:.2}  yaw={:.0}° pitch={:.0}°",
        p.x,
        p.y,
        p.z,
        cam.yaw.to_degrees(),
        cam.pitch.to_degrees()
    );
}

pub(crate) fn spawn_ammo_ui(commands: &mut Commands) {
    commands.spawn((
        Text::new(format!("{MAGAZINE_SIZE} / {MAGAZINE_SIZE}")),
        TextFont {
            font_size: 24.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(16.0),
            bottom: Val::Px(16.0),
            ..default()
        },
        GlobalZIndex(STATIC_UI_Z_INDEX),
        AmmoUi,
    ));
}

// Marks the always-visible ping readout so update_ping_ui can find it.
#[derive(Component)]
pub(crate) struct PingUi;

pub(crate) fn spawn_ping_ui(commands: &mut Commands) {
    commands.spawn((
        Text::new("Ping: -- ms"),
        TextFont {
            font_size: 16.0,
            ..default()
        },
        TextColor(Color::srgb(0.7, 0.85, 1.0)),
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(16.0),
            top: Val::Px(16.0),
            ..default()
        },
        GlobalZIndex(STATIC_UI_Z_INDEX),
        PingUi,
    ));
}

pub(crate) fn update_ping_ui(
    time: Res<Time>,
    client: Option<Res<RenetClient>>,
    mut timer: Local<f32>,
    mut text: Query<&mut Text, With<PingUi>>,
) {
    *timer -= time.delta_secs();
    if *timer > 0.0 {
        return;
    }
    *timer = PING_UI_REFRESH_SECS;

    let Ok(mut text) = text.single_mut() else {
        return;
    };
    text.0 = match client {
        Some(client) => format!("Ping: {:.0} ms", client.rtt() * 1000.0),
        None => "Ping: -- ms".to_string(),
    };
}

// cam.yaw's own convention: forward = Quat::from_rotation_y(yaw) * NEG_Z,
// so yaw=0 already faces "North" (-Z) here — heading is just that turned
// into a clockwise-from-North degree value (mouse-right decreases yaw,
// which should read as turning toward East/increasing heading, hence the
// negation; see this function's own module-level doc comment above).
pub(crate) fn heading_deg(yaw: f32) -> f32 {
    (-yaw.to_degrees()).rem_euclid(360.0)
}

// Shortest signed difference `a - b` wrapped into (-180, 180] — how far
// (and which way) to slide a mark whose fixed heading is `a` relative to
// the compass window currently centered on heading `b`.
pub(crate) fn heading_diff(a: f32, b: f32) -> f32 {
    ((a - b + 180.0).rem_euclid(360.0)) - 180.0
}

#[derive(Component)]
pub(crate) struct CompassMark(f32);

#[derive(Component)]
pub(crate) struct CompassHeadingText;

pub(crate) fn spawn_compass_ui(commands: &mut Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(12.0),
                left: Val::Percent(50.0),
                margin: UiRect::left(Val::Px(-COMPASS_WIDTH / 2.0)),
                width: Val::Px(COMPASS_WIDTH),
                height: Val::Px(COMPASS_HEIGHT),
                border: UiRect::all(Val::Px(1.0)),
                overflow: Overflow::clip(),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.35)),
            BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.25)),
            GlobalZIndex(STATIC_UI_Z_INDEX),
        ))
        .with_children(|root| {
            for &(heading, label) in &COMPASS_MARKS {
                root.spawn((
                    Text::new(label),
                    TextFont {
                        font_size: 16.0,
                        ..default()
                    },
                    TextColor(if heading == 0.0 { Color::srgb(1.0, 0.35, 0.3) } else { Color::srgb(0.85, 0.85, 0.85) }),
                    Node {
                        position_type: PositionType::Absolute,
                        top: Val::Px(4.0),
                        ..default()
                    },
                    CompassMark(heading),
                ));
            }
            // Center tick — stays put; the marks above scroll under it.
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(COMPASS_WIDTH / 2.0 - 1.0),
                    bottom: Val::Px(0.0),
                    width: Val::Px(2.0),
                    height: Val::Px(6.0),
                    ..default()
                },
                BackgroundColor(Color::srgb(1.0, 0.85, 0.2)),
            ));
        });

    // Numeric heading readout (e.g. "047°"), centered just under the tape.
    commands.spawn((
        Text::new("000°"),
        TextFont {
            font_size: 14.0,
            ..default()
        },
        TextColor(Color::srgb(1.0, 0.85, 0.2)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(12.0 + COMPASS_HEIGHT + 2.0),
            left: Val::Percent(50.0),
            margin: UiRect::left(Val::Px(-18.0)),
            ..default()
        },
        GlobalZIndex(STATIC_UI_Z_INDEX),
        CompassHeadingText,
    ));
}

pub(crate) fn update_compass_ui(
    camera: Query<&FpsCamera>,
    mut marks: Query<(&CompassMark, &mut Node), Without<CompassHeadingText>>,
    mut heading_text: Query<&mut Text, With<CompassHeadingText>>,
) {
    let Ok(cam) = camera.single() else {
        return;
    };
    let current = heading_deg(cam.yaw);
    for (mark, mut node) in &mut marks {
        let x = COMPASS_WIDTH / 2.0 + heading_diff(mark.0, current) * COMPASS_PX_PER_DEG;
        // -8px: rough half-glyph-width nudge so 1-2 letter labels look
        // centered on their heading rather than left-edge-aligned to it —
        // not pixel-perfect (labels are 1-2 chars), close enough for a HUD.
        node.left = Val::Px(x - 8.0);
    }
    if let Ok(mut text) = heading_text.single_mut() {
        text.0 = format!("{:03.0}°", current);
    }
}

pub(crate) fn update_ammo_ui(gun: Query<&Gun, Changed<Gun>>, mut text: Query<&mut Text, With<AmmoUi>>) {
    let Ok(gun) = gun.single() else {
        return;
    };
    let Ok(mut text) = text.single_mut() else {
        return;
    };
    text.0 = if gun.reload_remaining > 0.0 {
        "RELOADING...".to_string()
    } else {
        format!("{} / {MAGAZINE_SIZE}", gun.ammo)
    };
}

// Four independent hash marks instead of a fixed cross, so update_crosshair
// can push them apart from the center as bullet spread grows. direction
// points away from center: (0,-1) top, (0,1) bottom, (-1,0) left, (1,0) right.
#[derive(Component)]
pub(crate) struct CrosshairLine {
    direction: Vec2,
}

// Marks the crosshair's root Node so update_crosshair can hide the whole
// thing while ADS'd — the scope's own reticle takes over at that point.
#[derive(Component)]
pub(crate) struct CrosshairRoot;

pub(crate) fn spawn_debug_panel(commands: &mut Commands) {
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: 16.0,
            ..default()
        },
        TextColor(Color::srgb(0.6, 1.0, 0.6)),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(16.0),
            top: Val::Px(60.0),
            display: Display::None,
            ..default()
        },
        GlobalZIndex(STATIC_UI_Z_INDEX),
        DebugPanelText,
    ));
}

// Bundles the ~10 debug-toggle resource reads update_debug_panel needs
// into one param — that fn was already at Bevy's 16-param system function
// limit (see HitMarkerAssets' own doc comment for the same constraint hit
// elsewhere), and the refresh-throttle Local pushed it over.
#[derive(SystemParam)]
pub(crate) struct DebugPanelToggles<'w> {
    noclip: Res<'w, Noclip>,
    freeze: Res<'w, FreezeZombies>,
    show_hitboxes: Res<'w, ShowHitboxes>,
    god_mode: Res<'w, GodMode>,
    aim_blur: Res<'w, AimBlurEnabled>,
    dof_focal_alt: Res<'w, AimDofFocalDistanceAlt>,
    zombie_anim_disabled: Res<'w, ZombieAnimDisabled>,
    zombie_health_bars_hidden: Res<'w, ZombieHealthBarsHidden>,
    zombie_lod_disabled: Res<'w, ZombieLodDisabled>,
    player_model_visible: Res<'w, PlayerModelVisible>,
    autopilot: Res<'w, Autopilot>,
    autopilot_state: Res<'w, AutopilotState>,
}

pub(crate) fn update_debug_panel(
    panel_visible: Res<DebugPanelVisible>,
    toggles: DebugPanelToggles,
    timings: Res<PerfTimings>,
    debug_bots: Query<&DebugBot>,
    selected_bot: Res<SelectedDebugBot>,
    zombies: Query<(), With<Zombie>>,
    mut text: Query<(&mut Text, &mut Node), With<DebugPanelText>>,
    time: Res<Time>,
    mut refresh_elapsed: Local<f32>,
) {
    let Ok((mut text, mut node)) = text.single_mut() else {
        return;
    };
    if !panel_visible.0 {
        node.display = Display::None;
        return;
    }
    node.display = Display::Flex;

    // A human reading debug text doesn't need it re-formatted 120x/sec —
    // this was pure wasted string allocation every single frame the panel
    // happened to be open. 10Hz is still imperceptibly "live".
    *refresh_elapsed += time.delta_secs();
    if *refresh_elapsed < 0.1 {
        return;
    }
    *refresh_elapsed = 0.0;

    let on_off = |b: bool| if b { "ON" } else { "OFF" };
    let focal_distance = if toggles.dof_focal_alt.0 {
        AIM_DOF_FOCAL_DISTANCE_B
    } else {
        AIM_DOF_FOCAL_DISTANCE_A
    };
    text.0 = format!(
        "[DEBUG PANEL] (P to close)\n\
         N: Noclip [{}]\n\
         F: Freeze zombies [{}]\n\
         H: Show hitboxes [{}]\n\
         G: God mode [{}]\n\
         B: Blur when aiming [{}]\n\
         V: Blur focal distance [{:.2}m]\n\
         1: Spawn zombie in front of player\n\
         2: Kill all zombies\n\
         3: Set nearest zombie to 1 HP\n\
         4: Heal nearest zombie to full\n\
         5: Spawn another {} zombies\n\
         M: Zombie animation [{}]\n\
         J: Zombie health bars [{}]\n\
         K: Zombie LOD (distance shortcut) [{}]\n\
         T: Player model (debug visual) [{}]\n\
         I: Autopilot [{}] | Aiming: {}\n\
         6: Spawn debug bot at crosshair+2m | [/]: select prev/next bot\n\
         9: Move selected bot to crosshair | O: Despawn selected bot\n\
         Debug bots: {} (selected: {})\n\
         move_zombies: {:.2}ms | health bars: {:.2}ms | zombies: {} (near: {})\n\
         Zombies alive: {}",
        on_off(toggles.noclip.0),
        on_off(toggles.freeze.0),
        on_off(toggles.show_hitboxes.0),
        on_off(toggles.god_mode.0),
        on_off(toggles.aim_blur.0),
        focal_distance,
        ZOMBIE_SPAWNS.len(),
        on_off(!toggles.zombie_anim_disabled.0),
        on_off(!toggles.zombie_health_bars_hidden.0),
        on_off(!toggles.zombie_lod_disabled.0),
        on_off(toggles.player_model_visible.0),
        on_off(toggles.autopilot.0),
        toggles.autopilot_state.aiming_region.unwrap_or("-"),
        debug_bots.iter().count(),
        selected_bot.0.map_or("none".to_string(), |id| id.to_string()),
        timings.move_zombies_ms,
        timings.health_bars_ms,
        timings.zombie_count,
        timings.near_zombie_count,
        zombies.iter().count(),
    );
}

// Sets whichever zombie is closest to `from` to `value` HP — shared by the
// Digit3 (near-death) and Digit4 (full heal) debug actions.
pub(crate) fn set_nearest_zombie_health(
    zombies: &mut Query<(&Transform, &mut Health), With<Zombie>>,
    from: Vec3,
    value: f32,
) {
    let nearest = zombies.iter_mut().min_by(|(a, _), (b, _)| {
        a.translation
            .distance_squared(from)
            .total_cmp(&b.translation.distance_squared(from))
    });
    if let Some((_, mut health)) = nearest {
        health.0 = value;
    }
}

// Only live while the panel is open (see DebugPanelVisible/KeyP), so number
// keys don't do anything unexpected during normal play.
pub(crate) fn debug_panel_actions(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    panel_visible: Res<DebugPanelVisible>,
    asset_server: Res<AssetServer>,
    player: Query<&Transform, With<Player>>,
    camera: Query<&FpsCamera>,
    camera_transform: Query<&Transform, (With<FpsCamera>, Without<Player>)>,
    spatial_query: SpatialQuery,
    mut zombies: Query<(&Transform, &mut Health), With<Zombie>>,
    all_zombies: Query<Entity, With<Zombie>>,
    debug_bots: Query<&DebugBot>,
    mut selected_bot: ResMut<SelectedDebugBot>,
) {
    if !panel_visible.0 {
        return;
    }
    let Ok(player_transform) = player.single() else {
        return;
    };

    if keys.just_pressed(KeyCode::Digit1) {
        let forward = camera
            .single()
            .map(|cam| Quat::from_rotation_y(cam.yaw) * Vec3::NEG_Z)
            .unwrap_or(Vec3::NEG_Z);
        spawn_local_zombie(&mut commands, &asset_server, player_transform.translation + forward * 2.0);
    }
    if keys.just_pressed(KeyCode::Digit2) {
        for entity in &all_zombies {
            commands.entity(entity).try_despawn();
        }
    }
    if keys.just_pressed(KeyCode::Digit3) {
        set_nearest_zombie_health(&mut zombies, player_transform.translation, 1.0);
    }
    if keys.just_pressed(KeyCode::Digit4) {
        set_nearest_zombie_health(&mut zombies, player_transform.translation, ZOMBIE_MAX_HEALTH);
    }
    if keys.just_pressed(KeyCode::Digit5) {
        // Real spawning is a server decision now — just ask for more.
        commands.client_trigger(SpawnMoreZombies);
    }

    // 6: spawn a fake NetPlayer ("debug bot") a couple meters in front of
    // the local player, so multi-player zombie targeting can be tested
    // without a second real client — see on_net_player_replicated for how
    // it becomes visible. (Was KeyC, which collided with the always-on
    // crouch toggle — moved to the 6-9 range alongside the other
    // panel-only debug actions, same as 1-5.)
    if keys.just_pressed(KeyCode::Digit6) {
        let forward = camera
            .single()
            .map(|cam| Quat::from_rotation_y(cam.yaw) * Vec3::NEG_Z)
            .unwrap_or(Vec3::NEG_Z);
        commands.client_trigger(SpawnDebugBot {
            at: player_transform.translation + forward * 2.0,
        });
    }
    // [ / ]: cycle the selected bot through whichever ids are currently
    // replicated (sorted for a stable, predictable cycle order).
    if keys.just_pressed(KeyCode::BracketLeft) || keys.just_pressed(KeyCode::BracketRight) {
        let mut ids: Vec<u32> = debug_bots.iter().map(|bot| bot.0).collect();
        ids.sort_unstable();
        if !ids.is_empty() {
            let forward = keys.just_pressed(KeyCode::BracketRight);
            let next_index = match selected_bot.0.and_then(|current| ids.iter().position(|&id| id == current)) {
                Some(index) if forward => (index + 1) % ids.len(),
                Some(index) => (index + ids.len() - 1) % ids.len(),
                None => 0,
            };
            selected_bot.0 = Some(ids[next_index]);
        }
    }
    // 9: move the selected bot to wherever the local player is aiming
    // (same forward raycast gun_view_animation/shoot already use). Was
    // KeyX, which collided with the always-on prone toggle.
    if keys.just_pressed(KeyCode::Digit9) {
        if let Some(id) = selected_bot.0 {
            if let Ok(camera_transform) = camera_transform.single() {
                let filter = SpatialQueryFilter::default();
                let target = spatial_query
                    .cast_ray(camera_transform.translation, camera_transform.forward(), MAX_SHOT_DISTANCE, true, &filter)
                    .map(|hit| camera_transform.translation + camera_transform.forward() * hit.distance)
                    .unwrap_or(camera_transform.translation + camera_transform.forward() * 5.0);
                commands.client_trigger(MoveDebugBot { id, target });
            }
        }
    }
    // O: despawn the selected bot.
    if keys.just_pressed(KeyCode::KeyO) {
        if let Some(id) = selected_bot.0 {
            commands.client_trigger(DespawnDebugBot { id });
            selected_bot.0 = None;
        }
    }
}

pub(crate) fn spawn_crosshair(commands: &mut Commands) {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            GlobalZIndex(STATIC_UI_Z_INDEX),
            CrosshairRoot,
        ))
        .with_children(|root| {
            root.spawn(Node {
                width: Val::Px(CROSSHAIR_CONTAINER_PX),
                height: Val::Px(CROSSHAIR_CONTAINER_PX),
                position_type: PositionType::Relative,
                ..default()
            })
            .with_children(|cross| {
                for direction in [
                    Vec2::new(0.0, -1.0),
                    Vec2::new(0.0, 1.0),
                    Vec2::new(-1.0, 0.0),
                    Vec2::new(1.0, 0.0),
                ] {
                    cross.spawn((
                        // Sized/positioned every frame by update_crosshair —
                        // these are just placeholders until the first update.
                        Node {
                            position_type: PositionType::Absolute,
                            ..default()
                        },
                        BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.85)),
                        CrosshairLine { direction },
                    ));
                }
            });
        });
}

// Widens the crosshair's gap as the next shot's bullet spread grows — same
// formula shoot() actually fires with (current_bullet_spread), so the
// reticle always reflects real accuracy instead of just looking busy.
pub(crate) fn update_crosshair(
    mouse: Res<ButtonInput<MouseButton>>,
    touch_aiming: Res<TouchAiming>,
    cursor: Query<&CursorOptions, With<PrimaryWindow>>,
    camera: Query<&ViewRecoil, With<FpsCamera>>,
    player: Query<&PlayerStance, With<Player>>,
    mut root: Query<&mut Node, (With<CrosshairRoot>, Without<CrosshairLine>)>,
    mut lines: Query<(&CrosshairLine, &mut Node)>,
    local_moving: Res<LocalPlayerMoving>,
    local_sprinting: Res<LocalPlayerSprinting>,
) {
    let Ok(recoil) = camera.single() else {
        return;
    };
    let stance = player.single().copied().unwrap_or_default();
    let aiming = cursor.single().is_ok_and(|cursor| is_aiming(cursor, &mouse, touch_aiming.0));
    // Hidden entirely while ADS'd — the scope's own reticle takes over.
    if let Ok(mut root) = root.single_mut() {
        root.display = if aiming { Display::None } else { Display::Flex };
    }
    // LocalPlayerMoving/LocalPlayerSprinting instead of raw key checks —
    // see gun_view_animation's own comment on why (blind to touch input
    // and Autopilot's synthetic movement otherwise).
    let spread = current_bullet_spread(recoil.pitch_kick, aiming, local_moving.0, stance, local_sprinting.0);
    let gap = CROSSHAIR_BASE_GAP_PX + spread * CROSSHAIR_GAP_SPREAD_SCALE;

    for (line, mut node) in &mut lines {
        let near = Val::Px(CROSSHAIR_CENTER_PX - gap - CROSSHAIR_LINE_LENGTH_PX);
        let far = Val::Px(CROSSHAIR_CENTER_PX + gap);
        let centered_thickness = Val::Px(CROSSHAIR_CENTER_PX - CROSSHAIR_LINE_THICKNESS_PX / 2.0);
        if line.direction.x != 0.0 {
            node.width = Val::Px(CROSSHAIR_LINE_LENGTH_PX);
            node.height = Val::Px(CROSSHAIR_LINE_THICKNESS_PX);
            node.top = centered_thickness;
            node.left = if line.direction.x < 0.0 { near } else { far };
        } else {
            node.width = Val::Px(CROSSHAIR_LINE_THICKNESS_PX);
            node.height = Val::Px(CROSSHAIR_LINE_LENGTH_PX);
            node.left = centered_thickness;
            node.top = if line.direction.y < 0.0 { near } else { far };
        }
    }
}

// Toggles InspectMode on KeyL — bundles Noclip + hide-gun together (and
// restores both to their pre-inspect state on the way back out) so map
// debugging is one keypress instead of three. See InspectMode's own doc
// comment for why shoot() also needs to check it separately (this system
// only handles the fly/visibility side, not firing).
pub(crate) fn toggle_inspect_mode(
    keys: Res<ButtonInput<KeyCode>>,
    mut inspect: ResMut<InspectMode>,
    mut noclip: ResMut<Noclip>,
    mut gun_visible: ResMut<GunVisible>,
) {
    if !keys.just_pressed(KeyCode::KeyL) {
        return;
    }
    inspect.0 = !inspect.0;
    noclip.0 = inspect.0;
    gun_visible.0 = !inspect.0;
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn toggle_debug_flags(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut freeze: ResMut<FreezeZombies>,
    mut show_hitboxes: ResMut<ShowHitboxes>,
    mut god_mode: ResMut<GodMode>,
    mut panel_visible: ResMut<DebugPanelVisible>,
    mut aim_blur: ResMut<AimBlurEnabled>,
    mut dof_focal_alt: ResMut<AimDofFocalDistanceAlt>,
    mut zombie_anim_disabled: ResMut<ZombieAnimDisabled>,
    mut zombie_health_bars_hidden: ResMut<ZombieHealthBarsHidden>,
    mut zombie_lod_disabled: ResMut<ZombieLodDisabled>,
    mut player_model_visible: ResMut<PlayerModelVisible>,
    mut autopilot: ResMut<Autopilot>,
) {
    if keys.just_pressed(KeyCode::KeyF) {
        freeze.0 = !freeze.0;
        // Zombie position is server-authoritative — see SetFreezeZombies'
        // own doc comment for why this client-local flag alone (used below
        // by freeze_zombie_animations/zombies_damage_player) was never
        // enough to actually stop a zombie from moving.
        commands.client_trigger(SetFreezeZombies(freeze.0));
    }
    if keys.just_pressed(KeyCode::KeyH) {
        show_hitboxes.0 = !show_hitboxes.0;
    }
    if keys.just_pressed(KeyCode::KeyG) {
        god_mode.0 = !god_mode.0;
    }
    if keys.just_pressed(KeyCode::KeyP) {
        panel_visible.0 = !panel_visible.0;
    }
    if keys.just_pressed(KeyCode::KeyB) {
        aim_blur.0 = !aim_blur.0;
    }
    if keys.just_pressed(KeyCode::KeyV) {
        dof_focal_alt.0 = !dof_focal_alt.0;
    }
    if keys.just_pressed(KeyCode::KeyM) {
        zombie_anim_disabled.0 = !zombie_anim_disabled.0;
    }
    if keys.just_pressed(KeyCode::KeyJ) {
        zombie_health_bars_hidden.0 = !zombie_health_bars_hidden.0;
    }
    if keys.just_pressed(KeyCode::KeyK) {
        zombie_lod_disabled.0 = !zombie_lod_disabled.0;
    }
    if keys.just_pressed(KeyCode::KeyT) {
        player_model_visible.0 = !player_model_visible.0;
    }
    // I: master Autopilot on/off (client-local, crate::autopilot's own
    // decision loop — mirrors cadisplaylink::native_menu::set_autopilot's
    // iOS Settings-menu equivalent, just reached from a desktop hotkey
    // instead of a UIKit button, since that module is iOS-only
    // (#[cfg(target_os = "ios")]) and not reachable from here).
    if keys.just_pressed(KeyCode::KeyI) {
        autopilot.0 = !autopilot.0;
    }
}



use bevy::prelude::*;
use crate::*;

#[derive(Component)]
pub(crate) struct NativeUiRoot;

#[derive(Resource, Default)]
pub(crate) struct InspectMode(pub(crate) bool);

#[derive(Resource, Default)]
pub(crate) struct KillCount(pub(crate) u32);

pub(crate) const RECENTLY_HIT_ZOMBIE_TTL: f32 = 2.0;

#[derive(Resource, Default)]
pub(crate) struct RecentlyHitZombies(pub(crate) std::collections::HashMap<Entity, f32>);

pub(crate) fn count_local_kills(
    time: Res<Time>,
    mut removed_zombies: RemovedComponents<Zombie>,
    mut recently_hit: ResMut<RecentlyHitZombies>,
    mut kill_count: ResMut<KillCount>,
) {
    let dt = time.delta_secs();
    recently_hit.0.retain(|_, ttl| {
        *ttl -= dt;
        *ttl > 0.0
    });
    for entity in removed_zombies.read() {
        if recently_hit.0.remove(&entity).is_some() {
            kill_count.0 += 1;
        }
    }
}

#[derive(Component)]
pub(crate) struct HealthUi;

#[derive(Component)]
pub(crate) struct InspectUi;

#[derive(Component)]
pub(crate) struct FloatingDamageText {
    world_pos: Vec3,
    remaining: f32,
}

#[derive(Component)]
pub(crate) struct AmmoUi;

pub(crate) fn spawn_floating_damage(commands: &mut Commands, world_pos: Vec3, damage: f32) {
    let critical = damage >= HEAD_DAMAGE;
    commands.spawn((
        Text::new(format!("{damage:.1}")),
        TextFont {
            font_size: FontSize::Px(if critical {
                FLOATING_DAMAGE_CRIT_FONT_SIZE
            } else {
                FLOATING_DAMAGE_FONT_SIZE
            }),
            ..default()
        },
        TextColor(if critical {
            Color::srgb(1.0, 0.2, 0.15)
        } else {
            Color::WHITE
        }),
        Node {
            position_type: PositionType::Absolute,

            display: Display::None,
            ..default()
        },
        FloatingDamageText {
            world_pos,
            remaining: FLOATING_DAMAGE_LIFETIME,
        },
        NativeUiRoot,
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
            font_size: FontSize::Px(24.0),
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
        NativeUiRoot,
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

pub(crate) fn spawn_inspect_ui(commands: &mut Commands) {
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(18.0),
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
        NativeUiRoot,
        InspectUi,
    ));
}

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
        "INSPECT  x={:.2} y={:.2} z={:.2}  yaw={:.0} pitch={:.0}",
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
            font_size: FontSize::Px(24.0),
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
        NativeUiRoot,
    ));
}

#[derive(Component)]
pub(crate) struct PingUi;



pub(crate) fn heading_deg(yaw: f32) -> f32 {
    (-yaw.to_degrees()).rem_euclid(360.0)
}

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
            NativeUiRoot,
        ))
        .with_children(|root| {
            for &(heading, label) in &COMPASS_MARKS {
                root.spawn((
                    Text::new(label),
                    TextFont {
                        font_size: FontSize::Px(16.0),
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

    commands.spawn((
        Text::new("000"),
        TextFont {
            font_size: FontSize::Px(14.0),
            ..default()
        },
        TextColor(Color::srgb(1.0, 0.85, 0.2)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(12.0 + COMPASS_HEIGHT + 2.0),
            left: Val::Percent(50.0),
            ..default()
        },
        UiTransform::from_translation(Val2::percent(-50.0, 0.0)),
        GlobalZIndex(STATIC_UI_Z_INDEX),
        CompassHeadingText,
        NativeUiRoot,
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

        node.left = Val::Px(x - 8.0);
    }
    if let Ok(mut text) = heading_text.single_mut() {
        text.0 = format!("{:03.0}", current);
    }
}

#[derive(Component)]
pub(crate) struct RadarSweepLine {
    trail_index: u8,
}

#[derive(Component)]
pub(crate) struct RadarHeading;

#[derive(Component)]
pub(crate) struct RadarBlip {
    zombie: Entity,
}

fn spawn_radar_sweep_pivot(root: &mut ChildSpawnerCommands<'_>, radius: f32, marker: impl Component, line_color: Color) {
    root.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(RADAR_RADIUS - radius),
            top: Val::Px(RADAR_RADIUS - radius),
            width: Val::Px(radius * 2.0),
            height: Val::Px(radius * 2.0),
            ..default()
        },
        UiTransform::default(),
        marker,
    ))
    .with_children(|pivot| {
        pivot.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(radius - 1.5),
                top: Val::Px(radius),
                width: Val::Px(3.0),
                height: Val::Px(radius),
                ..default()
            },
            BackgroundColor(line_color),
        ));
    });
}

pub(crate) fn spawn_radar_ui(commands: &mut Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(RADAR_LEFT),
                top: Val::Px(RADAR_TOP),
                width: Val::Px(RADAR_SIZE),
                height: Val::Px(RADAR_SIZE),
                border_radius: BorderRadius::all(Val::Px(RADAR_RADIUS)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.05, 0.02, 0.55)),
            GlobalZIndex(STATIC_UI_Z_INDEX),
            NativeUiRoot,
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Px(RADAR_SIZE),
                    height: Val::Px(RADAR_SIZE),
                    border: UiRect::all(Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(RADAR_RADIUS)),
                    ..default()
                },
                BorderColor::all(Color::srgba(0.3, 1.0, 0.45, 0.6)),
            ));

            for ring_fraction in [0.34, 0.68] {
                let size = RADAR_SIZE * ring_fraction;
                root.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px((RADAR_SIZE - size) / 2.0),
                        top: Val::Px((RADAR_SIZE - size) / 2.0),
                        width: Val::Px(size),
                        height: Val::Px(size),
                        border: UiRect::all(Val::Px(1.0)),
                        border_radius: BorderRadius::all(Val::Px(size / 2.0)),
                        ..default()
                    },
                    BorderColor::all(Color::srgba(0.3, 1.0, 0.45, 0.25)),
                ));
            }

            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(RADAR_RADIUS - 0.5),
                    width: Val::Px(RADAR_SIZE),
                    height: Val::Px(1.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.3, 1.0, 0.45, 0.2)),
            ));
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(RADAR_RADIUS - 0.5),
                    top: Val::Px(0.0),
                    width: Val::Px(1.0),
                    height: Val::Px(RADAR_SIZE),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.3, 1.0, 0.45, 0.2)),
            ));

            for trail_index in 0..=RADAR_TRAIL_COUNT {
                let alpha = 0.85 * (1.0 - trail_index as f32 / (RADAR_TRAIL_COUNT as f32 + 1.0));
                spawn_radar_sweep_pivot(root, RADAR_RADIUS, RadarSweepLine { trail_index }, Color::srgba(0.4, 1.0, 0.5, alpha));
            }

            spawn_radar_sweep_pivot(root, RADAR_RADIUS * 0.4, RadarHeading, Color::srgb(1.0, 0.85, 0.2));

            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(RADAR_RADIUS - 3.0),
                    top: Val::Px(RADAR_RADIUS - 3.0),
                    width: Val::Px(6.0),
                    height: Val::Px(6.0),
                    border_radius: BorderRadius::all(Val::Px(3.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(1.0, 0.85, 0.2)),
            ));
        });
}

pub(crate) fn spawn_radar_blip(commands: &mut Commands, zombie: Entity) {
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            width: Val::Px(RADAR_BLIP_SIZE_PX),
            height: Val::Px(RADAR_BLIP_SIZE_PX),
            border_radius: BorderRadius::all(Val::Px(RADAR_BLIP_SIZE_PX / 2.0)),
            ..default()
        },
        BackgroundColor(Color::srgb(1.0, 0.25, 0.2)),
        GlobalZIndex(STATIC_UI_Z_INDEX),
        RadarBlip { zombie },
        NativeUiRoot,
    ));
}

pub(crate) fn update_radar_ui(
    time: Res<Time>,
    mut sweep_angle: Local<f32>,
    camera: Query<&FpsCamera>,
    player: Query<&Transform, With<Player>>,
    zombies: Query<&Transform, With<Zombie>>,
    mut commands: Commands,
    mut sweep_lines: Query<(&RadarSweepLine, &mut UiTransform)>,
    mut heading: Query<&mut UiTransform, (With<RadarHeading>, Without<RadarSweepLine>)>,
    mut blips: Query<(Entity, &RadarBlip, &mut Node)>,
) {
    *sweep_angle = (*sweep_angle + RADAR_SWEEP_DEG_PER_SEC * time.delta_secs()) % 360.0;
    for (line, mut transform) in &mut sweep_lines {
        transform.rotation = Rot2::degrees(*sweep_angle - line.trail_index as f32 * RADAR_TRAIL_STEP_DEG);
    }

    if let (Ok(cam), Ok(mut transform)) = (camera.single(), heading.single_mut()) {
        transform.rotation = Rot2::degrees(heading_deg(cam.yaw) - 180.0);
    }

    let Ok(player_transform) = player.single() else {
        return;
    };
    let origin = player_transform.translation;

    for (entity, blip, mut node) in &mut blips {
        let Ok(zombie_transform) = zombies.get(blip.zombie) else {
            commands.entity(entity).try_despawn();
            continue;
        };
        let offset = Vec2::new(zombie_transform.translation.x - origin.x, zombie_transform.translation.z - origin.z);
        let scaled = (offset / RADAR_RANGE * RADAR_RADIUS).clamp_length_max(RADAR_RADIUS - RADAR_BLIP_SIZE_PX / 2.0);
        node.left = Val::Px(RADAR_LEFT + RADAR_RADIUS + scaled.x - RADAR_BLIP_SIZE_PX / 2.0);
        node.top = Val::Px(RADAR_TOP + RADAR_RADIUS + scaled.y - RADAR_BLIP_SIZE_PX / 2.0);
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

#[derive(Component)]
pub(crate) struct CrosshairLine {
    direction: Vec2,
}

#[derive(Component)]
pub(crate) struct CrosshairRoot;

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
            NativeUiRoot,
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

    if let Ok(mut root) = root.single_mut() {
        root.display = if aiming { Display::None } else { Display::Flex };
    }

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
    mut aim_blur: ResMut<AimBlurEnabled>,
    mut dof_focal_alt: ResMut<AimDofFocalDistanceAlt>,
    mut zombie_anim_disabled: ResMut<ZombieAnimDisabled>,
    mut zombie_health_bars_hidden: ResMut<ZombieHealthBarsHidden>,
    mut player_model_visible: ResMut<PlayerModelVisible>,
    mut autopilot: ResMut<Autopilot>,
) {
    if keys.just_pressed(KeyCode::KeyF) {
        freeze.0 = !freeze.0;

        commands.trigger(SetFreezeZombies(freeze.0));
    }
    if keys.just_pressed(KeyCode::KeyH) {
        show_hitboxes.0 = !show_hitboxes.0;
    }
    if keys.just_pressed(KeyCode::KeyG) {
        god_mode.0 = !god_mode.0;
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
    if keys.just_pressed(KeyCode::KeyT) {
        player_model_visible.0 = !player_model_visible.0;
    }

    if keys.just_pressed(KeyCode::KeyI) {
        autopilot.0 = !autopilot.0;
    }
}

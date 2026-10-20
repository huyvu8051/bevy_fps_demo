
use bevy::prelude::*;
use crate::*;

#[derive(Resource)]
pub(crate) struct SettingsMenuVisible(pub(crate) bool);

#[derive(Resource, Default)]
pub(crate) struct MouseUiLocked(pub(crate) bool);

#[derive(Resource, Default)]
pub(crate) struct SettingsMenuScrollState {
    pub(crate) touch_id: Option<u64>,
    pub(crate) last_pos_y: f32,
}

#[derive(Component)]
pub(crate) struct SettingsMenuRoot;

#[derive(Resource, Default)]
pub(crate) struct SettingsMenuPageIndex(usize);

#[derive(Component)]
pub(crate) struct SettingsMenuPage(usize);

#[derive(Component)]
pub(crate) struct SettingsMenuPrevButton;
#[derive(Component)]
pub(crate) struct SettingsMenuNextButton;
#[derive(Component)]
pub(crate) struct SettingsMenuPageLabel;

#[derive(Component)]
pub(crate) struct ResolutionScaleButton(f32);

#[derive(Component)]
pub(crate) struct FpsCapButton(u32);

#[derive(Component)]
pub(crate) struct ShadowQualityButton(u8);

#[derive(Component)]
pub(crate) struct LightQualityButton(u8);

#[derive(Component)]
pub(crate) struct PointLightQualityButton(u8);

#[derive(Component)]
pub(crate) struct SettingsToggleButton;

#[derive(Component)]
pub(crate) struct MouseUiLockIndicator;

pub(crate) fn scaled_scene_extent(window_width: u32, window_height: u32, scale: f32) -> Extent3d {
    Extent3d {
        width: ((window_width as f32 * scale) as u32).max(RESOLUTION_SCALE_MIN_TEXTURE_PX),
        height: ((window_height as f32 * scale) as u32).max(RESOLUTION_SCALE_MIN_TEXTURE_PX),
        depth_or_array_layers: 1,
    }
}

pub(crate) fn resize_scene_render_target(
    windows: Query<&Window, With<PrimaryWindow>>,
    scale: Res<RenderResolutionScale>,
    target: Res<SceneRenderTarget>,
    mut images: ResMut<Assets<Image>>,
    mut applied: Local<(u32, u32, f32)>,
    mut pending: Local<Option<((u32, u32, f32), u8)>>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let current = (window.resolution.physical_width(), window.resolution.physical_height(), scale.0);
    if current == *applied {
        *pending = None;
        return;
    }
    match *pending {
        Some((size, streak)) if size == current => {
            if streak + 1 >= RESIZE_DEBOUNCE_FRAMES {
                *applied = current;
                *pending = None;
                if let Some(mut image) = images.get_mut(&target.0) {
                    image.resize(scaled_scene_extent(current.0, current.1, current.2));
                }
            } else {
                *pending = Some((size, streak + 1));
            }
        }
        _ => *pending = Some((current, 0)),
    }
}

pub(crate) fn spawn_settings_menu(commands: &mut Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(16.0),
                top: Val::Px(16.0),

                max_height: Val::Px(400.0),
                overflow: Overflow::scroll_y(),
                padding: UiRect::all(Val::Px(12.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
            ScrollPosition::default(),

            GlobalZIndex(STATIC_UI_Z_INDEX + 50),
            SettingsMenuRoot,
            NativeUiRoot,
        ))
        .with_children(|menu| {

            menu.spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(8.0),
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|row| {
                spawn_cheat_action_button(row, "<", SettingsMenuPrevButton);
                row.spawn((
                    Text::new("1/2"),
                    TextFont {
                        font_size: FontSize::Px(16.0),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                    SettingsMenuPageLabel,
                ));
                spawn_cheat_action_button(row, ">", SettingsMenuNextButton);
            });

            menu.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(8.0),
                    ..default()
                },
                SettingsMenuPage(0),
            ))
            .with_children(|page| {
                page.spawn((
                    Text::new("Resolution scale"),
                    TextFont {
                        font_size: FontSize::Px(16.0),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
                page.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(8.0),
                    ..default()
                })
                .with_children(|row| {
                    for &scale in &RESOLUTION_SCALE_PRESETS {
                        row.spawn((
                            Node {
                                width: Val::Px(60.0),
                                height: Val::Px(32.0),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                border_radius: BorderRadius::all(Val::Px(4.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.2)),
                            Button,
                            Interaction::default(),
                            ResolutionScaleButton(scale),
                        ))
                        .with_children(|btn| {
                            btn.spawn((
                                Text::new(format!("{:.0}%", scale * 100.0)),
                                TextFont {
                                    font_size: FontSize::Px(14.0),
                                    ..default()
                                },
                                TextColor(Color::WHITE),
                            ));
                        });
                    }
                });

                page.spawn((
                    Text::new("FPS cap"),
                    TextFont {
                        font_size: FontSize::Px(16.0),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
                page.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: Val::Px(8.0),
                    row_gap: Val::Px(8.0),
                    ..default()
                })
                .with_children(|row| {
                    for &cap in &FPS_CAP_PRESETS {
                        row.spawn((
                            Node {
                                width: Val::Px(50.0),
                                height: Val::Px(32.0),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                border_radius: BorderRadius::all(Val::Px(4.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.2)),
                            Button,
                            Interaction::default(),
                            FpsCapButton(cap),
                        ))
                        .with_children(|btn| {
                            btn.spawn((
                                Text::new(format!("{cap}")),
                                TextFont {
                                    font_size: FontSize::Px(14.0),
                                    ..default()
                                },
                                TextColor(Color::WHITE),
                            ));
                        });
                    }
                });

                page.spawn((
                    Text::new("Shadow quality"),
                    TextFont {
                        font_size: FontSize::Px(16.0),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
                page.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(8.0),
                    ..default()
                })
                .with_children(|row| {
                    for tier in 0..=5u8 {
                        row.spawn((
                            Node {
                                width: Val::Px(40.0),
                                height: Val::Px(32.0),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                border_radius: BorderRadius::all(Val::Px(4.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.2)),
                            Button,
                            Interaction::default(),
                            ShadowQualityButton(tier),
                        ))
                        .with_children(|btn| {
                            btn.spawn((
                                Text::new(tier.to_string()),
                                TextFont {
                                    font_size: FontSize::Px(14.0),
                                    ..default()
                                },
                                TextColor(Color::WHITE),
                            ));
                        });
                    }
                });

                page.spawn((
                    Text::new("Light quality"),
                    TextFont {
                        font_size: FontSize::Px(16.0),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
                page.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(8.0),
                    ..default()
                })
                .with_children(|row| {
                    for tier in 0..=5u8 {
                        row.spawn((
                            Node {
                                width: Val::Px(40.0),
                                height: Val::Px(32.0),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                border_radius: BorderRadius::all(Val::Px(4.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.2)),
                            Button,
                            Interaction::default(),
                            LightQualityButton(tier),
                        ))
                        .with_children(|btn| {
                            btn.spawn((
                                Text::new(tier.to_string()),
                                TextFont {
                                    font_size: FontSize::Px(14.0),
                                    ..default()
                                },
                                TextColor(Color::WHITE),
                            ));
                        });
                    }
                });

                page.spawn((
                    Text::new("Point light quality"),
                    TextFont {
                        font_size: FontSize::Px(16.0),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
                page.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(8.0),
                    ..default()
                })
                .with_children(|row| {
                    for tier in 0..=5u8 {
                        row.spawn((
                            Node {
                                width: Val::Px(40.0),
                                height: Val::Px(32.0),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                border_radius: BorderRadius::all(Val::Px(4.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.2)),
                            Button,
                            Interaction::default(),
                            PointLightQualityButton(tier),
                        ))
                        .with_children(|btn| {
                            btn.spawn((
                                Text::new(tier.to_string()),
                                TextFont {
                                    font_size: FontSize::Px(14.0),
                                    ..default()
                                },
                                TextColor(Color::WHITE),
                            ));
                        });
                    }
                });

                page.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(8.0),
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|row| {
                    spawn_cheat_toggle_button(row, "Autopilot", AutopilotButton);
                });

                page.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(8.0),
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|row| {
                    spawn_cheat_toggle_button(row, "Gun", CheatGunButton);
                });

                #[cfg(debug_assertions)]
                {
                    page.spawn((
                        Text::new("Cheats"),
                        TextFont {
                            font_size: FontSize::Px(16.0),
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                    page.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(8.0),
                        ..default()
                    })
                    .with_children(|row| {
                        spawn_cheat_toggle_button(row, "Noclip", CheatNoclipButton);
                        spawn_cheat_toggle_button(row, "God", CheatGodModeButton);
                        spawn_cheat_toggle_button(row, "Freeze", CheatFreezeButton);
                    });
                    page.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(8.0),
                        ..default()
                    })
                    .with_children(|row| {
                        spawn_cheat_toggle_button(row, "MSAA", CheatMsaaButton);
                        spawn_cheat_toggle_button(row, "Scope", CheatScopeCameraButton);
                        spawn_cheat_toggle_button(row, "Smoke", CheatSmokeButton);
                    });
                    page.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(8.0),
                        ..default()
                    })
                    .with_children(|row| {
                        spawn_cheat_toggle_button(row, "Hitboxes", CheatHitboxesButton);
                        spawn_cheat_toggle_button(row, "AimBlur", CheatAimBlurButton);
                    });
                    page.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(8.0),
                        ..default()
                    })
                    .with_children(|row| {
                        spawn_cheat_toggle_button(row, "NoZAnim", CheatZombieAnimButton);
                        spawn_cheat_toggle_button(row, "NoHPBar", CheatZombieHealthBarsButton);
                    });
                    page.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(8.0),
                        ..default()
                    })
                    .with_children(|row| {
                        spawn_cheat_toggle_button(row, "DofAlt", CheatDofAltButton);
                        spawn_cheat_toggle_button(row, "LookAt", CheatLookAtTargetButton);
                        spawn_cheat_toggle_button(row, "TraceAim", CheatShowAutopilotTargetButton);
                        spawn_cheat_toggle_button(row, "HitMarks", CheatShowHitMarkersButton);
                        spawn_cheat_toggle_button(row, "DmgNum", CheatShowFloatingDamageButton);
                        spawn_cheat_toggle_button(row, "Tracer", CheatShowTracersButton);
                    });
                    page.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(8.0),
                        ..default()
                    })
                    .with_children(|row| {

                        spawn_cheat_toggle_button(row, "NavDbg", CheatNavDebugButton);

                    });
                }
            });

            menu.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(8.0),
                    display: Display::None,
                    ..default()
                },
                SettingsMenuPage(1),
            ))
            .with_children(|page| {
                page.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(8.0),
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|row| {
                    spawn_cheat_action_button(row, "-10", ZombieCountDecButton);
                    spawn_cheat_action_button(row, "-1", ZombieCountDec1Button);
                    row.spawn((
                        Text::new("10"),
                        TextFont {
                            font_size: FontSize::Px(16.0),
                            ..default()
                        },
                        TextColor(Color::WHITE),
                        ZombieCountText,
                    ));
                    spawn_cheat_action_button(row, "+1", ZombieCountInc1Button);
                    spawn_cheat_action_button(row, "+10", ZombieCountIncButton);
                    spawn_cheat_toggle_button(row, "Auto", AutoMaintainButton);
                });
                page.spawn((
                    Text::new("Sensitivity"),
                    TextFont {
                        font_size: FontSize::Px(16.0),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
                page.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(8.0),
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|row| {
                    spawn_cheat_action_button(row, "-", SensitivityDecButton);
                    row.spawn((
                        Text::new("1.00x"),
                        TextFont {
                            font_size: FontSize::Px(16.0),
                            ..default()
                        },
                        TextColor(Color::WHITE),
                        SensitivityText,
                    ));
                    spawn_cheat_action_button(row, "+", SensitivityIncButton);
                });
                page.spawn((
                    Text::new("Damage"),
                    TextFont {
                        font_size: FontSize::Px(16.0),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
                page.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(8.0),
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|row| {
                    spawn_cheat_action_button(row, "-", DamageMultiplierDecButton);
                    row.spawn((
                        Text::new("1.00x"),
                        TextFont {
                            font_size: FontSize::Px(16.0),
                            ..default()
                        },
                        TextColor(Color::WHITE),
                        DamageMultiplierText,
                    ));
                    spawn_cheat_action_button(row, "+", DamageMultiplierIncButton);
                });

                #[cfg(debug_assertions)]
                {
                    page.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(8.0),
                        ..default()
                    })
                    .with_children(|row| {
                        spawn_cheat_action_button(row, "Kill All", CheatKillAllButton);
                        spawn_cheat_action_button(row, "+100", SpawnMoreZombiesButton);
                    });
                    page.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(8.0),
                        ..default()
                    })
                    .with_children(|row| {
                        spawn_cheat_action_button(row, "Heal Nearest", HealNearestButton);
                        spawn_cheat_action_button(row, "1HP Nearest", NearDeathNearestButton);
                    });

                    page.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(8.0),
                        ..default()
                    })
                    .with_children(|row| {
                        spawn_cheat_action_button(row, "Spawn 1", CheatSpawnZombieButton);
                        spawn_cheat_action_button(row, "Bot+", CheatSpawnBotButton);
                        spawn_cheat_action_button(row, "DelBot", CheatDespawnBotButton);
                    });
                    page.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(8.0),
                        ..default()
                    })
                    .with_children(|row| {
                        spawn_cheat_action_button(row, "<Bot", CheatPrevBotButton);
                        spawn_cheat_action_button(row, "Bot>", CheatNextBotButton);
                        spawn_cheat_action_button(row, "MoveBot", CheatMoveBotButton);
                    });
                }
            });
        });
}

pub(crate) fn spawn_cheat_button_node(parent: &mut ChildSpawnerCommands, label: &str, marker: impl Component) {
    parent
        .spawn((
            Node {
                width: Val::Px(96.0),
                height: Val::Px(36.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.2)),
            BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.5)),
            Button,
            Interaction::default(),
            marker,
        ))
        .with_children(|btn| {
            btn.spawn((
                Text::new(label),
                TextFont {
                    font_size: FontSize::Px(13.0),
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });
}

pub(crate) fn spawn_cheat_toggle_button(parent: &mut ChildSpawnerCommands, label: &str, marker: impl Component) {
    spawn_cheat_button_node(parent, label, marker);
}

pub(crate) fn spawn_cheat_action_button(parent: &mut ChildSpawnerCommands, label: &str, marker: impl Component) {
    spawn_cheat_button_node(parent, label, marker);
}


#[derive(Component)]
pub(crate) struct CheatNoclipButton;
#[derive(Component)]
pub(crate) struct CheatGodModeButton;
#[derive(Component)]
pub(crate) struct CheatFreezeButton;
#[derive(Component)]
pub(crate) struct CheatGunButton;
#[derive(Component)]
pub(crate) struct CheatMsaaButton;
#[derive(Component)]
pub(crate) struct CheatScopeCameraButton;

#[derive(Component)]
pub(crate) struct CheatSmokeButton;
#[derive(Component)]
pub(crate) struct CheatKillAllButton;
#[derive(Component)]
pub(crate) struct ZombieCountDecButton;
#[derive(Component)]
pub(crate) struct ZombieCountIncButton;
#[derive(Component)]
pub(crate) struct ZombieCountDec1Button;
#[derive(Component)]
pub(crate) struct ZombieCountInc1Button;
#[derive(Component)]
pub(crate) struct ZombieCountText;
#[derive(Component)]
pub(crate) struct AutoMaintainButton;
#[derive(Component)]
pub(crate) struct SensitivityDecButton;
#[derive(Component)]
pub(crate) struct SensitivityIncButton;
#[derive(Component)]
pub(crate) struct SensitivityText;
#[derive(Component)]
pub(crate) struct DamageMultiplierDecButton;
#[derive(Component)]
pub(crate) struct DamageMultiplierIncButton;
#[derive(Component)]
pub(crate) struct DamageMultiplierText;

#[derive(Resource)]
pub(crate) struct ZombieCountUi(pub(crate) u32);

#[derive(Resource)]
pub(crate) struct AutoMaintainUi(pub(crate) bool);

#[derive(Resource)]
pub(crate) struct DamageMultiplierUi(pub(crate) f32);
impl Default for DamageMultiplierUi {
    fn default() -> Self {
        Self(1.0)
    }
}

pub(crate) fn tapped<T: Component>(q: &Query<&Interaction, (With<T>, Changed<Interaction>)>) -> bool {
    q.iter().any(|i| *i == Interaction::Pressed)
}

#[derive(Component)]
pub(crate) struct CheatHitboxesButton;
#[derive(Component)]
pub(crate) struct CheatAimBlurButton;
#[derive(Component)]
pub(crate) struct CheatZombieAnimButton;
#[derive(Component)]
pub(crate) struct CheatZombieHealthBarsButton;
#[derive(Component)]
pub(crate) struct CheatDofAltButton;

#[derive(Component)]
pub(crate) struct AutopilotButton;
#[derive(Component)]
pub(crate) struct CheatSpawnZombieButton;
#[derive(Component)]
pub(crate) struct CheatSpawnBotButton;
#[derive(Component)]
pub(crate) struct CheatPrevBotButton;
#[derive(Component)]
pub(crate) struct CheatNextBotButton;
#[derive(Component)]
pub(crate) struct CheatMoveBotButton;
#[derive(Component)]
pub(crate) struct CheatDespawnBotButton;

#[derive(Component)]
pub(crate) struct CheatLookAtTargetButton;
#[derive(Component)]
pub(crate) struct CheatShowAutopilotTargetButton;
#[derive(Component)]
pub(crate) struct CheatShowHitMarkersButton;
#[derive(Component)]
pub(crate) struct CheatShowFloatingDamageButton;
#[derive(Component)]
pub(crate) struct CheatShowTracersButton;

#[derive(Component)]
pub(crate) struct CheatNavDebugButton;
pub(crate) fn cheat_toggle_buttons(
    mut commands: Commands,
    mut noclip: ResMut<Noclip>,
    mut god_mode: ResMut<GodMode>,
    mut freeze: ResMut<FreezeZombies>,
    mut gun_visible: ResMut<GunVisible>,
    mut map_msaa: ResMut<MapMsaaOn>,
    noclip_btn: Query<&Interaction, (With<CheatNoclipButton>, Changed<Interaction>)>,
    god_btn: Query<&Interaction, (With<CheatGodModeButton>, Changed<Interaction>)>,
    freeze_btn: Query<&Interaction, (With<CheatFreezeButton>, Changed<Interaction>)>,
    gun_btn: Query<&Interaction, (With<CheatGunButton>, Changed<Interaction>)>,
    msaa_btn: Query<&Interaction, (With<CheatMsaaButton>, Changed<Interaction>)>,
) {
    if tapped(&noclip_btn) {
        noclip.0 = !noclip.0;
    }

    if tapped(&god_btn) {
        god_mode.0 = !god_mode.0;
    }
    if tapped(&freeze_btn) {
        freeze.0 = !freeze.0;

        commands.trigger(SetFreezeZombies(freeze.0));
    }
    if tapped(&gun_btn) {
        gun_visible.0 = !gun_visible.0;
    }
    if tapped(&msaa_btn) {
        map_msaa.0 = !map_msaa.0;
    }
}

pub(crate) fn apply_map_render_toggles(
    gun_visible: Res<GunVisible>,
    map_msaa: Res<MapMsaaOn>,
    shadow_quality: Res<ShadowQuality>,
    light_quality: Res<LightQuality>,
    mut gun: Query<&mut Visibility, With<Gun>>,

    mut camera_msaa: Query<&mut Msaa, With<FpsCamera>>,
    mut sun: Query<&mut DirectionalLight, With<MainSun>>,
) {
    if let Ok(mut visibility) = gun.single_mut() {
        *visibility = if gun_visible.0 { Visibility::Inherited } else { Visibility::Hidden };
    }
    if let Ok(mut msaa) = camera_msaa.single_mut() {
        let target = if map_msaa.0 { Msaa::Sample4 } else { Msaa::Off };
        if *msaa != target {
            *msaa = target;
        }
    }
    if let Ok(mut light) = sun.single_mut() {
        let shadows_on = shadow_quality.0 > 0;
        if light.shadow_maps_enabled != shadows_on {
            light.shadow_maps_enabled = shadows_on;
        }
        let target_illuminance = MAIN_SUN_ILLUMINANCE * (light_quality.0.min(5) as f32 / 5.0);
        if light.illuminance != target_illuminance {
            light.illuminance = target_illuminance;
        }
    }
}

pub(crate) fn apply_point_light_quality(
    quality: Res<PointLightQuality>,
    mut lights: Query<(&mut PointLight, &PointLightBaseIntensity)>,
) {
    if !quality.is_changed() {
        return;
    }
    let fraction = quality.0.min(5) as f32 / 5.0;
    for (mut light, base) in &mut lights {
        let target = base.0 * fraction;
        if light.intensity != target {
            light.intensity = target;
        }
    }
}

pub(crate) fn cheat_toggle_buttons_2(
    mut show_hitboxes: ResMut<ShowHitboxes>,
    mut aim_blur: ResMut<AimBlurEnabled>,
    mut zombie_anim: ResMut<ZombieAnimDisabled>,
    mut zombie_health_bars: ResMut<ZombieHealthBarsHidden>,
    mut smoke: ResMut<SmokeEnabled>,
    hitboxes_btn: Query<&Interaction, (With<CheatHitboxesButton>, Changed<Interaction>)>,
    aim_blur_btn: Query<&Interaction, (With<CheatAimBlurButton>, Changed<Interaction>)>,
    zombie_anim_btn: Query<&Interaction, (With<CheatZombieAnimButton>, Changed<Interaction>)>,
    zombie_health_bars_btn: Query<&Interaction, (With<CheatZombieHealthBarsButton>, Changed<Interaction>)>,
    smoke_btn: Query<&Interaction, (With<CheatSmokeButton>, Changed<Interaction>)>,
) {
    if tapped(&hitboxes_btn) {
        show_hitboxes.0 = !show_hitboxes.0;
    }
    if tapped(&aim_blur_btn) {
        aim_blur.0 = !aim_blur.0;
    }
    if tapped(&zombie_anim_btn) {
        zombie_anim.0 = !zombie_anim.0;
    }
    if tapped(&zombie_health_bars_btn) {
        zombie_health_bars.0 = !zombie_health_bars.0;
    }
    if tapped(&smoke_btn) {
        smoke.0 = !smoke.0;
    }
}

pub(crate) fn cheat_toggle_buttons_3(
    mut dof_alt: ResMut<AimDofFocalDistanceAlt>,
    mut look_only: ResMut<AutopilotLookOnly>,
    mut show_aim: ResMut<ShowAutopilotTarget>,
    mut show_markers: ResMut<ShowHitMarkers>,
    mut show_floating_damage: ResMut<ShowFloatingDamage>,
    mut scope_camera: ResMut<ScopeCameraEnabled>,
    mut show_tracers: ResMut<ShowBulletTracers>,
    dof_alt_btn: Query<&Interaction, (With<CheatDofAltButton>, Changed<Interaction>)>,
    look_at_btn: Query<&Interaction, (With<CheatLookAtTargetButton>, Changed<Interaction>)>,
    show_aim_btn: Query<&Interaction, (With<CheatShowAutopilotTargetButton>, Changed<Interaction>)>,
    show_markers_btn: Query<&Interaction, (With<CheatShowHitMarkersButton>, Changed<Interaction>)>,
    show_floating_damage_btn: Query<&Interaction, (With<CheatShowFloatingDamageButton>, Changed<Interaction>)>,
    scope_camera_btn: Query<&Interaction, (With<CheatScopeCameraButton>, Changed<Interaction>)>,
    show_tracers_btn: Query<&Interaction, (With<CheatShowTracersButton>, Changed<Interaction>)>,
) {
    if tapped(&dof_alt_btn) {
        dof_alt.0 = !dof_alt.0;
    }
    if tapped(&look_at_btn) {
        look_only.0 = !look_only.0;
    }
    if tapped(&show_aim_btn) {
        show_aim.0 = !show_aim.0;
    }
    if tapped(&show_markers_btn) {
        show_markers.0 = !show_markers.0;
    }
    if tapped(&show_floating_damage_btn) {
        show_floating_damage.0 = !show_floating_damage.0;
    }
    if tapped(&show_tracers_btn) {
        show_tracers.0 = !show_tracers.0;
    }
    if tapped(&scope_camera_btn) {
        scope_camera.0 = !scope_camera.0;
    }
}

pub(crate) fn autopilot_toggle_button(mut autopilot: ResMut<Autopilot>, btn: Query<&Interaction, (With<AutopilotButton>, Changed<Interaction>)>) {
    if tapped(&btn) {
        autopilot.0 = !autopilot.0;
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn update_cheat_toggle_colors(
    noclip: Res<Noclip>,
    god_mode: Res<GodMode>,
    freeze: Res<FreezeZombies>,
    show_hitboxes: Res<ShowHitboxes>,
    aim_blur: Res<AimBlurEnabled>,
    zombie_anim: Res<ZombieAnimDisabled>,
    zombie_health_bars: Res<ZombieHealthBarsHidden>,
    gun_visible: Res<GunVisible>,
    map_msaa: Res<MapMsaaOn>,
    smoke: Res<SmokeEnabled>,
    mut buttons: Query<
        (
            &mut BackgroundColor,
            Option<&CheatNoclipButton>,
            Option<&CheatGodModeButton>,
            Option<&CheatFreezeButton>,
            Option<&CheatHitboxesButton>,
            Option<&CheatAimBlurButton>,
            Option<&CheatZombieAnimButton>,
            Option<&CheatZombieHealthBarsButton>,
            Option<&CheatGunButton>,
            Option<&CheatMsaaButton>,
            Option<&CheatSmokeButton>,
        ),
        Or<(
            With<CheatNoclipButton>,
            With<CheatGodModeButton>,
            With<CheatFreezeButton>,
            With<CheatHitboxesButton>,
            With<CheatAimBlurButton>,
            With<CheatZombieAnimButton>,
            With<CheatZombieHealthBarsButton>,
            With<CheatGunButton>,
            With<CheatMsaaButton>,
            With<CheatSmokeButton>,
        )>,
    >,
) {
    let color_for = |on: bool| if on { Color::srgba(0.3, 1.0, 0.4, 0.5) } else { Color::srgba(1.0, 1.0, 1.0, 0.2) };
    for (mut bg, noclip_m, god_m, freeze_m, hitboxes_m, aim_blur_m, zombie_anim_m, zombie_hp_m, gun_m, msaa_m, smoke_m) in
        &mut buttons
    {
        let on = if noclip_m.is_some() {
            noclip.0
        } else if god_m.is_some() {
            god_mode.0
        } else if freeze_m.is_some() {
            freeze.0
        } else if hitboxes_m.is_some() {
            show_hitboxes.0
        } else if aim_blur_m.is_some() {
            aim_blur.0
        } else if zombie_anim_m.is_some() {
            zombie_anim.0
        } else if zombie_hp_m.is_some() {
            zombie_health_bars.0
        } else if gun_m.is_some() {
            gun_visible.0
        } else if msaa_m.is_some() {
            map_msaa.0
        } else {
            smoke_m.is_some() && smoke.0
        };
        *bg = BackgroundColor(color_for(on));
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn update_extra_toggle_colors(
    autopilot: Res<Autopilot>,
    dof_alt: Res<AimDofFocalDistanceAlt>,
    look_only: Res<AutopilotLookOnly>,
    show_aim: Res<ShowAutopilotTarget>,
    show_markers: Res<ShowHitMarkers>,
    show_floating_damage: Res<ShowFloatingDamage>,
    scope_camera: Res<ScopeCameraEnabled>,
    show_tracers: Res<ShowBulletTracers>,
    mut buttons: Query<
        (
            &mut BackgroundColor,
            Option<&AutopilotButton>,
            Option<&CheatDofAltButton>,
            Option<&CheatLookAtTargetButton>,
            Option<&CheatShowAutopilotTargetButton>,
            Option<&CheatShowHitMarkersButton>,
            Option<&CheatShowFloatingDamageButton>,
            Option<&CheatScopeCameraButton>,
            Option<&CheatShowTracersButton>,
        ),
        Or<(
            With<AutopilotButton>,
            With<CheatDofAltButton>,
            With<CheatLookAtTargetButton>,
            With<CheatShowAutopilotTargetButton>,
            With<CheatShowHitMarkersButton>,
            With<CheatShowFloatingDamageButton>,
            With<CheatScopeCameraButton>,
            With<CheatShowTracersButton>,
        )>,
    >,
) {
    let color_for = |on: bool| if on { Color::srgba(0.3, 1.0, 0.4, 0.5) } else { Color::srgba(1.0, 1.0, 1.0, 0.2) };
    for (mut bg, auto_m, dof_m, look_m, show_aim_m, show_markers_m, show_floating_damage_m, scope_camera_m, show_tracers_m) in &mut buttons {
        let on = if auto_m.is_some() {
            autopilot.0
        } else if dof_m.is_some() {
            dof_alt.0
        } else if look_m.is_some() {
            look_only.0
        } else if show_aim_m.is_some() {
            show_aim.0
        } else if show_markers_m.is_some() {
            show_markers.0
        } else if show_floating_damage_m.is_some() {
            show_floating_damage.0
        } else if scope_camera_m.is_some() {
            scope_camera.0
        } else {
            show_tracers_m.is_some() && show_tracers.0
        };
        *bg = BackgroundColor(color_for(on));
    }
}

pub(crate) fn cheat_kill_all_button(mut commands: Commands, kill_btn: Query<&Interaction, (With<CheatKillAllButton>, Changed<Interaction>)>) {
    for interaction in &kill_btn {
        if *interaction == Interaction::Pressed {
            commands.trigger(ClearAllZombies);
        }
    }
}

#[derive(Component)]
pub(crate) struct SpawnMoreZombiesButton;
#[derive(Component)]
pub(crate) struct HealNearestButton;
#[derive(Component)]
pub(crate) struct NearDeathNearestButton;

pub(crate) fn cheat_zombie_health_buttons(
    mut commands: Commands,
    player: Query<&Transform, With<Player>>,
    mut zombies: Query<(&Transform, &mut Health), With<Zombie>>,
    spawn_more_btn: Query<&Interaction, (With<SpawnMoreZombiesButton>, Changed<Interaction>)>,
    heal_btn: Query<&Interaction, (With<HealNearestButton>, Changed<Interaction>)>,
    near_death_btn: Query<&Interaction, (With<NearDeathNearestButton>, Changed<Interaction>)>,
) {
    if tapped(&spawn_more_btn) {
        commands.trigger(SpawnMoreZombies);
    }
    let Ok(player_transform) = player.single() else {
        return;
    };
    if tapped(&heal_btn) {
        set_nearest_zombie_health(&mut zombies, player_transform.translation, ZOMBIE_MAX_HEALTH);
    }
    if tapped(&near_death_btn) {
        set_nearest_zombie_health(&mut zombies, player_transform.translation, 1.0);
    }
}

pub(crate) fn cheat_spawn_zombie_button(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    player: Query<&Transform, With<Player>>,
    camera: Query<&FpsCamera>,
    btn: Query<&Interaction, (With<CheatSpawnZombieButton>, Changed<Interaction>)>,
) {
    if !tapped(&btn) {
        return;
    }
    let Ok(player_transform) = player.single() else {
        return;
    };
    let forward = camera.single().map(|cam| Quat::from_rotation_y(cam.yaw) * Vec3::NEG_Z).unwrap_or(Vec3::NEG_Z);
    spawn_local_zombie(&mut commands, &asset_server, player_transform.translation + forward * 2.0);
}

pub(crate) fn zombie_count_buttons(
    mut commands: Commands,
    mut count: ResMut<ZombieCountUi>,
    dec_btn: Query<&Interaction, (With<ZombieCountDecButton>, Changed<Interaction>)>,
    inc_btn: Query<&Interaction, (With<ZombieCountIncButton>, Changed<Interaction>)>,
    dec1_btn: Query<&Interaction, (With<ZombieCountDec1Button>, Changed<Interaction>)>,
    inc1_btn: Query<&Interaction, (With<ZombieCountInc1Button>, Changed<Interaction>)>,
) {
    let max = shared::ZOMBIE_MAX_COUNT;
    let mut changed = false;
    for interaction in &dec_btn {
        if *interaction == Interaction::Pressed {
            count.0 = count.0.saturating_sub(10);
            changed = true;
        }
    }
    for interaction in &inc_btn {
        if *interaction == Interaction::Pressed {
            count.0 = (count.0 + 10).min(max);
            changed = true;
        }
    }
    for interaction in &dec1_btn {
        if *interaction == Interaction::Pressed {
            count.0 = count.0.saturating_sub(1);
            changed = true;
        }
    }
    for interaction in &inc1_btn {
        if *interaction == Interaction::Pressed {
            count.0 = (count.0 + 1).min(max);
            changed = true;
        }
    }
    if changed {
        commands.trigger(SetZombieCount(count.0));
    }
}

pub(crate) fn update_zombie_count_text(count: Res<ZombieCountUi>, mut text: Query<&mut Text, With<ZombieCountText>>) {
    if !count.is_changed() {
        return;
    }
    for mut text in &mut text {
        text.0 = format!("{}", count.0);
    }
}

pub(crate) fn sensitivity_buttons(
    mut scale: ResMut<SensitivityScale>,
    dec_btn: Query<&Interaction, (With<SensitivityDecButton>, Changed<Interaction>)>,
    inc_btn: Query<&Interaction, (With<SensitivityIncButton>, Changed<Interaction>)>,
) {
    for interaction in &dec_btn {
        if *interaction == Interaction::Pressed {
            scale.0 = (scale.0 - SENSITIVITY_SCALE_STEP).max(SENSITIVITY_SCALE_MIN);
        }
    }
    for interaction in &inc_btn {
        if *interaction == Interaction::Pressed {
            scale.0 = (scale.0 + SENSITIVITY_SCALE_STEP).min(SENSITIVITY_SCALE_MAX);
        }
    }
}

pub(crate) fn update_sensitivity_text(scale: Res<SensitivityScale>, mut text: Query<&mut Text, With<SensitivityText>>) {
    if !scale.is_changed() {
        return;
    }
    for mut text in &mut text {
        text.0 = format!("{:.2}x", scale.0);
    }
}

pub(crate) fn damage_multiplier_buttons(
    mut commands: Commands,
    mut multiplier: ResMut<DamageMultiplierUi>,
    dec_btn: Query<&Interaction, (With<DamageMultiplierDecButton>, Changed<Interaction>)>,
    inc_btn: Query<&Interaction, (With<DamageMultiplierIncButton>, Changed<Interaction>)>,
) {
    let mut changed = false;
    for interaction in &dec_btn {
        if *interaction == Interaction::Pressed {
            multiplier.0 = (multiplier.0 - DAMAGE_MULTIPLIER_STEP).max(DAMAGE_MULTIPLIER_MIN);
            changed = true;
        }
    }
    for interaction in &inc_btn {
        if *interaction == Interaction::Pressed {
            multiplier.0 = (multiplier.0 + DAMAGE_MULTIPLIER_STEP).min(DAMAGE_MULTIPLIER_MAX);
            changed = true;
        }
    }
    if changed {
        commands.trigger(SetDamageMultiplier(multiplier.0));
    }
}

pub(crate) fn update_damage_multiplier_text(multiplier: Res<DamageMultiplierUi>, mut text: Query<&mut Text, With<DamageMultiplierText>>) {
    if !multiplier.is_changed() {
        return;
    }
    for mut text in &mut text {
        text.0 = format!("{:.2}x", multiplier.0);
    }
}

pub(crate) fn auto_maintain_button(
    mut commands: Commands,
    mut auto_maintain: ResMut<AutoMaintainUi>,
    btn: Query<&Interaction, (With<AutoMaintainButton>, Changed<Interaction>)>,
) {
    for interaction in &btn {
        if *interaction == Interaction::Pressed {
            auto_maintain.0 = !auto_maintain.0;
            commands.trigger(SetAutoMaintainZombies(auto_maintain.0));
        }
    }
}

pub(crate) fn update_auto_maintain_button_color(auto_maintain: Res<AutoMaintainUi>, mut btn: Query<&mut BackgroundColor, With<AutoMaintainButton>>) {
    if !auto_maintain.is_changed() {
        return;
    }
    let color = if auto_maintain.0 { Color::srgba(0.3, 1.0, 0.4, 0.5) } else { Color::srgba(1.0, 1.0, 1.0, 0.2) };
    for mut bg in &mut btn {
        *bg = BackgroundColor(color);
    }
}

pub(crate) fn spawn_settings_toggle_button(commands: &mut Commands) {
    commands
        .spawn((
            Node {
                width: Val::Px(40.0),
                height: Val::Px(40.0),
                position_type: PositionType::Absolute,
                right: Val::Px(16.0),
                top: Val::Px(16.0),
                border: UiRect::all(Val::Px(2.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border_radius: BorderRadius::all(Val::Px(20.0)),
                ..default()
            },

            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.3)),
            TouchButtonBaseColor(Color::srgba(1.0, 1.0, 1.0, 0.3)),
            BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.6)),
            Button,
            Interaction::default(),
            GlobalZIndex(STATIC_UI_Z_INDEX),
            SettingsToggleButton,
            TouchControlButton,
            NativeUiRoot,
        ))
        .with_children(|btn| {

            btn.spawn((
                Text::new("=="),
                TextFont {
                    font_size: FontSize::Px(20.0),
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });
}

pub(crate) fn toggle_settings_menu(
    keys: Res<ButtonInput<KeyCode>>,
    mut visible: ResMut<SettingsMenuVisible>,
    mut was_pressed: Local<bool>,
    toggle_button: Query<&Interaction, With<SettingsToggleButton>>,
) {
    if keys.just_pressed(KeyCode::F1) {
        visible.0 = !visible.0;
    }
    let is_pressed = toggle_button.iter().any(|interaction| *interaction == Interaction::Pressed);
    if is_pressed && !*was_pressed {
        visible.0 = !visible.0;
    }
    *was_pressed = is_pressed;
}

pub(crate) fn update_settings_menu(visible: Res<SettingsMenuVisible>, mut menu: Query<&mut Node, With<SettingsMenuRoot>>) {
    let Ok(mut node) = menu.single_mut() else {
        return;
    };
    node.display = if visible.0 { Display::Flex } else { Display::None };
}

pub(crate) fn settings_menu_page_buttons(
    mut page_index: ResMut<SettingsMenuPageIndex>,
    prev_btn: Query<&Interaction, (With<SettingsMenuPrevButton>, Changed<Interaction>)>,
    next_btn: Query<&Interaction, (With<SettingsMenuNextButton>, Changed<Interaction>)>,
) {
    if tapped(&prev_btn) {
        page_index.0 = (page_index.0 + SETTINGS_MENU_PAGE_COUNT - 1) % SETTINGS_MENU_PAGE_COUNT;
    }
    if tapped(&next_btn) {
        page_index.0 = (page_index.0 + 1) % SETTINGS_MENU_PAGE_COUNT;
    }
}

pub(crate) fn update_settings_menu_pages(
    page_index: Res<SettingsMenuPageIndex>,
    mut pages: Query<(&SettingsMenuPage, &mut Node)>,
    mut label: Query<&mut Text, With<SettingsMenuPageLabel>>,
) {
    if !page_index.is_changed() {
        return;
    }
    for (page, mut node) in &mut pages {
        node.display = if page.0 == page_index.0 { Display::Flex } else { Display::None };
    }
    if let Ok(mut text) = label.single_mut() {
        text.0 = format!("{}/{}", page_index.0 + 1, SETTINGS_MENU_PAGE_COUNT);
    }
}

pub(crate) fn spawn_mouse_ui_lock_indicator(commands: &mut Commands) {
    commands.spawn((
        Text::new("UI LOCKED (Tab to unlock)"),
        TextFont {
            font_size: FontSize::Px(16.0),
            ..default()
        },
        TextColor(Color::srgb(1.0, 0.6, 0.2)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(16.0),
            left: Val::Percent(50.0),
            display: Display::None,
            ..default()
        },
        GlobalZIndex(STATIC_UI_Z_INDEX),
        MouseUiLockIndicator,
    ));
}

pub(crate) fn toggle_mouse_ui_lock(keys: Res<ButtonInput<KeyCode>>, mut locked: ResMut<MouseUiLocked>) {
    if keys.just_pressed(KeyCode::Tab) {
        locked.0 = !locked.0;
    }
}

pub(crate) fn suppress_ui_interaction_when_locked(locked: Res<MouseUiLocked>, mut buttons: Query<&mut Interaction, With<Button>>) {
    if !locked.0 {
        return;
    }
    for mut interaction in &mut buttons {
        if *interaction != Interaction::None {
            *interaction = Interaction::None;
        }
    }
}

pub(crate) fn hide_ui_when_locked(
    locked: Res<MouseUiLocked>,
    idle_hidden: Res<TouchControlsIdleHidden>,
    mut buttons: Query<&mut Node, (With<TouchControlButton>, Without<TouchJoystickBase>)>,
    mut joystick: Query<&mut Node, With<TouchJoystickBase>>,
) {
    if !locked.is_changed() && !idle_hidden.is_changed() {
        return;
    }
    let display = if locked.0 || idle_hidden.0 { Display::None } else { Display::Flex };
    for mut node in &mut buttons {
        node.display = display;
    }
    for mut node in &mut joystick {
        node.display = display;
    }
}

pub(crate) fn update_mouse_ui_lock_indicator(locked: Res<MouseUiLocked>, mut text: Query<&mut Node, With<MouseUiLockIndicator>>) {
    if !locked.is_changed() {
        return;
    }
    let Ok(mut node) = text.single_mut() else {
        return;
    };
    node.display = if locked.0 { Display::Flex } else { Display::None };
}

pub(crate) fn scroll_settings_menu(
    visible: Res<SettingsMenuVisible>,
    mut wheel: MessageReader<MouseWheel>,
    mut scroll_pos: Query<&mut ScrollPosition, With<SettingsMenuRoot>>,
) {
    if !visible.0 {
        wheel.clear();
        return;
    }
    let Ok(mut scroll_pos) = scroll_pos.single_mut() else {
        return;
    };
    for event in wheel.read() {
        let delta_y = match event.unit {
            MouseScrollUnit::Line => event.y * 24.0,
            MouseScrollUnit::Pixel => event.y,
        };
        scroll_pos.y = (scroll_pos.y - delta_y).max(0.0);
    }
}

pub(crate) fn resolution_scale_buttons(
    mut scale: ResMut<RenderResolutionScale>,
    buttons: Query<(&Interaction, &ResolutionScaleButton), Changed<Interaction>>,
) {
    for (interaction, button) in &buttons {
        if *interaction == Interaction::Pressed {
            scale.0 = button.0;
        }
    }
}

pub(crate) fn update_resolution_scale_button_colors(
    scale: Res<RenderResolutionScale>,
    mut buttons: Query<(&mut BackgroundColor, &ResolutionScaleButton)>,
) {
    for (mut bg, button) in &mut buttons {
        let on = button.0 == scale.0;
        *bg = BackgroundColor(if on { Color::srgba(0.3, 1.0, 0.4, 0.5) } else { Color::srgba(1.0, 1.0, 1.0, 0.2) });
    }
}

pub(crate) fn fps_cap_buttons(mut cap: ResMut<FpsCap>, buttons: Query<(&Interaction, &FpsCapButton), Changed<Interaction>>) {
    for (interaction, button) in &buttons {
        if *interaction == Interaction::Pressed {
            cap.0 = button.0;
        }
    }
}

pub(crate) fn update_fps_cap_button_colors(cap: Res<FpsCap>, mut buttons: Query<(&mut BackgroundColor, &FpsCapButton)>) {
    for (mut bg, button) in &mut buttons {
        let on = button.0 == cap.0;
        *bg = BackgroundColor(if on { Color::srgba(0.3, 1.0, 0.4, 0.5) } else { Color::srgba(1.0, 1.0, 1.0, 0.2) });
    }
}

pub(crate) fn shadow_quality_buttons(
    mut quality: ResMut<ShadowQuality>,
    buttons: Query<(&Interaction, &ShadowQualityButton), Changed<Interaction>>,
) {
    for (interaction, button) in &buttons {
        if *interaction == Interaction::Pressed {
            quality.0 = button.0;
        }
    }
}

pub(crate) fn update_shadow_quality_button_colors(
    quality: Res<ShadowQuality>,
    mut buttons: Query<(&mut BackgroundColor, &ShadowQualityButton)>,
) {
    for (mut bg, button) in &mut buttons {
        let on = button.0 == quality.0;
        *bg = BackgroundColor(if on { Color::srgba(0.3, 1.0, 0.4, 0.5) } else { Color::srgba(1.0, 1.0, 1.0, 0.2) });
    }
}

pub(crate) fn light_quality_buttons(
    mut quality: ResMut<LightQuality>,
    buttons: Query<(&Interaction, &LightQualityButton), Changed<Interaction>>,
) {
    for (interaction, button) in &buttons {
        if *interaction == Interaction::Pressed {
            quality.0 = button.0;
        }
    }
}

pub(crate) fn update_light_quality_button_colors(
    quality: Res<LightQuality>,
    mut buttons: Query<(&mut BackgroundColor, &LightQualityButton)>,
) {
    for (mut bg, button) in &mut buttons {
        let on = button.0 == quality.0;
        *bg = BackgroundColor(if on { Color::srgba(0.3, 1.0, 0.4, 0.5) } else { Color::srgba(1.0, 1.0, 1.0, 0.2) });
    }
}

pub(crate) fn point_light_quality_buttons(
    mut quality: ResMut<PointLightQuality>,
    buttons: Query<(&Interaction, &PointLightQualityButton), Changed<Interaction>>,
) {
    for (interaction, button) in &buttons {
        if *interaction == Interaction::Pressed {
            quality.0 = button.0;
        }
    }
}

pub(crate) fn update_point_light_quality_button_colors(
    quality: Res<PointLightQuality>,
    mut buttons: Query<(&mut BackgroundColor, &PointLightQualityButton)>,
) {
    for (mut bg, button) in &mut buttons {
        let on = button.0 == quality.0;
        *bg = BackgroundColor(if on { Color::srgba(0.3, 1.0, 0.4, 0.5) } else { Color::srgba(1.0, 1.0, 1.0, 0.2) });
    }
}


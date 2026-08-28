// --- Settings menu / cheat panel UI ---
// Extracted from lib.rs (Phase 2 of the gameplay-code-organization pass —
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

// Toggled by F1 or tapping SettingsToggleButton (desktop key + touch button,
// since this is a real user-facing menu, unlike the debug panel above which
// is keyboard-only). Drives the settings panel's Node.display in
// update_settings_menu, same show/hide pattern as DebugPanelVisible.
// OPEN_SETTINGS_MENU=1 starts the game with it already open — same idea as
// TOUCH_UI_PREVIEW/MAP_ID: a launch-time override so this (and, on iOS, the
// cadisplaylink Metal renderer's own draw of it) can be visually verified
// without having to tap/press F1 through every redeploy first.
#[derive(Resource)]
pub(crate) struct SettingsMenuVisible(pub(crate) bool);

// Toggled by Tab (desktop/hardware keyboard only — no touch button, since
// this exists specifically to stop a Bluetooth mouse's clicks from hitting
// touch UI buttons; a touchscreen player has no such problem). While
// locked, suppress_ui_interaction_when_locked forces every Button's
// Interaction back to None each frame, right after multi_touch_button_
// interaction/ui_focus_system compute it and before any *_button consumer
// system reads it — so a mouse click can never register as a UI tap, only
// as gameplay input (shoot()/reload_gun etc., which read ButtonInput<
// MouseButton>/KeyCode directly, not Interaction).
#[derive(Resource, Default)]
pub(crate) struct MouseUiLocked(pub(crate) bool);

// Which touch (if any) is currently dragging the settings menu's scroll
// area — touch_scroll_settings_menu's own equivalent of TouchMoveState/
// TouchLookState for the joystick/look-drag. scroll_settings_menu (desktop
// mouse wheel) already exists; this is the touch counterpart, since the
// menu's own Overflow::scroll_y() + ScrollPosition only gets driven by a
// wheel today — nothing drags it by finger on a touch-only device.
#[derive(Resource, Default)]
pub(crate) struct SettingsMenuScrollState {
    pub(crate) touch_id: Option<u64>,
    pub(crate) last_pos_y: f32,
}

#[derive(Component)]
pub(crate) struct SettingsMenuRoot;

// Which page of the settings menu is currently shown — the menu still
// scrolls too (see scroll_settings_menu/touch_scroll_settings_menu), but
// paging avoids ever needing to on a screen this small.
#[derive(Resource, Default)]
pub(crate) struct SettingsMenuPageIndex(usize);

// Marks each page container; update_settings_menu_pages compares this to
// SettingsMenuPageIndex to decide that page's Node.display. A child of
// SettingsMenuRoot, so it inherits the root's GlobalZIndex/stacking rather
// than needing its own.
#[derive(Component)]
pub(crate) struct SettingsMenuPage(usize);

#[derive(Component)]
pub(crate) struct SettingsMenuPrevButton;
#[derive(Component)]
pub(crate) struct SettingsMenuNextButton;
#[derive(Component)]
pub(crate) struct SettingsMenuPageLabel;

// One per RESOLUTION_SCALE_PRESETS entry — resolution_scale_buttons reads
// Interaction on these to know which preset was tapped/clicked.
#[derive(Component)]
pub(crate) struct ResolutionScaleButton(f32);

#[derive(Component)]
pub(crate) struct SettingsToggleButton;

// Marks the small "UI locked (Tab)" HUD text — update_mouse_ui_lock_indicator
// shows/hides it via Node.display in step with MouseUiLocked.
#[derive(Component)]
pub(crate) struct MouseUiLockIndicator;

// window physical size * scale, floored at RESOLUTION_SCALE_MIN_TEXTURE_PX
// on each axis so a tiny window at a low scale never asks wgpu for a
// zero-size (or otherwise degenerate) render target.
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
                if let Some(image) = images.get_mut(&target.0) {
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
                // Safety cap, not something the player is expected to hit
                // in normal use — content is split across
                // SettingsMenuPage(N) containers (settings_menu_page_
                // buttons/SettingsMenuPageIndex) instead.
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
            GlobalZIndex(STATIC_UI_Z_INDEX),
            SettingsMenuRoot,
        ))
        .with_children(|menu| {
            // Page nav row — a sibling of the SettingsMenuPage(N)
            // containers below (not inside one), so it's always visible
            // regardless of which page is active.
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
                        font_size: 16.0,
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
                        font_size: 16.0,
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
                                    font_size: 14.0,
                                    ..default()
                                },
                                TextColor(Color::WHITE),
                            ));
                        });
                    }
                });

                // Autopilot — real player-facing feature (see its own doc
                // comment), so unlike everything in the "Cheats" section
                // below this row is NOT debug-gated: it's the mobile
                // equivalent of desktop's always-available KeyI.
                page.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(8.0),
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|row| {
                    spawn_cheat_toggle_button(row, "Autopilot", AutopilotButton);
                });

                // Gun visibility — also a real player-facing feature (hide
                // the viewmodel for screenshots/clean framing), not a cheat,
                // so pulled out of the debug-only "Cheats" section below:
                // that gate meant this button silently didn't exist on any
                // release/mobile build, the only place touch players could
                // reach it at all (desktop has no keybinding for it either
                // — see native_menu's gun_visible for the one other place
                // this resource is toggled, a macOS/iOS app-menu item).
                page.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(8.0),
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|row| {
                    spawn_cheat_toggle_button(row, "Gun", CheatGunButton);
                });

                // Touch equivalent of the desktop debug panel's keyboard-only
                // cheats (KeyN/KeyG/KeyF/Digit1/Digit2) — that panel is
                // unreachable on mobile (no keyboard), so none of it existed
                // there before. Reuses the exact same Noclip/GodMode/
                // FreezeZombies resources and kill-all/spawn-one actions the
                // desktop keys already drive, just from a tap instead of a
                // keypress — same underlying behavior on both platforms.
                // Gated to debug builds only (previously spawned
                // unconditionally, in every build including release) — same
                // reasoning as the KeyP debug panel these mirror, which was
                // always implicitly "hidden" behind requiring a physical
                // keyboard but had no actual build-time gate either; a real
                // player shouldn't find a God Mode/Kill All button in their
                // Settings menu.
                #[cfg(debug_assertions)]
                {
                    page.spawn((
                        Text::new("Cheats"),
                        TextFont {
                            font_size: 16.0,
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
                        spawn_cheat_toggle_button(row, "Inspect", CheatInspectButton);
                    });
                    page.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(8.0),
                        ..default()
                    })
                    .with_children(|row| {
                        spawn_cheat_toggle_button(row, "MSAA", CheatMsaaButton);
                        spawn_cheat_toggle_button(row, "Shadow", CheatShadowButton);
                        spawn_cheat_toggle_button(row, "Scope", CheatScopeCameraButton);
                    });
                    page.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(8.0),
                        ..default()
                    })
                    .with_children(|row| {
                        spawn_cheat_toggle_button(row, "Hitboxes", CheatHitboxesButton);
                        spawn_cheat_toggle_button(row, "AimBlur", CheatAimBlurButton);
                        spawn_cheat_toggle_button(row, "PlayerMdl", CheatPlayerModelButton);
                    });
                    page.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(8.0),
                        ..default()
                    })
                    .with_children(|row| {
                        spawn_cheat_toggle_button(row, "NoZAnim", CheatZombieAnimButton);
                        spawn_cheat_toggle_button(row, "NoHPBar", CheatZombieHealthBarsButton);
                        spawn_cheat_toggle_button(row, "FullLOD", CheatZombieLodButton);
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
                        spawn_cheat_toggle_button(row, "Tracer", CheatShowTracersButton);
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
                    row.spawn((
                        Text::new("10"),
                        TextFont {
                            font_size: 16.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                        ZombieCountText,
                    ));
                    spawn_cheat_action_button(row, "+10", ZombieCountIncButton);
                    spawn_cheat_toggle_button(row, "Auto", AutoMaintainButton);
                });
                page.spawn((
                    Text::new("Sensitivity"),
                    TextFont {
                        font_size: 16.0,
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
                            font_size: 16.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                        SensitivityText,
                    ));
                    spawn_cheat_action_button(row, "+", SensitivityIncButton);
                });
                // Same debug-build-only gate as page 0's "Cheats" section
                // above — Kill All / Heal Nearest / 1HP Nearest are testing
                // tools, not something a real player should have.
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
                    // Mobile equivalents of debug_panel_actions' Digit1
                    // (spawn 1 local zombie) and the Digit6/[/]/Digit9/O
                    // "debug bot" fake-NetPlayer testing tools — see
                    // debug_bot_buttons' own doc comment.
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
}// Shared visual style for both cheat button flavors below — toggle buttons
// additionally get recolored by update_cheat_toggle_colors once spawned;
// action buttons (momentary, no on/off state) stay this same color always.
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
                    font_size: 13.0,
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
pub(crate) struct CheatInspectButton;
#[derive(Component)]
pub(crate) struct CheatGunButton;
#[derive(Component)]
pub(crate) struct CheatMsaaButton;
#[derive(Component)]
pub(crate) struct CheatShadowButton;
#[derive(Component)]
pub(crate) struct CheatScopeCameraButton;
#[derive(Component)]
pub(crate) struct CheatKillAllButton;
#[derive(Component)]
pub(crate) struct ZombieCountDecButton;
#[derive(Component)]
pub(crate) struct ZombieCountIncButton;
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

// Client's own best guess at the server's current zombie population
// target, so the cheat panel has a number to show and step from without
// waiting on a round-trip — SetZombieCount is fire-and-forget (see its own
// doc comment in crates/shared), the server never echoes the value back.
// Default matches ZombieTargetCount's own default on the server so the
// displayed number is right from the start, not just after the first tap.
#[derive(Resource)]
pub(crate) struct ZombieCountUi(pub(crate) u32);

// Same "client's own best guess" reasoning as ZombieCountUi — defaults to
// true to match AutoMaintainZombies' own server-side default.
#[derive(Resource)]
pub(crate) struct AutoMaintainUi(pub(crate) bool);

// True if any entity matching `T` was tapped/clicked this frame — shared by
// every touch/click button in this file that just needs a plain
// Changed<Interaction>-gated Pressed check (most of the cheat panel).
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
pub(crate) struct CheatZombieLodButton;
#[derive(Component)]
pub(crate) struct CheatPlayerModelButton;
#[derive(Component)]
pub(crate) struct CheatDofAltButton;
// NOT a "Cheat*" marker on purpose — Autopilot is a real player-facing
// feature (see its own module doc comment), not a debug cheat, so its
// button lives outside the #[cfg(debug_assertions)] "Cheats" section
// below, same reasoning as ResolutionScaleButton/SensitivityText.
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
// Mobile toggle for AutopilotLookOnly (see its own doc comment,
// autopilot.rs) — a debug/testing tool (forces the camera onto the
// nearest zombie without touching movement/fire), so it's gated inside
// the "Cheats" section like the other debug-only buttons, unlike the real
// AutopilotButton above.
#[derive(Component)]
pub(crate) struct CheatLookAtTargetButton;
#[derive(Component)]
pub(crate) struct CheatShowAutopilotTargetButton;
#[derive(Component)]
pub(crate) struct CheatShowHitMarkersButton;
#[derive(Component)]
pub(crate) struct CheatShowTracersButton;

// Changed<Interaction>-gated like every other touch button in this file —
// each tap flips the same resource the matching desktop key
// (N/G/F/H/B/M/J/K/T respectively) does. The full merged set of desktop
// debug_panel_actions' keyboard-only toggles — none of them were reachable
// on mobile at all before (no keyboard), see this fn's own callers.
pub(crate) fn cheat_toggle_buttons(
    mut commands: Commands,
    mut noclip: ResMut<Noclip>,
    mut inspect: ResMut<InspectMode>,
    mut god_mode: ResMut<GodMode>,
    mut freeze: ResMut<FreezeZombies>,
    mut gun_visible: ResMut<GunVisible>,
    mut map_msaa: ResMut<MapMsaaOn>,
    mut map_shadows: ResMut<MapShadowsOn>,
    noclip_btn: Query<&Interaction, (With<CheatNoclipButton>, Changed<Interaction>)>,
    inspect_btn: Query<&Interaction, (With<CheatInspectButton>, Changed<Interaction>)>,
    god_btn: Query<&Interaction, (With<CheatGodModeButton>, Changed<Interaction>)>,
    freeze_btn: Query<&Interaction, (With<CheatFreezeButton>, Changed<Interaction>)>,
    gun_btn: Query<&Interaction, (With<CheatGunButton>, Changed<Interaction>)>,
    msaa_btn: Query<&Interaction, (With<CheatMsaaButton>, Changed<Interaction>)>,
    shadow_btn: Query<&Interaction, (With<CheatShadowButton>, Changed<Interaction>)>,
) {
    if tapped(&noclip_btn) {
        noclip.0 = !noclip.0;
    }
    // Same bundling as desktop's toggle_inspect_mode (KeyL) — see
    // InspectMode's own doc comment.
    if tapped(&inspect_btn) {
        inspect.0 = !inspect.0;
        noclip.0 = inspect.0;
        gun_visible.0 = !inspect.0;
    }
    if tapped(&god_btn) {
        god_mode.0 = !god_mode.0;
    }
    if tapped(&freeze_btn) {
        freeze.0 = !freeze.0;
        // See toggle_debug_flags' KeyF handler for why this also needs to
        // reach the server, not just flip the local cosmetic flag.
        commands.client_trigger(SetFreezeZombies(freeze.0));
    }
    if tapped(&gun_btn) {
        gun_visible.0 = !gun_visible.0;
    }
    if tapped(&msaa_btn) {
        map_msaa.0 = !map_msaa.0;
    }
    if tapped(&shadow_btn) {
        map_shadows.0 = !map_shadows.0;
    }
}

// Applies GunVisible/MapMsaaOn/MapShadowsOn every frame — same
// resource-drives-a-component pattern as sync_player_model, cheap enough
// (3 single-entity queries) not to bother gating on resource_changed.
pub(crate) fn apply_map_render_toggles(
    gun_visible: Res<GunVisible>,
    map_msaa: Res<MapMsaaOn>,
    map_shadows: Res<MapShadowsOn>,
    mut gun: Query<&mut Visibility, With<Gun>>,
    // FpsCamera, not Camera3d — the AK scope's own render-to-texture
    // camera (see setup()) is ALSO a Camera3d, which would make
    // .single_mut() below error out with two matches.
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
        if light.shadows_enabled != map_shadows.0 {
            light.shadows_enabled = map_shadows.0;
        }
    }
}

// Split from cheat_toggle_buttons — one function with all 9 toggles' worth
// of params (18 total: 9 ResMut + 9 Query) exceeded the max arity Bevy's
// blanket impl supports for turning a plain fn into a System.
pub(crate) fn cheat_toggle_buttons_2(
    mut show_hitboxes: ResMut<ShowHitboxes>,
    mut aim_blur: ResMut<AimBlurEnabled>,
    mut zombie_anim: ResMut<ZombieAnimDisabled>,
    mut zombie_health_bars: ResMut<ZombieHealthBarsHidden>,
    mut zombie_lod: ResMut<ZombieLodDisabled>,
    mut player_model: ResMut<PlayerModelVisible>,
    hitboxes_btn: Query<&Interaction, (With<CheatHitboxesButton>, Changed<Interaction>)>,
    aim_blur_btn: Query<&Interaction, (With<CheatAimBlurButton>, Changed<Interaction>)>,
    zombie_anim_btn: Query<&Interaction, (With<CheatZombieAnimButton>, Changed<Interaction>)>,
    zombie_health_bars_btn: Query<&Interaction, (With<CheatZombieHealthBarsButton>, Changed<Interaction>)>,
    zombie_lod_btn: Query<&Interaction, (With<CheatZombieLodButton>, Changed<Interaction>)>,
    player_model_btn: Query<&Interaction, (With<CheatPlayerModelButton>, Changed<Interaction>)>,
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
    if tapped(&zombie_lod_btn) {
        zombie_lod.0 = !zombie_lod.0;
    }
    if tapped(&player_model_btn) {
        player_model.0 = !player_model.0;
    }
}

// Split out for the same arity reason as cheat_toggle_buttons_2 — this is
// the merged mobile equivalent of desktop's KeyV (see AimDofFocalDistanceAlt's
// own doc comment for what it A/B tests).
pub(crate) fn cheat_toggle_buttons_3(
    mut dof_alt: ResMut<AimDofFocalDistanceAlt>,
    mut look_only: ResMut<AutopilotLookOnly>,
    mut show_aim: ResMut<ShowAutopilotTarget>,
    mut show_markers: ResMut<ShowHitMarkers>,
    mut scope_camera: ResMut<ScopeCameraEnabled>,
    mut show_tracers: ResMut<ShowBulletTracers>,
    dof_alt_btn: Query<&Interaction, (With<CheatDofAltButton>, Changed<Interaction>)>,
    look_at_btn: Query<&Interaction, (With<CheatLookAtTargetButton>, Changed<Interaction>)>,
    show_aim_btn: Query<&Interaction, (With<CheatShowAutopilotTargetButton>, Changed<Interaction>)>,
    show_markers_btn: Query<&Interaction, (With<CheatShowHitMarkersButton>, Changed<Interaction>)>,
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
    if tapped(&show_tracers_btn) {
        show_tracers.0 = !show_tracers.0;
    }
    if tapped(&scope_camera_btn) {
        scope_camera.0 = !scope_camera.0;
    }
}

// Mobile equivalent of desktop's KeyI — see AutopilotButton's own doc
// comment for why this isn't grouped with the cheat_toggle_buttons* family
// despite the identical tapped()-flips-a-bool shape.
pub(crate) fn autopilot_toggle_button(mut autopilot: ResMut<Autopilot>, btn: Query<&Interaction, (With<AutopilotButton>, Changed<Interaction>)>) {
    if tapped(&btn) {
        autopilot.0 = !autopilot.0;
    }
}

// Recolors the 3 toggle buttons green-ish while their cheat is active —
// desktop's debug panel shows this as "[ON]"/"[OFF]" text instead, but
// these buttons have no room for that plus a label, so color stands in.
// One query with Option<&Marker> per button instead of 9 separate
// Query<&mut BackgroundColor, With<Marker>> params — Bevy's borrow checker
// can't prove 9 differently-filtered `&mut BackgroundColor` queries never
// alias the same entity without spelling out all 36 pairwise Without<>
// combinations, which Has<T>-via-Option sidesteps entirely by only ever
// borrowing BackgroundColor once per matched entity.
#[allow(clippy::type_complexity)]
pub(crate) fn update_cheat_toggle_colors(
    noclip: Res<Noclip>,
    god_mode: Res<GodMode>,
    freeze: Res<FreezeZombies>,
    show_hitboxes: Res<ShowHitboxes>,
    aim_blur: Res<AimBlurEnabled>,
    zombie_anim: Res<ZombieAnimDisabled>,
    zombie_health_bars: Res<ZombieHealthBarsHidden>,
    zombie_lod: Res<ZombieLodDisabled>,
    player_model: Res<PlayerModelVisible>,
    gun_visible: Res<GunVisible>,
    map_msaa: Res<MapMsaaOn>,
    map_shadows: Res<MapShadowsOn>,
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
            Option<&CheatZombieLodButton>,
            Option<&CheatPlayerModelButton>,
            Option<&CheatGunButton>,
            Option<&CheatMsaaButton>,
            Option<&CheatShadowButton>,
        ),
        Or<(
            With<CheatNoclipButton>,
            With<CheatGodModeButton>,
            With<CheatFreezeButton>,
            With<CheatHitboxesButton>,
            With<CheatAimBlurButton>,
            With<CheatZombieAnimButton>,
            With<CheatZombieHealthBarsButton>,
            With<CheatZombieLodButton>,
            With<CheatPlayerModelButton>,
            With<CheatGunButton>,
            With<CheatMsaaButton>,
            With<CheatShadowButton>,
        )>,
    >,
) {
    let color_for = |on: bool| if on { Color::srgba(0.3, 1.0, 0.4, 0.5) } else { Color::srgba(1.0, 1.0, 1.0, 0.2) };
    for (mut bg, noclip_m, god_m, freeze_m, hitboxes_m, aim_blur_m, zombie_anim_m, zombie_hp_m, zombie_lod_m, player_model_m, gun_m, msaa_m, shadow_m) in
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
        } else if zombie_lod_m.is_some() {
            zombie_lod.0
        } else if player_model_m.is_some() {
            player_model.0
        } else if gun_m.is_some() {
            gun_visible.0
        } else if msaa_m.is_some() {
            map_msaa.0
        } else {
            shadow_m.is_some() && map_shadows.0
        };
        *bg = BackgroundColor(color_for(on));
    }
}

// Same color-feedback idea as update_cheat_toggle_colors, split into its
// own query — AutopilotButton isn't a "Cheat*" marker (see its own doc
// comment), and folding a 14th Option<&Marker> into that fn's already-13-
// field tuple risked hitting Bevy's WorldQuery tuple arity ceiling for no
// real benefit.
#[allow(clippy::type_complexity)]
pub(crate) fn update_extra_toggle_colors(
    autopilot: Res<Autopilot>,
    dof_alt: Res<AimDofFocalDistanceAlt>,
    look_only: Res<AutopilotLookOnly>,
    show_aim: Res<ShowAutopilotTarget>,
    show_markers: Res<ShowHitMarkers>,
    scope_camera: Res<ScopeCameraEnabled>,
    show_tracers: Res<ShowBulletTracers>,
    inspect: Res<InspectMode>,
    mut buttons: Query<
        (
            &mut BackgroundColor,
            Option<&AutopilotButton>,
            Option<&CheatDofAltButton>,
            Option<&CheatLookAtTargetButton>,
            Option<&CheatShowAutopilotTargetButton>,
            Option<&CheatShowHitMarkersButton>,
            Option<&CheatScopeCameraButton>,
            Option<&CheatShowTracersButton>,
            Option<&CheatInspectButton>,
        ),
        Or<(
            With<AutopilotButton>,
            With<CheatDofAltButton>,
            With<CheatLookAtTargetButton>,
            With<CheatShowAutopilotTargetButton>,
            With<CheatShowHitMarkersButton>,
            With<CheatScopeCameraButton>,
            With<CheatShowTracersButton>,
            With<CheatInspectButton>,
        )>,
    >,
) {
    let color_for = |on: bool| if on { Color::srgba(0.3, 1.0, 0.4, 0.5) } else { Color::srgba(1.0, 1.0, 1.0, 0.2) };
    for (mut bg, auto_m, dof_m, look_m, show_aim_m, show_markers_m, scope_camera_m, show_tracers_m, inspect_m) in &mut buttons {
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
        } else if scope_camera_m.is_some() {
            scope_camera.0
        } else if show_tracers_m.is_some() {
            show_tracers.0
        } else {
            inspect_m.is_some() && inspect.0
        };
        *bg = BackgroundColor(color_for(on));
    }
}

// Kill All is authoritative (ClearAllZombies), unlike the old desktop-only
// debug panel's Digit2 action which just despawned the client's own local
// replicated copy and silently got it back on the next replication update
// (see ClearAllZombies' own doc comment in crates/shared) — worth fixing
// here since a real button inviting repeated taps would make that bug much
// more obvious/annoying than an easy-to-forget keyboard shortcut did.
pub(crate) fn cheat_kill_all_button(mut commands: Commands, kill_btn: Query<&Interaction, (With<CheatKillAllButton>, Changed<Interaction>)>) {
    for interaction in &kill_btn {
        if *interaction == Interaction::Pressed {
            commands.client_trigger(ClearAllZombies);
        }
    }
}

#[derive(Component)]
pub(crate) struct SpawnMoreZombiesButton;
#[derive(Component)]
pub(crate) struct HealNearestButton;
#[derive(Component)]
pub(crate) struct NearDeathNearestButton;

// Same 3 actions as debug_panel_actions' Digit5/Digit4/Digit3. Health writes
// here have the same caveat the desktop keys already had — Health is
// server-authoritative/replicated, so a client-side write like
// set_nearest_zombie_health's is cosmetic and gets overwritten by the next
// replication update; pre-existing behavior, not something new introduced
// by wiring it to a button instead of a key.
pub(crate) fn cheat_zombie_health_buttons(
    mut commands: Commands,
    player: Query<&Transform, With<Player>>,
    mut zombies: Query<(&Transform, &mut Health), With<Zombie>>,
    spawn_more_btn: Query<&Interaction, (With<SpawnMoreZombiesButton>, Changed<Interaction>)>,
    heal_btn: Query<&Interaction, (With<HealNearestButton>, Changed<Interaction>)>,
    near_death_btn: Query<&Interaction, (With<NearDeathNearestButton>, Changed<Interaction>)>,
) {
    if tapped(&spawn_more_btn) {
        commands.client_trigger(SpawnMoreZombies);
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

// Mobile equivalent of debug_panel_actions' Digit1 — spawns one local-only
// zombie 2m in front of the player, same as the desktop key.
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

// Mobile equivalent of debug_panel_actions' Digit6/BracketLeft/
// BracketRight/Digit9/KeyO — same body, just per-button instead of
// per-key. See that fn's own comments for why each action exists (fake
// NetPlayer "debug bot" for testing multiplayer zombie targeting without a
// second real client).
pub(crate) fn debug_bot_buttons(
    mut commands: Commands,
    player: Query<&Transform, With<Player>>,
    camera: Query<&FpsCamera>,
    camera_transform: Query<&Transform, (With<FpsCamera>, Without<Player>)>,
    spatial_query: SpatialQuery,
    debug_bots: Query<&DebugBot>,
    mut selected_bot: ResMut<SelectedDebugBot>,
    spawn_btn: Query<&Interaction, (With<CheatSpawnBotButton>, Changed<Interaction>)>,
    prev_btn: Query<&Interaction, (With<CheatPrevBotButton>, Changed<Interaction>)>,
    next_btn: Query<&Interaction, (With<CheatNextBotButton>, Changed<Interaction>)>,
    move_btn: Query<&Interaction, (With<CheatMoveBotButton>, Changed<Interaction>)>,
    despawn_btn: Query<&Interaction, (With<CheatDespawnBotButton>, Changed<Interaction>)>,
) {
    if tapped(&spawn_btn) {
        if let Ok(player_transform) = player.single() {
            let forward = camera.single().map(|cam| Quat::from_rotation_y(cam.yaw) * Vec3::NEG_Z).unwrap_or(Vec3::NEG_Z);
            commands.client_trigger(SpawnDebugBot { at: player_transform.translation + forward * 2.0 });
        }
    }
    let cycle_forward = tapped(&next_btn);
    if cycle_forward || tapped(&prev_btn) {
        let mut ids: Vec<u32> = debug_bots.iter().map(|bot| bot.0).collect();
        ids.sort_unstable();
        if !ids.is_empty() {
            let next_index = match selected_bot.0.and_then(|current| ids.iter().position(|&id| id == current)) {
                Some(index) if cycle_forward => (index + 1) % ids.len(),
                Some(index) => (index + ids.len() - 1) % ids.len(),
                None => 0,
            };
            selected_bot.0 = Some(ids[next_index]);
        }
    }
    if tapped(&move_btn) {
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
    if tapped(&despawn_btn) {
        if let Some(id) = selected_bot.0 {
            commands.client_trigger(DespawnDebugBot { id });
            selected_bot.0 = None;
        }
    }
}

// -/+ step the client's own tracked target by 10 (clamped to
// 0..=ZOMBIE_SPAWNS.len(), same bounds the server itself clamps to — see
// on_set_zombie_count) and send the new absolute value; update_zombie_count_text
// reflects ZombieCountUi.0 on screen immediately, no round-trip needed.
pub(crate) fn zombie_count_buttons(
    mut commands: Commands,
    mut count: ResMut<ZombieCountUi>,
    dec_btn: Query<&Interaction, (With<ZombieCountDecButton>, Changed<Interaction>)>,
    inc_btn: Query<&Interaction, (With<ZombieCountIncButton>, Changed<Interaction>)>,
) {
    let mut changed = false;
    for interaction in &dec_btn {
        if *interaction == Interaction::Pressed {
            count.0 = count.0.saturating_sub(10);
            changed = true;
        }
    }
    for interaction in &inc_btn {
        if *interaction == Interaction::Pressed {
            count.0 = (count.0 + 10).min(ZOMBIE_SPAWNS.len() as u32);
            changed = true;
        }
    }
    if changed {
        commands.client_trigger(SetZombieCount(count.0));
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

pub(crate) fn auto_maintain_button(
    mut commands: Commands,
    mut auto_maintain: ResMut<AutoMaintainUi>,
    btn: Query<&Interaction, (With<AutoMaintainButton>, Changed<Interaction>)>,
) {
    for interaction in &btn {
        if *interaction == Interaction::Pressed {
            auto_maintain.0 = !auto_maintain.0;
            commands.client_trigger(SetAutoMaintainZombies(auto_maintain.0));
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

// Small always-visible gear button (top-right) so touch users — who have no
// F1 key — can still reach the settings menu; toggle_settings_menu reads
// Interaction on it the same way touch_fire_button reads the fire button.
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
            // Alpha bumped + a border outline added, same reasoning as
            // spawn_touch_controls' own comment on why (too faint to notice
            // against a bright scene, reported as "can't see the menu
            // button" on-device).
            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.3)),
            BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.6)),
            Button,
            Interaction::default(),
            GlobalZIndex(STATIC_UI_Z_INDEX),
            SettingsToggleButton,
            TouchControlButton("=="),
        ))
        .with_children(|btn| {
            // Plain ASCII, not a Unicode glyph (e.g. gear "\u{2699}") —
            // Bevy's default bundled font is a Latin-subset font that
            // doesn't include most symbol-block characters, so a glyph
            // there would silently render as nothing instead of an error,
            // making the whole button look empty/invisible even though its
            // background+border were still there.
            btn.spawn((
                Text::new("=="),
                TextFont {
                    font_size: 20.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });
}

// F1 (desktop) or tapping SettingsToggleButton (any platform) flips
// SettingsMenuVisible; update_settings_menu applies it to the panel's
// Node.display. Changed<Interaction> (not a plain iter) so holding the
// button down doesn't toggle every single frame — only the instant it
// transitions to Pressed.
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

// Starts hidden (Display::None) — update_mouse_ui_lock_indicator flips it
// on only once the player actually locks UI input via Tab.
pub(crate) fn spawn_mouse_ui_lock_indicator(commands: &mut Commands) {
    commands.spawn((
        Text::new("UI LOCKED (Tab to unlock)"),
        TextFont {
            font_size: 16.0,
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

// Must run after multi_touch_button_interaction/Bevy's own ui_focus_system
// (both of which set Interaction from touch/mouse position) and before
// every *_button consumer system in this file — see MouseUiLocked's own
// doc comment. Forcing back to None (not just skipping) matters: without
// this, a click that started before the lock was toggled on could still
// read as Pressed by a downstream system this same frame.
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

// Display::None on a Button/the joystick base hides its entire subtree in
// one write — border, text children, and (for the joystick) the knob child
// all go with it, no separate per-component alpha bookkeeping needed. Also
// cheaper than dimming: a Display::None node is skipped by UI layout and
// never reaches the render extraction/draw step at all, vs. a dimmed node
// which still gets laid out and drawn (just with a low-alpha quad/glyphs)
// every frame. Only touches nodes that are Display::Flex to begin with
// (idle_only marker components toggling Display::None for other reasons —
// none currently apply to Button/joystick nodes — would otherwise get
// incorrectly forced back to Flex on unlock).
//
// Scoped to `TouchControlButton` (the Sprint/Jump/Crouch/Lean/... cluster
// this system actually exists for), NOT the bare `Button` marker — a plain
// `With<Button>` filter here used to also match Settings/GameOver/
// MapSelect buttons, which all use the same generic `Button` component.
// Harmless as long as those screens only stayed open for a few seconds
// (idle_elapsed never crossed TOUCH_CONTROLS_IDLE_HIDE_SECS), but became a
// real lockout once Map Select started waiting indefinitely for a tap
// before connecting: with no touches registered yet, the idle timer ran
// out and Display::None'd every Map Select button too — no touch reaches
// them once hidden, so there was no way back in.
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

// Desktop mouse wheel scroll for the settings menu — only while it's open
// (avoids stealing the same wheel adjust_scope_zoom already uses for scope
// zoom while aiming). MouseScrollUnit::Line (typical mouse wheel "clicks")
// vs ::Pixel (trackpad) get different multipliers since a "line" is a much
// bigger logical step than a trackpad pixel delta.
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

// Same Changed<Interaction>-gated Pressed check as toggle_settings_menu — a
// tap/click sets RenderResolutionScale directly; resize_scene_render_target
// picks up the change next frame.
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


use avian3d::prelude::*;
use bevy::app::AnimationSystems;
use bevy::transform::TransformSystems;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::RenderTarget;
use bevy::ecs::system::SystemParam;
use bevy::input::mouse::{MouseMotion, MouseScrollUnit, MouseWheel};
use bevy::picking::mesh_picking::MeshPickingPlugin;
use bevy::post_process::bloom::Bloom;
use bevy::post_process::dof::{DepthOfField, DepthOfFieldMode};
use bevy::prelude::*;
use bevy::asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
use bevy::window::{CursorGrabMode, CursorOptions, MonitorSelection, PrimaryWindow, Window, WindowMode};
use bevy::winit::{UpdateMode, WinitSettings};
use bevy_replicon::prelude::*;
use bevy_replicon::shared::backend::connected_client::NetworkId;

#[cfg(not(target_arch = "wasm32"))]
use bevy_replicon_renet2::netcode::NativeSocket;
use bevy_replicon_renet2::{
    netcode::{ClientAuthentication, NetcodeClientTransport},
    renet2::{ConnectionConfig, RenetClient},
    RenetChannelsExt, RepliconRenetPlugins,
};
use shared::{
    ClearAllZombies, ClientInput, DebugBot, DespawnDebugBot, find_animation_root, FireShot, GameOver, GameStateEvent, Health,
    HEAD_DAMAGE, HITBOX_BELLY_MIN_Y, HITBOX_CHEST_MIN_Y, HITBOX_HEAD_MIN_Y, HITBOX_LIMB_LATERAL_MIN,
    KnockbackVelocity, LastProcessedInput, MoveDebugBot, NetPlayer, NetTransform, Player, PlayerDied,
    ZombieAnimationPlayer,
    PlayerDiedFx, PlayerFiredFx,
    MapKind, scenario_player_spawn,
    PlayerStance, RequestRestart, Scenario, SetAutoMaintainZombies, SetDamageMultiplier, SetFreezeZombies, SetZombieCount, ShotConfirmed, ShotFx, SpawnDebugBot,
    SpawnMoreZombies, Zombie, ZombieAttackTimer, ZombieHitboxBones, ZombieKind, ZombieMotion, ZombieShowcaseClip, wire_zombie_hitbox_bones,
    zombie_kind_clip_count, zombie_kind_mesh_path, zombie_kind_anim_path,
    zombie_kind_pose_clips,
    INTERP_DELAY_SECS, MAX_SHOT_DISTANCE,
    HEAD_HITBOX_RADIUS, LIMB_HITBOX_RADIUS, TORSO_HITBOX_RADIUS,
    resolve_bone_world_positions, zombie_hit_damage_ray,
    PROTOCOL_ID,
    ZOMBIE_ATTACK_INTERVAL, ZOMBIE_LOD_DISTANCE, ZOMBIE_OUTER_HEIGHT, ZOMBIE_OUTER_RADIUS,
    zombie_pose_for, ZombiePose, ZOMBIE_MAX_HEALTH, ZOMBIE_SPAWNS, ZOMBIE_STOP_DISTANCE,
    ARENA_HALF_SIZE, ARENA_WALL_HEIGHT,
};

#[cfg(not(target_arch = "wasm32"))]
use shared::sim_latency::{sim_latency_from_env, LaggedSocket};

#[cfg(target_os = "ios")]
mod cadisplaylink;

mod autopilot;
use autopilot::{autopilot_pick_target, autopilot_think, Autopilot, AutopilotIntent, AutopilotLookOnly, AutopilotState};

mod touch_input;
use touch_input::*;

mod settings_menu;
use settings_menu::*;

mod movement_combat;
use movement_combat::*;

mod zombie_client;
use zombie_client::*;

mod vfx;
use vfx::*;

mod hud;
use hud::*;

mod character_tuning;
use character_tuning::*;

mod graphics_settings;
use graphics_settings::*;

mod scene_setup;
use scene_setup::*;

mod network_setup;
use network_setup::*;

mod item_gallery_cars;
use item_gallery_cars::*;

mod arena_layout;
use arena_layout::*;

mod map1_layout;
use map1_layout::*;

mod nav_debug;
use nav_debug::*;

mod settings_persistence;
use settings_persistence::*;

mod splash_screen;
use splash_screen::*;

const GUN_OFFSET: Vec3 = Vec3::new(0.22, -0.28, -0.65);

const GUN_ADS_OFFSET: Vec3 = Vec3::new(0.0, -0.307, -0.35);
const GUN_ADS_SPEED: f32 = 10.0;
const GUN_SCALE: f32 = 0.001;
const GUN_ROTATION_Y_DEG: f32 = -90.0;
const RECOIL_KICK: f32 = 0.12;

const RECOIL_KICK_UP: f32 = 0.01;
const RECOIL_TILT_UP_DEG: f32 = 4.0;

const RECOIL_ROLL_DEG: f32 = 3.0;
const RECOIL_YAW_KICK_DEG: f32 = 3.0;
const RECOIL_DURATION: f32 = 0.12;

const GUN_BOB_SPEED: f32 = 8.0;
const GUN_BOB_AMOUNT_X: f32 = 0.015;
const GUN_BOB_AMOUNT_Y: f32 = 0.012;
const GUN_BOB_BLEND_SPEED: f32 = 6.0;
const AIM_BOB_MULTIPLIER: f32 = 0.35;

const LOOK_SWAY_SENSITIVITY: f32 = 0.15;
const LOOK_SWAY_MAX_DEG: f32 = 0.5;
const LOOK_SWAY_SPEED: f32 = 10.0;

const WALL_GUN_RAISE_DISTANCE: f32 = 1.5;
const WALL_GUN_RAISE_HEIGHT: f32 = 0.18;
const WALL_GUN_RAISE_ROT_DEG: f32 = 90.0;
const WALL_GUN_RAISE_SPEED: f32 = 10.0;

const MAGAZINE_SIZE: u32 = 50;
const RELOAD_DURATION: f32 = 1.6;

const RELOAD_DIP: f32 = 0.12;
const RELOAD_TILT_DEG: f32 = 25.0;

const RELOAD_LEFT_DIP: f32 = 0.12;

const MUZZLE_OFFSET: Vec3 = Vec3::new(0.2, -0.005, -1.2);

const MUZZLE_FLASH_LIFETIME: f32 = 0.05;
const MUZZLE_LIGHT_INTENSITY: f32 = 120_000.0;
const MUZZLE_LIGHT_RANGE: f32 = 2.0;

const TRACER_CHANCE: f32 = 0.5;
const TRACER_LIFETIME: f32 = 0.045;
const TRACER_RADIUS: f32 = 0.012;

const TRACER_START_Y_OFFSET: f32 = -0.05;

const SMOKE_LIFETIME: f32 = 0.4;
const SMOKE_START_SCALE: f32 = 0.35;
const SMOKE_GROWTH_PER_SEC: f32 = 1.0;

const BLOOD_PARTICLE_COUNT: usize = 5;
const BLOOD_PARTICLE_SPEED: f32 = 2.0;
const BLOOD_PARTICLE_GRAVITY: f32 = -9.0;
const BLOOD_PARTICLE_LIFETIME: f32 = 0.6;
const BLOOD_PARTICLE_SIZE: f32 = 0.04;
const BLOOD_STAIN_SIZE: f32 = 0.12;
const BLOOD_STAIN_LIFETIME: f32 = 12.0;

const BLOOD_FLOOR_RAYCAST_DISTANCE: f32 = 5.0;
const BLOOD_FLOOR_STAIN_MIN: usize = 1;
const BLOOD_FLOOR_STAIN_MAX: usize = 3;
const BLOOD_FLOOR_STAIN_SPREAD: f32 = 0.3;

const SPARK_PARTICLE_MIN: usize = 4;
const SPARK_PARTICLE_MAX: usize = 9;
const SPARK_PARTICLE_SPEED: f32 = 3.0;
const SPARK_PARTICLE_GRAVITY: f32 = -9.0;
const SPARK_PARTICLE_LIFETIME: f32 = 0.25;
const SPARK_PARTICLE_SIZE: f32 = 0.02;

const FIRE_RATE: f32 = 0.1;

const RECOIL_PITCH_KICK: f32 = 0.022;
const RECOIL_PITCH_KICK_MAX: f32 = 0.16;
const RECOIL_YAW_JITTER: f32 = 0.012;
const RECOIL_RECOVERY_SPEED: f32 = 5.0;

const BULLET_SPREAD: f32 = 0.02;

const MOUSE_SENSITIVITY: f32 = 0.002;

const SENSITIVITY_SCALE_MIN: f32 = 0.25;
const SENSITIVITY_SCALE_MAX: f32 = 3.0;
const SENSITIVITY_SCALE_STEP: f32 = 0.5;

const DAMAGE_MULTIPLIER_MIN: f32 = 0.25;
const DAMAGE_MULTIPLIER_MAX: f32 = 20.0;
const DAMAGE_MULTIPLIER_STEP: f32 = 0.5;

const AIM_MOUSE_SENSITIVITY_MULTIPLIER: f32 = 0.5;

const PITCH_CLAMP: f32 = 1.54;

const TOUCH_JOYSTICK_BASE_PX: f32 = 140.0;
const TOUCH_JOYSTICK_KNOB_PX: f32 = 64.0;

const TOUCH_JOYSTICK_RADIUS_PX: f32 = (TOUCH_JOYSTICK_BASE_PX - TOUCH_JOYSTICK_KNOB_PX) / 2.0;
const TOUCH_FIRE_BUTTON_PX: f32 = 90.0;

const TOUCH_SCOPE_ZOOM_TRACK_WIDTH_PX: f32 = 44.0;
const TOUCH_SCOPE_ZOOM_TRACK_HEIGHT_PX: f32 = 200.0;
const TOUCH_SCOPE_ZOOM_HANDLE_PX: f32 = 40.0;

const STATIC_UI_Z_INDEX: i32 = 100;

const TOUCH_LOOK_SENSITIVITY_MULTIPLIER: f32 = 1.0;

impl Default for TouchControlsEnabled {
    fn default() -> Self {

        let mobile = cfg!(target_os = "ios") || cfg!(target_os = "android") || cfg!(target_arch = "wasm32");
        Self(mobile || std::env::var("TOUCH_UI_PREVIEW").is_ok())
    }
}

const CROSSHAIR_CONTAINER_PX: f32 = 50.0;
const CROSSHAIR_CENTER_PX: f32 = CROSSHAIR_CONTAINER_PX / 2.0;
const CROSSHAIR_LINE_LENGTH_PX: f32 = 6.0;
const CROSSHAIR_LINE_THICKNESS_PX: f32 = 2.0;
const CROSSHAIR_BASE_GAP_PX: f32 = 4.0;
const CROSSHAIR_GAP_SPREAD_SCALE: f32 = 500.0;

const BASE_FOV: f32 = std::f32::consts::PI / 4.0;
const AIM_FOV_MULTIPLIER: f32 = 0.6;

const AIM_ZOOM_SPEED: f32 = 12.0;
const AIM_SPREAD_MULTIPLIER: f32 = 0.35;

const AIM_DOF_FOCAL_DISTANCE_A: f32 = 0.2;
const AIM_DOF_FOCAL_DISTANCE_B: f32 = 0.25;
const AIM_DOF_APERTURE_F_STOPS: f32 = 11.0;

const RESOLUTION_SCALE_PRESETS: [f32; 7] = [0.2, 0.3, 0.4, 0.5, 0.65, 0.8, 1.0];

const FPS_CAP_PRESETS: [u32; 4] = [120, 80, 60, 30];

const SCOPE_TEXTURE_SIZE: u32 = 512;
const SCOPE_MIN_ZOOM: f32 = 6.0;
const SCOPE_MAX_ZOOM: f32 = 25.0;
const SCOPE_DEFAULT_ZOOM: f32 = 8.0;

const SCOPE_ZOOM_STEP_FACTOR: f32 = 1.15;
const SCOPE_BASE_FOV: f32 = BASE_FOV;

const AUTO_ZOOM_NEAR_DISTANCE: f32 = 10.0;
const AUTO_ZOOM_FAR_DISTANCE: f32 = 100.0;

const SCOPE_LENS_OFFSET: Vec3 = Vec3::new(0.0, 0.0, -0.2);

const SCOPE_LENS_SIZE: f32 = 0.03;

const SCOPE_LENS_HIDDEN_DROP: f32 = 0.03;

const SCOPE_CAMERA_HEIGHT_OFFSET: f32 = 0.0;

const SCOPE_HIP_HEIGHT_OFFSET: f32 = 0.05;

const SCOPE_ACTIVE_ADS_THRESHOLD: f32 = 0.02;
const SCOPE_HIDDEN_WALL_RAISE_THRESHOLD: f32 = 0.2;

const WORLD_LAYER: usize = 0;
const VIEWMODEL_LAYER: usize = 1;

const SCOPE_RETICLE_LAYER: usize = 2;
const SCOPE_RETICLE_DISTANCE: f32 = 0.3;

const SCOPE_RETICLE_WIDTH: f32 = 0.001;
const SCOPE_RETICLE_HEIGHT: f32 = 0.002;

const STATIONARY_SPREAD_MULTIPLIER: f32 = 0.4;

const SPRINT_SPREAD_MULTIPLIER: f32 = 2.2;
const CROUCH_SPREAD_MULTIPLIER: f32 = 0.6;
const PRONE_SPREAD_MULTIPLIER: f32 = 0.2;

const LEAN_SPEED: f32 = 6.0;
const LEAN_ROLL_DEG: f32 = 15.0;
const LEAN_SIDE_OFFSET: f32 = 0.4;

const EYE_HEIGHT_OFFSET: f32 = 0.65;

const CROUCH_EYE_HEIGHT_OFFSET: f32 = EYE_HEIGHT_OFFSET * 0.55;
const PRONE_EYE_HEIGHT_OFFSET: f32 = EYE_HEIGHT_OFFSET * 0.15;

const THIRD_PERSON_DISTANCE: f32 = 2.6;
const THIRD_PERSON_HEIGHT_OFFSET: f32 = 0.35;
const THIRD_PERSON_COLLISION_MARGIN: f32 = 0.3;

const THIRD_PERSON_SIDE_OFFSET: f32 = 0.9;
const THIRD_PERSON_SHOULDER_BIAS: f32 = 0.4;

const THIRD_PERSON_LOOK_AHEAD_DISTANCE: f32 = 8.0;

const HIT_MARKER_LIFETIME: f32 = 3.0;

const PLAYER_MAX_HEALTH: f32 = 100.0;

const ZOMBIE_ATTACK_DAMAGE: f32 = 12.0;

const FLOATING_DAMAGE_LIFETIME: f32 = 0.9;
const FLOATING_DAMAGE_RISE_SPEED: f32 = 0.7;
const FLOATING_DAMAGE_FONT_SIZE: f32 = 14.0;
const FLOATING_DAMAGE_CRIT_FONT_SIZE: f32 = 24.0;

const ZOMBIE_ANIM_FULL_DISTANCE: f32 = 8.0;
const ZOMBIE_ANIM_THROTTLE_HZ: f32 = 12.0;

const ZOMBIE_ANIM_CROWD_THRESHOLD_1: usize = 100;
const ZOMBIE_ANIM_CROWD_HZ_1: f32 = 12.0;
const ZOMBIE_ANIM_CROWD_THRESHOLD_2: usize = 200;
const ZOMBIE_ANIM_CROWD_HZ_2: f32 = 5.0;
const ZOMBIE_ANIM_CROWD_THRESHOLD_3: usize = 500;
const ZOMBIE_ANIM_CROWD_HZ_3: f32 = 2.0;

const ZOMBIE_HEALTH_BAR_HEIGHT: f32 = 1.3;
const ZOMBIE_HEALTH_BAR_WIDTH_PX: f32 = 50.0;
const ZOMBIE_HEALTH_BAR_HEIGHT_PX: f32 = 6.0;

const ZOMBIE_HEALTH_BAR_MAX_VISIBLE: usize = 30;

impl Default for SettingsMenuVisible {
    fn default() -> Self {
        Self(std::env::var("OPEN_SETTINGS_MENU").is_ok())
    }
}

impl Default for MapSelectionPending {
    fn default() -> Self {
        Self(network_setup::map_id_override().is_none())
    }
}

const SETTINGS_MENU_PAGE_COUNT: usize = 2;

impl Default for GunVisible {
    fn default() -> Self {
        Self(true)
    }
}

#[derive(Component)]
struct MainSun;

const MAIN_SUN_ILLUMINANCE: f32 = 8_000.0;

#[derive(Resource, Default)]
struct LocalPlayerMoving(bool);

#[derive(Resource, Default)]
struct LocalPlayerSprinting(bool);

#[derive(Resource, Default)]
struct LocalPlayerMoveAxis(Vec2);

#[derive(Resource, Default)]
struct SelectedDebugBot(Option<u32>);

#[derive(Resource, Default)]
struct PerfTimings {
    move_zombies_ms: f32,
    health_bars_ms: f32,
    zombie_count: usize,

    near_zombie_count: usize,
}

#[derive(Resource, Default)]
struct MouseLookDelta(Vec2);

const RECONNECT_INTERVAL: f32 = 2.0;

const INTERP_BUFFER_MAX_AGE_SECS: f32 = 0.5;

#[bevy_main]
pub fn main() {

    #[cfg(target_arch = "wasm32")]
    network_setup::set_map_id(2);

    let mut app = App::new();
    app.add_plugins((

            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Last Day in Saigon".to_string(),

                        mode: if cfg!(target_os = "android") {
                            WindowMode::BorderlessFullscreen(MonitorSelection::Current)
                        } else {
                            WindowMode::Windowed
                        },

                        canvas: if cfg!(target_arch = "wasm32") { Some("#bevy-canvas".to_string()) } else { None },
                        fit_canvas_to_parent: cfg!(target_arch = "wasm32"),
                        ..default()
                    }),
                    ..default()
                })

                .set(TaskPoolPlugin {
                    task_pool_options: if cfg!(target_os = "ios") || cfg!(target_os = "android") {
                        TaskPoolOptions::with_num_threads(4)
                    } else {
                        TaskPoolOptions::default()
                    },
                }),
            MeshPickingPlugin,
            PhysicsPlugins::default(),

            RepliconPlugins,
            RepliconRenetPlugins,
        ))

        .insert_resource(if cfg!(target_os = "ios") || cfg!(target_os = "android") {
            WinitSettings {
                focused_mode: UpdateMode::reactive(std::time::Duration::from_secs_f32(1.0 / 120.0)),
                ..WinitSettings::game()
            }
        } else {

            let cap = std::env::var("FPS_CAP")
                .ok()
                .and_then(|s| s.parse::<f32>().ok())
                .filter(|c| *c > 0.0)
                .unwrap_or(60.0);
            WinitSettings {
                focused_mode: UpdateMode::reactive(std::time::Duration::from_secs_f32(1.0 / cap)),
                unfocused_mode: UpdateMode::reactive_low_power(std::time::Duration::from_secs_f32(1.0 / 10.0)),
                ..WinitSettings::game()
            }
        });

    if std::env::var("EDITOR").is_ok() {
        app.add_plugins(bevy_editor_pls::EditorPlugin::default());

        app.add_systems(Update, fill_ambient_on_editor_cameras);

    }
    app.add_plugins(GamePlugin);
    app.run();
}

fn fill_ambient_on_editor_cameras(
    mut commands: Commands,
    cams: Query<Entity, (With<Camera3d>, Without<AmbientLight>)>,
) {
    for cam in &cams {
        commands.entity(cam).insert(AmbientLight {
            color: Color::WHITE,
            brightness: ambient_brightness(),
            ..default()
        });
    }
}

pub(crate) fn debug_keys_allowed(editor: Option<Res<bevy_editor_pls::editor::Editor>>) -> bool {
    !editor.is_some_and(|e| e.active())
}

pub(crate) struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
    app.replicate::<NetworkId>()
        .replicate::<NetPlayer>()
        .replicate::<NetTransform>()
        .replicate::<Zombie>()
        .replicate::<ZombieKind>()
        .replicate::<Health>()
        .replicate::<LastProcessedInput>()
        .replicate::<DebugBot>()

        .init_resource::<PlayerModelPoseOverride>()
        .register_type::<PlayerModelPose>()
        .register_type::<ZombiePose>()
        .register_type::<PlayerModelPoseOverride>()
        .add_client_event::<ClientInput>(Channel::Unreliable)
        .add_client_event::<SpawnMoreZombies>(Channel::Unordered)
        .add_client_event::<FireShot>(Channel::Ordered)
        .add_server_event::<ShotFx>(Channel::Ordered)
        .add_server_event::<ShotConfirmed>(Channel::Ordered)
        .add_server_event::<PlayerFiredFx>(Channel::Unordered)
        .add_server_event::<PlayerDiedFx>(Channel::Ordered)
        .add_client_event::<PlayerDied>(Channel::Ordered)
        .add_client_event::<RequestRestart>(Channel::Ordered)
        .add_server_event::<GameStateEvent>(Channel::Ordered)
        .add_client_event::<SpawnDebugBot>(Channel::Ordered)
        .add_client_event::<MoveDebugBot>(Channel::Ordered)
        .add_client_event::<DespawnDebugBot>(Channel::Ordered)
        .add_client_event::<SetZombieCount>(Channel::Ordered)
        .add_client_event::<ClearAllZombies>(Channel::Ordered)
        .add_client_event::<SetAutoMaintainZombies>(Channel::Ordered)
        .add_client_event::<SetFreezeZombies>(Channel::Ordered)
        .add_client_event::<SetDamageMultiplier>(Channel::Ordered)
        .init_resource::<ZombieCountUi>()
        .init_resource::<AutoMaintainUi>()

        .init_state::<AppState>()
        .add_systems(
            Update,
            (
                log_client_connection_state,
                mirror_local_player_net_transform,
                (buffer_net_transform, interpolate_net_transform).chain(),
                auto_reconnect,
            ),
        )
        .add_systems(OnExit(ClientState::Connected), despawn_replicated_on_disconnect)
        .add_observer(mark_local_player)
        .add_observer(on_zombie_replicated)
        .add_observer(on_zombie_kind_replicated)
        .add_observer(on_net_transform_replicated)
        .add_observer(on_net_player_replicated)
        .add_observer(on_shot_fx)
        .add_observer(on_shot_confirmed)
        .add_observer(on_player_fired_fx)
        .add_observer(on_player_died_fx)
        .add_observer(on_game_state_event)
        .add_plugins(ItemGalleryCarsPlugin)
        .add_plugins(ArenaLayoutPlugin)
        .add_plugins(Map1LayoutPlugin)

        .add_plugins(NavDebugPlugin)

        .insert_resource(Noclip(false))
        .init_resource::<InspectMode>()
        .init_resource::<Autopilot>()
        .init_resource::<AutopilotLookOnly>()
        .init_resource::<AutopilotState>()
        .init_resource::<AutopilotIntent>()
        .init_resource::<FreezeZombies>()
        .init_resource::<ShowHitboxes>()
        .init_resource::<RenderResolutionScale>()
        .init_resource::<FpsCap>()
        .init_resource::<ShowDistanceRings>()
        .init_resource::<DistanceLodEnabled>()
        .init_resource::<LodBandDistance>()
        .init_resource::<LodCarTextureLevel>()
        .init_resource::<LodCarPolyLevel>()
        .init_resource::<LodZombieTextureLevel>()
        .init_resource::<GodMode>()
        .init_resource::<ShowAutopilotTarget>()
        .init_resource::<ShowHitMarkers>()
        .init_resource::<ShowFloatingDamage>()
        .init_resource::<ShowBulletTracers>()
        .init_resource::<SettingsMenuVisible>()
        .init_resource::<SettingsMenuScrollState>()
        .init_resource::<SettingsMenuPageIndex>()
        .init_resource::<MouseUiLocked>()
        .init_resource::<TouchControlsIdleHidden>()
        .init_resource::<AimBlurEnabled>()
        .init_resource::<AimDofFocalDistanceAlt>()
        .init_resource::<MouseLookDelta>()
        .init_resource::<PerfTimings>()
        .init_resource::<ZombieAnimDisabled>()
        .init_resource::<ZombieHealthBarsHidden>()
        .init_resource::<ZombieLodDisabled>()
        .init_resource::<PredictedInputs>()
        .init_resource::<PredictionSmooth>()
        .init_resource::<PlayerModelVisible>()
        .init_resource::<CameraMode>()
        .init_resource::<PlayerModelTuning>()
        .init_resource::<GunVisible>()
        .init_resource::<CarsVisible>()
        .init_resource::<MapMsaaOn>()
        .init_resource::<ShadowQuality>()
        .init_resource::<LightQuality>()
        .init_resource::<PointLightQuality>()
        .init_resource::<TextureQuality>()
        .init_resource::<ZombieTextureQuality>()
        .init_resource::<PolyQuality>()
        .init_resource::<GunTextureQuality>()
        .init_resource::<GunPolyQuality>()
        .init_resource::<ScopeCameraEnabled>()
        .init_resource::<SelectedDebugBot>()
        .init_resource::<LocalPlayerMoving>()
.init_resource::<LocalPlayerSprinting>()
.init_resource::<LocalPlayerMoveAxis>()
        .init_resource::<TouchControlsEnabled>()
        .init_resource::<TouchMoveState>()
        .init_resource::<TouchMoveAxis>()
        .init_resource::<TouchScopeZoomState>()
        .init_resource::<TouchLookState>()
        .init_resource::<TouchFiring>()
        .init_resource::<TouchAiming>()
        .init_resource::<TouchJump>()
        .init_resource::<NoclipFlyInput>()
        .init_resource::<TouchReload>()
        .init_resource::<TouchSprintHeld>()
        .init_resource::<TouchLeanHeld>()
        .init_resource::<SensitivityScale>()
        .init_resource::<DamageMultiplierUi>()
        .init_resource::<KillCount>()
        .init_resource::<MapSelectionPending>()
        .init_resource::<RecentlyHitZombies>()
        .add_systems(Startup, (apply_persisted_settings, setup_common, force_game_over_on_start).chain())
        .add_systems(OnEnter(AppState::Menu), enter_menu)
        .add_systems(OnExit(AppState::Menu), exit_menu)
        .add_systems(OnEnter(AppState::InGame), (setup_gameplay, connect_to_server).chain());
    app.add_systems(Update, persist_settings_on_change);

    app.add_systems(
        Update,
        (
            (
                multi_touch_button_interaction,
                suppress_touch_buttons_while_menu_open,
                toggle_mouse_ui_lock,
                suppress_ui_interaction_when_locked,
                touch_move_axis,
                update_joystick_knob_visual,
                touch_look,
                touch_fire_button,
                touch_aim_button,
                touch_scope_zoom_drag,
                touch_jump_button,
                update_noclip_fly_input,
                touch_crouch_button,
                touch_reload_button,
                touch_prone_button,
                touch_sprint_button,
                touch_lean_buttons,
            )
                .chain(),
            (
                (
                    toggle_settings_menu,
                    update_settings_menu,
                    settings_menu_page_buttons,
                    update_settings_menu_pages,
                    update_mouse_ui_lock_indicator,
                    resolution_scale_buttons,
                update_resolution_scale_button_colors,
                (fps_cap_buttons, update_fps_cap_button_colors, apply_fps_cap_desktop).chain(),
                texture_quality_buttons,
                update_texture_quality_button_colors,
                poly_quality_buttons,
                update_poly_quality_button_colors,
                respawn_item_gallery_cars_on_quality_change,
                respawn_gun_mesh_parts_on_quality_change,
                    resize_scene_render_target,
                    scroll_settings_menu,
                    touch_scroll_settings_menu,
                    cheat_toggle_buttons,
                    apply_map_render_toggles,
                )
                    .chain(),
                (
                    cheat_toggle_buttons_2,
                    update_cheat_toggle_colors,
                    cheat_kill_all_button,
                    cheat_zombie_health_buttons,
                    zombie_count_buttons,
                    update_zombie_count_text,
                    sensitivity_buttons,
                    update_sensitivity_text,
                    damage_multiplier_buttons,
                    update_damage_multiplier_text,
                    auto_maintain_button,
                    update_auto_maintain_button_color,
                    update_touch_controls_idle_hidden,
                    hide_ui_when_locked,
                    gun_texture_quality_buttons,
                    update_gun_texture_quality_button_colors,
                    gun_poly_quality_buttons,
                    update_gun_poly_quality_button_colors,
                )
                    .chain(),
                (
                    cheat_toggle_buttons_3,
                    autopilot_toggle_button,
                    update_extra_toggle_colors,
                    cheat_spawn_zombie_button,
                    debug_bot_buttons,
                    shadow_quality_buttons,
                    update_shadow_quality_button_colors,
                    light_quality_buttons,
                    update_light_quality_button_colors,
                    apply_zombie_texture_quality,
                    zombie_texture_quality_buttons,
                    update_zombie_texture_quality_button_colors,
                    apply_point_light_quality,
                    point_light_quality_buttons,
                    update_point_light_quality_button_colors,
                    apply_cars_visible,
                )
                    .chain(),
            )
                .chain(),
            (
                (
                    grab_cursor,
                    toggle_noclip.run_if(debug_keys_allowed),
                    toggle_player_stance,
                    toggle_debug_flags.run_if(debug_keys_allowed),
                    autopilot_pick_target,
                    autopilot_think,
                    mouse_look,
                    shoot,
                    reload_gun,
                    apply_lean_input,
                    apply_view_recoil,
                    apply_aim_zoom,
                    apply_aim_dof,
                    send_player_input,
                    predict_player_movement,
                    sync_camera_to_player,
                    sync_player_model,
                    update_zombie_lod,
                    zombies_damage_player,
                )
                    .chain(),

                (
                    count_local_kills,
                    toggle_inspect_mode,
                    toggle_camera_mode,
                    sync_third_person_camera,
                )
                    .chain(),
            )
                .chain()
                .run_if(in_state(AppState::InGame)),
            (
                check_game_over,
                restart_game.run_if(debug_keys_allowed),
                restart_button,
                map_select_button,
                reset_config_button,
                wire_zombie_scene_animation_targets,
                wire_zombie_animations,
                wire_zombie_hitbox_bones,
                wire_gun_mesh,
                spawn_level_box_visuals,
                spawn_level_panel_visuals,
                update_zombie_animation,
                freeze_zombie_animations,

                (
                    wire_player_model_scene_nodes,
                    wire_player_model_animations,
                    update_remote_player_model_animation,
                    update_local_player_model_animation,
                    apply_player_model_pose_override,
                    update_health_ui,
                    update_inspect_ui,
                    update_compass_ui,
                    update_radar_ui,
                    update_zombie_health_bars,
                    update_floating_damage_texts,
                )
                    .chain(),
            )
                .chain(),
            (
                adjust_scope_zoom,
                update_scope_zoom_slider_visual,
                update_scope_zoom_slider_visibility,
                gun_view_animation,
                sync_scope_camera,
                update_scope_lens,
                update_ammo_ui,
                update_ping_ui,
                update_crosshair,
                set_hitbox_gizmo_depth_bias,
                draw_zombie_hitboxes,
                (update_splash_screen, distance_rings_toggle_button, update_distance_rings_button_color, draw_distance_rings).chain(),
                (
                    distance_lod_toggle_button,
                    update_distance_lod_button_color,
                    lod_stepper_buttons,
                    update_lod_stepper_texts,
                    apply_item_gallery_car_distance_lod,
                    apply_zombie_distance_lod_texture,
                )
                    .chain(),
                despawn_hit_markers,
                despawn_muzzle_flashes,
                despawn_bullet_tracers,
                update_smoke,
                update_blood_particles,
                update_spark_particles,
                despawn_blood_stains,
            )
                .chain(),
        )
            .chain(),
    );

    app.add_systems(
        PostUpdate,
        apply_player_model_travel_twist
            .after(AnimationSystems)
            .before(TransformSystems::Propagate)
            .run_if(in_state(AppState::InGame)),
    );
    }
}

const RESIZE_DEBOUNCE_FRAMES: u8 = 5;

impl Default for ZombieCountUi {
    fn default() -> Self {
        Self(10)
    }
}

impl Default for AutoMaintainUi {
    fn default() -> Self {
        Self(true)
    }
}

const TOUCH_CONTROLS_IDLE_HIDE_SECS: f32 = 10.0;

const PING_UI_REFRESH_SECS: f32 = 1.0 / 30.0;

const COMPASS_WIDTH: f32 = 280.0;
const COMPASS_HEIGHT: f32 = 26.0;

const COMPASS_PX_PER_DEG: f32 = 2.2;

const COMPASS_MARKS: [(f32, &str); 8] =
    [(0.0, "N"), (45.0, "NE"), (90.0, "E"), (135.0, "SE"), (180.0, "S"), (225.0, "SW"), (270.0, "W"), (315.0, "NW")];

const RADAR_LEFT: f32 = 16.0;
const RADAR_TOP: f32 = 56.0;
const RADAR_SIZE: f32 = 150.0;
const RADAR_RADIUS: f32 = RADAR_SIZE / 2.0;
const RADAR_RANGE: f32 = 40.0;
const RADAR_BLIP_SIZE_PX: f32 = 8.0;
const RADAR_SWEEP_DEG_PER_SEC: f32 = 60.0;
const RADAR_TRAIL_COUNT: u8 = 3;
const RADAR_TRAIL_STEP_DEG: f32 = 10.0;

const MAP_SELECT_ENTRIES: [(u8, &str, &str); 3] = [
    (1, "GAME MAP", "OFFICE BUILDING (B1-F3)"),
    (2, "ARENA", "OPEN FLAT ARENA, CHASE-AI ZOMBIES"),
    (3, "ITEM GALLERY", "ALL STANDARD ITEM SIZES ON FLAT FLOOR"),
];

#[unsafe(no_mangle)]
#[inline(never)]
#[cfg(target_os = "ios")]
pub extern "C" fn start_app() {
    main();
}

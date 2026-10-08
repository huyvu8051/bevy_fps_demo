use avian3d::prelude::*;
use bevy::animation::graph::AnimationGraph;
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
use bevy::render::view::Msaa;
use bevy::scene::SceneRoot;
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

#[derive(Resource)]
struct SensitivityScale(f32);
impl Default for SensitivityScale {
    fn default() -> Self {
        Self(1.0)
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

#[derive(Resource)]
pub(crate) struct FpsCap(pub(crate) u32);
impl Default for FpsCap {
    fn default() -> Self {
        let env_cap = std::env::var("FPS_CAP").ok().and_then(|s| s.parse::<u32>().ok()).filter(|c| *c > 0);
        Self(env_cap.unwrap_or(if cfg!(any(target_os = "ios", target_os = "android")) { 120 } else { 60 }))
    }
}

pub(crate) fn apply_fps_cap_desktop(cap: Res<FpsCap>, winit_settings: Option<ResMut<WinitSettings>>) {
    let Some(mut winit_settings) = winit_settings else { return };
    if !cap.is_changed() {
        return;
    }
    winit_settings.focused_mode = UpdateMode::reactive(std::time::Duration::from_secs_f32(1.0 / cap.0 as f32));
}

const RESOLUTION_SCALE_DEFAULT: f32 = 0.3;
const RESOLUTION_SCALE_MIN_TEXTURE_PX: u32 = 64;

fn resolution_scale_default() -> f32 {
    std::env::var("RENDER_SCALE")
        .ok()
        .and_then(|s| s.trim().parse::<f32>().ok())
        .map(|s| s.clamp(0.1, 1.0))
        .unwrap_or(RESOLUTION_SCALE_DEFAULT)
}

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

#[derive(Resource, Default)]
struct FreezeZombies(bool);

#[derive(Resource, Default)]
struct ShowHitboxes(bool);

#[derive(Resource, Default)]
struct ShowDistanceRings(bool);

#[derive(Resource)]
struct ShowAutopilotTarget(bool);

impl Default for ShowAutopilotTarget {
    fn default() -> Self {
        Self(std::env::var("SHOW_AUTOPILOT_TARGET").is_ok_and(|v| v == "1" || v.to_lowercase() == "true"))
    }
}

#[derive(Resource)]
struct ShowHitMarkers(bool);

impl Default for ShowHitMarkers {
    fn default() -> Self {
        if let Ok(val) = std::env::var("SHOW_HIT_MARKERS") {
            Self(val == "1" || val.to_lowercase() == "true")
        } else {
            Self(true)
        }
    }
}

#[derive(Resource)]
struct ShowFloatingDamage(bool);

impl Default for ShowFloatingDamage {
    fn default() -> Self {
        if let Ok(val) = std::env::var("SHOW_FLOATING_DAMAGE") {
            Self(val == "1" || val.to_lowercase() == "true")
        } else {
            Self(true)
        }
    }
}

#[derive(Resource)]
struct ShowBulletTracers(bool);

impl Default for ShowBulletTracers {
    fn default() -> Self {
        if let Ok(val) = std::env::var("SHOW_BULLET_TRACERS") {
            Self(val == "1" || val.to_lowercase() == "true")
        } else {
            Self(true)
        }
    }
}

#[derive(Resource, Default)]
struct GodMode(bool);

#[derive(Resource)]
struct RenderResolutionScale(f32);

impl Default for RenderResolutionScale {
    fn default() -> Self {
        Self(resolution_scale_default())
    }
}

#[derive(Resource)]
struct SceneRenderTarget(Handle<Image>);

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

#[derive(Resource)]
struct AimBlurEnabled(bool);

impl Default for AimBlurEnabled {
    fn default() -> Self {
        Self(true)
    }
}

#[derive(Resource, Default)]
struct AimDofFocalDistanceAlt(bool);

impl Default for GunVisible {
    fn default() -> Self {
        Self(true)
    }
}

#[derive(Resource, Default)]
struct MapMsaaOn(bool);

#[derive(Resource)]
pub(crate) struct ShadowQuality(pub(crate) u8);
impl Default for ShadowQuality {
    fn default() -> Self {
        if cfg!(any(target_os = "ios", target_os = "android")) {
            Self(1)
        } else {
            Self(if light_shadows_enabled() { 5 } else { 0 })
        }
    }
}

#[derive(Resource)]
pub(crate) struct LightQuality(pub(crate) u8);
impl Default for LightQuality {
    fn default() -> Self {
        Self(if dynamic_lights_enabled() { 3 } else { 0 })
    }
}

#[derive(Resource)]
pub(crate) struct PointLightQuality(pub(crate) u8);
impl Default for PointLightQuality {
    fn default() -> Self {
        Self(if dynamic_lights_enabled() { 3 } else { 0 })
    }
}

#[derive(Component)]
pub(crate) struct PointLightBaseIntensity(pub(crate) f32);

pub(crate) fn texture_quality_default() -> u8 {
    std::env::var("TEXTURE_QUALITY")
        .ok()
        .and_then(|s| s.trim().parse::<u8>().ok())
        .map(|q| q.clamp(0, 5))
        .unwrap_or(if cfg!(any(target_os = "ios", target_os = "android")) { 2 } else { 5 })
}

#[derive(Resource)]
pub(crate) struct TextureQuality(pub(crate) u8);
impl Default for TextureQuality {
    fn default() -> Self {
        Self(texture_quality_default())
    }
}

pub(crate) fn zombie_texture_quality_default() -> u8 {
    std::env::var("ZOMBIE_TEXTURE_QUALITY")
        .ok()
        .and_then(|s| s.trim().parse::<u8>().ok())
        .map(|q| q.clamp(0, 5))
        .unwrap_or(if cfg!(any(target_os = "ios", target_os = "android")) { 2 } else { 5 })
}

#[derive(Resource)]
pub(crate) struct ZombieTextureQuality(pub(crate) u8);
impl Default for ZombieTextureQuality {
    fn default() -> Self {
        Self(zombie_texture_quality_default())
    }
}

pub(crate) fn poly_quality_default() -> u8 {
    std::env::var("POLY_QUALITY")
        .ok()
        .and_then(|s| s.trim().parse::<u8>().ok())
        .map(|q| q.clamp(1, 5))
        .unwrap_or(if cfg!(any(target_os = "ios", target_os = "android")) { 2 } else { 5 })
}

#[derive(Resource)]
pub(crate) struct PolyQuality(pub(crate) u8);
impl Default for PolyQuality {
    fn default() -> Self {
        Self(poly_quality_default())
    }
}

#[derive(Resource, Default)]
pub(crate) struct DistanceLodEnabled(pub(crate) bool);

#[derive(Resource)]
pub(crate) struct LodBandDistance(pub(crate) [f32; 3]);
impl Default for LodBandDistance {
    fn default() -> Self {
        Self([ZOMBIE_LOD_DISTANCE, 30.0, 50.0])
    }
}
impl LodBandDistance {

    pub(crate) fn band_of(&self, distance: f32) -> usize {
        if distance <= self.0[0] {
            0
        } else if distance <= self.0[1] {
            1
        } else {
            2
        }
    }
}
pub(crate) const LOD_DISTANCE_STEP: f32 = 1.0;
pub(crate) const LOD_DISTANCE_MIN: f32 = 1.0;
pub(crate) const LOD_DISTANCE_MAX: f32 = 200.0;

#[derive(Resource)]
pub(crate) struct LodCarTextureLevel(pub(crate) [u8; 3]);
impl Default for LodCarTextureLevel {
    fn default() -> Self {
        Self([5, 3, 1])
    }
}

#[derive(Resource)]
pub(crate) struct LodCarPolyLevel(pub(crate) [u8; 3]);
impl Default for LodCarPolyLevel {
    fn default() -> Self {
        Self([5, 3, 1])
    }
}

#[derive(Resource)]
pub(crate) struct LodZombieTextureLevel(pub(crate) [u8; 3]);
impl Default for LodZombieTextureLevel {
    fn default() -> Self {
        Self([5, 3, 1])
    }
}

pub(crate) fn gun_poly_quality_default() -> u8 {
    std::env::var("GUN_POLY_QUALITY")
        .ok()
        .and_then(|s| s.trim().parse::<u8>().ok())
        .map(|q| q.clamp(1, 5))
        .unwrap_or(if cfg!(any(target_os = "ios", target_os = "android")) { 2 } else { 5 })
}

#[derive(Resource)]
pub(crate) struct GunPolyQuality(pub(crate) u8);
impl Default for GunPolyQuality {
    fn default() -> Self {
        Self(gun_poly_quality_default())
    }
}

pub(crate) fn gun_texture_quality_default() -> u8 {
    std::env::var("GUN_TEXTURE_QUALITY")
        .ok()
        .and_then(|s| s.trim().parse::<u8>().ok())
        .map(|q| q.clamp(0, 5))
        .unwrap_or(if cfg!(any(target_os = "ios", target_os = "android")) { 2 } else { 5 })
}

#[derive(Resource)]
pub(crate) struct GunTextureQuality(pub(crate) u8);
impl Default for GunTextureQuality {
    fn default() -> Self {
        Self(gun_texture_quality_default())
    }
}

pub(crate) fn gun_mesh_path(poly_quality: u8) -> &'static str {
    match poly_quality {
        1 => "models/ak47_scope/mesh/ak47_scope_light_p1.glb",
        2 => "models/ak47_scope/mesh/ak47_scope_light_p2.glb",
        3 => "models/ak47_scope/mesh/ak47_scope_light_p3.glb",
        4 => "models/ak47_scope/mesh/ak47_scope_light_p4.glb",
        _ => "models/ak47_scope/mesh/ak47_scope_light.glb",
    }
}

pub(crate) fn gun_texture_paths(mat_index: usize, texture_quality: u8) -> (String, String, String) {
    let q = texture_quality.min(5);
    let base_ext = if mat_index == 1 { "png" } else { "jpg" };
    (
        format!("models/ak47_scope/mesh/textures/mat{mat_index}_base_color_q{q}.{base_ext}"),
        format!("models/ak47_scope/mesh/textures/mat{mat_index}_normal_q{q}.jpg"),
        format!("models/ak47_scope/mesh/textures/mat{mat_index}_metallic_roughness_q{q}.jpg"),
    )
}

fn spawn_gun_mesh_parts(
    commands: &mut Commands,
    asset_server: &AssetServer,
    materials: &mut Assets<StandardMaterial>,
    gun_entity: Entity,
    poly_quality: u8,
    texture_quality: u8,
) {
    let mesh_path = gun_mesh_path(poly_quality);
    let linear = |path: String| {
        asset_server.load_with_settings(path, |settings: &mut bevy::image::ImageLoaderSettings| {
            settings.is_srgb = false;
        })
    };
    for mat_index in 0..2usize {
        let (base_color_path, normal_path, metallic_roughness_path) = gun_texture_paths(mat_index, texture_quality);
        let metallic_roughness_handle = linear(metallic_roughness_path);
        let material = materials.add(StandardMaterial {
            base_color_texture: Some(asset_server.load(base_color_path)),
            normal_map_texture: Some(linear(normal_path)),
            metallic_roughness_texture: Some(metallic_roughness_handle.clone()),
            occlusion_texture: Some(metallic_roughness_handle),
            cull_mode: None,
            alpha_mode: if mat_index == 1 { AlphaMode::Blend } else { AlphaMode::Opaque },
            ..default()
        });
        commands.spawn((
            Name::new(if mat_index == 0 { "Gun body" } else { "Gun scope lens" }),
            Mesh3d(asset_server.load(GltfAssetLabel::Primitive { mesh: mat_index, primitive: 0 }.from_asset(mesh_path))),
            MeshMaterial3d(material),
            Transform::IDENTITY,
            GunMeshPart,
            ChildOf(gun_entity),
        ));
    }
}

fn respawn_gun_mesh_parts_on_quality_change(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    poly_quality: Res<GunPolyQuality>,
    texture_quality: Res<GunTextureQuality>,
    gun: Query<Entity, With<Gun>>,
    existing_parts: Query<Entity, With<GunMeshPart>>,
) {
    let poly_changed = poly_quality.is_changed() && !poly_quality.is_added();
    let texture_changed = texture_quality.is_changed() && !texture_quality.is_added();
    if !poly_changed && !texture_changed {
        return;
    }
    let Ok(gun_entity) = gun.single() else { return };
    for entity in &existing_parts {
        commands.entity(entity).despawn();
    }
    spawn_gun_mesh_parts(&mut commands, &asset_server, &mut materials, gun_entity, poly_quality.0, texture_quality.0);
}

pub(crate) fn dynamic_lights_enabled() -> bool {
    cfg!(any(target_os = "ios", target_os = "android")) || std::env::var("LIGHTS").map(|v| v != "0").unwrap_or(true)
}
pub(crate) fn light_shadows_enabled() -> bool {
    cfg!(any(target_os = "ios", target_os = "android")) || std::env::var("SHADOWS").map(|v| v != "0").unwrap_or(true)
}

fn ambient_brightness() -> f32 {
    if dynamic_lights_enabled() { 250.0 } else { 1800.0 }
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

#[derive(Resource)]
struct GunshotSound(Handle<AudioSource>);

#[derive(Resource)]
struct ReloadSound(Handle<AudioSource>);

#[derive(SystemParam)]
struct VfxAssets<'w, 's> {
    hit_assets: Res<'w, HitMarkerAssets>,
    muzzle_fx: Res<'w, MuzzleFxAssets>,
    tracer: Res<'w, TracerAssets>,
    blood: Res<'w, BloodAssets>,
    sparks: Res<'w, SparkAssets>,
    gunshot: Res<'w, GunshotSound>,
    reload_sound: Res<'w, ReloadSound>,

    stance: Query<'w, 's, &'static PlayerStance, With<Player>>,

    recently_hit: ResMut<'w, RecentlyHitZombies>,
}

fn log_window_resolution(windows: Query<&Window, With<PrimaryWindow>>) {
    let Ok(window) = windows.single() else {
        return;
    };
    info!(
        "Window: {}x{} physical px, {:.1}x{:.1} logical px, scale factor {:.2}",
        window.resolution.physical_width(),
        window.resolution.physical_height(),
        window.resolution.width(),
        window.resolution.height(),
        window.resolution.scale_factor(),
    );
}

fn setup_common(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut images: ResMut<Assets<Image>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    player_model_tuning: Res<PlayerModelTuning>,
    render_resolution_scale: Res<RenderResolutionScale>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    let (window_width, window_height) = windows
        .single()
        .map(|window| (window.resolution.physical_width(), window.resolution.physical_height()))
        .unwrap_or((1280, 720));
    let mut scene_image = Image::new_fill(
        scaled_scene_extent(window_width, window_height, render_resolution_scale.0),
        TextureDimension::D2,
        &[0, 0, 0, 255],
        TextureFormat::Bgra8UnormSrgb,
        RenderAssetUsages::default(),
    );
    scene_image.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
    let scene_image_handle = images.add(scene_image);
    commands.insert_resource(SceneRenderTarget(scene_image_handle.clone()));

    commands.spawn((Camera2d, IsDefaultUiCamera));
    spawn_splash_screen(&mut commands, &asset_server);

    commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            position_type: PositionType::Absolute,
            ..default()
        },
        ImageNode::new(scene_image_handle),
        Pickable::IGNORE,
    ));

    commands.insert_resource(HitMarkerAssets {
        mesh: meshes.add(Sphere::new(0.03)),
        material: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.15, 0.1),
            emissive: LinearRgba::rgb(4.0, 0.3, 0.2),
            unlit: true,
            ..default()
        }),
    });

    commands.insert_resource(GunshotSound(asset_server.load("audio/gunshot.wav")));
    commands.insert_resource(ReloadSound(asset_server.load("audio/reload.wav")));

    commands.insert_resource(MuzzleFxAssets {
        flash_mesh: meshes.add(Rectangle::new(0.18, 0.18)),
        flash_material: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.85, 0.5),
            base_color_texture: Some(asset_server.load("textures/muzzle.png")),
            emissive: LinearRgba::rgb(6.0, 3.5, 1.0),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            cull_mode: None,
            ..default()
        }),
        smoke_mesh: meshes.add(Rectangle::new(0.25, 0.25)),
        smoke_material: materials.add(StandardMaterial {
            base_color: Color::srgba(0.75, 0.75, 0.75, 0.5),
            base_color_texture: Some(asset_server.load("textures/smoke.png")),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            cull_mode: None,
            ..default()
        }),
    });

    commands.insert_resource(TracerAssets {
        mesh: meshes.add(Cylinder::new(1.0, 1.0)),
        material: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.85, 0.3),
            emissive: LinearRgba::rgb(10.0, 6.0, 1.0),
            unlit: true,
            ..default()
        }),
    });

    commands.insert_resource(BloodAssets {
        particle_mesh: meshes.add(Rectangle::new(BLOOD_PARTICLE_SIZE, BLOOD_PARTICLE_SIZE)),
        particle_material: materials.add(StandardMaterial {
            base_color: Color::srgb(0.45, 0.02, 0.02),
            unlit: true,
            cull_mode: None,
            ..default()
        }),
        stain_mesh: meshes.add(Rectangle::new(BLOOD_STAIN_SIZE, BLOOD_STAIN_SIZE)),
        stain_material: materials.add(StandardMaterial {
            base_color: Color::srgba(0.35, 0.01, 0.01, 0.85),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            cull_mode: None,
            ..default()
        }),
    });

    commands.insert_resource(SparkAssets {
        mesh: meshes.add(Rectangle::new(SPARK_PARTICLE_SIZE, SPARK_PARTICLE_SIZE)),
        material: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.75, 0.2),
            emissive: LinearRgba::rgb(25.0, 12.0, 1.5),
            unlit: true,
            cull_mode: None,
            ..default()
        }),
    });

    commands.insert_resource(build_zombie_anim(&asset_server, &mut graphs));
    commands.insert_resource(build_zombie_kind_anims(&asset_server, &mut graphs));
    commands.insert_resource(build_player_model_anim(&asset_server, &mut graphs, &player_model_tuning));
    commands.insert_resource(ScopeZoom(SCOPE_DEFAULT_ZOOM));
    commands.insert_resource(GameOver(false));
    if network_setup::map_id_override().is_some() {
        next_state.set(AppState::InGame);
    } else {
        next_state.set(AppState::Menu);
    }
}

fn review_yaw_pitch() -> (f32, f32) {
    if current_scenario() != Scenario::ItemGallery {
        return (0.0, 0.0);
    }
    let deg = |var: &str| std::env::var(var).ok().and_then(|s| s.trim().parse::<f32>().ok()).unwrap_or(0.0);
    (deg("REVIEW_YAW").to_radians(), deg("REVIEW_PITCH").to_radians())
}

fn setup_gameplay(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    touch_enabled: Res<TouchControlsEnabled>,
    player_model_tuning: Res<PlayerModelTuning>,
    texture_quality: Res<TextureQuality>,
    poly_quality: Res<PolyQuality>,
    gun_texture_quality: Res<GunTextureQuality>,
    gun_poly_quality: Res<GunPolyQuality>,
    scene_target: Option<Res<SceneRenderTarget>>,
    mut pending: ResMut<MapSelectionPending>,
) {
    let Some(scene_target) = scene_target else {
        return;
    };
    pending.0 = false;
    let scenario = current_scenario();
    let player_spawn = scenario_player_spawn(scenario);

    match scenario.map_kind() {
        MapKind::ItemGallery => {
            let floor_color = [0.5, 0.5, 0.52];
            let panels = vec![shared::floor_panel(
                Vec3::ZERO,
                shared::ITEM_GALLERY_HALF_SIZE_X * 2.0,
                shared::ITEM_GALLERY_HALF_SIZE_Z * 2.0,
                shared::ARENA_WALL_THICKNESS,
                floor_color,
                0.9,
            )];
            spawn_level_pieces(&mut commands, Vec::new(), panels, false);
        }
        MapKind::Campaign(map_id) => {
            let (pieces, panels) = shared::campaign_level_pieces(map_id);
            spawn_level_pieces(&mut commands, pieces, panels, true);
            if map_id == 1 && dynamic_lights_enabled() {
                spawn_map1_lights(&mut commands);
            }
        }
        MapKind::Arena => {
            spawn_test_arena(&mut commands, &mut meshes, &mut materials, scenario.map_id());
            #[cfg(not(target_arch = "wasm32"))]
            spawn_item_gallery_test_props(&mut commands, &asset_server, &mut materials, poly_quality.0, texture_quality.0);
        }
    }

    commands.spawn((
        DirectionalLight {
            illuminance: if dynamic_lights_enabled() { MAIN_SUN_ILLUMINANCE } else { 0.0 },
            shadows_enabled: light_shadows_enabled(),
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.9, 0.5, 0.0)),
        MainSun,
    ));

    commands.spawn((
        Name::new("Player"),
        Transform::from_translation(player_spawn),
        Player {
            vertical_velocity: 0.0,
            grounded: false,
        },
        Health(PLAYER_MAX_HEALTH),
        PlayerStance::default(),
    ));

    commands.spawn((
        Name::new("PlayerModel (local, debug — KeyT/CameraMode)"),
        SceneRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(player_model_tuning.mesh.clone()))),
        Transform::from_translation(player_spawn).with_scale(Vec3::splat(player_model_tuning.scale)),
        Visibility::Hidden,
        PlayerModel,
        PlayerModelAnimRoot,
        PlayerModelSkeletonBones::default(),
    ));

    let (review_yaw, review_pitch) = review_yaw_pitch();
    let camera = commands
        .spawn((
            Name::new("FpsCamera (first-person, main render camera)"),
            Camera3d::default(),
            Projection::Perspective(PerspectiveProjection {
                fov: BASE_FOV,
                ..default()
            }),
            Transform::from_translation(player_spawn + Vec3::Y * EYE_HEIGHT_OFFSET),
            FpsCamera { yaw: review_yaw, pitch: review_pitch },
            ViewRecoil::default(),
            Lean::default(),
            AmbientLight {
                color: Color::WHITE,
                brightness: ambient_brightness(),
                ..default()
            },
            Bloom::default(),
            Msaa::Off,
            RenderLayers::from_layers(&[WORLD_LAYER, VIEWMODEL_LAYER]),
            RenderTarget::from(scene_target.0.clone()),
        ))
        .id();

    let gun_entity = commands
        .spawn((
            Name::new("Gun (first-person viewmodel)"),
            Transform::from_translation(GUN_OFFSET)
                .with_rotation(Quat::from_rotation_y(GUN_ROTATION_Y_DEG.to_radians()))
                .with_scale(Vec3::splat(GUN_SCALE)),
            Visibility::default(),
            Gun {
                base_offset: GUN_OFFSET,
                recoil: 0.0,
                ammo: MAGAZINE_SIZE,
                reload_remaining: 0.0,
                ads_blend: 0.0,
                wall_raise: 0.0,
                shoulder_blend: 0.0,
                recoil_roll: 0.0,
                bob_phase: 0.0,
                bob_blend: 0.0,
                look_sway_roll: 0.0,
            },
            ChildOf(camera),
        ))
        .id();
    spawn_gun_mesh_parts(&mut commands, &asset_server, &mut materials, gun_entity, gun_poly_quality.0, gun_texture_quality.0);

    commands.spawn((
        Name::new("ThirdPersonCamera (KeyU, inactive until CameraMode::ThirdPerson)"),
        Camera3d::default(),
        Camera {
            is_active: false,
            ..default()
        },
        Projection::Perspective(PerspectiveProjection {
            fov: BASE_FOV,
            ..default()
        }),
        Transform::from_translation(player_spawn + Vec3::Y * EYE_HEIGHT_OFFSET),
        ThirdPersonCamera,
        RenderLayers::from_layers(&[WORLD_LAYER]),
        RenderTarget::from(scene_target.0.clone()),
    ));

    let scope_size = Extent3d {
        width: SCOPE_TEXTURE_SIZE,
        height: SCOPE_TEXTURE_SIZE,
        ..default()
    };
    let mut scope_image = Image::new_fill(
        scope_size,
        TextureDimension::D2,
        &[0, 0, 0, 255],
        TextureFormat::Bgra8UnormSrgb,
        RenderAssetUsages::default(),
    );
    scope_image.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
    let scope_image_handle = images.add(scope_image);

    let scope_camera = commands
        .spawn((
            Name::new("ScopeCamera (renders the ADS lens texture)"),
            Camera3d::default(),
            Camera {
                order: -1,
                is_active: false,
                ..default()
            },
            RenderTarget::from(scope_image_handle.clone()),
            Projection::Perspective(PerspectiveProjection {
                fov: SCOPE_BASE_FOV / SCOPE_DEFAULT_ZOOM,
                ..default()
            }),
            Transform::from_translation(player_spawn + Vec3::Y * EYE_HEIGHT_OFFSET),
            AmbientLight {
                color: Color::WHITE,
                brightness: ambient_brightness(),
                ..default()
            },
            Msaa::Off,
            RenderLayers::from_layers(&[WORLD_LAYER, SCOPE_RETICLE_LAYER]),
            Bloom::default(),
            ScopeCamera,
        ))
        .id();

    commands.spawn((
        Mesh3d(meshes.add(
            Triangle2d::new(
                Vec2::new(0.0, 0.0),
                Vec2::new(-SCOPE_RETICLE_WIDTH / 2.0, -SCOPE_RETICLE_HEIGHT / 2.0),
                Vec2::new(SCOPE_RETICLE_WIDTH / 2.0, -SCOPE_RETICLE_HEIGHT / 2.0),
            )
            .mesh(),
        )),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.0, 0.0),
            emissive: LinearRgba::rgb(35.0, 0.0, 0.0),
            unlit: true,
            ..default()
        })),
        Transform::from_translation(Vec3::new(0.0, 0.0, -SCOPE_RETICLE_DISTANCE)),
        RenderLayers::layer(SCOPE_RETICLE_LAYER),
        ScopeReticle,
        ChildOf(scope_camera),
    ));

    commands.spawn((
        Mesh3d(meshes.add(Circle::new(SCOPE_LENS_SIZE / 2.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color_texture: Some(scope_image_handle),
            unlit: true,
            ..default()
        })),
        Transform::from_translation(SCOPE_LENS_OFFSET),
        Visibility::Hidden,
        RenderLayers::layer(VIEWMODEL_LAYER),
        ScopeLens,
        ChildOf(camera),
    ));

    spawn_crosshair(&mut commands);
    spawn_health_ui(&mut commands);
    spawn_ammo_ui(&mut commands);
    spawn_ping_ui(&mut commands);
    spawn_compass_ui(&mut commands);
    spawn_inspect_ui(&mut commands);
    spawn_settings_menu(&mut commands);
    spawn_settings_toggle_button(&mut commands);
    spawn_mouse_ui_lock_indicator(&mut commands);
    if touch_enabled.0 {
        spawn_touch_controls(&mut commands);
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

#[derive(Resource, Default)]
struct KillCount(u32);

const RECENTLY_HIT_ZOMBIE_TTL: f32 = 2.0;

#[derive(Resource, Default)]
struct RecentlyHitZombies(std::collections::HashMap<Entity, f32>);

const TOUCH_CONTROLS_IDLE_HIDE_SECS: f32 = 10.0;

#[derive(Component)]
struct ArenaGeometry;

#[derive(Component)]
pub(crate) struct ArenaBoxDims {
    pub(crate) shape: shared::ArenaShape,
    pub(crate) size: Vec3,
    pub(crate) index: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum LevelPieceKind {
    Box,
    Panel,
}

#[derive(Component)]
pub(crate) struct LevelPieceIndex {
    pub(crate) kind: LevelPieceKind,
    pub(crate) index: usize,
}

fn spawn_test_arena(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    map_id: u8,
) {
    let floor_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.35, 0.35, 0.38),
        perceptual_roughness: 0.9,
        ..default()
    });
    let wall_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.55, 0.5, 0.45),
        perceptual_roughness: 0.85,
        ..default()
    });

    for (i, b) in shared::arena_layout(map_id).iter().enumerate() {
        let material = if i == 0 { floor_material.clone() } else { wall_material.clone() };
        let (mesh, collider): (Handle<Mesh>, Collider) = match b.shape {
            shared::ArenaShape::Cuboid => (
                meshes.add(Cuboid::new(b.size.x, b.size.y, b.size.z)),
                Collider::cuboid(b.size.x, b.size.y, b.size.z),
            ),

            shared::ArenaShape::Cylinder => (
                meshes.add(Cylinder::new(b.size.x * 0.5, b.size.y)),
                Collider::cylinder(b.size.x * 0.5, b.size.y),
            ),
        };
        let mut piece = commands.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::from_translation(b.translation).with_rotation(b.rotation),
            RigidBody::Static,
            collider,
            ArenaGeometry,
            ArenaBoxDims { shape: b.shape, size: b.size, index: i },
        ));

        if b.shape == shared::ArenaShape::Cylinder {
            piece.insert(ClientNavObstacle);
        }
    }

    if dynamic_lights_enabled() {
        commands.spawn((
            PointLight {
                color: Color::srgb(1.0, 0.98, 0.92),
                intensity: 4_000_000.0,
                range: ARENA_HALF_SIZE * 3.0,
                shadows_enabled: light_shadows_enabled(),
                ..default()
            },
            PointLightBaseIntensity(4_000_000.0),
            Transform::from_xyz(0.0, ARENA_WALL_HEIGHT - 0.2, 0.0),
            ArenaGeometry,
        ));
    }
}

fn spawn_map1_lights(commands: &mut Commands) {

    for level in [shared::MAP1_B1, shared::MAP1_G, shared::MAP1_F2, shared::MAP1_F3] {
        commands.spawn((
            PointLight {
                color: Color::srgb(0.9, 0.9, 0.88),
                intensity: 4_000_000.0,
                range: (level.half_size_x + level.half_size_z) * 1.2,
                shadows_enabled: light_shadows_enabled(),
                ..default()
            },
            PointLightBaseIntensity(4_000_000.0),
            Transform::from_xyz(0.0, level.elevation + level.wall_height - 0.3, level.center_z()),
        ));
    }
}

fn spawn_level_box_visuals(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    boxes: Query<(Entity, &shared::LevelBox, &shared::LevelTransform), Added<shared::LevelBox>>,
) {
    for (entity, level_box, level_transform) in &boxes {
        let material = materials.add(StandardMaterial {
            base_color: Color::srgb(level_box.color[0], level_box.color[1], level_box.color[2]),
            perceptual_roughness: level_box.roughness,
            ..default()
        });
        commands.entity(entity).insert((
            Mesh3d(meshes.add(Cuboid::new(level_box.size.x, level_box.size.y, level_box.size.z))),
            MeshMaterial3d(material),
            Transform::from_translation(level_transform.translation).with_rotation(level_transform.rotation),
            RigidBody::Static,
            Collider::cuboid(level_box.size.x, level_box.size.y, level_box.size.z),
        ));
    }
}

fn spawn_level_panel_visuals(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    panels: Query<(Entity, &shared::LevelPanel, &shared::LevelTransform), Added<shared::LevelPanel>>,
) {
    for (entity, panel, level_transform) in &panels {
        let material = materials.add(StandardMaterial {
            base_color: Color::srgb(panel.color[0], panel.color[1], panel.color[2]),
            perceptual_roughness: panel.roughness,
            cull_mode: None,
            ..default()
        });
        commands.entity(entity).insert((
            Mesh3d(meshes.add(Rectangle::new(panel.width, panel.height))),
            MeshMaterial3d(material),
            Transform::from_translation(level_transform.translation).with_rotation(level_transform.rotation),
        ));
        if let Some(thickness) = panel.collider_thickness {
            let offset = if panel.collider_offset { -thickness / 2.0 } else { 0.0 };
            commands.entity(entity).insert(RigidBody::Static).with_children(|children| {
                children.spawn((Transform::from_xyz(0.0, 0.0, offset), Collider::cuboid(panel.width, panel.height, thickness)));
            });
        }
    }
}

fn spawn_level_pieces(commands: &mut Commands, pieces: Vec<shared::BoxPiece>, panels: Vec<shared::PanelPiece>, is_map1: bool) {
    for (index, (translation, rotation, size, color, roughness)) in pieces.into_iter().enumerate() {
        let mut piece = commands.spawn((shared::LevelBox::new(size, color, roughness), shared::LevelTransform::new(translation, rotation)));
        if is_map1 {

            piece.insert((ClientNavObstacle, LevelPieceIndex { kind: LevelPieceKind::Box, index }));
        }
    }
    for (index, (translation, rotation, width, height, color, roughness, collider_thickness, collider_offset)) in panels.into_iter().enumerate() {
        let panel = match collider_thickness {
            Some(thickness) if collider_offset => shared::LevelPanel::solid_surface(width, height, color, roughness, thickness),
            Some(thickness) => shared::LevelPanel::solid(width, height, color, roughness, thickness),
            None => shared::LevelPanel::new(width, height, color, roughness),
        };
        let mut piece = commands.spawn((panel, shared::LevelTransform::new(translation, rotation)));
        if is_map1 {
            piece.insert(LevelPieceIndex { kind: LevelPieceKind::Panel, index });
        }
    }
}

const PING_UI_REFRESH_SECS: f32 = 1.0 / 30.0;

const COMPASS_WIDTH: f32 = 280.0;
const COMPASS_HEIGHT: f32 = 26.0;

const COMPASS_PX_PER_DEG: f32 = 2.2;

const COMPASS_MARKS: [(f32, &str); 8] =
    [(0.0, "N"), (45.0, "NE"), (90.0, "E"), (135.0, "SE"), (180.0, "S"), (225.0, "SW"), (270.0, "W"), (315.0, "NW")];

fn count_local_kills(
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

const MAP_SELECT_ENTRIES: [(u8, &str, &str); 3] = [
    (1, "GAME MAP", "OFFICE BUILDING (B1-F3)"),
    (2, "ARENA", "OPEN FLAT ARENA, CHASE-AI ZOMBIES"),
    (3, "ITEM GALLERY", "ALL STANDARD ITEM SIZES ON FLAT FLOOR"),
];

fn update_noclip_fly_input(
    mut fly: ResMut<NoclipFlyInput>,
    jump_button: Query<&Interaction, With<TouchJumpButton>>,
    crouch_button: Query<&Interaction, With<TouchCrouchButton>>,
) {
    fly.up = jump_button.iter().any(|interaction| *interaction == Interaction::Pressed);
    fly.down = crouch_button.iter().any(|interaction| *interaction == Interaction::Pressed);
}

fn draw_bone_capsule(gizmos: &mut Gizmos, a: Vec3, b: Vec3, radius: f32, color: Color) {
    let segment = b - a;
    let Ok(direction) = Dir3::new(segment) else {

        return;
    };
    gizmos.primitive_3d(
        &Capsule3d { radius, half_length: segment.length() / 2.0 },

        Isometry3d::new(a.midpoint(b), Quat::from_rotation_arc(Vec3::Y, *direction)),
        color,
    );
}

#[derive(Resource, Default)]
struct PredictedInputs(std::collections::VecDeque<(u32, ClientInput, f32, f32, bool)>);

const INPUT_SEND_HZ: f32 = 32.0;

const RECONCILE_POS_THRESHOLD: f32 = 0.15;
const RECONCILE_SMOOTH_SECS: f32 = 0.12;

#[derive(Resource, Default)]
struct PredictionSmooth {
    offset: Vec3,
}

#[derive(Default)]
struct InputSendThrottleState {
    elapsed: f32,

    jump_edge_pending: bool,
}

#[derive(serde::Deserialize, Clone)]
pub(crate) struct CharacterAnimIndices {
    pub(crate) idle: usize,
    pub(crate) walk: usize,
    pub(crate) run: usize,
    pub(crate) shoot: usize,
    pub(crate) death: usize,

    #[serde(default)]
    pub(crate) run_back: usize,
}

#[derive(serde::Deserialize, Clone)]
pub(crate) struct CharacterRon {
    pub(crate) id: String,
    pub(crate) display_name: String,
    pub(crate) lore_ref: String,
    pub(crate) mesh: String,

    #[serde(default)]
    pub(crate) anim_source: String,
    pub(crate) scale: f32,
    pub(crate) y_offset: f32,
    pub(crate) facing_deg: f32,
    pub(crate) animations: CharacterAnimIndices,
    pub(crate) audio: Vec<String>,
}

pub(crate) const AVAILABLE_CHARACTER_DIRS: &[&str] = &["characters/soldier_meshy"];

fn load_character_ron_by_dir(dir: &str) -> CharacterRon {
    #[cfg(target_arch = "wasm32")]
    let text: String = match dir {
        "characters/soldier_meshy" => {
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/characters/soldier_meshy/character.ron")).to_string()
        }
        other => panic!("load_character_ron_by_dir: unknown character dir {other:?} — add an include_str! arm here to match a new entry in AVAILABLE_CHARACTER_DIRS"),
    };

    #[cfg(target_os = "ios")]
    let text: String = {
        let resource_path = crate::cadisplaylink::native_metal_renderer::bundle_resource_path()
            .unwrap_or_else(|| panic!("failed to read {dir}/character.ron: NSBundle.main.resourcePath was nil"));
        let path = std::path::Path::new(&resource_path).join("assets").join(dir).join("character.ron");
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read {dir}/character.ron at {path:?}: {e}"))
    };
    #[cfg(not(any(target_arch = "wasm32", target_os = "ios")))]
    let text: String = std::fs::read_to_string(format!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/{}/character.ron"), dir))
        .unwrap_or_else(|e| panic!("failed to read {dir}/character.ron: {e}"));
    ron::de::from_str(&text).unwrap_or_else(|e| panic!("failed to parse {dir}/character.ron: {e}"))
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn character_dir_from_mesh_path(mesh_path: &str) -> String {
    mesh_path.rsplit_once("/mesh/").map(|(dir, _)| dir.to_string()).unwrap_or_default()
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn format_character_ron(c: &CharacterRon) -> String {
    format!(
        "// Metadata for the \"{id}\" character — see docs/CHARACTER_IMPORT_CHECKLIST.md's\n\
         // \"Scale and origin\" section for why this can't just trust the mesh's native\n\
         // scale/pivot.\n\
         //\n\
         // scale/y_offset/facing_deg are F9-tunable (EDITOR=1): third-person (KeyU),\n\
         // Comma/Period nudges scale, Minus/Equal nudges y_offset — see\n\
         // nudge_player_model_tuning/save_player_model_tuning\n\
         // (crates/client/src/movement_combat.rs). `animations` isn't nudge-able,\n\
         // only hand-edited.\n\
         (\n\
         \u{20}   id: \"{id}\",\n\
         \u{20}   display_name: \"{display_name}\",\n\
         \u{20}   lore_ref: \"{lore_ref}\",\n\
         \n\
         \u{20}   mesh: \"{mesh}\",\n\
         \u{20}   // See docs/CHARACTER_IMPORT_CHECKLIST.md before replacing this mesh.\n\
         \u{20}   anim_source: \"{anim_source}\",\n\
         \u{20}   scale: {scale},\n\
         \u{20}   y_offset: {y_offset},\n\
         \u{20}   facing_deg: {facing_deg},\n\
         \n\
         \u{20}   // Indices into anim_source, not mesh.\n\
         \u{20}   animations: (\n\
         \u{20}       idle: {idle},\n\
         \u{20}       walk: {walk},\n\
         \u{20}       run: {run},\n\
         \u{20}       run_back: {run_back},\n\
         \u{20}       shoot: {shoot},\n\
         \u{20}       death: {death},\n\
         \u{20}   ),\n\
         \n\
         \u{20}   audio: [{audio}],\n\
         )\n",
        id = c.id,
        display_name = c.display_name,
        lore_ref = c.lore_ref,
        mesh = c.mesh,
        anim_source = c.anim_source,
        scale = ron_f32(c.scale),
        y_offset = ron_f32(c.y_offset),
        facing_deg = ron_f32(c.facing_deg),
        idle = c.animations.idle,
        walk = c.animations.walk,
        run = c.animations.run,
        run_back = c.animations.run_back,
        shoot = c.animations.shoot,
        death = c.animations.death,
        audio = c.audio.iter().map(|a| format!("\"{a}\"")).collect::<Vec<_>>().join(", "),
    )
}

#[derive(Resource, Clone)]
pub(crate) struct PlayerModelTuning {
    pub(crate) scale: f32,
    pub(crate) y_offset: f32,
    pub(crate) facing_deg: f32,
    pub(crate) animations: CharacterAnimIndices,
    pub(crate) id: String,
    pub(crate) display_name: String,
    pub(crate) lore_ref: String,
    pub(crate) mesh: String,
    pub(crate) anim_source: String,
    pub(crate) audio: Vec<String>,
}

impl PlayerModelTuning {

    pub(crate) fn load(dir: &str) -> Self {
        let c = load_character_ron_by_dir(dir);
        Self {
            scale: c.scale,
            y_offset: c.y_offset,
            facing_deg: c.facing_deg,
            animations: c.animations,
            id: c.id,
            display_name: c.display_name,
            lore_ref: c.lore_ref,
            mesh: c.mesh,
            anim_source: c.anim_source,
            audio: c.audio,
        }
    }

    pub(crate) fn anim_source_or_mesh(&self) -> &str {
        if self.anim_source.is_empty() {
            &self.mesh
        } else {
            &self.anim_source
        }
    }
}

impl Default for PlayerModelTuning {
    fn default() -> Self {
        Self::load(AVAILABLE_CHARACTER_DIRS[0])
    }
}

fn on_shot_confirmed(
    trigger: On<ShotConfirmed>,
    mut commands: Commands,
    time: Res<Time>,
    vfx_assets: VfxAssets,
    spatial_query: SpatialQuery,
    zombies: Query<(Entity, &Transform, &ZombieHitboxBones), With<Zombie>>,
    globals: Query<&GlobalTransform>,
    show_floating_damage: Res<ShowFloatingDamage>,
) {
    let ShotConfirmed { point, normal, damage, killed } = *trigger.event();
    if damage > 0.0 {
        info!("[shot] server confirmed: inner bone capsule hit, damage={damage:.1} killed={killed}");
    } else {
        info!("[shot] server confirmed: outer capsule hit but missed every inner bone capsule (0 damage)");
    }

    let nearest_bone = if killed {
        None
    } else {
        zombies
            .iter()
            .min_by(|(_, a, _), (_, b, _)| a.translation.distance_squared(point).total_cmp(&b.translation.distance_squared(point)))
            .and_then(|(_, _, bones)| nearest_zombie_bone(bones, point, &globals))
    };
    let bone_for_blood = nearest_bone.as_ref().map(|(entity, global)| (*entity, global));
    spawn_zombie_hit_feedback(&mut commands, &vfx_assets.blood, &vfx_assets.sparks, &spatial_query, &time, point, normal, damage, bone_for_blood, killed, show_floating_damage.0);
}

#[unsafe(no_mangle)]
#[inline(never)]
#[cfg(target_os = "ios")]
pub extern "C" fn start_app() {
    main();
}

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
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow, Window, WindowMode};
use bevy::winit::{UpdateMode, WinitSettings};
use shared::{
    ClearAllZombies, ClientInput, find_animation_root, FireShot, GameOver, GameStateEvent, Health,
    HEAD_DAMAGE, HITBOX_BELLY_MIN_Y, HITBOX_CHEST_MIN_Y, HITBOX_HEAD_MIN_Y, HITBOX_LIMB_LATERAL_MIN,
    KnockbackVelocity, NetPlayer, Player, PlayerDied, ZombieMotion,
    ZombieAnimationPlayer,
    PlayerStance, RequestRestart, Scenario, SetAutoMaintainZombies, SetDamageMultiplier, SetFreezeZombies, SetZombieCount, ShotConfirmed, ShotFx,
    SpawnMoreZombies, Zombie, ZombieAttackTimer, ZombieHitboxBones, ZombieKind, ZombieShowcaseClip, wire_zombie_hitbox_bones,
    zombie_kind_clip_count, zombie_kind_mesh_path, zombie_kind_anim_path, MAX_SHOT_DISTANCE,
    HEAD_HITBOX_RADIUS, LIMB_HITBOX_RADIUS, TORSO_HITBOX_RADIUS,
    resolve_bone_world_positions, zombie_hit_damage_ray,
    ZOMBIE_ATTACK_INTERVAL, ZOMBIE_OUTER_HEIGHT, ZOMBIE_OUTER_RADIUS,
    zombie_pose_for, ZombieAnim, ZombiePose, ZombieWalkVariant, ZOMBIE_KIND_WEIGHTS, ZOMBIE_MAX_HEALTH, ZOMBIE_STOP_DISTANCE,
};

#[cfg(target_vendor = "apple")]
pub mod cadisplaylink;

mod weapon;
use weapon::*;
mod dismember;
use dismember::*;
mod auto_lean;
mod lightmap;
mod autopilot;
use auto_lean::*;
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

mod audio;
use audio::*;

mod blood;
mod bullet_holes;
use blood::*;
use bullet_holes::*;

mod zombie_reaction;
use zombie_reaction::*;

mod gun_view;
use gun_view::*;

mod map;

mod screen_effects;
use screen_effects::*;

mod grip_ik;
mod muzzle_fx;
mod explosion;
mod ragdoll;
mod viewmodel_body;
mod scope_lens;
use grip_ik::*;
use muzzle_fx::*;
use explosion::*;
use ragdoll::*;
use viewmodel_body::*;
use scope_lens::*;

mod dissolve;
use dissolve::*;

mod damage_overlay;
use damage_overlay::*;

mod combat;


mod hud;
use hud::*;

mod character_tuning;
use character_tuning::*;

mod graphics_settings;
use graphics_settings::*;

mod scene_setup;
mod magazine_drop;
mod shell_casings;
use scene_setup::*;
use magazine_drop::*;
use shell_casings::*;

mod game_flow;
use game_flow::*;

mod f9_debug;
use f9_debug::*;

mod navigation;
use navigation::*;

mod sim;
use sim::SimPlugin;

mod zombie_nav;

mod settings_persistence;
use settings_persistence::*;

mod splash_screen;
use splash_screen::*;


/// How far the eye sits behind the player's own eye point. The gun and the
/// body subtract it again, so pulling back moves only the camera.
pub(crate) fn view_pullback() -> f32 {
    std::env::var("VIEW_PULLBACK").ok().and_then(|v| v.parse().ok()).unwrap_or(0.13)
}


/// Low ready: held near the middle and a little forward of the hip rest, so
/// coming back onto the sights is a short move instead of a full swing across
/// the body. Sits just right of centre, on the firing-hand side, and
/// muzzle-down, which is where a rifle hangs when its owner is waiting.
const GUN_READY_SPEED: f32 = 7.0;
const GUN_READY_LEAN_CUTOFF: f32 = 0.12;
const GUN_SETTLE_JITTER: Vec3 = Vec3::new(0.030, 0.024, 0.020);
/// Degrees about the bore the weapon can come to rest turned by.
const GUN_SETTLE_TWIST_DEG: f32 = 3.0;
const GUN_SETTLE_SPEED: f32 = 3.0;
const GUN_SHOULDER_SPEED: f32 = 9.0;
const GUN_BOB_SPEED: f32 = 8.0;
const GUN_BOB_AMOUNT_X: f32 = 0.015;
const GUN_BOB_AMOUNT_Y: f32 = 0.012;
const GUN_BOB_BLEND_SPEED: f32 = 6.0;
/// Radians the weapon's cant trails the body's. Arms are compliant; without
/// the lag the weapon rolls in exact step with the hips and reads as welded.
const GUN_BOB_LAG: f32 = 0.35;
const GUN_BOB_ROLL_DEG: f32 = 1.1;
const GUN_BOB_PITCH_DEG: f32 = 0.55;
/// Cadence and amplitude together, so a sprint is a different gait rather
/// than a walk with the volume turned up.
const SPRINT_BOB_GAIT: f32 = 1.35;
const BOB_GAIT_BLEND_SPEED: f32 = 4.0;

/// Deliberately a third of the weapon's. If the view moves as much as the
/// weapon, the weapon looks still and the world looks like it is shaking --
/// what sells a step is the weapon moving *within* the frame.
const CAMERA_BOB_AMOUNT_X: f32 = 0.005;
const CAMERA_BOB_AMOUNT_Y: f32 = 0.008;
/// Roll sells walking harder than translation does, and makes people sick
/// faster too. Under half a degree.
const CAMERA_BOB_ROLL_DEG: f32 = 0.40;
const AIM_BOB_MULTIPLIER: f32 = 0.35;

/// How far the weapon lags behind a turn. SPEED is what reads as weight:
/// lower leaves it behind for longer. SENSITIVITY has to keep an ordinary
/// turn inside MAX_DEG, or every movement alike pins it at full swing.
///
/// SENSITIVITY is degrees of lag per radian the view turned, so sway follows
/// the view rather than the mouse and stays right at any sensitivity or zoom.
const LOOK_SWAY_SENSITIVITY: f32 = 50.0;
const LOOK_SWAY_MAX_DEG: f32 = 3.5;
const LOOK_SWAY_SPEED: f32 = 5.0;

const WALL_GUN_RAISE_DISTANCE: f32 = 1.5;
const WALL_GUN_RAISE_HEIGHT: f32 = 0.18;
const WALL_GUN_RAISE_ROT_DEG: f32 = 90.0;
const WALL_GUN_RAISE_SPEED: f32 = 10.0;


const RELOAD_DIP: f32 = 0.12;
/// Muzzle up, about the firing grip. Degrees.
const RELOAD_TILT_DEG: f32 = 22.0;
/// Roll about the bore during a reload. Negative brings the magazine well
/// round towards the weak hand, which is the hand that pulls the magazine.
const RELOAD_ROLL_DEG: f32 = -20.0;


const MUZZLE_FLASH_LIFETIME: f32 = 0.05;
const FLASH_QUAD_SIZE: f32 = 0.32;
const MUZZLE_EMBER_COUNT: usize = 3;
const MUZZLE_EMBER_SIZE: f32 = 0.009;
const MUZZLE_EMBER_SPEED: f32 = 12.0;
const MUZZLE_EMBER_CONE: f32 = 0.22;
const MUZZLE_EMBER_LIFETIME: f32 = 0.13;

const MUZZLE_BLADE_CHANCE: f32 = 0.5;
const MUZZLE_BLADE_WIDTH: f32 = 0.55;
const MUZZLE_BLADE_LENGTH: f32 = 1.9;
const MUZZLE_LIGHT_INTENSITY: f32 = 210_000.0;
const MUZZLE_LIGHT_RANGE: f32 = 3.2;

const TRACER_CHANCE: f32 = 0.8;
const TRACER_SPEED: f32 = 380.0;
const TRACER_STREAK_LENGTH: f32 = 22.0;
const TRACER_WIDTH: f32 = 0.035;
const TRACER_MIN_PIXELS: f32 = 3.0;
const TRACER_NEAR_SKIP: f32 = 3.0;
const TRACER_FADE_STEPS: usize = 5;
const TRACER_SEGMENTS: usize = 14;

const TRACER_START_Y_OFFSET: f32 = -0.05;

const SMOKE_LIFETIME: f32 = 1.9;
const SMOKE_START_SCALE: f32 = 0.52;
/// How opaque the native path's stand-in puff starts. The real material is
/// thinner than this and builds up by overlapping; one flat quad cannot.
const SMOKE_STAND_IN_ALPHA: f32 = 0.34;
const SMOKE_GROWTH_PER_SEC: f32 = 1.5;
const SMOKE_DRAG: f32 = 1.2;
const SMOKE_MAX_PUFFS: usize = 9;
const SMOKE_BUOYANCY: f32 = 0.22;

/// Nothing goes up while the trigger is down. FIRE_RATE is 0.1, so a shot
/// resets the clock long before this, and the trail only starts once the
/// shooting has actually stopped.
const BARREL_SMOKE_DELAY: f32 = 0.25;
const BARREL_SMOKE_WINDOW: f32 = 1.6;
const BARREL_TRAIL_MAX_POINTS: usize = 26;
const BARREL_TRAIL_INTERVAL: f32 = 0.045;
const BARREL_TRAIL_LIFETIME: f32 = 2.4;
const BARREL_TRAIL_RISE: f32 = 0.34;
const BARREL_TRAIL_SWAY: f32 = 0.12;
const BARREL_TRAIL_WIDTH: f32 = 0.018;
const BARREL_TRAIL_SPREAD: f32 = 0.13;
const BARREL_TRAIL_ALPHA: f32 = 0.55;

/// The trail is laid down in world space, so a fast turn sweeps the muzzle
/// through a wide arc and paints a ribbon across the view. Faded out by how
/// hard the eye is moving: rad/s for the turn, m/s for the walk, against a
/// sprint of 6.4.
/// Nothing below the start: ordinary aiming and walking leave it alone, and
/// only a hard swing or a sprint takes it out. Measured in play, a turn
/// averages 0.5 rad/s and peaks near 4.7, a walk averages 1.1 m/s against a
/// sprint of 6.4.
const BARREL_TRAIL_TURN_START: f32 = 0.9;
const BARREL_TRAIL_TURN_FADE: f32 = 2.6;
const BARREL_TRAIL_MOVE_START: f32 = 2.5;
const BARREL_TRAIL_MOVE_FADE: f32 = 5.5;
const BARREL_TRAIL_FADE_SPEED: f32 = 6.0;

const BLOOD_PARTICLE_COUNT: usize = 14;
const BLOOD_PARTICLE_SPEED: f32 = 3.2;
const BLOOD_PARTICLE_GRAVITY: f32 = -9.0;
const BLOOD_PARTICLE_LIFETIME: f32 = 0.6;
const BLOOD_PARTICLE_SIZE: f32 = 0.075;
const BLOOD_DROPLET_TEXTURE_SIZE: u32 = 32;
const MAX_BLOOD_PARTICLES: usize = 900;
const BLOOD_STAIN_SIZE: f32 = 0.22;
const BLOOD_FLOOR_STAIN_SIZE: f32 = 0.55;
const BLOOD_STAIN_VARIANTS: usize = 4;
const BLOOD_STAIN_LIFETIME: f32 = 12.0;

const BLOOD_BURST_SIZE: f32 = 0.34;
const BLOOD_BURST_LIFETIME: f32 = 0.22;
const BLOOD_BURST_START_SCALE: f32 = 0.35;
const BLOOD_BURST_END_SCALE: f32 = 1.7;
const BLOOD_BURST_NORMAL_OFFSET: f32 = 0.05;

const BLOOD_EXIT_PARTICLE_COUNT: usize = 11;
const BLOOD_EXIT_SPEED: f32 = 6.0;
const BLOOD_EXIT_CONE: f32 = 0.38;
const BLOOD_EXIT_WOUND_OFFSET: f32 = 0.3;
const BLOOD_WALL_RAYCAST_DISTANCE: f32 = 4.5;
const BLOOD_WALL_STAIN_SIZE: f32 = 0.42;
const BLOOD_WALL_STAIN_COUNT: usize = 3;
const BLOOD_WALL_STAIN_SPREAD: f32 = 0.22;

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


// Recoil is per weapon now: see WeaponDef::recoil in weapon.rs.

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

        Self(cfg!(target_os = "ios") || std::env::var("TOUCH_UI_PREVIEW").is_ok())
    }
}

const CROSSHAIR_CONTAINER_PX: f32 = 50.0;
const CROSSHAIR_CENTER_PX: f32 = CROSSHAIR_CONTAINER_PX / 2.0;
const CROSSHAIR_LINE_LENGTH_PX: f32 = 6.0;
const CROSSHAIR_LINE_THICKNESS_PX: f32 = 2.0;
const CROSSHAIR_BASE_GAP_PX: f32 = 4.0;
const CROSSHAIR_GAP_SPREAD_SCALE: f32 = 500.0;

const BASE_FOV: f32 = std::f32::consts::PI / 4.0;

pub(crate) const PHYSICS_SUBSTEPS: u32 = 4;
pub(crate) const UI_LAYOUT_HZ: u64 = 30;

const AIM_ZOOM_SPEED: f32 = 12.0;
const AIM_SPREAD_MULTIPLIER: f32 = 0.35;

const AIM_DOF_FOCAL_DISTANCE_A: f32 = 0.2;
const AIM_DOF_FOCAL_DISTANCE_B: f32 = 0.25;
const AIM_DOF_APERTURE_F_STOPS: f32 = 11.0;

const RESOLUTION_SCALE_PRESETS: [f32; 7] = [0.2, 0.3, 0.4, 0.5, 0.65, 0.8, 1.0];

const FPS_CAP_PRESETS: [u32; 4] = [120, 80, 60, 30];

const SCOPE_TEXTURE_SIZE: u32 = 512;
/// Only the first frame, before update_scope_lens writes the weapon.s own
/// magnification over it.
const SCOPE_DEFAULT_ZOOM: f32 = 8.0;

const SCOPE_ZOOM_STEP_FACTOR: f32 = 1.15;
const SCOPE_BASE_FOV: f32 = BASE_FOV;

const AUTO_ZOOM_NEAR_DISTANCE: f32 = 10.0;
const AUTO_ZOOM_FAR_DISTANCE: f32 = 100.0;






const SCOPE_ACTIVE_ADS_THRESHOLD: f32 = 0.02;

const SCOPE_HIDDEN_WALL_RAISE_THRESHOLD: f32 = 0.2;

const WORLD_LAYER: usize = 0;
const VIEWMODEL_LAYER: usize = 1;

const SCOPE_RETICLE_LAYER: usize = 2;

/// The ring that blacks out everything outside the sight picture while the
/// main camera is doing the magnifying itself. Its own layer because the
/// viewmodel layer is exactly what gets switched off in that mode.
/// How much of the screen half height the sight picture fills in that mode.

/// A duplex reticle, in radians subtended. The mark hangs off the scope camera
/// turned to point down the barrel, so only its angular size is ever visible —
/// pushing it further out slides it along the same ray and changes nothing.
/// There is no throw to set, and no parallax: the mark shows the barrel's
/// direction, which is where the bullet goes, wherever the eye happens to be.
///
/// REACH overshoots the 0.098 rad the scope sees at default zoom, so the thick
/// posts run off the edge of the picture rather than stopping inside it.
const SCOPE_RETICLE_REACH: f32 = 0.070;
/// The AK's short scope keeps a lit chevron instead: close work, and a duplex
/// would cover more of the picture than it is worth at that magnification.
const SCOPE_CHEVRON_WIDTH: f32 = 0.00333;
const SCOPE_CHEVRON_HEIGHT: f32 = 0.00667;
/// Where the thick outer posts give way to the fine inner lines.
const SCOPE_RETICLE_TAPER: f32 = 0.42;
const SCOPE_RETICLE_POST_WIDTH: f32 = 0.0013;
const SCOPE_RETICLE_LINE_WIDTH: f32 = 0.00035;
/// Gap at dead centre, so the mark never hides what it is pointing at.
const SCOPE_RETICLE_CENTRE_GAP: f32 = 0.035;
const SCOPE_RETICLE_DOT_SIZE: f32 = 0.0011;
// Sits in the duplex's centre gap, which is SCOPE_RETICLE_CENTRE_GAP * REACH
// half wide, so it has to stay well inside that or it fills the hole the gap
// exists to leave.
const SCOPE_RETICLE_CENTRE_DOT_SIZE: f32 = 0.0013;
const SCOPE_RETICLE_DOTS_PER_ARM: usize = 4;

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

const HIT_MARKER_LIFETIME: f32 = 3.0;

const PLAYER_MAX_HEALTH: f32 = 100.0;

const ZOMBIE_ATTACK_DAMAGE: f32 = 12.0;

const FLOATING_DAMAGE_LIFETIME: f32 = 0.9;
const FLOATING_DAMAGE_RISE_SPEED: f32 = 0.7;
const FLOATING_DAMAGE_FONT_SIZE: f32 = 14.0;
const FLOATING_DAMAGE_CRIT_FONT_SIZE: f32 = 24.0;

const ZOMBIE_HEALTH_BAR_HEIGHT: f32 = 1.3;
const ZOMBIE_HEALTH_BAR_WIDTH_PX: f32 = 50.0;
const ZOMBIE_HEALTH_BAR_HEIGHT_PX: f32 = 6.0;

impl Default for SettingsMenuVisible {
    fn default() -> Self {
        Self(std::env::var("OPEN_SETTINGS_MENU").is_ok())
    }
}

impl Default for MapSelectionPending {
    fn default() -> Self {
        Self(game_flow::map_id_override().is_none())
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
struct PerfTimings {
    health_bars_ms: f32,

}

/// Radians the view turned this frame, after sensitivity and magnification.
/// The weapon lags this, not the raw mouse; see LOOK_SWAY_SENSITIVITY.
#[derive(Resource, Default)]
struct MouseLookDelta(Vec2);

#[bevy_main]
pub fn main() {
    let mut app = App::new();
    app.add_plugins((

            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Last Day in Saigon".to_string(),

                        mode: WindowMode::Windowed,
                        // VSYNC=0 unclamps the frame rate. Measuring a cost
                        // through vsync cannot see anything cheaper than a
                        // refresh: every setting reads as 8.33ms on a 120Hz
                        // panel whether it costs 2ms or 8.
                        present_mode: if std::env::var("VSYNC").map(|v| v == "0").unwrap_or(false) {
                            bevy::window::PresentMode::AutoNoVsync
                        } else {
                            bevy::window::PresentMode::default()
                        },
                        ..default()
                    }),
                    ..default()
                })

                .set(TaskPoolPlugin {
                    task_pool_options: if cfg!(target_os = "ios") {
                        TaskPoolOptions::with_num_threads(4)
                    } else {
                        TaskPoolOptions::default()
                    },
                }),
            MeshPickingPlugin,
            bevy::ui_render::prelude::UiMaterialPlugin::<DamageOverlayMaterial>::default(),
            MaterialPlugin::<ScopeLensMaterial>::default(),
            MaterialPlugin::<MuzzleSmokeMaterial>::default(),
            MaterialPlugin::<MuzzleFlashMaterial>::default(),
            bevy::core_pipeline::fullscreen_material::FullscreenMaterialPlugin::<ScreenEffects>::default(),
            PhysicsPlugins::default(),
            SimPlugin,
            zombie_nav::ZombieNavmeshPlugin,
        ))
        // Four, not Avian's default six: the solver was running at 384Hz for
        // capsule characters and ragdolls, and the two builds have to agree or
        // they are not the same game. Raise it again if joints go rubbery or
        // a crowd starts interpenetrating.
        .insert_resource(avian3d::prelude::SubstepCount(PHYSICS_SUBSTEPS))
        // The HUD is near static -- ammo, health, a compass strip -- and
        // recomputing its layout at the refresh rate cost 0.09ms a frame to
        // produce the same numbers. A newly spawned node now waits up to 33ms
        // to be placed, which is the price.
        .configure_sets(
            PostUpdate,
            bevy::ui::UiSystems::Layout.run_if(bevy::time::common_conditions::on_timer(
                std::time::Duration::from_millis(1000 / UI_LAYOUT_HZ),
            )),
        )

        .insert_resource(if cfg!(target_os = "ios") {
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

    app.add_plugins(GamePlugin);
    app.run();
}

pub(crate) fn debug_keys_allowed() -> bool {
    true
}

pub(crate) struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
    app.init_resource::<PlayerModelPoseOverride>()
        .register_type::<PlayerModelPose>()
        .register_type::<ZombiePose>()
        .register_type::<PlayerModelPoseOverride>()
        .init_resource::<ZombieCountUi>()
        .init_resource::<AutoMaintainUi>()

        .init_state::<AppState>()
        .init_resource::<AfterLoading>()
        .init_resource::<AimToggleMode>()
        .init_resource::<AimHeld>()
        .add_observer(on_zombie_replicated)
        .add_observer(on_zombie_kind_replicated)
        .add_observer(on_shot_fx)
        .add_observer(note_shot_for_barrel_smoke)
        .add_observer(on_explosion)
        .add_observer(on_shot_confirmed)
        .add_observer(on_game_state_event)
        .add_plugins(F9DebugPlugin)
        .add_plugins(navigation::ClientNavigationPlugin)
        // RENDER_DIAG=1 times every render graph node. On Metal only the CPU
        // side is recorded, which is the side a second camera is expensive on:
        // the graph is walked per view whether that view draws anything or not.
        .add_plugins(render_diagnostics_if_asked())
        .add_plugins(NoShadowPassPlugin)
        .add_plugins(DrawCountPlugin)

        .insert_resource(Noclip(false))
        .init_resource::<HitFlash>()
        .init_resource::<Autopilot>()
        .init_resource::<AutopilotLookOnly>()
        .init_resource::<AutopilotState>()
        .init_resource::<AutopilotIntent>()
        .init_resource::<FreezeZombies>()
        .init_resource::<ShowHitboxes>()
        .init_resource::<RenderResolutionScale>()
        .init_resource::<FpsCap>()
        .init_resource::<GodMode>()
        .init_resource::<ShowAutopilotTarget>()
        .init_resource::<ShowHitMarkers>()
        .init_resource::<ShowFloatingDamage>()
        .init_resource::<ShowBulletTracers>()
        .init_resource::<BarrelSmoke>()
        .init_resource::<LimbDamage>()
        .init_resource::<gun_view::WalkCycle>()
        .init_resource::<ActiveWeapon>()
        .init_resource::<weapon::WeaponSwitchRequest>()
        .add_systems(
            PostUpdate,
            sever_limbs
                .after(bevy::app::AnimationSystems)
                .before(TransformSystems::Propagate),
        )
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
        .init_resource::<AutoLean>()
        .init_resource::<PlayerModelTuning>()
        .init_resource::<GunVisible>()
        .init_resource::<MapMsaaOn>()
        .init_resource::<ShadowQuality>()
        .init_resource::<LightQuality>()
        .init_resource::<PointLightQuality>()
        .init_resource::<ScopeCameraEnabled>()
        .init_resource::<ScopeLensDistortion>()
        .init_resource::<ShadowCascades>()
        .init_resource::<BloomEnabled>()
        .init_resource::<ScreenEffectsEnabled>()
        .init_resource::<FogEnabled>()
        .init_resource::<BulletHolesEnabled>()
        .init_resource::<BloodEnabled>()
        .init_resource::<PlayerModelVisible>()
        .init_resource::<MuzzleSmokeEnabled>()
        .init_resource::<PortSmokeEnabled>()
        .init_resource::<BarrelSmokeEnabled>()
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
        .add_systems(Startup, spawn_pole_targets)
        .add_systems(OnEnter(AppState::Menu), enter_menu)
        .add_systems(OnExit(AppState::Menu), exit_menu)
        // Gameplay is built while the loading screen is still up, so the map
        // and the weapon are queued and waited on rather than popping in once
        // play has already started.
        .add_systems(
            OnEnter(AppState::Loading),
            (
                spawn_loading_screen,
                setup_gameplay.run_if(|after: Res<AfterLoading>| after.0 == AppState::InGame),
            ),
        )
        .add_systems(OnExit(AppState::Loading), despawn_loading_screen)
        .add_systems(Update, finish_loading.run_if(in_state(AppState::Loading)));
    app.add_systems(Update, persist_settings_on_change);
    app.add_systems(Update, build_zombie_anims_when_loaded);

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
                touch_weapon_button,
                touch_aim_button,
                touch_scope_zoom_drag,
                touch_jump_button,
                update_noclip_fly_input,
                touch_crouch_button,
                touch_reload_button,
                touch_prone_button,
                touch_sprint_button,
                touch_lean_buttons,
                update_touch_button_press_visual,
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
                )
                    .chain(),
                (
                    cheat_toggle_buttons_3,
                    (render_toggle_buttons, update_render_toggle_colors).chain(),
                    autopilot_toggle_button,
                    update_extra_toggle_colors,
                    cheat_spawn_zombie_button,
                    shadow_quality_buttons,
                    update_shadow_quality_button_colors,
                    light_quality_buttons,
                    update_light_quality_button_colors,
                    apply_point_light_quality,
                    point_light_quality_buttons,
                    update_point_light_quality_button_colors,
                )
                    .chain(),
            )
                .chain(),
            (
                (
                    grab_cursor,
                    update_aim_held,
                    toggle_noclip.run_if(debug_keys_allowed),
                    toggle_player_stance,
                    toggle_debug_flags.run_if(debug_keys_allowed),
                    autopilot_pick_target,
                    // No longer ordered against LandmassSystems::Output --
                    // landmass runs in FixedUpdate now, and this does not.
                    // The desired velocity read here can be one frame old,
                    // which is nothing against a 20Hz decision rate.
                    autopilot_think,
                    mouse_look,
                    // Before shoot, not after. Everything that leaves the
                    // barrel is placed from the weapon's transform, and a
                    // weapon posed afterwards leaves that transform a frame
                    // old -- 2cm of it while the weapon is swinging to ready,
                    // which shows as the flash hanging off the muzzle.
                    // Before everything that rides it: the weapon and the
                    // view both read this cycle in the same frame it is
                    // advanced, so they cannot drift apart.
                    advance_walk_cycle,
                    gun_view_animation,
                    shoot,
                    reload_gun,
                    suggest_auto_lean,
                    apply_lean_input,
                    apply_view_recoil,
                    apply_aim_zoom,
                    apply_aim_dof,
                    send_player_input.run_if(map::world_ready),
                    sync_camera_to_player,
                    zombies_damage_player,
                )
                    .chain(),

                (count_local_kills,).chain(),
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
                wire_zombie_animations.run_if(resource_exists::<ZombieKindAnims>),
                wire_zombie_hitbox_bones,
                wire_gun_mesh,
                wire_weapon_sockets,
                log_frame_pacing,
                (track_muzzle_socket, track_optical_axis),
                update_zombie_animation.run_if(resource_exists::<ZombieAnim>),
                freeze_zombie_animations,

                (
                    wire_player_model_scene_nodes,
                    (spawn_viewmodel_body, wire_viewmodel_body_meshes, apply_player_model_visible),
                    spawn_grip_sockets,
                    wire_player_model_animations.run_if(resource_exists::<PlayerModelAnim>),
                    update_local_player_model_animation.run_if(resource_exists::<PlayerModelAnim>),
                    apply_player_model_pose_override.run_if(resource_exists::<PlayerModelAnim>),
                    update_health_ui,
                    update_compass_ui,
                    update_radar_ui,
                    update_zombie_health_bars,
                    update_floating_damage_texts,
                    play_footsteps,
                    play_zombie_footsteps,
                    update_damage_overlay,
                    update_screen_effects,
                    demote_fully_metallic_imports,
                    exclude_oversized_shadow_casters,
                    (
                    test_blast_key.run_if(debug_keys_allowed),
                    switch_weapon,
                    forget_gone_zombies,
                    update_gore,
                    update_death,
                    spawn_what_was_armed_last_frame,
                    apply_pending_blast,
                    arm_ragdoll,
                    settle_corpse,
                )
                    .chain(),
                update_dissolve,
                )
                    .chain(),
            )
                .chain(),
            (
                adjust_scope_zoom,
                update_scope_zoom_slider_visual,
                update_scope_zoom_slider_visibility,
                sync_scope_camera,
                (place_scope_reticle, swap_scope_reticle),
                (update_scope_lens, apply_scope_lens_distortion, apply_bloom, apply_screen_effects_toggle, apply_fog).chain(),
                update_ammo_ui,
                update_crosshair,
                set_hitbox_gizmo_depth_bias,
                draw_zombie_hitboxes,
                update_splash_screen,
                despawn_hit_markers,
                despawn_muzzle_flashes,
                update_bullet_tracers,
                update_smoke,
                update_blood_particles,
                (
                    (update_blood_bursts, update_bullet_holes, update_impact_dust, update_casings, update_port_smoke),
                    (drop_magazine_on_reload, restore_magazine_after_reload, update_dropped_magazines),
                ),
                (
                    update_muzzle_smoke,
                    update_muzzle_flash,
                    update_barrel_smoke,
                    // Nested rather than added beside the others: this tuple's
                    // parent was already at Bevy's twenty-element limit.
                    //
                    // Only on the native path -- the windowed build renders
                    // the real materials and a second would draw over them.
                    (attach_muzzle_fx_stand_ins, fade_muzzle_fx_stand_ins)
                        .chain()
                        .run_if(native_renderer_in_use),
                ),
                update_spark_particles,
                despawn_blood_stains,
            )
                .chain(),
        )
            .chain(),
    );

    app.add_systems(
        PostUpdate,
        (
            pose_viewmodel_arms,
            shape_viewmodel_skeleton,
            drive_skeleton_from_ragdoll,
            aim_zombie_heads,
            apply_bone_flinch,
            hold_finger_grip,
        )
            .after(AnimationSystems)
            .before(TransformSystems::Propagate)
            .run_if(in_state(AppState::InGame)),
    );

    app.add_systems(
        PostUpdate,
        (
            drive_reload_hand,
            update_pole_targets,
            solve_grip_ik,
        )
            .chain()
            .after(TransformSystems::Propagate)
            .run_if(grip_ik_enabled)
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

const COMPASS_WIDTH: f32 = 280.0;
const COMPASS_HEIGHT: f32 = 26.0;

const COMPASS_PX_PER_DEG: f32 = 2.2;

const COMPASS_MARKS: [(f32, &str); 8] =
    [(0.0, "N"), (45.0, "NE"), (90.0, "E"), (135.0, "SE"), (180.0, "S"), (225.0, "SW"), (270.0, "W"), (315.0, "NW")];

const RADAR_LEFT: f32 = 16.0;
const RADAR_TOP: f32 = 16.0;
const RADAR_SIZE: f32 = 150.0;
const RADAR_RADIUS: f32 = RADAR_SIZE / 2.0;
const RADAR_RANGE: f32 = 40.0;
const RADAR_BLIP_SIZE_PX: f32 = 8.0;
const RADAR_SWEEP_DEG_PER_SEC: f32 = 60.0;
const RADAR_TRAIL_COUNT: u8 = 3;
const RADAR_TRAIL_STEP_DEG: f32 = 10.0;

#[unsafe(no_mangle)]
#[inline(never)]
#[cfg(target_os = "ios")]
pub extern "C" fn start_app() {
    main();
}

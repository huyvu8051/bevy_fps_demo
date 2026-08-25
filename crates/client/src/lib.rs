use avian3d::prelude::*;
use bevy::animation::graph::{AnimationGraph, AnimationGraphHandle, AnimationNodeIndex};
use bevy::animation::AnimationPlayer;
use bevy::dev_tools::fps_overlay::FpsOverlayPlugin;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::RenderTarget;
use bevy::ecs::system::SystemParam;
use bevy::input::mouse::{MouseMotion, MouseScrollUnit, MouseWheel};
use bevy::math::ops;
use bevy::picking::hover::HoverMap;
use bevy::picking::mesh_picking::MeshPickingPlugin;
use bevy::picking::pointer::PointerId;
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
use bevy_replicon_renet2::{
    netcode::{ClientAuthentication, NativeSocket, NetcodeClientTransport},
    renet2::{ConnectionConfig, RenetClient},
    RenetChannelsExt, RepliconRenetPlugins,
};
use shared::{
    ClearAllZombies, ClientInput, DebugBot, DespawnDebugBot, FireShot, GameOver, GameStateEvent, Health,
    HEAD_DAMAGE, HITBOX_BELLY_MIN_Y, HITBOX_CHEST_MIN_Y, HITBOX_HEAD_MIN_Y, HITBOX_LIMB_LATERAL_MIN,
    KnockbackVelocity, LastProcessedInput, MoveDebugBot, NetPlayer, NetTransform, Player, PlayerDied,
    PlayerDiedFx, PlayerFiredFx,
    PlayerStance, RequestRestart, SetAutoMaintainZombies, SetZombieCount, ShotFx, SpawnDebugBot,
    SpawnMoreZombies, Zombie, ZombieAttackTimer, ZombieHitboxBones, ZombieMotion, wire_zombie_hitbox_bones,
    resolve_bone_world_positions, zombie_hit_damage_bones, INTERP_DELAY_SECS, MAX_SHOT_DISTANCE, MOVE_SPEED, ZOMBIE_ATTACK_INTERVAL,
    ZOMBIE_LENGTH, ZOMBIE_LOD_DISTANCE, ZOMBIE_MAX_HEALTH, ZOMBIE_RADIUS, ZOMBIE_SCALE, ZOMBIE_SPAWNS,
    ZOMBIE_STOP_DISTANCE,
};
use shared::sim_latency::{sim_latency_from_env, LaggedSocket};

// Experimental CADisplayLink-driven entry point for MINIMAL_MAP_PROBE — see
// its own module doc comment. iOS-only; the extern "C" functions in
// cadisplaylink::ffi are the only way into it, called from a separate
// Xcode target's Swift code, never from the normal winit-driven app.
#[cfg(target_os = "ios")]
mod cadisplaylink;

// AI takeover of the local player (auto-aim/move/fire) — see its own
// module doc comment. Platform-independent (not iOS-only): its systems are
// wired into the same Update chain as the rest of real gameplay below.
mod autopilot;
use autopilot::{autopilot_pick_target, autopilot_think, Autopilot, AutopilotIntent, AutopilotState};

// Standalone FPS probe (camera + a small static primitive-built room, no
// game logic at all) — see its own module doc comment. Only reachable via
// cadisplaylink::build_cadisplaylink_fps_probe_app.
#[cfg(target_os = "ios")]
mod fps_probe;

// Downloaded once into assets/models/ so the app doesn't depend on a network
// request (and a remote repo's file layout) every time it starts.
// To load from a URL instead, add the WebAssetPlugin back (see bevy::asset::io::web)
// and point this at an https:// path — bevy treats both the same way.
const SCENE_PATH: &str = "models/Sponza/Sponza.gltf";

// Sponza's dim, column-choked halls make it a bad zombie-shooting test bed
// (poor sightlines, no open flat range to check bullet spread/spark/blood
// effects at varying distances). No other map asset is available (offline,
// no other .gltf on disk) and downloading one wasn't an option, so this is
// a plain procedural arena (see spawn_test_arena) — flat lit floor + a
// square perimeter wall, nothing fancy, just open sightlines. Flip this
// back to false to use Sponza again; PLAYER_SPAWN/ZOMBIE_SPAWNS/
// NAVMESH_BOUNDS below all branch on it too.
const USE_TEST_ARENA: bool = true;
// Half the arena's side length (so it spans -ARENA_HALF_SIZE..ARENA_HALF_SIZE
// on both X and Z) — see spawn_test_arena.
const ARENA_HALF_SIZE: f32 = 18.0;
const ARENA_WALL_HEIGHT: f32 = 5.0;
const ARENA_WALL_THICKNESS: f32 = 1.0;

// "AK47 with Scope" by Wolf Schevelenbos (Sketchfab, CC-BY 4.0) —
// https://sketchfab.com/3d-models/ak47-with-scope-d4d9720f3677485b958ceecc02f7965c
// Keep this attribution if the project is shared/published (license requirement).
// Modeled in millimeters (bounding box ~970x87x386mm), hence the 0.001 scale
// down to meters. Its long axis is local X, not the -Z Bevy viewmodels usually
// assume, hence the extra Y rotation to point the barrel forward.
const GUN_PATH: &str = "models/ak47_scope.glb";
const GUN_OFFSET: Vec3 = Vec3::new(0.22, -0.28, -0.65);
// Aim-down-sights pose while holding right mouse (see gun_view_animation) —
// centered horizontally and pulled in close to the camera so the scope sits
// roughly where the lens/eye would look through it. GUN_PATH's actual scope
// position on the model isn't known (can't see the rendered model to
// measure it), so this is a guess based on GUN_OFFSET's own general shape,
// not a calibrated value — may need nudging once actually seen in-game.
// Y tuned in-game across several rounds (-0.05 -> -0.18 -> -0.28 -> -0.38
// -> -0.25, the last one given as an exact value) — X/Z were confirmed
// correct from the start.
const GUN_ADS_OFFSET: Vec3 = Vec3::new(0.0, -0.307, -0.35);
const GUN_ADS_SPEED: f32 = 10.0;
const GUN_SCALE: f32 = 0.001;
const GUN_ROTATION_Y_DEG: f32 = -90.0;
const RECOIL_KICK: f32 = 0.12;
// Muzzle rise: the gun mesh itself also kicks upward (translation) and
// tilts its barrel up (rotation), on top of pulling back — separate from
// RECOIL_PITCH_KICK, which only moves the camera/aim point, not the
// visible gun model. Raised once already (0.05 -> 0.09) per "more like a
// real gun" feedback.
const RECOIL_KICK_UP: f32 = 0.01;
const RECOIL_TILT_UP_DEG: f32 = 4.0;
// Random per-shot left/right roll layered on top of RECOIL_TILT_UP_DEG's
// straight-up pitch (see Gun.recoil_roll) so the visual kick doesn't always
// snap along dead-vertical — purely cosmetic, same as RECOIL_TILT_UP_DEG.
// Roll alone only visibly swings whichever end is farthest from the gun
// model's rotation pivot (reported as "the tail wags but not the muzzle") —
// paired with a same-signed yaw kick so the muzzle sways side to side too.
const RECOIL_ROLL_DEG: f32 = 3.0;
const RECOIL_YAW_KICK_DEG: f32 = 3.0;
const RECOIL_DURATION: f32 = 0.12;

// Walk-bob: a figure-eight sway (side to side once, up-down twice per
// stride — feet alternate, so the bob dips on every step) while WASD is
// held, fading out via Gun.bob_blend when the player stops. Halved while
// aiming (AIM_BOB_MULTIPLIER) since ADS is meant to steady the view.
const GUN_BOB_SPEED: f32 = 8.0;
const GUN_BOB_AMOUNT_X: f32 = 0.015;
const GUN_BOB_AMOUNT_Y: f32 = 0.012;
const GUN_BOB_BLEND_SPEED: f32 = 6.0;
const AIM_BOB_MULTIPLIER: f32 = 0.35;

// Inertia lean: whipping the mouse sideways makes the gun's rear lag/lean
// the opposite way for a moment, PUBG-style, before springing back level.
// Degrees of roll per pixel of this frame's mouse-look delta, capped, eased
// toward that target (fast attack) and back to 0 (also via LOOK_SWAY_SPEED)
// once the mouse stops.
const LOOK_SWAY_SENSITIVITY: f32 = 0.15;
const LOOK_SWAY_MAX_DEG: f32 = 0.5;
const LOOK_SWAY_SPEED: f32 = 10.0;

// Pitches the gun barrel up out of the way when standing close to a wall in
// front (PUBG-style), so the viewmodel doesn't visibly clip through it.
// Distance is measured from the camera along its forward ray; the raise
// eases in as that distance shrinks below WALL_GUN_RAISE_DISTANCE, fully
// raised (barrel pointing straight up) at 0.
const WALL_GUN_RAISE_DISTANCE: f32 = 1.5;
const WALL_GUN_RAISE_HEIGHT: f32 = 0.18;
const WALL_GUN_RAISE_ROT_DEG: f32 = 90.0;
const WALL_GUN_RAISE_SPEED: f32 = 10.0;

const MAGAZINE_SIZE: u32 = 50;
const RELOAD_DURATION: f32 = 1.6;
// How far the gun dips down and tilts during a reload, peaking at the
// midpoint of RELOAD_DURATION and easing back to the normal pose at both
// ends (see the parabola in gun_view_animation).
const RELOAD_DIP: f32 = 0.12;
const RELOAD_TILT_DEG: f32 = 25.0;
// RELOAD_TILT_DEG is composed *inside* the gun model's own -90° Y rotation
// (unlike the recoil/wall tilts, which are applied outside it — see
// gun_view_animation), which twists what's meant to be a forward/back pitch
// into something that reads visually as a down-and-left lean instead
// (reported in-game). The down half is already RELOAD_DIP; this is the
// left half, kept separate/explicit here so the scope camera and lens can
// mirror it without needing the same axis-twisted rotation composition.
const RELOAD_LEFT_DIP: f32 = 0.12;

// Rough position of the barrel tip, in the same camera-local space as
// GUN_OFFSET. Muzzle flash/smoke spawn here. Since we can't see the model,
// this is a guess (further forward + a bit higher than the grip) — nudged
// up once already, since the flare was sitting below the barrel.
const MUZZLE_OFFSET: Vec3 = Vec3::new(0.2, -0.005, -1.2);

const MUZZLE_FLASH_LIFETIME: f32 = 0.05;
const MUZZLE_LIGHT_INTENSITY: f32 = 120_000.0;
const MUZZLE_LIGHT_RANGE: f32 = 2.0;

const SMOKE_LIFETIME: f32 = 0.4;
const SMOKE_START_SCALE: f32 = 0.35;
const SMOKE_GROWTH_PER_SEC: f32 = 1.0;

// Blood: only spawned on an actual zombie hit (see shoot()), never on
// environment hits.
const BLOOD_PARTICLE_COUNT: usize = 5;
const BLOOD_PARTICLE_SPEED: f32 = 2.0;
const BLOOD_PARTICLE_GRAVITY: f32 = -9.0;
const BLOOD_PARTICLE_LIFETIME: f32 = 0.6;
const BLOOD_PARTICLE_SIZE: f32 = 0.04;
const BLOOD_STAIN_SIZE: f32 = 0.12;
const BLOOD_STAIN_LIFETIME: f32 = 12.0;
// How far straight down to look for a floor beneath a hit, to place a
// second stain there (blood dripping down and landing) — separate from the
// stain placed right at the hit point itself. A random count of these are
// placed (not always 1), each from a slightly jittered origin so they don't
// all land in exactly the same spot.
const BLOOD_FLOOR_RAYCAST_DISTANCE: f32 = 5.0;
const BLOOD_FLOOR_STAIN_MIN: usize = 1;
const BLOOD_FLOOR_STAIN_MAX: usize = 3;
const BLOOD_FLOOR_STAIN_SPREAD: f32 = 0.3;

// Sparks: only spawned on a non-zombie hit (a wall/environment shot) — the
// mirror-image case of blood, which only spawns on a zombie hit. A random
// count each time (not always the same) so a burst doesn't look too
// mechanical. Cone spreads outward from the hit normal, biased toward it
// rather than fully random, so sparks read as bouncing off the surface
// instead of flying every which way.
const SPARK_PARTICLE_MIN: usize = 4;
const SPARK_PARTICLE_MAX: usize = 9;
const SPARK_PARTICLE_SPEED: f32 = 3.0;
const SPARK_PARTICLE_GRAVITY: f32 = -9.0;
const SPARK_PARTICLE_LIFETIME: f32 = 0.25;
const SPARK_PARTICLE_SIZE: f32 = 0.02;


// Automatic fire: how often a shot can go off while the mouse button is held.
// ~0.1s ≈ 600 rounds/min, roughly AK47-like.
const FIRE_RATE: f32 = 0.1;

// View recoil: each shot kicks the camera pitch up and nudges yaw randomly;
// both settle back down every frame. The kick is clamped so sustained fire
// doesn't spin the camera off into the sky.
const RECOIL_PITCH_KICK: f32 = 0.022;
const RECOIL_PITCH_KICK_MAX: f32 = 0.16;
const RECOIL_YAW_JITTER: f32 = 0.012;
const RECOIL_RECOVERY_SPEED: f32 = 5.0;

// Bullet spread grows with how "hot" the current recoil kick is, so shots
// drift off-target the longer you hold the trigger.
const BULLET_SPREAD: f32 = 0.02;

const MOUSE_SENSITIVITY: f32 = 0.002;
// User-facing multiplier on top of MOUSE_SENSITIVITY, adjustable at runtime
// via the settings menu's Sensitivity row — unlike AIM_MOUSE_SENSITIVITY_
// MULTIPLIER/TOUCH_LOOK_SENSITIVITY_MULTIPLIER (both fixed tuning
// constants), this one is meant to be changed by the player.
const SENSITIVITY_SCALE_MIN: f32 = 0.25;
const SENSITIVITY_SCALE_MAX: f32 = 3.0;
const SENSITIVITY_SCALE_STEP: f32 = 0.25;
// Mouse turns slower while ADS'd — real scopes/sights make fine aim
// adjustments feel twitchy at full sensitivity.
const AIM_MOUSE_SENSITIVITY_MULTIPLIER: f32 = 0.5;
// Same yaw/pitch clamp mouse_look applies — factored out so touch_look can
// share it instead of duplicating the magic numbers.
const PITCH_CLAMP: f32 = 1.54;

// --- Touch controls (iOS/Android) ---
// A single virtual joystick (bottom-left) for movement and a drag-to-look
// region (the whole right half of the screen — no visible Node needed,
// just compared against touch x-position) cover the two things WASD+mouse
// normally do; a fire button covers the one thing left-click does. Nothing
// else (aim/lean/stance/debug tools) has a touch equivalent yet — see the
// plan doc's "out of scope for now" section.
const TOUCH_JOYSTICK_BASE_PX: f32 = 140.0;
const TOUCH_JOYSTICK_KNOB_PX: f32 = 64.0;
// Max distance (px) the knob can be dragged from center — also the
// denominator that normalizes drag distance into a 0..1 move_axis
// magnitude, so dragging further than this doesn't move any faster.
const TOUCH_JOYSTICK_RADIUS_PX: f32 = (TOUCH_JOYSTICK_BASE_PX - TOUCH_JOYSTICK_KNOB_PX) / 2.0;
const TOUCH_FIRE_BUTTON_PX: f32 = 90.0;
// bevy_ui's own UiStack sorts root-level (parentless) UI nodes by
// (GlobalZIndex, ZIndex), falling back to ECS table/archetype iteration
// order — NOT spawn order — for nodes that tie on both. Root nodes are
// explicitly documented as having no stable ordering guarantee (see
// bevy_ui-0.18.1/src/stack.rs's test_ui_stack_system doc comment).
// Confirmed on-device: a churning root UI node (zombie health bar/damage
// number, spawned+despawned continuously during combat) can render ON TOP
// of a static button sharing its screen position, hiding it completely
// (Interaction/hit-testing is unaffected, only the draw order) — this was
// the real root cause of "buttons randomly disappear but still work".
// Every persistent/static top-level UI node (buttons, joystick, HUD text)
// gets this GlobalZIndex so it always draws above anything that doesn't
// (zombie health bars, floating damage numbers, hit markers all stay at
// the implicit default of 0).
const STATIC_UI_Z_INDEX: i32 = 100;
// Touch drag deltas are raw screen pixels per frame, same unit MouseMotion
// deltas are — reusing MOUSE_SENSITIVITY directly felt right in theory, but
// there's no device here to actually verify feel against, so this exists
// as an explicit second multiplier to tune later rather than assuming
// touch and mouse should feel identical.
const TOUCH_LOOK_SENSITIVITY_MULTIPLIER: f32 = 1.0;

// Auto-on for iOS/Android builds; TOUCH_UI_PREVIEW=1 forces it on under a
// desktop build too, since this dev environment has no physical
// device/simulator to build+run the real mobile targets on — this is the
// only way to visually iterate on the touch UI/systems from here.
#[derive(Resource)]
struct TouchControlsEnabled(bool);

impl Default for TouchControlsEnabled {
    fn default() -> Self {
        let mobile = cfg!(target_os = "ios") || cfg!(target_os = "android");
        Self(mobile || std::env::var("TOUCH_UI_PREVIEW").is_ok())
    }
}

#[derive(Component)]
struct TouchJoystickBase;
#[derive(Component)]
struct TouchJoystickKnob;
#[derive(Component)]
struct TouchFireButton;
#[derive(Component)]
struct TouchAimButton;

#[derive(Component)]
struct TouchReloadButton;
#[derive(Component)]
struct TouchJumpButton;
#[derive(Component)]
struct TouchCrouchButton;

// Which touch (Bevy's own per-touch id) is currently driving the joystick,
// and where it started — drag-from-start is the whole joystick model, no
// separate "return to center" animation needed since releasing just drops
// the tracked id (see touch_move_axis).
#[derive(Resource, Default)]
struct TouchMoveState {
    touch_id: Option<u64>,
    origin: Vec2,
}

// Computed fresh each frame by touch_move_axis, read by send_player_input
// exactly like the WASD-derived Vec2 it already builds — same units (x =
// right, y = forward), so send_player_input just adds the two together.
#[derive(Resource, Default)]
struct TouchMoveAxis(Vec2);

#[derive(Resource, Default)]
struct TouchLookState {
    touch_id: Option<u64>,
    last_pos: Vec2,
}

#[derive(Resource, Default)]
struct TouchFiring(bool);

// Toggled by TouchAimButton (touch_aim_button) — unlike TouchFiring (held),
// this is a toggle since there's no natural "hold to aim" gesture that
// doesn't also fight with the fire button under a thumb. Read by every
// is_aiming() call site the same way TouchFiring is read by shoot().
#[derive(Resource, Default)]
struct TouchAiming(bool);

// User-facing look sensitivity multiplier — see SENSITIVITY_SCALE_MIN/MAX's
// own comment. Applied on top of MOUSE_SENSITIVITY by both mouse_look and
// touch_look, so a change affects desktop and mobile identically.
#[derive(Resource)]
struct SensitivityScale(f32);
impl Default for SensitivityScale {
    fn default() -> Self {
        Self(1.0)
    }
}

// One-frame pulse, not a held state: touch_jump_button resets this to false
// every tick and only sets it true on the exact frame TouchJumpButton's
// Interaction changes to Pressed, matching keys.just_pressed(KeyCode::Space)
// (the real game's jump gate — see send_player_input) rather than
// keys.pressed (which would jump repeatedly every frame the button stays
// down). Generic across the real game and minimal_map_probe_main, which
// each consume it differently (physics jump vs. a simple bounce).
#[derive(Resource, Default)]
struct TouchJump(bool);

// Same one-frame-pulse shape as TouchJump — reload_gun ORs this in
// alongside keys.just_pressed(KeyCode::KeyR).
#[derive(Resource, Default)]
struct TouchReload(bool);

// Touch's up/down hold state for noclip flight (Jump = fly up, Crouch =
// fly down) — bundled into one resource/system (update_noclip_fly_input)
// rather than two separate ones so send_player_input only needs one extra
// param (it's already at Bevy's 16-param system function limit; see
// HitMarkerAssets' own doc comment for the same constraint hit elsewhere).
// Unlike TouchJump (one-frame pulse matching keys.just_pressed, for a real
// jump), these track whether the button is CURRENTLY held, matching
// keys.pressed(KeyCode::Space)-style continuous hold — touch has no
// keyboard hold state to read otherwise. touch_crouch_button (the normal-
// gameplay stance toggle) skips its own toggle while noclipping, so
// tapping Crouch during noclip doesn't leave PlayerStance stuck on
// Crouching once noclip is turned back off.
#[derive(Resource, Default)]
struct NoclipFlyInput {
    up: bool,
    down: bool,
}

// Bundles 3 touch-input resource reads send_player_input needs into one
// param — that fn was already at Bevy's 16-param system function limit
// (see HitMarkerAssets' own doc comment for the same constraint hit
// elsewhere), and NoclipFlyInput would have pushed it over.
#[derive(SystemParam)]
struct TouchGameplayInputs<'w> {
    aiming: Res<'w, TouchAiming>,
    jump: Res<'w, TouchJump>,
    noclip_fly: Res<'w, NoclipFlyInput>,
}

// Crosshair: 4 hash marks pushed apart from center by GAP, which grows with
// the current bullet spread (see current_bullet_spread/update_crosshair).
// GAP_SPREAD_SCALE converts spread (a small radian-ish value, roughly
// 0.002-0.03 across every multiplier combination) into a visible pixel
// range — picked by feel, not measured against anything.
const CROSSHAIR_CONTAINER_PX: f32 = 50.0;
const CROSSHAIR_CENTER_PX: f32 = CROSSHAIR_CONTAINER_PX / 2.0;
const CROSSHAIR_LINE_LENGTH_PX: f32 = 6.0;
const CROSSHAIR_LINE_THICKNESS_PX: f32 = 2.0;
const CROSSHAIR_BASE_GAP_PX: f32 = 4.0;
const CROSSHAIR_GAP_SPREAD_SCALE: f32 = 500.0;

// Right-click-to-aim: zooms the camera in (narrower FOV), tightens bullet
// spread, and slows movement — no gun-position/ironsight alignment, since
// that would need visually calibrating the viewmodel against the crosshair,
// which isn't possible without seeing the game rendered.
const BASE_FOV: f32 = std::f32::consts::PI / 4.0;
const AIM_FOV_MULTIPLIER: f32 = 0.6;
// How fast the FOV lerps toward its target each second (higher = snappier).
const AIM_ZOOM_SPEED: f32 = 12.0;
const AIM_SPREAD_MULTIPLIER: f32 = 0.35;
// Blurs the background while aiming (see apply_aim_dof) — the player's eye
// is focused on the scope, not the wider scene, so this sells that focus
// pull. Toggled on/off entirely via inserting/removing the component
// instead of always-on, since DoF isn't free.
// focal_distance is pinned close, near SCOPE_LENS_OFFSET's own distance
// from the camera (~0.2m), so the gun/scope viewmodel stays roughly in
// focus. That alone wasn't enough though — DepthOfField::default()'s
// aperture_f_stops (1.0, a very wide/shallow aperture) is so sensitive at
// close range that even a few cm of depth difference between the gun and
// the lens quad blurred one or the other anyway. Stopped down to f/11 to
// flatten that sensitivity — background blur will be much subtler as a
// result, but the near viewmodel stays reliably sharp.
// Two candidate focal distances, switchable live with KeyV (see
// AimDofFocalDistanceAlt) to A/B test which one keeps the gun/lens sharper
// without a rebuild. A (the default) matches SCOPE_LENS_OFFSET.z exactly
// and was confirmed sharpest in-game; B is the original looser guess, kept
// around as the alternate in case A ever needs re-checking against it.
const AIM_DOF_FOCAL_DISTANCE_A: f32 = 0.2;
const AIM_DOF_FOCAL_DISTANCE_B: f32 = 0.25;
const AIM_DOF_APERTURE_F_STOPS: f32 = 11.0;

// Dynamic resolution scaling: the main 3D camera renders into an offscreen
// texture sized at `window physical size * RenderResolutionScale`, instead
// of directly to the window — see the main camera's own setup() comment for
// why. RESOLUTION_SCALE_PRESETS drives the settings menu's buttons; the
// underlying texture supports any scale in (0, 1], these are just the ones
// exposed in the UI. MIN_TEXTURE_PX guards against a degenerate/zero-size
// render target on a tiny window at a low scale.
const RESOLUTION_SCALE_PRESETS: [f32; 7] = [0.2, 0.3, 0.4, 0.5, 0.65, 0.8, 1.0];
const RESOLUTION_SCALE_DEFAULT: f32 = 1.0;
const RESOLUTION_SCALE_MIN_TEXTURE_PX: u32 = 64;

// AK47 scope: a second camera renders the actual zoomed-in view to a small
// texture, which is displayed on a lens quad sitting in the scope's tube
// opening — a real optical zoom independent of the main camera/view FOV
// (unlike AIM_FOV_MULTIPLIER above, which just narrows the whole screen).
// Scroll wheel adjusts the zoom level while aiming.
const SCOPE_TEXTURE_SIZE: u32 = 512;
const SCOPE_MIN_ZOOM: f32 = 6.0;
const SCOPE_MAX_ZOOM: f32 = 25.0;
const SCOPE_DEFAULT_ZOOM: f32 = 8.0;
// Multiplicative, not additive — fov = SCOPE_BASE_FOV / zoom means a fixed
// +0.5-per-scroll step barely changes the FOV once zoom is already high
// (1/x flattens out), so zooming in felt like it kept slowing down. A fixed
// percentage step keeps the FOV change proportionally the same at every
// zoom level instead.
const SCOPE_ZOOM_STEP_FACTOR: f32 = 1.15;
const SCOPE_BASE_FOV: f32 = BASE_FOV;
// Guessed placement for the lens quad in the same camera-local space as
// GUN_ADS_OFFSET/MUZZLE_OFFSET (can't see the model to calibrate exactly) —
// closer to the camera than GUN_ADS_OFFSET.z so it renders in front of the
// gun mesh instead of getting clipped inside it.
const SCOPE_LENS_OFFSET: Vec3 = Vec3::new(0.0, 0.0, -0.2);
const SCOPE_LENS_SIZE: f32 = 0.03;
// How far the lens dips down below SCOPE_LENS_OFFSET.y while not aiming —
// X/Z stay pinned at SCOPE_LENS_OFFSET the whole time, so it only ever
// rises/drops straight up-down instead of sweeping in from the side.
const SCOPE_LENS_HIDDEN_DROP: f32 = 0.03;
// Raises the scope camera's eyepoint above the main camera's (the scope
// sits on top of the barrel, not at eye level) — offset along the camera's
// own local up so it stays correct regardless of pitch. See sync_scope_camera.
const SCOPE_CAMERA_HEIGHT_OFFSET: f32 = 0.0;
// While not aiming, the scope camera/lens sit slightly above center screen
// (like they're just resting up near the shoulder/eye line already) and
// ease straight down to dead center as ads_blend eases in — same decay
// shape as the X/Z sweep (full at ads_blend=0, zero at ads_blend=1).
const SCOPE_HIP_HEIGHT_OFFSET: f32 = 0.05;
// Active/rendered as soon as ADS starts blending in at all (so the lens is
// already visible and sliding into place during the transition, matching
// the gun mesh's own translation), hidden again once the gun raises for a
// wall. Not exactly 0.0 so it doesn't stay "active" forever chasing an
// asymptotic ease-toward-0 that never quite reaches it.
const SCOPE_ACTIVE_ADS_THRESHOLD: f32 = 0.02;
const SCOPE_HIDDEN_WALL_RAISE_THRESHOLD: f32 = 0.2;
// The gun viewmodel (and the scope lens quad) live on their own render
// layer, excluded from the scope camera's own view — otherwise the scope
// camera (sitting at the same position as the main camera) would render
// your own gun barrel floating in the middle of the zoomed-in texture. The
// main camera stays on both layers so the player still sees their own gun.
const WORLD_LAYER: usize = 0;
const VIEWMODEL_LAYER: usize = 1;
// A static red chevron reticle, PUBG-4x-style — baked directly into the
// scope camera's own view (its own render layer, invisible to the main
// camera) as a triangle sitting a fixed distance directly in front of it,
// so it's always dead center in the scope texture no matter where the
// player aims or how far they've zoomed.
const SCOPE_RETICLE_LAYER: usize = 2;
const SCOPE_RETICLE_DISTANCE: f32 = 0.3;
// Sized to look right at SCOPE_DEFAULT_ZOOM — the scope's FOV shrinks a lot
// between SCOPE_MIN_ZOOM and SCOPE_MAX_ZOOM (e.g. 18x), so a fixed-size mesh
// at a fixed distance would swing from tiny to covering the whole screen.
// update_scope_lens rescales the reticle's Transform.scale every frame by
// SCOPE_DEFAULT_ZOOM / current zoom to compensate, so it stays roughly
// constant on screen (like a real second-focal-plane reticle) instead of
// scaling with magnification.
const SCOPE_RETICLE_WIDTH: f32 = 0.001;
const SCOPE_RETICLE_HEIGHT: f32 = 0.002;
// Standing still (no WASD held) tightens spread further, stacking
// multiplicatively with AIM_SPREAD_MULTIPLIER — e.g. stationary + aiming
// ends up at STATIONARY_SPREAD_MULTIPLIER * AIM_SPREAD_MULTIPLIER of the
// base spread.
const STATIONARY_SPREAD_MULTIPLIER: f32 = 0.4;
// Stance/sprint also scale spread (see current_bullet_spread) — separate
// constants from the movement-speed ones (SPRINT_MULTIPLIER,
// CROUCH_SPEED_MULTIPLIER, PRONE_SPEED_MULTIPLIER) since "how fast" and
// "how accurate" don't have to move in lockstep.
const SPRINT_SPREAD_MULTIPLIER: f32 = 2.2;
const CROUCH_SPREAD_MULTIPLIER: f32 = 0.6;
const PRONE_SPREAD_MULTIPLIER: f32 = 0.2;

// Q/E lean: rolls the camera and shifts it sideways to peek around corners
// without moving the player capsule itself. No collision awareness (won't
// stop the peek at a wall) — a proper implementation would raycast sideways
// before applying the offset, skipped since it can't be visually verified
// here anyway.
const LEAN_SPEED: f32 = 6.0;
const LEAN_ROLL_DEG: f32 = 15.0;
const LEAN_SIDE_OFFSET: f32 = 0.4;

const EYE_HEIGHT_OFFSET: f32 = 0.65;

// Stance (PlayerStance, KeyC/KeyX) eye-height offsets — the movement-speed/
// capsule-length side of stance now lives in crates/server's move_players.
const CROUCH_EYE_HEIGHT_OFFSET: f32 = EYE_HEIGHT_OFFSET * 0.55;
const PRONE_EYE_HEIGHT_OFFSET: f32 = EYE_HEIGHT_OFFSET * 0.15;
// We don't know the real scale of this Sponza export (its own bounding box
// data was ambiguous — could be meters or centimeters), so the player spawns
// well above y=0 and lets gravity fall it onto whatever the real floor turns
// out to be, rather than guessing a spawn height that might be inside a wall.
const PLAYER_SPAWN: Vec3 = if USE_TEST_ARENA {
    Vec3::new(0.0, 3.0, ARENA_HALF_SIZE - 4.0)
} else {
    Vec3::new(0.0, 3.0, 4.0)
};

// How long a bullet-hole marker stays visible before despawning.
const HIT_MARKER_LIFETIME: f32 = 3.0;

const PLAYER_MAX_HEALTH: f32 = 100.0;
// Discrete bites instead of continuous per-second drain: a zombie within
// ZOMBIE_STOP_DISTANCE and chasing deals ZOMBIE_ATTACK_DAMAGE once every
// ZOMBIE_ATTACK_INTERVAL seconds (see ZombieAttackTimer/zombies_damage_player)
// — no separate attack animation, same simplicity tradeoff as the zombies'
// own one-shot-kill damage model.
const ZOMBIE_ATTACK_DAMAGE: f32 = 12.0;

// "Animated Zombie" by Quaternius (poly.pizza, CC-BY 3.0) —
// https://poly.pizza/m/jkrEvQZb8J — keep this attribution if shared/published.
// ZOMBIE_SCALE itself now lives in crates/shared, not here — see its own
// doc comment for why (the physics/hit-detection capsule is derived from
// it, so it can't be a client-only visual constant anymore).
const ZOMBIE_PATH: &str = "models/zombie.glb";
// Capsule center (the zombie's own Transform/NetTransform.translation) to
// feet. Applied to the mesh's child Transform, in world units (the child's
// parent — the zombie entity — is never scaled, only this child is), not
// the scaled mesh-local space.
const ZOMBIE_MODEL_Y_OFFSET: f32 = -(ZOMBIE_RADIUS + ZOMBIE_LENGTH / 2.0);

// "Soldier" by Quaternius (poly.pizza, CC-BY 3.0) —
// https://poly.pizza/m/oAArCNHjFB — keep this attribution if shared/published.
// Purely a debug visual for now (KeyT toggles it, see PlayerModelVisible) —
// third-person games would parent something like this to every player, but
// this project is first-person-only, so it's just there to eyeball whether
// the local player's own position/orientation tracking looks right without
// needing a second client. Unlike ZOMBIE_SCALE, this model's own node
// hierarchy already bakes in a correcting scale (exported at 100x, common
// for Blender-authored rigs), so it's spawned at Bevy's default scale (1.0)
// — not independently calibrated against anything in-game, so it may need
// visual tuning.
const PLAYER_MODEL_PATH: &str = "models/soldier.glb";

// Damage number popup: spawned at the hit point, drifts straight up in
// world space, and fades out over its lifetime.
const FLOATING_DAMAGE_LIFETIME: f32 = 0.9;
const FLOATING_DAMAGE_RISE_SPEED: f32 = 0.7;
const FLOATING_DAMAGE_FONT_SIZE: f32 = 14.0;
const FLOATING_DAMAGE_CRIT_FONT_SIZE: f32 = 24.0;

// ZOMBIE_RESPAWN_DELAY, ZOMBIE_MAX_HEALTH, HEAD/CHEST/BELLY/LIMB_DAMAGE,
// HITBOX_*, zombie_hit_damage_geometric, MAX_SHOT_DISTANCE now live in
// crates/shared — hit detection/damage is server-authoritative (see
// crates/server's handle_fire_shot). draw_zombie_hitboxes below still uses
// the HITBOX_* values (imported) purely for its debug visualization.
// ZOMBIE_AI_ENABLED, ZOMBIE_CHASE_RANGE, NAVMESH_*, ZOMBIE_STOP_DISTANCE,
// ZOMBIE_LOD_DISTANCE/REST_HEIGHT now live in crates/shared — movement/AI is
// server-authoritative now (see crates/server), but ZOMBIE_LOD_DISTANCE is
// still reused here client-side for update_zombie_lod's own (purely
// render-cost, not movement-cost) LOD decision.
// Full animation fidelity within this distance. Between here and
// ZOMBIE_LOD_DISTANCE (where freeze_zombie_animations pauses it outright),
// animation is instead throttled to ZOMBIE_ANIM_THROTTLE_HZ — still
// visibly moving, just choppier, at a fraction of the CPU-sample/GPU-skin
// cost of full 60fps animation. Per-zombie phase-shifted (via each one's
// own world X position) so a whole crowd doesn't visibly freeze/thaw in
// lockstep on the same frame.
const ZOMBIE_ANIM_FULL_DISTANCE: f32 = 8.0;
const ZOMBIE_ANIM_THROTTLE_HZ: f32 = 12.0;
// Global caps on animation tick rate based on the *total* live zombie
// count (not just how many are nearby) — past ZOMBIE_ANIM_CROWD_THRESHOLD_1
// every zombie is capped at ZOMBIE_ANIM_CROWD_HZ_1 (even ones within
// ZOMBIE_ANIM_FULL_DISTANCE, which would otherwise never throttle), each
// higher threshold dropping the cap further. Below
// ZOMBIE_ANIM_CROWD_THRESHOLD_1, no global cap applies — only the normal
// full/medium distance tiers.
const ZOMBIE_ANIM_CROWD_THRESHOLD_1: usize = 100;
const ZOMBIE_ANIM_CROWD_HZ_1: f32 = 12.0;
const ZOMBIE_ANIM_CROWD_THRESHOLD_2: usize = 200;
const ZOMBIE_ANIM_CROWD_HZ_2: f32 = 5.0;
const ZOMBIE_ANIM_CROWD_THRESHOLD_3: usize = 500;
const ZOMBIE_ANIM_CROWD_HZ_3: f32 = 2.0;

// ZOMBIE_AVOID_*/ZOMBIE_SEPARATION_* (obstacle avoidance/boids separation)
// now live in crates/shared and only crates/server's move_zombies uses them.

// Screen-space (not 3D billboard) health bar floating above each zombie —
// projected from its world position every frame via Camera::world_to_viewport.
// The zombie's own Transform.translation is roughly its physics capsule's
// center (avian3d capsule colliders are centered on the entity origin), not
// its feet — measured around y=0.8-0.9 for a resting capsule. For a ~2m-tall
// zombie (see ZOMBIE_SCALE) that puts the head roughly 1.1-1.3m above the
// root; lowered from an earlier 2.1 (visually confirmed too high — was
// floating a meter above the actual head).
const ZOMBIE_HEALTH_BAR_HEIGHT: f32 = 1.3;
const ZOMBIE_HEALTH_BAR_WIDTH_PX: f32 = 50.0;
const ZOMBIE_HEALTH_BAR_HEIGHT_PX: f32 = 6.0;
// Even among "near" (not motion.far) zombies, only the this-many closest to
// the player actually get their bar projected/laid out each frame — a
// crowd of hundreds all within LOD range would otherwise still pay for a
// bar each.
const ZOMBIE_HEALTH_BAR_MAX_VISIBLE: usize = 30;

// Animation clip order inside zombie.glb: 0=Bite, 1=Crawl, 2=Idle, 3=Run,
// 4=Walk (read from the file's own animations list). All 5 are wired up
// (see ZombiePose/update_zombie_animation) — previously only Idle/Walk
// were used, leaving Bite/Crawl/Run dead assets despite shipping in the
// model.
const ZOMBIE_ANIM_BITE: usize = 0;
const ZOMBIE_ANIM_CRAWL: usize = 1;
const ZOMBIE_ANIM_IDLE: usize = 2;
const ZOMBIE_ANIM_RUN: usize = 3;
const ZOMBIE_ANIM_WALK: usize = 4;
// Health fraction (of ZOMBIE_MAX_HEALTH) at/below which a zombie switches
// to the Crawl pose instead of Walk/Run — a "nearly dead, drag itself
// toward you" visual telegraph. Chosen loosely (no reference precedent —
// this state didn't exist before), aiming for "clearly wounded, not
// premature": one hit above the point ZOMBIE_MAX_HEALTH=8.0's head-shot
// one-shot-kill would apply anyway.
const ZOMBIE_CRAWL_HEALTH_FRACTION: f32 = 0.35;
// Distance (same units as ZOMBIE_STOP_DISTANCE=1.2/ZOMBIE_CHASE_RANGE=60)
// below which a moving zombie switches from Walk to Run — a "closing in
// for the kill" tension beat as it nears melee range, distinct from
// ZOMBIE_STOP_DISTANCE (which gates the Bite pose/actual damage tick, much
// closer). No reference precedent for this threshold either; picked as a
// noticeably-closer-than-chase-range midpoint.
const ZOMBIE_RUN_DISTANCE: f32 = 10.0;
// Animation clip order inside soldier.glb (24 clips total, read from the
// file's own animations list): 0=Death, 1=Gun_Shoot, 4=Idle, 5=Idle_Gun,
// 16=Run, 22=Walk (full list has many more not used here — kick/punch/
// sword/wave/etc. from the same generic asset pack).
// Idle_Gun instead of bare Idle — this game's player always holds a gun,
// so the weapon-drawn idle pose reads correctly instead of empty-handed.
const PLAYER_MODEL_ANIM_IDLE: usize = 5;
const PLAYER_MODEL_ANIM_WALK: usize = 22;
// Sprinting (NetTransform.sprinting, see its own doc comment for why this
// wasn't replicated before) — a "closing the distance" pose distinct from
// the ordinary Walk, mirroring ZombiePose's Run tier.
const PLAYER_MODEL_ANIM_RUN: usize = 16;
// One-shot poses, not part of the continuous idle/walk/run state — see
// PlayerFiredFx/PlayerDiedFx's own doc comments in crates/shared for why
// no equivalent signal existed before this pass.
const PLAYER_MODEL_ANIM_SHOOT: usize = 1;
const PLAYER_MODEL_ANIM_DEATH: usize = 0;

#[derive(Component)]
struct FpsCamera {
    yaw: f32,
    pitch: f32,
}

// Shared by shoot(), move_player_physics, and apply_aim_zoom so "aiming"
// means the same thing everywhere: cursor locked (actually playing, not
// paused/menu'd out) and right mouse button held — or, on touch, the
// TouchAimButton toggle (see touch_aim_button) is on. There's no touch
// equivalent of "cursor grab" gating this the way there is for mouse, since
// touch has no OS-level pointer lock concept — a touch player is just always
// considered "playing" for this check's purposes.
fn is_aiming(cursor: &CursorOptions, mouse: &ButtonInput<MouseButton>, touch_aiming: bool) -> bool {
    (cursor.grab_mode != CursorGrabMode::None && mouse.pressed(MouseButton::Right)) || touch_aiming
}

fn is_moving(keys: &ButtonInput<KeyCode>) -> bool {
    keys.pressed(KeyCode::KeyW)
        || keys.pressed(KeyCode::KeyA)
        || keys.pressed(KeyCode::KeyS)
        || keys.pressed(KeyCode::KeyD)
}

// Shared by shoot() (to actually offset the shot) and update_crosshair (to
// visually reflect it) so the reticle always shows the real spread the next
// shot would use — same recoil-heat/aim/stationary formula, just factored
// out so both stay in sync automatically instead of two copies drifting
// apart.
fn current_bullet_spread(pitch_kick: f32, aiming: bool, moving: bool, stance: PlayerStance, sprinting: bool) -> f32 {
    let heat = pitch_kick / RECOIL_PITCH_KICK_MAX;
    let mut spread = BULLET_SPREAD * (0.3 + heat);
    if aiming {
        spread *= AIM_SPREAD_MULTIPLIER;
    }
    if !moving {
        spread *= STATIONARY_SPREAD_MULTIPLIER;
    }
    // Sprinting is the least stable stance regardless of what PlayerStance
    // says (and move_player_physics only allows sprinting while Standing
    // anyway) — otherwise Prone is the most stable, Crouching in between.
    if sprinting {
        spread *= SPRINT_SPREAD_MULTIPLIER;
    } else {
        spread *= match stance {
            PlayerStance::Standing => 1.0,
            PlayerStance::Crouching => CROUCH_SPREAD_MULTIPLIER,
            PlayerStance::Prone => PRONE_SPREAD_MULTIPLIER,
        };
    }
    spread
}

#[derive(Component, Default)]
struct ViewRecoil {
    pitch_kick: f32,
    yaw_kick: f32,
}

// -1.0 (full lean left) .. 1.0 (full lean right), eased toward whichever of
// Q/E is held (or back to 0.0 if neither/both are) by apply_lean_input.
// apply_view_recoil turns this into camera roll, sync_camera_to_player turns
// it into a sideways offset.
#[derive(Component, Default)]
struct Lean {
    current: f32,
}

// Player, PlayerStance, Health, GameOver now live in crates/shared (plain
// gameplay data needed by both the future server and the client).

// Marks the "GAME OVER" overlay so restart_game can find and despawn it.
#[derive(Component)]
struct GameOverUi;

#[derive(Component)]
struct RestartButton;

// Debug/dev flight mode: toggled by KeyN. Sent to the server as part of
// ClientInput (see its own doc comment) and simulated by the shared
// simulate_player_step on both sides — bypasses MoveAndSlide (and thus
// gravity/collision) entirely, so the player can fly through walls to
// inspect the level. Not reset by restart_game (deliberately — no reason to
// drop out of noclip just because the player died).
#[derive(Resource, Default)]
struct Noclip(bool);

// Debug/dev: toggled by KeyF. Freezes zombies in place (move_zombies and
// zombies_damage_player both early-return) so hitbox regions can be tested
// against a stationary target instead of a moving one.
#[derive(Resource, Default)]
struct FreezeZombies(bool);

// Debug/dev: toggled by KeyH. Draws the height-band/lateral-distance regions
// zombie_hit_damage uses to pick head/chest/belly/limb, via draw_zombie_hitboxes.
#[derive(Resource, Default)]
struct ShowHitboxes(bool);

// Debug/dev: toggled by KeyG. Player takes no damage while true — see
// zombies_damage_player.
#[derive(Resource, Default)]
struct GodMode(bool);

// Debug/dev: toggled by KeyP. Shows debug_panel_actions' hotkeys plus
// current state of every other debug toggle, via update_debug_panel.
#[derive(Resource, Default)]
struct DebugPanelVisible(bool);

// User-facing (not a debug toggle): scales the main camera's offscreen
// render target — see RESOLUTION_SCALE_PRESETS' own comment. 1.0 = native
// resolution. resize_scene_render_target reads this every frame and resizes
// SceneRenderTarget's Image whenever it (or the window) changes.
#[derive(Resource)]
struct RenderResolutionScale(f32);

impl Default for RenderResolutionScale {
    fn default() -> Self {
        Self(RESOLUTION_SCALE_DEFAULT)
    }
}

// The Image the main 3D camera renders into instead of the window directly
// — see the camera's own setup() comment. Held as a resource (not just a
// local in setup()) so resize_scene_render_target can reach it later.
#[derive(Resource)]
struct SceneRenderTarget(Handle<Image>);

// Toggled by F1 or tapping SettingsToggleButton (desktop key + touch button,
// since this is a real user-facing menu, unlike the debug panel above which
// is keyboard-only). Drives the settings panel's Node.display in
// update_settings_menu, same show/hide pattern as DebugPanelVisible.
#[derive(Resource, Default)]
struct SettingsMenuVisible(bool);

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
struct MouseUiLocked(bool);

// Which touch (if any) is currently dragging the settings menu's scroll
// area — touch_scroll_settings_menu's own equivalent of TouchMoveState/
// TouchLookState for the joystick/look-drag. scroll_settings_menu (desktop
// mouse wheel) already exists; this is the touch counterpart, since the
// menu's own Overflow::scroll_y() + ScrollPosition only gets driven by a
// wheel today — nothing drags it by finger on a touch-only device.
#[derive(Resource, Default)]
struct SettingsMenuScrollState {
    touch_id: Option<u64>,
    last_pos_y: f32,
}

#[derive(Component)]
struct SettingsMenuRoot;

// Which page of the settings menu is currently shown — the menu still
// scrolls too (see scroll_settings_menu/touch_scroll_settings_menu), but
// paging avoids ever needing to on a screen this small.
#[derive(Resource, Default)]
struct SettingsMenuPageIndex(usize);

// How many SettingsMenuPage(N) containers spawn_settings_menu creates —
// settings_menu_page_buttons wraps SettingsMenuPageIndex against this so
// it never points past the last real page.
const SETTINGS_MENU_PAGE_COUNT: usize = 2;

// Marks each page container; update_settings_menu_pages compares this to
// SettingsMenuPageIndex to decide that page's Node.display. A child of
// SettingsMenuRoot, so it inherits the root's GlobalZIndex/stacking rather
// than needing its own.
#[derive(Component)]
struct SettingsMenuPage(usize);

#[derive(Component)]
struct SettingsMenuPrevButton;
#[derive(Component)]
struct SettingsMenuNextButton;
#[derive(Component)]
struct SettingsMenuPageLabel;

// One per RESOLUTION_SCALE_PRESETS entry — resolution_scale_buttons reads
// Interaction on these to know which preset was tapped/clicked.
#[derive(Component)]
struct ResolutionScaleButton(f32);

#[derive(Component)]
struct SettingsToggleButton;

// Marks the small "UI locked (Tab)" HUD text — update_mouse_ui_lock_indicator
// shows/hides it via Node.display in step with MouseUiLocked.
#[derive(Component)]
struct MouseUiLockIndicator;

// Debug/dev: toggled by KeyB. Lets apply_aim_dof's background blur be
// switched off quickly to check whether DoF is the cause of some visual
// issue, without having to comment the system out and rebuild.
#[derive(Resource)]
struct AimBlurEnabled(bool);

impl Default for AimBlurEnabled {
    fn default() -> Self {
        Self(true)
    }
}

// Debug/dev: toggled by KeyV. false = AIM_DOF_FOCAL_DISTANCE_A, true =
// AIM_DOF_FOCAL_DISTANCE_B — see apply_aim_dof.
#[derive(Resource, Default)]
struct AimDofFocalDistanceAlt(bool);

// Debug/dev: toggled by KeyM. Forces every zombie's animation paused
// (CPU sample + GPU skinning both skipped) regardless of distance/crowd
// tier — for isolating how much of the frame cost is actually animation,
// separate from the automatic LOD/crowd throttling in
// freeze_zombie_animations.
#[derive(Resource, Default)]
struct ZombieAnimDisabled(bool);

// Debug/dev: toggled by KeyJ. Hides every zombie health bar outright,
// skipping update_zombie_health_bars' screen-space projection/UI layout
// work entirely instead of just capping how many are shown.
#[derive(Resource, Default)]
struct ZombieHealthBarsHidden(bool);

// Debug/dev: toggled by KeyK. Forces every zombie onto the full-fidelity
// path in move_zombies (navmesh pathfind + raycast steering + MoveAndSlide
// collision), disabling the far-distance LOD shortcut entirely — useful to
// verify collision/pathing behavior for zombies that would normally be
// past ZOMBIE_LOD_DISTANCE.
#[derive(Resource, Default)]
struct ZombieLodDisabled(bool);

// Debug/dev: toggled by KeyT. Shows/hides the local player's own third-person
// PLAYER_MODEL_PATH visual — see its own doc comment for why this exists.
#[derive(Resource, Default)]
struct PlayerModelVisible(bool);

// Marks the standalone entity spawned for PLAYER_MODEL_PATH — see
// sync_player_model.
#[derive(Component)]
struct PlayerModel;

// Debug/dev toggles for the Cheats panel's "Gun"/"MSAA"/"Shadow" row — hide
// the viewmodel gun, or A/B the map's render cost the same way the
// CADisplayLink probe's own MSAA/SHADOW buttons do, just wired to the real
// game's own camera/sun instead of the probe's.
#[derive(Resource)]
struct GunVisible(bool);
impl Default for GunVisible {
    fn default() -> Self {
        Self(true)
    }
}

// Starts matching the main camera's own literal Msaa::Off at spawn (see
// setup()) — kept as a separate resource rather than reading the Msaa
// component back every frame so the toggle button's own color logic
// doesn't need a camera query.
#[derive(Resource, Default)]
struct MapMsaaOn(bool);

// Starts matching the sun DirectionalLight's own shadows_enabled: true at
// spawn (see setup()) — same reasoning as MapMsaaOn.
#[derive(Resource)]
struct MapShadowsOn(bool);
impl Default for MapShadowsOn {
    fn default() -> Self {
        Self(true)
    }
}

// Marks the main game's sun DirectionalLight (setup()'s own, not the
// per-hall accent lights from spawn_hall_lights) so apply_map_render_toggles
// can find it unambiguously.
#[derive(Component)]
struct MainSun;

// Marks any PLAYER_MODEL_PATH scene root that should animate — both the
// local debug PlayerModel (above) and the child spawned for every other
// NetPlayer in on_net_player_replicated. wire_player_model_animations walks
// up from a newly-spawned AnimationPlayer looking for this, same pattern
// wire_zombie_animations uses for Zombie.
#[derive(Component)]
struct PlayerModelAnimRoot;

// Links a PlayerModelAnimRoot entity to the AnimationPlayer entity somewhere
// in its own scene subtree (glTF scenes put the AnimationPlayer on a child,
// not the root) — mirrors ZombieAnimationPlayer.
#[derive(Component)]
struct PlayerModelAnimationPlayer(Entity);

// Was a plain bool (Walk vs Idle only) before Run/Shoot/Death got wired up
// — same shape/reasoning as ZombiePose. Shoot/Death aren't part of the
// continuous state this tracks (they're one-shot, played directly by
// on_player_fired_fx/on_player_died_fx without going through this
// enum/comparison at all — see those systems), so this only ever holds
// Idle/Walk/Run.
#[derive(Component, Default, Clone, Copy, PartialEq, Eq)]
enum PlayerModelPose {
    #[default]
    Idle,
    Walk,
    Run,
}

// Set by on_player_died_fx once that player's Death pose starts playing —
// excluded from update_remote_player_model_animation's own query so the
// next ordinary Idle/Walk/Run transition (which will keep arriving; the
// dead player's own NetTransform doesn't stop replicating) doesn't cut the
// Death animation short and snap back to Idle.
#[derive(Component)]
struct PlayerModelDead;

// Only the LOCAL player's own model reads this — see send_player_input,
// which is the one place that already knows this frame's move_axis before
// any network round trip, so the local debug model doesn't have to wait on
// its own replicated NetTransform.moving the way remote models do.
#[derive(Resource, Default)]
struct LocalPlayerMoving(bool);

// Same "this client's own best guess, no round trip needed" reasoning as
// LocalPlayerMoving — mirrors NetTransform.sprinting for the local debug
// model's own Run pose.
#[derive(Resource, Default)]
struct LocalPlayerSprinting(bool);

// Which debug bot (see shared::DebugBot) the C/[/]/X/O debug panel keybinds
// currently act on — client-only, never sent over the network.
#[derive(Resource, Default)]
struct SelectedDebugBot(Option<u32>);

// Rough per-system CPU timings (wall-clock ms this frame), shown in the
// debug panel so it's obvious which system is actually eating frame time as
// the zombie count climbs, instead of guessing — measured with a plain
// std::time::Instant around each system's own hot loop rather than pulling
// in a real profiler (tracy/puffin) for what's meant to be a quick check.
#[derive(Resource, Default)]
struct PerfTimings {
    move_zombies_ms: f32,
    health_bars_ms: f32,
    zombie_count: usize,
    // How many zombies this frame were within ZOMBIE_LOD_DISTANCE and paid
    // for the full navmesh+steering+MoveAndSlide path, vs the cheap
    // straight-line fallback beyond it.
    near_zombie_count: usize,
}

// This frame's raw mouse-look delta (set unconditionally every frame by
// mouse_look, including to Vec2::ZERO when there's no motion or the cursor
// isn't grabbed) — gun_view_animation reads it to drive the inertia lean
// (see Gun.look_sway_roll) without mouse_look needing to know anything
// about the gun.
#[derive(Resource, Default)]
struct MouseLookDelta(Vec2);

// Marks the debug panel's text node so update_debug_panel can find it.
#[derive(Component)]
struct DebugPanelText;

// Marks the always-visible health readout so update_health_ui can find it.
#[derive(Component)]
struct HealthUi;

// Screen-space health bar tracking a specific zombie — update_zombie_health_bars
// repositions it every frame and despawns it once the tracked zombie is gone.
#[derive(Component)]
struct ZombieHealthBar {
    zombie: Entity,
}

// The colored fill inside a ZombieHealthBar, whose width update_zombie_health_bars
// scales to the tracked zombie's remaining health fraction.
#[derive(Component)]
struct ZombieHealthBarFill;

// The "n.n/max" text above a ZombieHealthBar, kept in sync by
// update_zombie_health_bars alongside the fill's width.
#[derive(Component)]
struct ZombieHealthBarText;

// A damage number that pops up at the hit point and floats/fades out.
// Tracks its own world position (drifting upward each frame) rather than a
// zombie Entity, since it should keep floating even if the zombie that took
// the hit dies and despawns a moment later.
#[derive(Component)]
struct FloatingDamageText {
    world_pos: Vec3,
    remaining: f32,
}

#[derive(Component)]
struct Gun {
    base_offset: Vec3,
    recoil: f32,
    ammo: u32,
    // 0.0 = not reloading. Ticks down from RELOAD_DURATION.
    reload_remaining: f32,
    // 0.0 = hip-fire pose (base_offset), 1.0 = fully raised to GUN_ADS_OFFSET.
    // Eases toward whichever is_aiming() currently says, in gun_view_animation.
    ads_blend: f32,
    // 0.0 = normal height, 1.0 = fully raised (WALL_GUN_RAISE_HEIGHT) to
    // clear a wall in front. Eased toward the camera's forward-wall-distance
    // probe in gun_view_animation.
    wall_raise: f32,
    // 0.0 = right shoulder (normal), 1.0 = fully swapped to the left
    // shoulder. Eased directly from the raw lean-left key, independently of
    // Lean.current — its target only ever toggles between 0 and 1 (never a
    // ±1 range), so a reversal never sees a doubled distance/speed the way
    // driving it straight off Lean.current did.
    shoulder_blend: f32,
    // Random -1.0..1.0 left/right lean set fresh each shot (see shoot()),
    // scaled by RECOIL_ROLL_DEG and recoil_t in gun_view_animation — purely
    // cosmetic, same as RECOIL_TILT_UP_DEG, so the visual kick doesn't
    // always snap along dead-straight vertical line.
    recoil_roll: f32,
    // Walk-bob: bob_phase only advances while moving (see is_moving), and
    // bob_blend eases 0..1 toward "currently moving" so the bob fades in/out
    // smoothly instead of snapping to zero the instant WASD is released.
    bob_phase: f32,
    bob_blend: f32,
    // Inertia-style lean: eases toward a target proportional to this
    // frame's mouse-look yaw delta (negated — turning right should make the
    // gun's rear lag/lean left, PUBG-style), then springs back to 0 once
    // the mouse stops. See LOOK_SWAY_* constants.
    look_sway_roll: f32,
}

// The second camera that renders the scope's actual zoomed view onto
// SCOPE_TEXTURE_SIZE² render target, sampled by ScopeLens's material.
// Starts inactive — update_scope_lens only turns it on while ADS'd, so it
// isn't paying for a second render pass the rest of the time.
#[derive(Component)]
struct ScopeCamera;

// The small quad in front of the gun showing the scope camera's render
// target — the thing the player actually looks "through".
#[derive(Component)]
struct ScopeLens;

// Marks the static reticle chevron so shoot()'s raycast can exclude it —
// see VfxAssets.vfx_meshes.
#[derive(Component)]
struct ScopeReticle;

// Current scope magnification, adjusted by mouse wheel while aiming (see
// adjust_scope_zoom) and clamped to [SCOPE_MIN_ZOOM, SCOPE_MAX_ZOOM].
#[derive(Resource)]
struct ScopeZoom(f32);

// Marks the ammo counter text so update_ammo_ui can find it.
#[derive(Component)]
struct AmmoUi;

#[derive(Component)]
struct HitMarker {
    remaining: f32,
}

// Tags every render-mesh descendant of the Gun entity (see wire_gun_mesh),
// so shoot()'s raycast can exclude the viewmodel itself. Needed once ADS
// pulled the gun in close to and centered on the camera (GUN_ADS_OFFSET) —
// it now sits directly in the forward raycast's path, where GUN_OFFSET's
// off-center hip-fire position never did.
#[derive(Component)]
struct GunMesh;

#[derive(Component)]
struct MuzzleFlash {
    remaining: f32,
}

#[derive(Component)]
struct Smoke {
    velocity: Vec3,
    remaining: f32,
}

// A single flying blood droplet — spurts outward from a zombie hit, falls
// under its own gravity (separate from GRAVITY, blood falls faster/looks
// punchier), and despawns once its lifetime runs out.
#[derive(Component)]
struct BloodParticle {
    velocity: Vec3,
    remaining: f32,
}

// A single flying spark — the environment-hit counterpart of BloodParticle,
// see spawn_spark_effects.
#[derive(Component)]
struct SparkParticle {
    velocity: Vec3,
    remaining: f32,
}

// A static decal left where blood actually landed — either right at the
// hit point, or on a floor found by raycasting straight down from there.
#[derive(Component)]
struct BloodStain {
    remaining: f32,
}

// Zombie, ZombieMotion, KnockbackVelocity, ZombieAttackTimer, Chase now live
// in crates/shared (plain gameplay data needed by both the future server
// and the client).

// GameLayer, NearPlayer/near_player_scorer, ChaseAction/chase_action_system
// (big-brain Thinker AI) now live in crates/server — zombie AI/movement is
// server-authoritative.

// Points a zombie's root entity at the specific descendant entity that
// carries its AnimationPlayer, recorded once wire_zombie_animations finds it
// (the glTF scene loads asynchronously, so it doesn't exist yet at spawn time).
#[derive(Component)]
struct ZombieAnimationPlayer(Entity);

// Tracks which clip update_zombie_animation last picked, so it only issues
// a play() when the computed pose actually changes instead of every frame
// (NetTransform.translation/Health both change nearly every frame
// regardless). Was a plain bool (Walk vs Idle only) before all 5 of
// zombie.glb's clips got wired up — see update_zombie_animation's own doc
// comment for the priority order these variants are picked in.
#[derive(Component, Default, Clone, Copy, PartialEq, Eq)]
enum ZombiePose {
    #[default]
    Idle,
    Walk,
    Run,
    Crawl,
    Bite,
}

// --- Real navmesh (vleue_navigator, vendored fork) ---
//
// Auto-generating obstacles from Sponza's own Collider (NavmeshUpdaterPlugin
// slicing it at a fixed height) was tried and abandoned: that collider is a
// single merged trimesh for the whole building (~262k triangles across 103
// primitives), and slicing/triangulating it timed out even at 60s — shrinking
// the navmesh's outer bounds didn't help, since the slicing step runs over
// the full collider before bounds are ever applied. See spawn_zombie_navmesh
// for the hand-authored replacement.

// Fixed debug server address — matches crates/server's PORT. Override with
// SERVER_ADDR (e.g. "192.168.1.5:5031") to point at another machine on the
// LAN; the common case is just starting both binaries on localhost.
const PROTOCOL_ID: u64 = 0;

fn connect_to_server(mut commands: Commands, channels: Res<RepliconChannels>) {
    spawn_client_connection(&mut commands, &channels);
}

// How often auto_reconnect retries after losing the connection — mainly a
// dev-workflow thing: lets the server be killed and restarted (e.g. after
// rebuilding it) without also having to restart the client every time.
const RECONNECT_INTERVAL: f32 = 2.0;

// Runs every frame; only actually does anything once ClientState has been
// Disconnected for RECONNECT_INTERVAL seconds straight (the timer resets
// otherwise, including while Connecting/Connected). The stale RenetClient/
// NetcodeClientTransport left over from the dropped connection is removed
// first — resource_added::<RenetClient> (see bevy_replicon_renet2's own
// set_connecting) is what drives the Connecting state transition, and that
// only fires on a genuine remove-then-insert, not an insert over an
// already-present resource.
fn auto_reconnect(
    mut commands: Commands,
    time: Res<Time>,
    state: Res<State<ClientState>>,
    channels: Res<RepliconChannels>,
    mut disconnected_for: Local<f32>,
) {
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
fn despawn_replicated_on_disconnect(mut commands: Commands, entities: Query<Entity, Or<(With<Zombie>, With<NetPlayer>)>>) {
    for entity in &entities {
        commands.entity(entity).despawn();
    }
}

fn spawn_client_connection(commands: &mut Commands, channels: &RepliconChannels) {
    // to_socket_addrs (not parse::<SocketAddr>) so this also accepts a DNS
    // hostname, not just a literal IP — needed for e.g. playit.gg's
    // assigned tunnel domain (foo.tun.ply.gg:PORT), which a plain
    // SocketAddr parse rejects outright since it never resolves DNS.
    // TEMPORARY — default flipped to this Mac's Tailscale IP for an
    // on-device test build (icon-tap launches can't pick up an env var
    // override, same reasoning as minimal_map_probe_main's own forced-true
    // toggle). Only reachable while the test phone is on the same Tailscale
    // network and `cargo run -p server` is running locally. Revert to
    // "127.0.0.1:5031" once this test is done.
    let server_addr_str = std::env::var("SERVER_ADDR").unwrap_or_else(|_| "100.97.13.68:5031".to_string());
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
struct MyClientId(u64);

// Client-only marker (never sent over the network) for whichever replicated
// NetPlayer entity turned out to be this client's own — see MyClientId.
#[derive(Component)]
struct LocalPlayer;

fn mark_local_player(
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
fn on_net_player_replicated(
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
fn mirror_local_player_net_transform(
    mut players: Query<(&NetTransform, &mut Transform), (Changed<NetTransform>, With<LocalPlayer>)>,
) {
    for (net_transform, mut transform) in &mut players {
        transform.translation = net_transform.translation;
        transform.rotation = Quat::from_rotation_y(net_transform.yaw);
    }
}

// INTERP_DELAY_SECS now lives in crates/shared — crates/server's
// handle_fire_shot needs the same value for lag compensation (rewinding to
// roughly what the shooter's screen actually showed = network latency +
// this render delay).
// Snapshots older than this relative to the newest one are dropped —
// keeps the buffer from growing unbounded if an entity stops replicating
// (e.g. a zombie the LOD/crowd-hz throttle rarely updates) without needing
// a fixed-size ring buffer.
const INTERP_BUFFER_MAX_AGE_SECS: f32 = 0.5;

// Timestamped position/yaw history for interpolate_net_transform — every
// NetTransform-carrying entity gets one (see on_net_transform_replicated),
// including the local player's, but mirror_local_player_net_transform
// bypasses it for that one (Without<LocalPlayer> everywhere below).
#[derive(Component, Default)]
struct NetTransformBuffer {
    snapshots: std::collections::VecDeque<(f32, Vec3, f32)>,
}

fn on_net_transform_replicated(trigger: On<Add, NetTransform>, mut commands: Commands) {
    commands.entity(trigger.event().entity).insert(NetTransformBuffer::default());
}

// Appends a timestamped snapshot every time replication actually updates
// NetTransform — this is the "actually received" data interpolate_net_transform
// draws its two bracketing points from.
fn buffer_net_transform(
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
fn interpolate_net_transform(time: Res<Time>, mut entities: Query<(&NetTransformBuffer, &mut Transform), Without<LocalPlayer>>) {
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

// Client-side prediction + reconciliation for the client's pre-existing
// local Player entity (see setup()) — still what zombie AI/UI/damage all
// target, so this remains the one link back to it. Every frame: start from
// the server's last confirmed position (our own replicated NetPlayer's
// NetTransform), drop whichever buffered inputs the server has now
// confirmed (LastProcessedInput), then replay everything still
// unconfirmed through the exact same movement step the server itself
// runs (shared::movement::simulate_player_step) — including the input
// send_player_input just queued this frame, so movement is instant instead
// of waiting a full round trip.
//
// Known simplification: only translation is resynced from the server;
// vertical_velocity/grounded keep evolving from whatever this client last
// had locally rather than also being replicated and corrected. Good enough
// for ground movement (which is what player-vs-player visible drift would
// mostly show up as); a mispredicted jump/fall can drift slightly from the
// server's version until landing resets grounded.
// Reconciliation: only runs when LastProcessedInput actually advances (the
// server confirming a new input), not every render frame. Popping confirmed
// entries recovers the exact vertical_velocity/grounded the local
// simulation had right after applying the last-confirmed input (stashed in
// PredictedInputs by send_player_input) — resuming from that exact state,
// rather than whatever was left over from a previous reconciliation pass,
// is what avoids double-integrating gravity into vertical_velocity every
// frame. That old per-frame reset-and-replay-everything approach is exactly
// what caused the jump stutter this replaced: pending inputs stayed queued
// for several frames (RTT-sized), and each of those frames re-simulated
// the whole queue on top of already-advanced velocity instead of a
// consistent baseline, so gravity kept compounding until it visibly
// snapped/jittered.
fn predict_player_movement(
    local: Query<
        (&NetTransform, &LastProcessedInput),
        (With<NetPlayer>, With<LocalPlayer>, Without<Player>, Changed<LastProcessedInput>),
    >,
    mut player: Query<(&mut Transform, &mut Player, &PlayerStance), Without<NetPlayer>>,
    mut pending: ResMut<PredictedInputs>,
    mover: MoveAndSlide,
) {
    let Ok((net_transform, last_processed)) = local.single() else {
        return;
    };
    let Ok((mut transform, mut player, stance)) = player.single_mut() else {
        return;
    };

    let mut confirmed_baseline = None;
    while pending.0.front().is_some_and(|&(sequence, ..)| sequence <= last_processed.0) {
        let (_, _, _, vertical_velocity, grounded) = pending.0.pop_front().unwrap();
        confirmed_baseline = Some((vertical_velocity, grounded));
    }
    if let Some((vertical_velocity, grounded)) = confirmed_baseline {
        player.vertical_velocity = vertical_velocity;
        player.grounded = grounded;
    }

    transform.translation = net_transform.translation;
    for (_, input, dt, ..) in &pending.0 {
        shared::movement::simulate_player_step(&mover, &mut transform, &mut player, *stance, input, *dt);
    }
}

fn log_client_connection_state(state: Res<State<ClientState>>) {
    if state.is_changed() {
        info!("client connection state: {:?}", state.get());
    }
}

// #[bevy_main] requires the fn to be named exactly `main` — see its own
// macro's compile-error message. Desktop's src/main.rs (a thin `fn main() {
// client::main(); }`) and iOS's start_app extern shim below both call this
// same one, so there's exactly one place the App is actually built,
// regardless of platform.
#[bevy_main]
pub fn main() {
    let mut app = App::new();
    app.add_plugins((
            // Desktop: WindowResolution is left at Bevy's default (a plain
            // resizable 1280x720 window) — real render-cost scaling is done
            // independently via RenderResolutionScale's offscreen render
            // target (see setup()), not by shrinking the window itself.
            //
            // iOS/Android: WindowMode explicitly forced to
            // BorderlessFullscreen instead of leaving it at the same
            // default. Without this, winit still requests that same fixed
            // 1280x720-logical window on a phone — there's no "maximize" or
            // user-resize concept on mobile, so the OS just gives the app a
            // literal 1280x720-sized surface positioned somewhere on the
            // real screen instead of the actual full display bounds. On a
            // landscape-locked device this showed up as the game only
            // filling one corner of the screen after rotating (reported
            // in-game) — this is what actually caused that, not the
            // orientation lock or the missing LaunchScreen storyboard
            // (both real bugs, both already fixed, but neither was this).
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        mode: if cfg!(target_os = "ios") || cfg!(target_os = "android") {
                            WindowMode::BorderlessFullscreen(MonitorSelection::Current)
                        } else {
                            WindowMode::Windowed
                        },
                        ..default()
                    }),
                    ..default()
                })
                // Bevy's default TaskPoolOptions sizes its thread pools off
                // std::thread::available_parallelism(), which counts EVERY
                // logical core — on an iPhone that includes the slow
                // efficiency cores alongside the 2 fast performance cores,
                // unlike this dev Mac where all cores are genuinely fast.
                // Instruments' Time Profiler (Invert Call Tree + Hide
                // System Libraries) on-device showed >30% of CPU self-time
                // going to objc::rc::autorelease::autoreleasepool churn plus
                // parking_lot/std::sync::Mutex lock contention — both
                // classic symptoms of more worker threads than the hardware
                // can actually run in parallel, fighting over the same
                // locks instead of doing real work. Capped to 4 total
                // threads on iOS/Android as an experiment; desktop keeps
                // Bevy's normal auto-sized default (10 real cores on this
                // Mac, no contention observed there).
                .set(TaskPoolPlugin {
                    task_pool_options: if cfg!(target_os = "ios") || cfg!(target_os = "android") {
                        TaskPoolOptions::with_num_threads(4)
                    } else {
                        TaskPoolOptions::default()
                    },
                }),
            MeshPickingPlugin,
            PhysicsPlugins::default(),
            // A lightweight always-on FPS overlay. Bevy 0.18 doesn't have the
            // draggable DiagnosticsOverlay panels (that's 0.19-only), so this
            // is the only diagnostics UI available here. Default
            // refresh_interval (100ms, 10x/sec) reads as a flicker rather
            // than a number — slowed to 30x/sec (see PING_UI_REFRESH_SECS
            // below, kept at the same rate for consistency).
            // FpsOverlayPlugin {
            //     config: bevy::dev_tools::fps_overlay::FpsOverlayConfig {
            //         refresh_interval: std::time::Duration::from_secs_f32(1.0 / 60.0),
            //         ..default()
            //     },
            // },
            RepliconPlugins,
            RepliconRenetPlugins,
        ))
        // WinitSettings::game()'s default focused_mode is UpdateMode::Continuous
        // ("update as fast as possible, rely on VSync to actually cap it") —
        // a well-documented Bevy-on-iOS problem (bevyengine/bevy#16734: an
        // EMPTY scene still burns ~50% CPU and runs under 60fps on iOS) is
        // that iOS's winit backend doesn't reliably throttle Continuous mode
        // via VSync the way desktop does, so the main loop just spins,
        // repeatedly logging "processing non RedrawRequested event after the
        // main event loop: AboutToWait" — the exact message seen in this
        // project's own on-device logs. Forcing an explicit Reactive wait
        // (the fix Bevy's own official mobile example adopted after hitting
        // this) caps the update rate directly in winit's event loop,
        // independent of whatever Metal/CAMetalLayer vsync is or isn't
        // doing. iOS/Android only — desktop's Continuous default is fine
        // here (no equivalent throttling bug observed on macOS).
        //
        // 120Hz, not 60 — this is just the *ceiling* the app is willing to
        // update at, not a request that overrides the display's own
        // capability; a 60Hz-only screen (or a non-ProMotion iPhone) simply
        // never hits this ceiling and behaves the same as before. Getting
        // an actual 120Hz-capable device to render above 60Hz also requires
        // CADisableMinimumFrameDurationOnPhone in Info.plist (iOS caps
        // every app to 60Hz by default regardless of anything set here,
        // unless that key opts out) — see project.yml's own comment on it.
        .insert_resource(if cfg!(target_os = "ios") || cfg!(target_os = "android") {
            WinitSettings {
                focused_mode: UpdateMode::reactive(std::time::Duration::from_secs_f32(1.0 / 120.0)),
                ..WinitSettings::game()
            }
        } else {
            WinitSettings::game()
        });
    build_game_common(&mut app);
    app.run();
}

// Everything the real game needs that has nothing to do with which
// windowing backend is driving it (winit vs. the experimental
// CADisplayLink FFI entry point in cadisplaylink::ffi) — mirrors
// build_probe_common's own reasoning. DefaultPlugins/WindowPlugin/
// TaskPoolPlugin/WinitSettings stay in each entry point's own setup since
// they genuinely differ (WinitPlugin disabled + no WinitSettings on the
// CADisplayLink path), everything else is identical.
fn build_game_common(app: &mut App) {
    app.replicate::<NetworkId>()
        .replicate::<NetPlayer>()
        .replicate::<NetTransform>()
        .replicate::<Zombie>()
        .replicate::<Health>()
        .replicate::<LastProcessedInput>()
        .replicate::<DebugBot>()
        .add_client_event::<ClientInput>(Channel::Unreliable)
        .add_client_event::<SpawnMoreZombies>(Channel::Unordered)
        .add_client_event::<FireShot>(Channel::Ordered)
        .add_server_event::<ShotFx>(Channel::Ordered)
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
        .init_resource::<ZombieCountUi>()
        .init_resource::<AutoMaintainUi>()
        // Restored alongside the Update-schedule block above — this was
        // disabled in the same CPU-investigation commit (80de58d), not
        // pre-existing/intentional; Phase 6 needs real networking/
        // replication actually wired up for anything to spawn.
        .add_systems(Startup, connect_to_server)
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
        .add_observer(on_net_transform_replicated)
        .add_observer(on_net_player_replicated)
        .add_observer(on_shot_fx)
        .add_observer(on_player_fired_fx)
        .add_observer(on_player_died_fx)
        .add_observer(on_game_state_event)
        .init_resource::<Noclip>()
        .init_resource::<Autopilot>()
        .init_resource::<AutopilotState>()
        .init_resource::<AutopilotIntent>()
        .init_resource::<FreezeZombies>()
        .init_resource::<ShowHitboxes>()
        .init_resource::<GodMode>()
        .init_resource::<DebugPanelVisible>()
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
        .init_resource::<PlayerModelVisible>()
        .init_resource::<GunVisible>()
        .init_resource::<MapMsaaOn>()
        .init_resource::<MapShadowsOn>()
        .init_resource::<SelectedDebugBot>()
        .init_resource::<LocalPlayerMoving>()
.init_resource::<LocalPlayerSprinting>()
        .init_resource::<TouchControlsEnabled>()
        .init_resource::<TouchMoveState>()
        .init_resource::<TouchMoveAxis>()
        .init_resource::<TouchLookState>()
        .init_resource::<TouchFiring>()
        .init_resource::<TouchAiming>()
        .init_resource::<TouchJump>()
        .init_resource::<NoclipFlyInput>()
        .init_resource::<TouchReload>()
        .init_resource::<SensitivityScale>()
        .init_resource::<KillCount>()
        .init_resource::<RecentlyHitZombies>()
        .add_systems(Startup, (setup /*, log_window_resolution */,));
    // CPU bisection (temporary, not deleted): the ENTIRE Update schedule
    // below is commented out — rock-bottom baseline with only whatever
    // Bevy/wgpu itself does every frame (extraction/prepare/render graph)
    // plus setup()'s one-time Startup spawn (Sponza, lights, gun/zombie
    // models, all UI). No per-frame game logic at all. Compare this
    // Metal HUD reading against the full game to see how much of the CPU
    // cost is "our systems" vs. fixed engine/render overhead. Restored for
    // Phase 6 of the wgpu-replacement plan — the CPU-encoding-cost
    // investigation concluded the overhead is wgpu-architectural, not
    // driven by system count, so there's no reason to keep real gameplay
    // logic disabled, and Phase 6 needs it running for anything to move.
    app.add_systems(
        Update,
        (
            (
                multi_touch_button_interaction,
                toggle_mouse_ui_lock,
                suppress_ui_interaction_when_locked,
                touch_move_axis,
                update_joystick_knob_visual,
                touch_look,
                touch_fire_button,
                touch_aim_button,
                touch_jump_button,
                update_noclip_fly_input,
                touch_crouch_button,
                touch_reload_button,
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
                    auto_maintain_button,
                    update_auto_maintain_button_color,
                    update_touch_controls_idle_hidden,
                    hide_ui_when_locked,
                )
                    .chain(),
            )
                .chain(),
            (
                (
                    grab_cursor,
                    toggle_noclip,
                    toggle_player_stance,
                    toggle_debug_flags,
                    debug_panel_actions,
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
                // Nested one level down — this group is already at Bevy's
                // 20-system tuple arity limit (see the inner .chain() above),
                // so count_local_kills gets its own outer tuple slot instead
                // of a 21st flat entry. Still fully ordered: .chain() on this
                // outer 2-tuple runs the entire inner chain before this.
                count_local_kills,
            )
                .chain(),
            (
                check_game_over,
                restart_game,
                restart_button,
                wire_zombie_animations,
                wire_zombie_hitbox_bones,
                wire_gun_mesh,
                update_zombie_animation,
                freeze_zombie_animations,
                wire_player_model_animations,
                update_remote_player_model_animation,
                update_local_player_model_animation,
                update_health_ui,
                update_zombie_health_bars,
                update_floating_damage_texts,
            )
                .chain(),
            (
                adjust_scope_zoom,
                gun_view_animation,
                sync_scope_camera,
                update_scope_lens,
                update_ammo_ui,
                update_ping_ui,
                update_crosshair,
                draw_zombie_hitboxes,
                update_debug_panel,
                despawn_hit_markers,
                despawn_muzzle_flashes,
                update_smoke,
                update_blood_particles,
                update_spark_particles,
                despawn_blood_stains,
            )
                .chain(),
        )
            .chain(),
    );
}

#[derive(Resource)]
struct HitMarkerAssets {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
}

// Synthesized (no licensed sample available) noise-burst + sub-bass thump —
// see assets/audio/gunshot.wav. Spawned fresh with PlaybackSettings::DESPAWN
// on every shot in shoot(), rather than reused, so overlapping shots (fast
// fire rate) each get their own independent playback instead of cutting
// each other off.
#[derive(Resource)]
struct GunshotSound(Handle<AudioSource>);

// Synthesized mag-out/mag-in clack + bolt-release snap — see
// assets/audio/reload.wav. Played once when a reload starts (both the
// auto-reload in shoot() and the manual KeyR reload in reload_gun), not
// synced frame-by-frame to RELOAD_DURATION's dip/tilt animation.
#[derive(Resource)]
struct ReloadSound(Handle<AudioSource>);

// Bundles HitMarkerAssets + MuzzleFxAssets into a single system param.
// shoot() was at 17 function parameters, one over Bevy's hard limit of 16
// (see all_tuples!(impl_system_function, 0, 16, F) in bevy_ecs) — merging
// these two (always used together for spawning shot VFX) back under it
// without cutting anything shoot() actually needs.
#[derive(SystemParam)]
struct VfxAssets<'w, 's> {
    hit_assets: Res<'w, HitMarkerAssets>,
    muzzle_fx: Res<'w, MuzzleFxAssets>,
    blood: Res<'w, BloodAssets>,
    sparks: Res<'w, SparkAssets>,
    gunshot: Res<'w, GunshotSound>,
    reload_sound: Res<'w, ReloadSound>,
    // Also just along for the ride here (not vfx-related at all) — shoot()
    // needs the player's PlayerStance for current_bullet_spread and has no
    // room for a dedicated top-level param.
    stance: Query<'w, 's, &'static PlayerStance, With<Player>>,
    // Same "no room at the top level" reasoning, now that merging in the
    // Apple Watch companion app's kill-count tracking (RecentlyHitZombies)
    // pushed shoot() back up to 17 params post-merge with autopilot's own
    // two — see count_local_kills' own doc comment for what this tracks.
    recently_hit: ResMut<'w, RecentlyHitZombies>,
    // HITBOX_PLAN.md Phase 4: resolves each ZombieHitboxBones entity's live
    // world position for zombie_hit_damage_bones — same "no room at the
    // top level" reasoning as the two fields above.
    bone_transforms: Query<'w, 's, &'static GlobalTransform>,
}

#[derive(Resource)]
struct ZombieAnim {
    graph: Handle<AnimationGraph>,
    idle: AnimationNodeIndex,
    walk: AnimationNodeIndex,
    run: AnimationNodeIndex,
    crawl: AnimationNodeIndex,
    bite: AnimationNodeIndex,
}

#[derive(Resource)]
struct PlayerModelAnim {
    graph: Handle<AnimationGraph>,
    idle: AnimationNodeIndex,
    walk: AnimationNodeIndex,
    run: AnimationNodeIndex,
    shoot: AnimationNodeIndex,
    death: AnimationNodeIndex,
}

#[derive(Resource)]
struct MuzzleFxAssets {
    flash_mesh: Handle<Mesh>,
    flash_material: Handle<StandardMaterial>,
    smoke_mesh: Handle<Mesh>,
    smoke_material: Handle<StandardMaterial>,
}

#[derive(Resource)]
struct BloodAssets {
    particle_mesh: Handle<Mesh>,
    particle_material: Handle<StandardMaterial>,
    stain_mesh: Handle<Mesh>,
    stain_material: Handle<StandardMaterial>,
}

#[derive(Resource)]
struct SparkAssets {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
}

// Prints the *actual* resolved window size/scale factor at startup, so it's
// possible to confirm from the terminal whether WindowResolution::new(1280,
// 720) really took effect — on a HiDPI/Retina display, physical pixel count
// and how "sharp" the window looks on screen are two different things, and
// this makes the real numbers visible instead of guessing from the eye.
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

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut images: ResMut<Assets<Image>>,
    touch_enabled: Res<TouchControlsEnabled>,
    windows: Query<&Window, With<PrimaryWindow>>,
) {
    // Dynamic resolution scaling render target: the main 3D camera below
    // renders into this offscreen texture instead of straight to the
    // window, and a dedicated present_camera (also spawned below) displays
    // it stretched to fill the actual window — same render-to-texture-then-
    // display-on-a-quad idea the AK scope lens already uses, just for the
    // whole screen via a fullscreen UI ImageNode instead of a 3D quad, and
    // resizable instead of fixed. RESOLUTION_SCALE_PRESETS/
    // RenderResolutionScale controls the scale; resize_scene_render_target
    // keeps this texture's actual size in sync with it (and with the
    // window, if that changes too) every frame.
    let (window_width, window_height) = windows
        .single()
        .map(|window| (window.resolution.physical_width(), window.resolution.physical_height()))
        .unwrap_or((1280, 720));
    let mut scene_image = Image::new_fill(
        scaled_scene_extent(window_width, window_height, RESOLUTION_SCALE_DEFAULT),
        TextureDimension::D2,
        &[0, 0, 0, 255],
        TextureFormat::Bgra8UnormSrgb,
        RenderAssetUsages::default(),
    );
    scene_image.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
    let scene_image_handle = images.add(scene_image);
    commands.insert_resource(SceneRenderTarget(scene_image_handle.clone()));
    commands.insert_resource(RenderResolutionScale::default());

    // present_camera: a 2D camera targeting the actual window (default
    // RenderTarget, unlike the 3D camera below). IsDefaultUiCamera routes
    // every UI node spawned without an explicit UiTargetCamera to this
    // camera instead of the 3D one — meaning all of this project's existing
    // UI (crosshair, health bar, debug panel, touch controls, the settings
    // menu below) keeps rendering at full native resolution regardless of
    // RenderResolutionScale, same as any real game's HUD. Only the 3D scene
    // itself (via the fullscreen ImageNode right below) gets scaled.
    commands.spawn((Camera2d, IsDefaultUiCamera));

    // Fullscreen quad showing the scaled-down 3D render, stretched to fill
    // the window — Bevy UI lays this out at the window's real (100%) size
    // regardless of the source texture's actual pixel dimensions, so this
    // node itself never needs touching when the scale changes, only the
    // Image behind it (see resize_scene_render_target). Spawned before any
    // other UI below so it's the back-most sibling (Bevy UI draws root
    // nodes in insertion order) — otherwise it would paint over the
    // crosshair/health bar/etc.
    commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            position_type: PositionType::Absolute,
            ..default()
        },
        ImageNode::new(scene_image_handle.clone()),
        // Without this, this node — covering the entire screen — counted as
        // "the touch is over UI" for every single touch anywhere on screen
        // (see touch_over_ui's own doc comment), since bevy_picking treats
        // any UI Node as hoverable/blocking by default. That broke
        // touch_move_axis/touch_look outright the moment touch_over_ui was
        // added: every touch looked like it was over a UI element, so
        // neither system would ever claim one for movement/look. This is
        // just the background — it should never itself count as "UI" for
        // that purpose, only real interactive elements (buttons) should.
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

    // Both are flat quads facing the camera at spawn time (a "billboard" that
    // doesn't track the camera afterwards — fine given how briefly they live).
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

    // particle: small flying droplets that spurt out on a zombie hit and
    // fall under gravity (see spawn_blood_effects/update_blood_particles).
    // stain: flat decal left where blood actually lands — one right at the
    // hit point itself, another wherever a downward raycast from there
    // finds a floor (see BLOOD_FLOOR_RAYCAST_DISTANCE).
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

    if USE_TEST_ARENA {
        spawn_test_arena(&mut commands, &mut meshes, &mut materials);
    } else {
        commands.spawn((
            SceneRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(SCENE_PATH))),
            Transform::default(),
            RigidBody::Static,
            // Waits for the scene to finish loading, then generates a trimesh
            // collider for every mesh in the hierarchy — the whole level becomes
            // solid ground/walls/stairs without hand-placed colliders.
            ColliderConstructorHierarchy::new(ColliderConstructor::TrimeshFromMesh),
        ));
    }

    commands.spawn((
        DirectionalLight {
            illuminance: 8_000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.9, 0.5, 0.0)),
        MainSun,
    ));

    // HALL_LIGHT_XS's placement is calibrated to Sponza's own hall geometry
    // — the arena instead just gets a brighter ambient/directional light
    // (see spawn_test_arena) since it's a single open room.
    if !USE_TEST_ARENA {
        spawn_hall_lights(&mut commands);
    }

    commands
        .spawn((
            Transform::from_translation(PLAYER_SPAWN),
            Player {
                vertical_velocity: 0.0,
                grounded: false,
            },
            Health(PLAYER_MAX_HEALTH),
            PlayerStance::default(),
        ));
    commands.insert_resource(GameOver(false));

    // Not parented to the Player entity — sync_player_model copies position
    // and (camera) yaw onto it each frame instead, same "separate entity,
    // synced Transform" pattern move_zombies/zombies use for NetTransform,
    // so this doesn't have to fight the Player entity's own physics-owned
    // Transform or its avian3d collider.
    commands.spawn((
        SceneRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(PLAYER_MODEL_PATH))),
        Transform::from_translation(PLAYER_SPAWN).with_scale(Vec3::splat(PLAYER_MODEL_SCALE)),
        Visibility::Hidden,
        PlayerModel,
        PlayerModelAnimRoot,
    ));

    let zombie_clips: Vec<Handle<AnimationClip>> =
        [ZOMBIE_ANIM_IDLE, ZOMBIE_ANIM_WALK, ZOMBIE_ANIM_RUN, ZOMBIE_ANIM_CRAWL, ZOMBIE_ANIM_BITE]
            .into_iter()
            .map(|index| asset_server.load(GltfAssetLabel::Animation(index).from_asset(ZOMBIE_PATH)))
            .collect();
    let (zombie_graph, zombie_nodes) = AnimationGraph::from_clips(zombie_clips);
    let zombie_anim = ZombieAnim {
        graph: graphs.add(zombie_graph),
        idle: zombie_nodes[0],
        walk: zombie_nodes[1],
        run: zombie_nodes[2],
        crawl: zombie_nodes[3],
        bite: zombie_nodes[4],
    };

    // No local zombie spawning here anymore — zombies are server-spawned
    // and replicated; see the on_zombie_replicated observer.
    commands.insert_resource(zombie_anim);

    let player_model_clips: Vec<Handle<AnimationClip>> = [
        PLAYER_MODEL_ANIM_IDLE,
        PLAYER_MODEL_ANIM_WALK,
        PLAYER_MODEL_ANIM_RUN,
        PLAYER_MODEL_ANIM_SHOOT,
        PLAYER_MODEL_ANIM_DEATH,
    ]
    .into_iter()
    .map(|index| asset_server.load(GltfAssetLabel::Animation(index).from_asset(PLAYER_MODEL_PATH)))
    .collect();
    let (player_model_graph, player_model_nodes) = AnimationGraph::from_clips(player_model_clips);
    commands.insert_resource(PlayerModelAnim {
        graph: graphs.add(player_model_graph),
        idle: player_model_nodes[0],
        walk: player_model_nodes[1],
        run: player_model_nodes[2],
        shoot: player_model_nodes[3],
        death: player_model_nodes[4],
    });

    let camera = commands
        .spawn((
            Camera3d::default(),
            Projection::Perspective(PerspectiveProjection {
                fov: BASE_FOV,
                ..default()
            }),
            Transform::from_translation(PLAYER_SPAWN + Vec3::Y * EYE_HEIGHT_OFFSET),
            FpsCamera { yaw: 0.0, pitch: 0.0 },
            ViewRecoil::default(),
            Lean::default(),
            AmbientLight {
                color: Color::WHITE,
                brightness: 250.0,
                ..default()
            },
            // Glow around bright spots (muzzle flash, hit markers). Requires Hdr,
            // which Bloom pulls in automatically as a required component.
            Bloom::default(),
            // SSAO removed — it was still too heavy even at Low quality (extra
            // depth+normal prepass every frame). Keeping Msaa::Off since it's
            // cheaper than turning MSAA back on; edges will look a bit more
            // jagged without any anti-aliasing as a result.
            Msaa::Off,
            // Sees the world (0) and the gun viewmodel/scope lens (1) — the
            // scope camera below only sees layer 0.
            RenderLayers::from_layers(&[WORLD_LAYER, VIEWMODEL_LAYER]),
            // Renders into the resolution-scaled offscreen texture instead
            // of the window — see this fn's own comment on scene_image up
            // top for why (and present_camera/the fullscreen ImageNode for
            // how it actually reaches the screen).
            RenderTarget::from(scene_image_handle),
        ))
        .id();

    // Viewmodel: parented to the camera so it stays fixed in the bottom-right
    // of the screen regardless of where the player looks.
    commands.spawn((
        SceneRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(GUN_PATH))),
        Transform::from_translation(GUN_OFFSET)
            .with_rotation(Quat::from_rotation_y(GUN_ROTATION_Y_DEG.to_radians()))
            .with_scale(Vec3::splat(GUN_SCALE)),
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
    ));

    // Scope render target: the scope camera below renders the actual world
    // (minus the gun/lens, via VIEWMODEL_LAYER) into this texture every
    // frame, which the lens quad then displays as its own material.
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

    commands.insert_resource(ScopeZoom(SCOPE_DEFAULT_ZOOM));

    // order: -1 so it renders before the main camera each frame, so the
    // lens quad (rendered by the main camera) always samples this frame's
    // texture instead of lagging a frame behind. Starts inactive —
    // update_scope_lens only turns it (and the lens quad) on while ADS'd.
    let scope_camera = commands
        .spawn((
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
            Transform::from_translation(PLAYER_SPAWN + Vec3::Y * EYE_HEIGHT_OFFSET),
            AmbientLight {
                color: Color::WHITE,
                brightness: 250.0,
                ..default()
            },
            Msaa::Off,
            // World (0) so it can see out into the scene, plus its own
            // reticle layer (2) — never layer 1, so it never sees the gun.
            RenderLayers::from_layers(&[WORLD_LAYER, SCOPE_RETICLE_LAYER]),
            // So the reticle's high emissive value actually blooms/glows in
            // the scope's own render texture, laser-like, instead of just
            // being a flat bright-red triangle — Bloom pulls in Hdr as a
            // required component the same way the main camera's does.
            Bloom::default(),
            ScopeCamera,
        ))
        .id();

    // Chevron reticle: a single upward-pointing triangle, parented to the
    // scope camera itself so it always sits dead center in its view
    // regardless of zoom/aim. Unlit + a strongly overdriven emissive so it
    // blooms into a laser-like glow (see the scope camera's Bloom above)
    // instead of reading as a flat red shape.
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
    spawn_debug_panel(&mut commands);
    spawn_settings_menu(&mut commands);
    spawn_settings_toggle_button(&mut commands);
    spawn_mouse_ui_lock_indicator(&mut commands);
    if touch_enabled.0 {
        spawn_touch_controls(&mut commands);
    }
}

// window physical size * scale, floored at RESOLUTION_SCALE_MIN_TEXTURE_PX
// on each axis so a tiny window at a low scale never asks wgpu for a
// zero-size (or otherwise degenerate) render target.
fn scaled_scene_extent(window_width: u32, window_height: u32, scale: f32) -> Extent3d {
    Extent3d {
        width: ((window_width as f32 * scale) as u32).max(RESOLUTION_SCALE_MIN_TEXTURE_PX),
        height: ((window_height as f32 * scale) as u32).max(RESOLUTION_SCALE_MIN_TEXTURE_PX),
        depth_or_array_layers: 1,
    }
}

// Resizes SceneRenderTarget's backing Image whenever the window size or
// RenderResolutionScale changes — the fullscreen ImageNode in setup() always
// stretches it to 100%/100% regardless of its actual pixel size, so this is
// the only place that needs to react to either changing.
// Requires the new size to hold for RESIZE_DEBOUNCE_FRAMES straight ticks
// before actually resizing the texture — on iOS the window's reported
// physical size can apparently flicker by a pixel or two frame-to-frame
// (safe-area/notch geometry recomputing, most likely), and reacting to
// every single one of those with a real GPU texture reallocation was a
// plausible contributor to touch buttons intermittently not rendering for a
// frame (reported in-game, no other reproducible pattern found). A real
// resize (rotation, actually changing RenderResolutionScale) still reacts
// within a handful of frames — not perceptible as input lag — while
// single-frame noise gets filtered out before ever reaching image.resize().
const RESIZE_DEBOUNCE_FRAMES: u8 = 5;

fn resize_scene_render_target(
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

fn spawn_settings_menu(commands: &mut Commands) {
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
                    });
                    page.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(8.0),
                        ..default()
                    })
                    .with_children(|row| {
                        spawn_cheat_toggle_button(row, "Gun", CheatGunButton);
                        spawn_cheat_toggle_button(row, "MSAA", CheatMsaaButton);
                        spawn_cheat_toggle_button(row, "Shadow", CheatShadowButton);
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
                }
            });
        });
}// Shared visual style for both cheat button flavors below — toggle buttons
// additionally get recolored by update_cheat_toggle_colors once spawned;
// action buttons (momentary, no on/off state) stay this same color always.
fn spawn_cheat_button_node(parent: &mut ChildSpawnerCommands, label: &str, marker: impl Component) {
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

fn spawn_cheat_toggle_button(parent: &mut ChildSpawnerCommands, label: &str, marker: impl Component) {
    spawn_cheat_button_node(parent, label, marker);
}

fn spawn_cheat_action_button(parent: &mut ChildSpawnerCommands, label: &str, marker: impl Component) {
    spawn_cheat_button_node(parent, label, marker);
}

#[derive(Component)]
struct CheatNoclipButton;
#[derive(Component)]
struct CheatGodModeButton;
#[derive(Component)]
struct CheatFreezeButton;
#[derive(Component)]
struct CheatGunButton;
#[derive(Component)]
struct CheatMsaaButton;
#[derive(Component)]
struct CheatShadowButton;
#[derive(Component)]
struct CheatKillAllButton;
#[derive(Component)]
struct ZombieCountDecButton;
#[derive(Component)]
struct ZombieCountIncButton;
#[derive(Component)]
struct ZombieCountText;
#[derive(Component)]
struct AutoMaintainButton;
#[derive(Component)]
struct SensitivityDecButton;
#[derive(Component)]
struct SensitivityIncButton;
#[derive(Component)]
struct SensitivityText;

// Client's own best guess at the server's current zombie population
// target, so the cheat panel has a number to show and step from without
// waiting on a round-trip — SetZombieCount is fire-and-forget (see its own
// doc comment in crates/shared), the server never echoes the value back.
// Default matches ZombieTargetCount's own default on the server so the
// displayed number is right from the start, not just after the first tap.
#[derive(Resource)]
struct ZombieCountUi(u32);

impl Default for ZombieCountUi {
    fn default() -> Self {
        Self(10)
    }
}

// Same "client's own best guess" reasoning as ZombieCountUi — defaults to
// true to match AutoMaintainZombies' own server-side default.
#[derive(Resource)]
struct AutoMaintainUi(bool);

impl Default for AutoMaintainUi {
    fn default() -> Self {
        Self(true)
    }
}

// Apple Watch companion app feature: total zombies this local player has
// personally killed this session. Read by native_menu::kill_count via the
// Watch's own WatchConnectivity sync loop on the Swift side (see
// ViewController.swift's syncHud). Incremented by count_local_kills below,
// never by on_shot_fx directly — see RecentlyHitZombies' own doc comment
// for why a naive "ShotFx::ZombieHit{killed:true} arrived" count would
// under-count (the shooter's own client is excluded from receiving that
// broadcast — see on_shot_fx's doc comment — so most of this player's own
// kills would never be seen there at all).
#[derive(Resource, Default)]
struct KillCount(u32);

// Zombies this client's own shoot() just predicted a hit on, each paired
// with a countdown until the entry expires. count_local_kills treats a
// zombie's despawn (via RemovedComponents<Zombie>) as "I killed it" only
// while it's still in this map — otherwise an unrelated despawn (server
// respawn cycling, a teammate's kill, disconnect cleanup) would inflate
// this player's own kill count. The TTL just needs to comfortably outlast
// one damage-resolution round trip (FireShot -> server despawn ->
// replicated back to us) — see shoot()'s own doc comment on why hit
// resolution is never fully local.
const RECENTLY_HIT_ZOMBIE_TTL: f32 = 2.0;

#[derive(Resource, Default)]
struct RecentlyHitZombies(std::collections::HashMap<Entity, f32>);

// True if any entity matching `T` was tapped/clicked this frame — shared by
// every touch/click button in this file that just needs a plain
// Changed<Interaction>-gated Pressed check (most of the cheat panel).
fn tapped<T: Component>(q: &Query<&Interaction, (With<T>, Changed<Interaction>)>) -> bool {
    q.iter().any(|i| *i == Interaction::Pressed)
}

// bevy_ui's own ui_focus_system (the thing that normally drives Interaction)
// only ever looks at Touches::first_pressed_position() — a single,
// arbitrarily-chosen touch, full stop. On a touchscreen with two fingers
// down at once (e.g. holding the movement joystick while tapping Fire),
// whichever touch ui_focus_system picked as "first" gets normal hover/press
// handling, but every OTHER simultaneous touch is invisible to it — any
// button under a second finger silently never becomes Interaction::Pressed.
// This is a known community pain point, not a CADisplayLink-specific bug: a
// popular touch-joystick crate (SergioRibera/virtual_joystick) works around
// the exact same limitation by hand-testing Touches against node rects
// instead of relying on Interaction at all. This system does the same,
// narrower: it only forces Interaction::Pressed for actual Button entities
// hit by a touch OTHER than the "first" one ui_focus_system already
// manages — so it never fights ui_focus_system over the same touch, and
// never touches non-Button nodes (the joystick base/knob, which aren't
// Button, are naturally excluded by the With<Button> filter below).
// Clearing back to None on release is already handled for free: bevy_ui's
// own mouse_released check reacts to touches_input.any_just_released() —
// ANY touch ending, not just the first — and resets every Pressed node to
// None, so this system only ever needs to set, never clear.
// Runs first (see both call sites) so button-tap systems reading
// Changed<Interaction> later in the same Update tick see this frame's
// change immediately, not one frame late.
fn multi_touch_button_interaction(
    touches: Res<Touches>,
    primary_window: Query<&Window, With<PrimaryWindow>>,
    mut buttons: Query<(&ComputedNode, &UiGlobalTransform, &mut Interaction), With<Button>>,
) {
    let Some(first_id) = touches.iter().next().map(|t| t.id()) else {
        return;
    };
    let Ok(window) = primary_window.single() else {
        return;
    };
    // node.contains_point expects physical-pixel coordinates, same as
    // window.physical_cursor_position() — touch.position() is logical, so
    // needs the same scale-factor conversion ui_focus_system itself applies
    // to touches_input.first_pressed_position().
    let scale_factor = window.scale_factor();
    let other_touch_positions: Vec<Vec2> = touches
        .iter()
        .filter(|touch| touch.id() != first_id)
        .map(|touch| touch.position() * scale_factor)
        .collect();
    if other_touch_positions.is_empty() {
        return;
    }
    for (node, transform, mut interaction) in &mut buttons {
        if other_touch_positions.iter().any(|&pos| node.contains_point(*transform, pos)) {
            interaction.set_if_neq(Interaction::Pressed);
        }
    }
}

#[derive(Component)]
struct CheatHitboxesButton;
#[derive(Component)]
struct CheatAimBlurButton;
#[derive(Component)]
struct CheatZombieAnimButton;
#[derive(Component)]
struct CheatZombieHealthBarsButton;
#[derive(Component)]
struct CheatZombieLodButton;
#[derive(Component)]
struct CheatPlayerModelButton;

// Changed<Interaction>-gated like every other touch button in this file —
// each tap flips the same resource the matching desktop key
// (N/G/F/H/B/M/J/K/T respectively) does. The full merged set of desktop
// debug_panel_actions' keyboard-only toggles — none of them were reachable
// on mobile at all before (no keyboard), see this fn's own callers.
fn cheat_toggle_buttons(
    mut noclip: ResMut<Noclip>,
    mut god_mode: ResMut<GodMode>,
    mut freeze: ResMut<FreezeZombies>,
    mut gun_visible: ResMut<GunVisible>,
    mut map_msaa: ResMut<MapMsaaOn>,
    mut map_shadows: ResMut<MapShadowsOn>,
    noclip_btn: Query<&Interaction, (With<CheatNoclipButton>, Changed<Interaction>)>,
    god_btn: Query<&Interaction, (With<CheatGodModeButton>, Changed<Interaction>)>,
    freeze_btn: Query<&Interaction, (With<CheatFreezeButton>, Changed<Interaction>)>,
    gun_btn: Query<&Interaction, (With<CheatGunButton>, Changed<Interaction>)>,
    msaa_btn: Query<&Interaction, (With<CheatMsaaButton>, Changed<Interaction>)>,
    shadow_btn: Query<&Interaction, (With<CheatShadowButton>, Changed<Interaction>)>,
) {
    if tapped(&noclip_btn) {
        noclip.0 = !noclip.0;
    }
    if tapped(&god_btn) {
        god_mode.0 = !god_mode.0;
    }
    if tapped(&freeze_btn) {
        freeze.0 = !freeze.0;
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
fn apply_map_render_toggles(
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
fn cheat_toggle_buttons_2(
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
fn update_cheat_toggle_colors(
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

// Kill All is authoritative (ClearAllZombies), unlike the old desktop-only
// debug panel's Digit2 action which just despawned the client's own local
// replicated copy and silently got it back on the next replication update
// (see ClearAllZombies' own doc comment in crates/shared) — worth fixing
// here since a real button inviting repeated taps would make that bug much
// more obvious/annoying than an easy-to-forget keyboard shortcut did.
fn cheat_kill_all_button(mut commands: Commands, kill_btn: Query<&Interaction, (With<CheatKillAllButton>, Changed<Interaction>)>) {
    for interaction in &kill_btn {
        if *interaction == Interaction::Pressed {
            commands.client_trigger(ClearAllZombies);
        }
    }
}

#[derive(Component)]
struct SpawnMoreZombiesButton;
#[derive(Component)]
struct HealNearestButton;
#[derive(Component)]
struct NearDeathNearestButton;

// Same 3 actions as debug_panel_actions' Digit5/Digit4/Digit3. Health writes
// here have the same caveat the desktop keys already had — Health is
// server-authoritative/replicated, so a client-side write like
// set_nearest_zombie_health's is cosmetic and gets overwritten by the next
// replication update; pre-existing behavior, not something new introduced
// by wiring it to a button instead of a key.
fn cheat_zombie_health_buttons(
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

// -/+ step the client's own tracked target by 10 (clamped to
// 0..=ZOMBIE_SPAWNS.len(), same bounds the server itself clamps to — see
// on_set_zombie_count) and send the new absolute value; update_zombie_count_text
// reflects ZombieCountUi.0 on screen immediately, no round-trip needed.
fn zombie_count_buttons(
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

fn update_zombie_count_text(count: Res<ZombieCountUi>, mut text: Query<&mut Text, With<ZombieCountText>>) {
    if !count.is_changed() {
        return;
    }
    for mut text in &mut text {
        text.0 = format!("{}", count.0);
    }
}

fn sensitivity_buttons(
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

fn update_sensitivity_text(scale: Res<SensitivityScale>, mut text: Query<&mut Text, With<SensitivityText>>) {
    if !scale.is_changed() {
        return;
    }
    for mut text in &mut text {
        text.0 = format!("{:.2}x", scale.0);
    }
}

fn auto_maintain_button(
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

fn update_auto_maintain_button_color(auto_maintain: Res<AutoMaintainUi>, mut btn: Query<&mut BackgroundColor, With<AutoMaintainButton>>) {
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
fn spawn_settings_toggle_button(commands: &mut Commands) {
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
fn toggle_settings_menu(
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

fn update_settings_menu(visible: Res<SettingsMenuVisible>, mut menu: Query<&mut Node, With<SettingsMenuRoot>>) {
    let Ok(mut node) = menu.single_mut() else {
        return;
    };
    node.display = if visible.0 { Display::Flex } else { Display::None };
}

fn settings_menu_page_buttons(
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

fn update_settings_menu_pages(
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
fn spawn_mouse_ui_lock_indicator(commands: &mut Commands) {
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

fn toggle_mouse_ui_lock(keys: Res<ButtonInput<KeyCode>>, mut locked: ResMut<MouseUiLocked>) {
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
fn suppress_ui_interaction_when_locked(locked: Res<MouseUiLocked>, mut buttons: Query<&mut Interaction, With<Button>>) {
    if !locked.0 {
        return;
    }
    for mut interaction in &mut buttons {
        if *interaction != Interaction::None {
            *interaction = Interaction::None;
        }
    }
}

const TOUCH_CONTROLS_IDLE_HIDE_SECS: f32 = 10.0;

// Auto-hides touch controls (same set hide_ui_when_locked hides) after
// TOUCH_CONTROLS_IDLE_HIDE_SECS with no touch on screen — decluttering for
// screenshots/spectating, same idea as most mobile games' HUD auto-hide.
// Suppressed while the settings menu is open (an idle player reading the
// menu shouldn't watch its own buttons vanish out from under them) and
// reset instantly by any touch, anywhere on screen (not just a button —
// there'd be nothing to tap if the controls are already hidden).
#[derive(Resource, Default)]
struct TouchControlsIdleHidden(bool);

fn update_touch_controls_idle_hidden(
    time: Res<Time>,
    touches: Res<Touches>,
    settings_visible: Res<SettingsMenuVisible>,
    mut idle_elapsed: Local<f32>,
    mut hidden: ResMut<TouchControlsIdleHidden>,
) {
    if settings_visible.0 || touches.iter().next().is_some() {
        *idle_elapsed = 0.0;
        if hidden.0 {
            hidden.0 = false;
        }
        return;
    }
    *idle_elapsed += time.delta_secs();
    let should_hide = *idle_elapsed >= TOUCH_CONTROLS_IDLE_HIDE_SECS;
    if should_hide != hidden.0 {
        hidden.0 = should_hide;
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
fn hide_ui_when_locked(
    locked: Res<MouseUiLocked>,
    idle_hidden: Res<TouchControlsIdleHidden>,
    mut buttons: Query<&mut Node, (With<Button>, Without<TouchJoystickBase>)>,
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

fn update_mouse_ui_lock_indicator(locked: Res<MouseUiLocked>, mut text: Query<&mut Node, With<MouseUiLockIndicator>>) {
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
fn scroll_settings_menu(
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

// Touch equivalent of scroll_settings_menu — drags ScrollPosition.y to
// follow a finger, native-scroll-view style (content moves with the
// finger: dragging up reveals more content below). Same MouseUiLocked
// gate as touch_move_axis/touch_look (see MouseUiLocked's own doc comment)
// so an AssistiveTouch-synthesized "touch" from a mouse click doesn't also
// drag the menu while a hardware mouse is meant to be the only pointer.
fn touch_scroll_settings_menu(
    touches: Res<Touches>,
    visible: Res<SettingsMenuVisible>,
    mouse_ui_locked: Res<MouseUiLocked>,
    windows: Query<&Window, With<PrimaryWindow>>,
    menu: Query<(&ComputedNode, &UiGlobalTransform), With<SettingsMenuRoot>>,
    mut state: ResMut<SettingsMenuScrollState>,
    mut scroll_pos: Query<&mut ScrollPosition, With<SettingsMenuRoot>>,
) {
    if !visible.0 || mouse_ui_locked.0 {
        state.touch_id = None;
        return;
    }
    let Ok(window) = windows.single() else {
        return;
    };
    let scale_factor = window.scale_factor();
    let Ok((node, transform)) = menu.single() else {
        return;
    };

    if let Some(id) = state.touch_id {
        if touches.get_pressed(id).is_none() {
            state.touch_id = None;
        }
    }
    if state.touch_id.is_none() {
        if let Some(touch) = touches
            .iter_just_pressed()
            .find(|touch| node.contains_point(*transform, touch.position() * scale_factor))
        {
            state.touch_id = Some(touch.id());
            state.last_pos_y = touch.position().y;
        }
    }

    let Some(touch) = state.touch_id.and_then(|id| touches.get_pressed(id)) else {
        return;
    };
    let delta_y = touch.position().y - state.last_pos_y;
    state.last_pos_y = touch.position().y;
    if delta_y == 0.0 {
        return;
    }
    let Ok(mut scroll_pos) = scroll_pos.single_mut() else {
        return;
    };
    scroll_pos.y = (scroll_pos.y - delta_y).max(0.0);
}

// Same Changed<Interaction>-gated Pressed check as toggle_settings_menu — a
// tap/click sets RenderResolutionScale directly; resize_scene_render_target
// picks up the change next frame.
fn resolution_scale_buttons(
    mut scale: ResMut<RenderResolutionScale>,
    buttons: Query<(&Interaction, &ResolutionScaleButton), Changed<Interaction>>,
) {
    for (interaction, button) in &buttons {
        if *interaction == Interaction::Pressed {
            scale.0 = button.0;
        }
    }
}

// Sponza.gltf ships as pure geometry with no KHR_lights_punctual data (single
// unnamed mesh node, nothing to identify the hanging-lamp fixtures by), so
// there's no way to read exact lamp positions from the file. This guesses a
// handful of warm point lights spaced along the ~37m-long main hall
// (bounding box measured from the mesh data) to approximate the lit look
// seen in other Sponza demos. Shadows are off and intensity is modest to
// avoid repeating the earlier SSAO performance hit — nudge HALL_LIGHT_XS /
// height/intensity once you can see how well they line up with the geometry.
const HALL_LIGHT_XS: [f32; 5] = [-14.0, -7.0, 0.0, 7.0, 14.0];
const HALL_LIGHT_HEIGHT: f32 = 3.2;
const HALL_LIGHT_INTENSITY: f32 = 200_000.0;

// Shared by the initial spawn in setup() and respawn_zombies, so a killed
// zombie comes back identical to a freshly-spawned one.
// Reactive spawn: the server owns Zombie AI/movement now (see
// crates/server) — this fires when a replicated Zombie entity shows up
// (already carrying Transform/NetTransform via Zombie's own
// #[require(Transform)]) and just attaches the purely cosmetic/client-local
// pieces onto that same entity: the GLTF scene, health, hitbox bones for
// zombie_hit_damage, and a persistent Sensor collider so shoot() can
// raycast against it (see the collider's own comment below).
fn on_zombie_replicated(trigger: On<Add, Zombie>, mut commands: Commands, asset_server: Res<AssetServer>) {
    let entity = trigger.event().entity;
    commands.entity(entity).insert(zombie_cosmetic_bundle());
    commands.entity(entity).with_child(zombie_mesh_bundle(&asset_server));
    spawn_zombie_health_bar(&mut commands, entity);
}

// Debug-only: spawns a brand new, non-networked zombie purely on this
// client (Digit1's "spawn one in front of me" and the respawn-on-kill
// stand-in — see ZOMBIE_RESPAWN_DELAY's comment on why that one's a stopgap
// until damage/death is server-authoritative too). Stationary — no AI runs
// for it since that's server-only now, so it's really just a target dummy.
fn spawn_local_zombie(commands: &mut Commands, asset_server: &AssetServer, spawn_pos: Vec3) {
    let zombie_entity = commands
        .spawn((
            Transform::from_translation(spawn_pos),
            Zombie,
            // Not replicated, so unlike a real zombie it needs its own local
            // Health — nothing will ever set one via replication for it.
            // Also unkillable via shoot() now: the server has never heard of
            // this entity, so its raycast can't land on it.
            Health(ZOMBIE_MAX_HEALTH),
            zombie_cosmetic_bundle(),
        ))
        .with_child(zombie_mesh_bundle(asset_server))
        .id();
    spawn_zombie_health_bar(commands, zombie_entity);
}

// No collider here anymore — hit detection is server-authoritative now
// (crates/server's handle_fire_shot raycasts against its own zombie
// colliders), so the client doesn't need one of its own to shoot with.
// Deliberately doesn't include the mesh itself — see zombie_mesh_bundle.
// Visibility: the Zombie entity itself never had one before (nothing on it
// needed rendering directly) — now that zombie_mesh_bundle is a *child*
// or it, Bevy's visibility-propagation hook warns (bevy_ecs warning[B0004])
// if an ancestor is missing Visibility, since inherited visibility walks up
// the parent chain. Explicit here rather than relying on Zombie's own
// #[require] so this stays a client-only concern.
fn zombie_cosmetic_bundle() -> impl Bundle {
    (
        Visibility::default(),
        ZombieHitboxBones::default(),
        KnockbackVelocity::default(),
        ZombieAttackTimer::default(),
        ZombiePose::default(),
    )
}

// The visible mesh, on its own child entity instead of directly on the
// zombie's own Transform — that Transform is driven every frame by
// interpolate_net_transform (translation+rotation only, from replicated
// NetTransform), so any position offset baked into it directly would get
// clobbered the very next frame. A child's Transform is independent, which
// is what lets ZOMBIE_MODEL_Y_OFFSET actually stick (same reasoning
// PLAYER_MODEL_PATH's child already relied on). wire_zombie_animations/
// wire_zombie_hitbox_bones walk back up through ChildOf to find the Zombie
// ancestor regardless of how many hops that takes, so this extra nesting
// level doesn't break either of them.
fn zombie_mesh_bundle(asset_server: &AssetServer) -> impl Bundle {
    (
        SceneRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(ZOMBIE_PATH))),
        Transform::from_xyz(0.0, ZOMBIE_MODEL_Y_OFFSET, 0.0).with_scale(Vec3::splat(ZOMBIE_SCALE)),
    )
}

fn spawn_zombie_health_bar(commands: &mut Commands, zombie: Entity) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(ZOMBIE_HEALTH_BAR_WIDTH_PX),
                height: Val::Px(ZOMBIE_HEALTH_BAR_HEIGHT_PX),
                // Hidden until update_zombie_health_bars positions it — avoids
                // a one-frame flash at the top-left corner (Val::Px(0.0)
                // default) before the first world_to_viewport projection.
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
            ZombieHealthBar { zombie },
        ))
        .with_children(|bar| {
            bar.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.2, 0.9, 0.2)),
                ZombieHealthBarFill,
            ));
            // Sits just above the bar itself rather than overlapping it —
            // the bar is only ZOMBIE_HEALTH_BAR_HEIGHT_PX (6px) tall, too
            // thin to hold readable text.
            bar.spawn((
                Text::new(format!("{ZOMBIE_MAX_HEALTH}/{ZOMBIE_MAX_HEALTH}")),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(-14.0),
                    width: Val::Percent(100.0),
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                ZombieHealthBarText,
            ));
        });
}

// Projects each tracked zombie's head position to screen space every frame
// and resizes its fill bar to match remaining health. Despawns the bar once
// its zombie is gone (killed or otherwise despawned) instead of relying on
// any despawn-children-together mechanism, since the bar is a sibling UI
// entity, not a child of the zombie.
fn update_zombie_health_bars(
    mut commands: Commands,
    mut timings: ResMut<PerfTimings>,
    hidden: Res<ZombieHealthBarsHidden>,
    camera: Query<(&Camera, &GlobalTransform), With<FpsCamera>>,
    player: Query<&Transform, With<Player>>,
    zombies: Query<(Entity, &Transform, &Health, &ZombieMotion), With<Zombie>>,
    mut bars: Query<(Entity, &ZombieHealthBar, &mut Node, &Children)>,
    mut fills: Query<&mut Node, (With<ZombieHealthBarFill>, Without<ZombieHealthBar>)>,
    mut texts: Query<&mut Text, With<ZombieHealthBarText>>,
) {
    let perf_start = std::time::Instant::now();
    let Ok((camera, camera_transform)) = camera.single() else {
        return;
    };
    let Ok(player_transform) = player.single() else {
        return;
    };

    if hidden.0 {
        for (_, _, mut node, _) in &mut bars {
            node.display = Display::None;
        }
        timings.health_bars_ms = perf_start.elapsed().as_secs_f32() * 1000.0;
        return;
    }

    // Bars are only shown at all for zombies that are both near (not
    // motion.far) AND among the ZOMBIE_HEALTH_BAR_MAX_VISIBLE closest to
    // the player — a crowd of hundreds of nearby zombies would otherwise
    // still update/layout a bar for every single one of them.
    let mut candidates: Vec<(Entity, f32)> = zombies
        .iter()
        .filter(|(_, _, _, motion)| !motion.far)
        .map(|(entity, transform, ..)| (entity, transform.translation.distance(player_transform.translation)))
        .collect();
    candidates.sort_by(|a, b| a.1.total_cmp(&b.1));
    candidates.truncate(ZOMBIE_HEALTH_BAR_MAX_VISIBLE);
    let visible_zombies: std::collections::HashSet<Entity> = candidates.into_iter().map(|(entity, _)| entity).collect();

    for (bar_entity, bar, mut node, children) in &mut bars {
        let Ok((_, zombie_transform, health, _)) = zombies.get(bar.zombie) else {
            // try_despawn instead of despawn: observed in testing that a bar
            // can already be gone by the time this runs (exact interleaving
            // with other despawn commands the same frame isn't pinned down),
            // and a plain despawn logs a noisy "entity already despawned"
            // warning for what's a harmless no-op either way.
            commands.entity(bar_entity).try_despawn();
            continue;
        };

        // Skips the screen-space projection + UI layout update entirely
        // unless this zombie made the near+closest-N cut above.
        if !visible_zombies.contains(&bar.zombie) {
            node.display = Display::None;
            continue;
        }

        let head_pos = zombie_transform.translation + Vec3::Y * ZOMBIE_HEALTH_BAR_HEIGHT;
        let Ok(screen_pos) = camera.world_to_viewport(camera_transform, head_pos) else {
            // Behind the camera's near plane — hide instead of drawing a
            // bar at some nonsensical projected position.
            node.display = Display::None;
            continue;
        };
        node.display = Display::Flex;
        node.left = Val::Px(screen_pos.x - ZOMBIE_HEALTH_BAR_WIDTH_PX / 2.0);
        node.top = Val::Px(screen_pos.y);

        let fraction = (health.0 / ZOMBIE_MAX_HEALTH).clamp(0.0, 1.0);
        for &child in children {
            if let Ok(mut fill_node) = fills.get_mut(child) {
                fill_node.width = Val::Percent(fraction * 100.0);
            }
            if let Ok(mut text) = texts.get_mut(child) {
                text.0 = format!("{:.1}/{ZOMBIE_MAX_HEALTH}", health.0.max(0.0));
            }
        }
    }
    timings.health_bars_ms = perf_start.elapsed().as_secs_f32() * 1000.0;
}

fn spawn_floating_damage(commands: &mut Commands, world_pos: Vec3, damage: f32) {
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

fn update_floating_damage_texts(
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

// Spawns a burst of flying blood particles at a zombie hit, plus two static
// stains: one right at the hit point (blood on the wound/whatever surface
// was actually struck), and one on the floor beneath it if a downward
// raycast finds one within BLOOD_FLOOR_RAYCAST_DISTANCE (blood dripping
// down and landing) — reuses the shoot()'s own SpatialQuery rather than
// taking one as a fresh parameter, since shoot() is already at Bevy's
// 16-parameter-per-system limit (see VfxAssets' doc comment).
// `zombie` is `Some` only when the hit zombie survived (shoot() checks
// health after applying damage) — a wound stain parented to a despawning
// corpse would just despawn right along with it, so there's no point.
fn spawn_blood_effects(
    commands: &mut Commands,
    blood_assets: &BloodAssets,
    spatial_query: &SpatialQuery,
    time: &Time,
    hit_point: Vec3,
    hit_normal: Vec3,
    zombie: Option<(Entity, &GlobalTransform)>,
) {
    for i in 0..BLOOD_PARTICLE_COUNT {
        // Same "deterministic pseudo-randomness via sin" trick used
        // elsewhere (recoil jitter, bullet spread) — offset each particle's
        // seed by its index so they don't all fly the same direction.
        let seed = time.elapsed_secs() * 977.0 + i as f32 * 131.7;
        let dir = Vec3::new(ops::sin(seed * 1.3), ops::sin(seed * 0.7).abs() * 0.6, ops::sin(seed * 2.1))
            .normalize_or_zero();
        commands.spawn((
            Mesh3d(blood_assets.particle_mesh.clone()),
            MeshMaterial3d(blood_assets.particle_material.clone()),
            Transform::from_translation(hit_point),
            BloodParticle {
                velocity: dir * BLOOD_PARTICLE_SPEED,
                remaining: BLOOD_PARTICLE_LIFETIME,
            },
        ));
    }

    // The wound itself — follows the zombie around if it's still alive,
    // otherwise just sits in world space wherever it landed.
    match zombie {
        Some((entity, zombie_global)) => {
            spawn_blood_stain_on_zombie(commands, blood_assets, entity, zombie_global, hit_point, hit_normal);
        }
        None => spawn_blood_stain(commands, blood_assets, None, hit_point, hit_normal),
    }

    // A random number of drips land on whatever's below — not always
    // exactly one, and not all from the exact same spot, so a burst doesn't
    // read as a single too-tidy dot.
    let range = (BLOOD_FLOOR_STAIN_MAX - BLOOD_FLOOR_STAIN_MIN + 1) as f32;
    let count = (BLOOD_FLOOR_STAIN_MIN as f32 + ops::sin(time.elapsed_secs() * 457.0).abs() * range) as usize;
    let count = count.clamp(BLOOD_FLOOR_STAIN_MIN, BLOOD_FLOOR_STAIN_MAX);

    let Ok(down) = Dir3::new(Vec3::NEG_Y) else {
        return;
    };
    let filter = SpatialQueryFilter::default();
    for i in 0..count {
        let seed = time.elapsed_secs() * 613.0 + i as f32 * 219.3;
        let jitter = Vec3::new(ops::sin(seed), 0.0, ops::sin(seed * 1.7)) * BLOOD_FLOOR_STAIN_SPREAD;
        let origin = hit_point + jitter;
        if let Some(floor_hit) = spatial_query.cast_ray(origin, down, BLOOD_FLOOR_RAYCAST_DISTANCE, true, &filter) {
            let floor_point = origin + Vec3::NEG_Y * floor_hit.distance;
            // Floor drips are always world-space (not parented) even if the
            // zombie survived — they've already fallen and landed
            // separately from the body, so there's nothing to follow.
            spawn_blood_stain(commands, blood_assets, None, floor_point, floor_hit.normal);
        }
    }
}

// Spawns a burst of flying sparks at a non-zombie (wall/environment) hit —
// the mirror image of spawn_blood_effects. Biased outward from hit_normal
// (lerped toward a random direction, not fully random) so they read as
// bouncing off the surface rather than scattering every which way.
fn spawn_spark_effects(commands: &mut Commands, spark_assets: &SparkAssets, time: &Time, hit_point: Vec3, hit_normal: Vec3) {
    let normal = hit_normal.normalize_or_zero();
    let range = (SPARK_PARTICLE_MAX - SPARK_PARTICLE_MIN + 1) as f32;
    let count = SPARK_PARTICLE_MIN + (ops::sin(time.elapsed_secs() * 349.0).abs() * range) as usize;
    let count = count.clamp(SPARK_PARTICLE_MIN, SPARK_PARTICLE_MAX);

    for i in 0..count {
        let seed = time.elapsed_secs() * 883.0 + i as f32 * 157.1;
        let random_dir =
            Vec3::new(ops::sin(seed * 1.3), ops::sin(seed * 0.7), ops::sin(seed * 2.1)).normalize_or_zero();
        let dir = (normal + random_dir).normalize_or_zero();
        // Varies each spark's speed a bit too, so the burst doesn't look
        // like a uniform ring.
        let speed = SPARK_PARTICLE_SPEED * (0.6 + ops::sin(seed * 3.7).abs() * 0.6);
        commands.spawn((
            Mesh3d(spark_assets.mesh.clone()),
            MeshMaterial3d(spark_assets.material.clone()),
            Transform::from_translation(hit_point),
            SparkParticle {
                velocity: dir * speed,
                remaining: SPARK_PARTICLE_LIFETIME,
            },
        ));
    }
}

// `parent` is Some when this stain should be attached to (and move with)
// that entity instead of sitting still in world space.
fn spawn_blood_stain(
    commands: &mut Commands,
    blood_assets: &BloodAssets,
    parent: Option<Entity>,
    point: Vec3,
    normal: Vec3,
) {
    let rotation = Quat::from_rotation_arc(Vec3::Z, normal.normalize_or_zero());
    let transform = Transform::from_translation(point + normal.normalize_or_zero() * 0.01).with_rotation(rotation);
    let mut stain = commands.spawn((
        Mesh3d(blood_assets.stain_mesh.clone()),
        MeshMaterial3d(blood_assets.stain_material.clone()),
        transform,
        BloodStain {
            remaining: BLOOD_STAIN_LIFETIME,
        },
    ));
    if let Some(parent) = parent {
        stain.insert(ChildOf(parent));
    }
}

// Computes the stain's transform relative to the zombie (GlobalTransform::
// reparented_to handles both the zombie's current rotation and its
// ZOMBIE_SCALE — without this, a child entity's declared size would end up
// multiplied by that scale, same issue as the capsule Collider in
// spawn_zombie) and spawns it as a child, so it rides along as the zombie
// moves/turns/animates.
fn spawn_blood_stain_on_zombie(
    commands: &mut Commands,
    blood_assets: &BloodAssets,
    zombie: Entity,
    zombie_global: &GlobalTransform,
    hit_point: Vec3,
    hit_normal: Vec3,
) {
    let rotation = Quat::from_rotation_arc(Vec3::Z, hit_normal.normalize_or_zero());
    let world_transform =
        GlobalTransform::from(Transform::from_translation(hit_point + hit_normal.normalize_or_zero() * 0.01).with_rotation(rotation));
    let local_transform = world_transform.reparented_to(zombie_global);

    commands.entity(zombie).with_children(|parent| {
        parent.spawn((
            Mesh3d(blood_assets.stain_mesh.clone()),
            MeshMaterial3d(blood_assets.stain_material.clone()),
            local_transform,
            BloodStain {
                remaining: BLOOD_STAIN_LIFETIME,
            },
        ));
    });
}

fn update_blood_particles(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut Transform, &mut BloodParticle)>,
) {
    let dt = time.delta_secs();
    for (entity, mut transform, mut particle) in &mut query {
        particle.velocity.y += BLOOD_PARTICLE_GRAVITY * dt;
        transform.translation += particle.velocity * dt;

        particle.remaining -= dt;
        if particle.remaining <= 0.0 {
            commands.entity(entity).try_despawn();
        }
    }
}

fn update_spark_particles(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut Transform, &mut SparkParticle)>,
) {
    let dt = time.delta_secs();
    for (entity, mut transform, mut particle) in &mut query {
        particle.velocity.y += SPARK_PARTICLE_GRAVITY * dt;
        transform.translation += particle.velocity * dt;

        particle.remaining -= dt;
        if particle.remaining <= 0.0 {
            commands.entity(entity).try_despawn();
        }
    }
}

fn despawn_blood_stains(mut commands: Commands, time: Res<Time>, mut query: Query<(Entity, &mut BloodStain)>) {
    for (entity, mut stain) in &mut query {
        stain.remaining -= time.delta_secs();
        if stain.remaining <= 0.0 {
            commands.entity(entity).try_despawn();
        }
    }
}

// spawn_zombie_navmesh/column_footprint now live in crates/server —
// pathfinding is server-authoritative.

// A plain flat-floored square room (see USE_TEST_ARENA) — a proper level
// editor/second .gltf asset wasn't available, so this is hand-built from
// Bevy's own primitives: a floor plus four perimeter walls, each with a
// matching Collider so avian3d treats them the same as Sponza's
// mesh-derived colliders (RigidBody::Static + Collider::cuboid).
fn spawn_test_arena(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>) {
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

    let floor_size = ARENA_HALF_SIZE * 2.0;
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(floor_size, ARENA_WALL_THICKNESS, floor_size))),
        MeshMaterial3d(floor_material),
        Transform::from_xyz(0.0, -ARENA_WALL_THICKNESS / 2.0, 0.0),
        RigidBody::Static,
        Collider::cuboid(floor_size, ARENA_WALL_THICKNESS, floor_size),
    ));

    // (offset from center, length) for each of the 4 perimeter walls — two
    // run along X (north/south), two along Z (east/west), each centered on
    // its own edge and long enough to span the full square including the
    // corners (+ARENA_WALL_THICKNESS so adjacent walls overlap slightly
    // there instead of leaving a gap).
    let wall_span = floor_size + ARENA_WALL_THICKNESS;
    let walls = [
        // North/south (run along X, thin along Z)
        (Vec3::new(0.0, ARENA_WALL_HEIGHT / 2.0, -ARENA_HALF_SIZE), wall_span, ARENA_WALL_THICKNESS),
        (Vec3::new(0.0, ARENA_WALL_HEIGHT / 2.0, ARENA_HALF_SIZE), wall_span, ARENA_WALL_THICKNESS),
        // East/west (run along Z, thin along X)
        (Vec3::new(-ARENA_HALF_SIZE, ARENA_WALL_HEIGHT / 2.0, 0.0), ARENA_WALL_THICKNESS, wall_span),
        (Vec3::new(ARENA_HALF_SIZE, ARENA_WALL_HEIGHT / 2.0, 0.0), ARENA_WALL_THICKNESS, wall_span),
    ];
    for (center, size_x, size_z) in walls {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(size_x, ARENA_WALL_HEIGHT, size_z))),
            MeshMaterial3d(wall_material.clone()),
            Transform::from_translation(center),
            RigidBody::Static,
            Collider::cuboid(size_x, ARENA_WALL_HEIGHT, size_z),
        ));
    }

    // AmbientLight is per-camera in this Bevy version (not a global
    // resource) — the main camera already gets its own in setup(). A single
    // bright, wide-range PointLight overhead is enough to light this flat
    // open room evenly (no columns/corners to leave dark, unlike Sponza).
    commands.spawn((
        PointLight {
            color: Color::srgb(1.0, 0.98, 0.92),
            intensity: 4_000_000.0,
            range: ARENA_HALF_SIZE * 3.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(0.0, ARENA_WALL_HEIGHT - 0.2, 0.0),
    ));
}

fn spawn_hall_lights(commands: &mut Commands) {
    for &x in &HALL_LIGHT_XS {
        commands.spawn((
            PointLight {
                color: Color::srgb(1.0, 0.78, 0.5),
                intensity: HALL_LIGHT_INTENSITY,
                range: 12.0,
                shadows_enabled: false,
                ..default()
            },
            Transform::from_xyz(x, HALL_LIGHT_HEIGHT, 0.0),
        ));
    }
}

fn spawn_health_ui(commands: &mut Commands) {
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

fn update_health_ui(
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

fn spawn_ammo_ui(commands: &mut Commands) {
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
struct PingUi;

fn spawn_ping_ui(commands: &mut Commands) {
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

// RenetClient::rtt() is renet2's own round-trip estimate (seconds) off the
// live connection — reflects real network conditions plus whatever
// SIM_LATENCY_MS/SIM_LATENCY_JITTER_MS is currently simulating (see
// shared::sim_latency), so this doubles as a way to verify the simulated
// latency/jitter config is actually taking effect. Rewriting the text every
// render frame made the number flicker too fast to actually read, so this
// only updates 30x/sec (same idea and rate as FpsOverlayConfig's own
// refresh_interval).
const PING_UI_REFRESH_SECS: f32 = 1.0 / 30.0;

fn update_ping_ui(
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

fn update_ammo_ui(gun: Query<&Gun, Changed<Gun>>, mut text: Query<&mut Text, With<AmmoUi>>) {
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

// Any zombie actively chasing (has Chase) and within melee range continuously
// drains player health — no attack animation/cooldown, same simplicity
// tradeoff as the zombies' own one-shot-kill damage model in shoot().
fn zombies_damage_player(
    time: Res<Time>,
    game_over: Res<GameOver>,
    freeze: Res<FreezeZombies>,
    god_mode: Res<GodMode>,
    // Chase is server-only now (big-brain runs server-side) — this just
    // checks distance instead of requiring the zombie to also carry a
    // locally-nonexistent Chase marker.
    mut zombies: Query<(&Transform, &mut ZombieAttackTimer), With<Zombie>>,
    mut player: Query<(&Transform, &mut Health), With<Player>>,
    mut elapsed: Local<f32>,
) {
    if game_over.0 || freeze.0 || god_mode.0 {
        return;
    }
    // ZombieAttackTimer counts down in real wall-clock seconds, so unlike
    // update_zombie_lod's pure distance check, throttling this can't just
    // skip frames — it has to pass the FULL accumulated dt since the last
    // time this actually ran, not a single frame's delta, or the attack
    // cooldown would tick down slower than real time and damage would
    // land later than ZOMBIE_ATTACK_INTERVAL actually specifies.
    *elapsed += time.delta_secs();
    if *elapsed < 1.0 / INPUT_SEND_HZ {
        return;
    }
    let dt = *elapsed;
    *elapsed = 0.0;
    let Ok((player_transform, mut health)) = player.single_mut() else {
        return;
    };
    for (zombie_transform, mut attack_timer) in &mut zombies {
        let in_range = zombie_transform.translation.distance(player_transform.translation) <= ZOMBIE_STOP_DISTANCE;
        if !in_range {
            continue;
        }
        attack_timer.0 -= dt;
        if attack_timer.0 <= 0.0 {
            health.0 -= ZOMBIE_ATTACK_DAMAGE;
            attack_timer.0 = ZOMBIE_ATTACK_INTERVAL;
        }
    }
}

// Apple Watch companion app's kill counter — see KillCount's own doc
// comment for why this can't just watch ShotFx::ZombieHit{killed:true}
// (the shooter never receives that broadcast for their own shot, only
// everyone else does). Instead, shoot() records every zombie this client
// predicted a local hit on into RecentlyHitZombies with a short TTL; this
// system watches for those same entities actually leaving the world
// (RemovedComponents<Zombie> — real replicated despawn, not just cosmetic)
// and counts it as a kill only while the TTL hasn't expired yet, so an
// unrelated despawn (server respawn cycling, another player's kill,
// disconnect cleanup) long after the TTL window doesn't get miscounted.
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

fn check_game_over(
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

// Just asks the server to restart — it's the one that actually decides how
// (despawn+respawn zombies, revive every connected player), same as every
// other piece of shared world state moved there in earlier phases. Any
// client whose own GameOver is set can trigger it; see on_game_state_event
// for the actual local reset once the server confirms via GameStateEvent.
fn restart_game(mut commands: Commands, keys: Res<ButtonInput<KeyCode>>, game_over: Res<GameOver>) {
    if !game_over.0 || !keys.just_pressed(KeyCode::KeyR) {
        return;
    }
    commands.client_trigger(RequestRestart);
}

// Touch/click path for the same request — see RestartButton's own spawn
// site. Changed<Interaction>-gated like the other touch buttons, so a tap
// only fires once instead of every frame it's held.
fn restart_button(mut commands: Commands, buttons: Query<&Interaction, (With<RestartButton>, Changed<Interaction>)>) {
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
fn on_game_state_event(
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
                transform.translation = PLAYER_SPAWN;
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

// Four independent hash marks instead of a fixed cross, so update_crosshair
// can push them apart from the center as bullet spread grows. direction
// points away from center: (0,-1) top, (0,1) bottom, (-1,0) left, (1,0) right.
#[derive(Component)]
struct CrosshairLine {
    direction: Vec2,
}

// Marks the crosshair's root Node so update_crosshair can hide the whole
// thing while ADS'd — the scope's own reticle takes over at that point.
#[derive(Component)]
struct CrosshairRoot;

fn spawn_debug_panel(commands: &mut Commands) {
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
struct DebugPanelToggles<'w> {
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
}

fn update_debug_panel(
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
fn set_nearest_zombie_health(
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
fn debug_panel_actions(
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

fn spawn_crosshair(commands: &mut Commands) {
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
fn update_crosshair(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    touch_aiming: Res<TouchAiming>,
    cursor: Query<&CursorOptions, With<PrimaryWindow>>,
    camera: Query<&ViewRecoil, With<FpsCamera>>,
    player: Query<&PlayerStance, With<Player>>,
    mut root: Query<&mut Node, (With<CrosshairRoot>, Without<CrosshairLine>)>,
    mut lines: Query<(&CrosshairLine, &mut Node)>,
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
    let sprinting = keys.pressed(KeyCode::ShiftLeft) && stance == PlayerStance::Standing;
    let spread = current_bullet_spread(recoil.pitch_kick, aiming, is_moving(&keys), stance, sprinting);
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

// Bottom-left joystick base+knob, bottom-right fire button — same
// absolute-Node/BackgroundColor style spawn_crosshair/spawn_health_ui
// already use elsewhere in setup(). The look region (right half of the
// screen) deliberately has no visible Node — see touch_look, which just
// compares touch x-position against window width/2 instead of hit-testing
// a UI element, so there's nothing to draw for it.
fn spawn_touch_controls(commands: &mut Commands) {
    commands
        .spawn((
            Node {
                width: Val::Px(TOUCH_JOYSTICK_BASE_PX),
                height: Val::Px(TOUCH_JOYSTICK_BASE_PX),
                position_type: PositionType::Absolute,
                left: Val::Px(32.0),
                bottom: Val::Px(32.0),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(TOUCH_JOYSTICK_BASE_PX / 2.0)),
                ..default()
            },
            // Alpha bumped from the original 0.15/0.35 (barely visible
            // against a bright in-game background — reported as "can't see
            // the buttons at all" on-device) plus an opaque-ish white border
            // outline so the button's outline reads even over a
            // similarly-light patch of scene behind it.
            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.3)),
            BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.6)),
            GlobalZIndex(STATIC_UI_Z_INDEX),
            TouchJoystickBase,
        ))
        .with_children(|base| {
            base.spawn((
                Node {
                    width: Val::Px(TOUCH_JOYSTICK_KNOB_PX),
                    height: Val::Px(TOUCH_JOYSTICK_KNOB_PX),
                    position_type: PositionType::Absolute,
                    left: Val::Px((TOUCH_JOYSTICK_BASE_PX - TOUCH_JOYSTICK_KNOB_PX) / 2.0),
                    top: Val::Px((TOUCH_JOYSTICK_BASE_PX - TOUCH_JOYSTICK_KNOB_PX) / 2.0),
                    border_radius: BorderRadius::all(Val::Px(TOUCH_JOYSTICK_KNOB_PX / 2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.55)),
                TouchJoystickKnob,
            ));
        });

    commands.spawn((
        Node {
            width: Val::Px(TOUCH_FIRE_BUTTON_PX),
            height: Val::Px(TOUCH_FIRE_BUTTON_PX),
            position_type: PositionType::Absolute,
            right: Val::Px(32.0),
            bottom: Val::Px(48.0),
            border: UiRect::all(Val::Px(2.0)),
            border_radius: BorderRadius::all(Val::Px(TOUCH_FIRE_BUTTON_PX / 2.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(1.0, 0.3, 0.3, 0.55)),
        BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.6)),
        Button,
        Interaction::default(),
        GlobalZIndex(STATIC_UI_Z_INDEX),
        TouchFireButton,
    ));

    // Toggle (not hold) — see TouchAiming's own doc comment for why. Sits
    // just above the fire button, out of the way of a firing thumb.
    commands
        .spawn((
            Node {
                width: Val::Px(TOUCH_FIRE_BUTTON_PX * 0.7),
                height: Val::Px(TOUCH_FIRE_BUTTON_PX * 0.7),
                position_type: PositionType::Absolute,
                right: Val::Px(32.0 + (TOUCH_FIRE_BUTTON_PX - TOUCH_FIRE_BUTTON_PX * 0.7) / 2.0),
                bottom: Val::Px(48.0 + TOUCH_FIRE_BUTTON_PX + 16.0),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(TOUCH_FIRE_BUTTON_PX * 0.35)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.3, 0.5, 1.0, 0.4)),
            BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.6)),
            Button,
            Interaction::default(),
            GlobalZIndex(STATIC_UI_Z_INDEX),
            TouchAimButton,
        ))
        .with_children(|btn| {
            btn.spawn((
                Text::new("ADS"),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });

    // Reload — to the left of Fire (same row/height), out of the way of
    // ADS above and a firing thumb still resting on Fire.
    commands
        .spawn((
            Node {
                width: Val::Px(TOUCH_FIRE_BUTTON_PX * 0.7),
                height: Val::Px(TOUCH_FIRE_BUTTON_PX * 0.7),
                position_type: PositionType::Absolute,
                right: Val::Px(32.0 + TOUCH_FIRE_BUTTON_PX + 16.0),
                bottom: Val::Px(48.0 + (TOUCH_FIRE_BUTTON_PX - TOUCH_FIRE_BUTTON_PX * 0.7) / 2.0),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(TOUCH_FIRE_BUTTON_PX * 0.35)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(1.0, 0.8, 0.2, 0.4)),
            BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.6)),
            Button,
            Interaction::default(),
            GlobalZIndex(STATIC_UI_Z_INDEX),
            TouchReloadButton,
        ))
        .with_children(|btn| {
            btn.spawn((
                Text::new("RELOAD"),
                TextFont {
                    font_size: 10.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });

    // Jump/Crouch sit just above the joystick, out of the thumb's drag path
    // (which stays within TOUCH_JOYSTICK_RADIUS_PX of the base) — side by
    // side rather than stacked so neither ends up directly above the other
    // under a moving thumb.
    let stance_btn_px = TOUCH_FIRE_BUTTON_PX * 0.6;
    let stance_btn_bottom = 32.0 + TOUCH_JOYSTICK_BASE_PX + 16.0;
    spawn_touch_round_button(commands, "JUMP", 32.0, stance_btn_bottom, stance_btn_px, Color::srgba(0.3, 1.0, 0.4, 0.4), TouchJumpButton);
    spawn_touch_round_button(
        commands,
        "CROUCH",
        32.0 + stance_btn_px + 12.0,
        stance_btn_bottom,
        stance_btn_px,
        Color::srgba(1.0, 0.8, 0.3, 0.4),
        TouchCrouchButton,
    );
}

// Shared helper for the small round left/right/right-labeled touch buttons
// (Jump, Crouch) — mirrors the TouchAimButton node built by hand just above,
// factored out since there are now two more of these with only position/
// label/color/marker differing.
fn spawn_touch_round_button(commands: &mut Commands, label: &str, left_px: f32, bottom_px: f32, size_px: f32, color: Color, marker: impl Component) {
    commands
        .spawn((
            Node {
                width: Val::Px(size_px),
                height: Val::Px(size_px),
                position_type: PositionType::Absolute,
                left: Val::Px(left_px),
                bottom: Val::Px(bottom_px),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(size_px / 2.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(color),
            BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.6)),
            Button,
            Interaction::default(),
            GlobalZIndex(STATIC_UI_Z_INDEX),
            marker,
        ))
        .with_children(|btn| {
            btn.spawn((
                Text::new(label),
                TextFont {
                    font_size: 10.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });
}

// Toggles TouchAiming on the instant a tap lands (Changed<Interaction>, same
// debounce pattern as toggle_settings_menu — without it, holding the button
// down would flip the toggle every single frame instead of once per tap).
fn touch_aim_button(mut aiming: ResMut<TouchAiming>, button: Query<&Interaction, (With<TouchAimButton>, Changed<Interaction>)>) {
    for interaction in &button {
        if *interaction == Interaction::Pressed {
            aiming.0 = !aiming.0;
        }
    }
}

// Resets to false every tick and only goes true on the exact frame the
// button's Interaction changes to Pressed — see TouchJump's own doc comment
// for why this has to be an edge, not a held level. Shared by the real game
// and minimal_map_probe_main; each has its own consumer of TouchJump.
fn touch_jump_button(mut jump: ResMut<TouchJump>, button: Query<&Interaction, (With<TouchJumpButton>, Changed<Interaction>)>) {
    jump.0 = button.iter().any(|interaction| *interaction == Interaction::Pressed);
}

// Same one-frame-pulse shape as touch_jump_button.
fn touch_reload_button(mut reload: ResMut<TouchReload>, button: Query<&Interaction, (With<TouchReloadButton>, Changed<Interaction>)>) {
    reload.0 = button.iter().any(|interaction| *interaction == Interaction::Pressed);
}

// No Changed<> gate on either query (unlike touch_jump_button/touch_crouch_
// button) — see NoclipFlyInput's own doc comment for why noclip flight
// needs a continuous held level instead of an edge pulse/toggle.
fn update_noclip_fly_input(
    mut fly: ResMut<NoclipFlyInput>,
    jump_button: Query<&Interaction, With<TouchJumpButton>>,
    crouch_button: Query<&Interaction, With<TouchCrouchButton>>,
) {
    fly.up = jump_button.iter().any(|interaction| *interaction == Interaction::Pressed);
    fly.down = crouch_button.iter().any(|interaction| *interaction == Interaction::Pressed);
}

// Same tap-edge toggle as touch_aim_button, mirroring toggle_player_stance's
// own KeyC branch (Standing<->Crouching, switches straight over from Prone
// too) so a touch tap and a desktop KeyC press behave identically. Skipped
// entirely while noclipping — send_player_input reads TouchCrouchHeld
// directly for fly-down instead, so tapping Crouch during noclip doesn't
// leave PlayerStance stuck on Crouching once noclip is turned back off.
fn touch_crouch_button(
    noclip: Res<Noclip>,
    button: Query<&Interaction, (With<TouchCrouchButton>, Changed<Interaction>)>,
    mut player: Query<&mut PlayerStance, With<Player>>,
) {
    if noclip.0 {
        return;
    }
    for interaction in &button {
        if *interaction == Interaction::Pressed {
            let Ok(mut stance) = player.single_mut() else {
                return;
            };
            *stance = if *stance == PlayerStance::Crouching {
                PlayerStance::Standing
            } else {
                PlayerStance::Crouching
            };
        }
    }
}

// Claims whichever touch starts on the left half of the screen as the
// movement touch, tracks it while held, and writes the drag-from-origin
// vector (clamped/normalized to TOUCH_JOYSTICK_RADIUS_PX) into
// TouchMoveAxis — send_player_input adds this straight into the same
// move_axis WASD builds. Dropping the touch (finger lifted) snaps back to
// zero; there's no separate "spring back to center" tween.
// True if this touch is currently hovering/pressing any UI element (a
// settings-menu button, in practice — nothing else in this game's UI sits
// over the movement/look halves of the screen). Without this check,
// touch_move_axis/touch_look would ALSO claim the exact same touch a
// button tap uses as a gameplay input (a settings-menu button lives on the
// left half, right where the joystick claims any new touch) — the button's
// own Interaction still fires correctly either way, but the joystick
// snapping to a touch that's really tapping a menu button (and staying
// claimed after release, since the touch never started as a "real"
// movement drag) was the actual cause of the settings menu closing itself
// right back up on the very next tap.
// Only counts a hover as "real UI" if the hovered entity also has
// Interaction (i.e. an actual Button) — every Node is hoverable by default
// in bevy_picking regardless of whether it's interactive, so without this
// filter, touching purely decorative UI (crosshair, health bar text, even
// the joystick's own base/knob visuals, which have no Button either) would
// ALSO count as "over UI" and block touch_move_axis/touch_look, including
// the joystick blocking its own drag.
fn touch_over_ui(touch_id: u64, hover_map: &HoverMap, interactive: &Query<(), With<Interaction>>) -> bool {
    hover_map
        .get(&PointerId::Touch(touch_id))
        .is_some_and(|hits| hits.keys().any(|&entity| interactive.contains(entity)))
}

// touch_over_ui above is a no-op in this custom CADisplayLink pipeline —
// HoverMap only gets populated by bevy_picking's touch_pick_events system,
// which reads WindowEvent::TouchInput, and this app only ever emits the
// raw TouchInput message (see ffi.rs's touched()), so HoverMap stays
// permanently empty and touch_over_ui always returns false. Concretely
// this meant a touch starting anywhere on the LEFT half of the screen —
// including on top of the open settings menu, which can span past the
// midline — always got claimed by the joystick (touch_move_axis) first,
// so the menu's own touch_scroll_settings_menu never got a chance to see
// that touch. This hand-rolled rect check (same ComputedNode::contains_
// point technique multi_touch_button_interaction/touch_scroll_settings_
// menu already use) is a working substitute, scoped to just the one node
// that actually needs excluding here.
fn touch_over_settings_menu(
    pos_physical: Vec2,
    visible: &SettingsMenuVisible,
    menu: &Query<(&ComputedNode, &UiGlobalTransform), With<SettingsMenuRoot>>,
) -> bool {
    visible.0 && menu.single().is_ok_and(|(node, transform)| node.contains_point(*transform, pos_physical))
}

fn touch_move_axis(
    touches: Res<Touches>,
    windows: Query<&Window, With<PrimaryWindow>>,
    hover_map: Res<HoverMap>,
    interactive: Query<(), With<Interaction>>,
    mouse_ui_locked: Res<MouseUiLocked>,
    settings_visible: Res<SettingsMenuVisible>,
    settings_menu: Query<(&ComputedNode, &UiGlobalTransform), With<SettingsMenuRoot>>,
    mut state: ResMut<TouchMoveState>,
    mut axis: ResMut<TouchMoveAxis>,
) {
    // AssistiveTouch (the only way to use a Bluetooth mouse on iPhone —
    // see GameControllerInput.swift) synthesizes real UITouch events at the
    // pointer's location on click, which this system sees exactly like a
    // finger tap — MouseUiLocked exists so a mouse click can't drive
    // movement via the joystick zone this way.
    if mouse_ui_locked.0 {
        state.touch_id = None;
        axis.0 = Vec2::ZERO;
        return;
    }
    let Ok(window) = windows.single() else {
        axis.0 = Vec2::ZERO;
        return;
    };
    let half_width = window.width() / 2.0;
    let scale_factor = window.scale_factor();

    if let Some(id) = state.touch_id {
        if touches.get_pressed(id).is_none() {
            state.touch_id = None;
        }
    }
    if state.touch_id.is_none() {
        if let Some(touch) = touches.iter_just_pressed().find(|touch| {
            touch.position().x < half_width
                && !touch_over_ui(touch.id(), &hover_map, &interactive)
                && !touch_over_settings_menu(touch.position() * scale_factor, &settings_visible, &settings_menu)
        }) {
            state.touch_id = Some(touch.id());
            state.origin = touch.position();
        }
    }

    axis.0 = match state.touch_id.and_then(|id| touches.get_pressed(id)) {
        Some(touch) => {
            let drag = (touch.position() - state.origin).clamp_length_max(TOUCH_JOYSTICK_RADIUS_PX) / TOUCH_JOYSTICK_RADIUS_PX;
            // Screen Y grows downward; move_axis.y is "forward" (matches
            // WASD's KeyW = +1.0), so this flips the sign.
            Vec2::new(drag.x, -drag.y)
        }
        None => Vec2::ZERO,
    };
}

// Purely cosmetic: slides the joystick knob to follow the tracked drag so
// there's visual feedback, independent of the actual TouchMoveAxis math
// above.
fn update_joystick_knob_visual(state: Res<TouchMoveState>, touches: Res<Touches>, mut knob: Query<&mut Node, With<TouchJoystickKnob>>) {
    let Ok(mut node) = knob.single_mut() else {
        return;
    };
    let offset = match state.touch_id.and_then(|id| touches.get_pressed(id)) {
        Some(touch) => (touch.position() - state.origin).clamp_length_max(TOUCH_JOYSTICK_RADIUS_PX),
        None => Vec2::ZERO,
    };
    let center = (TOUCH_JOYSTICK_BASE_PX - TOUCH_JOYSTICK_KNOB_PX) / 2.0;
    node.left = Val::Px(center + offset.x);
    node.top = Val::Px(center + offset.y);
}

// Mirror of mouse_look for the right half of the screen — writes into the
// exact same FpsCamera.yaw/pitch and MouseLookDelta, so everything
// downstream (apply_view_recoil composing the final camera rotation,
// recoil kick, etc.) doesn't need to know or care whether the drag came
// from a mouse or a finger.
fn touch_look(
    touches: Res<Touches>,
    windows: Query<&Window, With<PrimaryWindow>>,
    hover_map: Res<HoverMap>,
    interactive: Query<(), With<Interaction>>,
    mouse_ui_locked: Res<MouseUiLocked>,
    settings_visible: Res<SettingsMenuVisible>,
    settings_menu: Query<(&ComputedNode, &UiGlobalTransform), With<SettingsMenuRoot>>,
    mut state: ResMut<TouchLookState>,
    mut look_delta: ResMut<MouseLookDelta>,
    sensitivity_scale: Res<SensitivityScale>,
    mut query: Query<&mut FpsCamera>,
    autopilot: Res<Autopilot>,
) {
    // Autopilot drives FpsCamera.yaw/pitch itself (see autopilot_think,
    // which already ran earlier this same Update tick) — real touch-look
    // input is ignored entirely while it's on, same as mouse_look below.
    if autopilot.0 {
        return;
    }
    // See touch_move_axis's own comment on MouseUiLocked/AssistiveTouch —
    // same reasoning here so a mouse click on the right half doesn't also
    // yank the camera via the touch look-drag zone while GCMouse deltas
    // (mouse_look) are already driving it.
    if mouse_ui_locked.0 {
        state.touch_id = None;
        return;
    }
    let Ok(window) = windows.single() else {
        return;
    };
    let half_width = window.width() / 2.0;
    let scale_factor = window.scale_factor();

    if let Some(id) = state.touch_id {
        if touches.get_pressed(id).is_none() {
            state.touch_id = None;
        }
    }
    if state.touch_id.is_none() {
        if let Some(touch) = touches.iter_just_pressed().find(|touch| {
            touch.position().x >= half_width
                && !touch_over_ui(touch.id(), &hover_map, &interactive)
                && !touch_over_settings_menu(touch.position() * scale_factor, &settings_visible, &settings_menu)
        }) {
            state.touch_id = Some(touch.id());
            state.last_pos = touch.position();
        }
    }

    let Some(touch) = state.touch_id.and_then(|id| touches.get_pressed(id)) else {
        return;
    };
    let delta = touch.position() - state.last_pos;
    state.last_pos = touch.position();
    if delta == Vec2::ZERO {
        return;
    }

    look_delta.0 = delta;
    let sensitivity = MOUSE_SENSITIVITY * TOUCH_LOOK_SENSITIVITY_MULTIPLIER * sensitivity_scale.0;
    for mut cam in &mut query {
        cam.yaw -= delta.x * sensitivity;
        cam.pitch = (cam.pitch - delta.y * sensitivity).clamp(-PITCH_CLAMP, PITCH_CLAMP);
    }
}

// Held (not just tapped) → auto-fires the same way holding left-click does
// on desktop, via shoot()'s own FIRE_RATE cooldown.
fn touch_fire_button(mut firing: ResMut<TouchFiring>, buttons: Query<&Interaction, With<TouchFireButton>>) {
    firing.0 = buttons.iter().any(|interaction| matches!(interaction, Interaction::Pressed));
}

// Click into the window to lock/hide the cursor for mouse-look; Esc releases it.
fn grab_cursor(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut cursor: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    let Ok(mut cursor) = cursor.single_mut() else {
        return;
    };

    if mouse.just_pressed(MouseButton::Left) && cursor.grab_mode == CursorGrabMode::None {
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
    }
    if keys.just_pressed(KeyCode::Escape) {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    }
}

// Updates the player's aim (yaw/pitch) from mouse movement. Doesn't touch the
// Transform directly — apply_view_recoil composes the final rotation from
// this aim plus the current recoil kick, every frame.
fn mouse_look(
    cursor: Query<&CursorOptions, With<PrimaryWindow>>,
    mouse: Res<ButtonInput<MouseButton>>,
    touch_aiming: Res<TouchAiming>,
    zoom: Res<ScopeZoom>,
    mut motion: MessageReader<MouseMotion>,
    mut look_delta: ResMut<MouseLookDelta>,
    sensitivity_scale: Res<SensitivityScale>,
    mut query: Query<&mut FpsCamera>,
    autopilot: Res<Autopilot>,
) {
    // Autopilot drives FpsCamera.yaw/pitch itself — see autopilot_think,
    // which already ran earlier this same Update tick.
    if autopilot.0 {
        return;
    }
    let Ok(cursor) = cursor.single() else {
        look_delta.0 = Vec2::ZERO;
        return;
    };
    if cursor.grab_mode == CursorGrabMode::None {
        motion.clear();
        look_delta.0 = Vec2::ZERO;
        return;
    }

    let delta: Vec2 = motion.read().map(|e| e.delta).sum();
    look_delta.0 = delta;
    if delta == Vec2::ZERO {
        return;
    }

    // AIM_MOUSE_SENSITIVITY_MULTIPLIER is calibrated at SCOPE_DEFAULT_ZOOM;
    // scaled further by how far the current scope zoom (adjustable via
    // scroll wheel, see adjust_scope_zoom) is from that baseline, so a
    // given mouse move covers the same angular slice of the zoomed view no
    // matter the magnification — otherwise the same flat multiplier would
    // feel twitchy at max zoom and sluggish at min zoom.
    let sensitivity = if is_aiming(cursor, &mouse, touch_aiming.0) {
        MOUSE_SENSITIVITY * AIM_MOUSE_SENSITIVITY_MULTIPLIER * (SCOPE_DEFAULT_ZOOM / zoom.0)
    } else {
        MOUSE_SENSITIVITY
    } * sensitivity_scale.0;
    for mut cam in &mut query {
        cam.yaw -= delta.x * sensitivity;
        cam.pitch = (cam.pitch - delta.y * sensitivity).clamp(-PITCH_CLAMP, PITCH_CLAMP);
    }
}

// Composes the camera's final rotation from the mouse-driven aim plus the
// current recoil kick, and lets the kick spring back down over time.
fn apply_view_recoil(
    time: Res<Time>,
    mut query: Query<(&mut Transform, &FpsCamera, &mut ViewRecoil, &Lean)>,
) {
    for (mut transform, cam, mut recoil, lean) in &mut query {
        let decay = (RECOIL_RECOVERY_SPEED * time.delta_secs()).min(1.0);
        recoil.pitch_kick *= 1.0 - decay;
        recoil.yaw_kick *= 1.0 - decay;

        let pitch = (cam.pitch + recoil.pitch_kick).clamp(-1.54, 1.54);
        let yaw = cam.yaw + recoil.yaw_kick;
        // Negated: a positive Z rotation in Bevy's right-handed coords rolls
        // the camera the opposite way from what "lean left tilts view left"
        // intuitively expects (confirmed backwards when actually tested).
        let roll = -LEAN_ROLL_DEG.to_radians() * lean.current;
        transform.rotation = Quat::from_euler(EulerRot::YXZ, yaw, pitch, roll);
    }
}

// Q leans left, E leans right; releasing both (or holding both at once)
// eases back to upright. Gated on !game_over like the other player-facing
// input systems.
fn apply_lean_input(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    game_over: Res<GameOver>,
    mut query: Query<&mut Lean>,
) {
    let target: f32 = if game_over.0 {
        0.0
    } else {
        match (keys.pressed(KeyCode::KeyQ), keys.pressed(KeyCode::KeyE)) {
            (true, false) => -1.0,
            (false, true) => 1.0,
            _ => 0.0,
        }
    };
    for mut lean in &mut query {
        // Plain proportional lerp — reacts immediately to input, no routing
        // through center. The "fast on reversal" complaint turned out to be
        // about the gun mesh's shoulder swap, not this camera roll value;
        // see gun_view_animation's own shoulder_blend for that fix instead.
        let t = (LEAN_SPEED * time.delta_secs()).min(1.0);
        lean.current += (target - lean.current) * t;
    }
}

// Lerps the camera's FOV toward a zoomed-in value while aiming, and back to
// BASE_FOV otherwise — same lerp-toward-target shape as apply_view_recoil's
// decay, just easing up instead of down.
fn apply_aim_zoom(
    time: Res<Time>,
    mouse: Res<ButtonInput<MouseButton>>,
    touch_aiming: Res<TouchAiming>,
    cursor: Query<&CursorOptions, With<PrimaryWindow>>,
    mut camera: Query<&mut Projection, With<FpsCamera>>,
) {
    let Ok(cursor) = cursor.single() else {
        return;
    };
    let target_fov = if is_aiming(cursor, &mouse, touch_aiming.0) {
        BASE_FOV * AIM_FOV_MULTIPLIER
    } else {
        BASE_FOV
    };

    let Ok(mut projection) = camera.single_mut() else {
        return;
    };
    let Projection::Perspective(perspective) = projection.as_mut() else {
        return;
    };
    let t = (AIM_ZOOM_SPEED * time.delta_secs()).min(1.0);
    perspective.fov += (target_fov - perspective.fov) * t;
}

// Blurs the background while aiming (see AIM_DOF_FOCAL_DISTANCE) by
// inserting/removing DepthOfField on the main camera outright, rather than
// leaving it always attached — no blur at all outside ADS.
fn apply_aim_dof(
    mut commands: Commands,
    mouse: Res<ButtonInput<MouseButton>>,
    touch_aiming: Res<TouchAiming>,
    aim_blur: Res<AimBlurEnabled>,
    dof_focal_alt: Res<AimDofFocalDistanceAlt>,
    cursor: Query<&CursorOptions, With<PrimaryWindow>>,
    camera: Query<Entity, With<FpsCamera>>,
) {
    let (Ok(cursor), Ok(camera_entity)) = (cursor.single(), camera.single()) else {
        return;
    };
    if aim_blur.0 && is_aiming(cursor, &mouse, touch_aiming.0) {
        let focal_distance = if dof_focal_alt.0 {
            AIM_DOF_FOCAL_DISTANCE_B
        } else {
            AIM_DOF_FOCAL_DISTANCE_A
        };
        commands.entity(camera_entity).insert(DepthOfField {
            focal_distance,
            aperture_f_stops: AIM_DOF_APERTURE_F_STOPS,
            mode: DepthOfFieldMode::Gaussian,
            ..default()
        });
    } else {
        commands.entity(camera_entity).remove::<DepthOfField>();
    }
}

// Toggles Noclip on KeyN — separate system from send_player_input so the
// toggle itself still works even while game_over freezes real input sending.
fn toggle_noclip(keys: Res<ButtonInput<KeyCode>>, mut noclip: ResMut<Noclip>) {
    if keys.just_pressed(KeyCode::KeyN) {
        noclip.0 = !noclip.0;
    }
}

// KeyC toggles Standing<->Crouching; KeyX toggles Standing<->Prone. Pressing
// C while Prone (or X while Crouching) switches directly to the other
// stance rather than needing to stand up first.
fn toggle_player_stance(keys: Res<ButtonInput<KeyCode>>, mut player: Query<&mut PlayerStance>) {
    let Ok(mut stance) = player.single_mut() else {
        return;
    };
    if keys.just_pressed(KeyCode::KeyC) {
        *stance = if *stance == PlayerStance::Crouching {
            PlayerStance::Standing
        } else {
            PlayerStance::Crouching
        };
    }
    if keys.just_pressed(KeyCode::KeyX) {
        *stance = if *stance == PlayerStance::Prone {
            PlayerStance::Standing
        } else {
            PlayerStance::Prone
        };
    }
}

fn toggle_debug_flags(
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
) {
    if keys.just_pressed(KeyCode::KeyF) {
        freeze.0 = !freeze.0;
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
}

// FreezeZombies stops move_zombies (position) but not, on its own, the
// AnimationPlayer's own playback — this pauses/resumes every currently
// playing animation on each zombie to match, so a frozen zombie doesn't
// keep walking/idling in place. Runs every frame rather than reacting to
// FreezeZombies changing; pause()/resume() are idempotent so that's fine.
// Also pauses animation (CPU sampling + GPU skinning) for zombies past
// ZOMBIE_LOD_DISTANCE (see ZombieMotion.far) — same "not worth paying for
// something too far to be scrutinized" trade-off as move_zombies' own LOD
// path, folded into this system rather than a separate one so the two
// pause/resume sources can't fight each other over system ordering.
fn freeze_zombie_animations(
    time: Res<Time>,
    freeze: Res<FreezeZombies>,
    anim_disabled: Res<ZombieAnimDisabled>,
    player: Query<&Transform, With<Player>>,
    zombies: Query<(&Transform, &ZombieAnimationPlayer, &ZombieMotion), (With<Zombie>, Without<Player>)>,
    mut players: Query<&mut AnimationPlayer>,
) {
    let Ok(player_transform) = player.single() else {
        return;
    };
    if anim_disabled.0 {
        for (_, link, _) in &zombies {
            if let Ok(mut player) = players.get_mut(link.0) {
                for (_, active) in player.playing_animations_mut() {
                    active.pause();
                }
            }
        }
        return;
    }
    // Global cap based on total live zombie count — see the
    // ZOMBIE_ANIM_CROWD_* constants' doc comment. f32::INFINITY means "no
    // global cap", so the per-zombie distance tier below decides alone.
    let total_zombie_count = zombies.iter().count();
    let global_max_hz = if total_zombie_count > ZOMBIE_ANIM_CROWD_THRESHOLD_3 {
        ZOMBIE_ANIM_CROWD_HZ_3
    } else if total_zombie_count > ZOMBIE_ANIM_CROWD_THRESHOLD_2 {
        ZOMBIE_ANIM_CROWD_HZ_2
    } else if total_zombie_count > ZOMBIE_ANIM_CROWD_THRESHOLD_1 {
        ZOMBIE_ANIM_CROWD_HZ_1
    } else {
        f32::INFINITY
    };

    for (zombie_transform, link, motion) in &zombies {
        let Ok(mut player) = players.get_mut(link.0) else {
            continue;
        };
        let distance = zombie_transform.translation.distance(player_transform.translation);
        // Within ZOMBIE_ANIM_FULL_DISTANCE, full fidelity unless the global
        // crowd cap kicks in; beyond it, the normal medium-tier rate,
        // further capped by the crowd limit too if that's lower.
        let hz = if distance <= ZOMBIE_ANIM_FULL_DISTANCE {
            global_max_hz
        } else {
            ZOMBIE_ANIM_THROTTLE_HZ.min(global_max_hz)
        };
        // Cheap per-entity phase offset so throttled zombies don't all
        // toggle pause/resume on the exact same frame.
        let phase = ops::sin(zombie_transform.translation.x * 12.9898).abs();
        let throttle_cycle = (time.elapsed_secs() * hz + phase * 2.0) % 2.0;
        let throttled_paused_this_tick = hz.is_finite() && throttle_cycle < 1.0;
        let should_pause = freeze.0 || motion.far || throttled_paused_this_tick;
        for (_, active) in player.playing_animations_mut() {
            if should_pause {
                active.pause();
            } else {
                active.resume();
            }
        }
    }
}

// Draws the same head/chest/belly bands and limb lateral-distance threshold
// zombie_hit_damage actually uses to pick damage, as horizontal rings around
// each zombie — so what you see lines up exactly with what determines
// damage, not just an approximation of it.
const HITBOX_BONE_MARKER_RADIUS: f32 = 0.08;

// Draws a small colored sphere at each resolved hitbox bone's actual
// GlobalTransform — this is exactly what zombie_hit_damage measures
// distance to, so it tracks the real animated pose (arms swinging, head
// bobbing) instead of a fixed shape. Falls back to the old height-band
// rings (drawn relative to the zombie's root, not any bone) only for a
// zombie whose bones haven't resolved yet — see wire_zombie_hitbox_bones.
fn draw_zombie_hitboxes(
    show: Res<ShowHitboxes>,
    zombies: Query<(&Transform, &ZombieHitboxBones), With<Zombie>>,
    globals: Query<&GlobalTransform>,
    mut gizmos: Gizmos,
) {
    if !show.0 {
        return;
    }
    for (transform, bones) in &zombies {
        // The actual collision capsule handle_fire_shot/debug_bot_fire test
        // shots against (Collider::capsule(ZOMBIE_RADIUS, ZOMBIE_LENGTH),
        // centered on this Transform — see ZOMBIE_MODEL_Y_OFFSET's own doc
        // comment for why the rendered mesh is offset to put its feet at the
        // capsule's bottom). Drawn in white so it's visually distinct from
        // the colored damage-region markers below — if this capsule's top
        // ends up rendering above the visible head, that's ZOMBIE_SCALE/
        // ZOMBIE_MESH_REFERENCE_HEIGHT not actually matching the mesh's real
        // height, not a bug in the raycast itself.
        // abs(): avian3d's Collider::capsule builds the shape from two
        // endpoints (±length/2), so a negative ZOMBIE_LENGTH just flips
        // which endpoint is which — the real physics capsule ends up sized
        // off |length|, same as here. Bevy's gizmo primitive instead adds
        // half_length + radius directly with no abs(), so without this it
        // silently draws a squashed, wrong-sized capsule for a negative
        // input instead of matching what's actually being tested against.
        gizmos.primitive_3d(
            &Capsule3d { radius: ZOMBIE_RADIUS, half_length: (ZOMBIE_LENGTH / 2.0).abs() },
            Isometry3d::from_translation(transform.translation),
            Color::WHITE,
        );

        let mut any_bone = false;
        for (bone, color) in [
            (bones.head, Color::srgb(1.0, 0.2, 0.2)),
            (bones.chest, Color::srgb(1.0, 0.6, 0.0)),
            (bones.belly, Color::srgb(1.0, 1.0, 0.0)),
        ] {
            if let Some(pos) = bone.and_then(|e| globals.get(e).ok()).map(GlobalTransform::translation) {
                gizmos.sphere(Isometry3d::from_translation(pos), HITBOX_BONE_MARKER_RADIUS, color);
                any_bone = true;
            }
        }
        for &limb in &bones.limbs {
            if let Ok(global) = globals.get(limb) {
                gizmos.sphere(
                    Isometry3d::from_translation(global.translation()),
                    HITBOX_BONE_MARKER_RADIUS,
                    Color::srgb(0.2, 0.6, 1.0),
                );
                any_bone = true;
            }
        }

        if any_bone {
            continue;
        }
        // No bones resolved yet (scene still loading) — show the geometric
        // fallback so there's still *something* visible.
        let root = transform.translation;
        let ring_rotation = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
        gizmos.circle(
            Isometry3d::new(root + Vec3::Y * HITBOX_HEAD_MIN_Y, ring_rotation),
            HITBOX_LIMB_LATERAL_MIN,
            Color::srgb(1.0, 0.2, 0.2),
        );
        gizmos.circle(
            Isometry3d::new(root + Vec3::Y * HITBOX_CHEST_MIN_Y, ring_rotation),
            HITBOX_LIMB_LATERAL_MIN,
            Color::srgb(1.0, 0.6, 0.0),
        );
        gizmos.circle(
            Isometry3d::new(root + Vec3::Y * HITBOX_BELLY_MIN_Y, ring_rotation),
            HITBOX_LIMB_LATERAL_MIN,
            Color::srgb(1.0, 1.0, 0.0),
        );
    }
}

// Every unacknowledged input this client has sent, oldest first: the input
// itself, the dt it was generated with, and the Player physics state
// (vertical_velocity/grounded) immediately *after* send_player_input
// applied it locally. That trailing state is what lets
// predict_player_movement's reconciliation resume from an exact baseline
// instead of guessing — see its own doc comment for why that matters.
// Entries are dropped once LastProcessedInput (replicated from the
// player's own NetPlayer entity) catches up to their sequence number.
#[derive(Resource, Default)]
struct PredictedInputs(std::collections::VecDeque<(u32, ClientInput, f32, f32, bool)>);

// No point sending network input faster than crates/server's own
// SERVER_TICK_HZ can act on it.
const INPUT_SEND_HZ: f32 = 30.0;

// send_player_input's own throttle state — bundled into one Local (instead
// of two) since that fn was already at Bevy's 16-param system function
// limit (see HitMarkerAssets' own doc comment for the same constraint hit
// elsewhere).
#[derive(Default)]
struct InputSendThrottleState {
    elapsed: f32,
    // just_pressed()-style jump edges are only true for a single Update
    // tick; if that tick happens to fall between two throttled network
    // sends, a naive "send whatever `jump` is right now" would silently
    // drop the jump server-side. OR-ing every edge seen since the last
    // send into this flag (cleared only once actually sent) guarantees a
    // jump the player felt happen locally also reaches the server, just
    // possibly up to 1/INPUT_SEND_HZ later than the exact input frame.
    jump_edge_pending: bool,
}

// Captures the new input and applies it to the local Player immediately —
// this is what makes movement feel instant instead of waiting a round trip.
// predict_player_movement only reconciles (rare, network-tick-rate
// frequency), it doesn't replay this per-frame application.
fn send_player_input(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    touch: TouchGameplayInputs,
    time: Res<Time>,
    game_over: Res<GameOver>,
    mut commands: Commands,
    mut pending: ResMut<PredictedInputs>,
    mut sequence: Local<u32>,
    camera_query: Query<&FpsCamera>,
    cursor: Query<&CursorOptions, With<PrimaryWindow>>,
    mut player_query: Query<(&mut Transform, &mut Player, &PlayerStance), Without<NetPlayer>>,
    noclip: Res<Noclip>,
    touch_move_axis: Res<TouchMoveAxis>,
    // Bundled into one nested-tuple param instead of flat ones — this fn
    // was already at Bevy's flat SystemParam tuple arity limit (16);
    // adding LocalPlayerSprinting as a 17th flat param broke `IntoSystem`
    // for the whole Update-schedule `.chain()` tuple this fn sits in.
    // Tuples of SystemParams are themselves valid SystemParams, so nesting
    // sidesteps the limit without restructuring anything else — Autopilot/
    // AutopilotIntent are folded into the same tuple for the same reason.
    (mut local_moving, mut local_sprinting, autopilot, intent): (
        ResMut<LocalPlayerMoving>,
        ResMut<LocalPlayerSprinting>,
        Res<Autopilot>,
        Res<AutopilotIntent>,
    ),
    mover: MoveAndSlide,
    // Local prediction below still runs every single Update tick (up to
    // 120Hz on iOS via CADisplayLink) for smooth immediate feedback — only
    // the network SEND at the bottom of this fn is rate-limited, since
    // the server only ticks at SERVER_TICK_HZ (30Hz) and can't act on
    // input faster than that anyway; sending up to 4x more packets/sec
    // than the server can consume was pure wasted CPU/bandwidth on both
    // ends, not extra responsiveness.
    mut input_send_state: Local<InputSendThrottleState>,
) {
    if game_over.0 {
        return;
    }
    let Ok(cam) = camera_query.single() else {
        return;
    };
    let Ok((mut transform, mut player, stance)) = player_query.single_mut() else {
        return;
    };
    let stance = *stance;

    let aiming = cursor.single().is_ok_and(|cursor| is_aiming(cursor, &mouse, touch.aiming.0));

    let (move_axis, sprinting) = if autopilot.0 {
        (intent.move_axis, intent.sprint)
    } else {
        let sprinting = keys.pressed(KeyCode::ShiftLeft) && stance == PlayerStance::Standing;
        let mut move_axis = Vec2::ZERO;
        if keys.pressed(KeyCode::KeyW) {
            move_axis.y += 1.0;
        }
        if keys.pressed(KeyCode::KeyS) {
            move_axis.y -= 1.0;
        }
        if keys.pressed(KeyCode::KeyD) {
            move_axis.x += 1.0;
        }
        if keys.pressed(KeyCode::KeyA) {
            move_axis.x -= 1.0;
        }
        // Harmless to add unconditionally: on real mobile there's no
        // keyboard, and on desktop (only reachable via TOUCH_UI_PREVIEW)
        // this is 0 unless actively testing the touch joystick.
        (
            (move_axis + touch_move_axis.0).clamp_length_max(1.0),
            sprinting,
        )
    };
    local_moving.0 = move_axis != Vec2::ZERO;
    local_sprinting.0 = sprinting;

    *sequence += 1;
    let dt = time.delta_secs();
    // While noclipping, Crouch flies down (see simulate_noclip_step's own
    // "vertical" match on input.stance) — touch.noclip_fly.down overrides
    // the *reported* stance for this input only, without touching the
    // real PlayerStance component (touch_crouch_button already skips its
    // own toggle during noclip for the same reason: don't leave the
    // player stuck Crouching once noclip turns back off).
    let effective_stance = if noclip.0 && touch.noclip_fly.down { PlayerStance::Crouching } else { stance };
    let input = ClientInput {
        sequence: *sequence,
        move_axis,
        yaw: cam.yaw,
        // Held while noclipping (hold Space, or touch.noclip_fly.up, to
        // fly up) vs. a one-shot press for a real jump —
        // simulate_player_step's two branches (noclip vs normal) never
        // both read this in the same frame, so it's safe to give it
        // different semantics per mode instead of adding a second field.
        jump: if noclip.0 {
            keys.pressed(KeyCode::Space) || touch.noclip_fly.up
        } else {
            keys.just_pressed(KeyCode::Space) || touch.jump.0
        },
        sprint: sprinting,
        aiming,
        stance: effective_stance,
        noclip: noclip.0,
    };
    shared::movement::simulate_player_step(&mover, &mut transform, &mut player, stance, &input, dt);
    pending.0.push_back((*sequence, input, dt, player.vertical_velocity, player.grounded));

    input_send_state.jump_edge_pending |= input.jump;
    input_send_state.elapsed += dt;
    if input_send_state.elapsed < 1.0 / INPUT_SEND_HZ {
        return;
    }
    input_send_state.elapsed = 0.0;
    let mut sent_input = input;
    sent_input.jump = input_send_state.jump_edge_pending;
    input_send_state.jump_edge_pending = false;
    commands.client_trigger(sent_input);
}

// The camera doesn't move itself anymore — it just follows the physics body
// at eye height every frame. Its rotation is set separately by
// apply_view_recoil, which runs earlier in the chain.
fn sync_camera_to_player(
    player_query: Query<(&Transform, &PlayerStance), (With<Player>, Without<FpsCamera>)>,
    mut camera_query: Query<(&mut Transform, &Lean), With<FpsCamera>>,
) {
    let Ok((player_transform, stance)) = player_query.single() else {
        return;
    };
    let Ok((mut camera_transform, lean)) = camera_query.single_mut() else {
        return;
    };
    let eye_height_offset = match stance {
        PlayerStance::Standing => EYE_HEIGHT_OFFSET,
        PlayerStance::Crouching => CROUCH_EYE_HEIGHT_OFFSET,
        PlayerStance::Prone => PRONE_EYE_HEIGHT_OFFSET,
    };
    let eye = player_transform.translation + Vec3::Y * eye_height_offset;
    // Uses the rotation apply_view_recoil already set this frame (it runs
    // earlier in the chain) so the peek offset follows whichever way the
    // camera is actually facing, including its current lean roll.
    let side_offset = *camera_transform.right() * LEAN_SIDE_OFFSET * lean.current;
    camera_transform.translation = eye + side_offset;
}

// -0.8 (capsule half-height: PLAYER_RADIUS 0.35 + half PLAYER_LENGTH 0.9)
// assumed PLAYER_MODEL_PATH's rig root sits at its own feet, like
// ZOMBIE_PATH's does — measured (via a temporary per-frame log comparing
// this model's world Y against a zombie's own proven-correct ground-contact
// Y) to land EXACTLY on the floor at -0.8, yet still visibly floats.
// Inspecting soldier.glb's raw joint hierarchy directly (its "Root" bone
// sits close to "Hips", well above "Foot.L"/"Foot.R") confirms this rig's
// root is NOT at its feet, unlike the zombie's — it needs an *additional*
// drop for the hip-to-ground distance the zombie's rig doesn't have. No
// exact figure available without decoding the full skinned bind pose
// (bone rotations, not just translations, position the legs), so this is
// a reasoned estimate (typical hip height ≈ half standing height, scaled
// by PLAYER_MODEL_SCALE) rather than a measured one — confirm in-game and
// nudge further if the feet still don't sit exactly on the floor.
const PLAYER_MODEL_Y_OFFSET: f32 = -1.3;

// PLAYER_MODEL_PATH (soldier.glb, a different generic asset pack than
// ZOMBIE_PATH) had never actually been looked at in-game before — spawned
// at native scale with no facing correction, unlike ZOMBIE_PATH's own
// ZOMBIE_SCALE. Reported: at scale 1.0 the model rendered visibly taller
// than a zombie standing next to it, and walked backward relative to its
// own facing (this asset's rig faces +Z, the opposite of Bevy's
// -Z-forward convention that NetTransform.yaw/FpsCamera assume everywhere
// else). ZOMBIE_SCALE itself (0.25) turned out too small once actually
// checked (the local debug model — the only self-check available, since
// this is a first-person game with no real third-person camera — read as
// tiny/crumpled at extreme close range); this halves the gap between
// "native size" and ZOMBIE_SCALE as a best-effort middle value. Only a
// second client actually looking at this player's remote model from a
// normal distance can judge the exact number properly — nudge after that.
const PLAYER_MODEL_SCALE: f32 = 0.5;
const PLAYER_MODEL_FACING_CORRECTION: f32 = std::f32::consts::PI;

// Keeps PlayerModel (see its own doc comment) glued to the local player's
// position and look yaw every frame — not parented, so this is the
// equivalent of what parenting would do automatically, minus fighting the
// Player entity's own avian3d-owned Transform.
fn sync_player_model(
    visible: Res<PlayerModelVisible>,
    player: Query<&Transform, (With<Player>, Without<PlayerModel>)>,
    camera: Query<&FpsCamera>,
    mut model: Query<(&mut Transform, &mut Visibility), (With<PlayerModel>, Without<Player>)>,
) {
    let Ok((mut transform, mut visibility)) = model.single_mut() else {
        return;
    };
    *visibility = if visible.0 { Visibility::Visible } else { Visibility::Hidden };
    if !visible.0 {
        return;
    }
    let Ok(player_transform) = player.single() else {
        return;
    };
    let yaw = camera.single().map(|cam| cam.yaw).unwrap_or(0.0);
    transform.translation = player_transform.translation + Vec3::Y * PLAYER_MODEL_Y_OFFSET;
    // + PLAYER_MODEL_FACING_CORRECTION: see PLAYER_MODEL_SCALE's own doc
    // comment — this asset's rig faces +Z, not Bevy's -Z-forward
    // convention, so a bare `Quat::from_rotation_y(yaw)` (matching every
    // other yaw-driven rotation in this file) rendered the model walking
    // backward relative to where it was actually headed.
    transform.rotation = Quat::from_rotation_y(yaw + PLAYER_MODEL_FACING_CORRECTION);
}


// Holding left click (while the cursor is locked) fires automatically at
// FIRE_RATE: each shot kicks the gun model back, kicks the camera's view up
// with a bit of random jitter, and raycasts from the camera into the scene
// (with spread that grows the longer you hold the trigger). On a hit, it
// drops a small glowing marker at the impact point.
fn shoot(
    mut commands: Commands,
    time: Res<Time>,
    game_over: Res<GameOver>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    touch_aiming: Res<TouchAiming>,
    cursor: Query<&CursorOptions, With<PrimaryWindow>>,
    mut camera: Query<(&Transform, &mut ViewRecoil), With<FpsCamera>>,
    mut gun: Query<&mut Gun>,
    mut vfx_assets: VfxAssets,
    spatial_query: SpatialQuery,
    zombies: Query<(Entity, &Transform, &GlobalTransform, &ZombieHitboxBones), With<Zombie>>,
    touch_firing: Res<TouchFiring>,
    mut cooldown: Local<f32>,
    autopilot: Res<Autopilot>,
    intent: Res<AutopilotIntent>,
) {
    *cooldown = (*cooldown - time.delta_secs()).max(0.0);

    if game_over.0 {
        return;
    }
    let Ok(cursor) = cursor.single() else {
        return;
    };
    let firing = (cursor.grab_mode != CursorGrabMode::None && mouse.pressed(MouseButton::Left))
        || touch_firing.0
        || (autopilot.0 && intent.fire);
    if !firing || *cooldown > 0.0 {
        return;
    }
    let Ok(mut gun) = gun.single_mut() else {
        return;
    };
    // Mid-reload — click, no bang. Out-of-ammo can't happen here anymore:
    // the shot that empties the mag triggers a reload immediately below, so
    // ammo == 0 always implies reload_remaining > 0 too.
    if gun.ammo == 0 || gun.reload_remaining > 0.0 {
        return;
    }
    *cooldown = FIRE_RATE;
    gun.ammo -= 1;
    gun.recoil = RECOIL_DURATION;
    // Different frequency multiplier than the yaw jitter below so the two
    // don't stay correlated shot to shot.
    gun.recoil_roll = ops::sin(time.elapsed_secs() * 641.0);
    commands.spawn((AudioPlayer(vfx_assets.gunshot.0.clone()), PlaybackSettings::DESPAWN));
    if gun.ammo == 0 {
        // Auto-reload once the mag empties from firing — manual reload via
        // reload_gun (KeyR) is still how you top off early.
        gun.reload_remaining = RELOAD_DURATION;
        commands.spawn((AudioPlayer(vfx_assets.reload_sound.0.clone()), PlaybackSettings::DESPAWN));
    }

    let Ok((camera_transform, mut recoil)) = camera.single_mut() else {
        return;
    };

    recoil.pitch_kick = (recoil.pitch_kick + RECOIL_PITCH_KICK).min(RECOIL_PITCH_KICK_MAX);
    // Cheap deterministic "randomness" (no rand crate needed) so left/right
    // jitter varies shot to shot instead of always pulling the same way.
    let jitter = ops::sin(time.elapsed_secs() * 977.0);
    recoil.yaw_kick += jitter * RECOIL_YAW_JITTER;

    let stance = vfx_assets.stance.single().copied().unwrap_or_default();
    let sprinting = keys.pressed(KeyCode::ShiftLeft) && stance == PlayerStance::Standing;
    let spread = current_bullet_spread(
        recoil.pitch_kick,
        is_aiming(cursor, &mouse, touch_aiming.0),
        is_moving(&keys),
        stance,
        sprinting,
    );
    let spread_x = ops::sin(time.elapsed_secs() * 613.0) * spread;
    let spread_y = ops::sin(time.elapsed_secs() * 811.0) * spread;
    let spread_dir = *camera_transform.forward()
        + *camera_transform.right() * spread_x
        + *camera_transform.up() * spread_y;
    let direction = Dir3::new(spread_dir).unwrap_or(camera_transform.forward());

    // Mirror the same shoulder swap gun_view_animation applies to the gun
    // mesh's X offset (reading the same eased gun.shoulder_blend it writes,
    // not Lean.current directly, so this stays in sync with the mesh's own
    // independently-eased swap instead of the raw lean value), so the
    // flash/smoke spawn at the muzzle instead of floating on the old
    // (right-shoulder) side while leaning left.
    let swapped_muzzle_offset = Vec3::new(
        MUZZLE_OFFSET.x * (1.0 - 2.0 * gun.shoulder_blend),
        MUZZLE_OFFSET.y,
        MUZZLE_OFFSET.z,
    );

    // World-space, not parented to the camera: a flash/puff should stay put
    // where it was fired, not swing around if the player turns right after.
    let muzzle_world_pos =
        camera_transform.translation + camera_transform.rotation * swapped_muzzle_offset;

    // Copying the camera's rotation onto the quad makes its face point back
    // at the camera (a cheap "billboard" — good enough since both effects
    // are gone within a fraction of a second).
    commands.spawn((
        Mesh3d(vfx_assets.muzzle_fx.flash_mesh.clone()),
        MeshMaterial3d(vfx_assets.muzzle_fx.flash_material.clone()),
        Transform::from_translation(muzzle_world_pos).with_rotation(camera_transform.rotation),
        PointLight {
            intensity: MUZZLE_LIGHT_INTENSITY,
            range: MUZZLE_LIGHT_RANGE,
            color: Color::srgb(1.0, 0.7, 0.3),
            shadows_enabled: false,
            ..default()
        },
        MuzzleFlash {
            remaining: MUZZLE_FLASH_LIFETIME,
        },
    ));

    commands.spawn((
        Mesh3d(vfx_assets.muzzle_fx.smoke_mesh.clone()),
        MeshMaterial3d(vfx_assets.muzzle_fx.smoke_material.clone()),
        Transform::from_translation(muzzle_world_pos)
            .with_rotation(camera_transform.rotation)
            .with_scale(Vec3::splat(SMOKE_START_SCALE)),
        Smoke {
            velocity: *camera_transform.forward() * 0.4 + Vec3::Y * 0.5,
            remaining: SMOKE_LIFETIME,
        },
    ));

    // Hit detection/damage is still server-authoritative (see crates/server's
    // handle_fire_shot) — this sends the exact ray (already carrying this
    // shot's own spread/recoil jitter) for the server to resolve for real.
    // But waiting a full round trip just to show the shooter their own hit
    // response reads as input lag, so this also does its own local
    // capsule-vs-zombie raycast (client-side zombies have no Collider
    // component to test against via SpatialQuery, unlike the server, so
    // this builds one manually — see ZOMBIE_RADIUS/ZOMBIE_LENGTH) and, on a
    // hit, predicts hitmarker/blood/damage-number immediately using the same
    // shared zombie_hit_damage_bones the server itself uses — same
    // approach Source engine's "weapon prediction" uses for L4D: the client
    // predicts its own shot's effects entirely, and the server (see
    // handle_fire_shot's BroadcastExcept) knows not to echo them back to
    // this same client, only to everyone else. No wall-occlusion check
    // against this predicted ray — acceptable for the current open arena
    // layout (NAVMESH_COLUMN_OBSTACLES is empty), but a zombie visible
    // through/behind level geometry could show a mispredicted hit if that
    // ever changes.
    let zombie_capsule = Collider::capsule(ZOMBIE_RADIUS, ZOMBIE_LENGTH);
    let predicted_hit = zombies
        .iter()
        .filter_map(|(entity, transform, global, bones)| {
            zombie_capsule
                .cast_ray(transform.translation, transform.rotation, camera_transform.translation, *direction, MAX_SHOT_DISTANCE, true)
                .map(|(distance, normal)| (distance, entity, transform, global, bones, normal))
        })
        .min_by(|(a, ..), (b, ..)| a.total_cmp(b));
    if let Some((distance, entity, transform, global, bones, normal)) = predicted_hit {
        let point = camera_transform.translation + *direction * distance;
        commands.spawn((
            Mesh3d(vfx_assets.hit_assets.mesh.clone()),
            MeshMaterial3d(vfx_assets.hit_assets.material.clone()),
            Transform::from_translation(point + normal.normalize_or_zero() * 0.01),
            HitMarker {
                remaining: HIT_MARKER_LIFETIME,
            },
        ));
        // HITBOX_PLAN.md Phase 4: real per-bone hit detection — the client
        // has always had live (not even lag-compensated, since this is the
        // shooter's own local prediction) bones, unlike the server which
        // needed Phase 1-2's headless-skeleton work first.
        let resolved_bones = resolve_bone_world_positions(bones, |bone| vfx_assets.bone_transforms.get(bone).ok().map(|g| g.translation()));
        let damage = zombie_hit_damage_bones(point, transform, &resolved_bones);
        spawn_floating_damage(&mut commands, point, damage);
        spawn_blood_effects(&mut commands, &vfx_assets.blood, &spatial_query, &time, point, normal, Some((entity, global)));

        // Mark this zombie as "hit by us, watch for it dying" — see
        // count_local_kills, which is the only place this entry is ever
        // read/removed. Overwrites any still-live entry from an earlier
        // shot with a fresh TTL, which is fine: either way a kill shortly
        // after is this player's.
        vfx_assets.recently_hit.0.insert(entity, RECENTLY_HIT_ZOMBIE_TTL);
    }

    commands.client_trigger(FireShot {
        origin: camera_transform.translation,
        direction: *direction,
    });
}

// Plays the server-decided outcome of a shot — for every client EXCEPT the
// shooter, who already predicted and showed this same hit locally in
// shoot() and is excluded from receiving it at all (see handle_fire_shot's
// SendMode::BroadcastExcept). A remote player's shot was never predicted by
// this client, so it always gets the full effect here.
fn on_shot_fx(
    trigger: On<ShotFx>,
    mut commands: Commands,
    time: Res<Time>,
    vfx_assets: VfxAssets,
    spatial_query: SpatialQuery,
    zombies: Query<(Entity, &Transform), With<Zombie>>,
    globals: Query<&GlobalTransform>,
) {
    match *trigger.event() {
        ShotFx::ZombieHit { point, normal, damage, killed } => {
            spawn_floating_damage(&mut commands, point, damage);

            commands.spawn((
                Mesh3d(vfx_assets.hit_assets.mesh.clone()),
                MeshMaterial3d(vfx_assets.hit_assets.material.clone()),
                Transform::from_translation(point + normal.normalize_or_zero() * 0.01),
                HitMarker {
                    remaining: HIT_MARKER_LIFETIME,
                },
            ));

            // The server doesn't send which zombie entity it was (avoiding
            // cross-network entity mapping for a purely cosmetic detail) —
            // parenting the wound stain onto whichever zombie is nearest
            // the hit point is visually indistinguishable and much simpler.
            let zombie_for_blood = if killed {
                None
            } else {
                zombies
                    .iter()
                    .min_by(|(_, a), (_, b)| {
                        a.translation.distance_squared(point).total_cmp(&b.translation.distance_squared(point))
                    })
                    .and_then(|(entity, _)| globals.get(entity).ok().map(|g| (entity, g)))
            };
            spawn_blood_effects(&mut commands, &vfx_assets.blood, &spatial_query, &time, point, normal, zombie_for_blood);
        }
        ShotFx::SurfaceHit { point, normal } => {
            commands.spawn((
                Mesh3d(vfx_assets.hit_assets.mesh.clone()),
                MeshMaterial3d(vfx_assets.hit_assets.material.clone()),
                Transform::from_translation(point + normal.normalize_or_zero() * 0.01),
                HitMarker {
                    remaining: HIT_MARKER_LIFETIME,
                },
            ));
            spawn_spark_effects(&mut commands, &vfx_assets.sparks, &time, point, normal);
        }
    }
}

// Pressing R starts a reload unless the mag's already full or a reload is
// already running. The magazine refills instantly once RELOAD_DURATION
// elapses (see gun_view_animation) — no partial reloads, no spare-mag count
// (infinite reserve ammo), same simplicity tradeoff as the rest of the gun's
// systems.
fn reload_gun(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    touch_reload: Res<TouchReload>,
    game_over: Res<GameOver>,
    reload_sound: Res<ReloadSound>,
    mut gun: Query<&mut Gun>,
) {
    if game_over.0 || !(keys.just_pressed(KeyCode::KeyR) || touch_reload.0) {
        return;
    }
    let Ok(mut gun) = gun.single_mut() else {
        return;
    };
    if gun.ammo == MAGAZINE_SIZE || gun.reload_remaining > 0.0 {
        return;
    }
    gun.reload_remaining = RELOAD_DURATION;
    commands.spawn((AudioPlayer(reload_sound.0.clone()), PlaybackSettings::DESPAWN));
}

// Keeps the scope's render-to-texture camera glued to the main camera's
// position/orientation every frame — it has no parent/player-follow logic
// of its own, it just mirrors the real camera exactly.
fn sync_scope_camera(
    camera: Query<&Transform, (With<FpsCamera>, Without<ScopeCamera>)>,
    gun: Query<&Gun>,
    mut scope_camera: Query<&mut Transform, (With<ScopeCamera>, Without<FpsCamera>)>,
) {
    let (Ok(camera_transform), Ok(gun), Ok(mut scope_transform)) =
        (camera.single(), gun.single(), scope_camera.single_mut())
    else {
        return;
    };
    *scope_transform = *camera_transform;

    // Converts a delta expressed in the same local X/Y/Z space Gun's own
    // Transform.translation uses (see gun_view_animation) into a world
    // offset for the (unparented) scope camera: local X/Y map straight onto
    // right()/up(), but local Z is inverted relative to forward() since the
    // camera faces its own local -Z.
    let local_to_world = |local: Vec3| {
        camera_transform.right() * local.x + camera_transform.up() * local.y
            - camera_transform.forward() * local.z
    };

    // Mirrors the same lateral + "pulled in closer" sweep the gun mesh
    // itself makes from GUN_OFFSET to GUN_ADS_OFFSET as ads_blend eases in
    // (gun_view_animation's base_pos), so the scope camera's eyepoint
    // travels together with the gun instead of just rising straight up in
    // place — same mirrored-shoulder treatment on X too.
    let sweep_x = (GUN_OFFSET.x - GUN_ADS_OFFSET.x) * (1.0 - gun.ads_blend);
    let swapped_sweep_x = sweep_x * (1.0 - 2.0 * gun.shoulder_blend);
    let sweep_z = (GUN_OFFSET.z - GUN_ADS_OFFSET.z) * (1.0 - gun.ads_blend);
    let sweep_y = SCOPE_HIP_HEIGHT_OFFSET * (1.0 - gun.ads_blend);

    // Same recoil kick (X/Y/Z) and reload dip/tilt gun_view_animation stacks
    // on top of that base position — recoil_t/dip peak the same way they do
    // there, so the whole view (not just position) wobbles along with the
    // gun on every axis instead of holding rock-steady through recoil and
    // reload.
    let recoil_t = gun.recoil / RECOIL_DURATION;
    let reload_t = gun.reload_remaining / RELOAD_DURATION;
    let dip = 4.0 * reload_t * (1.0 - reload_t);
    let recoil_and_reload = Vec3::new(
        -RELOAD_LEFT_DIP * dip,
        RECOIL_KICK_UP * recoil_t - RELOAD_DIP * dip,
        RECOIL_KICK * recoil_t,
    );

    // Same walk-bob gun_view_animation applies to the gun mesh (reads
    // gun.bob_phase/bob_blend a frame stale, same as ads_blend/wall_raise
    // above — imperceptible for a continuous oscillation).
    let bob_aim_scale = 1.0 - gun.ads_blend * (1.0 - AIM_BOB_MULTIPLIER);
    let bob_x = ops::sin(gun.bob_phase) * GUN_BOB_AMOUNT_X * gun.bob_blend * bob_aim_scale;
    let bob_y =
        ops::sin(gun.bob_phase * 2.0).abs() * GUN_BOB_AMOUNT_Y * gun.bob_blend * bob_aim_scale;

    scope_transform.translation += local_to_world(Vec3::new(swapped_sweep_x, sweep_y, sweep_z))
        + local_to_world(recoil_and_reload)
        + local_to_world(Vec3::new(bob_x, -bob_y, 0.0))
        + camera_transform.up() * SCOPE_CAMERA_HEIGHT_OFFSET * gun.ads_blend;
    scope_transform.rotation *= Quat::from_rotation_z(gun.look_sway_roll.to_radians())
        * Quat::from_rotation_y(RECOIL_YAW_KICK_DEG.to_radians() * gun.recoil_roll * recoil_t)
        * Quat::from_rotation_z(RECOIL_ROLL_DEG.to_radians() * gun.recoil_roll * recoil_t)
        * Quat::from_rotation_x((RELOAD_TILT_DEG * dip + RECOIL_TILT_UP_DEG * recoil_t).to_radians());
}

// Scroll wheel steps ScopeZoom up/down while aiming — ignored (and drained,
// so a scroll during ADS doesn't "carry over" and suddenly apply once you
// start aiming again) the rest of the time.
fn adjust_scope_zoom(
    mut wheel: MessageReader<MouseWheel>,
    mouse: Res<ButtonInput<MouseButton>>,
    touch_aiming: Res<TouchAiming>,
    cursor: Query<&CursorOptions, With<PrimaryWindow>>,
    mut zoom: ResMut<ScopeZoom>,
) {
    let Ok(cursor) = cursor.single() else {
        return;
    };
    if !is_aiming(cursor, &mouse, touch_aiming.0) {
        wheel.clear();
        return;
    }
    for event in wheel.read() {
        zoom.0 = (zoom.0 * SCOPE_ZOOM_STEP_FACTOR.powf(event.y.signum()))
            .clamp(SCOPE_MIN_ZOOM, SCOPE_MAX_ZOOM);
    }
}

// Turns the scope camera/lens on as soon as ADS starts blending in at all
// (off again once the gun raises for a wall), applies the current
// ScopeZoom to the scope camera's FOV, and slides the lens quad in from
// GUN_OFFSET (the gun's own hip-fire pose) to SCOPE_LENS_OFFSET using the
// exact same gun.ads_blend lerp gun_view_animation drives the gun mesh's
// own translation with, then mirrors its X onto whichever shoulder the gun
// is currently on — so the lens visually travels in/out together with the
// gun instead of just popping in once ADS is mostly done.
fn update_scope_lens(
    gun: Query<&Gun>,
    zoom: Res<ScopeZoom>,
    mut scope_camera: Query<(&mut Camera, &mut Projection), With<ScopeCamera>>,
    mut lens: Query<(&mut Transform, &mut Visibility), (With<ScopeLens>, Without<ScopeReticle>)>,
    mut reticle: Query<&mut Transform, (With<ScopeReticle>, Without<ScopeLens>)>,
) {
    let Ok(gun) = gun.single() else {
        return;
    };
    let visible = gun.ads_blend > SCOPE_ACTIVE_ADS_THRESHOLD
        && gun.wall_raise < SCOPE_HIDDEN_WALL_RAISE_THRESHOLD;

    if let Ok((mut camera, mut projection)) = scope_camera.single_mut() {
        camera.is_active = visible;
        if let Projection::Perspective(perspective) = projection.as_mut() {
            perspective.fov = SCOPE_BASE_FOV / zoom.0;
        }
    }

    // Keeps the reticle roughly constant on screen across zoom levels (see
    // SCOPE_RETICLE_WIDTH/HEIGHT's doc comment) — it's sized to look right
    // at SCOPE_DEFAULT_ZOOM, so scale down as zoom increases past that.
    if let Ok(mut reticle_transform) = reticle.single_mut() {
        reticle_transform.scale = Vec3::splat(SCOPE_DEFAULT_ZOOM / zoom.0);
    }

    if let Ok((mut transform, mut visibility)) = lens.single_mut() {
        // X and Z both sweep in from GUN_OFFSET to SCOPE_LENS_OFFSET with
        // ads_blend — same lateral + "pulled in closer" travel as the gun
        // mesh and the scope camera (sync_scope_camera). Y also dips down a
        // little below SCOPE_LENS_OFFSET.y while not aiming. Recoil kick and
        // reload dip/tilt stack on top the same way they do on the gun
        // mesh's own Transform.translation — this is a plain local-space
        // translation just like Gun's, so no right()/forward() conversion
        // is needed here (unlike sync_scope_camera, which has to rebuild a
        // world offset for the free-floating scope camera).
        let lens_x = GUN_OFFSET.x + (SCOPE_LENS_OFFSET.x - GUN_OFFSET.x) * gun.ads_blend;
        let recoil_t = gun.recoil / RECOIL_DURATION;
        let reload_t = gun.reload_remaining / RELOAD_DURATION;
        let dip = 4.0 * reload_t * (1.0 - reload_t);
        let bob_aim_scale = 1.0 - gun.ads_blend * (1.0 - AIM_BOB_MULTIPLIER);
        let bob_x = ops::sin(gun.bob_phase) * GUN_BOB_AMOUNT_X * gun.bob_blend * bob_aim_scale;
        let bob_y = ops::sin(gun.bob_phase * 2.0).abs()
            * GUN_BOB_AMOUNT_Y
            * gun.bob_blend
            * bob_aim_scale;
        let swapped_x = lens_x * (1.0 - 2.0 * gun.shoulder_blend) - RELOAD_LEFT_DIP * dip + bob_x;
        let lens_y = SCOPE_LENS_OFFSET.y - SCOPE_LENS_HIDDEN_DROP * (1.0 - gun.ads_blend)
            + SCOPE_HIP_HEIGHT_OFFSET * (1.0 - gun.ads_blend)
            + RECOIL_KICK_UP * recoil_t
            - RELOAD_DIP * dip
            - bob_y;
        let lens_z =
            GUN_OFFSET.z + (SCOPE_LENS_OFFSET.z - GUN_OFFSET.z) * gun.ads_blend + RECOIL_KICK * recoil_t;
        transform.translation = Vec3::new(swapped_x, lens_y, lens_z);
        transform.rotation = Quat::from_rotation_z(gun.look_sway_roll.to_radians())
            * Quat::from_rotation_y(RECOIL_YAW_KICK_DEG.to_radians() * gun.recoil_roll * recoil_t)
            * Quat::from_rotation_z(RECOIL_ROLL_DEG.to_radians() * gun.recoil_roll * recoil_t)
            * Quat::from_rotation_x((RELOAD_TILT_DEG * dip + RECOIL_TILT_UP_DEG * recoil_t).to_radians());
        *visibility = if visible { Visibility::Visible } else { Visibility::Hidden };
    }
}

// Drives the gun mesh's Transform from Gun's recoil/reload_remaining state —
// both are just countdown timers, this is the only system that turns them
// into an actual pose each frame (also where reload_remaining hitting 0
// refills the magazine).
fn gun_view_animation(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    touch_aiming: Res<TouchAiming>,
    game_over: Res<GameOver>,
    look_delta: Res<MouseLookDelta>,
    cursor: Query<&CursorOptions, With<PrimaryWindow>>,
    camera: Query<&Transform, (With<FpsCamera>, Without<Gun>)>,
    spatial_query: SpatialQuery,
    mut query: Query<(&mut Transform, &mut Gun), Without<FpsCamera>>,
) {
    let aiming = cursor.single().is_ok_and(|cursor| is_aiming(cursor, &mouse, touch_aiming.0));
    let shoulder_target = if !game_over.0 && keys.pressed(KeyCode::KeyQ) { 1.0 } else { 0.0 };
    // Wall-raise probe: how close is the wall directly in front of the
    // camera? 0.0 = clear, 1.0 = right up against it.
    let wall_target = camera.single().ok().map_or(0.0, |camera_transform| {
        let filter = SpatialQueryFilter::default();
        spatial_query
            .cast_ray(
                camera_transform.translation,
                camera_transform.forward(),
                WALL_GUN_RAISE_DISTANCE,
                true,
                &filter,
            )
            .map_or(0.0, |hit| {
                1.0 - (hit.distance / WALL_GUN_RAISE_DISTANCE).clamp(0.0, 1.0)
            })
    });
    for (mut transform, mut gun) in &mut query {
        if gun.recoil > 0.0 {
            gun.recoil = (gun.recoil - time.delta_secs()).max(0.0);
        }
        if gun.reload_remaining > 0.0 {
            gun.reload_remaining = (gun.reload_remaining - time.delta_secs()).max(0.0);
            if gun.reload_remaining == 0.0 {
                gun.ammo = MAGAZINE_SIZE;
            }
        }

        let ads_target = if aiming { 1.0 } else { 0.0 };
        let ads_step = (GUN_ADS_SPEED * time.delta_secs()).min(1.0);
        gun.ads_blend += (ads_target - gun.ads_blend) * ads_step;

        let wall_raise_step = (WALL_GUN_RAISE_SPEED * time.delta_secs()).min(1.0);
        gun.wall_raise += (wall_target - gun.wall_raise) * wall_raise_step;

        let shoulder_step = (LEAN_SPEED * time.delta_secs()).min(1.0);
        gun.shoulder_blend += (shoulder_target - gun.shoulder_blend) * shoulder_step;

        let moving = is_moving(&keys);
        let bob_target = if moving { 1.0 } else { 0.0 };
        let bob_blend_step = (GUN_BOB_BLEND_SPEED * time.delta_secs()).min(1.0);
        gun.bob_blend += (bob_target - gun.bob_blend) * bob_blend_step;
        if moving {
            gun.bob_phase += GUN_BOB_SPEED * time.delta_secs();
        }
        let bob_aim_scale = 1.0 - gun.ads_blend * (1.0 - AIM_BOB_MULTIPLIER);
        let bob_x = ops::sin(gun.bob_phase) * GUN_BOB_AMOUNT_X * gun.bob_blend * bob_aim_scale;
        let bob_y =
            ops::sin(gun.bob_phase * 2.0).abs() * GUN_BOB_AMOUNT_Y * gun.bob_blend * bob_aim_scale;

        // Sign flipped from the initial guess — reported backwards in-game.
        let sway_target =
            (look_delta.0.x * LOOK_SWAY_SENSITIVITY).clamp(-LOOK_SWAY_MAX_DEG, LOOK_SWAY_MAX_DEG);
        let sway_step = (LOOK_SWAY_SPEED * time.delta_secs()).min(1.0);
        gun.look_sway_roll += (sway_target - gun.look_sway_roll) * sway_step;

        let recoil_t = gun.recoil / RECOIL_DURATION;
        let reload_t = gun.reload_remaining / RELOAD_DURATION;
        // Parabola: 0 at both ends of the reload, 1 at the midpoint — the
        // gun dips down/tilts away and comes back up as the reload finishes.
        let dip = 4.0 * reload_t * (1.0 - reload_t);

        let base_pos = gun.base_offset.lerp(GUN_ADS_OFFSET, gun.ads_blend);
        // Holding the lean-left key swaps the gun over to the left shoulder
        // by mirroring its X offset, instead of just nudging it —
        // shoulder_blend eases 0 (right shoulder, normal) -> 1 (fully
        // mirrored to the left). Leaning right keeps the normal right-
        // shoulder pose.
        let swapped_x = base_pos.x * (1.0 - 2.0 * gun.shoulder_blend);
        transform.translation = Vec3::new(swapped_x, base_pos.y, base_pos.z)
            + Vec3::new(
                bob_x,
                RECOIL_KICK_UP * recoil_t - RELOAD_DIP * dip - bob_y
                    + WALL_GUN_RAISE_HEIGHT * gun.wall_raise,
                RECOIL_KICK * recoil_t,
            );
        // Recoil tilt is applied as the outer rotation (camera-local space),
        // not composed inside the model's own base orientation like the
        // reload tilt is — doing it inside meant the -90° base Y rotation
        // twisted "tilt around local X" into something closer to a sideways
        // roll than an up/down pitch (reported as the gun leaning right on
        // recoil instead of the muzzle rising).
        let base_rotation = Quat::from_rotation_y(GUN_ROTATION_Y_DEG.to_radians())
            * Quat::from_rotation_x(RELOAD_TILT_DEG.to_radians() * dip);
        // Sign flipped: the previous sign raised the stock/rear instead of
        // the muzzle.
        // Same outer/camera-space composition as the recoil tilt above, same
        // sign so it pitches the muzzle up rather than down.
        let wall_raise_tilt = WALL_GUN_RAISE_ROT_DEG.to_radians() * gun.wall_raise;
        transform.rotation = Quat::from_rotation_x(wall_raise_tilt)
            * Quat::from_rotation_z(gun.look_sway_roll.to_radians())
            * Quat::from_rotation_y(RECOIL_YAW_KICK_DEG.to_radians() * gun.recoil_roll * recoil_t)
            * Quat::from_rotation_z(RECOIL_ROLL_DEG.to_radians() * gun.recoil_roll * recoil_t)
            * Quat::from_rotation_x(RECOIL_TILT_UP_DEG.to_radians() * recoil_t)
            * base_rotation;
    }
}

fn despawn_hit_markers(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut HitMarker)>,
) {
    for (entity, mut marker) in &mut query {
        marker.remaining -= time.delta_secs();
        if marker.remaining <= 0.0 {
            commands.entity(entity).try_despawn();
        }
    }
}

fn despawn_muzzle_flashes(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut MuzzleFlash)>,
) {
    for (entity, mut flash) in &mut query {
        flash.remaining -= time.delta_secs();
        if flash.remaining <= 0.0 {
            commands.entity(entity).try_despawn();
        }
    }
}

// Puffs drift up/forward and swell as they age, then vanish once their
// lifetime runs out.
fn update_smoke(mut commands: Commands, time: Res<Time>, mut query: Query<(Entity, &mut Transform, &mut Smoke)>) {
    let dt = time.delta_secs();
    for (entity, mut transform, mut smoke) in &mut query {
        transform.translation += smoke.velocity * dt;
        transform.scale *= 1.0 + SMOKE_GROWTH_PER_SEC * dt;

        smoke.remaining -= dt;
        if smoke.remaining <= 0.0 {
            commands.entity(entity).try_despawn();
        }
    }
}

// Zombie AI/pathfinding/movement (big-brain + vleue_navigator +
// move_zombies/steer_around_obstacles) now live in crates/server —
// zombies are server-authoritative, replicated down as NetTransform.
//
// This client only decides its own render-cost LOD tier per zombie —
// distinct from (and simpler than) the server's movement-cost LOD, but
// reusing the same ZOMBIE_LOD_DISTANCE threshold — since ZombieMotion.far is
// still what freeze_zombie_animations/update_zombie_health_bars key off of.
fn update_zombie_lod(
    mut timings: ResMut<PerfTimings>,
    lod_disabled: Res<ZombieLodDisabled>,
    player: Query<&Transform, With<Player>>,
    mut zombies: Query<(&Transform, &mut ZombieMotion), (With<Zombie>, Without<Player>)>,
    time: Res<Time>,
    mut elapsed: Local<f32>,
) {
    // No dt-dependent math here (just a per-zombie distance threshold), so
    // unlike zombies_damage_player this doesn't need accumulated-dt
    // bookkeeping to stay correct — motion.far just ends up up to
    // 1/INPUT_SEND_HZ stale, imperceptible for a render-cost LOD tier.
    *elapsed += time.delta_secs();
    if *elapsed < 1.0 / INPUT_SEND_HZ {
        return;
    }
    *elapsed = 0.0;
    let perf_start = std::time::Instant::now();
    let Ok(player_transform) = player.single() else {
        return;
    };
    let mut near_count = 0usize;
    let mut total = 0usize;
    for (zombie_transform, mut motion) in &mut zombies {
        total += 1;
        let distance = zombie_transform.translation.distance(player_transform.translation);
        motion.far = !lod_disabled.0 && distance > ZOMBIE_LOD_DISTANCE;
        if !motion.far {
            near_count += 1;
        }
    }
    timings.move_zombies_ms = perf_start.elapsed().as_secs_f32() * 1000.0;
    timings.zombie_count = total;
    timings.near_zombie_count = near_count;
}

// Ticks down each pending respawn timer (queued by shoot() when a zombie
// dies) and spawns a replacement once it hits zero, cycling through
// ZOMBIE_SPAWNS so the population stays at 3 instead of thinning out.
// The zombie scene's AnimationPlayer only exists once its glTF finishes
// loading (async), so this watches for new ones, wires up the shared
// AnimationGraph, starts them on Idle, and records which zombie root each
// player belongs to (by walking up the spawned hierarchy) for
// update_zombie_animation to use later.
// Walks up from a newly-spawned AnimationPlayer through ChildOf looking for
// an ancestor matching `is_root`, up to 20 hops (glTF scenes nest deeply).
// Shared by wire_zombie_animations/wire_player_model_animations so each only
// has to say what root type it's looking for, not re-implement the walk.
fn find_animation_root(entity: Entity, child_of: &Query<&ChildOf>, is_root: impl Fn(Entity) -> bool) -> Option<Entity> {
    let mut current = entity;
    for _ in 0..20 {
        if is_root(current) {
            return Some(current);
        }
        current = child_of.get(current).ok()?.0;
    }
    None
}

// Determining the root type before touching anything (rather than assigning
// ZombieAnim's graph unconditionally, the way this used to work back when
// zombies were the only animated character in the scene) matters now that
// wire_player_model_animations runs against the exact same
// Added<AnimationPlayer> query — assigning first and checking ancestry after
// would let both systems clobber each other's graph on whichever entity
// type they don't actually own.
fn wire_zombie_animations(
    mut commands: Commands,
    anim: Res<ZombieAnim>,
    child_of: Query<&ChildOf>,
    zombies: Query<(), With<Zombie>>,
    mut new_players: Query<(Entity, &mut AnimationPlayer), Added<AnimationPlayer>>,
) {
    for (player_entity, mut player) in &mut new_players {
        let Some(root) = find_animation_root(player_entity, &child_of, |e| zombies.contains(e)) else {
            continue;
        };
        // try_insert, not insert: with 100 zombies all loading their glTF
        // scenes at once, one can despawn (killed instantly, or the
        // respawn queue replacing it) between this query running and the
        // command actually applying — same benign race chase_action_system
        // already works around below. Plain insert() surfaced it as a
        // "entity despawned" warning, once per zombie, right at startup.
        commands
            .entity(player_entity)
            .try_insert(AnimationGraphHandle(anim.graph.clone()));
        player.play(anim.idle).repeat();
        commands.entity(root).try_insert(ZombieAnimationPlayer(player_entity));
    }
}

// Mirror of wire_zombie_animations for PLAYER_MODEL_PATH — covers both the
// local debug PlayerModel and every remote NetPlayer's model child, since
// both carry PlayerModelAnimRoot (see its own doc comment).
fn wire_player_model_animations(
    mut commands: Commands,
    anim: Res<PlayerModelAnim>,
    child_of: Query<&ChildOf>,
    roots: Query<(), With<PlayerModelAnimRoot>>,
    mut new_players: Query<(Entity, &mut AnimationPlayer), Added<AnimationPlayer>>,
) {
    for (player_entity, mut player) in &mut new_players {
        let Some(root) = find_animation_root(player_entity, &child_of, |e| roots.contains(e)) else {
            continue;
        };
        commands
            .entity(player_entity)
            .try_insert(AnimationGraphHandle(anim.graph.clone()));
        player.play(anim.idle).repeat();
        commands
            .entity(root)
            .try_insert((PlayerModelAnimationPlayer(player_entity), PlayerModelPose::default()));
    }
}

// Remote/bot player models: driven by the parent NetPlayer entity's
// replicated NetTransform.moving (see move_players/move_debug_bots on the
// server, which now actually set it) — same signal update_zombie_animation
// already uses for zombies. Doesn't touch the local debug model (it has no
// ChildOf, so it never matches this query — see
// update_local_player_model_animation instead).
fn update_remote_player_model_animation(
    parents: Query<&NetTransform, With<NetPlayer>>,
    mut roots: Query<
        (&ChildOf, &PlayerModelAnimationPlayer, &mut PlayerModelPose),
        (With<PlayerModelAnimRoot>, Without<PlayerModelDead>),
    >,
    anim: Res<PlayerModelAnim>,
    mut players: Query<&mut AnimationPlayer>,
) {
    for (child_of, link, mut pose) in &mut roots {
        let Ok(net_transform) = parents.get(child_of.0) else {
            continue;
        };
        // sprinting alone doesn't imply moving (holding Shift while stood
        // still is a real, if unusual, input) — gate Run on both, same as
        // the sprint boolean itself only being computed while genuinely
        // trying to move on the sending client (send_player_input's own
        // `sprinting` local).
        let new_pose = if net_transform.moving && net_transform.sprinting {
            PlayerModelPose::Run
        } else if net_transform.moving {
            PlayerModelPose::Walk
        } else {
            PlayerModelPose::Idle
        };
        if new_pose == *pose {
            continue;
        }
        *pose = new_pose;
        if let Ok(mut player) = players.get_mut(link.0) {
            player.stop_all();
            let node = match new_pose {
                PlayerModelPose::Idle => anim.idle,
                PlayerModelPose::Walk => anim.walk,
                PlayerModelPose::Run => anim.run,
            };
            player.play(node).repeat();
        }
    }
}

// The local debug model has no NetPlayer parent to read NetTransform.moving
// from (it's a standalone entity, see PlayerModel's own doc comment) — reads
// LocalPlayerMoving instead, which send_player_input sets from this frame's
// own move_axis directly (no need to wait on a replication round trip for
// something only this client ever sees anyway).
fn update_local_player_model_animation(
    moving: Res<LocalPlayerMoving>,
    sprinting: Res<LocalPlayerSprinting>,
    mut roots: Query<(&PlayerModelAnimationPlayer, &mut PlayerModelPose), (With<PlayerModelAnimRoot>, With<PlayerModel>)>,
    anim: Res<PlayerModelAnim>,
    mut players: Query<&mut AnimationPlayer>,
) {
    let Ok((link, mut pose)) = roots.single_mut() else {
        return;
    };
    let new_pose = if moving.0 && sprinting.0 {
        PlayerModelPose::Run
    } else if moving.0 {
        PlayerModelPose::Walk
    } else {
        PlayerModelPose::Idle
    };
    if new_pose == *pose {
        return;
    }
    *pose = new_pose;
    if let Ok(mut player) = players.get_mut(link.0) {
        player.stop_all();
        let node = match new_pose {
            PlayerModelPose::Idle => anim.idle,
            PlayerModelPose::Walk => anim.walk,
            PlayerModelPose::Run => anim.run,
        };
        player.play(node).repeat();
    }
}

// Broadcast the instant ANY client fires (see PlayerFiredFx's own doc
// comment in crate::shared for why no such signal existed before this
// pass) — finds that specific shooter's own NetPlayer entity via its
// replicated NetworkId and plays a brief, non-looping Gun_Shoot pose on
// their third-person model, for every OTHER client watching (the shooter's
// own client already knows it fired locally and has no third-person view
// of itself in normal play). A linear scan over connected players/roots is
// fine at this player count — same shape as the local/remote animation
// systems above, no indexed lookup infrastructure exists for this either.
fn on_player_fired_fx(
    trigger: On<PlayerFiredFx>,
    net_players: Query<(Entity, &NetworkId), With<NetPlayer>>,
    roots: Query<(&ChildOf, &PlayerModelAnimationPlayer), (With<PlayerModelAnimRoot>, Without<PlayerModelDead>)>,
    anim: Res<PlayerModelAnim>,
    mut players: Query<&mut AnimationPlayer>,
) {
    let Some((shooter, _)) = net_players.iter().find(|(_, id)| id.get() == trigger.shooter_network_id) else {
        return;
    };
    let Some((_, link)) = roots.iter().find(|(child_of, _)| child_of.0 == shooter) else {
        return;
    };
    if let Ok(mut player) = players.get_mut(link.0) {
        // Not .repeat() — a one-shot pose. update_remote_player_model_animation
        // will naturally take back over and restart Idle/Walk/Run the next
        // time this player's NetTransform.moving/sprinting actually changes;
        // until then this just plays once and holds its last frame, which
        // reads fine for a quick recoil-of-the-arms gesture.
        player.play(anim.shoot);
    }
}

// Same shooter-lookup shape as on_player_fired_fx above, but marks the
// player's model PlayerModelDead first so the ordinary Idle/Walk/Run
// system (which keeps running — NetTransform doesn't stop replicating just
// because this player died) doesn't cut the Death pose short.
fn on_player_died_fx(
    trigger: On<PlayerDiedFx>,
    mut commands: Commands,
    net_players: Query<(Entity, &NetworkId), With<NetPlayer>>,
    roots: Query<(Entity, &ChildOf, &PlayerModelAnimationPlayer), With<PlayerModelAnimRoot>>,
    anim: Res<PlayerModelAnim>,
    mut players: Query<&mut AnimationPlayer>,
) {
    let Some((who, _)) = net_players.iter().find(|(_, id)| id.get() == trigger.who_network_id) else {
        return;
    };
    let Some((root, _, link)) = roots.iter().find(|(_, child_of, _)| child_of.0 == who) else {
        return;
    };
    commands.entity(root).insert(PlayerModelDead);
    if let Ok(mut player) = players.get_mut(link.0) {
        player.stop_all();
        player.play(anim.death);
    }
}

// Tags every new Mesh3d that turns out to be a descendant of the Gun entity
// with GunMesh, once the ak47_scope.glb scene finishes loading (same async
// timing problem as wire_zombie_animations/wire_zombie_hitbox_bones) — see
// GunMesh's doc comment for why this needs to exist.
fn wire_gun_mesh(
    mut commands: Commands,
    child_of: Query<&ChildOf>,
    gun: Query<(), With<Gun>>,
    new_meshes: Query<Entity, Added<Mesh3d>>,
) {
    for mesh_entity in &new_meshes {
        let mut current = mesh_entity;
        for _ in 0..20 {
            if gun.contains(current) {
                commands
                    .entity(mesh_entity)
                    .insert((GunMesh, RenderLayers::layer(VIEWMODEL_LAYER)));
                break;
            }
            let Ok(parent) = child_of.get(current) else {
                break;
            };
            current = parent.0;
        }
    }
}

// Switches a zombie between its Walk and Idle clips exactly when `Chase` is
// added/removed, rather than every frame.
// Switches a zombie between its Walk and Idle clips based on the server's
// replicated NetTransform.moving flag (Chase itself isn't replicated — see
// NetTransform's own comment on why).
// All 5 of zombie.glb's clips, picked by priority (highest first) from
// state this client already computes/has locally — no new replication
// needed, same "each client checks distance/Health itself" precedent
// zombies_damage_player already set for the identical ZOMBIE_STOP_DISTANCE
// melee check:
//   1. Bite  — within ZOMBIE_STOP_DISTANCE of the local player (the same
//      range zombies_damage_player uses to actually deal damage) —
//      overrides everything else while in melee range, wounded or not.
//   2. Crawl — Health at/below ZOMBIE_CRAWL_HEALTH_FRACTION of max, not
//      currently biting — a "nearly dead" telegraph with no prior
//      precedent in this codebase (there was no wounded-state concept
//      before this).
//   3. Run   — moving (NetTransform.moving) and within ZOMBIE_RUN_DISTANCE
//      of the player, not crawling/biting — a "closing in" tension beat.
//   4. Walk  — moving, none of the above.
//   5. Idle  — not moving, none of the above.
fn update_zombie_animation(
    anim: Res<ZombieAnim>,
    player: Query<&Transform, With<Player>>,
    mut zombies: Query<(&Transform, &NetTransform, &Health, &ZombieAnimationPlayer, &mut ZombiePose), With<Zombie>>,
    mut players: Query<&mut AnimationPlayer>,
) {
    let Ok(player_transform) = player.single() else {
        return;
    };
    for (zombie_transform, net_transform, health, link, mut pose) in &mut zombies {
        let distance = zombie_transform.translation.distance(player_transform.translation);
        let new_pose = if distance <= ZOMBIE_STOP_DISTANCE {
            ZombiePose::Bite
        } else if health.0 / ZOMBIE_MAX_HEALTH <= ZOMBIE_CRAWL_HEALTH_FRACTION {
            ZombiePose::Crawl
        } else if net_transform.moving && distance <= ZOMBIE_RUN_DISTANCE {
            ZombiePose::Run
        } else if net_transform.moving {
            ZombiePose::Walk
        } else {
            ZombiePose::Idle
        };
        if new_pose == *pose {
            continue;
        }
        *pose = new_pose;
        if let Ok(mut player) = players.get_mut(link.0) {
            player.stop_all();
            let node = match new_pose {
                ZombiePose::Idle => anim.idle,
                ZombiePose::Walk => anim.walk,
                ZombiePose::Run => anim.run,
                ZombiePose::Crawl => anim.crawl,
                ZombiePose::Bite => anim.bite,
            };
            player.play(node).repeat();
        }
    }
}

// The C entry point gen/apple/Sources/client/main.mm actually calls into —
// see mobile_scaffold_test's own generated lib.rs template (cargo-mobile2's
// bevy template), copied verbatim.
#[unsafe(no_mangle)]
#[inline(never)]
#[cfg(target_os = "ios")]
pub extern "C" fn start_app() {
    main();
}

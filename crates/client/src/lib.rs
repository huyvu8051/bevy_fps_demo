use avian3d::prelude::*;
use bevy::animation::graph::AnimationGraph;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::RenderTarget;
use bevy::ecs::system::SystemParam;
use bevy::input::mouse::{MouseMotion, MouseScrollUnit, MouseWheel};
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
    ClearAllZombies, ClientInput, DebugBot, DespawnDebugBot, find_animation_root, FireShot, GameOver, GameStateEvent, Health,
    HEAD_DAMAGE, HITBOX_BELLY_MIN_Y, HITBOX_CHEST_MIN_Y, HITBOX_HEAD_MIN_Y, HITBOX_LIMB_LATERAL_MIN,
    KnockbackVelocity, LastProcessedInput, MoveDebugBot, NetPlayer, NetTransform, Player, PlayerDied,
    ZombieAnimationPlayer,
    PlayerDiedFx, PlayerFiredFx,
    PlayerStance, RequestRestart, Scenario, SetAutoMaintainZombies, SetFreezeZombies, SetZombieCount, ShotConfirmed, ShotFx, SpawnDebugBot,
    SpawnMoreZombies, Zombie, ZombieAttackTimer, ZombieHitboxBones, ZombieMotion, wire_zombie_hitbox_bones,
    INTERP_DELAY_SECS, MAX_SHOT_DISTANCE,
    HEAD_HITBOX_RADIUS, LIMB_HITBOX_RADIUS, TORSO_HITBOX_RADIUS,
    resolve_bone_world_positions, zombie_hit_damage_ray,
    NAVMESH_COLUMN_OBSTACLES, PILLAR_RADIUS, PROTOCOL_ID, ZOMBIE_ANIM_BITE, ZOMBIE_ANIM_CRAWL, ZOMBIE_ANIM_IDLE, ZOMBIE_ANIM_RUN, ZOMBIE_ANIM_WALK,
    ZOMBIE_ATTACK_INTERVAL, ZOMBIE_LOD_DISTANCE, ZOMBIE_MODEL_Y_OFFSET, ZOMBIE_OUTER_HEIGHT, ZOMBIE_OUTER_RADIUS, ZOMBIE_PATH,
    zombie_pose_for, ZombiePose, ZOMBIE_MAX_HEALTH, ZOMBIE_SCALE, ZOMBIE_SPAWNS, ZOMBIE_STOP_DISTANCE,
    ARENA_HALF_SIZE, ARENA_WALL_HEIGHT, ARENA_WALL_THICKNESS,
};
use shared::sim_latency::{sim_latency_from_env, LaggedSocket};

// Experimental CADisplayLink-driven entry point for MINIMAL_MAP_PROBE — see
// its own module doc comment. iOS-only; the extern "C" functions in
// cadisplaylink::ffi are the only way into it, called from a separate
// Xcode target's Swift code, never from the normal winit-driven app.
#[cfg(target_os = "ios")]
mod cadisplaylink;

// AI takeover of the local player (auto-aim/move/fire) — see this module's
// own doc comment for the full picture.
mod autopilot;
use autopilot::{autopilot_pick_target, autopilot_think, Autopilot, AutopilotIntent, AutopilotLookOnly, AutopilotState};

// Touch/mobile input handling (joystick, on-screen buttons, drag-to-look) —
// see this module's own doc comment. Phase 1 of splitting lib.rs's
// gameplay code into topic files (pure code motion — the master Update
// system `.chain()` below is unchanged).
mod touch_input;
use touch_input::*;

// Settings menu / cheat panel UI — see this module's own doc comment.
// Phase 2 of splitting lib.rs's gameplay code into topic files.
mod settings_menu;
use settings_menu::*;

// Movement/camera/combat pipeline — see this module's own doc comment.
// Phase 3 of splitting lib.rs's gameplay code into topic files.
mod movement_combat;
use movement_combat::*;

// Zombie + player-model client-side representation — see this module's
// own doc comment. Phase 4 of splitting lib.rs's gameplay code into
// topic files.
mod zombie_client;
use zombie_client::*;

// VFX: scope camera, hit feedback, particles/decals, gun view animation
// — see this module's own doc comment. Phase 5 of splitting lib.rs's
// gameplay code into topic files.
mod vfx;
use vfx::*;

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
// ARENA_HALF_SIZE/ARENA_WALL_HEIGHT/ARENA_WALL_THICKNESS now imported from
// shared::arena (see spawn_test_arena) — this used to be its own local,
// never-synced copy that silently drifted from the server's authoritative
// arena size (crates/server's spawn_arena_colliders, which always used the
// shared constant), so the client's own rendered/collidable floor+walls
// stayed frozen at the old size while the server's real world — and the
// player's own authoritative spawn point — moved to the new one. That's
// what "spawning outside the map" was: PLAYER_SPAWN below (this crate)
// used to compute off the stale local value while the server placed the
// player using the correct, larger one.

// Swaps in the Map 1 "Tòa nhà văn phòng" — Tầng B1 (Gara) greybox
// blockout (docs/maps/map01_highrise.md, crates/shared's
// map1_b1_garage.rs) in place of the generic USE_TEST_ARENA square room —
// checked FIRST, so USE_TEST_ARENA's own bool only matters when this is
// false. Temporary dev flag for trying out the real Map 1 layout; flip
// back to false to go back to the plain arena. PLAYER_SPAWN below also
// branches on this.
const USE_MAP1_B1_GARAGE: bool = true;

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

// Bullet tracer: a thin glowing streak drawn from muzzle to impact, shown on
// only a fraction of shots — same idea as tracer rounds loaded a few-in-ten
// into a real magazine, not every bullet. TRACER_CHANCE is the fraction that
// gets one (0.5 = roughly every other shot, via the same sin-based
// deterministic "randomness" the rest of this file already uses for spread/
// recoil jitter — see spawn_bullet_tracer's own doc comment). Kept alive
// only a few frames (TRACER_LIFETIME) — a tracer is meant to read as a fast
// streak, not linger like a laser beam.
const TRACER_CHANCE: f32 = 0.5;
const TRACER_LIFETIME: f32 = 0.045;
const TRACER_RADIUS: f32 = 0.012;
// Tracer-only nudge below muzzle_world_pos — the tracer's start point was
// reading as coming from the front sight instead of the barrel itself.
// Deliberately not folded into MUZZLE_OFFSET (which also positions the
// flash/smoke quads, unaffected by this report) — this only lowers where
// the tracer streak starts.
const TRACER_START_Y_OFFSET: f32 = -0.05;

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
// Vertical scope-zoom slider (only relevant while ADS'd — desktop's
// equivalent is the scroll wheel, see adjust_scope_zoom) — a track the
// player drags up/down, dragging up = more zoom (SCOPE_MAX_ZOOM at the
// top), down = less (SCOPE_MIN_ZOOM at the bottom), same "drag a handle
// along a track" shape as the joystick above but 1D instead of 2D.
const TOUCH_SCOPE_ZOOM_TRACK_WIDTH_PX: f32 = 44.0;
const TOUCH_SCOPE_ZOOM_TRACK_HEIGHT_PX: f32 = 200.0;
const TOUCH_SCOPE_ZOOM_HANDLE_PX: f32 = 40.0;
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


impl Default for TouchControlsEnabled {
    fn default() -> Self {
        let mobile = cfg!(target_os = "ios") || cfg!(target_os = "android");
        Self(mobile || std::env::var("TOUCH_UI_PREVIEW").is_ok())
    }
}










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
// While autopilot has a zombie locked, the scope auto-zooms to frame it
// instead of taking scroll-wheel input: at AUTO_ZOOM_NEAR_DISTANCE or
// closer the target is already big in view so zoom stays at
// SCOPE_MIN_ZOOM, at AUTO_ZOOM_FAR_DISTANCE or beyond it ramps up to
// SCOPE_MAX_ZOOM, linearly interpolated in between.
const AUTO_ZOOM_NEAR_DISTANCE: f32 = 10.0;
const AUTO_ZOOM_FAR_DISTANCE: f32 = 100.0;
// Guessed placement for the lens quad in the same camera-local space as
// GUN_ADS_OFFSET/MUZZLE_OFFSET (can't see the model to calibrate exactly) —
// closer to the camera than GUN_ADS_OFFSET.z so it renders in front of the
// gun mesh instead of getting clipped inside it.
const SCOPE_LENS_OFFSET: Vec3 = Vec3::new(0.0, 0.0, -0.2);
// A forced-ADS diagnostic screenshot (bypassing the real aim input path)
// made 0.03 look like a barely-visible speck and briefly looked like a fix
// was needed — but confirmed correct via actually playing the desktop
// build normally (real ADS input, not forced): 0.03 is the right size
// after all. Left at its original value.
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
const PLAYER_SPAWN: Vec3 = if USE_MAP1_B1_GARAGE {
    // Map 1 B1 Garage greybox — "Khu A" spawn corner near the entrance,
    // see shared::GARAGE_PLAYER_SPAWN / crates/shared/src/map1/garage.rs.
    shared::GARAGE_PLAYER_SPAWN
} else if USE_TEST_ARENA {
    // Arena center — matches shared::player::PLAYER_SPAWN (used by the
    // server, the actual authority on where the player really is), so
    // zombie distance (ZOMBIE_SPAWNS, spread across the whole arena) ends
    // up roughly symmetric in every direction instead of closer on
    // whichever edge the player used to spawn near.
    Vec3::new(0.0, 3.0, 0.0)
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

// ZOMBIE_PATH/ZOMBIE_SCALE now live in crates/shared — see ZOMBIE_PATH's
// own doc comment for why (crates/server needs the exact same file/index
// set for its own real skeleton, not just this crate's visual one).
// ZOMBIE_MODEL_Y_OFFSET now lives in crates/shared — crates/server's own
// skeleton child (on_zombie_spawned, for real bone hit detection) needs the
// exact same scale+offset the visible mesh uses, or its bones end up ~4x
// too far from the root (unscaled) and nowhere near where the outer
// capsule/visible mesh actually are. See its own doc comment.

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
const PLAYER_MODEL_PATH: &str = "characters/soldier/mesh/soldier.glb";

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

// ZOMBIE_ANIM_BITE/CRAWL/IDLE/RUN/WALK now live in crates/shared (see their
// own doc comment) — all 5 are wired up (see ZombiePose/
// update_zombie_animation) — previously only Idle/Walk were used, leaving
// Bite/Crawl/Run dead assets despite shipping in the model.
// ZombiePose/ZOMBIE_CRAWL_HEALTH_FRACTION/ZOMBIE_RUN_DISTANCE/
// zombie_pose_for now live in crates/shared — crates/server drives its own
// AnimationPlayer through the same pose transitions for real hit-detection
// accuracy now, see zombie_pose_for's own doc comment for why this can't
// be two independently-drifting copies.
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






// Player, PlayerStance, Health, GameOver now live in crates/shared (plain
// gameplay data needed by both the future server and the client).

// Marks the "GAME OVER" overlay so restart_game can find and despawn it.
#[derive(Component)]
struct GameOverUi;

#[derive(Component)]
struct RestartButton;


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
struct InspectMode(bool);

// Debug/dev: toggled by KeyF. Freezes zombies in place (move_zombies and
// zombies_damage_player both early-return) so hitbox regions can be tested
// against a stationary target instead of a moving one.
#[derive(Resource, Default)]
struct FreezeZombies(bool);

// Debug/dev: toggled by KeyH. Draws the height-band/lateral-distance regions
// zombie_hit_damage uses to pick head/chest/belly/limb, via draw_zombie_hitboxes.
#[derive(Resource, Default)]
struct ShowHitboxes(bool);

// Debug/dev: toggled by settings/cheats or env SHOW_AUTOPILOT_TARGET. Shows the autopilot's red target aim sphere.
#[derive(Resource)]
struct ShowAutopilotTarget(bool);

impl Default for ShowAutopilotTarget {
    fn default() -> Self {
        Self(std::env::var("SHOW_AUTOPILOT_TARGET").is_ok_and(|v| v == "1" || v.to_lowercase() == "true"))
    }
}

// Debug/dev: toggled by settings/cheats or env SHOW_HIT_MARKERS. Enables/disables hit marker spheres on hits.
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

// Debug/dev: toggled by settings/cheats or env SHOW_BULLET_TRACERS. Gates
// spawn_bullet_tracer entirely — same on/off-toggle shape as ShowHitMarkers
// right above, just for the tracer streak instead of the hit-marker sphere.
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


impl Default for SettingsMenuVisible {
    fn default() -> Self {
        Self(std::env::var("OPEN_SETTINGS_MENU").is_ok())
    }
}

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
struct MapSelectionPending(bool);

impl Default for MapSelectionPending {
    fn default() -> Self {
        Self(std::env::var("MAP_ID").is_err())
    }
}





// How many SettingsMenuPage(N) containers spawn_settings_menu creates —
// settings_menu_page_buttons wraps SettingsMenuPageIndex against this so
// it never points past the last real page.
const SETTINGS_MENU_PAGE_COUNT: usize = 2;






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

// Marks the InspectMode coordinate readout so update_inspect_ui can find
// it — only shown while InspectMode is on (see that system). Exists so
// checking a newly-built floor's geometry against its own FloorLevel
// constants (elevation, wall lines, ramp gaps) is "fly there and read the
// numbers off screen" instead of guessing from the render alone.
#[derive(Component)]
struct InspectUi;




// A damage number that pops up at the hit point and floats/fades out.
// Tracks its own world position (drifting upward each frame) rather than a
// zombie Entity, since it should keep floating even if the zombie that took
// the hit dies and despawns a moment later.
#[derive(Component)]
struct FloatingDamageText {
    world_pos: Vec3,
    remaining: f32,
}






// Marks the ammo counter text so update_ammo_ui can find it.
#[derive(Component)]
struct AmmoUi;









// Zombie, ZombieMotion, KnockbackVelocity, ZombieAttackTimer, Chase now live
// in crates/shared (plain gameplay data needed by both the future server
// and the client).

// GameLayer, NearPlayer/near_player_scorer, ChaseAction/chase_action_system
// (big-brain Thinker AI) now live in crates/server — zombie AI/movement is
// server-authoritative.

// ZombieAnimationPlayer now lives in crates/shared — crates/server's own
// wire_server_zombie_animations needs the exact same root->AnimationPlayer
// link now that it drives real pose transitions too (see ZombiePose's own
// doc comment), not just a static Idle loop.

// --- Real navmesh (vleue_navigator, vendored fork) ---
//
// Auto-generating obstacles from Sponza's own Collider (NavmeshUpdaterPlugin
// slicing it at a fixed height) was tried and abandoned: that collider is a
// single merged trimesh for the whole building (~262k triangles across 103
// primitives), and slicing/triangulating it timed out even at 60s — shrinking
// the navmesh's outer bounds didn't help, since the slicing step runs over
// the full collider before bounds are ever applied. See spawn_zombie_navmesh
// for the hand-authored replacement.

// PROTOCOL_ID now lives in crates/shared — see its own doc comment.
// Fixed debug server address — matches crates/server's PORT. Override with
// SERVER_ADDR (e.g. "192.168.1.5:5031") to point at another machine on the
// LAN; the common case is just starting both binaries on localhost.

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
fn despawn_replicated_on_disconnect(mut commands: Commands, entities: Query<Entity, Or<(With<Zombie>, With<NetPlayer>)>>) {
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
fn current_scenario() -> Scenario {
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
fn autopilot_enabled_by_default(scenario: Scenario) -> bool {
    !matches!(scenario, Scenario::RenderCompare | Scenario::ItemGallery)
}

fn spawn_client_connection(commands: &mut Commands, channels: &RepliconChannels) {
    // to_socket_addrs (not parse::<SocketAddr>) so this also accepts a DNS
    // hostname, not just a literal IP — needed for e.g. playit.gg's
    // assigned tunnel domain (foo.tun.ply.gg:PORT), which a plain
    // SocketAddr parse rejects outright since it never resolves DNS.
    // TEMPORARY — default host is this Mac's Tailscale IP for an on-device
    // test build. Only reachable while the test phone is on the same
    // Tailscale network. Port comes from current_scenario() — see its own
    // doc comment and shared::Scenario's for the map-select-menu ->
    // port-per-scenario routing this implements. SERVER_ADDR (full
    // host:port), if set, overrides both host AND scenario-based port
    // entirely — the explicit escape hatch for custom testing.
    let server_addr_str = std::env::var("SERVER_ADDR").unwrap_or_else(|_| {
        let host = std::env::var("SERVER_HOST").unwrap_or_else(|_| "100.97.13.68".to_string());
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
    // Dev-only level editor (bevy_editor_pls, vendored + patched for Bevy
    // 0.18 under third_party/) — off unless EDITOR=1 is set, so normal
    // play (and every other build target: iOS/Android/CADisplayLink)
    // never pulls it in. Desktop-only on purpose: it's for building Map 1
    // geometry on the dev Mac, not for on-device use.
    if std::env::var("EDITOR").is_ok() {
        app.add_plugins(bevy_editor_pls::EditorPlugin::default());
    }
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
        // LevelBox/LevelPanel/LevelTransform used to be registered here —
        // no longer replicated (see level_piece.rs's own doc comment):
        // this process now builds its own local copy of the map from
        // shared::map1_level_pieces/item_gallery_level_pieces (see
        // setup()), same data the server independently builds its
        // colliders from.
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
        .init_resource::<ZombieCountUi>()
        .init_resource::<AutoMaintainUi>()
        // Restored alongside the Update-schedule block above — this was
        // disabled in the same CPU-investigation commit (80de58d), not
        // pre-existing/intentional; Phase 6 needs real networking/
        // replication actually wired up for anything to spawn.
        // Held off until a map's actually been picked — see
        // MapSelectionPending's own doc comment and map_select_button,
        // which calls spawn_client_connection directly once one is.
        .add_systems(Startup, connect_to_server.run_if(|pending: Res<MapSelectionPending>| !pending.0))
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
        .add_observer(on_shot_confirmed)
        .add_observer(on_player_fired_fx)
        .add_observer(on_player_died_fx)
        .add_observer(on_game_state_event)
        // Scenario::StressTest (Map 2) auto-enables noclip — the player
        // spawns high above the arena specifically to observe a full
        // zombie horde from a safe distance, no manual toggle needed. Every
        // other scenario keeps Noclip's normal off-by-default.
        .insert_resource(Noclip(current_scenario() == Scenario::StressTest))
        .init_resource::<InspectMode>()
        .init_resource::<Autopilot>()
        .init_resource::<AutopilotLookOnly>()
        .init_resource::<AutopilotState>()
        .init_resource::<AutopilotIntent>()
        .init_resource::<FreezeZombies>()
        .init_resource::<ShowHitboxes>()
        .init_resource::<GodMode>()
        .init_resource::<ShowAutopilotTarget>()
        .init_resource::<ShowHitMarkers>()
        .init_resource::<ShowBulletTracers>()
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
        .init_resource::<ScopeCameraEnabled>()
        .init_resource::<SelectedDebugBot>()
        .init_resource::<LocalPlayerMoving>()
.init_resource::<LocalPlayerSprinting>()
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
        .init_resource::<KillCount>()
        .init_resource::<MapSelectionPending>()
        .init_resource::<RecentlyHitZombies>()
        .add_systems(Startup, (setup, force_game_over_on_start, spawn_map_select_ui_if_pending /*, log_window_resolution */).chain());
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
                (
                    cheat_toggle_buttons_3,
                    autopilot_toggle_button,
                    update_extra_toggle_colors,
                    cheat_spawn_zombie_button,
                    debug_bot_buttons,
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
                // so count_local_kills/toggle_inspect_mode get their own outer
                // tuple slot instead of a 21st+ flat entry. Still fully
                // ordered: .chain() on this outer tuple runs the entire inner
                // chain before this.
                (count_local_kills, toggle_inspect_mode).chain(),
            )
                .chain(),
            (
                check_game_over,
                restart_game,
                restart_button,
                map_select_button,
                wire_zombie_animations,
                wire_zombie_hitbox_bones,
                wire_gun_mesh,
                spawn_level_box_visuals,
                spawn_level_panel_visuals,
                update_zombie_animation,
                freeze_zombie_animations,
                wire_player_model_animations,
                update_remote_player_model_animation,
                update_local_player_model_animation,
                update_health_ui,
                update_inspect_ui,
                update_compass_ui,
                update_zombie_health_bars,
                update_floating_damage_texts,
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
                update_debug_panel,
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
    tracer: Res<'w, TracerAssets>,
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

    commands.insert_resource(TracerAssets {
        mesh: meshes.add(Cylinder::new(1.0, 1.0)),
        material: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.85, 0.3),
            emissive: LinearRgba::rgb(10.0, 6.0, 1.0),
            unlit: true,
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

    if current_scenario() == Scenario::ItemGallery {
        // Flat scale-reference rig (crates/shared/src/item_gallery.rs) —
        // checked before USE_MAP1_B1_GARAGE below since that constant is
        // always true regardless of scenario.
        let (pieces, panels) = shared::item_gallery_level_pieces();
        spawn_level_pieces(&mut commands, pieces, panels);
    } else if USE_MAP1_B1_GARAGE {
        // Sàn/tường/dốc/bàn/phòng: build cục bộ từ đúng
        // shared::map1_level_pieces() server cũng gọi để dựng collider
        // của nó — xem level_piece.rs's module doc comment cho lý do
        // không còn replicate nữa. spawn_level_box_visuals/
        // spawn_level_panel_visuals bên dưới tự phản ứng với
        // Added<LevelBox>/Added<LevelPanel> để dựng mesh+collider.
        let (pieces, panels) = shared::map1_level_pieces();
        spawn_level_pieces(&mut commands, pieces, panels);
        spawn_map1_lights(&mut commands);
    } else if USE_TEST_ARENA {
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
    spawn_compass_ui(&mut commands);
    spawn_inspect_ui(&mut commands);
    spawn_debug_panel(&mut commands);
    spawn_settings_menu(&mut commands);
    spawn_settings_toggle_button(&mut commands);
    spawn_mouse_ui_lock_indicator(&mut commands);
    if touch_enabled.0 {
        spawn_touch_controls(&mut commands);
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






























const TOUCH_CONTROLS_IDLE_HIDE_SECS: f32 = 10.0;








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

    // Free-standing pillars — cover/obstacles for testing navigation and
    // line-of-sight (autopilot targeting, zombie pathfinding) instead of
    // the previously wide-open flat room. Same positions as crates/server's
    // spawn_arena_colliders and the navmesh cutout (column_footprint in
    // crates/server, driven by this same NAVMESH_COLUMN_OBSTACLES array),
    // so the client's visual/collider copy can't drift out of sync with
    // the server's authoritative one.
    let pillar_mesh = meshes.add(Cylinder::new(PILLAR_RADIUS, ARENA_WALL_HEIGHT));
    for &(x, z) in &NAVMESH_COLUMN_OBSTACLES {
        commands.spawn((
            Mesh3d(pillar_mesh.clone()),
            MeshMaterial3d(wall_material.clone()),
            Transform::from_xyz(x, ARENA_WALL_HEIGHT / 2.0, z),
            RigidBody::Static,
            Collider::cylinder(PILLAR_RADIUS, ARENA_WALL_HEIGHT),
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

// Map 1 "Tòa nhà văn phòng" — đèn tĩnh cho garage/lobby/F2/F3. Phần hình
// học sàn/tường/dốc/bàn/phòng KHÔNG còn dựng ở đây nữa — SERVER quyết
// định hết rồi replicate ra dưới dạng LevelBox (xem spawn_level_box_visuals
// bên dưới và doc comment ở crates/shared/src/level_piece.rs). Đèn tĩnh
// (PointLight) không nằm trong LevelBox (không có lý do gameplay/va chạm
// gì để gửi qua mạng), nên vẫn dựng ở client, đọc cùng hằng số vị trí
// shared::map1 mà server dùng để sinh hình học — thuần cosmetic, để ở
// local là an toàn.
fn spawn_map1_lights(commands: &mut Commands) {
    // B1 Garage — không cửa sổ, cần 1 đèn overhead sáng hoặc phòng sẽ tối om.
    commands.spawn((
        PointLight {
            color: Color::srgb(0.85, 0.88, 1.0),
            intensity: 3_000_000.0,
            range: (shared::GARAGE_HALF_SIZE_X + shared::GARAGE_HALF_SIZE_Z) * 1.5,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(0.0, ARENA_WALL_HEIGHT - 0.2, 0.0),
    ));

    // G Lobby — trần cao, không gian mở → 2 đèn thay vì 1.
    for &(lx, lz) in &[(-6.0, shared::LOBBY_CENTER_Z - 4.0), (6.0, shared::LOBBY_CENTER_Z + 4.0)] {
        commands.spawn((
            PointLight {
                color: Color::srgb(0.95, 0.93, 0.88),
                intensity: 4_500_000.0,
                range: (shared::LOBBY_HALF_SIZE_X + shared::LOBBY_HALF_SIZE_Z) * 1.2,
                shadows_enabled: true,
                ..default()
            },
            Transform::from_xyz(lx, shared::FLOOR_HEIGHT + shared::LOBBY_WALL_HEIGHT - 0.3, lz),
        ));
    }

    // F2 Office — trần thấp hơn Sảnh → 3 đèn dọc dãy cubicle để phủ đều.
    for &(lx, lz) in &[
        (0.0, shared::F2_NORTH_WALL_Z + 5.0),
        (0.0, shared::F2_CENTER_Z),
        (0.0, shared::F2_SOUTH_WALL_Z - 5.0),
    ] {
        commands.spawn((
            PointLight {
                color: Color::srgb(0.95, 0.95, 0.9),
                intensity: 2_500_000.0,
                range: shared::F2_HALF_SIZE_X * 1.5,
                shadows_enabled: true,
                ..default()
            },
            Transform::from_xyz(lx, shared::F2_ELEVATION + shared::F2_WALL_HEIGHT - 0.3, lz),
        ));
    }

    // F3 Office — hành lang dài hơn F2 → 4 đèn để phủ đều hết chiều dài
    // ("hành lang chính, dài, mở" theo mockup).
    for &(lx, lz) in &[
        (0.0, shared::F3_NORTH_WALL_Z + 5.0),
        (0.0, shared::F3_NORTH_WALL_Z + 14.0),
        (0.0, shared::F3_CENTER_Z + 4.0),
        (0.0, shared::F3_SOUTH_WALL_Z - 5.0),
    ] {
        commands.spawn((
            PointLight {
                color: Color::srgb(0.95, 0.9, 0.85),
                intensity: 2_500_000.0,
                range: shared::F3_HALF_SIZE_X * 1.5,
                shadows_enabled: true,
                ..default()
            },
            Transform::from_xyz(lx, shared::F3_ELEVATION + shared::F3_WALL_HEIGHT - 0.3, lz),
        ));
    }
}

// Phản ứng với bất kỳ LevelBox nào server replicate xuống (xem doc
// comment của component đó ở crates/shared/src/level_piece.rs) bằng cách
// tự dựng mesh + collider cục bộ cho nó — đây chính là điểm mấu chốt của
// LevelBox: CHỈ 1 hệ thống này lo hết mọi tầng Map 1 (và bất kỳ tầng nào
// sau này), nên sửa map không bao giờ cần build lại client trên bất kỳ
// nền tảng nào nữa. Mỗi box tạo 1 StandardMaterial riêng (tốn hơn dùng
// chung, vì nhiều box trùng màu) nhưng đơn giản và đúng — đáng cân nhắc
// lại nếu sau này scale lên nhiều hơn 1 map test.
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

// Same reactive idea as spawn_level_box_visuals, for the other shape —
// cull_mode: None so a thin flat panel doesn't disappear when seen from
// its back side. Collider is conditional on panel.collider_thickness
// (see LevelPanel's own doc comment): None stays pure decoration (walk
// through it), Some(t) gets a thin cuboid collider so it still blocks
// movement like a wall/floor.
//
// For a floor/ramp (panel.collider_offset == true), LevelTransform's
// translation is the VISIBLE surface (flush with where the player's feet
// actually are — "up" is always the same one direction, so this is safe
// to shift) rather than the collider's center; the collider goes on a
// CHILD entity offset -thickness/2 along local Z to land back where a
// LevelBox's collider always was. Skipping this was an earlier bug
// report ("bị hở" — floor meshes floating half a thickness below where
// the player actually stands).
//
// For a wall (collider_offset == false), translation IS already the
// collider's center, unshifted — a wall has exactly one side that needs
// to sit flush with the floor, but WHICH side differs per wall
// (north/south/east/west), so there's no single shift direction that's
// safe for every one; an earlier attempt to shift walls the same way as
// floors moved south/east walls off the floor's edge instead of onto it
// (see LevelPanel's own doc comment for the fuller story).
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

/// Spawns 1 LOCAL (no Replicated) `LevelBox`/`LevelPanel` entity per
/// piece from `shared::map1_level_pieces()`/`item_gallery_level_pieces()`
/// — the exact same data the server independently builds its colliders
/// from (see level_piece.rs's own doc comment). spawn_level_box_visuals/
/// spawn_level_panel_visuals above react to `Added<LevelBox>`/
/// `Added<LevelPanel>` regardless of where the entity came from, so they
/// pick these up and build mesh+collider the same way they used to for
/// server-replicated ones — no changes needed there.
fn spawn_level_pieces(commands: &mut Commands, pieces: Vec<shared::BoxPiece>, panels: Vec<shared::PanelPiece>) {
    for (translation, rotation, size, color, roughness) in pieces {
        commands.spawn((shared::LevelBox::new(size, color, roughness), shared::LevelTransform::new(translation, rotation)));
    }
    for (translation, rotation, width, height, color, roughness, collider_thickness, collider_offset) in panels {
        let panel = match collider_thickness {
            Some(thickness) if collider_offset => shared::LevelPanel::solid_surface(width, height, color, roughness, thickness),
            Some(thickness) => shared::LevelPanel::solid(width, height, color, roughness, thickness),
            None => shared::LevelPanel::new(width, height, color, roughness),
        };
        commands.spawn((panel, shared::LevelTransform::new(translation, rotation)));
    }
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

// Bottom-center so it doesn't collide with health (top-left)/ammo
// (bottom-right)/ping (top-right) — hidden by default, only meaningful
// while InspectMode is on.
fn spawn_inspect_ui(commands: &mut Commands) {
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
fn update_inspect_ui(
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

// --- PUBG-style top-of-screen compass ---
// A fixed-width "window" (COMPASS_WIDTH px, Overflow::clip()'d) onto a
// ring of 8 heading labels (N/NE/E/.../NW) parked at their real compass
// degree — update_compass_ui slides each one left/right every frame by
// how far its fixed heading is from the camera's CURRENT heading, so the
// strip scrolls under a stationary center tick exactly like a real
// compass tape, rather than the labels themselves moving in the world.
//
// "Heading" here is 0..360°, clockwise from North (screen-compass
// convention: N=0, E=90, S=180, W=270) — NOT the same number as
// FpsCamera.yaw (radians, positive = turning toward -X/"West" by
// Quat::from_rotation_y's own convention, see forward-vector call sites
// elsewhere in this file). heading_deg() below is the one conversion
// point between the two; nothing else in this file needs to know the
// mapping.
const COMPASS_WIDTH: f32 = 280.0;
const COMPASS_HEIGHT: f32 = 26.0;
// Screen px per degree of heading difference — 2.2 px/° over a 280px
// window shows a bit over ±63° of the horizon at once, enough to always
// have 2-3 marks in view without the strip feeling empty.
const COMPASS_PX_PER_DEG: f32 = 2.2;
// (heading_deg, label) — N first/red so it reads as the "primary" mark;
// order otherwise doesn't matter, each is positioned independently by
// its own heading_deg.
const COMPASS_MARKS: [(f32, &str); 8] =
    [(0.0, "N"), (45.0, "NE"), (90.0, "E"), (135.0, "SE"), (180.0, "S"), (225.0, "SW"), (270.0, "W"), (315.0, "NW")];

// cam.yaw's own convention: forward = Quat::from_rotation_y(yaw) * NEG_Z,
// so yaw=0 already faces "North" (-Z) here — heading is just that turned
// into a clockwise-from-North degree value (mouse-right decreases yaw,
// which should read as turning toward East/increasing heading, hence the
// negation; see this function's own module-level doc comment above).
fn heading_deg(yaw: f32) -> f32 {
    (-yaw.to_degrees()).rem_euclid(360.0)
}

// Shortest signed difference `a - b` wrapped into (-180, 180] — how far
// (and which way) to slide a mark whose fixed heading is `a` relative to
// the compass window currently centered on heading `b`.
fn heading_diff(a: f32, b: f32) -> f32 {
    ((a - b + 180.0).rem_euclid(360.0)) - 180.0
}

#[derive(Component)]
struct CompassMark(f32);

#[derive(Component)]
struct CompassHeadingText;

fn spawn_compass_ui(commands: &mut Commands) {
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

fn update_compass_ui(
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

    spawn_game_over_ui(&mut commands);
}

// Factored out of check_game_over so GAME_OVER_ON_START (see its own doc
// comment) can spawn the exact same overlay without needing a real death
// first — same launch-time-verification idea as OPEN_SETTINGS_MENU/
// TOUCH_UI_PREVIEW.
fn spawn_game_over_ui(commands: &mut Commands) {
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
fn force_game_over_on_start(mut commands: Commands, mut game_over: ResMut<GameOver>) {
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
struct MapSelectRoot;

// `.0` is the MAP_ID this button picks — see MAP_SELECT_ENTRIES.
#[derive(Component)]
struct MapSelectButton(u8);

// (MAP_ID, button title, one-line description) — keep in sync with
// Scenario::from_map_id (crates/shared/src/network.rs). ASCII/English only
// (not the original Swift menu's Vietnamese text) since this renders
// through native_metal_probe.rs's bitmap font on iOS too, which only has
// uppercase A-Z/digits/a handful of symbols — see glyph_index's own doc
// comment.
const MAP_SELECT_ENTRIES: [(u8, &str, &str); 6] = [
    (0, "GAME MAP", "OFFICE BUILDING (B1-F3)"),
    (1, "MAP 1", "HITBOX TEST - 5 STATIONARY ZOMBIES"),
    (2, "MAP 2", "STRESS TEST - 50 ZOMBIES, NOCLIP"),
    (3, "MAP 3", "RENDER COMPARE - 3 ZOMBIES, 3 POSES, AUTOPILOT OFF"),
    (4, "TEST ARENA", "LEGACY FLAT ARENA + PILLARS"),
    (5, "ITEM GALLERY", "ALL STANDARD ITEM SIZES ON FLAT FLOOR"),
];

fn spawn_map_select_ui(commands: &mut Commands) {
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

fn spawn_map_select_ui_if_pending(mut commands: Commands, pending: Res<MapSelectionPending>) {
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
fn map_select_button(
    mut commands: Commands,
    buttons: Query<(&Interaction, &MapSelectButton), Changed<Interaction>>,
    root: Query<Entity, With<MapSelectRoot>>,
    mut pending: ResMut<MapSelectionPending>,
    mut noclip: ResMut<Noclip>,
    mut autopilot: ResMut<Autopilot>,
    channels: Res<RepliconChannels>,
    // Every currently-spawned level-geometry entity — despawned + rebuilt
    // below for whichever scenario was just picked. Needed because level
    // geometry is now built LOCALLY at Startup (see setup()) instead of
    // replicated from the server (see level_piece.rs's own doc comment)
    // — Startup only runs once, at the DEFAULT scenario (MAP_ID unset),
    // so picking a different map from this in-game menu used to leave the
    // stale default geometry on screen forever (reported bug: picking
    // ITEM GALLERY still showed the B1 garage) unless re-spawned here.
    existing_geometry: Query<Entity, Or<(With<shared::LevelBox>, With<shared::LevelPanel>)>>,
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
        // Mirrors the App-build-time `Noclip(current_scenario() ==
        // StressTest)` default (see its own comment) — that default gets
        // baked in before any map is actually picked (MAP_ID isn't set
        // yet), so a StressTest pick needs to apply the same auto-enable
        // itself now that the choice is real.
        noclip.0 = scenario == Scenario::StressTest;
        // Same reasoning — Autopilot::default() also baked in before any
        // map was picked (see autopilot_enabled_by_default's own doc
        // comment), needs re-applying now that the choice is real.
        autopilot.0 = autopilot_enabled_by_default(scenario);

        for entity in &existing_geometry {
            commands.entity(entity).despawn();
        }
        if scenario == Scenario::ItemGallery {
            let (pieces, panels) = shared::item_gallery_level_pieces();
            spawn_level_pieces(&mut commands, pieces, panels);
        } else if USE_MAP1_B1_GARAGE && scenario != Scenario::LegacyArena {
            let (pieces, panels) = shared::map1_level_pieces();
            spawn_level_pieces(&mut commands, pieces, panels);
            // Static lights (spawn_map1_lights) aren't re-spawned here —
            // they're already up from Startup and every non-ItemGallery,
            // non-LegacyArena scenario reuses the same map1 geometry, so
            // there's nothing stale to replace for the common case this
            // menu is actually used for. Switching TO LegacyArena/GLTF
            // from here would leave both map1's lights AND its own
            // never-spawned geometry stale — pre-existing gap, not
            // touched by this fix (this menu's own switch matrix always
            // routed through USE_MAP1_B1_GARAGE's else-branches before
            // ItemGallery existed too).
        }
        spawn_client_connection(&mut commands, &channels);
    }
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
    autopilot: Res<'w, Autopilot>,
    autopilot_state: Res<'w, AutopilotState>,
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





















// Toggles InspectMode on KeyL — bundles Noclip + hide-gun together (and
// restores both to their pre-inspect state on the way back out) so map
// debugging is one keypress instead of three. See InspectMode's own doc
// comment for why shoot() also needs to check it separately (this system
// only handles the fly/visibility side, not firing).
fn toggle_inspect_mode(
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
fn toggle_debug_flags(
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


// Draws a real Capsule3d gizmo spanning two resolved bone positions —
// belly->chest for the torso, or one consecutive joint pair for a limb
// segment — instead of a sphere at each endpoint. This is what
// zombie_hit_damage_bones (crates/shared) actually tests a hit against
// (point-to-segment distance <= radius), so what you see here is the real
// hitbox shape, not just markers at its corners. `a`/`b` order doesn't
// matter — the capsule is symmetric.
fn draw_bone_capsule(gizmos: &mut Gizmos, a: Vec3, b: Vec3, radius: f32, color: Color) {
    let segment = b - a;
    let Ok(direction) = Dir3::new(segment) else {
        // Zero-length (both joints resolved to the same point, e.g. still
        // mid-load) — nothing meaningful to draw.
        return;
    };
    gizmos.primitive_3d(
        &Capsule3d { radius, half_length: segment.length() / 2.0 },
        // Capsule3d's local up axis is Y — from_rotation_arc finds the
        // rotation that takes Y to this segment's actual direction, same
        // trick used to orient any gizmo primitive along an arbitrary axis.
        Isometry3d::new(a.midpoint(b), Quat::from_rotation_arc(Vec3::Y, *direction)),
        color,
    );
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






// Fires only for the shooter, once the server has resolved their own shot —
// see ShotConfirmed's own doc comment for why the floating damage number
// and blood-vs-spark choice wait on this instead of a local guess in
// shoot(). The hitmarker itself already showed immediately there — this
// only adds the damage-dependent half of the feedback, once it's known for
// real.
fn on_shot_confirmed(
    trigger: On<ShotConfirmed>,
    mut commands: Commands,
    time: Res<Time>,
    vfx_assets: VfxAssets,
    spatial_query: SpatialQuery,
    zombies: Query<(Entity, &Transform, &ZombieHitboxBones), With<Zombie>>,
    globals: Query<&GlobalTransform>,
) {
    let ShotConfirmed { point, normal, damage, killed } = *trigger.event();
    if damage > 0.0 {
        info!("[shot] server confirmed: inner bone capsule hit, damage={damage:.1} killed={killed}");
    } else {
        info!("[shot] server confirmed: outer capsule hit but missed every inner bone capsule (0 damage)");
    }
    // Same "nearest zombie, then nearest bone of that zombie" approximation
    // on_shot_fx uses — ShotConfirmed doesn't carry which zombie entity it
    // was either, same reasoning (avoids cross-network entity mapping for a
    // purely cosmetic detail; see nearest_zombie_bone's own doc comment for
    // why a bone, not the zombie root).
    let nearest_bone = if killed {
        None
    } else {
        zombies
            .iter()
            .min_by(|(_, a, _), (_, b, _)| a.translation.distance_squared(point).total_cmp(&b.translation.distance_squared(point)))
            .and_then(|(_, _, bones)| nearest_zombie_bone(bones, point, &globals))
    };
    let bone_for_blood = nearest_bone.as_ref().map(|(entity, global)| (*entity, global));
    spawn_zombie_hit_feedback(&mut commands, &vfx_assets.blood, &vfx_assets.sparks, &spatial_query, &time, point, normal, damage, bone_for_blood, killed);
}














// Ticks down each pending respawn timer (queued by shoot() when a zombie
// dies) and spawns a replacement once it hits zero, cycling through
// ZOMBIE_SPAWNS so the population stays at 3 instead of thinning out.
// The zombie scene's AnimationPlayer only exists once its glTF finishes
// loading (async), so this watches for new ones, wires up the shared
// AnimationGraph, starts them on Idle, and records which zombie root each
// player belongs to (by walking up the spawned hierarchy) for
// update_zombie_animation to use later.
// find_animation_root now lives in crates/shared — crates/server's own
// wire_server_zombie_animations needs the same walk-up-to-root lookup.









// The C entry point gen/apple/Sources/client/main.mm actually calls into —
// see mobile_scaffold_test's own generated lib.rs template (cargo-mobile2's
// bevy template), copied verbatim.
#[unsafe(no_mangle)]
#[inline(never)]
#[cfg(target_os = "ios")]
pub extern "C" fn start_app() {
    main();
}

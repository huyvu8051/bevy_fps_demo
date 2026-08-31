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
// NativeSocket only exists under renet2's "native_transport" feature — real
// UDP sockets, which don't exist in a browser sandbox at all. Only used by
// network_setup.rs's native spawn_client_connection (wasm32 gets its own
// stub there that doesn't need it) — see that fn's own doc comment for the
// full wasm networking story.
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
    PlayerStance, RequestRestart, Scenario, SetAutoMaintainZombies, SetFreezeZombies, SetZombieCount, ShotConfirmed, ShotFx, SpawnDebugBot,
    SpawnMoreZombies, Zombie, ZombieAttackTimer, ZombieHitboxBones, ZombieKind, ZombieMotion, ZombieShowcaseClip, wire_zombie_hitbox_bones,
    zombie_kind_clip_count, zombie_kind_mesh_path, zombie_kind_mesh_transform, zombie_kind_gameplay_mesh_transform,
    zombie_kind_pose_clips, zombie_kind_showcase_root_y,
    INTERP_DELAY_SECS, MAX_SHOT_DISTANCE,
    HEAD_HITBOX_RADIUS, LIMB_HITBOX_RADIUS, TORSO_HITBOX_RADIUS,
    resolve_bone_world_positions, zombie_hit_damage_ray,
    PROTOCOL_ID,
    ZOMBIE_ATTACK_INTERVAL, ZOMBIE_LOD_DISTANCE, ZOMBIE_OUTER_HEIGHT, ZOMBIE_OUTER_RADIUS,
    zombie_pose_for, ZombiePose, ZOMBIE_MAX_HEALTH, ZOMBIE_SPAWNS, ZOMBIE_STOP_DISTANCE,
    ARENA_HALF_SIZE, ARENA_WALL_HEIGHT,
};
// Only network_setup.rs's native (non-wasm32) spawn_client_connection uses
// these — a wasm32 client can't open a raw UDP socket at all (browser
// sandbox), so there's no native socket here to wrap in simulated latency.
#[cfg(not(target_arch = "wasm32"))]
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

// HUD: health/ammo/ping/compass/crosshair/debug panel/floating damage —
// see this module's own doc comment. Phase 6 of splitting lib.rs's
// gameplay code into topic files.
mod hud;
use hud::*;

// Networking setup: connection, replication mirroring, game-over/
// map-select/restart flow — see this module's own doc comment. Phase 7
// (the last) of splitting lib.rs's gameplay code into topic files.
mod network_setup;
use network_setup::*;

// Meshy.ai car test props (Scenario::ItemGallery) — spawn + collision +
// physics, split out of lib.rs/vfx.rs into its own plugin since it's
// genuinely orthogonal to the rest of gameplay (see this module's own doc
// comment for why that's safe re: GamePlugin's master `.chain()`).
mod item_gallery_cars;
use item_gallery_cars::*;

// MapKind::Arena geometry is data-driven now (arena_layout_map<ID>.ron per
// map id); this module is the EDITOR=1 F9 write-back for it, same idea as
// item_gallery_cars.rs's F9 saver for the Map 5 cars.
mod arena_layout;
use arena_layout::*;

// Client-side navmesh + path-trace overlay for debugging the server's
// zombie pathfinding — see the module doc comment. Plugin only added in
// debug builds; the "NavDbg" settings toggle drives it.
mod nav_debug;
use nav_debug::*;

// Which level geometry a scenario renders is decided in ONE place —
// shared's `Scenario::map_kind()` — and matched on in setup() /
// network_setup's map_select_button, mirroring the server's own
// spawn_arena_colliders. `spawn_test_arena` builds MapKind::Arena;
// shared::map1_level_pieces + spawn_map1_lights build MapKind::Building;
// shared::item_gallery_level_pieces builds MapKind::ItemGallery.

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
        // wasm32 (crates/web) always shows the touch controller UI — the
        // web build is meant to be opened from a phone browser (see
        // network_setup.rs's wasm32 spawn_client_connection, which reads
        // window.location.hostname so a phone on the same LAN connects
        // back to the dev machine), same reasoning as the iOS/Android
        // auto-on below, not gated behind TOUCH_UI_PREVIEW since there's
        // no "desktop preview" concept for a build that only ever runs in
        // a browser.
        let mobile = cfg!(target_os = "ios") || cfg!(target_os = "android") || cfg!(target_arch = "wasm32");
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
// Default 40% — the 3D scene renders into a 0.4x offscreen target and gets
// upscaled to fill the window. Override per-run with RENDER_SCALE, or live
// via the settings-menu presets.
const RESOLUTION_SCALE_DEFAULT: f32 = 0.4;
const RESOLUTION_SCALE_MIN_TEXTURE_PX: u32 = 64;

/// Startup value for `RenderResolutionScale` — `RESOLUTION_SCALE_DEFAULT`
/// unless `RENDER_SCALE` env overrides it (clamped to the sane 0.1..=1.0
/// range the offscreen target supports). The in-game settings-menu presets
/// still change it at runtime from whatever this returns. Also feeds the
/// the offscreen game-camera render target.
fn resolution_scale_default() -> f32 {
    std::env::var("RENDER_SCALE")
        .ok()
        .and_then(|s| s.trim().parse::<f32>().ok())
        .map(|s| s.clamp(0.1, 1.0))
        .unwrap_or(RESOLUTION_SCALE_DEFAULT)
}

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
// Client's first-frame camera position — the server's replicated
// NetTransform corrects it within a tick or two. Per-scenario value comes
// from the same shared::scenario_player_spawn the server's own authoritative
// spawn uses, so the two can't disagree; resolved at runtime in setup().

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


// User-facing (not a debug toggle): scales the main camera's offscreen
// render target — see RESOLUTION_SCALE_PRESETS' own comment. 1.0 = native
// resolution. resize_scene_render_target reads this every frame and resizes
// SceneRenderTarget's Image whenever it (or the window) changes.
#[derive(Resource)]
struct RenderResolutionScale(f32);

impl Default for RenderResolutionScale {
    fn default() -> Self {
        Self(resolution_scale_default())
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


impl Default for MapSelectionPending {
    fn default() -> Self {
        Self(network_setup::map_id_override().is_none())
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

// Starts matching the sun DirectionalLight's own shadows_enabled at spawn
// (see setup()) — same reasoning as MapMsaaOn. Default now follows
// light_shadows_enabled() so a desktop dev run starts with shadows off.
#[derive(Resource)]
struct MapShadowsOn(bool);
impl Default for MapShadowsOn {
    fn default() -> Self {
        Self(light_shadows_enabled())
    }
}

// Dynamic lighting is expensive on a warm laptop during walk-around /
// editor sessions and adds nothing to judging prop scale, so desktop
// defaults it OFF: no sun, no hall/map point lights — just the per-camera
// AmbientLight (flat but everything stays clearly visible). `LIGHTS=1`
// brings the light entities back; `SHADOWS=1` additionally re-enables
// their shadow maps (the priciest part — point-light shadows are a 6-face
// cubemap render each, per frame). Mobile (iOS/Android) always keeps full
// lighting + shadows — this is purely a desktop dev-ergonomics / thermal
// lever, not a shipped downgrade.
pub(crate) fn dynamic_lights_enabled() -> bool {
    cfg!(any(target_os = "ios", target_os = "android")) || std::env::var("LIGHTS").is_ok()
}
pub(crate) fn light_shadows_enabled() -> bool {
    cfg!(any(target_os = "ios", target_os = "android")) || std::env::var("SHADOWS").is_ok()
}

/// AmbientLight brightness for the two scene cameras — bumped hard when
/// dynamic lights are off so an ambient-only scene doesn't read as murky.
fn ambient_brightness() -> f32 {
    if dynamic_lights_enabled() { 250.0 } else { 1800.0 }
}


// Marks the main game's sun DirectionalLight (setup()'s own, not the
// map1 / arena point lights) so apply_map_render_toggles can find it
// unambiguously.
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


// How often auto_reconnect retries after losing the connection — mainly a
// dev-workflow thing: lets the server be killed and restarted (e.g. after
// rebuilding it) without also having to restart the client every time.
const RECONNECT_INTERVAL: f32 = 2.0;











// INTERP_DELAY_SECS now lives in crates/shared — crates/server's
// handle_fire_shot needs the same value for lag compensation (rewinding to
// roughly what the shooter's screen actually showed = network latency +
// this render delay).
// Snapshots older than this relative to the newest one are dropped —
// keeps the buffer from growing unbounded if an entity stops replicating
// (e.g. a zombie the LOD/crowd-hz throttle rarely updates) without needing
// a fixed-size ring buffer.
const INTERP_BUFFER_MAX_AGE_SECS: f32 = 0.5;







// #[bevy_main] requires the fn to be named exactly `main` — see its own
// macro's compile-error message. Desktop's src/main.rs (a thin `fn main() {
// client::main(); }`) and iOS's start_app extern shim below both call this
// same one, so there's exactly one place the App is actually built,
// regardless of platform.
#[bevy_main]
pub fn main() {
    // The web build always launches straight into map 2 (Arena/
    // StressTest — matches the "ARENA" map-select button), skipping the
    // menu entirely, same idea as a native `MAP_ID=2` launch bypassing it
    // (see MapSelectionPending::default) — the web build's whole purpose
    // is "open this link on a phone and immediately be in a playable
    // scene", not a general-purpose map picker. Paired with
    // TouchControlsEnabled::default() forcing the touch controller UI on
    // for every wasm32 build too (same section, above).
    #[cfg(target_arch = "wasm32")]
    network_setup::set_map_id(2);

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
                        // wasm32: winit's web backend has no OS window at
                        // all — `mode`/`WindowResolution` above are moot,
                        // it just creates (or reuses, per `canvas` below) a
                        // <canvas> element sized at Bevy's own default
                        // (1280x720 logical px) and leaves it there,
                        // regardless of the actual browser viewport —
                        // confirmed live: every screenshot taken of the web
                        // build so far showed the canvas as a small fixed
                        // box, not filling the tab. `canvas` points winit
                        // at the specific <canvas id="bevy-canvas"> element
                        // crates/web/index.html creates (inside a padded
                        // container div, see that file's own comment) —
                        // without this, winit creates its own bare canvas
                        // appended straight to <body>, outside that padded
                        // container. `fit_canvas_to_parent` then makes
                        // winit continuously resize that canvas to match
                        // its parent (the padded div) every frame, so it
                        // tracks the real viewport size (and orientation
                        // changes) instead of staying pinned at 1280x720.
                        canvas: if cfg!(target_arch = "wasm32") { Some("#bevy-canvas".to_string()) } else { None },
                        fit_canvas_to_parent: cfg!(target_arch = "wasm32"),
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
            // Desktop: cap the update/redraw rate instead of
            // WinitSettings::game()'s UpdateMode::Continuous. Continuous
            // renders flat-out and only relies on VSync to throttle — on a
            // 120Hz ProMotion Mac, or with EDITOR=1 adding an egui pass, or
            // a near-empty scene like ItemGallery, that just spins the GPU
            // hot for no visible benefit in a walk-around / editor session.
            // Reactive at 1/FPS_CAP still redraws every interval with zero
            // input (so animation/physics stay smooth) but stops the
            // uncapped spin. FPS_CAP env overrides the 60 default; unfocused
            // (window in background while you read the terminal) drops to
            // 10Hz.
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
    // Dev-only level editor (bevy_editor_pls, vendored + patched for Bevy
    // 0.18 under vendor/) — off unless EDITOR=1 is set, so normal
    // play (and every other build target: iOS/Android/CADisplayLink)
    // never pulls it in. Desktop-only on purpose: it's for building Map 1
    // geometry on the dev Mac, not for on-device use.
    if std::env::var("EDITOR").is_ok() {
        app.add_plugins(bevy_editor_pls::EditorPlugin::default());
        // AmbientLight is per-camera in Bevy 0.18 (no global resource
        // fallback), and only the two game cameras in setup() carry one.
        // bevy_editor_pls spawns its own free / pan-orbit editor cameras
        // without it, so when dynamic_lights_enabled() is false the editor
        // viewport renders pitch black while the game view is fine. Give
        // every ambient-less 3D camera the same fill the game camera uses.
        app.add_systems(Update, fill_ambient_on_editor_cameras);
        // NOTE: RENDER_SCALE / RenderResolutionScale only shrink the GAME
        // camera (it renders to an offscreen Image that a fullscreen
        // ImageNode then stretches back). The bevy_editor_pls editor
        // cameras render straight to the window, and MainPassResolutionOverride
        // needs an upscaler (DLSS/FSR — none on macOS) to expand the
        // reduced pass back to full size, so it just left the view in a
        // black-bordered corner. Editor-viewport downscale is dropped;
        // FPS_CAP + LIGHTS=0 already cover editor thermals.
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

// Everything the real game needs that has nothing to do with which
// windowing backend is driving it (winit vs. the experimental
// CADisplayLink FFI entry point in cadisplaylink::ffi) — mirrors
// build_probe_common's own reasoning. DefaultPlugins/WindowPlugin/
// TaskPoolPlugin/WinitSettings stay in each entry point's own setup since
// they genuinely differ (WinitPlugin disabled + no WinitSettings on the
// CADisplayLink path), everything else is identical.
//
// A real `Plugin`, not just a free function — wraps the ~90-system
// master Update chain built up across the 7-phase lib.rs file split
// (see chat/commit history: lib.rs was one 8688-line file; each phase
// moved a topic's systems into its own file — touch_input/settings_menu/
// movement_combat/zombie_client/vfx/hud/network_setup — but the
// REGISTRATION below stays in exactly this one place, in exactly this
// order). Splitting that registration itself across N independent
// Plugins would NOT be safe: Bevy doesn't guarantee relative ordering
// between separate `add_systems` calls, and this pipeline's correctness
// depends on running in this precise sequence (e.g. camera sync must
// see this frame's already-applied movement prediction, not next
// frame's). One `GamePlugin` = one place that owns the order.
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
        // Dev-only navmesh / path-trace overlay (nav_debug.rs) — a no-op in
        // release builds (see NavDebugPlugin::build), driven by the debug-
        // gated "NavDbg" settings button.
        .add_plugins(NavDebugPlugin)
        // Noclip is a manual cheat toggle for every scenario now — off by
        // default. (Map 2 used to auto-enable it for observing the horde
        // from above; it's a walkable arena now.)
        .insert_resource(Noclip(false))
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
        .init_resource::<PredictionSmooth>()
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
    // First-frame camera/player position — see the PLAYER_SPAWN comment
    // above; the server's replicated NetTransform takes over almost
    // immediately.
    let player_spawn = scenario_player_spawn(current_scenario());

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
        scaled_scene_extent(window_width, window_height, resolution_scale_default()),
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

    // Quick-review look-direction override, paired with server-side
    // REVIEW_SPAWN (shared::item_gallery_player_spawn) — set env
    // `REVIEW_YAW`/`REVIEW_PITCH` (degrees) so a freshly imported prop is
    // already in frame on connect instead of turning to find it by hand
    // every time. Only applied in ItemGallery; other scenarios always
    // start facing forward (yaw 0, pitch 0) as before.
    fn review_yaw_pitch() -> (f32, f32) {
        if current_scenario() != Scenario::ItemGallery {
            return (0.0, 0.0);
        }
        let deg = |var: &str| std::env::var(var).ok().and_then(|s| s.trim().parse::<f32>().ok()).unwrap_or(0.0);
        (deg("REVIEW_YAW").to_radians(), deg("REVIEW_PITCH").to_radians())
    }

    // Geometry per scenario — same decision the server's
    // spawn_arena_colliders makes, via shared's Scenario::map_kind().
    // spawn_level_box_visuals / spawn_level_panel_visuals below react to
    // Added<LevelBox>/Added<LevelPanel> to build mesh+collider for the
    // Building/ItemGallery pieces.
    match current_scenario().map_kind() {
        MapKind::ItemGallery => {
            let (pieces, panels) = shared::item_gallery_level_pieces();
            spawn_level_pieces(&mut commands, pieces, panels);
            spawn_item_gallery_test_props(&mut commands, &asset_server);
            spawn_item_gallery_zombie_showcase(&mut commands, &asset_server);
        }
        MapKind::Building => {
            let (pieces, panels) = shared::map1_level_pieces();
            spawn_level_pieces(&mut commands, pieces, panels);
            if dynamic_lights_enabled() {
                spawn_map1_lights(&mut commands);
            }
        }
        MapKind::Arena => {
            spawn_test_arena(&mut commands, &mut meshes, &mut materials, current_scenario().map_id());
            // Temporarily hidden on web only: the web build force-starts on
            // map 2 (StressTest, an Arena map — see wasm32 set_map_id(2) in
            // main() above), and the 16 meshy.ai car test props this spawns
            // add real vertex/texture weight for no gameplay reason there.
            // Testing iPhone load/perf without them for now; desktop keeps
            // its cars unchanged. Remove this cfg to restore always-on.
            #[cfg(not(target_arch = "wasm32"))]
            spawn_item_gallery_test_props(&mut commands, &asset_server);
        }
    }

    // spawn_test_arena carries its own overhead light; Building gets
    // spawn_map1_lights above. This is just the shared sun.
    if dynamic_lights_enabled() {
        commands.spawn((
            DirectionalLight {
                illuminance: 8_000.0,
                shadows_enabled: light_shadows_enabled(),
                ..default()
            },
            Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.9, 0.5, 0.0)),
            MainSun,
        ));
    }

    commands
        .spawn((
            Transform::from_translation(player_spawn),
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
        Transform::from_translation(player_spawn).with_scale(Vec3::splat(PLAYER_MODEL_SCALE)),
        Visibility::Hidden,
        PlayerModel,
        PlayerModelAnimRoot,
    ));

    // Per-kind 5-pose gameplay graphs (mirrors the server's
    // ServerZombieAnim). Zombies are server-spawned + replicated now — no
    // local spawning here; see the on_zombie_replicated observer.
    commands.insert_resource(build_zombie_anim(&asset_server, &mut graphs));
    // Per-kind full-clip graphs — only the ItemGallery showcase rig uses
    // these (one zombie pinned per clip); harmless everywhere else.
    commands.insert_resource(build_zombie_kind_anims(&asset_server, &mut graphs));

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

    let (review_yaw, review_pitch) = review_yaw_pitch();
    let camera = commands
        .spawn((
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
            Transform::from_translation(player_spawn + Vec3::Y * EYE_HEIGHT_OFFSET),
            AmbientLight {
                color: Color::WHITE,
                brightness: ambient_brightness(),
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

























// spawn_zombie_navmesh/column_footprint now live in crates/server —
// pathfinding is server-authoritative.

// The MapKind::Arena geometry — a plain flat-floored square room: floor +
// 4 perimeter walls + free-standing pillars + a centre test ramp, each
// with a matching RigidBody::Static + Collider. Read from
// crates/client/assets/data/arena_layout_map<ID>.ron (shared::arena_layout)
// so it stays byte-for-byte in step with the server's own
// spawn_flat_arena_colliders. Tags every entity spawn_test_arena creates
// (boxes + light) with ArenaGeometry so map_select_button can despawn the
// arena when the player picks a different map from the in-game menu — the
// same job LevelBox/LevelPanel do for the Building geometry.
#[derive(Component)]
struct ArenaGeometry;

/// The shape + size + row index of one arena piece, kept on the entity so
/// F9 (save_arena_layout_on_key) can write the live Transform back to the
/// RON in the original row order. Not on the arena light — that has no row.
#[derive(Component)]
pub(crate) struct ArenaBoxDims {
    pub(crate) shape: shared::ArenaShape,
    pub(crate) size: Vec3,
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

    // Row 0 is the floor by convention (see the RON header); the rest —
    // walls, cylindrical pillars, the tilted ramp — share the wall
    // material. One entity per row: mesh + its own RigidBody::Static +
    // Collider (cuboid or cylinder per b.shape), exactly the piece the
    // server spawns collider-only.
    for (i, b) in shared::arena_layout(map_id).iter().enumerate() {
        let material = if i == 0 { floor_material.clone() } else { wall_material.clone() };
        let (mesh, collider): (Handle<Mesh>, Collider) = match b.shape {
            shared::ArenaShape::Cuboid => (
                meshes.add(Cuboid::new(b.size.x, b.size.y, b.size.z)),
                Collider::cuboid(b.size.x, b.size.y, b.size.z),
            ),
            // size.x is the full diameter, size.y the height.
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
        // Pillars are the holes the client-side nav_debug overlay carves
        // out of its reconstructed navmesh (mirrors the server's
        // ArenaObstacle tag). Harmless when NavDebugPlugin is a no-op.
        if b.shape == shared::ArenaShape::Cylinder {
            piece.insert(ClientNavObstacle);
        }
    }

    // AmbientLight is per-camera in this Bevy version (not a global
    // resource) — the main camera already gets its own in setup(). A single
    // bright, wide-range PointLight overhead is enough to light this flat
    // open room evenly (no columns/corners to leave dark, unlike Sponza).
    if dynamic_lights_enabled() {
        commands.spawn((
            PointLight {
                color: Color::srgb(1.0, 0.98, 0.92),
                intensity: 4_000_000.0,
                range: ARENA_HALF_SIZE * 3.0,
                shadows_enabled: light_shadows_enabled(),
                ..default()
            },
            Transform::from_xyz(0.0, ARENA_WALL_HEIGHT - 0.2, 0.0),
            ArenaGeometry,
        ));
    }
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
            shadows_enabled: light_shadows_enabled(),
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
                shadows_enabled: light_shadows_enabled(),
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
                shadows_enabled: light_shadows_enabled(),
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
                shadows_enabled: light_shadows_enabled(),
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







// RenetClient::rtt() is renet2's own round-trip estimate (seconds) off the
// live connection — reflects real network conditions plus whatever
// SIM_LATENCY_MS/SIM_LATENCY_JITTER_MS is currently simulating (see
// shared::sim_latency), so this doubles as a way to verify the simulated
// latency/jitter config is actually taking effect. Rewriting the text every
// render frame made the number flicker too fast to actually read, so this
// only updates 30x/sec (same idea and rate as FpsOverlayConfig's own
// refresh_interval).
const PING_UI_REFRESH_SECS: f32 = 1.0 / 30.0;


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






// (MAP_ID, button title, one-line description) — keep in sync with
// Scenario::from_map_id (crates/shared/src/network.rs). ASCII/English only
// (not the original Swift menu's Vietnamese text) since this renders
// through native_metal_probe.rs's bitmap font on iOS too, which only has
// uppercase A-Z/digits/a handful of symbols — see glyph_index's own doc
// comment.
//
// These are the only 3 scenarios that exist at all now (`Scenario` in
// crates/shared/src/network.rs) — HitboxTest/RenderCompare/LegacyArena were
// dedicated-server-only test rigs never reachable from this menu, removed
// wholesale rather than kept around unreachable (see git history). Each of
// the 3 below builds a visibly different world (Scenario::map_kind), so
// map_select_button can rebuild them properly.
const MAP_SELECT_ENTRIES: [(u8, &str, &str); 3] = [
    (0, "GAME MAP", "OFFICE BUILDING (B1-F3)"),
    (2, "ARENA", "OPEN FLAT ARENA, CHASE-AI ZOMBIES"),
    (5, "ITEM GALLERY", "ALL STANDARD ITEM SIZES ON FLAT FLOOR"),
];





















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
// SERVER_TICK_HZ can act on it — MUST equal it.
const INPUT_SEND_HZ: f32 = 32.0;

// predict_player_movement: only snap the body toward the server when the
// disagreement exceeds THRESHOLD metres (below it, the steady per-frame-vs-
// -30Hz MoveAndSlide gap that used to jerk every reconcile is ignored).
// When it does snap, the visual delta goes into PredictionSmooth.offset and
// sync_camera_to_player eases it to zero over SMOOTH_SECS — body teleports,
// camera glides.
const RECONCILE_POS_THRESHOLD: f32 = 0.15;
const RECONCILE_SMOOTH_SECS: f32 = 0.12;

#[derive(Resource, Default)]
struct PredictionSmooth {
    offset: Vec3,
}

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

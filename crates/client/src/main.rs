use avian3d::prelude::*;
use bevy::animation::graph::{AnimationGraph, AnimationGraphHandle, AnimationNodeIndex};
use bevy::animation::AnimationPlayer;
use bevy::dev_tools::fps_overlay::FpsOverlayPlugin;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::RenderTarget;
use bevy::ecs::system::SystemParam;
use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::math::ops;
use bevy::picking::mesh_picking::MeshPickingPlugin;
use bevy::post_process::bloom::Bloom;
use bevy::post_process::dof::{DepthOfField, DepthOfFieldMode};
use bevy::prelude::*;
use bevy::asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
use bevy::render::view::Msaa;
use bevy::scene::SceneRoot;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow, Window};
use bevy_replicon::prelude::*;
use bevy_replicon::shared::backend::connected_client::NetworkId;
use bevy_replicon_renet2::{
    netcode::{ClientAuthentication, NativeSocket, NetcodeClientTransport},
    renet2::{ConnectionConfig, RenetClient},
    RenetChannelsExt, RepliconRenetPlugins,
};
use shared::{
    ClientInput, FireShot, GameOver, GameStateEvent, Health, HEAD_DAMAGE, HITBOX_BELLY_MIN_Y,
    HITBOX_CHEST_MIN_Y, HITBOX_HEAD_MIN_Y, HITBOX_LIMB_LATERAL_MIN, KnockbackVelocity, NetPlayer,
    NetTransform, Player, PlayerDied, PlayerStance, RequestRestart, ShotFx, SpawnMoreZombies, Zombie,
    ZombieAttackTimer, ZombieMotion, ZOMBIE_ATTACK_INTERVAL, ZOMBIE_LOD_DISTANCE, ZOMBIE_MAX_HEALTH,
    ZOMBIE_SPAWNS, ZOMBIE_STOP_DISTANCE,
};
use shared::sim_latency::{sim_latency_from_env, LaggedSocket};

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
// Mouse turns slower while ADS'd — real scopes/sights make fine aim
// adjustments feel twitchy at full sensitivity.
const AIM_MOUSE_SENSITIVITY_MULTIPLIER: f32 = 0.5;

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
// Calibrated in-game against a 1m reference cube: this comes out to a ~2m
// tall zombie, appropriately looming over the player.
const ZOMBIE_SCALE: f32 = 0.1;
const ZOMBIE_PATH: &str = "models/zombie.glb";

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
// 4=Walk (read from the file's own animations list).
const ZOMBIE_ANIM_IDLE: usize = 2;
const ZOMBIE_ANIM_WALK: usize = 4;

#[derive(Component)]
struct FpsCamera {
    yaw: f32,
    pitch: f32,
}

// Shared by shoot(), move_player_physics, and apply_aim_zoom so "aiming"
// means the same thing everywhere: cursor locked (actually playing, not
// paused/menu'd out) and right mouse button held.
fn is_aiming(cursor: &CursorOptions, mouse: &ButtonInput<MouseButton>) -> bool {
    cursor.grab_mode != CursorGrabMode::None && mouse.pressed(MouseButton::Right)
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

// Debug/dev flight mode: toggled by KeyN. Bypasses MoveAndSlide (and thus
// gravity/collision) entirely — see move_player_physics — so the player can
// fly through walls to inspect the level. Not reset by restart_game
// (deliberately — no reason to drop out of noclip just because the player
// died).
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
// a play() when NetTransform.moving actually flips instead of every frame
// (NetTransform.translation itself changes nearly every frame regardless).
#[derive(Component, Default)]
struct ZombiePlayingWalk(bool);

// Bone entities used for nearest-bone hit detection (see zombie_hit_damage).
// zombie.glb's rig (single skinned mesh, bones only for animation — see the
// earlier comment on HITBOX_* constants) names its nodes after standard
// humanoid bones; wire_zombie_hitbox_bones matches against those names as
// they show up (same async-loading problem as ZombieAnimationPlayer).
// Fields are individually optional/incrementally filled rather than an
// all-or-nothing bundle, so a hit lands correctly even mid-load using
// whichever bones have resolved so far.
#[derive(Component, Default)]
struct ZombieHitboxBones {
    head: Option<Entity>,
    chest: Option<Entity>,
    belly: Option<Entity>,
    limbs: Vec<Entity>,
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
    let server_addr: std::net::SocketAddr = std::env::var("SERVER_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:5031".to_string())
        .parse()
        .expect("SERVER_ADDR must be a valid socket address, e.g. 127.0.0.1:5031");

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

// The local player's own replicated pose is copied straight through,
// uninterpolated — feeds apply_server_player_transform below. Adding
// interpolation's fixed rendering delay here would double up with however
// much round-trip latency input already has (see prediction, still future
// work) instead of smoothing anything, since there's no "other player" to
// smooth relative to; it's this client's own last-known-authoritative pose.
fn mirror_local_player_net_transform(
    mut players: Query<(&NetTransform, &mut Transform), (Changed<NetTransform>, With<LocalPlayer>)>,
) {
    for (net_transform, mut transform) in &mut players {
        transform.translation = net_transform.translation;
        transform.rotation = Quat::from_rotation_y(net_transform.yaw);
    }
}

// How far in the past (relative to local received-snapshot time) everyone
// else's replicated position is rendered at — always enough of a buffer to
// interpolate between two *actually received* snapshots instead of
// snapping to each new one as it lands (choppy under network jitter/the
// gaps between replication ticks). Time is measured by when this client
// received each snapshot, not the server's own clock (no clock sync needed
// for this simplified approach — it just smooths out this client's own
// arrival jitter).
const INTERP_DELAY_SECS: f32 = 0.1;
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

// The client's pre-existing local Player entity (see setup() and
// PlayerEntity) is still what zombie AI/UI/damage all target — this is the
// one remaining link back to it: copy the server-authoritative position
// from our own replicated NetPlayer entity onto it every frame.
fn apply_server_player_transform(
    local: Query<&Transform, (With<NetPlayer>, With<LocalPlayer>, Without<Player>)>,
    mut player: Query<&mut Transform, With<Player>>,
) {
    let Ok(net_transform) = local.single() else {
        return;
    };
    let Ok(mut transform) = player.single_mut() else {
        return;
    };
    transform.translation = net_transform.translation;
}

fn log_client_connection_state(state: Res<State<ClientState>>) {
    if state.is_changed() {
        info!("client connection state: {:?}", state.get());
    }
}

fn main() {
    App::new()
        .add_plugins((
            // Window resolution is intentionally left at its default (native
            // window size). Shrinking it also shrinks how big the window
            // looks on screen (and the HUD with it) — Bevy 0.19 has no
            // built-in render-scale/upscale feature to reduce GPU pixel cost
            // independently of window size (checked bevy_render/bevy_camera/
            // bevy_anti_alias — no FSR/dynamic-resolution equivalent exists).
            // Doing that properly would mean a custom low-res render target
            // + upscale blit pass. The real perf wins here are elsewhere:
            // SSAO removed, Sponza textures downscaled to 512px, Bloom kept
            // modest — see earlier changes.
            DefaultPlugins,
            MeshPickingPlugin,
            PhysicsPlugins::default(),
            // A lightweight always-on FPS overlay. Bevy 0.18 doesn't have the
            // draggable DiagnosticsOverlay panels (that's 0.19-only), so this
            // is the only diagnostics UI available here.
            FpsOverlayPlugin::default(),
            RepliconPlugins,
            RepliconRenetPlugins,
        ))
        .replicate::<NetworkId>()
        .replicate::<NetPlayer>()
        .replicate::<NetTransform>()
        .replicate::<Zombie>()
        .replicate::<Health>()
        .add_client_event::<ClientInput>(Channel::Unreliable)
        .add_client_event::<SpawnMoreZombies>(Channel::Unordered)
        .add_client_event::<FireShot>(Channel::Ordered)
        .add_server_event::<ShotFx>(Channel::Ordered)
        .add_client_event::<PlayerDied>(Channel::Ordered)
        .add_client_event::<RequestRestart>(Channel::Ordered)
        .add_server_event::<GameStateEvent>(Channel::Ordered)
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
        .add_observer(on_shot_fx)
        .add_observer(on_game_state_event)
        .init_resource::<Noclip>()
        .init_resource::<FreezeZombies>()
        .init_resource::<ShowHitboxes>()
        .init_resource::<GodMode>()
        .init_resource::<DebugPanelVisible>()
        .init_resource::<AimBlurEnabled>()
        .init_resource::<AimDofFocalDistanceAlt>()
        .init_resource::<MouseLookDelta>()
        .init_resource::<PerfTimings>()
        .init_resource::<ZombieAnimDisabled>()
        .init_resource::<ZombieHealthBarsHidden>()
        .init_resource::<ZombieLodDisabled>()
        .add_systems(Startup, (setup, log_window_resolution))
        .add_systems(
            Update,
            (
                // Tuples of systems only implement IntoSystemConfigs up to
                // arity 20, so this is split into nested chained groups
                // rather than one flat tuple — .chain() on the outer tuple
                // still orders everything from grab_cursor through
                // despawn_blood_stains, just via a few hops instead of one.
                (
                    grab_cursor,
                    toggle_noclip,
                    toggle_player_stance,
                    toggle_debug_flags,
                    debug_panel_actions,
                    mouse_look,
                    shoot,
                    reload_gun,
                    apply_lean_input,
                    apply_view_recoil,
                    apply_aim_zoom,
                    apply_aim_dof,
                    send_player_input,
                    apply_server_player_transform,
                    sync_camera_to_player,
                    update_zombie_lod,
                    zombies_damage_player,
                )
                    .chain(),
                (
                    check_game_over,
                    restart_game,
                    wire_zombie_animations,
                    wire_zombie_hitbox_bones,
                    wire_gun_mesh,
                    update_zombie_animation,
                    freeze_zombie_animations,
                    update_health_ui,
                    update_zombie_health_bars,
                    update_floating_damage_texts,
                )
                    .chain(),
                (
                    adjust_scope_zoom,
                    // gun_view_animation must run before sync_scope_camera
                    // and update_scope_lens — both mirror per-frame gun
                    // state (look_sway_roll, bob_phase/blend, ads_blend,
                    // recoil_roll, etc.) that gun_view_animation computes
                    // fresh each frame. Running sync_scope_camera first
                    // meant it read last frame's stale value instead,
                    // invisible most of the time but a visible one-frame
                    // lag between the lens quad and the round scope
                    // camera's own rendered content during a fast mouse
                    // whip (reported in-game).
                    gun_view_animation,
                    sync_scope_camera,
                    update_scope_lens,
                    update_ammo_ui,
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
        )
        .run();
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
}

#[derive(Resource)]
struct ZombieAnim {
    graph: Handle<AnimationGraph>,
    idle: AnimationNodeIndex,
    walk: AnimationNodeIndex,
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
) {
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

    let zombie_clips: Vec<Handle<AnimationClip>> = [ZOMBIE_ANIM_IDLE, ZOMBIE_ANIM_WALK]
        .into_iter()
        .map(|index| asset_server.load(GltfAssetLabel::Animation(index).from_asset(ZOMBIE_PATH)))
        .collect();
    let (zombie_graph, zombie_nodes) = AnimationGraph::from_clips(zombie_clips);
    let zombie_anim = ZombieAnim {
        graph: graphs.add(zombie_graph),
        idle: zombie_nodes[0],
        walk: zombie_nodes[1],
    };

    // No local zombie spawning here anymore — zombies are server-spawned
    // and replicated; see the on_zombie_replicated observer.
    commands.insert_resource(zombie_anim);

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
    spawn_debug_panel(&mut commands);
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
fn on_zombie_replicated(
    trigger: On<Add, Zombie>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut transforms: Query<&mut Transform>,
) {
    let entity = trigger.event().entity;
    if let Ok(mut transform) = transforms.get_mut(entity) {
        transform.scale = Vec3::splat(ZOMBIE_SCALE);
    }
    commands.entity(entity).insert(zombie_cosmetic_bundle(&asset_server));
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
            Transform::from_translation(spawn_pos).with_scale(Vec3::splat(ZOMBIE_SCALE)),
            Zombie,
            // Not replicated, so unlike a real zombie it needs its own local
            // Health — nothing will ever set one via replication for it.
            // Also unkillable via shoot() now: the server has never heard of
            // this entity, so its raycast can't land on it.
            Health(ZOMBIE_MAX_HEALTH),
            zombie_cosmetic_bundle(asset_server),
        ))
        .id();
    spawn_zombie_health_bar(commands, zombie_entity);
}

// No collider here anymore — hit detection is server-authoritative now
// (crates/server's handle_fire_shot raycasts against its own zombie
// colliders), so the client doesn't need one of its own to shoot with.
fn zombie_cosmetic_bundle(asset_server: &AssetServer) -> impl Bundle {
    (
        SceneRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(ZOMBIE_PATH))),
        ZombieHitboxBones::default(),
        KnockbackVelocity::default(),
        ZombieAttackTimer::default(),
        ZombiePlayingWalk::default(),
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
        AmmoUi,
    ));
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
) {
    if game_over.0 || freeze.0 || god_mode.0 {
        return;
    }
    let Ok((player_transform, mut health)) = player.single_mut() else {
        return;
    };
    let dt = time.delta_secs();
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
        DebugPanelText,
    ));
}

fn update_debug_panel(
    panel_visible: Res<DebugPanelVisible>,
    noclip: Res<Noclip>,
    freeze: Res<FreezeZombies>,
    show_hitboxes: Res<ShowHitboxes>,
    god_mode: Res<GodMode>,
    aim_blur: Res<AimBlurEnabled>,
    dof_focal_alt: Res<AimDofFocalDistanceAlt>,
    timings: Res<PerfTimings>,
    zombie_anim_disabled: Res<ZombieAnimDisabled>,
    zombie_health_bars_hidden: Res<ZombieHealthBarsHidden>,
    zombie_lod_disabled: Res<ZombieLodDisabled>,
    zombies: Query<(), With<Zombie>>,
    mut text: Query<(&mut Text, &mut Node), With<DebugPanelText>>,
) {
    let Ok((mut text, mut node)) = text.single_mut() else {
        return;
    };
    if !panel_visible.0 {
        node.display = Display::None;
        return;
    }
    node.display = Display::Flex;

    let on_off = |b: bool| if b { "ON" } else { "OFF" };
    let focal_distance = if dof_focal_alt.0 {
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
         move_zombies: {:.2}ms | health bars: {:.2}ms | zombies: {} (near: {})\n\
         Zombies alive: {}",
        on_off(noclip.0),
        on_off(freeze.0),
        on_off(show_hitboxes.0),
        on_off(god_mode.0),
        on_off(aim_blur.0),
        focal_distance,
        ZOMBIE_SPAWNS.len(),
        on_off(!zombie_anim_disabled.0),
        on_off(!zombie_health_bars_hidden.0),
        on_off(!zombie_lod_disabled.0),
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
    mut zombies: Query<(&Transform, &mut Health), With<Zombie>>,
    all_zombies: Query<Entity, With<Zombie>>,
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
    let aiming = cursor.single().is_ok_and(|cursor| is_aiming(cursor, &mouse));
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
    zoom: Res<ScopeZoom>,
    mut motion: MessageReader<MouseMotion>,
    mut look_delta: ResMut<MouseLookDelta>,
    mut query: Query<&mut FpsCamera>,
) {
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
    let sensitivity = if is_aiming(cursor, &mouse) {
        MOUSE_SENSITIVITY * AIM_MOUSE_SENSITIVITY_MULTIPLIER * (SCOPE_DEFAULT_ZOOM / zoom.0)
    } else {
        MOUSE_SENSITIVITY
    };
    for mut cam in &mut query {
        cam.yaw -= delta.x * sensitivity;
        cam.pitch = (cam.pitch - delta.y * sensitivity).clamp(-1.54, 1.54);
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
    cursor: Query<&CursorOptions, With<PrimaryWindow>>,
    mut camera: Query<&mut Projection, With<FpsCamera>>,
) {
    let Ok(cursor) = cursor.single() else {
        return;
    };
    let target_fov = if is_aiming(cursor, &mouse) {
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
    aim_blur: Res<AimBlurEnabled>,
    dof_focal_alt: Res<AimDofFocalDistanceAlt>,
    cursor: Query<&CursorOptions, With<PrimaryWindow>>,
    camera: Query<Entity, With<FpsCamera>>,
) {
    let (Ok(cursor), Ok(camera_entity)) = (cursor.single(), camera.single()) else {
        return;
    };
    if aim_blur.0 && is_aiming(cursor, &mouse) {
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

// Ground-based movement: WASD sets horizontal velocity relative to where the
// camera is *facing on the horizontal plane* (yaw only — looking up/down no
// longer flies you into the air like the old noclip version did). Gravity
// and jumping are handled here too, and avian3d's move_and_slide sweeps the
// player's capsule against the level collider, sliding along walls/slopes
// and stopping at anything it can't get past.
//
// Caveat: this is plain move-and-slide with no dedicated step-up logic, so
// stairs/ledges taller than roughly the capsule radius may block movement
// instead of being climbed — untested since this can't be visually verified
// here.
// Toggles Noclip on KeyN. Separate from move_player_physics so the toggle
// still works even while game_over freezes movement.
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

// Player movement is now server-authoritative (see crates/server's
// move_players) — this no longer touches Transform or avian3d at all, it
// just packages input into a ClientInput and sends it. The resulting
// position comes back later via replication (see apply_server_player_transform).
// Noclip's fly-through-walls behavior had no server-side equivalent when
// this moved, so KeyN's toggle is now inert (kept only so the debug panel
// line and toggle system don't need removing too).
fn send_player_input(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    game_over: Res<GameOver>,
    mut commands: Commands,
    camera_query: Query<&FpsCamera>,
    cursor: Query<&CursorOptions, With<PrimaryWindow>>,
    player_query: Query<&PlayerStance, With<Player>>,
) {
    if game_over.0 {
        return;
    }
    let Ok(cam) = camera_query.single() else {
        return;
    };
    let Ok(stance) = player_query.single() else {
        return;
    };

    let aiming = cursor.single().is_ok_and(|cursor| is_aiming(cursor, &mouse));
    let sprinting = keys.pressed(KeyCode::ShiftLeft) && *stance == PlayerStance::Standing;

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

    commands.client_trigger(ClientInput {
        move_axis,
        yaw: cam.yaw,
        jump: keys.just_pressed(KeyCode::Space),
        sprint: sprinting,
        aiming,
        stance: *stance,
    });
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
    cursor: Query<&CursorOptions, With<PrimaryWindow>>,
    mut camera: Query<(&Transform, &mut ViewRecoil), With<FpsCamera>>,
    mut gun: Query<&mut Gun>,
    vfx_assets: VfxAssets,
    mut cooldown: Local<f32>,
) {
    *cooldown = (*cooldown - time.delta_secs()).max(0.0);

    if game_over.0 {
        return;
    }
    let Ok(cursor) = cursor.single() else {
        return;
    };
    let firing = cursor.grab_mode != CursorGrabMode::None && mouse.pressed(MouseButton::Left);
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
        is_aiming(cursor, &mouse),
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

    // Hit detection/damage is server-authoritative now (see crates/server's
    // handle_fire_shot) — this just sends the exact ray (already carrying
    // this shot's own spread/recoil jitter) and waits for the broadcast
    // ShotFx to actually spawn hitmarker/blood/spark/floating-damage VFX
    // (see on_shot_fx), same as every other client watching this shot land.
    commands.client_trigger(FireShot {
        origin: camera_transform.translation,
        direction: *direction,
    });
}

// Plays the actual, server-decided outcome of a shot — hitmarker always,
// plus either blood+floating-damage (zombie) or sparks (wall/floor). Runs
// on every client, including the shooter, so what everyone sees always
// matches what the server actually resolved.
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
            commands.spawn((
                Mesh3d(vfx_assets.hit_assets.mesh.clone()),
                MeshMaterial3d(vfx_assets.hit_assets.material.clone()),
                Transform::from_translation(point + normal.normalize_or_zero() * 0.01),
                HitMarker {
                    remaining: HIT_MARKER_LIFETIME,
                },
            ));
            spawn_floating_damage(&mut commands, point, damage);

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
    game_over: Res<GameOver>,
    reload_sound: Res<ReloadSound>,
    mut gun: Query<&mut Gun>,
) {
    if game_over.0 || !keys.just_pressed(KeyCode::KeyR) {
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
    cursor: Query<&CursorOptions, With<PrimaryWindow>>,
    mut zoom: ResMut<ScopeZoom>,
) {
    let Ok(cursor) = cursor.single() else {
        return;
    };
    if !is_aiming(cursor, &mouse) {
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
    game_over: Res<GameOver>,
    look_delta: Res<MouseLookDelta>,
    cursor: Query<&CursorOptions, With<PrimaryWindow>>,
    camera: Query<&Transform, (With<FpsCamera>, Without<Gun>)>,
    spatial_query: SpatialQuery,
    mut query: Query<(&mut Transform, &mut Gun), Without<FpsCamera>>,
) {
    let aiming = cursor.single().is_ok_and(|cursor| is_aiming(cursor, &mouse));
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
) {
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
fn wire_zombie_animations(
    mut commands: Commands,
    anim: Res<ZombieAnim>,
    child_of: Query<&ChildOf>,
    zombies: Query<(), With<Zombie>>,
    mut new_players: Query<(Entity, &mut AnimationPlayer), Added<AnimationPlayer>>,
) {
    for (player_entity, mut player) in &mut new_players {
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

        let mut current = player_entity;
        for _ in 0..20 {
            if zombies.contains(current) {
                commands
                    .entity(current)
                    .try_insert(ZombieAnimationPlayer(player_entity));
                break;
            }
            let Ok(parent) = child_of.get(current) else {
                break;
            };
            current = parent.0;
        }
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

// Matches newly-spawned bone entities (glTF nodes get a Name component from
// their node name) against zombie.glb's known rig names, and records
// whichever ones matter for zombie_hit_damage on the zombie root's
// ZombieHitboxBones. Same "wait for the async scene load" pattern as
// wire_zombie_animations, just run against every new Name instead of every
// new AnimationPlayer.
fn wire_zombie_hitbox_bones(
    child_of: Query<&ChildOf>,
    mut zombies: Query<&mut ZombieHitboxBones, With<Zombie>>,
    new_names: Query<(Entity, &Name), Added<Name>>,
) {
    for (bone_entity, name) in &new_names {
        let is_head = name.as_str() == "Head";
        let is_chest = name.as_str() == "Spine2";
        let is_belly = name.as_str() == "Spine";
        // Upper arm/leg alone left forearm/hand/lower-leg/foot hits with no
        // nearby marker of their own, misattributing to whatever tracked
        // bone happened to be geometrically closest — covering the full
        // chain fixes that.
        let is_limb = matches!(
            name.as_str(),
            "LeftArm"
                | "RightArm"
                | "LeftForeArm"
                | "RightForeArm"
                | "LeftHand"
                | "RightHand"
                | "LeftUpLeg"
                | "RightUpLeg"
                | "LeftLeg"
                | "RightLeg"
                | "LeftFoot"
                | "RightFoot"
        );
        if !(is_head || is_chest || is_belly || is_limb) {
            continue;
        }

        let mut current = bone_entity;
        for _ in 0..20 {
            if let Ok(mut bones) = zombies.get_mut(current) {
                if is_head {
                    bones.head = Some(bone_entity);
                } else if is_chest {
                    bones.chest = Some(bone_entity);
                } else if is_belly {
                    bones.belly = Some(bone_entity);
                } else {
                    bones.limbs.push(bone_entity);
                }
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
fn update_zombie_animation(
    anim: Res<ZombieAnim>,
    mut zombies: Query<(&NetTransform, &ZombieAnimationPlayer, &mut ZombiePlayingWalk), With<Zombie>>,
    mut players: Query<&mut AnimationPlayer>,
) {
    for (net_transform, link, mut playing_walk) in &mut zombies {
        if net_transform.moving == playing_walk.0 {
            continue;
        }
        playing_walk.0 = net_transform.moving;
        if let Ok(mut player) = players.get_mut(link.0) {
            player.stop_all();
            if net_transform.moving {
                player.play(anim.walk).repeat();
            } else {
                player.play(anim.idle).repeat();
            }
        }
    }
}

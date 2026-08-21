use avian3d::prelude::*;
use bevy::animation::graph::{AnimationGraph, AnimationGraphHandle, AnimationNodeIndex};
use bevy::animation::AnimationPlayer;
use bevy::dev_tools::fps_overlay::FpsOverlayPlugin;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::RenderTarget;
use bevy::ecs::system::SystemParam;
use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::math::ops;
use bevy::picking::mesh_picking::ray_cast::{MeshRayCast, MeshRayCastSettings};
use bevy::picking::mesh_picking::MeshPickingPlugin;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
use bevy::render::view::Msaa;
use bevy::scene::SceneRoot;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow, Window};
use big_brain::prelude::*;
use vleue_navigator::prelude::*;

// Downloaded once into assets/models/ so the app doesn't depend on a network
// request (and a remote repo's file layout) every time it starts.
// To load from a URL instead, add the WebAssetPlugin back (see bevy::asset::io::web)
// and point this at an https:// path — bevy treats both the same way.
const SCENE_PATH: &str = "models/Sponza/Sponza.gltf";

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
const RECOIL_DURATION: f32 = 0.12;

// Pitches the gun barrel up out of the way when standing close to a wall in
// front (PUBG-style), so the viewmodel doesn't visibly clip through it.
// Distance is measured from the camera along its forward ray; the raise
// eases in as that distance shrinks below WALL_GUN_RAISE_DISTANCE, fully
// raised (barrel pointing straight up) at 0.
const WALL_GUN_RAISE_DISTANCE: f32 = 1.5;
const WALL_GUN_RAISE_HEIGHT: f32 = 0.18;
const WALL_GUN_RAISE_ROT_DEG: f32 = 90.0;
const WALL_GUN_RAISE_SPEED: f32 = 10.0;

const MAGAZINE_SIZE: u32 = 40;
const RELOAD_DURATION: f32 = 1.6;
// How far the gun dips down and tilts during a reload, peaking at the
// midpoint of RELOAD_DURATION and easing back to the normal pose at both
// ends (see the parabola in gun_view_animation).
const RELOAD_DIP: f32 = 0.12;
const RELOAD_TILT_DEG: f32 = 25.0;

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

// Horizontal-only impulse (see KnockbackVelocity) applied to a zombie that
// survives a hit, along the bullet's own travel direction. Headshots always
// kill outright (HEAD_DAMAGE == ZOMBIE_MAX_HEALTH) so never actually get to
// stagger — this only shows up on body/limb hits that don't finish the kill.
const KNOCKBACK_FORCE: f32 = 3.5;
const KNOCKBACK_DECAY_RATE: f32 = 6.0;

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

// Lowered from 6.0 — felt too fast.
const MOVE_SPEED: f32 = 4.0;
const MOUSE_SENSITIVITY: f32 = 0.002;

// Noclip flies faster than normal walking, along the full look direction
// (including pitch) rather than the ground-projected one — see
// move_player_physics.
const NOCLIP_SPEED: f32 = 10.0;

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
const AIM_MOVE_SPEED_MULTIPLIER: f32 = 0.5;
const AIM_SPREAD_MULTIPLIER: f32 = 0.35;

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
// Raises the scope camera's eyepoint above the main camera's (the scope
// sits on top of the barrel, not at eye level) — offset along the camera's
// own local up so it stays correct regardless of pitch. See sync_scope_camera.
const SCOPE_CAMERA_HEIGHT_OFFSET: f32 = 0.0;
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

// Player capsule: radius + cylinder length between the two hemispherical
// caps, so total standing height = PLAYER_LENGTH + 2 * PLAYER_RADIUS ≈ 1.6m.
const PLAYER_RADIUS: f32 = 0.35;
const PLAYER_LENGTH: f32 = 0.9;
const EYE_HEIGHT_OFFSET: f32 = 0.65;

// Stance (PlayerStance, KeyC/KeyX) and sprint (ShiftLeft while standing)
// each rebuild the capsule shorter and re-scale move speed every frame in
// move_player_physics — no persistent Collider to keep in sync, since the
// shape is already passed fresh to move_and_slide() each call. Crouch/prone
// capsule lengths are shorter than PLAYER_LENGTH, so the character visually
// (and physically) gets shorter, not just slower.
const SPRINT_MULTIPLIER: f32 = 1.6;
const CROUCH_SPEED_MULTIPLIER: f32 = 0.5;
const PRONE_SPEED_MULTIPLIER: f32 = 0.25;
const CROUCH_LENGTH: f32 = PLAYER_LENGTH * 0.5;
const PRONE_LENGTH: f32 = PLAYER_RADIUS * 0.3;
const CROUCH_EYE_HEIGHT_OFFSET: f32 = EYE_HEIGHT_OFFSET * 0.55;
const PRONE_EYE_HEIGHT_OFFSET: f32 = EYE_HEIGHT_OFFSET * 0.15;
// Snappier than real-world 9.81 — common for FPS games so jumps/falls don't
// feel floaty.
const GRAVITY: f32 = -20.0;
const JUMP_SPEED: f32 = 6.5;
// A hit surface counts as "ground" (for jumping/gravity reset) only if its
// normal is at least this close to straight up; steeper slopes/walls don't.
const GROUND_NORMAL_MIN_Y: f32 = 0.6;
// We don't know the real scale of this Sponza export (its own bounding box
// data was ambiguous — could be meters or centimeters), so the player spawns
// well above y=0 and lets gravity fall it onto whatever the real floor turns
// out to be, rather than guessing a spawn height that might be inside a wall.
const PLAYER_SPAWN: Vec3 = Vec3::new(0.0, 3.0, 4.0);

// How long a bullet-hole marker stays visible before despawning.
const HIT_MARKER_LIFETIME: f32 = 3.0;

// Max range for both the render-mesh raycast and the zombie-capsule
// physics raycast in shoot() — far past anything reachable inside Sponza.
const MAX_SHOT_DISTANCE: f32 = 200.0;

const PLAYER_MAX_HEALTH: f32 = 100.0;
// Discrete bites instead of continuous per-second drain: a zombie within
// ZOMBIE_STOP_DISTANCE and chasing deals ZOMBIE_ATTACK_DAMAGE once every
// ZOMBIE_ATTACK_INTERVAL seconds (see ZombieAttackTimer/zombies_damage_player)
// — no separate attack animation, same simplicity tradeoff as the zombies'
// own one-shot-kill damage model.
const ZOMBIE_ATTACK_DAMAGE: f32 = 12.0;
const ZOMBIE_ATTACK_INTERVAL: f32 = 1.0;

// "Animated Zombie" by Quaternius (poly.pizza, CC-BY 3.0) —
// https://poly.pizza/m/jkrEvQZb8J — keep this attribution if shared/published.
// Calibrated in-game against a 1m reference cube: this comes out to a ~2m
// tall zombie, appropriately looming over the player.
const ZOMBIE_SCALE: f32 = 0.1;
const ZOMBIE_PATH: &str = "models/zombie.glb";
// Y is the player's actual measured resting height (from the debug position
// log: Player @ y≈0.79), not a guess-and-let-gravity-settle value like
// before — needed because with ZOMBIE_AI_ENABLED off, zombies never get a
// Chase component and so never run move_zombies (which is where gravity is
// applied), meaning they'd otherwise just hang frozen at their spawn height
// forever instead of falling to the floor.
const ZOMBIE_SPAWNS: [Vec3; 5] = [
    Vec3::new(5.0, 0.9, -3.0),
    Vec3::new(-6.0, 0.9, -6.0),
    Vec3::new(8.0, 0.9, 3.0),
    Vec3::new(-4.0, 0.9, 6.0),
    Vec3::new(2.0, 0.9, -8.0),
];
// A dead zombie is replaced after this long, keeping the count topped up to
// ZOMBIE_SPAWNS.len() (5) instead of slowly depleting to zero.
const ZOMBIE_RESPAWN_DELAY: f32 = 1.0;
// Reuses the player's capsule dimensions (roughly human-sized).
const ZOMBIE_RADIUS: f32 = PLAYER_RADIUS;
const ZOMBIE_LENGTH: f32 = PLAYER_LENGTH;
const ZOMBIE_SPEED: f32 = 1.0;
// Body-shot kills still take this many hits; a headshot always kills in one
// (see zombie_hit_damage). Raised from 3.0 — zombies were dying too fast to
// body shots.
const ZOMBIE_MAX_HEALTH: f32 = 5.0;

// Per-region damage, picked by zombie_hit_damage from where the raycast hit
// lands relative to the zombie's root. The zombie.glb rig has no separate
// per-body-part meshes to hit-test against directly (single "Zombie_Cylinder"
// mesh, see the model's own node list) — bones exist for skinning/animation
// only, not as distinct raycast targets — so regions are approximated
// geometrically from the hit point instead of a real hitbox lookup.
const HEAD_DAMAGE: f32 = ZOMBIE_MAX_HEALTH; // always a one-shot kill
const CHEST_DAMAGE: f32 = 1.5;
const BELLY_DAMAGE: f32 = 1.0;
const LIMB_DAMAGE: f32 = 0.5;

// Height bands (world-space meters, relative to the zombie's root — which
// per ZOMBIE_HEALTH_BAR_HEIGHT's own comment sits around the physics
// capsule's center, roughly hip/waist height, not the feet) and a lateral
// distance from the vertical axis through the root, used to tell an
// out-flung arm from the torso at similar heights. Rough estimates for a
// ~2m-tall zombie — not visually calibrated.
const HITBOX_HEAD_MIN_Y: f32 = 0.9;
const HITBOX_CHEST_MIN_Y: f32 = 0.3;
const HITBOX_BELLY_MIN_Y: f32 = -0.2;
const HITBOX_LIMB_LATERAL_MIN: f32 = 0.35;

// Damage number popup: spawned at the hit point, drifts straight up in
// world space, and fades out over its lifetime.
const FLOATING_DAMAGE_LIFETIME: f32 = 0.9;
const FLOATING_DAMAGE_RISE_SPEED: f32 = 0.7;
const FLOATING_DAMAGE_FONT_SIZE: f32 = 14.0;
const FLOATING_DAMAGE_CRIT_FONT_SIZE: f32 = 24.0;

// Categorizes a raycast hit on a zombie into head/chest/belly/limb(-or-leg)
// and returns the damage for that region. Works in the zombie's own local
// space (undoes its current yaw) so the lateral-distance check means the
// same thing regardless of which way the zombie is currently facing.
// Preferred path: nearest-bone. Picks whichever known bone (head/chest/
// belly/limb) sits closest to the hit point in world space, using each
// bone's own GlobalTransform — so this tracks the zombie's actual current
// animated pose, not the bind pose the mesh raycast itself is stuck on.
// Falls back to the geometric approximation (zombie_hit_damage_geometric)
// for whichever bones haven't resolved yet (ZombieHitboxBones fills in
// asynchronously as the glTF scene loads — see wire_zombie_hitbox_bones) or
// if none have.
fn zombie_hit_damage(
    hit_point: Vec3,
    zombie_transform: &Transform,
    bones: &ZombieHitboxBones,
    globals: &Query<&GlobalTransform>,
) -> f32 {
    let candidates = bones
        .head
        .into_iter()
        .map(|e| (e, HEAD_DAMAGE))
        .chain(bones.chest.into_iter().map(|e| (e, CHEST_DAMAGE)))
        .chain(bones.belly.into_iter().map(|e| (e, BELLY_DAMAGE)))
        .chain(bones.limbs.iter().map(|&e| (e, LIMB_DAMAGE)));

    let nearest = candidates
        .filter_map(|(entity, damage)| {
            let bone_pos = globals.get(entity).ok()?.translation();
            Some((damage, hit_point.distance_squared(bone_pos)))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1));

    match nearest {
        Some((damage, _)) => damage,
        None => zombie_hit_damage_geometric(hit_point, zombie_transform),
    }
}

// Approximates head/chest/belly/limb from the hit point's height and
// lateral distance from the zombie's own vertical axis — used only as a
// fallback until wire_zombie_hitbox_bones resolves real bone entities. See
// the comment on HITBOX_* for why there's no real per-part hitbox to test
// against directly.
fn zombie_hit_damage_geometric(hit_point: Vec3, zombie_transform: &Transform) -> f32 {
    let relative = hit_point - zombie_transform.translation;
    let local = zombie_transform.rotation.inverse() * relative;
    let lateral = local.xz().length();

    if local.y >= HITBOX_HEAD_MIN_Y {
        HEAD_DAMAGE
    } else if lateral >= HITBOX_LIMB_LATERAL_MIN {
        // An arm (or, near the ground, a leg) sticking out from the torso.
        LIMB_DAMAGE
    } else if local.y >= HITBOX_CHEST_MIN_Y {
        CHEST_DAMAGE
    } else if local.y >= HITBOX_BELLY_MIN_Y {
        BELLY_DAMAGE
    } else {
        // Below the belly band and close to the central axis: legs.
        LIMB_DAMAGE
    }
}
// Flip to false to spawn zombies without their chase AI, for isolating
// whether apparent movement is the AI reacting or an actual bug.
const ZOMBIE_AI_ENABLED: bool = true;
// Large enough that zombies start chasing basically as soon as they spawn —
// this is meant as target practice, not a stealth encounter.
const ZOMBIE_CHASE_RANGE: f32 = 60.0;
// Height at which the flat navmesh plane slices the Sponza collider to build
// obstacle geometry — high enough to clear the floor slab (zombies/player
// rest at y≈0.9), low enough to still intersect columns and walls.
const NAVMESH_HEIGHT: f32 = 1.0;
// Outer walkable boundary, wound counter-clockwise (matching
// vleue_navigator's own examples). Shrunk to just the main hall (matches
// HALL_LIGHT_XS's -14..14 span) rather than the whole building — tried in
// case it reduces the obstacle-polygon count that comes out of slicing
// Sponza's single merged collider. Note: NavmeshUpdaterPlugin slices that
// collider in full regardless of these bounds (no pre-filtering), so this
// is unlikely to fix the timeout on its own — see NAVMESH_HEIGHT build_timeout
// comment.
const NAVMESH_BOUNDS: [Vec2; 4] = [
    Vec2::new(-15.0, -5.0),
    Vec2::new(15.0, -5.0),
    Vec2::new(15.0, 5.0),
    Vec2::new(-15.0, 5.0),
];
// (x, z) world positions of column obstacle holes to cut into the navmesh —
// empty for now. Add entries here once the real column positions are known
// (visually, in-game); see spawn_zombie_navmesh/column_footprint.
const NAVMESH_COLUMN_OBSTACLES: [(f32, f32); 0] = [];
// Zombies and the player have no collider registered against each other
// (each only collides with the static Sponza level, per avian3d's spatial
// index), so without a deliberate stopping distance a zombie that catches up
// just walks straight through/into the player and rides along with every
// move — this stops it a bit short instead, like a melee attack range.
const ZOMBIE_STOP_DISTANCE: f32 = 1.2;

// Cheap stand-in for real pathfinding (no navmesh — Sponza is a single
// merged mesh with no per-object separation to build one from, and every
// navmesh crate checked either needs a physics-engine-specific Collider type
// that conflicts with our avian3d version, or is pinned to an old Bevy). If
// a raycast toward the player is blocked within this distance, the zombie
// tries a handful of alternate angles and steers around whatever's in the
// way instead of pushing straight into it forever.
const ZOMBIE_AVOID_PROBE_DISTANCE: f32 = 2.5;
const ZOMBIE_AVOID_ANGLES_DEG: [f32; 6] = [25.0, -25.0, 50.0, -50.0, 80.0, -80.0];

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

// The physics body: a capsule moved each frame via avian3d's MoveAndSlide.
// Separate from the camera entity (which just gets its translation copied
// from this each frame) so look rotation and physics movement don't fight
// over the same Transform.
#[derive(Component)]
struct Player {
    vertical_velocity: f32,
    grounded: bool,
}

// KeyC toggles Standing<->Crouching, KeyX toggles Standing<->Prone (see
// toggle_player_stance). Affects move speed, the physics capsule's length
// (move_player_physics — rebuilt fresh every frame anyway, so no persistent
// Collider to keep in sync), and camera eye height (sync_camera_to_player).
#[derive(Component, Default, Clone, Copy, PartialEq, Eq)]
enum PlayerStance {
    #[default]
    Standing,
    Crouching,
    Prone,
}

#[derive(Component)]
struct Health(f32);

// Global game-over flag. Checked (early-return) at the top of the systems
// that should freeze once the player dies: move_player_physics, shoot,
// move_zombies. Reset by restart_game.
#[derive(Resource, Default)]
struct GameOver(bool);

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

// A static decal left where blood actually landed — either right at the
// hit point, or on a floor found by raycasting straight down from there.
#[derive(Component)]
struct BloodStain {
    remaining: f32,
}

// Marks the root of a zombie's spawned scene, so a raycast hit anywhere in
// its (deep, multi-node) hierarchy can be traced back up to something we can
// despawn, and so move_zombies can find every zombie each frame.
#[derive(Component)]
struct Zombie;

// Gravity/ground state, same idea as Player but tracked per-zombie.
#[derive(Component, Default)]
struct ZombieMotion {
    vertical_velocity: f32,
    grounded: bool,
}

// Horizontal impulse from a bullet hit, added on top of chase movement in
// move_zombies and exponentially decayed back to zero (same decay shape as
// ViewRecoil's kick) — a hit knocks a zombie back for a moment instead of
// its position being unaffected by anything but its own chase logic.
#[derive(Component, Default)]
struct KnockbackVelocity(Vec3);

// Counts down to the next bite — only ticks while the zombie is actually in
// range (zombies_damage_player), so wandering out of melee range and back
// doesn't let an attack "bank" while it was away. Starts at
// ZOMBIE_ATTACK_INTERVAL rather than 0.0 so a zombie that just caught up
// gets a beat before its first bite instead of landing one the instant it
// arrives.
#[derive(Component)]
struct ZombieAttackTimer(f32);

impl Default for ZombieAttackTimer {
    fn default() -> Self {
        Self(ZOMBIE_ATTACK_INTERVAL)
    }
}

// Driven by big-brain's Thinker/Scorer/Action below (NearPlayer scorer picks
// ChaseAction once in range). ChaseAction's Requested/Cancelled handlers
// insert/remove this component — move_zombies reads it to know which
// entities should be walking, and update_zombie_animation reads its
// presence/absence to pick Walk vs Idle, same as the rest of this file did
// under seldom_state.
#[derive(Component, Clone)]
#[component(storage = "SparseSet")]
struct Chase {
    target: Entity,
    speed: f32,
}

// Scorer: 1.0 when within ZOMBIE_CHASE_RANGE of the player, 0.0 otherwise.
// big-brain's FirstToScore picker (threshold below) turns that into a
// binary pick, same effect as seldom_state's near_player trigger/negation.
#[derive(Clone, Component, Debug, ScorerBuilder)]
struct NearPlayer;

fn near_player_scorer(
    player: Res<PlayerEntity>,
    transforms: Query<&Transform>,
    mut query: Query<(&Actor, &mut Score), With<NearPlayer>>,
) {
    for (Actor(actor), mut score) in &mut query {
        let (Ok(actor_transform), Ok(player_transform)) =
            (transforms.get(*actor), transforms.get(player.0))
        else {
            score.set(0.0);
            continue;
        };
        let distance = actor_transform
            .translation
            .distance(player_transform.translation);
        score.set(if distance <= ZOMBIE_CHASE_RANGE { 1.0 } else { 0.0 });
    }
}

// Action: inserts Chase on Requested (entering the picked state), removes it
// on Cancelled (score dropped below threshold, big-brain deselects it). Stays
// Executing indefinitely otherwise — chasing has no "done" state, only
// "no longer in range".
#[derive(Clone, Component, Debug, ActionBuilder)]
struct ChaseAction;

fn chase_action_system(
    player: Res<PlayerEntity>,
    mut commands: Commands,
    mut query: Query<(&Actor, &mut ActionState, &ActionSpan)>,
) {
    for (Actor(actor), mut state, span) in &mut query {
        let _guard = span.span().enter();
        match *state {
            // try_insert/try_remove instead of insert/remove: the actor can
            // already be despawned (shoot() killed it) by the time this
            // command actually applies — observed as a noisy but harmless
            // "entity already despawned" warning in testing.
            ActionState::Requested => {
                commands.entity(*actor).try_insert(Chase {
                    target: player.0,
                    speed: ZOMBIE_SPEED,
                });
                *state = ActionState::Executing;
            }
            ActionState::Cancelled => {
                commands.entity(*actor).try_remove::<Chase>();
                *state = ActionState::Failure;
            }
            _ => {}
        }
    }
}

// Points a zombie's root entity at the specific descendant entity that
// carries its AnimationPlayer, recorded once wire_zombie_animations finds it
// (the glTF scene loads asynchronously, so it doesn't exist yet at spawn time).
#[derive(Component)]
struct ZombieAnimationPlayer(Entity);

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
            BigBrainPlugin::new(PreUpdate),
            VleueNavigatorPlugin,
            // A lightweight always-on FPS overlay. Bevy 0.18 doesn't have the
            // draggable DiagnosticsOverlay panels (that's 0.19-only), so this
            // is the only diagnostics UI available here.
            FpsOverlayPlugin::default(),
        ))
        .init_resource::<Noclip>()
        .init_resource::<FreezeZombies>()
        .init_resource::<ShowHitboxes>()
        .init_resource::<GodMode>()
        .init_resource::<DebugPanelVisible>()
        .add_systems(Startup, (setup, log_window_resolution))
        .add_systems(
            PreUpdate,
            (
                chase_action_system.in_set(BigBrainSet::Actions),
                near_player_scorer.in_set(BigBrainSet::Scorers),
            ),
        )
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
                    move_player_physics,
                    sync_camera_to_player,
                    move_zombies,
                    zombies_damage_player,
                )
                    .chain(),
                (
                    check_game_over,
                    restart_game,
                    respawn_zombies,
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
                    sync_scope_camera,
                    adjust_scope_zoom,
                    gun_view_animation,
                    update_scope_lens,
                    update_ammo_ui,
                    update_crosshair,
                    draw_zombie_hitboxes,
                    update_debug_panel,
                    despawn_hit_markers,
                    despawn_muzzle_flashes,
                    update_smoke,
                    update_blood_particles,
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
    // Folded in here too (rather than its own top-level param) for the same
    // 16-param-limit reason — see the SpatialQuery addition in shoot(). Also
    // excludes GunMesh and ScopeLens now — see their own doc comments for
    // why (both sit right in front of the camera and would otherwise catch
    // the shot raycast before it reaches anything downrange).
    vfx_meshes: Query<
        'w,
        's,
        (),
        Or<(With<HitMarker>, With<MuzzleFlash>, With<Smoke>, With<GunMesh>, With<ScopeLens>)>,
    >,
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

// So systems outside setup() (like respawn_zombies) can still hand the
// player entity to a freshly spawned zombie's chase AI.
#[derive(Resource)]
struct PlayerEntity(Entity);

// Countdown timers for zombies killed in shoot(); respawn_zombies spawns a
// replacement once each hits zero, keeping the population topped back up to
// ZOMBIE_SPAWNS.len() instead of thinning out permanently.
#[derive(Resource, Default)]
struct PendingZombieRespawns(Vec<f32>);

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
    mut navmeshes: ResMut<Assets<NavMesh>>,
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

    commands.spawn((
        SceneRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(SCENE_PATH))),
        Transform::default(),
        RigidBody::Static,
        // Waits for the scene to finish loading, then generates a trimesh
        // collider for every mesh in the hierarchy — the whole level becomes
        // solid ground/walls/stairs without hand-placed colliders.
        ColliderConstructorHierarchy::new(ColliderConstructor::TrimeshFromMesh),
    ));

    spawn_zombie_navmesh(&mut commands, &mut navmeshes);

    commands.spawn((
        DirectionalLight {
            illuminance: 8_000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.9, 0.5, 0.0)),
    ));

    spawn_hall_lights(&mut commands);

    let player = commands
        .spawn((
            Transform::from_translation(PLAYER_SPAWN),
            Player {
                vertical_velocity: 0.0,
                grounded: false,
            },
            Health(PLAYER_MAX_HEALTH),
            PlayerStance::default(),
        ))
        .id();
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

    for &spawn_pos in &ZOMBIE_SPAWNS {
        spawn_zombie(&mut commands, &asset_server, spawn_pos);
    }
    commands.insert_resource(zombie_anim);
    commands.insert_resource(PlayerEntity(player));
    commands.insert_resource(PendingZombieRespawns::default());

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
    commands.spawn((
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
        RenderLayers::layer(WORLD_LAYER),
        ScopeCamera,
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
fn spawn_zombie(commands: &mut Commands, asset_server: &AssetServer, spawn_pos: Vec3) {
    let mut zombie = commands.spawn((
        SceneRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(ZOMBIE_PATH))),
        Transform::from_translation(spawn_pos).with_scale(Vec3::splat(ZOMBIE_SCALE)),
        Zombie,
        ZombieMotion::default(),
        Health(ZOMBIE_MAX_HEALTH),
        ZombieHitboxBones::default(),
        KnockbackVelocity::default(),
        ZombieAttackTimer::default(),
        // A persistent collider so shoot() can raycast against the zombie's
        // actual current position/orientation (avian3d's SpatialQuery)
        // instead of only the render mesh, which is stuck testing against
        // the animation's bind pose (see zombie_hit_damage's doc comment on
        // why the visible, animated silhouette isn't what gets hit-tested).
        // Sensor: detectable by queries but produces no contact response,
        // so it doesn't fight move_zombies' manual MoveAndSlide sweep.
        // Divided by ZOMBIE_SCALE because avian3d scales colliders by the
        // entity's own Transform.scale, and this entity's scale is
        // ZOMBIE_SCALE — without compensating, the capsule would end up
        // roughly ZOMBIE_SCALE (0.1x) the size it's declared here.
        Collider::capsule(ZOMBIE_RADIUS / ZOMBIE_SCALE, ZOMBIE_LENGTH / ZOMBIE_SCALE),
        Sensor,
    ));
    // Can be flipped off to test whether apparent zombie movement is the
    // chase AI reacting instantly, versus an actual movement-coupling bug.
    if ZOMBIE_AI_ENABLED {
        zombie.insert(
            Thinker::build()
                .picker(FirstToScore { threshold: 0.8 })
                .when(NearPlayer, ChaseAction),
        );
    }
    let zombie_entity = zombie.id();
    spawn_zombie_health_bar(commands, zombie_entity);
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
    camera: Query<(&Camera, &GlobalTransform), With<FpsCamera>>,
    zombies: Query<(&Transform, &Health), With<Zombie>>,
    mut bars: Query<(Entity, &ZombieHealthBar, &mut Node, &Children)>,
    mut fills: Query<&mut Node, (With<ZombieHealthBarFill>, Without<ZombieHealthBar>)>,
    mut texts: Query<&mut Text, With<ZombieHealthBarText>>,
) {
    let Ok((camera, camera_transform)) = camera.single() else {
        return;
    };
    for (bar_entity, bar, mut node, children) in &mut bars {
        let Ok((zombie_transform, health)) = zombies.get(bar.zombie) else {
            // try_despawn instead of despawn: observed in testing that a bar
            // can already be gone by the time this runs (exact interleaving
            // with other despawn commands the same frame isn't pinned down),
            // and a plain despawn logs a noisy "entity already despawned"
            // warning for what's a harmless no-op either way.
            commands.entity(bar_entity).try_despawn();
            continue;
        };

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

fn despawn_blood_stains(mut commands: Commands, time: Res<Time>, mut query: Query<(Entity, &mut BloodStain)>) {
    for (entity, mut stain) in &mut query {
        stain.remaining -= time.delta_secs();
        if stain.remaining <= 0.0 {
            commands.entity(entity).try_despawn();
        }
    }
}

// Builds the navmesh directly from hand-authored edges instead of slicing
// Sponza's collider (see the comment above ChaseAction... no — see the
// comment block near NAVMESH_HEIGHT: auto-generation from the collider timed
// out on the full ~262k-triangle merged mesh). This is instant and exact
// instead of an async build with a timeout, but the tradeoff is that
// NAVMESH_COLUMN_OBSTACLES has to be measured/placed by hand — currently
// empty, since column positions can't be measured without seeing the
// rendered level. steer_around_obstacles (local raycast avoidance, still
// active in move_zombies) is what currently keeps zombies off columns;
// this navmesh only contributes a straight-line route within NAVMESH_BOUNDS
// until real obstacle polygons are added here.
fn spawn_zombie_navmesh(commands: &mut Commands, navmeshes: &mut Assets<NavMesh>) {
    let mut navmesh = NavMesh::from_edge_and_obstacles(
        NAVMESH_BOUNDS.to_vec(),
        NAVMESH_COLUMN_OBSTACLES
            .iter()
            .map(|&(x, z)| column_footprint(x, z))
            .collect(),
    );
    // Local mesh-space (x, y) maps to world (x, NAVMESH_HEIGHT, z) — same
    // horizontal-plane rotation used by vleue_navigator's own avian3d
    // example, just applied to the asset's transform directly instead of an
    // entity's, since there's no NavMeshSettings/NavmeshUpdaterPlugin
    // involved in this hand-authored path.
    navmesh.set_transform(
        Transform::from_xyz(0.0, NAVMESH_HEIGHT, 0.0)
            .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
    );
    // Assets::insert only errors for Index-based ids with a stale
    // generation; get_single() is a fixed Uuid id, which per its own docs
    // never errors.
    navmeshes
        .insert(ManagedNavMesh::get_single().id(), navmesh)
        .expect("inserting under a Uuid asset id never fails");
    commands.spawn(ManagedNavMesh::single());
}

// A small square hole, wound clockwise (opposite NAVMESH_BOUNDS' CCW winding
// — polyanya/geo's convention for interior rings/holes) around (x, z).
fn column_footprint(x: f32, z: f32) -> Vec<Vec2> {
    const COLUMN_RADIUS: f32 = 0.5;
    vec![
        Vec2::new(x - COLUMN_RADIUS, z - COLUMN_RADIUS),
        Vec2::new(x - COLUMN_RADIUS, z + COLUMN_RADIUS),
        Vec2::new(x + COLUMN_RADIUS, z + COLUMN_RADIUS),
        Vec2::new(x + COLUMN_RADIUS, z - COLUMN_RADIUS),
    ]
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
    mut zombies: Query<(&Transform, &mut ZombieAttackTimer), (With<Zombie>, With<Chase>)>,
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

// Resets the player, despawns and respawns zombies, and tears down the
// GAME OVER overlay — everything spawn_zombie/setup did the first time,
// minus the one-time asset loads (ZombieAnim, PlayerEntity are still valid).
fn restart_game(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut game_over: ResMut<GameOver>,
    asset_server: Res<AssetServer>,
    mut player: Query<(&mut Transform, &mut Player, &mut Health)>,
    zombies: Query<Entity, With<Zombie>>,
    mut respawns: ResMut<PendingZombieRespawns>,
    game_over_ui: Query<Entity, With<GameOverUi>>,
) {
    if !game_over.0 || !keys.just_pressed(KeyCode::KeyR) {
        return;
    }

    let Ok((mut transform, mut player, mut health)) = player.single_mut() else {
        return;
    };
    transform.translation = PLAYER_SPAWN;
    player.vertical_velocity = 0.0;
    player.grounded = false;
    health.0 = PLAYER_MAX_HEALTH;

    for entity in &zombies {
        commands.entity(entity).try_despawn();
    }
    respawns.0.clear();
    for &spawn_pos in &ZOMBIE_SPAWNS {
        spawn_zombie(&mut commands, &asset_server, spawn_pos);
    }

    for entity in &game_over_ui {
        commands.entity(entity).try_despawn();
    }
    game_over.0 = false;
}

// Four independent hash marks instead of a fixed cross, so update_crosshair
// can push them apart from the center as bullet spread grows. direction
// points away from center: (0,-1) top, (0,1) bottom, (-1,0) left, (1,0) right.
#[derive(Component)]
struct CrosshairLine {
    direction: Vec2,
}

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
    text.0 = format!(
        "[DEBUG PANEL] (P to close)\n\
         N: Noclip [{}]\n\
         F: Freeze zombies [{}]\n\
         H: Show hitboxes [{}]\n\
         G: God mode [{}]\n\
         1: Spawn zombie in front of player\n\
         2: Kill all zombies\n\
         3: Set nearest zombie to 1 HP\n\
         4: Heal nearest zombie to full\n\
         Zombies alive: {}",
        on_off(noclip.0),
        on_off(freeze.0),
        on_off(show_hitboxes.0),
        on_off(god_mode.0),
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
    mut respawns: ResMut<PendingZombieRespawns>,
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
        spawn_zombie(&mut commands, &asset_server, player_transform.translation + forward * 2.0);
    }
    if keys.just_pressed(KeyCode::Digit2) {
        for entity in &all_zombies {
            commands.entity(entity).try_despawn();
        }
        respawns.0.clear();
    }
    if keys.just_pressed(KeyCode::Digit3) {
        set_nearest_zombie_health(&mut zombies, player_transform.translation, 1.0);
    }
    if keys.just_pressed(KeyCode::Digit4) {
        set_nearest_zombie_health(&mut zombies, player_transform.translation, ZOMBIE_MAX_HEALTH);
    }
}

fn spawn_crosshair(commands: &mut Commands) {
    commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            position_type: PositionType::Absolute,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        })
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
    mut lines: Query<(&CrosshairLine, &mut Node)>,
) {
    let Ok(recoil) = camera.single() else {
        return;
    };
    let stance = player.single().copied().unwrap_or_default();
    let aiming = cursor.single().is_ok_and(|cursor| is_aiming(cursor, &mouse));
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
    mut motion: MessageReader<MouseMotion>,
    mut query: Query<&mut FpsCamera>,
) {
    let Ok(cursor) = cursor.single() else {
        return;
    };
    if cursor.grab_mode == CursorGrabMode::None {
        motion.clear();
        return;
    }

    let delta: Vec2 = motion.read().map(|e| e.delta).sum();
    if delta == Vec2::ZERO {
        return;
    }

    for mut cam in &mut query {
        cam.yaw -= delta.x * MOUSE_SENSITIVITY;
        cam.pitch = (cam.pitch - delta.y * MOUSE_SENSITIVITY).clamp(-1.54, 1.54);
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
}

// FreezeZombies stops move_zombies (position) but not, on its own, the
// AnimationPlayer's own playback — this pauses/resumes every currently
// playing animation on each zombie to match, so a frozen zombie doesn't
// keep walking/idling in place. Runs every frame rather than reacting to
// FreezeZombies changing; pause()/resume() are idempotent so that's fine.
fn freeze_zombie_animations(
    freeze: Res<FreezeZombies>,
    zombies: Query<&ZombieAnimationPlayer, With<Zombie>>,
    mut players: Query<&mut AnimationPlayer>,
) {
    for link in &zombies {
        let Ok(mut player) = players.get_mut(link.0) else {
            continue;
        };
        for (_, active) in player.playing_animations_mut() {
            if freeze.0 {
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

fn move_player_physics(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    time: Res<Time>,
    game_over: Res<GameOver>,
    noclip: Res<Noclip>,
    camera_query: Query<&FpsCamera>,
    cursor: Query<&CursorOptions, With<PrimaryWindow>>,
    mut player_query: Query<(&mut Transform, &mut Player, &PlayerStance)>,
    mover: MoveAndSlide,
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

    if noclip.0 {
        // Flies along the full look direction (yaw + pitch), not just the
        // ground-projected one, so looking up/down and holding W moves you
        // up/down too — no gravity, no collision, no MoveAndSlide at all.
        let rotation = Quat::from_euler(EulerRot::YXZ, cam.yaw, cam.pitch, 0.0);
        let forward = rotation * Vec3::NEG_Z;
        let right = rotation * Vec3::X;

        let mut direction = Vec3::ZERO;
        if keys.pressed(KeyCode::KeyW) {
            direction += forward;
        }
        if keys.pressed(KeyCode::KeyS) {
            direction -= forward;
        }
        if keys.pressed(KeyCode::KeyD) {
            direction += right;
        }
        if keys.pressed(KeyCode::KeyA) {
            direction -= right;
        }
        if keys.pressed(KeyCode::Space) {
            direction += Vec3::Y;
        }
        if keys.pressed(KeyCode::ShiftLeft) {
            direction -= Vec3::Y;
        }
        if direction != Vec3::ZERO {
            transform.translation += direction.normalize() * NOCLIP_SPEED * time.delta_secs();
        }
        player.vertical_velocity = 0.0;
        player.grounded = false;
        return;
    }

    let aiming = cursor.single().is_ok_and(|cursor| is_aiming(cursor, &mouse));
    let sprinting = keys.pressed(KeyCode::ShiftLeft) && *stance == PlayerStance::Standing;
    let move_speed = if aiming {
        MOVE_SPEED * AIM_MOVE_SPEED_MULTIPLIER
    } else {
        match stance {
            PlayerStance::Standing if sprinting => MOVE_SPEED * SPRINT_MULTIPLIER,
            PlayerStance::Standing => MOVE_SPEED,
            PlayerStance::Crouching => MOVE_SPEED * CROUCH_SPEED_MULTIPLIER,
            PlayerStance::Prone => MOVE_SPEED * PRONE_SPEED_MULTIPLIER,
        }
    };
    let capsule_length = match stance {
        PlayerStance::Standing => PLAYER_LENGTH,
        PlayerStance::Crouching => CROUCH_LENGTH,
        PlayerStance::Prone => PRONE_LENGTH,
    };

    let yaw_rotation = Quat::from_rotation_y(cam.yaw);
    let forward = yaw_rotation * Vec3::NEG_Z;
    let right = yaw_rotation * Vec3::X;

    let mut horizontal = Vec3::ZERO;
    if keys.pressed(KeyCode::KeyW) {
        horizontal += forward;
    }
    if keys.pressed(KeyCode::KeyS) {
        horizontal -= forward;
    }
    if keys.pressed(KeyCode::KeyD) {
        horizontal += right;
    }
    if keys.pressed(KeyCode::KeyA) {
        horizontal -= right;
    }
    if horizontal != Vec3::ZERO {
        horizontal = horizontal.normalize() * move_speed;
    }

    let mut vertical_velocity = player.vertical_velocity;
    if player.grounded {
        if keys.just_pressed(KeyCode::Space) {
            vertical_velocity = JUMP_SPEED;
        } else if vertical_velocity < 0.0 {
            // Small downward bias keeps the capsule pressed against the
            // ground/slope instead of endlessly re-detecting "just landed".
            vertical_velocity = -1.0;
        }
    }
    vertical_velocity += GRAVITY * time.delta_secs();

    let velocity = horizontal + Vec3::Y * vertical_velocity;
    let shape = Collider::capsule(PLAYER_RADIUS, capsule_length);
    let config = MoveAndSlideConfig::default();
    let filter = SpatialQueryFilter::default();

    let mut grounded_this_frame = false;
    let output = mover.move_and_slide(
        &shape,
        transform.translation,
        transform.rotation,
        velocity,
        time.delta(),
        &config,
        &filter,
        |hit| {
            if hit.normal.y > GROUND_NORMAL_MIN_Y {
                grounded_this_frame = true;
            }
            MoveAndSlideHitResponse::Accept
        },
    );

    transform.translation = output.position;
    player.grounded = grounded_this_frame;
    player.vertical_velocity = if grounded_this_frame {
        vertical_velocity.min(0.0)
    } else {
        vertical_velocity
    };
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
    mut raycast: MeshRayCast,
    vfx_assets: VfxAssets,
    child_of: Query<&ChildOf>,
    mut zombies: Query<(&Transform, &mut Health, &ZombieHitboxBones, &mut KnockbackVelocity), With<Zombie>>,
    globals: Query<&GlobalTransform>,
    spatial_query: SpatialQuery,
    mut respawns: ResMut<PendingZombieRespawns>,
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
    if gun.ammo == 0 {
        // Auto-reload once the mag empties from firing — manual reload via
        // reload_gun (KeyR) is still how you top off early.
        gun.reload_remaining = RELOAD_DURATION;
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

    let ray = Ray3d::new(camera_transform.translation, direction);
    // Excludes our own decorative VFX meshes from the raycast — HitMarker in
    // particular sits proud of whatever surface it was spawned on (offset
    // along the hit normal, toward the camera), which puts it directly in
    // the path of a follow-up shot at the same spot. Without this filter, a
    // second shot could hit the first shot's leftover marker sphere instead
    // of whatever's behind it (e.g. a zombie), registering as a "hit" with
    // no damage applied — this was reported as shots on a zombie not
    // registering damage.
    let filter = |entity: Entity| !vfx_assets.vfx_meshes.contains(entity);
    let settings = MeshRayCastSettings::default().with_filter(&filter);
    let mesh_hit = raycast
        .cast_ray(ray, &settings)
        .first()
        .map(|(entity, hit)| (*entity, hit.point, hit.normal, hit.distance));

    // Also raycasts against zombies' physics capsules (the Collider/Sensor
    // added in spawn_zombie). The render-mesh raycast above only ever tests
    // the animation's bind pose, not whatever pose is actually being
    // displayed this frame (Bevy's mesh_picking doesn't read GPU skinning
    // results back to the CPU) — a shot that visually lands on a swinging
    // arm can miss entirely because that arm only exists there once
    // animation has deformed the mesh. The capsule instead tracks the
    // zombie's real Transform every frame, independent of animation, so
    // this is what actually makes hits land reliably on a moving zombie.
    let spatial_filter = SpatialQueryFilter::default();
    let physics_hit = spatial_query
        .cast_ray(ray.origin, direction, MAX_SHOT_DISTANCE, true, &spatial_filter)
        .map(|hit| (hit.entity, ray.origin + *direction * hit.distance, hit.normal, hit.distance));

    // Whichever is closer wins — a zombie's capsule can be behind a wall
    // relative to the mesh hit (or vice versa), so this isn't just "prefer
    // the physics hit".
    let closest = match (mesh_hit, physics_hit) {
        (Some(mesh), Some(physics)) if physics.3 < mesh.3 => Some(physics),
        (Some(mesh), _) => Some(mesh),
        (None, physics) => physics,
    };

    if let Some((hit_entity, hit_point, hit_normal, hit_distance)) = closest {
        info!("Hit at {:?} (distance {:.2}m)", hit_point, hit_distance);
        commands.spawn((
            Mesh3d(vfx_assets.hit_assets.mesh.clone()),
            MeshMaterial3d(vfx_assets.hit_assets.material.clone()),
            Transform::from_translation(hit_point + hit_normal.normalize_or_zero() * 0.01),
            HitMarker {
                remaining: HIT_MARKER_LIFETIME,
            },
        ));

        // The ray hits a mesh primitive somewhere deep inside the zombie's
        // glTF hierarchy (mesh_hit) or the zombie root's own capsule
        // collider directly (physics_hit) — walk up the parent chain either
        // way to find the Zombie-tagged root. Damage depends on where the
        // hit landed (zombie_hit_damage) — despawns once accumulated
        // damage reaches ZOMBIE_MAX_HEALTH.
        let mut current = hit_entity;
        for _ in 0..20 {
            if let Ok((zombie_transform, mut health, bones, mut knockback)) = zombies.get_mut(current) {
                let damage = zombie_hit_damage(hit_point, zombie_transform, bones, &globals);
                health.0 -= damage;
                spawn_floating_damage(&mut commands, hit_point, damage);

                // Only parent the wound stain onto the zombie if it's about
                // to keep existing to carry it around.
                let alive = health.0 > 0.0;
                let zombie_for_blood = alive.then(|| globals.get(current).ok().map(|g| (current, g))).flatten();
                spawn_blood_effects(
                    &mut commands,
                    &vfx_assets.blood,
                    &spatial_query,
                    &time,
                    hit_point,
                    hit_normal,
                    zombie_for_blood,
                );

                // A stagger from the bullet's own travel direction — only
                // meaningful if the zombie is still around to be knocked
                // (headshots kill outright, so never reach this).
                if alive {
                    let push = Vec3::new(direction.x, 0.0, direction.z).normalize_or_zero() * KNOCKBACK_FORCE;
                    knockback.0 += push;
                }

                if !alive {
                    commands.entity(current).try_despawn();
                    respawns.0.push(ZOMBIE_RESPAWN_DELAY);
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

// Pressing R starts a reload unless the mag's already full or a reload is
// already running. The magazine refills instantly once RELOAD_DURATION
// elapses (see gun_view_animation) — no partial reloads, no spare-mag count
// (infinite reserve ammo), same simplicity tradeoff as the rest of the gun's
// systems.
fn reload_gun(keys: Res<ButtonInput<KeyCode>>, game_over: Res<GameOver>, mut gun: Query<&mut Gun>) {
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
    // Eases in with ads_blend — same lerp-toward-target value the gun mesh
    // itself uses, so the scope camera's eyepoint rises into place over the
    // same ADS transition instead of snapping straight to full height.
    scope_transform.translation += camera_transform.up() * SCOPE_CAMERA_HEIGHT_OFFSET * gun.ads_blend;
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
    mut lens: Query<(&mut Transform, &mut Visibility), With<ScopeLens>>,
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

    if let Ok((mut transform, mut visibility)) = lens.single_mut() {
        let lens_pos = GUN_OFFSET.lerp(SCOPE_LENS_OFFSET, gun.ads_blend);
        let swapped_x = lens_pos.x * (1.0 - 2.0 * gun.shoulder_blend);
        transform.translation = Vec3::new(swapped_x, lens_pos.y, lens_pos.z);
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
                0.0,
                RECOIL_KICK_UP * recoil_t - RELOAD_DIP * dip
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

// --- Zombie AI (big-brain, vendored fork ported to Bevy 0.18) ---
//
// Each zombie's Thinker picks ChaseAction once NearPlayer scores above the
// FirstToScore threshold (defined above spawn_zombie). big-brain only
// manages *when* the Chase component is present (chase_action_system
// inserts/removes it on Requested/Cancelled) — the actual walking happens in
// the plain move_zombies system below.

// Casts a ray toward `desired_direction`; if it's blocked within
// ZOMBIE_AVOID_PROBE_DISTANCE (or closer than the target itself), tries a
// handful of angles rotated left/right around it and returns the first clear
// one. This is a cheap stand-in for real pathfinding — good enough to route
// around a single column, not a substitute for an actual navmesh. Falls back
// to the original direction if every angle is blocked too.
fn steer_around_obstacles(
    spatial_query: &SpatialQuery,
    origin: Vec3,
    desired_direction: Vec3,
    distance_to_target: f32,
    filter: &SpatialQueryFilter,
) -> Vec3 {
    let probe_distance = ZOMBIE_AVOID_PROBE_DISTANCE.min(distance_to_target);
    let is_clear = |direction: Vec3| {
        Dir3::new(direction).is_ok_and(|dir| {
            spatial_query
                .cast_ray(origin, dir, probe_distance, false, filter)
                .is_none()
        })
    };

    if is_clear(desired_direction) {
        return desired_direction;
    }
    for angle_deg in ZOMBIE_AVOID_ANGLES_DEG {
        let rotated = Quat::from_rotation_y(angle_deg.to_radians()) * desired_direction;
        if is_clear(rotated) {
            return rotated;
        }
    }
    // Every probed angle is blocked too — keep going straight. move_and_slide
    // will still stop the zombie at the actual wall instead of clipping through.
    desired_direction
}

// Walks every zombie with an active `Chase` toward its target using the same
// avian3d move-and-slide + manual gravity approach as the player, so zombies
// get the same wall/floor collision (and the same stairs caveat).
fn move_zombies(
    time: Res<Time>,
    game_over: Res<GameOver>,
    freeze: Res<FreezeZombies>,
    mut zombies: Query<(&mut Transform, &mut ZombieMotion, &Chase, &mut KnockbackVelocity), With<Zombie>>,
    targets: Query<&Transform, Without<Zombie>>,
    navmesh: Query<&ManagedNavMesh>,
    navmeshes: Res<Assets<NavMesh>>,
    mover: MoveAndSlide,
) {
    if game_over.0 || freeze.0 {
        return;
    }
    // Built once per frame (not per zombie) — Direct update mode means this
    // is ready as soon as the level's collider finishes loading and the
    // first navmesh build completes; None until then, or if the entity
    // hasn't spawned/query fails for any reason.
    let navmesh = navmesh
        .single()
        .ok()
        .and_then(|handle| navmeshes.get(handle));

    for (mut transform, mut motion, chase, mut knockback) in &mut zombies {
        let Ok(target_transform) = targets.get(chase.target) else {
            continue;
        };

        let to_target = target_transform.translation - transform.translation;
        let horizontal = Vec3::new(to_target.x, 0.0, to_target.z);
        let distance = horizontal.length();
        let filter = SpatialQueryFilter::default();
        let horizontal_velocity = if distance > ZOMBIE_STOP_DISTANCE {
            // Route via the navmesh's next waypoint when available — this is
            // what actually routes around a whole column instead of just
            // nudging past whatever the raycast probe below happens to see.
            // Falls back to a straight line at the player (with the old
            // raycast-only avoidance) if the navmesh isn't built yet or no
            // path exists between the two points.
            let desired_direction = navmesh
                .and_then(|navmesh| {
                    navmesh.transformed_path(transform.translation, target_transform.translation)
                })
                .and_then(|path| path.path.first().copied())
                .map(|waypoint| {
                    Vec3::new(
                        waypoint.x - transform.translation.x,
                        0.0,
                        waypoint.z - transform.translation.z,
                    )
                    .normalize_or_zero()
                })
                .filter(|direction| *direction != Vec3::ZERO)
                .unwrap_or(horizontal / distance);
            // Raycast probing still runs on top of the navmesh direction —
            // cheap local avoidance for anything the (fairly coarse) navmesh
            // slice doesn't resolve, e.g. another zombie in the way.
            let steered = steer_around_obstacles(
                &mover.spatial_query,
                transform.translation,
                desired_direction,
                distance,
                &filter,
            );
            steered * chase.speed
        } else {
            Vec3::ZERO
        };

        if motion.grounded && motion.vertical_velocity < 0.0 {
            motion.vertical_velocity = -1.0;
        }
        motion.vertical_velocity += GRAVITY * time.delta_secs();

        // Same exponential-decay shape as ViewRecoil's kick — knockback.0
        // is set (once, an instant impulse) in shoot() and just fades back
        // to zero here every frame after.
        let decay = (KNOCKBACK_DECAY_RATE * time.delta_secs()).min(1.0);
        knockback.0 *= 1.0 - decay;

        let velocity = horizontal_velocity + knockback.0 + Vec3::Y * motion.vertical_velocity;
        let shape = Collider::capsule(ZOMBIE_RADIUS, ZOMBIE_LENGTH);
        let config = MoveAndSlideConfig::default();

        let mut grounded_this_frame = false;
        let output = mover.move_and_slide(
            &shape,
            transform.translation,
            transform.rotation,
            velocity,
            time.delta(),
            &config,
            &filter,
            |hit| {
                if hit.normal.y > GROUND_NORMAL_MIN_Y {
                    grounded_this_frame = true;
                }
                MoveAndSlideHitResponse::Accept
            },
        );

        transform.translation = output.position;
        motion.grounded = grounded_this_frame;
        motion.vertical_velocity = if grounded_this_frame {
            motion.vertical_velocity.min(0.0)
        } else {
            motion.vertical_velocity
        };

        // Face the walk direction. look_to() points local -Z at the target,
        // but this model's rig treats local +Z as forward (confirmed: with
        // the un-negated direction, zombies walked toward the player with
        // their back turned), hence the negation.
        if horizontal_velocity != Vec3::ZERO {
            transform.look_to(-horizontal_velocity, Vec3::Y);
        }
    }
}

// Ticks down each pending respawn timer (queued by shoot() when a zombie
// dies) and spawns a replacement once it hits zero, cycling through
// ZOMBIE_SPAWNS so the population stays at 3 instead of thinning out.
fn respawn_zombies(
    time: Res<Time>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut respawns: ResMut<PendingZombieRespawns>,
    mut next_spawn_index: Local<usize>,
) {
    let dt = time.delta_secs();
    let mut i = 0;
    while i < respawns.0.len() {
        respawns.0[i] -= dt;
        if respawns.0[i] <= 0.0 {
            respawns.0.swap_remove(i);
            let spawn_pos = ZOMBIE_SPAWNS[*next_spawn_index % ZOMBIE_SPAWNS.len()];
            *next_spawn_index += 1;
            spawn_zombie(&mut commands, &asset_server, spawn_pos);
        } else {
            i += 1;
        }
    }
}

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
        commands
            .entity(player_entity)
            .insert(AnimationGraphHandle(anim.graph.clone()));
        player.play(anim.idle).repeat();

        let mut current = player_entity;
        for _ in 0..20 {
            if zombies.contains(current) {
                commands
                    .entity(current)
                    .insert(ZombieAnimationPlayer(player_entity));
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
fn update_zombie_animation(
    anim: Res<ZombieAnim>,
    started_chasing: Query<&ZombieAnimationPlayer, Added<Chase>>,
    mut stopped_chasing: RemovedComponents<Chase>,
    zombies: Query<&ZombieAnimationPlayer, With<Zombie>>,
    mut players: Query<&mut AnimationPlayer>,
) {
    for link in &started_chasing {
        if let Ok(mut player) = players.get_mut(link.0) {
            player.stop_all();
            player.play(anim.walk).repeat();
        }
    }
    for entity in stopped_chasing.read() {
        if let Ok(link) = zombies.get(entity) {
            if let Ok(mut player) = players.get_mut(link.0) {
                player.stop_all();
                player.play(anim.idle).repeat();
            }
        }
    }
}

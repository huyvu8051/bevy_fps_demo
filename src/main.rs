use avian3d::prelude::*;
// Not a glob import: bevior_tree::prelude also exports a `Node` trait that
// collides with bevy::prelude's `Node` (UI) struct.
use bevior_tree::prelude::{
    BehaviorTree, BehaviorTreePlugin, BehaviorTreeRoot, InfiniteLoop, NodeResult, Sequence,
    TaskBridge, TaskChecker, TaskDefinition, TaskEvent, TaskEventListener, TaskStatus,
    insert_while_running,
};
use bevy::animation::graph::{AnimationGraph, AnimationGraphHandle, AnimationNodeIndex};
use bevy::animation::AnimationPlayer;
use bevy::dev_tools::diagnostics_overlay::{DiagnosticsOverlay, DiagnosticsOverlayPlugin};
use bevy::dev_tools::fps_overlay::FpsOverlayPlugin;
use bevy::input::mouse::MouseMotion;
use bevy::math::ops;
use bevy::pbr::diagnostic::MaterialAllocatorDiagnosticPlugin;
use bevy::picking::mesh_picking::ray_cast::{MeshRayCast, MeshRayCastSettings};
use bevy::picking::mesh_picking::MeshPickingPlugin;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::diagnostic::MeshAllocatorDiagnosticPlugin;
use bevy::render::view::Msaa;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow, Window};
use bevy::world_serialization::WorldAssetRoot;

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
const GUN_SCALE: f32 = 0.001;
const GUN_ROTATION_Y_DEG: f32 = -90.0;
const RECOIL_KICK: f32 = 0.12;
const RECOIL_DURATION: f32 = 0.12;

// Rough position of the barrel tip, in the same camera-local space as
// GUN_OFFSET. Muzzle flash/smoke spawn here. Since we can't see the model,
// this is a guess (further forward + a bit higher than the grip) — nudge it
// once you can see whether the flash lines up with the actual muzzle.
const MUZZLE_OFFSET: Vec3 = Vec3::new(0.22, -0.12, -1.55);

const MUZZLE_FLASH_LIFETIME: f32 = 0.05;
const MUZZLE_LIGHT_INTENSITY: f32 = 120_000.0;
const MUZZLE_LIGHT_RANGE: f32 = 2.0;

const SMOKE_LIFETIME: f32 = 0.4;
const SMOKE_START_SCALE: f32 = 0.35;
const SMOKE_GROWTH_PER_SEC: f32 = 1.0;

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

const MOVE_SPEED: f32 = 6.0;
const MOUSE_SENSITIVITY: f32 = 0.002;

// Player capsule: radius + cylinder length between the two hemispherical
// caps, so total standing height = PLAYER_LENGTH + 2 * PLAYER_RADIUS ≈ 1.6m.
const PLAYER_RADIUS: f32 = 0.35;
const PLAYER_LENGTH: f32 = 0.9;
const EYE_HEIGHT_OFFSET: f32 = 0.65;
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
// Flip to false to spawn zombies without their chase AI, for isolating
// whether apparent movement is the AI reacting or an actual bug.
const ZOMBIE_AI_ENABLED: bool = true;
// Large enough that zombies start chasing basically as soon as they spawn —
// this is meant as target practice, not a stealth encounter.
const ZOMBIE_CHASE_RANGE: f32 = 60.0;
// Zombies and the player have no collider registered against each other
// (each only collides with the static Sponza level, per avian3d's spatial
// index), so without a deliberate stopping distance a zombie that catches up
// just walks straight through/into the player and rides along with every
// move — this stops it a bit short instead, like a melee attack range.
const ZOMBIE_STOP_DISTANCE: f32 = 1.2;

// Animation clip order inside zombie.glb: 0=Bite, 1=Crawl, 2=Idle, 3=Run,
// 4=Walk (read from the file's own animations list).
const ZOMBIE_ANIM_IDLE: usize = 2;
const ZOMBIE_ANIM_WALK: usize = 4;

#[derive(Component)]
struct FpsCamera {
    yaw: f32,
    pitch: f32,
}

#[derive(Component, Default)]
struct ViewRecoil {
    pitch_kick: f32,
    yaw_kick: f32,
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

#[derive(Component)]
struct Gun {
    base_offset: Vec3,
    recoil: f32,
}

#[derive(Component)]
struct HitMarker {
    remaining: f32,
}

#[derive(Component)]
struct MuzzleFlash {
    remaining: f32,
}

#[derive(Component)]
struct Smoke {
    velocity: Vec3,
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

// Inserted on a zombie by the ChaseTask while it's running (and removed when
// it stops), following bevior_tree's `insert_while_running` pattern. Read by
// move_zombies to know which entities should be walking toward the player,
// and by update_zombie_animation to pick Walk vs Idle.
#[derive(Component, Clone)]
struct Chase {
    target: Entity,
    speed: f32,
}

// Points a zombie's root entity at the specific descendant entity that
// carries its AnimationPlayer, recorded once wire_zombie_animations finds it
// (the glTF scene loads asynchronously, so it doesn't exist yet at spawn time).
#[derive(Component)]
struct ZombieAnimationPlayer(Entity);

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
            BehaviorTreePlugin::default(),
            // Draggable/collapsible perf panels + a lightweight always-on FPS overlay.
            // FpsOverlayPlugin already pulls in FrameTimeDiagnosticsPlugin itself.
            DiagnosticsOverlayPlugin,
            FpsOverlayPlugin::default(),
            MaterialAllocatorDiagnosticPlugin::<StandardMaterial>::default(),
            MeshAllocatorDiagnosticPlugin,
        ))
        .add_systems(Startup, (setup, log_window_resolution))
        .add_systems(
            Update,
            (
                grab_cursor,
                mouse_look,
                shoot,
                apply_view_recoil,
                move_player_physics,
                sync_camera_to_player,
                move_zombies,
                respawn_zombies,
                wire_zombie_animations,
                update_zombie_animation,
                gun_recoil,
                despawn_hit_markers,
                despawn_muzzle_flashes,
                update_smoke,
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
    mut tree_assets: ResMut<Assets<BehaviorTreeRoot>>,
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

    commands.spawn((
        WorldAssetRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(SCENE_PATH))),
        Transform::default(),
        RigidBody::Static,
        // Waits for the scene to finish loading, then generates a trimesh
        // collider for every mesh in the hierarchy — the whole level becomes
        // solid ground/walls/stairs without hand-placed colliders.
        ColliderConstructorHierarchy::new(ColliderConstructor::TrimeshFromMesh),
    ));

    // Drag these windows around by their header; click the header to collapse.
    commands.spawn(DiagnosticsOverlay::fps());
    commands.spawn(DiagnosticsOverlay::mesh_and_standard_material());

    commands.spawn((
        DirectionalLight {
            illuminance: 8_000.0,
            shadow_maps_enabled: true,
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
        ))
        .id();

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
        spawn_zombie(&mut commands, &asset_server, &mut tree_assets, player, spawn_pos);
    }
    commands.insert_resource(zombie_anim);
    commands.insert_resource(PlayerEntity(player));
    commands.insert_resource(PendingZombieRespawns::default());

    let camera = commands
        .spawn((
            Camera3d::default(),
            Transform::from_translation(PLAYER_SPAWN + Vec3::Y * EYE_HEIGHT_OFFSET),
            FpsCamera { yaw: 0.0, pitch: 0.0 },
            ViewRecoil::default(),
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
        ))
        .id();

    // Viewmodel: parented to the camera so it stays fixed in the bottom-right
    // of the screen regardless of where the player looks.
    commands.spawn((
        WorldAssetRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(GUN_PATH))),
        Transform::from_translation(GUN_OFFSET)
            .with_rotation(Quat::from_rotation_y(GUN_ROTATION_Y_DEG.to_radians()))
            .with_scale(Vec3::splat(GUN_SCALE)),
        Gun {
            base_offset: GUN_OFFSET,
            recoil: 0.0,
        },
        ChildOf(camera),
    ));

    spawn_crosshair(&mut commands);
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
fn spawn_zombie(
    commands: &mut Commands,
    asset_server: &AssetServer,
    tree_assets: &mut Assets<BehaviorTreeRoot>,
    player: Entity,
    spawn_pos: Vec3,
) {
    let mut zombie = commands.spawn((
        WorldAssetRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(ZOMBIE_PATH))),
        Transform::from_translation(spawn_pos).with_scale(Vec3::splat(ZOMBIE_SCALE)),
        Zombie,
        ZombieMotion::default(),
    ));
    // Can be flipped off to test whether apparent zombie movement is the
    // chase AI reacting instantly, versus an actual movement-coupling bug.
    if ZOMBIE_AI_ENABLED {
        // Wait until the player is within range, then chase until they
        // leave it again — see NearTaskDefinition/ChaseTaskDefinition below
        // for what each task actually checks.
        zombie.insert(BehaviorTree::from_node(
            InfiniteLoop::new(Sequence::new(vec![
                Box::new(TaskBridge::new(Box::new(NearTaskDefinition {
                    target: player,
                    range: ZOMBIE_CHASE_RANGE,
                }))),
                Box::new(TaskBridge::new(Box::new(ChaseTaskDefinition {
                    target: player,
                    range: ZOMBIE_CHASE_RANGE,
                    speed: ZOMBIE_SPEED,
                }))),
            ])),
            tree_assets,
        ));
    }
}

fn spawn_hall_lights(commands: &mut Commands) {
    for &x in &HALL_LIGHT_XS {
        commands.spawn((
            PointLight {
                color: Color::srgb(1.0, 0.78, 0.5),
                intensity: HALL_LIGHT_INTENSITY,
                range: 12.0,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::from_xyz(x, HALL_LIGHT_HEIGHT, 0.0),
        ));
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
                width: Val::Px(20.0),
                height: Val::Px(20.0),
                position_type: PositionType::Relative,
                ..default()
            })
            .with_children(|cross| {
                cross.spawn((
                    Node {
                        width: Val::Px(16.0),
                        height: Val::Px(2.0),
                        position_type: PositionType::Absolute,
                        left: Val::Px(2.0),
                        top: Val::Px(9.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.85)),
                ));
                cross.spawn((
                    Node {
                        width: Val::Px(2.0),
                        height: Val::Px(16.0),
                        position_type: PositionType::Absolute,
                        left: Val::Px(9.0),
                        top: Val::Px(2.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.85)),
                ));
            });
        });
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
fn apply_view_recoil(time: Res<Time>, mut query: Query<(&mut Transform, &FpsCamera, &mut ViewRecoil)>) {
    for (mut transform, cam, mut recoil) in &mut query {
        let decay = (RECOIL_RECOVERY_SPEED * time.delta_secs()).min(1.0);
        recoil.pitch_kick *= 1.0 - decay;
        recoil.yaw_kick *= 1.0 - decay;

        let pitch = (cam.pitch + recoil.pitch_kick).clamp(-1.54, 1.54);
        let yaw = cam.yaw + recoil.yaw_kick;
        transform.rotation = Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0);
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
fn move_player_physics(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    camera_query: Query<&FpsCamera>,
    mut player_query: Query<(&mut Transform, &mut Player)>,
    mover: MoveAndSlide,
) {
    let Ok(cam) = camera_query.single() else {
        return;
    };
    let Ok((mut transform, mut player)) = player_query.single_mut() else {
        return;
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
        horizontal = horizontal.normalize() * MOVE_SPEED;
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
    let shape = Collider::capsule(PLAYER_RADIUS, PLAYER_LENGTH);
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
    player_query: Query<&Transform, (With<Player>, Without<FpsCamera>)>,
    mut camera_query: Query<&mut Transform, With<FpsCamera>>,
) {
    let Ok(player_transform) = player_query.single() else {
        return;
    };
    let Ok(mut camera_transform) = camera_query.single_mut() else {
        return;
    };
    camera_transform.translation = player_transform.translation + Vec3::Y * EYE_HEIGHT_OFFSET;
}

// Holding left click (while the cursor is locked) fires automatically at
// FIRE_RATE: each shot kicks the gun model back, kicks the camera's view up
// with a bit of random jitter, and raycasts from the camera into the scene
// (with spread that grows the longer you hold the trigger). On a hit, it
// drops a small glowing marker at the impact point.
fn shoot(
    mut commands: Commands,
    time: Res<Time>,
    mouse: Res<ButtonInput<MouseButton>>,
    cursor: Query<&CursorOptions, With<PrimaryWindow>>,
    mut camera: Query<(&Transform, &mut ViewRecoil), With<FpsCamera>>,
    mut gun: Query<&mut Gun>,
    mut raycast: MeshRayCast,
    hit_assets: Res<HitMarkerAssets>,
    muzzle_fx: Res<MuzzleFxAssets>,
    child_of: Query<&ChildOf>,
    zombies: Query<(), With<Zombie>>,
    mut respawns: ResMut<PendingZombieRespawns>,
    mut cooldown: Local<f32>,
) {
    *cooldown = (*cooldown - time.delta_secs()).max(0.0);

    let Ok(cursor) = cursor.single() else {
        return;
    };
    let firing = cursor.grab_mode != CursorGrabMode::None && mouse.pressed(MouseButton::Left);
    if !firing || *cooldown > 0.0 {
        return;
    }
    *cooldown = FIRE_RATE;

    for mut gun in &mut gun {
        gun.recoil = RECOIL_DURATION;
    }

    let Ok((camera_transform, mut recoil)) = camera.single_mut() else {
        return;
    };

    recoil.pitch_kick = (recoil.pitch_kick + RECOIL_PITCH_KICK).min(RECOIL_PITCH_KICK_MAX);
    // Cheap deterministic "randomness" (no rand crate needed) so left/right
    // jitter varies shot to shot instead of always pulling the same way.
    let jitter = ops::sin(time.elapsed_secs() * 977.0);
    recoil.yaw_kick += jitter * RECOIL_YAW_JITTER;

    let heat = recoil.pitch_kick / RECOIL_PITCH_KICK_MAX;
    let spread = BULLET_SPREAD * (0.3 + heat);
    let spread_x = ops::sin(time.elapsed_secs() * 613.0) * spread;
    let spread_y = ops::sin(time.elapsed_secs() * 811.0) * spread;
    let spread_dir = *camera_transform.forward()
        + *camera_transform.right() * spread_x
        + *camera_transform.up() * spread_y;
    let direction = Dir3::new(spread_dir).unwrap_or(camera_transform.forward());

    // World-space, not parented to the camera: a flash/puff should stay put
    // where it was fired, not swing around if the player turns right after.
    let muzzle_world_pos = camera_transform.translation + camera_transform.rotation * MUZZLE_OFFSET;

    // Copying the camera's rotation onto the quad makes its face point back
    // at the camera (a cheap "billboard" — good enough since both effects
    // are gone within a fraction of a second).
    commands.spawn((
        Mesh3d(muzzle_fx.flash_mesh.clone()),
        MeshMaterial3d(muzzle_fx.flash_material.clone()),
        Transform::from_translation(muzzle_world_pos).with_rotation(camera_transform.rotation),
        PointLight {
            intensity: MUZZLE_LIGHT_INTENSITY,
            range: MUZZLE_LIGHT_RANGE,
            color: Color::srgb(1.0, 0.7, 0.3),
            shadow_maps_enabled: false,
            ..default()
        },
        MuzzleFlash {
            remaining: MUZZLE_FLASH_LIFETIME,
        },
    ));

    commands.spawn((
        Mesh3d(muzzle_fx.smoke_mesh.clone()),
        MeshMaterial3d(muzzle_fx.smoke_material.clone()),
        Transform::from_translation(muzzle_world_pos)
            .with_rotation(camera_transform.rotation)
            .with_scale(Vec3::splat(SMOKE_START_SCALE)),
        Smoke {
            velocity: *camera_transform.forward() * 0.4 + Vec3::Y * 0.5,
            remaining: SMOKE_LIFETIME,
        },
    ));

    let ray = Ray3d::new(camera_transform.translation, direction);
    let settings = MeshRayCastSettings::default();

    if let Some((hit_entity, hit)) = raycast.cast_ray(ray, &settings).first() {
        let hit_entity = *hit_entity;
        info!("Hit at {:?} (distance {:.2}m)", hit.point, hit.distance);
        commands.spawn((
            Mesh3d(hit_assets.mesh.clone()),
            MeshMaterial3d(hit_assets.material.clone()),
            Transform::from_translation(hit.point + hit.normal.normalize_or_zero() * 0.01),
            HitMarker {
                remaining: HIT_MARKER_LIFETIME,
            },
        ));

        // The ray hits a mesh primitive somewhere deep inside the zombie's
        // glTF hierarchy, not the Zombie-tagged root itself — walk up the
        // parent chain to find it. One-shot kill, no health tracking.
        let mut current = hit_entity;
        for _ in 0..20 {
            if zombies.contains(current) {
                commands.entity(current).despawn();
                respawns.0.push(ZOMBIE_RESPAWN_DELAY);
                break;
            }
            let Ok(parent) = child_of.get(current) else {
                break;
            };
            current = parent.0;
        }
    }
}

fn gun_recoil(time: Res<Time>, mut query: Query<(&mut Transform, &mut Gun)>) {
    for (mut transform, mut gun) in &mut query {
        if gun.recoil <= 0.0 {
            continue;
        }
        gun.recoil = (gun.recoil - time.delta_secs()).max(0.0);
        let t = gun.recoil / RECOIL_DURATION;
        transform.translation = gun.base_offset + Vec3::new(0.0, 0.0, RECOIL_KICK * t);
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
            commands.entity(entity).despawn();
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
            commands.entity(entity).despawn();
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
            commands.entity(entity).despawn();
        }
    }
}

// --- Zombie AI (bevior_tree) ---
//
// Each zombie runs `InfiniteLoop(Sequence(NearTask, ChaseTask))`:
//   - NearTask blocks (returns Running) until the player is within range,
//     then succeeds and hands off to ChaseTask.
//   - ChaseTask inserts a `Chase` component (read by move_zombies) for as
//     long as the player stays in range, then fails once they leave it,
//     which sends the loop back to NearTask to wait again.
//
// bevior_tree only manages *when* `Chase` is present — the actual walking
// happens in the plain `move_zombies` system below, same pattern as the
// crate's own `chase.rs` example.

#[derive(Debug)]
struct NearTaskDefinition {
    target: Entity,
    range: f32,
}
impl TaskDefinition for NearTaskDefinition {
    fn build_checker(&self) -> Box<TaskChecker> {
        let target = self.target;
        let range = self.range;
        Box::new(IntoSystem::into_system(
            move |In(entity): In<Entity>, transforms: Query<&Transform>| {
                let (Ok(zombie), Ok(player)) = (transforms.get(entity), transforms.get(target))
                else {
                    return TaskStatus::Complete(NodeResult::Failure);
                };
                if zombie.translation.distance(player.translation) <= range {
                    TaskStatus::Complete(NodeResult::Success)
                } else {
                    TaskStatus::Running
                }
            },
        ))
    }
    fn build_event_listeners(&self) -> Vec<(TaskEvent, Box<TaskEventListener>)> {
        vec![]
    }
}

#[derive(Debug)]
struct ChaseTaskDefinition {
    target: Entity,
    range: f32,
    speed: f32,
}
impl TaskDefinition for ChaseTaskDefinition {
    fn build_checker(&self) -> Box<TaskChecker> {
        let target = self.target;
        let range = self.range;
        Box::new(IntoSystem::into_system(
            move |In(entity): In<Entity>, transforms: Query<&Transform>| {
                let (Ok(zombie), Ok(player)) = (transforms.get(entity), transforms.get(target))
                else {
                    return TaskStatus::Complete(NodeResult::Failure);
                };
                if zombie.translation.distance(player.translation) <= range {
                    TaskStatus::Running
                } else {
                    TaskStatus::Complete(NodeResult::Failure)
                }
            },
        ))
    }
    fn build_event_listeners(&self) -> Vec<(TaskEvent, Box<TaskEventListener>)> {
        insert_while_running(Chase {
            target: self.target,
            speed: self.speed,
        })
    }
}

// Walks every zombie with an active `Chase` toward its target using the same
// avian3d move-and-slide + manual gravity approach as the player, so zombies
// get the same wall/floor collision (and the same stairs caveat).
fn move_zombies(
    time: Res<Time>,
    mut zombies: Query<(&mut Transform, &mut ZombieMotion, &Chase), With<Zombie>>,
    targets: Query<&Transform, Without<Zombie>>,
    mover: MoveAndSlide,
) {
    for (mut transform, mut motion, chase) in &mut zombies {
        let Ok(target_transform) = targets.get(chase.target) else {
            continue;
        };

        let to_target = target_transform.translation - transform.translation;
        let horizontal = Vec3::new(to_target.x, 0.0, to_target.z);
        let horizontal_velocity =
            if horizontal.length_squared() > ZOMBIE_STOP_DISTANCE * ZOMBIE_STOP_DISTANCE {
                horizontal.normalize() * chase.speed
            } else {
                Vec3::ZERO
            };

        if motion.grounded && motion.vertical_velocity < 0.0 {
            motion.vertical_velocity = -1.0;
        }
        motion.vertical_velocity += GRAVITY * time.delta_secs();

        let velocity = horizontal_velocity + Vec3::Y * motion.vertical_velocity;
        let shape = Collider::capsule(ZOMBIE_RADIUS, ZOMBIE_LENGTH);
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
    mut tree_assets: ResMut<Assets<BehaviorTreeRoot>>,
    player: Res<PlayerEntity>,
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
            spawn_zombie(&mut commands, &asset_server, &mut tree_assets, player.0, spawn_pos);
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

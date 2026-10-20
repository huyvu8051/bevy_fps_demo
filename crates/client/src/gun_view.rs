use bevy::prelude::*;

use crate::*;

#[derive(Resource)]
pub(crate) struct ScopeCameraEnabled(pub(crate) bool);
impl Default for ScopeCameraEnabled {
    fn default() -> Self {
        Self(true)
    }
}

#[derive(Component)]
pub(crate) struct ScopeCamera;

#[derive(Component)]
pub(crate) struct ScopeLens;

#[derive(Component)]
pub(crate) struct ScopeReticle;

/// The two glass planes of the scope, as the mesh itself marks them. Both the
/// camera that looks through the scope and the disc that shows what it saw used
/// to be placed by replaying the gun's whole animation a second time; riding
/// these instead means recoil, sway, bob and the shoulder swap come for free and
/// cannot drift out of step.
///
/// The socket's scale carries the lens radius, so a unit circle under it is the
/// real glass. A camera must take the position and the turn but never that
/// scale.
#[derive(Component)]
pub(crate) struct ScopeObjectiveSocket;

#[derive(Component)]
pub(crate) struct ScopeOcularSocket;

/// Pins the sights up so the scope can be looked at without waiting for the
/// autopilot to pick a target far enough away to want them.
pub(crate) fn force_ads() -> bool {
    std::env::var("FORCE_ADS").map(|value| value == "1").unwrap_or(false)
}

/// Meshes that do not mark their scope fall back to the tuned offsets, the same
/// way a mesh that is not in metres falls back to mesh_scale.
pub(crate) fn wire_scope_sockets(
    mut commands: Commands,
    child_of: Query<&ChildOf>,
    gun: Query<(), With<Gun>>,
    new_names: Query<(Entity, &Name), Added<Name>>,
) {
    for (entity, name) in &new_names {
        let marker = match name.as_str() {
            "Socket_ScopeObjective" => 0,
            "Socket_ScopeOcular" => 1,
            _ => continue,
        };
        let mut current = entity;
        let mut under_gun = false;
        for _ in 0..20 {
            if gun.contains(current) {
                under_gun = true;
                break;
            }
            let Ok(parent) = child_of.get(current) else { break };
            current = parent.0;
        }
        if !under_gun {
            continue;
        }
        if marker == 0 {
            commands.entity(entity).insert(ScopeObjectiveSocket);
        } else {
            commands.entity(entity).insert(ScopeOcularSocket);
        }
        info!("[scope] {name} wired");
    }
}

#[derive(Resource)]
pub(crate) struct ScopeZoom(pub(crate) f32);

pub(crate) fn update_scope_zoom_slider_visibility(
    mouse: Res<ButtonInput<MouseButton>>,
    touch_aiming: Res<TouchAiming>,
    cursor: Query<&CursorOptions, With<PrimaryWindow>>,
    mut track: Query<&mut Node, With<TouchScopeZoomSliderTrack>>,
) {
    let Ok(mut node) = track.single_mut() else {
        return;
    };
    let aiming = cursor.single().is_ok_and(|c| is_aiming(c, &mouse, touch_aiming.0));
    node.display = if aiming { Display::Flex } else { Display::None };
}

pub(crate) struct GunPoseTimings {
    recoil_t: f32,
    dip: f32,
    bob_x: f32,
    bob_y: f32,
}

pub(crate) fn gun_pose_timings(gun: &Gun) -> GunPoseTimings {
    let recoil_t = gun.recoil / RECOIL_DURATION;
    let reload_t = gun.reload_remaining / RELOAD_DURATION;

    let dip = 4.0 * reload_t * (1.0 - reload_t);
    let bob_aim_scale = 1.0 - gun.ads_blend * (1.0 - AIM_BOB_MULTIPLIER);
    let bob_x = ops::sin(gun.bob_phase) * GUN_BOB_AMOUNT_X * gun.bob_blend * bob_aim_scale;
    // abs() already doubles the rate: |sin| repeats every pi, not every tau.
    // One dip per footfall, against a full sway every two.
    let bob_y = ops::sin(gun.bob_phase).abs() * GUN_BOB_AMOUNT_Y * gun.bob_blend * bob_aim_scale;
    GunPoseTimings { recoil_t, dip, bob_x, bob_y }
}

pub(crate) fn gun_recoil_sway_rotation(gun: &Gun, timings: &GunPoseTimings) -> Quat {
    Quat::from_rotation_z(gun.look_sway_roll.to_radians())
        * Quat::from_rotation_y(RECOIL_YAW_KICK_DEG.to_radians() * gun.recoil_roll * timings.recoil_t)
        * Quat::from_rotation_z(RECOIL_ROLL_DEG.to_radians() * gun.recoil_roll * timings.recoil_t)
        * Quat::from_rotation_x((RELOAD_TILT_DEG * timings.dip + RECOIL_TILT_UP_DEG * timings.recoil_t).to_radians())
}

pub(crate) fn sync_scope_camera(
    camera: Query<&Transform, (With<FpsCamera>, Without<ScopeCamera>, Without<Gun>)>,
    gun: Query<(&Gun, &Transform), (Without<FpsCamera>, Without<ScopeCamera>)>,
    objective: Query<&GlobalTransform, With<ScopeObjectiveSocket>>,
    mut scope_camera: Query<&mut Transform, (With<ScopeCamera>, Without<FpsCamera>, Without<Gun>)>,
) {
    // Light enters a scope at the front lens, and the mesh says where that is.
    // Position and turn only: the socket's scale is the lens radius, and a
    // camera that took it would scale the whole world it renders.
    if let (Ok(socket), Ok(mut scope_transform)) = (objective.single(), scope_camera.single_mut()) {
        let placed = socket.compute_transform();
        *scope_transform =
            Transform::from_translation(placed.translation).with_rotation(placed.rotation);
        return;
    }

    let (Ok(camera_transform), Ok((gun, gun_transform)), Ok(mut scope_transform)) =
        (camera.single(), gun.single(), scope_camera.single_mut())
    else {
        return;
    };
    *scope_transform = *camera_transform;

    let local_to_world = |local: Vec3| {
        camera_transform.right() * local.x + camera_transform.up() * local.y
            - camera_transform.forward() * local.z
    };

    scope_transform.translation += local_to_world(gun_transform.translation)

        + camera_transform.up() * (SCOPE_HIP_HEIGHT_OFFSET * (1.0 - gun.ads_blend) + SCOPE_CAMERA_HEIGHT_OFFSET * gun.ads_blend);

    scope_transform.rotation *= gun_recoil_sway_rotation(gun, &gun_pose_timings(gun));
}

pub(crate) fn adjust_scope_zoom(
    mut wheel: MessageReader<MouseWheel>,
    mouse: Res<ButtonInput<MouseButton>>,
    touch_aiming: Res<TouchAiming>,
    cursor: Query<&CursorOptions, With<PrimaryWindow>>,
    mut zoom: ResMut<ScopeZoom>,
    camera: Query<&GlobalTransform, With<FpsCamera>>,
    autopilot: Res<Autopilot>,
    autopilot_state: Res<AutopilotState>,
    zombies: Query<&Transform, With<Zombie>>,
) {
    let Ok(cursor) = cursor.single() else {
        return;
    };
    if !is_aiming(cursor, &mouse, touch_aiming.0) {
        wheel.clear();
        return;
    }

    if autopilot.0 {
        if let Some(target_entity) = autopilot_state.target {
            if let (Ok(camera_global), Ok(zombie_transform)) =
                (camera.single(), zombies.get(target_entity))
            {
                let distance = camera_global.translation().distance(zombie_transform.translation);
                let t = ((distance - AUTO_ZOOM_NEAR_DISTANCE)
                    / (AUTO_ZOOM_FAR_DISTANCE - AUTO_ZOOM_NEAR_DISTANCE))
                    .clamp(0.0, 1.0);
                zoom.0 = SCOPE_MIN_ZOOM + (SCOPE_MAX_ZOOM - SCOPE_MIN_ZOOM) * t;
                wheel.clear();
                return;
            }
        }
    }
    for event in wheel.read() {
        zoom.0 = (zoom.0 * SCOPE_ZOOM_STEP_FACTOR.powf(event.y.signum()))
            .clamp(SCOPE_MIN_ZOOM, SCOPE_MAX_ZOOM);
    }
}

pub(crate) fn update_scope_lens(
    gun: Query<&Gun>,
    zoom: Res<ScopeZoom>,
    scope_camera_enabled: Res<ScopeCameraEnabled>,
    mut scope_camera: Query<(&mut Camera, &mut Projection), With<ScopeCamera>>,
    mut lens: Query<(&mut Transform, &mut Visibility), (With<ScopeLens>, Without<ScopeReticle>)>,
    mut reticle: Query<&mut Transform, (With<ScopeReticle>, Without<ScopeLens>)>,
    ocular: Query<&GlobalTransform, With<ScopeOcularSocket>>,
    eye: Query<&GlobalTransform, With<FpsCamera>>,
    active: Res<ActiveWeapon>,
) {
    let Ok(gun) = gun.single() else {
        return;
    };
    let visible = scope_camera_enabled.0
        && gun.ads_blend > SCOPE_ACTIVE_ADS_THRESHOLD
        && gun.wall_raise < SCOPE_HIDDEN_WALL_RAISE_THRESHOLD;

    if let Ok((mut camera, mut projection)) = scope_camera.single_mut() {
        camera.is_active = visible;
        if let Projection::Perspective(perspective) = projection.as_mut() {
            perspective.fov = SCOPE_BASE_FOV / zoom.0;
        }
    }

    if let Ok(mut reticle_transform) = reticle.single_mut() {
        reticle_transform.scale = Vec3::splat(SCOPE_DEFAULT_ZOOM / zoom.0);
    }

    if let Ok((mut transform, mut visibility)) = lens.single_mut() {
        *visibility = if visible { Visibility::Visible } else { Visibility::Hidden };

        // What the player looks into is the rear glass, so sit on it. The disc
        // is a unit circle and the socket's scale is the lens radius, which
        // makes the picture exactly as wide as the real eyepiece.
        if let (Ok(socket), Ok(eye)) = (ocular.single(), eye.single()) {
            *transform = socket.reparented_to(eye);
            return;
        }

        transform.scale = Vec3::splat(SCOPE_LENS_SIZE * 0.5);
        let rest = active.def().rest_offset;
        let lens = active.def().scope_lens_offset;
        let lens_x = rest.x + (lens.x - rest.x) * gun.ads_blend;
        let timings = gun_pose_timings(gun);
        let swapped_x = lens_x * (1.0 - 2.0 * gun.shoulder_blend) - RELOAD_LEFT_DIP * timings.dip + timings.bob_x;
        let lens_y = lens.y - SCOPE_LENS_HIDDEN_DROP * (1.0 - gun.ads_blend)
            + SCOPE_HIP_HEIGHT_OFFSET * (1.0 - gun.ads_blend)
            + RECOIL_KICK_UP * timings.recoil_t
            - RELOAD_DIP * timings.dip
            - timings.bob_y;
        let lens_z = rest.z + (lens.z - rest.z) * gun.ads_blend + RECOIL_KICK * timings.recoil_t;
        transform.translation = Vec3::new(swapped_x, lens_y, lens_z);
        transform.rotation = gun_recoil_sway_rotation(gun, &timings);
    }
}

pub(crate) fn gun_view_animation(
    time: Res<Time>,
    mouse: Res<ButtonInput<MouseButton>>,
    touch_aiming: Res<TouchAiming>,
    game_over: Res<GameOver>,
    look_delta: Res<MouseLookDelta>,
    cursor: Query<&CursorOptions, With<PrimaryWindow>>,
    camera: Query<(&Transform, &Lean), (With<FpsCamera>, Without<Gun>)>,
    spatial_query: SpatialQuery,
    mut query: Query<(&mut Transform, &mut Gun), Without<FpsCamera>>,
    local_moving: Res<LocalPlayerMoving>,
    local_sprinting: Res<LocalPlayerSprinting>,
    active: Res<ActiveWeapon>,
) {
    let camera_single = camera.single().ok();
    let aiming = cursor.single().is_ok_and(|cursor| is_aiming(cursor, &mouse, touch_aiming.0));

    // Reads the requested lean, not the eased camera value: keying off the
    // eased one makes the gun wait for the camera to settle before it starts
    // moving, instead of travelling alongside it.
    let leaning = camera_single
        .is_some_and(|(_, lean)| lean.target.abs() > GUN_READY_LEAN_CUTOFF);

    // Keyed off the requested lean, not the eased camera value. The swap only
    // uses the negative half of the range, so coming out of a right lean it
    // stays pinned at zero until the camera crosses the middle and then picks
    // up on the slowest part of the camera ease. That is the gun lagging the
    // press. Reading the input instead starts both at the same instant.
    let shoulder_target = camera_single.map_or(0.0, |(_, lean)| (-lean.target).clamp(0.0, 1.0));

    let wall_target = camera_single.map_or(0.0, |(camera_transform, _)| {
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
                gun.ammo = active.def().magazine;
            }
        }

        let ads_target = if aiming || force_ads() { 1.0 } else { 0.0 };
        let ads_step = (GUN_ADS_SPEED * time.delta_secs()).min(1.0);
        gun.ads_blend += (ads_target - gun.ads_blend) * ads_step;

        // Having aimed once means the player is working a sight line and will
        // likely aim again, so the gun waits near the middle instead of
        // swinging all the way back to the hip. Breaking into a sprint says
        // they are done with that sight line, so it drops to the normal rest.
        if aiming {
            gun.ready_armed = true;
        } else if local_sprinting.0 || game_over.0 {
            gun.ready_armed = false;
        }
        // Leaning takes the gun back to its shoulder: the swap is a deliberate
        // move to the other side of cover, and a centred ready pose has nothing
        // to swap. Releasing the lean settles back to ready.
        let ready_target =
            if gun.ready_armed && !aiming && !leaning { 1.0 } else { 0.0 };
        let ready_step = (GUN_READY_SPEED * time.delta_secs()).min(1.0);
        gun.ready_blend += (ready_target - gun.ready_blend) * ready_step;

        // One roll per disturbance, so each return lands somewhere new.
        let disturbed = aiming || leaning || local_sprinting.0;
        if disturbed {
            if !gun.settle_rolled {
                gun.settle_rolled = true;
                gun.settle_jitter = settle_jitter(time.elapsed_secs());
            }
        } else {
            gun.settle_rolled = false;
        }

        // Chased rather than used raw. The offset lands straight on the final
        // translation with nothing else smoothing it, so swapping the value in
        // one frame moves the gun in one frame.
        let settle_step = (GUN_SETTLE_SPEED * time.delta_secs()).min(1.0);
        let settle_gap = gun.settle_jitter - gun.settle_current;
        gun.settle_current += settle_gap * settle_step;

        let wall_raise_step = (WALL_GUN_RAISE_SPEED * time.delta_secs()).min(1.0);
        gun.wall_raise += (wall_target - gun.wall_raise) * wall_raise_step;

        let shoulder_step = (GUN_SHOULDER_SPEED * time.delta_secs()).min(1.0);
        gun.shoulder_blend += (shoulder_target - gun.shoulder_blend) * shoulder_step;

        let moving = local_moving.0;
        let bob_target = if moving { 1.0 } else { 0.0 };
        let bob_blend_step = (GUN_BOB_BLEND_SPEED * time.delta_secs()).min(1.0);
        gun.bob_blend += (bob_target - gun.bob_blend) * bob_blend_step;
        if moving {
            gun.bob_phase += GUN_BOB_SPEED * time.delta_secs();
        }

        let sway_target =
            (look_delta.0.x * LOOK_SWAY_SENSITIVITY).clamp(-LOOK_SWAY_MAX_DEG, LOOK_SWAY_MAX_DEG);
        let sway_step = (LOOK_SWAY_SPEED * time.delta_secs()).min(1.0);
        gun.look_sway_roll += (sway_target - gun.look_sway_roll) * sway_step;

        let timings = gun_pose_timings(&gun);

        let resting = gun.base_offset.lerp(active.def().ready_offset, gun.ready_blend) + gun.settle_current;
        let base_pos = resting.lerp(active.def().ads_offset, gun.ads_blend) - Vec3::Z * view_pullback();

        let swapped_x = base_pos.x * (1.0 - 2.0 * gun.shoulder_blend);
        transform.translation = Vec3::new(swapped_x, base_pos.y, base_pos.z)
            + Vec3::new(
                timings.bob_x,
                RECOIL_KICK_UP * timings.recoil_t - RELOAD_DIP * timings.dip - timings.bob_y
                    + WALL_GUN_RAISE_HEIGHT * gun.wall_raise,
                RECOIL_KICK * timings.recoil_t,
            );

        let base_rotation = Quat::from_rotation_y(active.def().mesh_yaw_deg.to_radians())
            * Quat::from_rotation_x(RELOAD_TILT_DEG.to_radians() * timings.dip);

        let wall_raise_tilt = WALL_GUN_RAISE_ROT_DEG.to_radians() * gun.wall_raise;
        transform.rotation = Quat::from_rotation_x(wall_raise_tilt)
            * Quat::from_rotation_z(gun.look_sway_roll.to_radians())
            * Quat::from_rotation_y(RECOIL_YAW_KICK_DEG.to_radians() * gun.recoil_roll * timings.recoil_t)
            * Quat::from_rotation_z(RECOIL_ROLL_DEG.to_radians() * gun.recoil_roll * timings.recoil_t)
            * Quat::from_rotation_x(RECOIL_TILT_UP_DEG.to_radians() * timings.recoil_t)
            * base_rotation;
    }
}


/// Where the gun settles, nudged a little each time. Coming to rest on the
/// exact same millimetre after every aim is what makes the pose read as a
/// still image rather than a held weapon.
fn settle_jitter(seed: f32) -> Vec3 {
    let axis = |salt: f32| (ops::sin(seed * salt) * 43758.547).fract().abs() * 2.0 - 1.0;
    Vec3::new(
        axis(12.9898) * GUN_SETTLE_JITTER.x,
        axis(78.233) * GUN_SETTLE_JITTER.y,
        axis(37.719) * GUN_SETTLE_JITTER.z,
    )
}

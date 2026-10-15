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
    let bob_y = ops::sin(gun.bob_phase * 2.0).abs() * GUN_BOB_AMOUNT_Y * gun.bob_blend * bob_aim_scale;
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
    mut scope_camera: Query<&mut Transform, (With<ScopeCamera>, Without<FpsCamera>, Without<Gun>)>,
) {
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

        let lens_x = GUN_OFFSET.x + (SCOPE_LENS_OFFSET.x - GUN_OFFSET.x) * gun.ads_blend;
        let timings = gun_pose_timings(gun);
        let swapped_x = lens_x * (1.0 - 2.0 * gun.shoulder_blend) - RELOAD_LEFT_DIP * timings.dip + timings.bob_x;
        let lens_y = SCOPE_LENS_OFFSET.y - SCOPE_LENS_HIDDEN_DROP * (1.0 - gun.ads_blend)
            + SCOPE_HIP_HEIGHT_OFFSET * (1.0 - gun.ads_blend)
            + RECOIL_KICK_UP * timings.recoil_t
            - RELOAD_DIP * timings.dip
            - timings.bob_y;
        let lens_z = GUN_OFFSET.z + (SCOPE_LENS_OFFSET.z - GUN_OFFSET.z) * gun.ads_blend + RECOIL_KICK * timings.recoil_t;
        transform.translation = Vec3::new(swapped_x, lens_y, lens_z);
        transform.rotation = gun_recoil_sway_rotation(gun, &timings);
        *visibility = if visible { Visibility::Visible } else { Visibility::Hidden };
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
) {
    let camera_single = camera.single().ok();
    let aiming = cursor.single().is_ok_and(|cursor| is_aiming(cursor, &mouse, touch_aiming.0));

    let shoulder_target = if game_over.0 {
        0.0
    } else {
        camera_single.is_some_and(|(_, lean)| lean.current < -0.5) as u8 as f32
    };

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

        let base_pos = gun.base_offset.lerp(GUN_ADS_OFFSET, gun.ads_blend);

        let swapped_x = base_pos.x * (1.0 - 2.0 * gun.shoulder_blend);
        transform.translation = Vec3::new(swapped_x, base_pos.y, base_pos.z)
            + Vec3::new(
                timings.bob_x,
                RECOIL_KICK_UP * timings.recoil_t - RELOAD_DIP * timings.dip - timings.bob_y
                    + WALL_GUN_RAISE_HEIGHT * gun.wall_raise,
                RECOIL_KICK * timings.recoil_t,
            );

        let base_rotation = Quat::from_rotation_y(GUN_ROTATION_Y_DEG.to_radians())
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


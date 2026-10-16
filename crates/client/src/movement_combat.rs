
use bevy::prelude::*;
use crate::*;

#[derive(Component)]
pub(crate) struct FpsCamera {
    pub(crate) yaw: f32,
    pub(crate) pitch: f32,
}

#[derive(Resource, Default, PartialEq, Eq, Clone, Copy)]
pub(crate) enum CameraMode {
    #[default]
    FirstPerson,
    ThirdPerson,
}

#[derive(Component)]
pub(crate) struct ThirdPersonCamera;

pub(crate) fn is_aiming(cursor: &CursorOptions, mouse: &ButtonInput<MouseButton>, touch_aiming: bool) -> bool {
    (cursor.grab_mode != CursorGrabMode::None && mouse.pressed(MouseButton::Right)) || touch_aiming
}

pub(crate) fn current_bullet_spread(pitch_kick: f32, aiming: bool, moving: bool, stance: PlayerStance, sprinting: bool) -> f32 {
    let heat = pitch_kick / RECOIL_PITCH_KICK_MAX;
    let mut spread = BULLET_SPREAD * (0.3 + heat);
    if aiming {
        spread *= AIM_SPREAD_MULTIPLIER;
    }
    if !moving {
        spread *= STATIONARY_SPREAD_MULTIPLIER;
    }

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
pub(crate) struct ViewRecoil {
    pub(crate) pitch_kick: f32,
    pub(crate) yaw_kick: f32,
}

#[derive(Component, Default)]
pub(crate) struct Lean {

    pub(crate) current: f32,
}

#[derive(Resource, Default)]
pub(crate) struct Noclip(pub(crate) bool);

#[derive(Resource)]
pub(crate) struct GunVisible(pub(crate) bool);

pub(crate) const INPUT_SEND_HZ: f32 = 32.0;


pub(crate) fn zombies_damage_player(
    time: Res<Time>,
    game_over: Res<GameOver>,
    freeze: Res<FreezeZombies>,
    god_mode: Res<GodMode>,

    mut zombies: Query<(&Transform, &mut ZombieAttackTimer), With<Zombie>>,
    mut player: Query<(&Transform, &mut Health), With<Player>>,
    mut hit_flash: ResMut<HitFlash>,
    mut elapsed: Local<f32>,
) {
    if game_over.0 || freeze.0 || god_mode.0 {
        return;
    }

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
            hit_flash.0 = 1.0;
            attack_timer.0 = ZOMBIE_ATTACK_INTERVAL;
        }
    }
}

pub(crate) fn grab_cursor(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut cursor: Query<&mut CursorOptions, With<PrimaryWindow>>,
    pending: Res<MapSelectionPending>,
    menu: Res<SettingsMenuVisible>,
) {
    let Ok(mut cursor) = cursor.single_mut() else {
        return;
    };

    if keys.just_pressed(KeyCode::Backquote) {
        if cursor.grab_mode == CursorGrabMode::None {
            cursor.grab_mode = CursorGrabMode::Locked;
            cursor.visible = false;
        } else {
            cursor.grab_mode = CursorGrabMode::None;
            cursor.visible = true;
        }
    }

    if mouse.just_pressed(MouseButton::Left)
        && cursor.grab_mode == CursorGrabMode::None
        && !pending.0
        && !menu.0
    {
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
    }

    if keys.just_pressed(KeyCode::Escape) {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    }
}

pub(crate) fn mouse_look(
    cursor: Query<&CursorOptions, With<PrimaryWindow>>,
    mouse: Res<ButtonInput<MouseButton>>,
    touch_aiming: Res<TouchAiming>,
    zoom: Res<ScopeZoom>,
    mut motion: MessageReader<MouseMotion>,
    mut look_delta: ResMut<MouseLookDelta>,
    sensitivity_scale: Res<SensitivityScale>,
    mut query: Query<&mut FpsCamera>,
    autopilot: Res<Autopilot>,
    look_only: Res<AutopilotLookOnly>,
) {

    if autopilot.0 || look_only.0 {
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

pub(crate) fn apply_view_recoil(
    time: Res<Time>,
    mut query: Query<(&mut Transform, &FpsCamera, &mut ViewRecoil, &Lean)>,
) {
    for (mut transform, cam, mut recoil, lean) in &mut query {
        let decay = (RECOIL_RECOVERY_SPEED * time.delta_secs()).min(1.0);
        recoil.pitch_kick *= 1.0 - decay;
        recoil.yaw_kick *= 1.0 - decay;

        let pitch = (cam.pitch + recoil.pitch_kick).clamp(-1.54, 1.54);
        let yaw = cam.yaw + recoil.yaw_kick;

        let roll = -LEAN_ROLL_DEG.to_radians() * lean.current;
        transform.rotation = Quat::from_euler(EulerRot::YXZ, yaw, pitch, roll);
    }
}

pub(crate) fn apply_lean_input(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    touch_lean: Res<TouchLeanHeld>,
    game_over: Res<GameOver>,
    autopilot: Res<Autopilot>,
    mut query: Query<&mut Lean>,
) {

    if autopilot.0 {
        return;
    }
    let target: f32 = if game_over.0 {
        0.0
    } else {
        let left = keys.pressed(KeyCode::KeyQ) || touch_lean.left;
        let right = keys.pressed(KeyCode::KeyE) || touch_lean.right;
        match (left, right) {
            (true, false) => -1.0,
            (false, true) => 1.0,
            _ => 0.0,
        }
    };
    for mut lean in &mut query {

        let t = (LEAN_SPEED * time.delta_secs()).min(1.0);
        lean.current += (target - lean.current) * t;
    }
}

pub(crate) fn apply_aim_zoom(
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

pub(crate) fn apply_aim_dof(
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

pub(crate) fn toggle_noclip(keys: Res<ButtonInput<KeyCode>>, mut noclip: ResMut<Noclip>) {
    if keys.just_pressed(KeyCode::KeyN) {
        noclip.0 = !noclip.0;
    }
}

pub(crate) fn toggle_camera_mode(
    keys: Res<ButtonInput<KeyCode>>,
    mut mode: ResMut<CameraMode>,
    mut player_model_visible: ResMut<PlayerModelVisible>,
    mut fps_camera: Query<&mut Camera, (With<FpsCamera>, Without<ThirdPersonCamera>)>,
    mut third_person_camera: Query<&mut Camera, (With<ThirdPersonCamera>, Without<FpsCamera>)>,
) {
    if !keys.just_pressed(KeyCode::KeyU) {
        return;
    }
    *mode = match *mode {
        CameraMode::FirstPerson => CameraMode::ThirdPerson,
        CameraMode::ThirdPerson => CameraMode::FirstPerson,
    };
    let third_person = *mode == CameraMode::ThirdPerson;
    if let Ok(mut camera) = fps_camera.single_mut() {
        camera.is_active = !third_person;
    }
    if let Ok(mut camera) = third_person_camera.single_mut() {
        camera.is_active = third_person;
    }
    player_model_visible.0 = third_person;
}

pub(crate) fn sync_third_person_camera(
    mode: Res<CameraMode>,
    spatial_query: SpatialQuery,
    eye: Query<(Entity, &Transform, &FpsCamera, &Lean), Without<ThirdPersonCamera>>,
    mut camera: Query<&mut Transform, (With<ThirdPersonCamera>, Without<FpsCamera>)>,
) {
    if *mode != CameraMode::ThirdPerson {
        return;
    }
    let Ok((eye_entity, eye_transform, cam, lean)) = eye.single() else {
        return;
    };
    let Ok(mut camera_transform) = camera.single_mut() else {
        return;
    };
    let yaw_only = Quat::from_rotation_y(cam.yaw);
    let horizontal_forward = yaw_only * Vec3::NEG_Z;
    let horizontal_right = yaw_only * Vec3::X;

    let side_offset = THIRD_PERSON_SIDE_OFFSET * (lean.current + THIRD_PERSON_SHOULDER_BIAS).clamp(-1.0, 1.0);
    let desired = eye_transform.translation - horizontal_forward * THIRD_PERSON_DISTANCE
        + horizontal_right * side_offset
        + Vec3::Y * THIRD_PERSON_HEIGHT_OFFSET;
    let offset = desired - eye_transform.translation;
    let distance = offset.length();
    let Ok(direction) = Dir3::new(offset) else {
        return;
    };
    let filter = SpatialQueryFilter::from_excluded_entities([eye_entity]);
    let safe_distance = spatial_query
        .cast_ray(eye_transform.translation, direction, distance, true, &filter)
        .map(|hit| (hit.distance - THIRD_PERSON_COLLISION_MARGIN).max(0.0))
        .unwrap_or(distance);
    camera_transform.translation = eye_transform.translation + direction * safe_distance;

    let aim_dir = Quat::from_euler(EulerRot::YXZ, cam.yaw, cam.pitch, 0.0) * Vec3::NEG_Z;
    let look_target = eye_transform.translation + aim_dir * THIRD_PERSON_LOOK_AHEAD_DISTANCE;
    camera_transform.look_at(look_target, Vec3::Y);
}

pub(crate) fn toggle_player_stance(keys: Res<ButtonInput<KeyCode>>, mut player: Query<&mut PlayerStance>) {
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

pub(crate) fn send_player_input(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    touch: TouchGameplayInputs,
    time: Res<Time>,
    game_over: Res<GameOver>,
    mut sequence: Local<u32>,
    camera_query: Query<&FpsCamera>,
    cursor: Query<&CursorOptions, With<PrimaryWindow>>,
    mut player_query: Query<(&mut Transform, &mut Player, &PlayerStance), Without<NetPlayer>>,
    noclip: Res<Noclip>,
    touch_move_axis: Res<TouchMoveAxis>,

    (mut local_moving, mut local_sprinting, mut local_move_axis, autopilot, intent): (
        ResMut<LocalPlayerMoving>,
        ResMut<LocalPlayerSprinting>,
        ResMut<LocalPlayerMoveAxis>,
        Res<Autopilot>,
        Res<AutopilotIntent>,
    ),
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
    let stance = *stance;

    let aiming = cursor.single().is_ok_and(|cursor| is_aiming(cursor, &mouse, touch.aiming.0));

    let (move_axis, sprinting) = if autopilot.0 {
        (intent.move_axis, intent.sprint)
    } else {
        let sprinting = (keys.pressed(KeyCode::ShiftLeft) || touch.sprint.0) && stance == PlayerStance::Standing;
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

        (
            (move_axis + touch_move_axis.0).clamp_length_max(1.0),
            sprinting,
        )
    };
    local_moving.0 = move_axis != Vec2::ZERO;
    local_sprinting.0 = sprinting;
    local_move_axis.0 = move_axis;

    *sequence += 1;
    let dt = time.delta_secs();

    let effective_stance = if noclip.0 && touch.noclip_fly.down { PlayerStance::Crouching } else { stance };
    let input = ClientInput {
        sequence: *sequence,
        move_axis,
        yaw: cam.yaw,

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

}

pub(crate) fn sync_camera_to_player(
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

    let side_offset = *camera_transform.right() * LEAN_SIDE_OFFSET * lean.current;
    camera_transform.translation = eye + side_offset;
}

pub(crate) fn sync_player_model(
    visible: Res<PlayerModelVisible>,
    tuning: Res<PlayerModelTuning>,
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

    transform.scale = Vec3::splat(tuning.scale);
    transform.translation = player_transform.translation + Vec3::Y * tuning.y_offset;

    transform.rotation = Quat::from_rotation_y(yaw + tuning.facing_deg.to_radians());
}

fn local_yaw_correction(parent_rotation: Quat, world_angle: f32) -> Quat {
    let world_yaw = Quat::from_rotation_y(world_angle);
    parent_rotation.inverse() * world_yaw * parent_rotation
}

fn wrap_to_pi(angle: f32) -> f32 {
    let two_pi = std::f32::consts::TAU;
    let wrapped = (angle + std::f32::consts::PI).rem_euclid(two_pi) - std::f32::consts::PI;
    wrapped
}

pub(crate) fn apply_player_model_travel_twist(
    move_axis: Res<LocalPlayerMoveAxis>,
    sprinting: Res<LocalPlayerSprinting>,
    bones: Query<&PlayerModelSkeletonBones, With<PlayerModel>>,
    child_of: Query<&ChildOf>,
    globals: Query<&GlobalTransform>,
    mut transforms: Query<&mut Transform>,
) {
    let Ok(bones) = bones.single() else {
        return;
    };
    let axis = move_axis.0;
    if axis == Vec2::ZERO {
        return;
    }

    let angle = -axis.x.atan2(axis.y);

    let running_backward = sprinting.0 && axis.y < 0.0;
    let angle = if running_backward { wrap_to_pi(angle - std::f32::consts::PI) } else { angle };

    for bone in [bones.left_upper_leg, bones.right_upper_leg] {
        let Some(bone) = bone else { continue };
        let Ok(parent) = child_of.get(bone) else { continue };
        let Ok(parent_global) = globals.get(parent.0) else { continue };
        let Ok(mut t) = transforms.get_mut(bone) else { continue };
        t.rotation = local_yaw_correction(parent_global.rotation(), angle) * t.rotation;
    }
}

pub(crate) fn nudge_player_model_tuning(keys: Res<ButtonInput<KeyCode>>, mut tuning: ResMut<PlayerModelTuning>) {
    const SCALE_STEP: f32 = 0.02;
    const Y_OFFSET_STEP: f32 = 0.02;
    if keys.just_pressed(KeyCode::Comma) {
        tuning.scale = (tuning.scale - SCALE_STEP).max(0.05);
    }
    if keys.just_pressed(KeyCode::Period) {
        tuning.scale += SCALE_STEP;
    }
    if keys.just_pressed(KeyCode::Minus) {
        tuning.y_offset -= Y_OFFSET_STEP;
    }
    if keys.just_pressed(KeyCode::Equal) {
        tuning.y_offset += Y_OFFSET_STEP;
    }
}

pub(crate) fn save_player_model_tuning(
    keys: Res<ButtonInput<KeyCode>>,
    tuning: Res<PlayerModelTuning>,
    mut toast: ResMut<F9SaveToast>,
) {
    if !keys.just_pressed(KeyCode::F9) {
        return;
    }
    let out = format_character_ron(&CharacterRon {
        id: tuning.id.clone(),
        display_name: tuning.display_name.clone(),
        lore_ref: tuning.lore_ref.clone(),
        mesh: tuning.mesh.clone(),
        anim_source: tuning.anim_source.clone(),
        scale: tuning.scale,
        y_offset: tuning.y_offset,
        facing_deg: tuning.facing_deg,
        animations: tuning.animations.clone(),
        audio: tuning.audio.clone(),
    });
    let dir = character_dir_from_mesh_path(&tuning.mesh);
    let path = format!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/{}/character.ron"), dir);
    match std::fs::write(&path, out) {
        Ok(()) => {
            info!("saved {} character.ron -> {path}", tuning.id);
            toast.ok_also("F9: saved character.ron", &format!(" + {}", tuning.id));
        }
        Err(e) => {
            error!("F9 {} character.ron save failed writing {path}: {e}", tuning.id);
            toast.err(format!("F9: {} character.ron save FAILED — {e}", tuning.id));
        }
    }
}

pub(crate) fn shoot(
    mut commands: Commands,
    time: Res<Time>,
    game_over: Res<GameOver>,
    mouse: Res<ButtonInput<MouseButton>>,
    touch_aiming: Res<TouchAiming>,
    cursor: Query<&CursorOptions, With<PrimaryWindow>>,
    mut camera: Query<(&Transform, &mut ViewRecoil), With<FpsCamera>>,
    mut gun: Query<&mut Gun>,
    mut vfx_assets: VfxAssets,
    zombies: Query<(Entity, &ZombieHitboxBones), With<Zombie>>,
    globals: Query<&GlobalTransform>,
    spatial_query: SpatialQuery,
    touch_firing: Res<TouchFiring>,
    mut cooldown: Local<f32>,
    mut smoke_materials: ResMut<Assets<MuzzleSmokeMaterial>>,
    (autopilot, intent, local_moving, local_sprinting, show_hit_markers, show_tracers): (
        Res<Autopilot>,
        Res<AutopilotIntent>,
        Res<LocalPlayerMoving>,
        Res<LocalPlayerSprinting>,
        Res<ShowHitMarkers>,
        Res<ShowBulletTracers>,
    ),
) {
    *cooldown = (*cooldown - time.delta_secs()).max(0.0);

    if game_over.0 {
        return;
    }
    let Ok(cursor) = cursor.single() else {
        return;
    };

    let firing = if autopilot.0 {
        intent.fire
    } else {
        (cursor.grab_mode != CursorGrabMode::None && mouse.pressed(MouseButton::Left)) || touch_firing.0
    };
    if !firing || *cooldown > 0.0 {
        return;
    }
    let Ok(mut gun) = gun.single_mut() else {
        return;
    };

    if gun.ammo == 0 || gun.reload_remaining > 0.0 {
        return;
    }
    *cooldown = FIRE_RATE;
    gun.ammo -= 1;
    gun.recoil = RECOIL_DURATION;

    gun.recoil_roll = ops::sin(time.elapsed_secs() * 641.0);
    commands.spawn((AudioPlayer(vfx_assets.gunshot.0.clone()), PlaybackSettings::DESPAWN, GunshotVoice));
    if gun.ammo == 0 {

        gun.reload_remaining = RELOAD_DURATION;
        commands.spawn((AudioPlayer(vfx_assets.reload_sound.0.clone()), PlaybackSettings::DESPAWN));
    }

    let Ok((camera_transform, mut recoil)) = camera.single_mut() else {
        return;
    };

    recoil.pitch_kick = (recoil.pitch_kick + RECOIL_PITCH_KICK).min(RECOIL_PITCH_KICK_MAX);

    let jitter = ops::sin(time.elapsed_secs() * 977.0);
    recoil.yaw_kick += jitter * RECOIL_YAW_JITTER;

    let stance = vfx_assets.stance.single().copied().unwrap_or_default();

    let spread = current_bullet_spread(
        recoil.pitch_kick,
        is_aiming(cursor, &mouse, touch_aiming.0),
        local_moving.0,
        stance,
        local_sprinting.0,
    );
    let spread_x = ops::sin(time.elapsed_secs() * 613.0) * spread;
    let spread_y = ops::sin(time.elapsed_secs() * 811.0) * spread;
    let spread_dir = *camera_transform.forward()
        + *camera_transform.right() * spread_x
        + *camera_transform.up() * spread_y;
    let direction = Dir3::new(spread_dir).unwrap_or(camera_transform.forward());

    let swapped_muzzle_offset = Vec3::new(
        MUZZLE_OFFSET.x * (1.0 - 2.0 * gun.shoulder_blend),
        MUZZLE_OFFSET.y,
        MUZZLE_OFFSET.z,
    );

    let muzzle_world_pos =
        camera_transform.translation + camera_transform.rotation * swapped_muzzle_offset;

    commands.spawn((
        Mesh3d(vfx_assets.muzzle_fx.flash_mesh.clone()),
        MeshMaterial3d(vfx_assets.muzzle_fx.flash_material.clone()),
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
        Mesh3d(vfx_assets.muzzle_fx.smoke_mesh.clone()),
        MeshMaterial3d(smoke_materials.add(muzzle_smoke_material(time.elapsed_secs() * 173.0))),
        Transform::from_translation(muzzle_world_pos)
            .with_rotation(camera_transform.rotation)
            .with_scale(Vec3::splat(SMOKE_START_SCALE)),
        Smoke {
            velocity: *camera_transform.forward() * 0.4 + Vec3::Y * 0.5,
            remaining: SMOKE_LIFETIME,
        },
    ));

    let wall_distance = spatial_query
        .cast_ray(camera_transform.translation, direction, MAX_SHOT_DISTANCE, true, &SpatialQueryFilter::default())
        .map(|hit| hit.distance);
    let predicted_hit = zombies
        .iter()
        .filter_map(|(entity, bones)| {
            let bone_positions = resolve_bone_world_positions(bones, |bone| globals.get(bone).ok().map(GlobalTransform::translation));
            zombie_hit_damage_ray(camera_transform.translation, direction, MAX_SHOT_DISTANCE, &bone_positions)
                .map(|(point, normal, _damage)| (point.distance(camera_transform.translation), entity, point, normal))
        })
        .filter(|(distance, ..)| !wall_distance.is_some_and(|w| w < distance - 0.1))
        .min_by(|(a, ..), (b, ..)| a.total_cmp(b));
    if let Some((distance, entity, point, normal)) = predicted_hit {
        info!("[shot] local prediction: real bone hit zombie={entity} distance={distance:.2}");
        if show_hit_markers.0 {
            commands.spawn((
                Mesh3d(vfx_assets.hit_assets.mesh.clone()),
                MeshMaterial3d(vfx_assets.hit_assets.material.clone()),
                Transform::from_translation(point + normal.normalize_or_zero() * 0.01),
                HitMarker {
                    remaining: HIT_MARKER_LIFETIME,
                },
            ));
        }

        if autopilot.0 && is_aiming(cursor, &mouse, touch_aiming.0) {
            *cooldown = FIRE_RATE * (1.0 + (distance / 10.0).min(4.0));
        }

        vfx_assets.recently_hit.0.insert(entity, RECENTLY_HIT_ZOMBIE_TTL);
    }

    let tracer_end = predicted_hit.map_or(camera_transform.translation + *direction * MAX_SHOT_DISTANCE, |(_, _, point, _)| point);
    let tracer_start = muzzle_world_pos + Vec3::Y * TRACER_START_Y_OFFSET;
    spawn_bullet_tracer(&mut commands, &vfx_assets.tracer, time.elapsed_secs() * 733.0, tracer_start, tracer_end, show_tracers.0);

    info!("[shot] fired: origin={:.2?} direction={:.2?}", camera_transform.translation, *direction);
    commands.trigger(FireShot {
        origin: camera_transform.translation,
        direction: *direction,
    });
}

pub(crate) fn reload_gun(
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

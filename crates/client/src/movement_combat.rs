
use bevy::prelude::*;
use crate::*;

#[derive(Component)]
pub(crate) struct FpsCamera {
    pub(crate) yaw: f32,
    pub(crate) pitch: f32,
}

/// Whether the right button is held down for the sights, or clicked to put
/// them up and clicked again to drop them.
#[derive(Resource, Default)]
pub(crate) struct AimToggleMode(pub(crate) bool);

/// Whether the sights are wanted, as of this frame. One place decides it, so
/// nothing downstream has to know which of the two modes is on.
#[derive(Resource, Default)]
pub(crate) struct AimHeld(pub(crate) bool);

pub(crate) fn update_aim_held(
    mode: Res<AimToggleMode>,
    mouse: Res<ButtonInput<MouseButton>>,
    cursor: Query<&CursorOptions, With<PrimaryWindow>>,
    game_over: Res<GameOver>,
    mut held: ResMut<AimHeld>,
) {
    // Nothing is aimed through a menu or a death screen, and a toggle left on
    // across either would come back up on its own.
    let free = cursor.single().is_ok_and(|c| c.grab_mode != CursorGrabMode::None) && !game_over.0;
    if !free {
        held.0 = false;
        return;
    }
    held.0 = if mode.0 {
        held.0 ^ mouse.just_pressed(MouseButton::Right)
    } else {
        mouse.pressed(MouseButton::Right)
    };
}

pub(crate) fn is_aiming(cursor: &CursorOptions, held: &AimHeld, touch_aiming: bool) -> bool {
    (cursor.grab_mode != CursorGrabMode::None && held.0) || touch_aiming
}

/// `climb_max` is the firing weapon's own ceiling: heat is how far into
/// *this* weapon's climb the burst has got, or a gentler one reads as hot.
pub(crate) fn current_bullet_spread(climb: f32, climb_max: f32, aiming: bool, moving: bool, stance: PlayerStance, sprinting: bool) -> f32 {
    let heat = climb / climb_max;
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
    pub(crate) climb: f32,
    pub(crate) wander: f32,
    pub(crate) shove: f32,
    /// Where each of the three above is headed. The shot moves these; the
    /// three chase them, so the view is struck over a few frames instead of
    /// being moved whole in one.
    pub(crate) climb_target: f32,
    pub(crate) wander_target: f32,
    pub(crate) shove_target: f32,
}

#[derive(Component, Default)]
pub(crate) struct Lean {

    pub(crate) current: f32,
    pub(crate) target: f32,
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
    active: Res<ActiveWeapon>,
    cursor: Query<&CursorOptions, With<PrimaryWindow>>,
    aim_held: Res<AimHeld>,
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
    let sensitivity = if is_aiming(cursor, &aim_held, touch_aiming.0) {
        MOUSE_SENSITIVITY * AIM_MOUSE_SENSITIVITY_MULTIPLIER * (active.def().scope.default_zoom / zoom.0)
    } else {
        MOUSE_SENSITIVITY
    } * sensitivity_scale.0;
    // Radians the view turned, not pixels the mouse moved. Sway is a weapon
    // left behind by a turn, so it has to be measured against the turn: read
    // off the mouse it ignored both the aim multiplier and the magnification,
    // and a scope that slowed the view to a quarter swam four times as much.
    look_delta.0 = delta * sensitivity;
    if delta == Vec2::ZERO {
        return;
    }

    for mut cam in &mut query {
        cam.yaw -= delta.x * sensitivity;
        cam.pitch = (cam.pitch - delta.y * sensitivity).clamp(-PITCH_CLAMP, PITCH_CLAMP);
    }
}

pub(crate) fn apply_view_recoil(
    time: Res<Time>,
    active: Res<ActiveWeapon>,
    mut query: Query<(&mut Transform, &FpsCamera, &mut ViewRecoil, &Lean)>,
) {
    for (mut transform, cam, mut recoil, lean) in &mut query {
        // The marks ease home at the weapon's recovery rate; the view runs
        // after them far faster. That split is what reads as a blow absorbed
        // rather than a view being dragged: it leaves hard, slows as it
        // reaches the end of its travel, and only then comes back.
        let kick = &active.def().recoil;
        let settling = ops::exp(-kick.view.recover_speed * time.delta_secs());
        recoil.climb_target *= settling;
        recoil.wander_target *= settling;
        recoil.shove_target *= settling;

        let strike = 1.0 - ops::exp(-kick.strike_speed * time.delta_secs());
        recoil.climb += (recoil.climb_target - recoil.climb) * strike;
        recoil.wander += (recoil.wander_target - recoil.wander) * strike;
        recoil.shove += (recoil.shove_target - recoil.shove) * strike;

        let pitch = (cam.pitch + recoil.climb).clamp(-1.54, 1.54);
        let yaw = cam.yaw + recoil.wander;

        let roll = -LEAN_ROLL_DEG.to_radians() * lean.current;
        transform.rotation = Quat::from_euler(EulerRot::YXZ, yaw, pitch, roll);
    }
}

fn lean_logging() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("LEAN_LOG").is_ok())
}

pub(crate) fn apply_lean_input(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    touch_lean: Res<TouchLeanHeld>,
    game_over: Res<GameOver>,
    autopilot: Res<Autopilot>,
    auto_lean: Res<AutoLean>,
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
            _ => auto_lean.suggestion,
        }
    };
    for mut lean in &mut query {

        lean.target = target;
        let t = (LEAN_SPEED * time.delta_secs()).min(1.0);
        let before = lean.current;
        lean.current += (target - lean.current) * t;
        if lean_logging() && (target != 0.0 || lean.current.abs() > 0.001) {
            let dt = time.delta_secs().max(1e-5);
            info!(
                "[lean] target={:+.2} current={:+.4} rate={:6.2}/s roll={:+6.1}deg",
                target,
                lean.current,
                (lean.current - before) / dt,
                -LEAN_ROLL_DEG * lean.current,
            );
        }
    }
}

pub(crate) fn apply_aim_zoom(
    time: Res<Time>,
    aim_held: Res<AimHeld>,
    touch_aiming: Res<TouchAiming>,
    cursor: Query<&CursorOptions, With<PrimaryWindow>>,
    mut camera: Query<&mut Projection, With<FpsCamera>>,
) {
    let Ok(cursor) = cursor.single() else {
        return;
    };
    let target_fov = if is_aiming(cursor, &aim_held, touch_aiming.0) {
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
    aim_held: Res<AimHeld>,
    touch_aiming: Res<TouchAiming>,
    aim_blur: Res<AimBlurEnabled>,
    dof_focal_alt: Res<AimDofFocalDistanceAlt>,
    cursor: Query<&CursorOptions, With<PrimaryWindow>>,
    camera: Query<Entity, With<FpsCamera>>,
) {
    let (Ok(cursor), Ok(camera_entity)) = (cursor.single(), camera.single()) else {
        return;
    };
    // force_ads() counts, the way it does for the weapon pose. A flag that
    // pins the sights up but leaves the sighted post-processing off makes a
    // measurement run read cheaper than the game it is measuring.
    if aim_blur.0 && (is_aiming(cursor, &aim_held, touch_aiming.0) || force_ads()) {
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
    aim_held: Res<AimHeld>,
    keys: Res<ButtonInput<KeyCode>>,
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

    let aiming = cursor.single().is_ok_and(|cursor| is_aiming(cursor, &aim_held, touch.aiming.0));

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
    mut camera_query: Query<(&mut Transform, &Lean, &ViewRecoil), With<FpsCamera>>,
) {
    let Ok((player_transform, stance)) = player_query.single() else {
        return;
    };
    let Ok((mut camera_transform, lean, recoil)) = camera_query.single_mut() else {
        return;
    };
    let eye_height_offset = match stance {
        PlayerStance::Standing => EYE_HEIGHT_OFFSET,
        PlayerStance::Crouching => CROUCH_EYE_HEIGHT_OFFSET,
        PlayerStance::Prone => PRONE_EYE_HEIGHT_OFFSET,
    };

    let eye = player_transform.translation + Vec3::Y * eye_height_offset;

    let side_offset = *camera_transform.right() * LEAN_SIDE_OFFSET * lean.current;
    let pullback = *camera_transform.back() * view_pullback();
    // Along the camera's own back, not a levelled one: a kick that stays
    // horizontal while the view climbs reads as the player being shoved.
    let shove = *camera_transform.back() * recoil.shove;
    camera_transform.translation = eye + side_offset + pullback + shove;
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
    aim_held: Res<AimHeld>,
    touch_aiming: Res<TouchAiming>,
    cursor: Query<&CursorOptions, With<PrimaryWindow>>,
    mut camera: Query<(&Transform, &mut ViewRecoil), With<FpsCamera>>,
    mut gun: Query<(&mut Gun, &Transform), Without<FpsCamera>>,
    mut vfx_assets: VfxAssets,
    zombies: Query<(Entity, &ZombieHitboxBones), With<Zombie>>,
    globals: Query<&GlobalTransform>,
    spatial_query: SpatialQuery,
    (touch_firing, casings, mouse): (Res<TouchFiring>, Res<CasingAssets>, Res<ButtonInput<MouseButton>>),
    mut cooldown: Local<f32>,
    (mut smoke_materials, mut flash_materials): (
        ResMut<Assets<MuzzleSmokeMaterial>>,
        ResMut<Assets<MuzzleFlashMaterial>>,
    ),
    (autopilot, intent, local_moving, local_sprinting, show_hit_markers, show_tracers, active, muzzle_smoke, port_smoke): (
        Res<Autopilot>,
        Res<AutopilotIntent>,
        Res<LocalPlayerMoving>,
        Res<LocalPlayerSprinting>,
        Res<ShowHitMarkers>,
        Res<ShowBulletTracers>,
        Res<ActiveWeapon>,
        Res<MuzzleSmokeEnabled>,
        Res<PortSmokeEnabled>,
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
    let Ok((mut gun, gun_transform)) = gun.single_mut() else {
        return;
    };

    if gun.ammo == 0 || gun.reload_remaining > 0.0 {
        return;
    }
    *cooldown = active.def().fire_rate;
    gun.ammo -= 1;
    gun.recoil_target = 1.0;

    gun.recoil_roll = ops::sin(time.elapsed_secs() * 641.0);
    commands.spawn((AudioPlayer(vfx_assets.gunshot.of(active.0)), PlaybackSettings::DESPAWN, GunshotVoice));
    if gun.ammo == 0 {

        gun.reload_remaining = active.def().reload_duration;
        commands.spawn((AudioPlayer(vfx_assets.reload_sound.of(active.0)), PlaybackSettings::DESPAWN));
    }

    let Ok((camera_transform, mut recoil)) = camera.single_mut() else {
        return;
    };

    let kick = &active.def().recoil;
    recoil.climb_target = (recoil.climb_target + kick.view.climb_per_shot).min(kick.view.climb_max);

    let jitter = ops::sin(time.elapsed_secs() * 977.0);
    recoil.wander_target += jitter * kick.view.wander_per_shot;
    recoil.shove_target = (recoil.shove_target + kick.view.shove_per_shot).min(kick.view.shove_max);

    let stance = vfx_assets.stance.single().copied().unwrap_or_default();

    let spread = current_bullet_spread(
        recoil.climb,
        kick.view.climb_max,
        is_aiming(cursor, &aim_held, touch_aiming.0),
        local_moving.0,
        stance,
        local_sprinting.0,
    );
    // Out of the barrel, not out of the eye. The two part company by every
    // degree of sway and recoil the weapon has, so firing down the view made
    // the sights a decoration: the shot went where the player looked whatever
    // the weapon was doing, and no mark on the glass could have told the
    // truth about it.
    let muzzle_in_cam = gun_transform.mul_transform(gun.muzzle_local);
    let muzzle_world_pos =
        camera_transform.translation + camera_transform.rotation * muzzle_in_cam.translation;
    let bore = camera_transform.rotation * muzzle_in_cam.rotation;
    let bore_axis = bore * Vec3::NEG_Z;

    let spread_x = ops::sin(time.elapsed_secs() * 613.0) * spread;
    let spread_y = ops::sin(time.elapsed_secs() * 811.0) * spread;
    // Scattered about the bore's own axes, so spread stays symmetric about
    // the barrel however the weapon is canted.
    let spread_dir = bore_axis + bore * Vec3::X * spread_x + bore * Vec3::Y * spread_y;
    let direction = Dir3::new(spread_dir).unwrap_or(Dir3::new(bore_axis).unwrap_or(Dir3::NEG_Z));

    commands.spawn((
        Mesh3d(vfx_assets.muzzle_fx.flash_mesh.clone()),
        MeshMaterial3d(flash_materials.add(muzzle_flash_material(time.elapsed_secs() * 419.0))),
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

    if muzzle_smoke.0 {
        commands.spawn((
            Mesh3d(vfx_assets.muzzle_fx.smoke_mesh.clone()),
            MeshMaterial3d(smoke_materials.add(muzzle_smoke_material(
                time.elapsed_secs() * 173.0,
                vfx_assets.muzzle_fx.smoke_atlas.clone(),
            ))),
            Transform::from_translation(muzzle_world_pos)
                .with_rotation(camera_transform.rotation)
                .with_scale(Vec3::splat(SMOKE_START_SCALE)),
            Smoke {
                velocity: *camera_transform.forward() * 0.4 + Vec3::Y * 0.5,
                remaining: SMOKE_LIFETIME,
            },
        ));
    }

    // Down the barrel, not down the view: recoil and sway turn the weapon
    // away from where the camera points, and what leaves the muzzle leaves
    // along the muzzle.
    let forward = bore_axis;
    let right = *camera_transform.right();
    let up = *camera_transform.up();

    for i in 0..MUZZLE_EMBER_COUNT {
        let seed = time.elapsed_secs() * 617.0 + i as f32 * 143.9;
        let spread = (right * ops::sin(seed * 1.7) + up * ops::sin(seed * 2.9)) * MUZZLE_EMBER_CONE;
        let dir = (forward + spread).normalize_or_zero();
        let speed = MUZZLE_EMBER_SPEED * (0.7 + ops::sin(seed * 3.1).abs() * 0.3);
        commands.spawn((
            Mesh3d(vfx_assets.muzzle_fx.ember_mesh.clone()),
            MeshMaterial3d(vfx_assets.sparks.material.clone()),
            Transform::from_translation(muzzle_world_pos),
            SparkParticle {
                velocity: dir * speed,
                remaining: MUZZLE_EMBER_LIFETIME,
            },
        ));
    }

    eject_casing(
        &mut commands,
        &casings,
        &mut smoke_materials,
        camera_transform,
        Vec3::ZERO,
        time.elapsed_secs() * 311.0,
        gun_transform.mul_transform(gun.eject_local).translation,
        port_smoke.0,
    );

    let shot_shows_flame_blade =
        ops::sin(time.elapsed_secs() * 733.0).abs() < MUZZLE_BLADE_CHANCE;
    if shot_shows_flame_blade {
        let half_length = FLASH_QUAD_SIZE * MUZZLE_BLADE_LENGTH * 0.5;
        commands.spawn((
            Mesh3d(vfx_assets.muzzle_fx.flash_mesh.clone()),
            MeshMaterial3d(flash_materials.add(muzzle_blade_material(time.elapsed_secs() * 283.0))),
            Transform::from_translation(muzzle_world_pos + forward * half_length)
                .with_scale(Vec3::new(MUZZLE_BLADE_WIDTH, MUZZLE_BLADE_LENGTH, 1.0)),
            MuzzleBlade { bore_axis: forward },
            MuzzleFlash {
                remaining: MUZZLE_FLASH_LIFETIME,
            },
        ));
    }


    let wall_distance = spatial_query
        .cast_ray(muzzle_world_pos, direction, MAX_SHOT_DISTANCE, true, &SpatialQueryFilter::default())
        .map(|hit| hit.distance);
    let predicted_hit = zombies
        .iter()
        .filter_map(|(entity, bones)| {
            let bone_positions = resolve_bone_world_positions(bones, |bone| globals.get(bone).ok().map(GlobalTransform::translation));
            zombie_hit_damage_ray(muzzle_world_pos, direction, MAX_SHOT_DISTANCE, &bone_positions)
                .map(|hit| (hit.point.distance(muzzle_world_pos), entity, hit.point, hit.normal))
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

        if autopilot.0 && is_aiming(cursor, &aim_held, touch_aiming.0) {
            *cooldown = active.def().fire_rate * (1.0 + (distance / 10.0).min(4.0));
        }

        vfx_assets.recently_hit.0.insert(entity, RECENTLY_HIT_ZOMBIE_TTL);
    }

    let tracer_end = predicted_hit.map_or(muzzle_world_pos + *direction * MAX_SHOT_DISTANCE, |(_, _, point, _)| point);
    let tracer_start = muzzle_world_pos + Vec3::Y * TRACER_START_Y_OFFSET;
    spawn_bullet_tracer(&mut commands, &vfx_assets.tracer, time.elapsed_secs() * 733.0, tracer_start, tracer_end, show_tracers.0);

    info!("[shot] fired: origin={:.2?} direction={:.2?}", muzzle_world_pos, *direction);
    // The muzzle, so the shot that is scored is the one that was drawn. An eye
    // origin here against a bore origin in the prediction above would have the
    // two disagree by the whole offset between them.
    commands.trigger(FireShot {
        origin: muzzle_world_pos,
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
    active: Res<ActiveWeapon>,
) {
    if game_over.0 || !(keys.just_pressed(KeyCode::KeyR) || touch_reload.0) {
        return;
    }
    let Ok(mut gun) = gun.single_mut() else {
        return;
    };
    if gun.ammo == active.def().magazine || gun.reload_remaining > 0.0 {
        return;
    }
    gun.reload_remaining = active.def().reload_duration;
    commands.spawn((AudioPlayer(reload_sound.of(active.0)), PlaybackSettings::DESPAWN));
}



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

#[derive(Component)]
pub(crate) struct HitMarker {
    pub(crate) remaining: f32,
}

#[derive(Component)]
pub(crate) struct MuzzleFlash {
    pub(crate) remaining: f32,
}

#[derive(Component)]
pub(crate) struct Smoke {
    pub(crate) velocity: Vec3,
    pub(crate) remaining: f32,
}

#[derive(Component)]
pub(crate) struct BulletTracer {
    pub(crate) remaining: f32,
}

#[derive(Component)]
pub(crate) struct BloodParticle {
    pub(crate) velocity: Vec3,
    pub(crate) remaining: f32,
}

#[derive(Component)]
pub(crate) struct SparkParticle {
    pub(crate) velocity: Vec3,
    pub(crate) remaining: f32,
}

#[derive(Component)]
pub(crate) struct BloodStain {
    pub(crate) remaining: f32,
}

#[derive(Resource)]
pub(crate) struct HitMarkerAssets {
    pub(crate) mesh: Handle<Mesh>,
    pub(crate) material: Handle<StandardMaterial>,
}

#[derive(Resource)]
pub(crate) struct MuzzleFxAssets {
    pub(crate) flash_mesh: Handle<Mesh>,
    pub(crate) flash_material: Handle<StandardMaterial>,
    pub(crate) smoke_mesh: Handle<Mesh>,
    pub(crate) smoke_material: Handle<StandardMaterial>,
}

#[derive(Resource)]
pub(crate) struct TracerAssets {
    pub(crate) mesh: Handle<Mesh>,
    pub(crate) material: Handle<StandardMaterial>,
}

#[derive(Resource)]
pub(crate) struct BloodAssets {
    pub(crate) particle_mesh: Handle<Mesh>,
    pub(crate) particle_material: Handle<StandardMaterial>,
    pub(crate) stain_mesh: Handle<Mesh>,
    pub(crate) stain_material: Handle<StandardMaterial>,
}

#[derive(Resource)]
pub(crate) struct SparkAssets {
    pub(crate) mesh: Handle<Mesh>,
    pub(crate) material: Handle<StandardMaterial>,
}

#[derive(Resource)]
pub(crate) struct GunshotSound(pub(crate) Handle<AudioSource>);

#[derive(Resource)]
pub(crate) struct ReloadSound(pub(crate) Handle<AudioSource>);

#[derive(SystemParam)]
pub(crate) struct VfxAssets<'w, 's> {
    pub(crate) hit_assets: Res<'w, HitMarkerAssets>,
    pub(crate) muzzle_fx: Res<'w, MuzzleFxAssets>,
    pub(crate) tracer: Res<'w, TracerAssets>,
    pub(crate) blood: Res<'w, BloodAssets>,
    pub(crate) sparks: Res<'w, SparkAssets>,
    pub(crate) gunshot: Res<'w, GunshotSound>,
    pub(crate) reload_sound: Res<'w, ReloadSound>,

    pub(crate) stance: Query<'w, 's, &'static PlayerStance, With<Player>>,

    pub(crate) recently_hit: ResMut<'w, RecentlyHitZombies>,
}

pub(crate) fn on_shot_confirmed(
    trigger: On<ShotConfirmed>,
    mut commands: Commands,
    time: Res<Time>,
    vfx_assets: VfxAssets,
    spatial_query: SpatialQuery,
    zombies: Query<(Entity, &Transform, &ZombieHitboxBones), With<Zombie>>,
    globals: Query<&GlobalTransform>,
    show_floating_damage: Res<ShowFloatingDamage>,
) {
    let ShotConfirmed { point, normal, damage, killed } = *trigger.event();
    if damage > 0.0 {
        info!("[shot] server confirmed: inner bone capsule hit, damage={damage:.1} killed={killed}");
    } else {
        info!("[shot] server confirmed: outer capsule hit but missed every inner bone capsule (0 damage)");
    }

    let nearest_bone = if killed {
        None
    } else {
        zombies
            .iter()
            .min_by(|(_, a, _), (_, b, _)| a.translation.distance_squared(point).total_cmp(&b.translation.distance_squared(point)))
            .and_then(|(_, _, bones)| nearest_zombie_bone(bones, point, &globals))
    };
    let bone_for_blood = nearest_bone.as_ref().map(|(entity, global)| (*entity, global));
    spawn_zombie_hit_feedback(&mut commands, &vfx_assets.blood, &vfx_assets.sparks, &spatial_query, &time, point, normal, damage, bone_for_blood, killed, show_floating_damage.0);
}

pub(crate) fn spawn_blood_effects(
    commands: &mut Commands,
    blood_assets: &BloodAssets,
    spatial_query: &SpatialQuery,
    time: &Time,
    hit_point: Vec3,
    hit_normal: Vec3,
    bone: Option<(Entity, &GlobalTransform)>,
    killed: bool,
) {
    for i in 0..BLOOD_PARTICLE_COUNT {

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

    match bone {
        Some((entity, bone_global)) => {
            spawn_blood_stain_on_zombie(commands, blood_assets, entity, bone_global, hit_point, hit_normal);
        }
        None if !killed => spawn_blood_stain(commands, blood_assets, None, hit_point, hit_normal),
        None => {}
    }

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

            spawn_blood_stain(commands, blood_assets, None, floor_point, floor_hit.normal);
        }
    }
}

pub(crate) fn spawn_zombie_hit_feedback(
    commands: &mut Commands,
    blood_assets: &BloodAssets,
    spark_assets: &SparkAssets,
    spatial_query: &SpatialQuery,
    time: &Time,
    point: Vec3,
    normal: Vec3,
    damage: f32,
    bone_for_blood: Option<(Entity, &GlobalTransform)>,
    killed: bool,
    show_floating_damage: bool,
) {
    if damage > 0.0 {
        if show_floating_damage {
            spawn_floating_damage(commands, point, damage);
        }
        spawn_blood_effects(commands, blood_assets, spatial_query, time, point, normal, bone_for_blood, killed);
    } else {
        spawn_spark_effects(commands, spark_assets, time, point, normal);
    }
}

pub(crate) fn spawn_spark_effects(commands: &mut Commands, spark_assets: &SparkAssets, time: &Time, hit_point: Vec3, hit_normal: Vec3) {
    let normal = hit_normal.normalize_or_zero();
    let range = (SPARK_PARTICLE_MAX - SPARK_PARTICLE_MIN + 1) as f32;
    let count = SPARK_PARTICLE_MIN + (ops::sin(time.elapsed_secs() * 349.0).abs() * range) as usize;
    let count = count.clamp(SPARK_PARTICLE_MIN, SPARK_PARTICLE_MAX);

    for i in 0..count {
        let seed = time.elapsed_secs() * 883.0 + i as f32 * 157.1;
        let random_dir =
            Vec3::new(ops::sin(seed * 1.3), ops::sin(seed * 0.7), ops::sin(seed * 2.1)).normalize_or_zero();
        let dir = (normal + random_dir).normalize_or_zero();

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

pub(crate) fn spawn_bullet_tracer(commands: &mut Commands, tracer_assets: &TracerAssets, seed: f32, start: Vec3, end: Vec3, show_tracers: bool) {
    if !show_tracers {
        return;
    }

    if (ops::sin(seed) + 1.0) * 0.5 >= TRACER_CHANCE {
        return;
    }
    let delta = end - start;
    let length = delta.length();
    if length < f32::EPSILON {
        return;
    }
    let rotation = Quat::from_rotation_arc(Vec3::Y, delta / length);
    commands.spawn((
        Mesh3d(tracer_assets.mesh.clone()),
        MeshMaterial3d(tracer_assets.material.clone()),
        Transform::from_translation(start + delta * 0.5)
            .with_rotation(rotation)
            .with_scale(Vec3::new(TRACER_RADIUS, length, TRACER_RADIUS)),
        BulletTracer {
            remaining: TRACER_LIFETIME,
        },
    ));
}

pub(crate) fn spawn_blood_stain(
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

pub(crate) fn spawn_blood_stain_on_zombie(
    commands: &mut Commands,
    blood_assets: &BloodAssets,
    bone: Entity,
    bone_global: &GlobalTransform,
    hit_point: Vec3,
    hit_normal: Vec3,
) {
    let rotation = Quat::from_rotation_arc(Vec3::Z, hit_normal.normalize_or_zero());
    let world_transform =
        GlobalTransform::from(Transform::from_translation(hit_point + hit_normal.normalize_or_zero() * 0.01).with_rotation(rotation));
    let local_transform = world_transform.reparented_to(bone_global);

    commands.entity(bone).with_children(|parent| {
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

pub(crate) fn update_blood_particles(
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

pub(crate) fn update_spark_particles(
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

pub(crate) fn despawn_blood_stains(mut commands: Commands, time: Res<Time>, mut query: Query<(Entity, &mut BloodStain)>) {
    for (entity, mut stain) in &mut query {
        stain.remaining -= time.delta_secs();
        if stain.remaining <= 0.0 {
            commands.entity(entity).try_despawn();
        }
    }
}

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

pub(crate) fn set_hitbox_gizmo_depth_bias(mut config_store: ResMut<GizmoConfigStore>) {

    let (config, _) = config_store.config_mut::<DefaultGizmoConfigGroup>();
    config.depth_bias = -1.0;
}

fn draw_bone_capsule(gizmos: &mut Gizmos, a: Vec3, b: Vec3, radius: f32, color: Color) {
    let segment = b - a;
    let Ok(direction) = Dir3::new(segment) else {
        return;
    };
    gizmos.primitive_3d(
        &Capsule3d { radius, half_length: segment.length() / 2.0 },
        Isometry3d::new(a.midpoint(b), Quat::from_rotation_arc(Vec3::Y, *direction)),
        color,
    );
}

pub(crate) fn draw_zombie_hitboxes(
    show: Res<ShowHitboxes>,
    zombies: Query<(Entity, &Transform, &ZombieHitboxBones), With<Zombie>>,
    globals: Query<&GlobalTransform>,
    mut gizmos: Gizmos,
    autopilot_state: Res<AutopilotState>,
    show_aim: Res<ShowAutopilotTarget>,
) {
    if !show.0 {
        return;
    }

    for (zombie_entity, transform, bones) in &zombies {
        let is_autopilot_target = autopilot_state.target == Some(zombie_entity);

        gizmos.primitive_3d(
            &Cylinder { radius: ZOMBIE_OUTER_RADIUS, half_height: ZOMBIE_OUTER_HEIGHT / 2.0 },
            Isometry3d::from_translation(transform.translation),
            Color::WHITE,
        );

        let mut any_bone = false;
        let bone_pos = |e: Entity| globals.get(e).ok().map(GlobalTransform::translation);
        let mut joints_to_draw = Vec::new();

        if let Some(pos) = bones.head.and_then(bone_pos) {
            gizmos.sphere(Isometry3d::from_translation(pos), HEAD_HITBOX_RADIUS, Color::srgb(1.0, 0.2, 0.2));
            joints_to_draw.push(pos);
            any_bone = true;
        }
        if let (Some(belly), Some(chest)) = (bones.belly.and_then(bone_pos), bones.chest.and_then(bone_pos)) {
            draw_bone_capsule(&mut gizmos, belly, chest, TORSO_HITBOX_RADIUS, Color::srgb(1.0, 0.8, 0.0));
            joints_to_draw.push(belly);
            joints_to_draw.push(chest);
            any_bone = true;
        }
        for chain in [&bones.left_arm, &bones.right_arm, &bones.left_leg, &bones.right_leg] {
            for joint_entity in chain.iter() {
                if let Some(pos) = joint_entity.and_then(bone_pos) {
                    joints_to_draw.push(pos);
                }
            }
            for pair in chain.windows(2) {
                if let (Some(a), Some(b)) = (pair[0].and_then(bone_pos), pair[1].and_then(bone_pos)) {
                    draw_bone_capsule(&mut gizmos, a, b, LIMB_HITBOX_RADIUS, Color::srgb(0.2, 0.6, 1.0));
                    any_bone = true;
                }
            }
        }

        let joint_color = if is_autopilot_target && show_aim.0 {
            Color::srgb(0.0, 1.0, 1.0)
        } else {
            Color::srgb(0.0, 1.0, 0.0)
        };
        for joint_pos in &joints_to_draw {
            gizmos.sphere(Isometry3d::from_translation(*joint_pos), 0.04, joint_color);
        }

        if is_autopilot_target && show_aim.0 {
            let resolved_bones = resolve_bone_world_positions(bones, |bone| globals.get(bone).ok().map(|g| g.translation()));
            if let Some((aim_point, _)) = shared::random_aim_point(&resolved_bones, zombie_entity) {
                gizmos.sphere(Isometry3d::from_translation(aim_point), 0.08, Color::srgb(1.0, 0.0, 0.0));
            }
        }

        if any_bone {
            continue;
        }

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

pub(crate) fn distance_rings_toggle_button(mut show: ResMut<ShowDistanceRings>, btn: Query<&Interaction, (With<CheatDistanceRingsButton>, Changed<Interaction>)>) {
    if tapped(&btn) {
        show.0 = !show.0;
    }
}

pub(crate) fn update_distance_rings_button_color(show: Res<ShowDistanceRings>, mut btn: Query<&mut BackgroundColor, With<CheatDistanceRingsButton>>) {
    if !show.is_changed() {
        return;
    }
    let color = if show.0 { Color::srgba(0.3, 1.0, 0.4, 0.5) } else { Color::srgba(1.0, 1.0, 1.0, 0.2) };
    for mut bg in &mut btn {
        *bg = BackgroundColor(color);
    }
}

pub(crate) fn draw_distance_rings(show: Res<ShowDistanceRings>, bands: Res<LodBandDistance>, player: Query<&Transform, With<Player>>, mut gizmos: Gizmos) {
    if !show.0 {
        return;
    }
    let Ok(player_transform) = player.single() else {
        return;
    };
    let center = player_transform.translation;
    let ring_rotation = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
    let max_radius = bands.0.iter().copied().fold(0.0, f32::max).max(1.0);
    for &radius in &bands.0 {
        let t = radius / max_radius;
        let color = Color::srgb(t, 1.0 - t, 0.0);
        gizmos.circle(Isometry3d::new(center, ring_rotation), radius, color);
    }
}

pub(crate) fn nearest_zombie_bone(bones: &ZombieHitboxBones, point: Vec3, globals: &Query<&GlobalTransform>) -> Option<(Entity, GlobalTransform)> {
    bones
        .head
        .into_iter()
        .chain(bones.belly)
        .chain(bones.chest)
        .chain(bones.left_arm.into_iter().flatten())
        .chain(bones.right_arm.into_iter().flatten())
        .chain(bones.left_leg.into_iter().flatten())
        .chain(bones.right_leg.into_iter().flatten())
        .filter_map(|entity| globals.get(entity).ok().map(|g| (entity, *g)))
        .min_by(|(_, a), (_, b)| a.translation().distance_squared(point).total_cmp(&b.translation().distance_squared(point)))
}

pub(crate) fn on_shot_fx(
    trigger: On<ShotFx>,
    mut commands: Commands,
    time: Res<Time>,
    vfx_assets: VfxAssets,
    spatial_query: SpatialQuery,
    zombies: Query<(Entity, &Transform, &ZombieHitboxBones), With<Zombie>>,
    globals: Query<&GlobalTransform>,
    show_hit_markers: Res<ShowHitMarkers>,
    show_tracers: Res<ShowBulletTracers>,
    show_floating_damage: Res<ShowFloatingDamage>,
) {
    match *trigger.event() {
        ShotFx::ZombieHit { origin, point, normal, damage, killed } => {
            if damage > 0.0 {
                info!("[shot] (remote player) inner bone capsule hit: damage={damage:.1} killed={killed}");
            } else {
                info!("[shot] (remote player) outer capsule hit but missed every inner bone capsule (0 damage)");
            }
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

            spawn_bullet_tracer(&mut commands, &vfx_assets.tracer, time.elapsed_secs() * 619.0 + point.x * 13.7 + point.z * 7.3, origin, point, show_tracers.0);

            let nearest_bone = if killed {
                None
            } else {
                zombies
                    .iter()
                    .min_by(|(_, a, _), (_, b, _)| {
                        a.translation.distance_squared(point).total_cmp(&b.translation.distance_squared(point))
                    })
                    .and_then(|(_, _, bones)| nearest_zombie_bone(bones, point, &globals))
            };
            let bone_for_blood = nearest_bone.as_ref().map(|(entity, global)| (*entity, global));
            spawn_zombie_hit_feedback(&mut commands, &vfx_assets.blood, &vfx_assets.sparks, &spatial_query, &time, point, normal, damage, bone_for_blood, killed, show_floating_damage.0);
        }
        ShotFx::SurfaceHit { origin, point, normal } => {
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
            spawn_bullet_tracer(&mut commands, &vfx_assets.tracer, time.elapsed_secs() * 619.0 + point.x * 13.7 + point.z * 7.3, origin, point, show_tracers.0);
            spawn_spark_effects(&mut commands, &vfx_assets.sparks, &time, point, normal);
        }
    }
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

pub(crate) fn despawn_hit_markers(
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

pub(crate) fn despawn_muzzle_flashes(
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

pub(crate) fn despawn_bullet_tracers(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut BulletTracer)>,
) {
    for (entity, mut tracer) in &mut query {
        tracer.remaining -= time.delta_secs();
        if tracer.remaining <= 0.0 {
            commands.entity(entity).try_despawn();
        }
    }
}

pub(crate) fn update_smoke(mut commands: Commands, time: Res<Time>, mut query: Query<(Entity, &mut Transform, &mut Smoke)>) {
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

pub(crate) fn on_player_fired_fx(
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

        player.play(anim.shoot);
    }
}

pub(crate) fn on_player_died_fx(
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

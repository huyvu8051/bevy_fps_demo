
use bevy::prelude::*;
use crate::*;






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
pub(crate) struct SparkParticle {
    pub(crate) velocity: Vec3,
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
    pub(crate) ember_mesh: Handle<Mesh>,
    pub(crate) smoke_mesh: Handle<Mesh>,
}

#[derive(Resource)]
pub(crate) struct TracerAssets {
    pub(crate) mesh: Handle<Mesh>,
    pub(crate) material: Handle<StandardMaterial>,
}


#[derive(Resource)]
pub(crate) struct SparkAssets {
    pub(crate) mesh: Handle<Mesh>,
    pub(crate) material: Handle<StandardMaterial>,
}

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
    child_of: Query<&ChildOf>,
    show_floating_damage: Res<ShowFloatingDamage>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let ShotConfirmed { origin, point, normal, damage, killed } = *trigger.event();
    if damage > 0.0 {
        info!("[shot] server confirmed: inner bone capsule hit, damage={damage:.1} killed={killed}");
    } else {
        info!("[shot] server confirmed: outer capsule hit but missed every inner bone capsule (0 damage)");
    }

    let hit_zombie = if killed {
        None
    } else {
        zombies
            .iter()
            .min_by(|(_, a, _), (_, b, _)| a.translation.distance_squared(point).total_cmp(&b.translation.distance_squared(point)))
    };
    let nearest_bone = hit_zombie.and_then(|(_, _, bones)| nearest_zombie_bone(bones, point, &globals));

    if let (Some((_, _, bones)), Some((bone, _))) = (hit_zombie, nearest_bone.as_ref()) {
        spawn_bone_flinch(&mut commands, bones, *bone, -normal, damage, &globals, &child_of);
    }

    let bone_for_blood = nearest_bone.as_ref().map(|(entity, global)| (*entity, global));
    spawn_zombie_hit_feedback(&mut commands, &vfx_assets.blood, &vfx_assets.sparks, &spatial_query, &time, point, normal, damage, bone_for_blood, killed, show_floating_damage.0, &mut materials, (point - origin).normalize_or_zero());
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
    mut materials: ResMut<Assets<StandardMaterial>>,
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
            spawn_zombie_hit_feedback(&mut commands, &vfx_assets.blood, &vfx_assets.sparks, &spatial_query, &time, point, normal, damage, bone_for_blood, killed, show_floating_damage.0, &mut materials, (point - origin).normalize_or_zero());
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
        ShotFx::Penetrated { point, normal } => {
            spawn_spark_effects(&mut commands, &vfx_assets.sparks, &time, point, normal);
        }
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
        smoke.velocity *= ops::exp(-SMOKE_DRAG * dt);
        smoke.velocity.y += SMOKE_BUOYANCY * dt;
        transform.translation += smoke.velocity * dt;
        transform.scale *= 1.0 + SMOKE_GROWTH_PER_SEC * dt;

        smoke.remaining -= dt;
        if smoke.remaining <= 0.0 {
            commands.entity(entity).try_despawn();
        }
    }

    let live = query.iter().count();
    if live > SMOKE_MAX_PUFFS {
        let mut by_age: Vec<(Entity, f32)> = query.iter().map(|(entity, _, smoke)| (entity, smoke.remaining)).collect();
        by_age.sort_by(|a, b| a.1.total_cmp(&b.1));
        for (entity, _) in by_age.into_iter().take(live - SMOKE_MAX_PUFFS) {
            commands.entity(entity).try_despawn();
        }
    }
}



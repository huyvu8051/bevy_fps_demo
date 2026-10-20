use bevy::prelude::*;

use crate::*;

#[derive(Component)]
pub(crate) struct BloodParticle {
    pub(crate) velocity: Vec3,
    pub(crate) remaining: f32,
}

#[derive(Component)]
pub(crate) struct BloodStain {
    pub(crate) remaining: f32,
}

#[derive(Resource)]
pub(crate) struct BloodAssets {
    pub(crate) particle_mesh: Handle<Mesh>,
    pub(crate) particle_material: Handle<StandardMaterial>,
    pub(crate) stain_mesh: Handle<Mesh>,
    pub(crate) floor_stain_mesh: Handle<Mesh>,
    pub(crate) stain_materials: Vec<Handle<StandardMaterial>>,
    pub(crate) wall_stain_mesh: Handle<Mesh>,
    pub(crate) burst_mesh: Handle<Mesh>,
    pub(crate) burst_materials: Vec<Handle<StandardMaterial>>,
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
    materials: &mut Assets<StandardMaterial>,
    shot_direction: Vec3,
) {
    spawn_blood_burst(commands, blood_assets, materials, hit_point, hit_normal, time.elapsed_secs() * 311.0);
    spawn_exit_wound(commands, blood_assets, materials, spatial_query, hit_point, shot_direction, time.elapsed_secs() * 541.0);

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
            spawn_blood_stain_on_zombie(commands, blood_assets, entity, bone_global, hit_point, hit_normal, time.elapsed_secs() * 733.0);
        }
        None if !killed => spawn_blood_stain(commands, blood_assets, None, hit_point, hit_normal, time.elapsed_secs() * 733.0, blood_assets.stain_mesh.clone()),
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

            spawn_blood_stain(commands, blood_assets, None, floor_point, floor_hit.normal, seed, blood_assets.floor_stain_mesh.clone());
        }
    }
}

fn stain_variant(blood_assets: &BloodAssets, seed: f32) -> Option<Handle<StandardMaterial>> {
    let materials = &blood_assets.stain_materials;
    if materials.is_empty() {
        return None;
    }
    let pick = (ops::sin(seed) * 43758.547).fract().abs();
    materials.get((pick * materials.len() as f32) as usize % materials.len()).cloned()
}

fn stain_rotation(normal: Vec3, seed: f32) -> Quat {
    let spin = (ops::sin(seed * 1.7) * 43758.547).fract().abs() * std::f32::consts::TAU;
    Quat::from_rotation_arc(Vec3::Z, normal.normalize_or_zero()) * Quat::from_rotation_z(spin)
}

pub(crate) fn spawn_blood_stain(
    commands: &mut Commands,
    blood_assets: &BloodAssets,
    parent: Option<Entity>,
    point: Vec3,
    normal: Vec3,
    seed: f32,
    mesh: Handle<Mesh>,
) {
    let Some(material) = stain_variant(blood_assets, seed) else {
        return;
    };
    let transform = Transform::from_translation(point + normal.normalize_or_zero() * 0.01)
        .with_rotation(stain_rotation(normal, seed));
    let mut stain = commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(material),
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
    seed: f32,
) {
    let Some(material) = stain_variant(blood_assets, seed) else {
        return;
    };
    let world_transform = GlobalTransform::from(
        Transform::from_translation(hit_point + hit_normal.normalize_or_zero() * 0.01)
            .with_rotation(stain_rotation(hit_normal, seed)),
    );
    let local_transform = world_transform.reparented_to(bone_global);

    commands.entity(bone).with_children(|parent| {
        parent.spawn((
            Mesh3d(blood_assets.stain_mesh.clone()),
            MeshMaterial3d(material),
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
    camera: Query<&GlobalTransform, With<FpsCamera>>,
    mut query: Query<(Entity, &mut Transform, &mut BloodParticle)>,
) {
    let dt = time.delta_secs();
    let eye = camera.single().ok().map(|global| global.translation());
    // A severance bleeds from both faces of the cut for over a second, so a
    // few at once puts hundreds of these up. Oldest go first, as elsewhere.
    let mut over_budget = query.iter().len().saturating_sub(MAX_BLOOD_PARTICLES);
    for (entity, mut transform, mut particle) in &mut query {
        if over_budget > 0 && particle.remaining > 0.1 {
            particle.remaining = 0.1;
            over_budget -= 1;
        }
        particle.velocity.y += BLOOD_PARTICLE_GRAVITY * dt;
        transform.translation += particle.velocity * dt;

        if let Some(eye) = eye {
            let to_eye = (eye - transform.translation).normalize_or_zero();
            if to_eye != Vec3::ZERO {
                transform.rotation = Quat::from_rotation_arc(Vec3::Z, to_eye);
            }
        }

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


#[derive(Component)]
pub(crate) struct BloodBurst {
    remaining: f32,
    spin: f32,
    material: Handle<StandardMaterial>,
}

fn spawn_blood_burst(
    commands: &mut Commands,
    blood_assets: &BloodAssets,
    materials: &mut Assets<StandardMaterial>,
    hit_point: Vec3,
    hit_normal: Vec3,
    seed: f32,
) {
    let variants = &blood_assets.burst_materials;
    if variants.is_empty() {
        return;
    }
    let index = ((ops::sin(seed).abs() * variants.len() as f32) as usize).min(variants.len() - 1);
    let Some(source) = materials.get(&variants[index]).cloned() else {
        return;
    };
    let material = materials.add(source);
    commands.spawn((
        Mesh3d(blood_assets.burst_mesh.clone()),
        MeshMaterial3d(material.clone()),
        Transform::from_translation(hit_point + hit_normal.normalize_or_zero() * BLOOD_BURST_NORMAL_OFFSET)
            .with_scale(Vec3::splat(BLOOD_BURST_START_SCALE)),
        BloodBurst { remaining: BLOOD_BURST_LIFETIME, spin: seed, material },
    ));
}

pub(crate) fn update_blood_bursts(
    mut commands: Commands,
    time: Res<Time>,
    camera: Query<&GlobalTransform, With<FpsCamera>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut query: Query<(Entity, &mut Transform, &mut BloodBurst)>,
) {
    let dt = time.delta_secs();
    let eye = camera.single().ok().map(|global| global.translation());
    for (entity, mut transform, mut burst) in &mut query {
        burst.remaining -= dt;
        if burst.remaining <= 0.0 {
            commands.entity(entity).try_despawn();
            continue;
        }
        let life = 1.0 - (burst.remaining / BLOOD_BURST_LIFETIME).clamp(0.0, 1.0);
        let eased_out = 1.0 - (1.0 - life) * (1.0 - life);
        transform.scale = Vec3::splat(BLOOD_BURST_START_SCALE + (BLOOD_BURST_END_SCALE - BLOOD_BURST_START_SCALE) * eased_out);

        if let Some(eye) = eye {
            let to_eye = (eye - transform.translation).normalize_or_zero();
            if to_eye != Vec3::ZERO {
                transform.rotation = Quat::from_rotation_arc(Vec3::Z, to_eye) * Quat::from_rotation_z(burst.spin);
            }
        }
        if let Some(mut material) = materials.get_mut(&burst.material) {
            material.base_color.set_alpha(1.0 - life);
        }
    }
}

fn spawn_exit_wound(
    commands: &mut Commands,
    blood_assets: &BloodAssets,
    materials: &mut Assets<StandardMaterial>,
    spatial_query: &SpatialQuery,
    hit_point: Vec3,
    shot_direction: Vec3,
    seed_base: f32,
) {
    let Ok(forward) = Dir3::new(shot_direction) else {
        return;
    };
    let lateral = forward.any_orthonormal_vector();
    let other = forward.cross(lateral);

    for i in 0..BLOOD_EXIT_PARTICLE_COUNT {
        let seed = seed_base + i as f32 * 97.3;
        let spread = (lateral * ops::sin(seed * 1.9) + other * ops::sin(seed * 2.7)) * BLOOD_EXIT_CONE;
        let dir = (*forward + spread).normalize_or_zero();
        let speed = BLOOD_EXIT_SPEED * (0.65 + ops::sin(seed * 3.3).abs() * 0.35);
        commands.spawn((
            Mesh3d(blood_assets.particle_mesh.clone()),
            MeshMaterial3d(blood_assets.particle_material.clone()),
            Transform::from_translation(hit_point),
            BloodParticle {
                velocity: dir * speed,
                remaining: BLOOD_PARTICLE_LIFETIME,
            },
        ));
    }

    let exit_point = hit_point + *forward * BLOOD_EXIT_WOUND_OFFSET;
    spawn_blood_burst(commands, blood_assets, materials, exit_point, *forward, seed_base * 1.7);

    let filter = SpatialQueryFilter::default();
    for i in 0..BLOOD_WALL_STAIN_COUNT {
        let seed = seed_base + i as f32 * 263.1;
        let jitter = (lateral * ops::sin(seed * 1.3) + other * ops::sin(seed * 2.1)) * BLOOD_WALL_STAIN_SPREAD;
        let Ok(ray) = Dir3::new(*forward + jitter) else {
            continue;
        };
        let Some(wall) = spatial_query.cast_ray(exit_point, ray, BLOOD_WALL_RAYCAST_DISTANCE, true, &filter) else {
            continue;
        };
        let wall_point = exit_point + *ray * wall.distance;
        spawn_blood_stain(commands, blood_assets, None, wall_point, wall.normal, seed, blood_assets.wall_stain_mesh.clone());
    }
}

/// A round droplet with a soft rim, generated rather than shipped: the
/// particle quad was drawn with flat colour and no alpha, so every droplet
/// was a hard red square.
pub(crate) fn droplet_image(size: u32) -> Image {
    let centre = (size as f32 - 1.0) * 0.5;
    let mut data = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let dx = (x as f32 - centre) / centre;
            let dy = (y as f32 - centre) / centre;
            let edge = (1.0 - (dx * dx + dy * dy).sqrt()).clamp(0.0, 1.0);
            // cubed keeps a solid core and spends the falloff near the rim
            let alpha = edge * edge * edge;
            data.extend_from_slice(&[255, 255, 255, (alpha * 255.0) as u8]);
        }
    }
    Image::new(
        Extent3d { width: size, height: size, depth_or_array_layers: 1 },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

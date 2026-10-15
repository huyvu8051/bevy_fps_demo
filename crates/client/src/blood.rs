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

pub(crate) fn despawn_blood_stains(mut commands: Commands, time: Res<Time>, mut query: Query<(Entity, &mut BloodStain)>) {
    for (entity, mut stain) in &mut query {
        stain.remaining -= time.delta_secs();
        if stain.remaining <= 0.0 {
            commands.entity(entity).try_despawn();
        }
    }
}


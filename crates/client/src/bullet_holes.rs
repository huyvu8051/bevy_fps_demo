use bevy::prelude::*;

use crate::*;

const HOLE_VARIANTS: usize = 4;
const HOLE_SIZE: f32 = 0.14;
const HOLE_LIFETIME: f32 = 45.0;
const HOLE_FADE_SECONDS: f32 = 4.0;
const HOLE_SURFACE_OFFSET: f32 = 0.006;
const MAX_HOLES: usize = 64;

const DUST_COUNT: usize = 7;
const DUST_SPEED: f32 = 1.4;
const DUST_CONE: f32 = 0.55;
const DUST_SIZE: f32 = 0.028;
const DUST_LIFETIME: f32 = 0.5;
const DUST_GRAVITY: f32 = -2.2;

#[derive(Resource)]
pub(crate) struct BulletHoleAssets {
    hole_mesh: Handle<Mesh>,
    hole_materials: Vec<Handle<StandardMaterial>>,
    dust_mesh: Handle<Mesh>,
    dust_material: Handle<StandardMaterial>,
}

#[derive(Component)]
pub(crate) struct BulletHole {
    remaining: f32,
}

#[derive(Component)]
pub(crate) struct ImpactDust {
    velocity: Vec3,
    remaining: f32,
}

pub(crate) fn build_bullet_hole_assets(
    asset_server: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) -> BulletHoleAssets {
    BulletHoleAssets {
        hole_mesh: meshes.add(Rectangle::new(HOLE_SIZE, HOLE_SIZE)),
        hole_materials: (0..HOLE_VARIANTS)
            .map(|i| {
                materials.add(StandardMaterial {
                    base_color_texture: Some(
                        asset_server.load(format!("textures/impact/hole_{i}.png")),
                    ),
                    alpha_mode: AlphaMode::Blend,
                    perceptual_roughness: 0.95,
                    metallic: 0.0,
                    // the decal sits a few millimetres off the wall it belongs
                    // to, which is not enough to beat depth precision at range
                    depth_bias: 10.0,
                    cull_mode: None,
                    ..default()
                })
            })
            .collect(),
        dust_mesh: meshes.add(Rectangle::new(DUST_SIZE, DUST_SIZE)),
        dust_material: materials.add(StandardMaterial {
            base_color: Color::srgba(0.72, 0.69, 0.63, 0.5),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            cull_mode: None,
            ..default()
        }),
    }
}

fn facing(normal: Vec3, seed: f32) -> Quat {
    let spin = (ops::sin(seed * 1.7) * 43758.547).fract().abs() * std::f32::consts::TAU;
    Quat::from_rotation_arc(Vec3::Z, normal.normalize_or_zero()) * Quat::from_rotation_z(spin)
}

pub(crate) fn spawn_bullet_hole(
    commands: &mut Commands,
    assets: &BulletHoleAssets,
    materials: &mut Assets<StandardMaterial>,
    point: Vec3,
    normal: Vec3,
    seed: f32,
    enabled: bool,
) {
    if !enabled {
        return;
    }
    let normal = normal.normalize_or_zero();
    if normal == Vec3::ZERO {
        return;
    }
    let variant = (ops::sin(seed * 2.9).abs() * HOLE_VARIANTS as f32) as usize;
    let Some(shared) = assets.hole_materials.get(variant.min(HOLE_VARIANTS - 1)) else {
        return;
    };
    // Each hole owns its material so it can fade out alone; sharing one per
    // variant means the first hole to expire dims every other hole like it.
    let Some(own) = materials.get(shared).cloned() else { return };
    let material = materials.add(own);

    commands.spawn((
        Mesh3d(assets.hole_mesh.clone()),
        MeshMaterial3d(material),
        Transform::from_translation(point + normal * HOLE_SURFACE_OFFSET)
            .with_rotation(facing(normal, seed)),
        BulletHole { remaining: HOLE_LIFETIME },
    ));

    let sideways = normal.any_orthonormal_vector();
    let other = normal.cross(sideways);
    for i in 0..DUST_COUNT {
        let scatter = seed + i as f32 * 71.3;
        let spread = (sideways * ops::sin(scatter * 1.9) + other * ops::sin(scatter * 2.7)) * DUST_CONE;
        let direction = (normal + spread).normalize_or_zero();
        let speed = DUST_SPEED * (0.5 + ops::sin(scatter * 3.3).abs() * 0.5);
        commands.spawn((
            Mesh3d(assets.dust_mesh.clone()),
            MeshMaterial3d(assets.dust_material.clone()),
            Transform::from_translation(point + normal * HOLE_SURFACE_OFFSET),
            ImpactDust { velocity: direction * speed, remaining: DUST_LIFETIME },
        ));
    }
}

/// Holes are permanent-looking but not permanent: an unbounded decal spawn is
/// a leak, and at ten rounds a second it is a fast one.
pub(crate) fn update_bullet_holes(
    time: Res<Time>,
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut holes: Query<(Entity, &mut BulletHole, &MeshMaterial3d<StandardMaterial>)>,
) {
    let live = holes.iter().len();
    let mut over_budget = live.saturating_sub(MAX_HOLES);

    for (entity, mut hole, _) in &mut holes {
        if over_budget > 0 && hole.remaining > HOLE_FADE_SECONDS {
            hole.remaining = HOLE_FADE_SECONDS;
            over_budget -= 1;
        }
        hole.remaining -= time.delta_secs();
        if hole.remaining <= 0.0 {
            commands.entity(entity).despawn();
        }
    }

    for (_, hole, handle) in &holes {
        if hole.remaining >= HOLE_FADE_SECONDS {
            continue;
        }
        if let Some(mut material) = materials.get_mut(&handle.0) {
            material.base_color.set_alpha(hole.remaining / HOLE_FADE_SECONDS);
        }
    }
}

pub(crate) fn update_impact_dust(
    time: Res<Time>,
    mut commands: Commands,
    camera: Query<&GlobalTransform, With<FpsCamera>>,
    mut dust: Query<(Entity, &mut Transform, &mut ImpactDust)>,
) {
    let dt = time.delta_secs();
    let eye = camera.single().ok().map(|global| global.translation());
    for (entity, mut transform, mut speck) in &mut dust {
        speck.velocity.y += DUST_GRAVITY * dt;
        transform.translation += speck.velocity * dt;
        if let Some(eye) = eye {
            let to_eye = (eye - transform.translation).normalize_or_zero();
            if to_eye != Vec3::ZERO {
                transform.rotation = Quat::from_rotation_arc(Vec3::Z, to_eye);
            }
        }
        speck.remaining -= dt;
        if speck.remaining <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}

/// Whether a shot leaves a mark on what it hit.
///
/// Each hole is an entity with its own material instance, and they stay until
/// their lifetime runs out, so a long firefight accumulates them. Gated the
/// same way tracers are: the flag goes into the spawn rather than a system
/// deciding not to call it, so there is one place that knows.
#[derive(Resource)]
pub(crate) struct BulletHolesEnabled(pub(crate) bool);

impl Default for BulletHolesEnabled {
    fn default() -> Self {
        Self(true)
    }
}

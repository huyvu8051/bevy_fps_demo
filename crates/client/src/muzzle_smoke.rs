use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;

use crate::*;

const MUZZLE_SMOKE_SHADER: &str = "shaders/muzzle_smoke.wgsl";
const MUZZLE_SMOKE_DENSITY: f32 = 5.5;
const MUZZLE_SMOKE_BILLOW: f32 = 1.1;

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub(crate) struct MuzzleSmokeMaterial {
    /// x = life progress 0..1, y = per-puff seed, z = density, w = billow speed
    #[uniform(0)]
    settings: Vec4,
}

impl Material for MuzzleSmokeMaterial {
    fn fragment_shader() -> ShaderRef {
        MUZZLE_SMOKE_SHADER.into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
}

pub(crate) fn muzzle_smoke_material(seed: f32) -> MuzzleSmokeMaterial {
    MuzzleSmokeMaterial {
        settings: Vec4::new(0.0, seed, MUZZLE_SMOKE_DENSITY, MUZZLE_SMOKE_BILLOW),
    }
}

/// Feeds each puff's own material its life progress, and keeps the quad facing
/// the eye — the raymarch assumes the slab is perpendicular to the view.
pub(crate) fn update_muzzle_smoke(
    camera: Query<&GlobalTransform, With<FpsCamera>>,
    mut materials: ResMut<Assets<MuzzleSmokeMaterial>>,
    mut query: Query<(&mut Transform, &Smoke, &MeshMaterial3d<MuzzleSmokeMaterial>)>,
) {
    let eye = camera.single().ok().map(|global| global.translation());
    for (mut transform, smoke, handle) in &mut query {
        if let Some(mut material) = materials.get_mut(&handle.0) {
            material.settings.x = 1.0 - (smoke.remaining / SMOKE_LIFETIME).clamp(0.0, 1.0);
        }
        if let Some(eye) = eye {
            let to_eye = (eye - transform.translation).normalize_or_zero();
            if to_eye != Vec3::ZERO {
                transform.rotation = Quat::from_rotation_arc(Vec3::Z, to_eye);
            }
        }
    }
}

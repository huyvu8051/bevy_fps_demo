use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;

use crate::*;

const MUZZLE_SMOKE_SHADER: &str = "shaders/muzzle_smoke.wgsl";
const MUZZLE_SMOKE_DENSITY: f32 = 4.7;
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

const MUZZLE_FLASH_SHADER: &str = "shaders/muzzle_flash.wgsl";
const MUZZLE_FLASH_RAY_SCALE: f32 = 2.6;
const MUZZLE_FLASH_CORE_SIZE: f32 = 0.32;

/// The tongue of flame along the bore. Its quad is locked to the barrel axis
/// and only spun about it to face the eye, so it needs the axis kept around.
#[derive(Component)]
pub(crate) struct MuzzleBlade {
    pub(crate) axis: Vec3,
}

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub(crate) struct MuzzleFlashMaterial {
    /// x = life progress 0..1, y = seed, z = ray scale, w = core size
    #[uniform(0)]
    settings: Vec4,
    /// x = 0 radial core / 1 directional blade, y = facing (CPU-side fresnel)
    #[uniform(1)]
    shape: Vec4,
}

impl Material for MuzzleFlashMaterial {
    fn fragment_shader() -> ShaderRef {
        MUZZLE_FLASH_SHADER.into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Add
    }
}

pub(crate) fn muzzle_flash_material(seed: f32) -> MuzzleFlashMaterial {
    MuzzleFlashMaterial {
        settings: Vec4::new(0.0, seed.fract(), MUZZLE_FLASH_RAY_SCALE, MUZZLE_FLASH_CORE_SIZE),
        shape: Vec4::new(0.0, 1.0, 0.0, 0.0),
    }
}

pub(crate) fn muzzle_blade_material(seed: f32) -> MuzzleFlashMaterial {
    MuzzleFlashMaterial {
        settings: Vec4::new(0.0, seed.fract(), MUZZLE_FLASH_RAY_SCALE, MUZZLE_FLASH_CORE_SIZE),
        shape: Vec4::new(1.0, 1.0, 0.0, 0.0),
    }
}

/// Drives each flash through its (very short) life, and orients it: the core
/// quad faces the eye outright, the blade may only spin about the bore axis.
pub(crate) fn update_muzzle_flash(
    camera: Query<&GlobalTransform, With<FpsCamera>>,
    mut materials: ResMut<Assets<MuzzleFlashMaterial>>,
    mut query: Query<(&mut Transform, &MuzzleFlash, &MeshMaterial3d<MuzzleFlashMaterial>, Option<&MuzzleBlade>)>,
) {
    let eye = camera.single().ok().map(|global| global.translation());
    for (mut transform, flash, handle, blade) in &mut query {
        let mut facing = 1.0;
        if let Some(eye) = eye {
            let to_eye = (eye - transform.translation).normalize_or_zero();
            match blade {
                Some(blade) => {
                    // strip the component along the bore: what is left is how much
                    // of the quad we can ever turn toward the eye
                    let normal = to_eye - blade.axis * to_eye.dot(blade.axis);
                    facing = normal.length();
                    if facing > 1e-3 {
                        let z = normal / facing;
                        let y = blade.axis;
                        transform.rotation = Quat::from_mat3(&Mat3::from_cols(y.cross(z), y, z));
                    }
                }
                None if to_eye != Vec3::ZERO => {
                    transform.rotation = Quat::from_rotation_arc(Vec3::Z, to_eye);
                }
                None => {}
            }
        }
        if let Some(mut material) = materials.get_mut(&handle.0) {
            material.settings.x = 1.0 - (flash.remaining / MUZZLE_FLASH_LIFETIME).clamp(0.0, 1.0);
            material.shape.y = facing;
        }
    }
}

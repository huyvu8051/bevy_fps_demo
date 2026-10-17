use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;

use crate::*;

const MUZZLE_SMOKE_SHADER: &str = "shaders/muzzle_smoke.wgsl";
const MUZZLE_SMOKE_DENSITY: f32 = 4.7;
const MUZZLE_SMOKE_BILLOW: f32 = 1.1;

const MUZZLE_FLASH_SHADER: &str = "shaders/muzzle_flash.wgsl";
const MUZZLE_FLASH_RAY_SCALE: f32 = 2.6;
const MUZZLE_FLASH_CORE_SIZE: f32 = 0.32;

const SHAPE_RADIAL_CORE: f32 = 0.0;
const SHAPE_DIRECTIONAL_BLADE: f32 = 1.0;

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub(crate) struct MuzzleSmokeMaterial {
    #[uniform(0)]
    settings: Vec4,
}

impl MuzzleSmokeMaterial {
    fn new(seed: f32) -> Self {
        Self {
            settings: Vec4::new(0.0, seed, MUZZLE_SMOKE_DENSITY, MUZZLE_SMOKE_BILLOW),
        }
    }

    fn set_life_progress(&mut self, progress: f32) {
        self.settings.x = progress;
    }
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
    MuzzleSmokeMaterial::new(seed)
}

#[derive(Component)]
pub(crate) struct MuzzleBlade {
    pub(crate) bore_axis: Vec3,
}

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub(crate) struct MuzzleFlashMaterial {
    #[uniform(0)]
    settings: Vec4,
    #[uniform(1)]
    shape: Vec4,
}

impl MuzzleFlashMaterial {
    fn new(seed: f32, shape_kind: f32) -> Self {
        Self {
            settings: Vec4::new(
                0.0,
                seed.fract(),
                MUZZLE_FLASH_RAY_SCALE,
                MUZZLE_FLASH_CORE_SIZE,
            ),
            shape: Vec4::new(shape_kind, 1.0, 0.0, 0.0),
        }
    }

    fn set_life_progress(&mut self, progress: f32) {
        self.settings.x = progress;
    }

    fn set_how_squarely_it_faces_the_eye(&mut self, facing: f32) {
        self.shape.y = facing;
    }
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
    MuzzleFlashMaterial::new(seed, SHAPE_RADIAL_CORE)
}

pub(crate) fn muzzle_blade_material(seed: f32) -> MuzzleFlashMaterial {
    MuzzleFlashMaterial::new(seed, SHAPE_DIRECTIONAL_BLADE)
}

fn elapsed_fraction_of(remaining: f32, lifetime: f32) -> f32 {
    1.0 - (remaining / lifetime).clamp(0.0, 1.0)
}

fn component_perpendicular_to(vector: Vec3, axis: Vec3) -> Vec3 {
    vector - axis * vector.dot(axis)
}

fn facing_the_eye(position: Vec3, eye: Vec3) -> Option<Vec3> {
    let to_eye = (eye - position).normalize_or_zero();
    (to_eye != Vec3::ZERO).then_some(to_eye)
}

pub(crate) fn update_muzzle_smoke(
    camera: Query<&GlobalTransform, With<FpsCamera>>,
    mut materials: ResMut<Assets<MuzzleSmokeMaterial>>,
    mut query: Query<(&mut Transform, &Smoke, &MeshMaterial3d<MuzzleSmokeMaterial>)>,
) {
    let eye = camera.single().ok().map(|global| global.translation());
    for (mut transform, smoke, handle) in &mut query {
        if let Some(mut material) = materials.get_mut(&handle.0) {
            material.set_life_progress(elapsed_fraction_of(smoke.remaining, SMOKE_LIFETIME));
        }
        let Some(eye) = eye else { continue };
        if let Some(to_eye) = facing_the_eye(transform.translation, eye) {
            transform.rotation = Quat::from_rotation_arc(Vec3::Z, to_eye);
        }
    }
}

pub(crate) fn update_muzzle_flash(
    camera: Query<&GlobalTransform, With<FpsCamera>>,
    mut materials: ResMut<Assets<MuzzleFlashMaterial>>,
    mut query: Query<(
        &mut Transform,
        &MuzzleFlash,
        &MeshMaterial3d<MuzzleFlashMaterial>,
        Option<&MuzzleBlade>,
    )>,
) {
    let eye = camera.single().ok().map(|global| global.translation());
    for (mut transform, flash, handle, blade) in &mut query {
        let mut facing = 1.0;
        if let Some(eye) = eye {
            match (blade, facing_the_eye(transform.translation, eye)) {
                (Some(blade), Some(to_eye)) => {
                    let across_bore = component_perpendicular_to(to_eye, blade.bore_axis);
                    facing = across_bore.length();
                    if facing > 1e-3 {
                        let z = across_bore / facing;
                        let y = blade.bore_axis;
                        transform.rotation = Quat::from_mat3(&Mat3::from_cols(y.cross(z), y, z));
                    }
                }
                (None, Some(to_eye)) => {
                    transform.rotation = Quat::from_rotation_arc(Vec3::Z, to_eye);
                }
                _ => {}
            }
        }
        if let Some(mut material) = materials.get_mut(&handle.0) {
            material.set_life_progress(elapsed_fraction_of(
                flash.remaining,
                MUZZLE_FLASH_LIFETIME,
            ));
            material.set_how_squarely_it_faces_the_eye(facing);
        }
    }
}

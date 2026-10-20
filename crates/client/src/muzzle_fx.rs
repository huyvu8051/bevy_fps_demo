use bevy::prelude::*;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;

use crate::*;

const MUZZLE_SMOKE_SHADER: &str = "shaders/muzzle_smoke.wgsl";
const MUZZLE_SMOKE_DENSITY: f32 = 4.7;
const MUZZLE_SMOKE_BILLOW: f32 = 1.5;

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

    pub(crate) fn set_life_progress(&mut self, progress: f32) {
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

pub(crate) fn elapsed_fraction_of(remaining: f32, lifetime: f32) -> f32 {
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


const RIBBON_COLUMNS: usize = 3;

struct TrailPoint {
    position: Vec3,
    age: f32,
    sway_seed: f32,
}

/// One continuous ribbon rising off a hot barrel, the way a stick of incense
/// smokes. Separate puffs read as lumps, and each puff cost a raymarch; the
/// whole stream here is a single unlit mesh.
#[derive(Resource, Default)]
pub(crate) struct BarrelSmoke {
    since_shot: f32,
    next_point: f32,
    points: Vec<TrailPoint>,
    /// Previous eye pose, so the turn and walk rates come from the camera
    /// itself and cover autopilot as well as the mouse.
    last_eye: Option<(Vec3, Quat)>,
    visible: f32,
}

#[derive(Resource)]
pub(crate) struct BarrelSmokeRibbon {
    pub(crate) mesh: Handle<Mesh>,
}

pub(crate) fn note_shot_for_barrel_smoke(_: On<FireShot>, mut barrel: ResMut<BarrelSmoke>) {
    barrel.since_shot = 0.0;
}

/// Allocated once at full length and never resized. Rewriting a mesh with a
/// different vertex count every frame frees slab entries the render world is
/// still pointing at, which Bevy reports as a use-after-free.
pub(crate) fn empty_ribbon_mesh() -> Mesh {
    let rows = BARREL_TRAIL_MAX_POINTS * RIBBON_COLUMNS;
    let mut indices = Vec::with_capacity((BARREL_TRAIL_MAX_POINTS - 1) * 12);
    for index in 0..BARREL_TRAIL_MAX_POINTS - 1 {
        let here = (index * RIBBON_COLUMNS) as u32;
        let next = ((index + 1) * RIBBON_COLUMNS) as u32;
        // two strips: outer edge to centre on each side, so alpha can fall off
        // across the width instead of ending at a hard cut
        for side in 0..2u32 {
            indices.extend_from_slice(&[
                here + side,
                here + side + 1,
                next + side,
                here + side + 1,
                next + side + 1,
                next + side,
            ]);
        }
    }
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0f32; 3]; rows])
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 0.0, 1.0f32]; rows])
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0f32; 2]; rows])
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, vec![[0.0f32; 4]; rows])
        .with_inserted_indices(Indices::U32(indices))
}

pub(crate) fn update_barrel_smoke(
    time: Res<Time>,
    mut barrel: ResMut<BarrelSmoke>,
    mut meshes: ResMut<Assets<Mesh>>,
    ribbon: Res<BarrelSmokeRibbon>,
    camera: Query<&Transform, With<FpsCamera>>,
    gun: Query<(&Transform, &Gun), Without<FpsCamera>>,
    active: Res<ActiveWeapon>,
) {
    let dt = time.delta_secs();
    barrel.since_shot += dt;
    barrel.next_point -= dt;

    for point in &mut barrel.points {
        point.age += dt;
        let drift = Vec3::new(
            ops::sin(point.sway_seed + point.age * 1.7),
            0.0,
            ops::cos(point.sway_seed * 1.3 + point.age * 1.4),
        );
        point.position += (Vec3::Y * BARREL_TRAIL_RISE + drift * BARREL_TRAIL_SWAY) * dt;
    }
    barrel.points.retain(|point| point.age < BARREL_TRAIL_LIFETIME);

    let Ok(eye) = camera.single() else { return };

    let (turn, travel) = match barrel.last_eye {
        Some((was_at, was_facing)) => (
            was_facing.angle_between(eye.rotation) / dt.max(1e-4),
            was_at.distance(eye.translation) / dt.max(1e-4),
        ),
        None => (0.0, 0.0),
    };
    barrel.last_eye = Some((eye.translation, eye.rotation));
    let ramp = |value: f32, start: f32, end: f32| ((value - start) / (end - start)).clamp(0.0, 1.0);
    let motion = ramp(turn, BARREL_TRAIL_TURN_START, BARREL_TRAIL_TURN_FADE)
        .max(ramp(travel, BARREL_TRAIL_MOVE_START, BARREL_TRAIL_MOVE_FADE));
    let settle = (BARREL_TRAIL_FADE_SPEED * dt).min(1.0);
    barrel.visible += ((1.0 - motion) - barrel.visible) * settle;

    // A barrel only smokes once the firing stops, and new points stop going
    // down while the eye is moving hard, or coming to a stop would reveal the
    // streak the turn had already banked.
    let venting = barrel.since_shot >= BARREL_SMOKE_DELAY
        && barrel.since_shot <= BARREL_SMOKE_DELAY + BARREL_SMOKE_WINDOW;
    if barrel.visible > 0.35 && venting && barrel.next_point <= 0.0 {
        barrel.next_point = BARREL_TRAIL_INTERVAL;
        if let Ok((gun_transform, gun)) = gun.single() {
            let gun_drift = gun_transform.translation - gun.base_offset;
            let muzzle = eye.translation + eye.rotation * (active.def().muzzle_offset + gun_drift);
            // newest first, so index order runs muzzle upward
            barrel.points.insert(
                0,
                TrailPoint { position: muzzle, age: 0.0, sway_seed: time.elapsed_secs() * 7.3 },
            );
            barrel.points.truncate(BARREL_TRAIL_MAX_POINTS);
        }
    }

    let Some(mut mesh) = meshes.get_mut(&ribbon.mesh) else { return };
    write_ribbon(&barrel.points, eye.translation, barrel.visible, &mut mesh);
}

fn write_ribbon(points: &[TrailPoint], eye: Vec3, visible: f32, mesh: &mut Mesh) {
    let rows = BARREL_TRAIL_MAX_POINTS * RIBBON_COLUMNS;
    let mut positions = vec![[0.0f32; 3]; rows];
    let mut normals = vec![[0.0, 0.0, 1.0f32]; rows];
    let mut uvs = vec![[0.0f32; 2]; rows];
    let mut colors = vec![[0.0f32; 4]; rows];

    let live = points.len().min(BARREL_TRAIL_MAX_POINTS);
    for index in 0..live {
        let point = &points[index];
        let span = if index + 1 < live {
            points[index + 1].position - point.position
        } else if index > 0 {
            point.position - points[index - 1].position
        } else {
            Vec3::Y
        };
        let along = span.try_normalize().unwrap_or(Vec3::Y);
        let to_eye = (eye - point.position).try_normalize().unwrap_or(Vec3::Z);
        let side = along.cross(to_eye).try_normalize().unwrap_or(Vec3::X);

        let life = (point.age / BARREL_TRAIL_LIFETIME).clamp(0.0, 1.0);
        let half = (BARREL_TRAIL_WIDTH + point.age * BARREL_TRAIL_SPREAD) * 0.5;
        // squared so the column holds together off the muzzle and thins high up
        let alpha = BARREL_TRAIL_ALPHA * (1.0 - life) * (1.0 - life) * visible;

        // Edge columns carry no alpha, so the ribbon fades out sideways rather
        // than ending on a cut line and reading as a paper streamer.
        let row = index * RIBBON_COLUMNS;
        for (column, (offset, weight)) in
            [(-half, 0.0), (0.0, 1.0), (half, 0.0)].into_iter().enumerate()
        {
            positions[row + column] = (point.position + side * offset).to_array();
            normals[row + column] = to_eye.to_array();
            uvs[row + column] = [column as f32 * 0.5, life];
            colors[row + column] = [1.0, 1.0, 1.0, alpha * weight];
        }
    }

    // Unused rows collapse onto the oldest live point with no alpha, keeping
    // the buffer length constant instead of resizing it.
    let tail = points.get(live.saturating_sub(1)).map_or(Vec3::ZERO, |point| point.position);
    for index in live..BARREL_TRAIL_MAX_POINTS {
        let row = index * RIBBON_COLUMNS;
        for column in 0..RIBBON_COLUMNS {
            positions[row + column] = tail.to_array();
        }
    }

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
}

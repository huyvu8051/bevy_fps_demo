use bevy::prelude::*;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;

use crate::*;

const MUZZLE_SMOKE_SHADER: &str = "shaders/muzzle_smoke.wgsl";

const MUZZLE_FLASH_SHADER: &str = "shaders/muzzle_flash.wgsl";
const MUZZLE_FLASH_RAY_SCALE: f32 = 2.6;
const MUZZLE_FLASH_CORE_SIZE: f32 = 0.32;

const SHAPE_RADIAL_CORE: f32 = 0.0;
const SHAPE_DIRECTIONAL_BLADE: f32 = 1.0;

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub(crate) struct MuzzleSmokeMaterial {
    #[uniform(0)]
    settings: Vec4,
    #[texture(1)]
    #[sampler(2)]
    atlas: Handle<Image>,
}

impl MuzzleSmokeMaterial {
    fn new(seed: f32, atlas: Handle<Image>) -> Self {
        Self {
            settings: Vec4::new(0.0, seed % std::f32::consts::TAU, 0.0, 0.0),
            atlas,
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

pub(crate) fn muzzle_smoke_material(seed: f32, atlas: Handle<Image>) -> MuzzleSmokeMaterial {
    MuzzleSmokeMaterial::new(seed, atlas)
}

/// The sprite sheet the puff shader reads, baked offline from the raymarch it
/// replaced. One 1024 texture against 960 noise lookups per pixel, which is
/// what made a sustained burst fill-rate bound.
pub(crate) const SMOKE_ATLAS_PATH: &str = "textures/muzzle_smoke_atlas.png";

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

/// The three powder effects switch separately. They read as one thing but
/// they do not cost like one: the puff spawns per shot, the port wisp rides
/// every casing, and the trail is a mesh rebuilt each frame it lives.
macro_rules! smoke_switch {
    ($name:ident) => {
        #[derive(Resource)]
        pub(crate) struct $name(pub(crate) bool);

        impl Default for $name {
            fn default() -> Self {
                Self(true)
            }
        }
    };
}

smoke_switch!(MuzzleSmokeEnabled);
smoke_switch!(PortSmokeEnabled);
smoke_switch!(BarrelSmokeEnabled);

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
    smoke: Res<BarrelSmokeEnabled>,
) {
    // Switched off mid-trail the banked points have to go too, or the column
    // hangs in the air with nothing left to feed it.
    if !smoke.0 {
        barrel.points.clear();
        barrel.visible = 0.0;
        if let Some(mut mesh) = meshes.get_mut(&ribbon.mesh) {
            write_ribbon(&[], Vec3::ZERO, 0.0, &mut mesh);
        }
        return;
    }

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
            let muzzle_in_cam = gun_transform.mul_transform(gun.muzzle_local);
            let muzzle = eye.translation + eye.rotation * muzzle_in_cam.translation;
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

/// The native renderer only looks at `StandardMaterial`, so on that path the
/// flash and the smoke are invisible: their own materials carry their own
/// shaders and the extractor's query never matches them.
///
/// Rather than port two WGSL shaders, each gets a plain stand-in alongside.
/// The flash loses its shape for 0.05s, which is not long enough to read. The
/// smoke keeps its fade, because two seconds is.
pub(crate) fn attach_muzzle_fx_stand_ins(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut masks: Local<Option<(Handle<Image>, Handle<Image>)>>,
    flashes: Query<Entity, (Added<MuzzleFlash>, Without<MeshMaterial3d<StandardMaterial>>)>,
    smoke: Query<Entity, (Added<Smoke>, Without<MeshMaterial3d<StandardMaterial>>)>,
) {
    // The shape of both lives in the alpha channel. Without one the quad is
    // exactly that -- a square -- which is what a flat emissive stand-in
    // looked like the first time round.
    let (flash_mask, smoke_mask) = masks
        .get_or_insert_with(|| (
            images.add(alpha_mask(64, flash_alpha)),
            images.add(alpha_mask(64, smoke_alpha)),
        ))
        .clone();
    for entity in &flashes {
        // Black base, all of it emissive: the base colour is what the lighting
        // multiplies, and a muzzle flash is not lit by anything.
        commands.entity(entity).insert(MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgba(0.0, 0.0, 0.0, 1.0),
            base_color_texture: Some(flash_mask.clone()),
            // Inside 0..1, not an HDR value: this path writes straight to an
            // 8-bit target with no tonemapping, so (12, 6, 2) clipped every
            // channel to full and the flash came out white. The colour has to
            // live in the ratio, and the ratio only survives below 1.0.
            emissive: LinearRgba::new(1.0, 0.46, 0.13, 1.0),
            alpha_mode: bevy::material::AlphaMode::Blend,
            unlit: true,
            ..default()
        })));
    }
    for entity in &smoke {
        commands.entity(entity).insert(MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgba(0.42, 0.40, 0.38, SMOKE_STAND_IN_ALPHA),
            base_color_texture: Some(smoke_mask.clone()),
            alpha_mode: bevy::material::AlphaMode::Blend,
            unlit: true,
            ..default()
        })));
    }
}

/// Fades the stand-ins, which have no shader to do it for them.
pub(crate) fn fade_muzzle_fx_stand_ins(
    mut materials: ResMut<Assets<StandardMaterial>>,
    flashes: Query<(&MuzzleFlash, &MeshMaterial3d<StandardMaterial>)>,
    smoke: Query<(&Smoke, &MeshMaterial3d<StandardMaterial>)>,
) {
    for (flash, handle) in &flashes {
        if let Some(mut material) = materials.get_mut(&handle.0) {
            let left = 1.0 - elapsed_fraction_of(flash.remaining, MUZZLE_FLASH_LIFETIME);
            material.base_color.set_alpha(left);
        }
    }
    for (puff, handle) in &smoke {
        if let Some(mut material) = materials.get_mut(&handle.0) {
            let left = 1.0 - elapsed_fraction_of(puff.remaining, SMOKE_LIFETIME);
            material.base_color.set_alpha(left * SMOKE_STAND_IN_ALPHA);
        }
    }
}

/// A white square whose alpha is `shape(x, y)` over -1..1 in both axes.
///
/// `RenderAssetUsages::default()` rather than RENDER_WORLD: the native
/// extractor reads `image.data` on the CPU, and dropping it leaves the decal
/// untextured -- the same square this exists to avoid.
fn alpha_mask(size: u32, shape: fn(f32, f32) -> f32) -> Image {
    let mut data = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let fx = (x as f32 + 0.5) / size as f32 * 2.0 - 1.0;
            let fy = (y as f32 + 0.5) / size as f32 * 2.0 - 1.0;
            data.extend_from_slice(&[255, 255, 255, (shape(fx, fy).clamp(0.0, 1.0) * 255.0) as u8]);
        }
    }
    Image::new(
        bevy::render::render_resource::Extent3d { width: size, height: size, depth_or_array_layers: 1 },
        bevy::render::render_resource::TextureDimension::D2,
        data,
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        bevy::asset::RenderAssetUsages::default(),
    )
}

/// A bright core with four petals -- the shape the real shader draws, minus
/// its animation.
fn flash_alpha(x: f32, y: f32) -> f32 {
    let r = (x * x + y * y).sqrt();
    let reach = (1.0 - r).max(0.0);
    let petal = ops::cos(ops::atan2(y, x) * 2.0).abs().powf(5.0);
    (reach.powf(3.0) * 0.75 + reach.powf(1.3) * petal * 0.9).min(1.0)
}

/// A soft round puff, densest just off centre so it does not read as a disc.
fn smoke_alpha(x: f32, y: f32) -> f32 {
    let r = (x * x + y * y).sqrt();
    (1.0 - r).max(0.0).powf(1.8)
}

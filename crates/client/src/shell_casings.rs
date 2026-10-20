use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::math::ops;

use crate::{elapsed_fraction_of, muzzle_smoke_material, FpsCamera, MuzzleSmokeMaterial};


const CASING_LENGTH: f32 = 0.039;
const CASING_RADIUS: f32 = 0.0049;
const CASING_DENSITY: f32 = 8400.0;

/// Relative to the gun root. The bore runs 0.275 above that root, so the port
/// sits just under it rather than down at the magazine.
///
/// x stays pushed out from the receiver: tight against it the brass spawns
/// inside the viewmodel arm mesh and is hidden for the few frames it is on
/// screen.
const EJECT_PORT: Vec3 = Vec3::new(0.10, 0.225, 0.03);

const PORT_SMOKE_COUNT: usize = 3;
const PORT_SMOKE_SPEED: f32 = 0.55;
const PORT_SMOKE_RISE: f32 = 0.5;
const PORT_SMOKE_SIZE: f32 = 0.05;
const PORT_SMOKE_LIFETIME: f32 = 0.7;
const PORT_SMOKE_GROWTH: f32 = 1.6;
const EJECT_SPEED: f32 = 2.6;
const EJECT_UP: f32 = 0.55;
const EJECT_SCATTER: f32 = 0.22;
const EJECT_SPIN: f32 = 26.0;

const CASING_LIFETIME: f32 = 25.0;
/// A magazine is 50 rounds, so a budget under that threw brass away mid-burst.
const MAX_CASINGS: usize = 60;

#[derive(Resource)]
pub(crate) struct CasingAssets {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
    smoke_mesh: Handle<Mesh>,
}

#[derive(Component)]
pub(crate) struct PortSmoke {
    velocity: Vec3,
    remaining: f32,
}

#[derive(Component)]
pub(crate) struct ShellCasing {
    remaining: f32,
}

pub(crate) fn build_casing_assets(
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) -> CasingAssets {
    CasingAssets {
        mesh: meshes.add(Cylinder::new(CASING_RADIUS, CASING_LENGTH)),
        material: materials.add(StandardMaterial {
            base_color: Color::srgb(0.72, 0.56, 0.21),
            metallic: 1.0,
            perceptual_roughness: 0.28,
            ..default()
        }),
        smoke_mesh: meshes.add(Rectangle::new(PORT_SMOKE_SIZE, PORT_SMOKE_SIZE)),
    }
}

fn scatter(seed: f32) -> Vec3 {
    Vec3::new(ops::sin(seed * 1.7), ops::sin(seed * 2.3), ops::sin(seed * 3.1))
}

/// Throws a casing out of the ejection port. The port is on the gun, so recoil,
/// sway and the ADS pull all carry it; the player's own motion is added on top
/// or casings hang in the air behind a sprinting player.
pub(crate) fn eject_casing(
    commands: &mut Commands,
    assets: &CasingAssets,
    smoke_materials: &mut Assets<MuzzleSmokeMaterial>,
    eye: &Transform,
    gun: &Transform,
    player_velocity: Vec3,
    seed: f32,
) {
    let port = eye.translation + eye.rotation * (gun.translation + EJECT_PORT);
    let right = *eye.right();
    let up = *eye.up();
    let away = (right + up * EJECT_UP + scatter(seed) * EJECT_SCATTER).normalize_or_zero();

    commands.spawn((
        Name::new("ShellCasing"),
        Mesh3d(assets.mesh.clone()),
        MeshMaterial3d(assets.material.clone()),
        Transform::from_translation(port).with_rotation(eye.rotation),
        RigidBody::Dynamic,
        Collider::cylinder(CASING_RADIUS, CASING_LENGTH),
        ColliderDensity(CASING_DENSITY),
        // Map geometry is a trimesh, which has no thickness, and a 5mm casing
        // covers 8cm in a physics step once gravity has it. Without swept
        // collision it passes straight through the floor and is never seen
        // landing.
        SweptCcd::default(),
        LinearVelocity(away * EJECT_SPEED + player_velocity),
        AngularVelocity(scatter(seed * 1.9) * EJECT_SPIN),
        ShellCasing { remaining: CASING_LIFETIME },
    ));

    for i in 0..PORT_SMOKE_COUNT {
        let seed = seed + i as f32 * 53.7;
        let drift = (away + up * PORT_SMOKE_RISE + scatter(seed) * 0.4).normalize_or_zero();
        commands.spawn((
            Mesh3d(assets.smoke_mesh.clone()),
            MeshMaterial3d(smoke_materials.add(muzzle_smoke_material(seed * 97.0 + i as f32 * 31.0))),
            Transform::from_translation(port),
            PortSmoke { velocity: drift * PORT_SMOKE_SPEED, remaining: PORT_SMOKE_LIFETIME },
        ));
    }
}

pub(crate) fn update_port_smoke(
    time: Res<Time>,
    mut commands: Commands,
    camera: Query<&GlobalTransform, With<FpsCamera>>,
    mut materials: ResMut<Assets<MuzzleSmokeMaterial>>,
    mut puffs: Query<(Entity, &mut Transform, &mut PortSmoke, &MeshMaterial3d<MuzzleSmokeMaterial>)>,
) {
    let dt = time.delta_secs();
    let eye = camera.single().ok().map(|global| global.translation());
    for (entity, mut transform, mut puff, handle) in &mut puffs {
        transform.translation += puff.velocity * dt;
        transform.scale *= 1.0 + PORT_SMOKE_GROWTH * dt;
        if let Some(eye) = eye {
            let to_eye = (eye - transform.translation).normalize_or_zero();
            if to_eye != Vec3::ZERO {
                transform.rotation = Quat::from_rotation_arc(Vec3::Z, to_eye);
            }
        }
        puff.remaining -= dt;
        if puff.remaining <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        if let Some(mut material) = materials.get_mut(&handle.0) {
            material.set_life_progress(elapsed_fraction_of(puff.remaining, PORT_SMOKE_LIFETIME));
        }
    }
}

/// Casings are cheap but not free, and a magazine dump leaves a pile of rigid
/// bodies the solver keeps revisiting. Oldest go first once over budget.
pub(crate) fn update_casings(
    time: Res<Time>,
    mut commands: Commands,
    mut casings: Query<(Entity, &mut ShellCasing)>,
) {
    // Shorten the oldest instead of despawning outright: brass vanishing from
    // under your feet mid-burst is more noticeable than brass fading early.
    let mut over_budget = casings.iter().len().saturating_sub(MAX_CASINGS);
    for (entity, mut casing) in &mut casings {
        if over_budget > 0 && casing.remaining > 1.0 {
            over_budget -= 1;
            casing.remaining = 1.0;
        }
        casing.remaining -= time.delta_secs();
        if casing.remaining <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::math::ops;


const CASING_LENGTH: f32 = 0.039;
const CASING_RADIUS: f32 = 0.0049;
const CASING_DENSITY: f32 = 8400.0;

const EJECT_PORT: Vec3 = Vec3::new(0.055, 0.02, 0.02);
const EJECT_SPEED: f32 = 2.6;
const EJECT_UP: f32 = 0.55;
const EJECT_SCATTER: f32 = 0.22;
const EJECT_SPIN: f32 = 26.0;

const CASING_LIFETIME: f32 = 9.0;
const MAX_CASINGS: usize = 24;

#[derive(Resource)]
pub(crate) struct CasingAssets {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
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
        LinearVelocity(away * EJECT_SPEED + player_velocity),
        AngularVelocity(scatter(seed * 1.9) * EJECT_SPIN),
        ShellCasing { remaining: CASING_LIFETIME },
    ));
}

/// Casings are cheap but not free, and a magazine dump leaves a pile of rigid
/// bodies the solver keeps revisiting. Oldest go first once over budget.
pub(crate) fn update_casings(
    time: Res<Time>,
    mut commands: Commands,
    mut casings: Query<(Entity, &mut ShellCasing)>,
) {
    let mut over_budget = casings.iter().len().saturating_sub(MAX_CASINGS);
    for (entity, mut casing) in &mut casings {
        if over_budget > 0 {
            over_budget -= 1;
            commands.entity(entity).despawn();
            continue;
        }
        casing.remaining -= time.delta_secs();
        if casing.remaining <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}

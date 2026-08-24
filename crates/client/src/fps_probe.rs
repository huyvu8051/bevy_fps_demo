//! Absolute-minimum FPS probe: a single static camera looking at a plain
//! room built from 5 primitive cuboids (floor + 4 walls, no glTF, no
//! materials/lighting complexity beyond one directional light) — no game
//! logic, no UI, no touch input, no networking, no physics, no per-frame
//! systems at all. Deliberately independent of everything else in this
//! crate so it measures raw achievable FPS/CPU for "camera + a small
//! static scene" with nothing else able to be blamed for the number.
//!
//! Wired up only through cadisplaylink::build_cadisplaylink_fps_probe_app
//! (see that function for how to launch it on-device).

use bevy::prelude::*;

const ARENA_HALF_SIZE: f32 = 10.0;
const ARENA_WALL_HEIGHT: f32 = 4.0;
const ARENA_WALL_THICKNESS: f32 = 0.3;

pub fn spawn_arena_and_camera(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 3.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 3000.0,
            shadows_enabled: false,
            ..Default::default()
        },
        Transform::from_xyz(4.0, 8.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    let floor_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.35, 0.35, 0.38),
        perceptual_roughness: 0.9,
        ..Default::default()
    });
    let wall_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.55, 0.5, 0.45),
        perceptual_roughness: 0.85,
        ..Default::default()
    });

    let floor_size = ARENA_HALF_SIZE * 2.0;
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(floor_size, ARENA_WALL_THICKNESS, floor_size))),
        MeshMaterial3d(floor_material),
        Transform::from_xyz(0.0, -ARENA_WALL_THICKNESS / 2.0, 0.0),
    ));

    let wall_span = floor_size + ARENA_WALL_THICKNESS;
    let walls = [
        (Vec3::new(0.0, ARENA_WALL_HEIGHT / 2.0, -ARENA_HALF_SIZE), wall_span, ARENA_WALL_THICKNESS),
        (Vec3::new(0.0, ARENA_WALL_HEIGHT / 2.0, ARENA_HALF_SIZE), wall_span, ARENA_WALL_THICKNESS),
        (Vec3::new(-ARENA_HALF_SIZE, ARENA_WALL_HEIGHT / 2.0, 0.0), ARENA_WALL_THICKNESS, wall_span),
        (Vec3::new(ARENA_HALF_SIZE, ARENA_WALL_HEIGHT / 2.0, 0.0), ARENA_WALL_THICKNESS, wall_span),
    ];
    for (position, size_x, size_z) in walls {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(size_x, ARENA_WALL_HEIGHT, size_z))),
            MeshMaterial3d(wall_material.clone()),
            Transform::from_translation(position),
        ));
    }
}

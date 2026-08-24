//! FPS probe: loads the real Sponza scene (same asset the main game uses)
//! with a free-fly camera (WASD + mouse-look, no physics/collision, no
//! touch input) — no other game logic, no UI, no networking. Deliberately
//! independent of everything else in this crate so it measures achievable
//! FPS/CPU for "camera + Sponza" with nothing else able to be blamed for
//! the number, while still letting you actually walk around and look at
//! different parts of the scene on-device.
//!
//! Wired up only through cadisplaylink::build_cadisplaylink_fps_probe_app
//! (see that function for how to launch it on-device).

use bevy::input::mouse::MouseMotion;
use bevy::prelude::*;

const SCENE_PATH: &str = "models/Sponza/Sponza.gltf";
const MOVE_SPEED: f32 = 6.0;
const LOOK_SENSITIVITY: f32 = 0.003;

#[derive(Component, Default)]
pub struct FlyCamera {
    pub yaw: f32,
    pub pitch: f32,
}

pub fn spawn_arena_and_camera(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 3.0, 4.0).looking_at(Vec3::new(0.0, 3.0, 0.0), Vec3::Y),
        FlyCamera::default(),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 8_000.0,
            shadows_enabled: false,
            ..Default::default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.9, 0.5, 0.0)),
    ));
    commands.spawn((
        SceneRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(SCENE_PATH))),
        Transform::default(),
    ));
}

// WASD moves relative to look direction (Y-locked, so W/S don't fly up/down
// looking at the ceiling/floor); no gravity, no collision — pure fly-cam.
pub fn fly_camera_move(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut query: Query<(&mut Transform, &FlyCamera)>,
) {
    let Ok((mut transform, fly)) = query.single_mut() else {
        return;
    };
    let yaw_rot = Quat::from_rotation_y(fly.yaw);
    let forward = yaw_rot * Vec3::NEG_Z;
    let right = yaw_rot * Vec3::X;

    let mut direction = Vec3::ZERO;
    if keys.pressed(KeyCode::KeyW) {
        direction += forward;
    }
    if keys.pressed(KeyCode::KeyS) {
        direction -= forward;
    }
    if keys.pressed(KeyCode::KeyD) {
        direction += right;
    }
    if keys.pressed(KeyCode::KeyA) {
        direction -= right;
    }
    if keys.pressed(KeyCode::Space) {
        direction += Vec3::Y;
    }
    if keys.pressed(KeyCode::ShiftLeft) {
        direction -= Vec3::Y;
    }
    if direction != Vec3::ZERO {
        transform.translation += direction.normalize() * MOVE_SPEED * time.delta_secs();
    }
}

// Reads raw MouseMotion (fed on iOS by a connected Bluetooth mouse via
// GameControllerInput.swift, which locks look the instant one connects —
// see cadisplaylink_mouse_connected) — same yaw/pitch accumulation math as
// the real game's mouse_look, simplified (no aim-sensitivity modifier).
pub fn fly_camera_look(mut motion: MessageReader<MouseMotion>, mut query: Query<(&mut Transform, &mut FlyCamera)>) {
    let Ok((mut transform, mut fly)) = query.single_mut() else {
        return;
    };
    let mut delta = Vec2::ZERO;
    for event in motion.read() {
        delta += event.delta;
    }
    if delta == Vec2::ZERO {
        return;
    }
    fly.yaw -= delta.x * LOOK_SENSITIVITY;
    fly.pitch = (fly.pitch - delta.y * LOOK_SENSITIVITY).clamp(-1.54, 1.54);
    transform.rotation = Quat::from_euler(EulerRot::YXZ, fly.yaw, fly.pitch, 0.0);
}

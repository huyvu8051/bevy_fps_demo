//! Loads the same Sponza glTF our own MINIMAL_MAP_PROBE defaults to, so the
//! CADisplayLink-driven CPU/FPS numbers measured here are directly
//! comparable to the winit-driven MINIMAL_MAP_PROBE build in crates/client
//! — same geometry, same lighting, only the event-loop/windowing layer
//! differs.

use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;

pub struct SimpleScenePlugin;

impl Plugin for SimpleScenePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(FrameTimeDiagnosticsPlugin::default())
            .init_resource::<CpuUsageState>()
            .add_systems(Startup, setup)
            .add_systems(Update, (sample_cpu_usage, update_debug_overlay));
    }
}

#[derive(Component)]
struct DebugOverlayText;

// getrusage(RUSAGE_SELF) gives cumulative process CPU time (user+system)
// since process start — same primitive `top`/Instruments use under the
// hood. Sampling it once a second and dividing by wall-clock elapsed gives
// a process CPU% comparable to what Instruments/Activity Monitor report,
// without pulling in a whole sysinfo crate for one number.
#[derive(Resource)]
struct CpuUsageState {
    last_sample_wall: f64,
    last_cpu_seconds: f64,
    last_percent: f32,
}

impl Default for CpuUsageState {
    fn default() -> Self {
        Self {
            last_sample_wall: 0.0,
            last_cpu_seconds: process_cpu_seconds(),
            last_percent: 0.0,
        }
    }
}

fn process_cpu_seconds() -> f64 {
    unsafe {
        let mut usage: libc::rusage = std::mem::zeroed();
        libc::getrusage(libc::RUSAGE_SELF, &mut usage);
        let user = usage.ru_utime.tv_sec as f64 + usage.ru_utime.tv_usec as f64 / 1_000_000.0;
        let sys = usage.ru_stime.tv_sec as f64 + usage.ru_stime.tv_usec as f64 / 1_000_000.0;
        user + sys
    }
}

fn sample_cpu_usage(time: Res<Time>, mut state: ResMut<CpuUsageState>) {
    let elapsed_since_start = time.elapsed_secs_f64();
    let elapsed_since_sample = elapsed_since_start - state.last_sample_wall;
    if elapsed_since_sample < 1.0 {
        return;
    }
    let cpu_now = process_cpu_seconds();
    let cpu_delta = cpu_now - state.last_cpu_seconds;
    state.last_percent = ((cpu_delta / elapsed_since_sample) * 100.0) as f32;
    state.last_cpu_seconds = cpu_now;
    state.last_sample_wall = elapsed_since_start;
}

fn update_debug_overlay(
    diagnostics: Res<DiagnosticsStore>,
    cpu: Res<CpuUsageState>,
    mut text: Query<&mut Text, With<DebugOverlayText>>,
) {
    let Ok(mut text) = text.single_mut() else {
        return;
    };
    let fps = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|d| d.smoothed())
        .unwrap_or(0.0);
    text.0 = format!("FPS: {fps:.1}\nCPU: {:.1}%  (process, 1s avg)", cpu.last_percent);
}

// Same glTF our own MINIMAL_MAP_PROBE loads by default (crates/client's
// SCENE_PATH) — copied into this experiment's assets/models/Sponza/ so the
// two apps are showing identical geometry, not just similarly-shaped ones.
const SCENE_PATH: &str = "models/Sponza/Sponza.gltf";

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 3.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 3000.0,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn(SceneRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(SCENE_PATH))));
    commands.spawn((
        Text::new("FPS: --\nCPU: --"),
        TextFont {
            font_size: 20.0,
            ..default()
        },
        TextColor(Color::srgb(1.0, 1.0, 0.3)),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(16.0),
            top: Val::Px(16.0),
            ..default()
        },
        DebugOverlayText,
    ));
}

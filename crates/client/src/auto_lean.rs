use avian3d::prelude::*;
use bevy::prelude::*;

use crate::autopilot::has_line_of_sight;
use crate::*;

const AUTO_LEAN_RANGE: f32 = 26.0;
const AUTO_LEAN_MIN_RANGE: f32 = 2.5;
const AUTO_LEAN_FORWARD_DOT: f32 = 0.35;
const AUTO_LEAN_HOLD_SECONDS: f32 = 0.35;

#[derive(Resource, Default)]
pub(crate) struct AutoLean {
    pub(crate) suggestion: f32,
    hold: f32,
}

fn aim_height(point: Vec3) -> Vec3 {
    point + Vec3::Y * (ZOMBIE_OUTER_HEIGHT * 0.65)
}

/// Looks for a zombie the player cannot see from where they stand but could
/// see by leaning, and offers that direction. Only ever a suggestion: pressing
/// Q or E overrides it, so the assist never takes the lean away from you.
pub(crate) fn suggest_auto_lean(
    time: Res<Time>,
    autopilot: Res<Autopilot>,
    game_over: Res<GameOver>,
    spatial_query: SpatialQuery,
    camera: Query<(&GlobalTransform, &FpsCamera)>,
    zombies: Query<&Transform, With<Zombie>>,
    mut auto: ResMut<AutoLean>,
) {
    if autopilot.0 || game_over.0 {
        auto.suggestion = 0.0;
        auto.hold = 0.0;
        return;
    }
    let Ok((eye_global, cam)) = camera.single() else {
        auto.suggestion = 0.0;
        return;
    };
    let eye = eye_global.translation();
    let forward = *eye_global.forward();
    let sideways = Vec3::new(ops::cos(cam.yaw), 0.0, -ops::sin(cam.yaw));

    let mut best: Option<(f32, f32)> = None;
    for zombie in &zombies {
        let target = aim_height(zombie.translation);
        let delta = target - eye;
        let distance = delta.length();
        if !(AUTO_LEAN_MIN_RANGE..AUTO_LEAN_RANGE).contains(&distance) {
            continue;
        }
        if delta.normalize_or_zero().dot(forward) < AUTO_LEAN_FORWARD_DOT {
            continue;
        }
        if has_line_of_sight(&spatial_query, eye, target) {
            continue;
        }
        let direction = if has_line_of_sight(&spatial_query, eye + sideways * LEAN_SIDE_OFFSET, target) {
            1.0
        } else if has_line_of_sight(&spatial_query, eye - sideways * LEAN_SIDE_OFFSET, target) {
            -1.0
        } else {
            continue;
        };
        if best.is_none_or(|(closest, _)| distance < closest) {
            best = Some((distance, direction));
        }
    }

    match best {
        Some((_, direction)) => {
            auto.suggestion = direction;
            auto.hold = AUTO_LEAN_HOLD_SECONDS;
        }
        // Hold the last suggestion briefly. Peeking usually reveals the zombie,
        // which removes the reason to peek, and without this the lean snaps
        // straight back and oscillates.
        None => {
            auto.hold -= time.delta_secs();
            if auto.hold <= 0.0 {
                auto.suggestion = 0.0;
            }
        }
    }
}

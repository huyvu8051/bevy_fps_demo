use avian3d::prelude::*;
use bevy::prelude::*;

use crate::*;

const COVER_RANGE: f32 = 2.6;
const OPENED_RANGE: f32 = 5.0;
const ADS_THRESHOLD: f32 = 0.5;

/// How far out to try, smallest first. Both the roll and the sideways shift
/// scale off the same value, so a fraction here is a genuinely shallower lean
/// rather than a rounded-off full one.
const PEEK_STEPS: [f32; 5] = [0.25, 0.45, 0.65, 0.85, 1.0];
const HOLD_SECONDS: f32 = 0.3;

#[derive(Resource, Default)]
pub(crate) struct AutoLean {
    pub(crate) suggestion: f32,
    hold: f32,
}

fn blocked_within(spatial_query: &SpatialQuery, from: Vec3, facing: Dir3, range: f32) -> bool {
    spatial_query
        .cast_ray(from, facing, range, true, &SpatialQueryFilter::default())
        .is_some()
}

/// Leans out around cover the player is pressed against, the way Call of Duty
/// does: driven by the geometry in front of the gun, not by where an enemy is.
///
/// The test is asymmetry. If the shot is blocked from here but clear from an
/// eye shifted sideways, then an edge is being hugged and leaning that way
/// opens the angle. Blocked both ways is a flat wall, and nothing to peek.
pub(crate) fn suggest_auto_lean(
    time: Res<Time>,
    autopilot: Res<Autopilot>,
    game_over: Res<GameOver>,
    spatial_query: SpatialQuery,
    camera: Query<(&GlobalTransform, &FpsCamera, &Lean)>,
    gun: Query<&Gun>,
    mut auto: ResMut<AutoLean>,
) {
    let aiming = gun.single().map(|g| g.ads_blend >= ADS_THRESHOLD).unwrap_or(false);
    if autopilot.0 || game_over.0 || !aiming {
        auto.suggestion = 0.0;
        auto.hold = 0.0;
        return;
    }
    let Ok((eye_global, cam, lean)) = camera.single() else { return };
    let Ok(facing) = Dir3::new(*eye_global.forward()) else { return };
    let sideways = Vec3::new(ops::cos(cam.yaw), 0.0, -ops::sin(cam.yaw));

    // Probe from where the eye would be standing straight, not from where it
    // currently is. The camera position already carries the lean this system
    // asked for, so testing from it means a successful lean clears the very
    // cover check that requested it, the suggestion drops, the body swings
    // back, and the corner starts over — which is the judder.
    let eye = eye_global.translation() - sideways * LEAN_SIDE_OFFSET * lean.current;

    // The smallest step that clears only grazes the edge: enough to see a
    // sliver past it, not enough to aim through it. Take the next step out so
    // the peek is actually usable, which is what the corner is for.
    let mut direction = None;
    if blocked_within(&spatial_query, eye, facing, COVER_RANGE) {
        'search: for (step, amount) in PEEK_STEPS.iter().enumerate() {
            for side in [1.0, -1.0] {
                let probe = eye + sideways * side * LEAN_SIDE_OFFSET * amount;
                if !blocked_within(&spatial_query, probe, facing, OPENED_RANGE) {
                    let clear = PEEK_STEPS[(step + 1).min(PEEK_STEPS.len() - 1)];
                    direction = Some(side * clear);
                    break 'search;
                }
            }
        }
    }

    match direction {
        Some(side) => {
            auto.suggestion = side;
            auto.hold = HOLD_SECONDS;
        }
        // Held briefly on the way out. Leaning moves the eye past the edge,
        // which clears the very probe that asked for the lean, so without this
        // it would snap back and oscillate on the corner.
        None => {
            auto.hold -= time.delta_secs();
            if auto.hold <= 0.0 {
                auto.suggestion = 0.0;
            }
        }
    }
}

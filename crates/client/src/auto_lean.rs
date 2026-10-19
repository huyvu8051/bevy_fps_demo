use avian3d::prelude::*;
use bevy::prelude::*;

use crate::*;

const COVER_RANGE: f32 = 1.3;
const OPENED_RANGE: f32 = 5.0;
const PEEK_OFFSET: f32 = LEAN_SIDE_OFFSET;
const ADS_THRESHOLD: f32 = 0.5;
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
    camera: Query<(&GlobalTransform, &FpsCamera)>,
    gun: Query<&Gun>,
    mut auto: ResMut<AutoLean>,
) {
    let aiming = gun.single().map(|g| g.ads_blend >= ADS_THRESHOLD).unwrap_or(false);
    if autopilot.0 || game_over.0 || !aiming {
        auto.suggestion = 0.0;
        auto.hold = 0.0;
        return;
    }
    let Ok((eye_global, cam)) = camera.single() else { return };
    let eye = eye_global.translation();
    let Ok(facing) = Dir3::new(*eye_global.forward()) else { return };
    let sideways = Vec3::new(ops::cos(cam.yaw), 0.0, -ops::sin(cam.yaw));

    let direction = if !blocked_within(&spatial_query, eye, facing, COVER_RANGE) {
        None
    } else if !blocked_within(&spatial_query, eye + sideways * PEEK_OFFSET, facing, OPENED_RANGE) {
        Some(1.0)
    } else if !blocked_within(&spatial_query, eye - sideways * PEEK_OFFSET, facing, OPENED_RANGE) {
        Some(-1.0)
    } else {
        None
    };

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

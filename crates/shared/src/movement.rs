use avian3d::prelude::*;
use bevy::prelude::*;

use crate::{
    ClientInput, Player, PlayerStance, AIM_MOVE_SPEED_MULTIPLIER, CROUCH_LENGTH, CROUCH_SPEED_MULTIPLIER,
    GRAVITY, GROUND_NORMAL_MIN_Y, JUMP_SPEED, MOVE_SPEED, PLAYER_LENGTH, PLAYER_RADIUS, PRONE_LENGTH,
    PRONE_SPEED_MULTIPLIER, SPRINT_MULTIPLIER,
};

// The one authoritative player-movement step, shared by crates/server
// (where it's the real simulation, driven by whatever ClientInput actually
// arrived) and crates/client (where it's client-side prediction, run
// immediately on the local input before the server round-trip completes,
// then re-run during reconciliation to replay any not-yet-confirmed
// inputs on top of the server's latest confirmed position). Both sides
// must do the *exact* same math here or prediction will constantly
// mispredict and visibly correct itself.
pub fn simulate_player_step(mover: &MoveAndSlide, transform: &mut Transform, player: &mut Player, stance: PlayerStance, input: &ClientInput, dt: f32) {
    let move_speed = if input.aiming {
        MOVE_SPEED * AIM_MOVE_SPEED_MULTIPLIER
    } else {
        match stance {
            PlayerStance::Standing if input.sprint => MOVE_SPEED * SPRINT_MULTIPLIER,
            PlayerStance::Standing => MOVE_SPEED,
            PlayerStance::Crouching => MOVE_SPEED * CROUCH_SPEED_MULTIPLIER,
            PlayerStance::Prone => MOVE_SPEED * PRONE_SPEED_MULTIPLIER,
        }
    };
    let capsule_length = match stance {
        PlayerStance::Standing => PLAYER_LENGTH,
        PlayerStance::Crouching => CROUCH_LENGTH,
        PlayerStance::Prone => PRONE_LENGTH,
    };

    let yaw_rotation = Quat::from_rotation_y(input.yaw);
    let forward = yaw_rotation * Vec3::NEG_Z;
    let right = yaw_rotation * Vec3::X;
    let mut horizontal = forward * input.move_axis.y + right * input.move_axis.x;
    if horizontal != Vec3::ZERO {
        horizontal = horizontal.normalize() * move_speed;
    }

    let mut vertical_velocity = player.vertical_velocity;
    if player.grounded {
        if input.jump {
            vertical_velocity = JUMP_SPEED;
        } else if vertical_velocity < 0.0 {
            vertical_velocity = -1.0;
        }
    }
    vertical_velocity += GRAVITY * dt;

    let velocity = horizontal + Vec3::Y * vertical_velocity;
    let shape = Collider::capsule(PLAYER_RADIUS, capsule_length);
    let config = MoveAndSlideConfig::default();
    let filter = SpatialQueryFilter::default();

    let mut grounded_this_frame = false;
    let output = mover.move_and_slide(
        &shape,
        transform.translation,
        transform.rotation,
        velocity,
        std::time::Duration::from_secs_f32(dt.max(0.0)),
        &config,
        &filter,
        |hit| {
            if hit.normal.y > GROUND_NORMAL_MIN_Y {
                grounded_this_frame = true;
            }
            MoveAndSlideHitResponse::Accept
        },
    );

    transform.translation = output.position;
    player.grounded = grounded_this_frame;
    player.vertical_velocity = if grounded_this_frame { vertical_velocity.min(0.0) } else { vertical_velocity };
}

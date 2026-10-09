use avian3d::prelude::*;
use bevy::prelude::*;

use crate::{
    ClientInput, Player, PlayerStance, AIM_MOVE_SPEED_MULTIPLIER, CROUCH_LENGTH, CROUCH_SPEED_MULTIPLIER,
    GRAVITY, ground_normal_min_y, JUMP_SPEED, MOVE_SPEED, NOCLIP_SPEED, PLAYER_LENGTH, PLAYER_RADIUS,
    PRONE_LENGTH, PRONE_SPEED_MULTIPLIER, SPRINT_MULTIPLIER,
};

/// Uphill direction along a surface with this normal.
fn slope_tangent(normal: Vec3) -> Vec3 {
    (Vec3::Y - normal * normal.dot(Vec3::Y)).normalize_or_zero()
}

/// Handles one `move_and_slide` contact and reports whether it counts as
/// ground.
///
/// Sliding alone bleeds velocity downhill every frame, which is what makes a
/// walkable ramp feel unclimbable, and lets a steep face shove a falling body
/// upwards. Cancelling the along-slope component in those two cases is what
/// keeps both from happening.
pub fn resolve_slope_hit(hit: &mut MoveAndSlideHitData, input_velocity: Vec3, grounded: bool) -> bool {
    let normal = hit.normal.as_vec3();
    let walkable = normal.y > ground_normal_min_y();
    let tangent = slope_tangent(normal);
    let intent = input_velocity.dot(tangent);
    let along_slope = hit.velocity.dot(tangent);

    let pushed_up_a_cliff = !walkable && along_slope > 0.0 && intent <= 0.0;
    let dragged_down_a_ramp = walkable && grounded && along_slope < 0.0 && intent >= 0.0;
    if pushed_up_a_cliff || dragged_down_a_ramp {
        *hit.velocity -= tangent * along_slope;
    }

    walkable
}

pub fn simulate_player_step(mover: &MoveAndSlide, transform: &mut Transform, player: &mut Player, stance: PlayerStance, input: &ClientInput, dt: f32) {
    if input.noclip {
        simulate_noclip_step(transform, input, dt);
        player.vertical_velocity = 0.0;
        player.grounded = true;
        return;
    }

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

    let was_grounded = player.grounded;
    let mut grounded_this_frame = false;
    let output = mover.move_and_slide(
        &shape,
        transform.translation,
        transform.rotation,
        velocity,
        std::time::Duration::from_secs_f32(dt.max(0.0)),
        &config,
        &filter,
        |mut hit| {
            if resolve_slope_hit(&mut hit, velocity, was_grounded) {
                grounded_this_frame = true;
            }
            MoveAndSlideHitResponse::Accept
        },
    );

    transform.translation = output.position;
    player.grounded = grounded_this_frame;
    player.vertical_velocity = if grounded_this_frame { vertical_velocity.min(0.0) } else { vertical_velocity };
}

fn simulate_noclip_step(transform: &mut Transform, input: &ClientInput, dt: f32) {
    let speed = if input.sprint { NOCLIP_SPEED * SPRINT_MULTIPLIER } else { NOCLIP_SPEED };

    let yaw_rotation = Quat::from_rotation_y(input.yaw);
    let forward = yaw_rotation * Vec3::NEG_Z;
    let right = yaw_rotation * Vec3::X;
    let horizontal = forward * input.move_axis.y + right * input.move_axis.x;

    let vertical = if input.jump {
        1.0
    } else if input.stance == PlayerStance::Crouching {
        -1.0
    } else {
        0.0
    };

    let direction = horizontal + Vec3::Y * vertical;
    let direction = direction.normalize_or_zero();
    transform.translation += direction * speed * dt;
}

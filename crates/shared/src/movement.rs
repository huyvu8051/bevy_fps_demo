use avian3d::prelude::*;
use bevy::prelude::*;

use crate::{
    ClientInput, Player, PlayerStance, AIM_MOVE_SPEED_MULTIPLIER, CROUCH_LENGTH, CROUCH_SPEED_MULTIPLIER,
    GRAVITY, ground_normal_min_y, JUMP_SPEED, MOVE_SPEED, NOCLIP_SPEED, PLAYER_LENGTH, PLAYER_RADIUS,
    PRONE_LENGTH, PRONE_SPEED_MULTIPLIER, SPRINT_MULTIPLIER,
};

const PROGRESS_MEANING_UNOBSTRUCTED: f32 = 0.7;
const MIN_HEADROOM_FRACTION: f32 = 0.5;
const LANDING_PROBE_OVERSHOOT: f32 = 0.05;

pub const STEP_HEIGHT: f32 = 0.45;

pub fn blocked_horizontally(start: Vec3, end: Vec3, horizontal_velocity: Vec3, dt: f32) -> bool {
    let wanted = horizontal_velocity.length() * dt;
    if wanted <= 1e-4 {
        return false;
    }
    let moved = Vec3::new(end.x - start.x, 0.0, end.z - start.z).length();
    moved < wanted * PROGRESS_MEANING_UNOBSTRUCTED
}

pub fn try_step_up(
    mover: &MoveAndSlide,
    shape: &Collider,
    rotation: Quat,
    start: Vec3,
    horizontal_velocity: Vec3,
    dt: f32,
    config: &MoveAndSlideConfig,
    filter: &SpatialQueryFilter,
) -> Option<Vec3> {
    let headroom = mover
        .spatial_query
        .cast_shape(shape, start, rotation, Dir3::Y, &ShapeCastConfig::from_max_distance(STEP_HEIGHT), filter)
        .map_or(STEP_HEIGHT, |hit| hit.distance);
    if headroom < STEP_HEIGHT * MIN_HEADROOM_FRACTION {
        return None;
    }

    let raised = start + Vec3::Y * headroom;
    let stepped = mover.move_and_slide(
        shape,
        raised,
        rotation,
        horizontal_velocity,
        std::time::Duration::from_secs_f32(dt.max(0.0)),
        config,
        filter,
        |_| MoveAndSlideHitResponse::Accept,
    );

    let landing = mover.spatial_query.cast_shape(
        shape,
        stepped.position,
        rotation,
        Dir3::NEG_Y,
        &ShapeCastConfig::from_max_distance(headroom + LANDING_PROBE_OVERSHOOT),
        filter,
    )?;
    if landing.normal1.y <= ground_normal_min_y() {
        return None;
    }
    Some(stepped.position - Vec3::Y * landing.distance)
}

fn slope_tangent(normal: Vec3) -> Vec3 {
    (Vec3::Y - normal * normal.dot(Vec3::Y)).normalize_or_zero()
}

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

    let horizontal_velocity = Vec3::new(velocity.x, 0.0, velocity.z);
    let stepped_up = was_grounded
        .then(|| blocked_horizontally(transform.translation, output.position, horizontal_velocity, dt))
        .unwrap_or(false)
        .then(|| {
            try_step_up(
                &mover,
                &shape,
                transform.rotation,
                transform.translation,
                horizontal_velocity,
                dt,
                &config,
                &filter,
            )
        })
        .flatten();

    transform.translation = stepped_up.unwrap_or(output.position);
    player.grounded = grounded_this_frame || stepped_up.is_some();
    player.vertical_velocity = if player.grounded { vertical_velocity.min(0.0) } else { vertical_velocity };
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

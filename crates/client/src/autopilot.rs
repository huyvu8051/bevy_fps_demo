
use avian3d::prelude::{SpatialQuery, SpatialQueryFilter};
use bevy::math::ops;
use bevy::prelude::*;

use crate::{FpsCamera, Lean, Player, TouchAiming, LEAN_SIDE_OFFSET};
use shared::{random_aim_point, resolve_bone_world_positions, Health, NetPlayer, Zombie, ZombieHitboxBones, PLAYER_MAX_HEALTH};

#[derive(Resource)]
pub(crate) struct Autopilot(pub(crate) bool);
impl Default for Autopilot {
    fn default() -> Self {
        Autopilot(crate::autopilot_enabled_by_default(crate::current_scenario()))
    }
}

#[derive(Resource)]
pub(crate) struct AutopilotLookOnly(pub(crate) bool);
impl Default for AutopilotLookOnly {

    fn default() -> Self {
        Self(std::env::var("FORCE_LOOK_AT_TARGET").is_ok())
    }
}

#[derive(Resource, Default)]
pub(crate) struct AutopilotState {
    pub(crate) target: Option<Entity>,
    retarget_timer: f32,

    pub(crate) aiming_region: Option<&'static str>,

    hold_fire_timer: f32,
    pub(crate) time_since_swap: f32,

    tap_fire_timer: f32,

    travel_dir: Option<Vec3>,
}

#[derive(Resource, Default)]
pub(crate) struct AutopilotIntent {
    pub(crate) move_axis: Vec2,
    pub(crate) sprint: bool,
    pub(crate) fire: bool,
    pub(crate) aim: bool,
}

const AUTOPILOT_DETECT_RANGE: f32 = 80.0;

const AUTOPILOT_ENGAGE_RANGE: f32 = 40.0;

const AUTOPILOT_RETREAT_RANGE: f32 = 5.0;
const AUTOPILOT_RETARGET_INTERVAL: f32 = 1.2;

const AUTOPILOT_ADS_MIN_DISTANCE: f32 = 25.0;

const AUTOPILOT_TARGET_SWITCH_HOLD_MIN: f32 = 0.1;
const AUTOPILOT_TARGET_SWITCH_HOLD_MAX: f32 = 0.3;

const AUTOPILOT_TAP_FIRE_DISTANCE: f32 = 20.0;

const AUTOPILOT_TAP_FIRE_INTERVAL: f32 = 0.15;

const AUTOPILOT_TURN_RATE_DEG: f32 = 260.0;

const AUTOPILOT_TURN_FAR_DISTANCE: f32 = 55.0;

const AUTOPILOT_TURN_FADE_IN_DURATION: f32 = 0.4;

const AUTOPILOT_FIRE_AIM_TOLERANCE_DEG: f32 = 12.0;
const AUTOPILOT_SPRINT_APPROACH_DISTANCE: f32 = 22.0;

/// How far a blind approach has to be before it is worth sprinting. Pathing
/// around a corner to something 15m away is a walk; the old threshold was the
/// retreat range, so any detour past 5m broke into a run.
const AUTOPILOT_SPRINT_PATHING_DISTANCE: f32 = 15.0;
const AUTOPILOT_LOW_HEALTH_FRACTION: f32 = 0.3;
const AUTOPILOT_LOW_HEALTH_RETREAT_BONUS: f32 = 4.0;
const AUTOPILOT_IDLE_SCAN_DEG_PER_SEC: f32 = 20.0;

const AUTOPILOT_LOS_SLACK: f32 = 0.3;

const AUTOPILOT_PEEK_MAX_DISTANCE: f32 = 32.0;

const AUTOPILOT_LEAN_SPEED: f32 = 6.0;

const AUTOPILOT_AVOID_ANGLES_DEG: [f32; 6] = [22.0, -22.0, 45.0, -45.0, 70.0, -70.0];
const AUTOPILOT_AVOID_PROBE_DISTANCE: f32 = 6.0;

const AUTOPILOT_DODGE_CYCLE_HZ: f32 = 0.4;
const AUTOPILOT_DODGE_DUTY_CYCLE: f32 = 0.35;

const AUTOPILOT_DECISION_HZ: f32 = 20.0;

const AUTOPILOT_TARGET_BEHIND_PENALTY: f32 = 0.6;

fn has_line_of_sight(spatial_query: &SpatialQuery, eye: Vec3, point: Vec3) -> bool {
    let delta = point - eye;
    let distance = delta.length();
    match Dir3::new(delta) {
        Ok(direction) => spatial_query
            .cast_ray(eye, direction, (distance - AUTOPILOT_LOS_SLACK).max(0.0), true, &SpatialQueryFilter::default())
            .is_none(),
        Err(_) => true,
    }
}

pub(crate) fn autopilot_pick_target(
    time: Res<Time>,
    autopilot: Res<Autopilot>,
    look_only: Res<AutopilotLookOnly>,
    mut state: ResMut<AutopilotState>,
    player: Query<&Transform, (With<Player>, Without<NetPlayer>)>,
    camera: Query<&GlobalTransform, With<FpsCamera>>,
    zombies: Query<(Entity, &Transform), With<Zombie>>,
    spatial_query: SpatialQuery,
) {
    if !autopilot.0 && !look_only.0 {
        return;
    }
    let Ok(player_transform) = player.single() else {
        return;
    };
    state.retarget_timer -= time.delta_secs();
    let target_alive = state.target.is_some_and(|target| zombies.contains(target));
    if !target_alive || state.retarget_timer <= 0.0 {

        let forward_xz = camera
            .single()
            .map(|g| {
                let f = g.forward();
                Vec2::new(f.x, f.z).normalize_or_zero()
            })
            .unwrap_or(Vec2::Y);
        let eye = camera.single().map(|g| g.translation()).unwrap_or(player_transform.translation);
        let new_target = zombies
            .iter()
            .filter_map(|(zombie, transform)| {
                let offset = transform.translation - player_transform.translation;
                let distance = offset.length();
                if distance > AUTOPILOT_DETECT_RANGE {
                    return None;
                }
                let to_zombie_xz = Vec2::new(offset.x, offset.z).normalize_or_zero();

                let facing = forward_xz.dot(to_zombie_xz).clamp(-1.0, 1.0);

                let behind = (1.0 - facing) * 0.5;
                let weighted_distance = distance * (1.0 + behind * AUTOPILOT_TARGET_BEHIND_PENALTY);
                let occluded = !has_line_of_sight(&spatial_query, eye, transform.translation);
                let rank = if occluded { distance } else { weighted_distance };
                Some((zombie, occluded, rank))
            })
            .min_by(|(_, a_occluded, a), (_, b_occluded, b)| a_occluded.cmp(b_occluded).then(a.total_cmp(b)))
            .map(|(zombie, ..)| zombie);
        if new_target != state.target {

            let base = (ops::sin(time.elapsed_secs() * 947.0) + 1.0) * 0.5;
            state.hold_fire_timer =
                AUTOPILOT_TARGET_SWITCH_HOLD_MIN + base * (AUTOPILOT_TARGET_SWITCH_HOLD_MAX - AUTOPILOT_TARGET_SWITCH_HOLD_MIN);
            state.time_since_swap = 0.0;
        }
        state.target = new_target;
        state.retarget_timer = AUTOPILOT_RETARGET_INTERVAL;
    }
}

pub(crate) fn autopilot_think(
    time: Res<Time>,
    autopilot: Res<Autopilot>,
    look_only: Res<AutopilotLookOnly>,
    mut state: ResMut<AutopilotState>,
    mut intent: ResMut<AutopilotIntent>,
    player: Query<&Transform, (With<Player>, Without<NetPlayer>, Without<FpsCamera>)>,
    zombies: Query<(Entity, &Transform, &ZombieHitboxBones), With<Zombie>>,
    globals: Query<&GlobalTransform>,
    health: Query<&Health, With<Player>>,
    mut camera: Query<(&mut FpsCamera, &GlobalTransform)>,
    mut lean_query: Query<&mut Lean, With<FpsCamera>>,
    spatial_query: SpatialQuery,
    mut decision_timer: Local<f32>,
    mut touch_aiming: ResMut<TouchAiming>,
    archipelagos: Query<&bevy_landmass::prelude::Archipelago3d>,
    mut pathing: Query<
        (
            &mut bevy_landmass::prelude::AgentTarget3d,
            &bevy_landmass::prelude::AgentDesiredVelocity3d,
        ),
        (With<Player>, Without<NetPlayer>),
    >,
) {
    if !autopilot.0 {

        if autopilot.is_changed() && touch_aiming.0 {
            touch_aiming.0 = false;
        }
        if !look_only.0 {
            return;
        }
    }
    let Ok(player_transform) = player.single() else {
        return;
    };
    let Ok((mut cam, cam_global)) = camera.single_mut() else {
        return;
    };
    let dt = time.delta_secs();

    let Some((target_entity, target_transform, target_bones)) =
        state.target.and_then(|target| zombies.get(target).ok().map(|(e, t, b)| (e, t, b)))
    else {

        cam.yaw += AUTOPILOT_IDLE_SCAN_DEG_PER_SEC.to_radians() * dt;
        *intent = AutopilotIntent::default();
        if autopilot.0 {
            touch_aiming.0 = false;
        }
        state.aiming_region = None;
        return;
    };

    let resolved_bones = resolve_bone_world_positions(target_bones, |bone| globals.get(bone).ok().map(|g| g.translation()));
    let (aim_point, aiming_region) = random_aim_point(&resolved_bones, target_entity)
        .map_or((target_transform.translation, None), |(point, region)| (point, Some(region)));
    state.aiming_region = aiming_region;

    let camera_position = cam_global.translation();
    let to_target = aim_point - camera_position;

    let horizontal_distance = Vec2::new(to_target.x, to_target.z).length();

    let clear_to_aim_from = |eye: Vec3| -> bool { has_line_of_sight(&spatial_query, eye, aim_point) };
    let straight_los = clear_to_aim_from(camera_position);

    let cam_right = Vec3::new(ops::cos(cam.yaw), 0.0, -ops::sin(cam.yaw));
    let peek_dir = if straight_los || horizontal_distance > AUTOPILOT_PEEK_MAX_DISTANCE {
        0.0
    } else if clear_to_aim_from(camera_position + cam_right * LEAN_SIDE_OFFSET) {
        1.0
    } else if clear_to_aim_from(camera_position - cam_right * LEAN_SIDE_OFFSET) {
        -1.0
    } else {
        0.0
    };

    if autopilot.0 {
        if let Ok(mut lean) = lean_query.single_mut() {
            let step = (AUTOPILOT_LEAN_SPEED * dt).min(1.0);
            lean.current += (peek_dir - lean.current) * step;
        }
    }

    let t = (horizontal_distance / AUTOPILOT_TURN_FAR_DISTANCE).clamp(0.0, 1.0);
    let ease = (1.0 + ops::cos(t * std::f32::consts::PI)) * 0.5;
    let distance_scale = 0.15 + (1.0 - 0.15) * ease;

    state.time_since_swap += dt;
    let fade_t = (state.time_since_swap / AUTOPILOT_TURN_FADE_IN_DURATION).clamp(0.0, 1.0);
    let fade_in_scale = ops::sin(fade_t * std::f32::consts::FRAC_PI_2);
    let turn_rate_scale = distance_scale * fade_in_scale;
    let turn_step = AUTOPILOT_TURN_RATE_DEG.to_radians() * turn_rate_scale * dt;

    let blind_approach = !straight_los && peek_dir == 0.0;
    let aim_vector = if blind_approach { state.travel_dir.unwrap_or(to_target) } else { to_target };

    let desired_yaw = (-aim_vector.x).atan2(-aim_vector.z);
    let desired_pitch = if blind_approach {
        0.0
    } else {
        aim_vector.y.atan2(horizontal_distance.max(0.01)).clamp(-1.5, 1.5)
    };
    cam.yaw = turn_towards_angle(cam.yaw, desired_yaw, turn_step);
    cam.pitch = turn_towards_angle(cam.pitch, desired_pitch, turn_step);

    state.hold_fire_timer = (state.hold_fire_timer - dt).max(0.0);
    state.tap_fire_timer = (state.tap_fire_timer - dt).max(0.0);

    let raw_aim_error = (cam.yaw - desired_yaw).rem_euclid(std::f32::consts::TAU);
    let aim_error = raw_aim_error.min(std::f32::consts::TAU - raw_aim_error);
    let want_fire = state.hold_fire_timer <= 0.0
        && straight_los
        && horizontal_distance <= AUTOPILOT_ENGAGE_RANGE + 1.0
        && aim_error < AUTOPILOT_FIRE_AIM_TOLERANCE_DEG.to_radians();

    intent.fire = if want_fire && horizontal_distance > AUTOPILOT_TAP_FIRE_DISTANCE {
        if state.tap_fire_timer <= 0.0 {
            state.tap_fire_timer = AUTOPILOT_TAP_FIRE_INTERVAL;
            true
        } else {
            false
        }
    } else {
        want_fire
    };
    intent.aim = straight_los
        && horizontal_distance <= AUTOPILOT_ENGAGE_RANGE + 1.0
        && horizontal_distance > AUTOPILOT_ADS_MIN_DISTANCE;
    if autopilot.0 {
        touch_aiming.0 = intent.aim;
    }

    *decision_timer -= dt;
    if *decision_timer > 0.0 {
        return;
    }
    *decision_timer = 1.0 / AUTOPILOT_DECISION_HZ;

    let low_health = health
        .single()
        .map(|h| h.0 <= PLAYER_MAX_HEALTH * AUTOPILOT_LOW_HEALTH_FRACTION)
        .unwrap_or(false);
    let retreat_range = if low_health {
        AUTOPILOT_RETREAT_RANGE + AUTOPILOT_LOW_HEALTH_RETREAT_BONUS
    } else {
        AUTOPILOT_RETREAT_RANGE
    };

    let nearby: Vec<Vec3> = zombies
        .iter()
        .map(|(_, t, _)| t.translation)
        .filter(|p| player_transform.translation.distance(*p) <= retreat_range)
        .collect();

    let move_axis = if !nearby.is_empty() {
        let average = nearby.iter().copied().sum::<Vec3>() / nearby.len() as f32;
        let away = player_transform.translation - average;
        yaw_relative_axis(cam.yaw, away)
    } else if !straight_los && peek_dir == 0.0 {

        let toward = target_transform.translation - player_transform.translation;
        let toward = Vec3::new(toward.x, 0.0, toward.z).normalize_or_zero();
        let routed = path_direction(
            &archipelagos,
            &mut pathing,
            target_transform.translation - Vec3::Y * shared::ZOMBIE_REST_HEIGHT,
        );
        let world_dir =
            routed.unwrap_or_else(|| steer_toward(&spatial_query, camera_position, toward, &SpatialQueryFilter::default()));
        state.travel_dir = Some(world_dir);
        yaw_relative_axis(cam.yaw, world_dir)
    } else if !straight_los {

        Vec2::ZERO
    } else if horizontal_distance > AUTOPILOT_ENGAGE_RANGE {
        Vec2::new(0.0, 1.0)
    } else {

        let phase = (time.elapsed_secs() * AUTOPILOT_DODGE_CYCLE_HZ).rem_euclid(1.0);
        if phase < AUTOPILOT_DODGE_DUTY_CYCLE {
            let direction = if (time.elapsed_secs() * AUTOPILOT_DODGE_CYCLE_HZ) as i32 % 2 == 0 { 1.0 } else { -1.0 };
            Vec2::new(direction, 0.0)
        } else {
            Vec2::ZERO
        }
    };

    intent.move_axis = move_axis;
    intent.sprint = nearby.is_empty()
        && (horizontal_distance > AUTOPILOT_SPRINT_APPROACH_DISTANCE
            || (!straight_los
                && peek_dir == 0.0
                && horizontal_distance > AUTOPILOT_SPRINT_PATHING_DISTANCE));
}
fn path_direction(
    archipelagos: &Query<&bevy_landmass::prelude::Archipelago3d>,
    pathing: &mut Query<
        (
            &mut bevy_landmass::prelude::AgentTarget3d,
            &bevy_landmass::prelude::AgentDesiredVelocity3d,
        ),
        (With<Player>, Without<NetPlayer>),
    >,
    target: Vec3,
) -> Option<Vec3> {
    let snapped = archipelagos
        .single()
        .ok()
        .and_then(|archipelago| {
            archipelago
                .sample_point(target, &crate::navigation::off_mesh_target_tolerance())
                .ok()
                .map(|sampled| sampled.point())
        })
        .unwrap_or(target);
    let (mut agent_target, desired) = pathing.single_mut().ok()?;
    *agent_target = bevy_landmass::prelude::AgentTarget3d::Point(snapped);
    let velocity = desired.velocity();
    Some(Vec3::new(velocity.x, 0.0, velocity.z))
        .map(|v| v.normalize_or_zero())
        .filter(|v| *v != Vec3::ZERO)
}

fn steer_toward(spatial_query: &SpatialQuery, origin: Vec3, desired: Vec3, filter: &SpatialQueryFilter) -> Vec3 {
    let clear = |dir: Vec3| {
        Dir3::new(dir).is_ok_and(|d| {
            spatial_query
                .cast_ray(origin, d, AUTOPILOT_AVOID_PROBE_DISTANCE, true, filter)
                .is_none()
        })
    };
    if clear(desired) {
        return desired;
    }
    for angle_deg in AUTOPILOT_AVOID_ANGLES_DEG {
        let rotated = Quat::from_rotation_y(angle_deg.to_radians()) * desired;
        if clear(rotated) {
            return rotated;
        }
    }
    desired
}

fn turn_towards_angle(current: f32, target: f32, max_step: f32) -> f32 {
    let mut delta = (target - current) % std::f32::consts::TAU;
    if delta > std::f32::consts::PI {
        delta -= std::f32::consts::TAU;
    } else if delta < -std::f32::consts::PI {
        delta += std::f32::consts::TAU;
    }
    current + delta.clamp(-max_step, max_step)
}

fn yaw_relative_axis(yaw: f32, world_dir: Vec3) -> Vec2 {
    let forward = Vec3::new(-ops::sin(yaw), 0.0, -ops::cos(yaw));
    let right = Vec3::new(ops::cos(yaw), 0.0, -ops::sin(yaw));
    Vec2::new(world_dir.dot(right), world_dir.dot(forward)).normalize_or_zero()
}

// Optional AI takeover of the local player: auto-aims, auto-moves, and
// auto-fires at zombies instead of reading touch/mouse/keyboard input.
// Autopilot itself is ON by default (see Autopilot's Default impl) so
// opening the game plays itself; toggleable from the always-visible first
// settings-menu page (see cadisplaylink::native_menu::autopilot/
// set_autopilot — deliberately NOT using the debug-only bool_toggle!/
// bool_toggle_ffi! macros, since this is a real player-facing feature, not
// a debug cheat).
//
// Client-only: this module's own autopilot_pick_target/autopilot_think
// decide everything locally. (A server-side implementation — driving this
// same player's entity authoritatively, so it kept working through an AFK
// window or a dropped connection — existed at one point and was removed;
// see git history if that's ever worth resurrecting.)
//
// Design: this module NEVER duplicates gameplay logic
// (movement prediction, ammo/reload, spread/hitscan, animation) — it only
// computes a small AutopilotIntent each frame, and the systems that
// already read real input (mouse_look/touch_look, send_player_input,
// shoot) get one `if autopilot.0 { ... } else { ...existing... }` branch
// each, the same inline-branching style already used there for Noclip.
//
// Target selection mirrors the server's debug-bot AI (bot_pick_target,
// crates/server/src/main.rs) — nearest zombie in range, committed to and
// only periodically reevaluated so aim doesn't flicker between two
// similarly-close zombies. Debug bots never back away once a zombie closes
// distance (they just stop approaching); this module adds that missing
// retreat behavior, and makes it swarm-aware (backs away from the average
// position of every nearby zombie, not just the committed target) — a
// direct fix for the "fixates on one target while a horde overwhelms from
// the side" failure mode both Left 4 Dead and World War Z's companion bots
// are documented to have.

use bevy::math::ops;
use bevy::prelude::*;

use crate::{FpsCamera, Player};
use shared::{random_aim_point, resolve_bone_world_positions, Health, NetPlayer, Zombie, ZombieHitboxBones, PLAYER_MAX_HEALTH};

/// Master on/off switch — ON by default, see the module doc comment.
#[derive(Resource)]
pub(crate) struct Autopilot(pub(crate) bool);
impl Default for Autopilot {
    fn default() -> Self {
        Autopilot(true)
    }
}

/// Which zombie autopilot has committed to, mirroring bot_pick_target's
/// commit-and-periodically-reevaluate pattern so aim doesn't flicker.
#[derive(Resource, Default)]
pub(crate) struct AutopilotState {
    target: Option<Entity>,
    retarget_timer: f32,
}

/// Synthesized "virtual input" for this frame — read by send_player_input/
/// shoot in place of real touch/keyboard/mouse state while Autopilot is on.
#[derive(Resource, Default)]
pub(crate) struct AutopilotIntent {
    pub(crate) move_axis: Vec2,
    pub(crate) sprint: bool,
    pub(crate) fire: bool,
}

// Mirrors DEBUG_BOT_DETECT_RANGE (40.0, shared/src/lib.rs) — a bit tighter
// since the player (unlike a debug bot) also needs to react/retreat, not
// just approach.
const AUTOPILOT_DETECT_RANGE: f32 = 35.0;
// The "comfortable shooting distance" — a bit past DEBUG_BOT_ENGAGE_RANGE
// (10.0) since the player should shoot at range, not melee.
const AUTOPILOT_ENGAGE_RANGE: f32 = 14.0;
// New: debug bots have no equivalent of this — below this distance,
// autopilot backs away instead of just stopping.
const AUTOPILOT_RETREAT_RANGE: f32 = 5.0;
const AUTOPILOT_RETARGET_INTERVAL: f32 = 1.2;
// Rate-limited aim tracking, mirroring ZOMBIE_TURN_RATE_DEG (180.0,
// shared/src/lib.rs) used everywhere else in this codebase for the same
// "looks like tracking, not an instant aimbot snap" reasoning.
const AUTOPILOT_TURN_RATE_DEG: f32 = 360.0;
// Don't fire until aim has actually caught up to the target.
const AUTOPILOT_FIRE_AIM_TOLERANCE_DEG: f32 = 12.0;
// Radians subtracted from the computed aim pitch — see autopilot_think's
// own comment on why (apply_view_recoil kicks the actual fired direction
// up during a sustained spray; aiming a bit low to begin with keeps shots
// landing closer to center over a spray instead of climbing off-target).
// ~4.3 degrees, playtest-tunable.
const AUTOPILOT_AIM_PITCH_LOWER: f32 = 0.075;
const AUTOPILOT_SPRINT_APPROACH_DISTANCE: f32 = 22.0;
const AUTOPILOT_LOW_HEALTH_FRACTION: f32 = 0.3;
const AUTOPILOT_LOW_HEALTH_RETREAT_BONUS: f32 = 4.0;
const AUTOPILOT_IDLE_SCAN_DEG_PER_SEC: f32 = 20.0;
// How often (per second) the comfortable-band dodge cycle repeats, and what
// fraction of each cycle is spent actually stepping sideways vs. standing
// still (true Idle) — see the comfortable-band branch's own comment for why
// this can't just be a continuous sine wave.
const AUTOPILOT_DODGE_CYCLE_HZ: f32 = 0.4;
const AUTOPILOT_DODGE_DUTY_CYCLE: f32 = 0.35;
// autopilot_think ran its full decision logic (including an O(zombie count)
// scan for the swarm-retreat check) unconditionally every Update tick — up
// to 120/sec on a ProMotion iPhone — even though "should I retreat/approach/
// dodge" doesn't need re-deciding anywhere near that often. Aim tracking
// (turn_towards_angle) and the fire gate (aim error + the single already-
// committed target's distance, both O(1)) still run every frame for smooth
// camera motion and responsive shooting; only the expensive nearby-zombie
// scan + resulting move_axis/sprint decision is throttled to this rate —
// same "don't recompute faster than the result can even be acted on"
// reasoning as send_player_input's own INPUT_SEND_HZ.
const AUTOPILOT_DECISION_HZ: f32 = 20.0;

// Nearest-in-range, commit-and-reevaluate — same shape as the server's
// bot_pick_target, adapted to run purely client-side against zombies'
// already-replicated Transform (confirmed present client-side: shoot()
// builds its own predictive hitscan against this exact query shape).
pub(crate) fn autopilot_pick_target(
    time: Res<Time>,
    autopilot: Res<Autopilot>,
    mut state: ResMut<AutopilotState>,
    player: Query<&Transform, (With<Player>, Without<NetPlayer>)>,
    zombies: Query<(Entity, &Transform), With<Zombie>>,
) {
    if !autopilot.0 {
        return;
    }
    let Ok(player_transform) = player.single() else {
        return;
    };
    state.retarget_timer -= time.delta_secs();
    let target_alive = state.target.is_some_and(|target| zombies.contains(target));
    if !target_alive || state.retarget_timer <= 0.0 {
        state.target = zombies
            .iter()
            .map(|(zombie, transform)| (zombie, player_transform.translation.distance(transform.translation)))
            .filter(|&(_, distance)| distance <= AUTOPILOT_DETECT_RANGE)
            .min_by(|(_, a), (_, b)| a.total_cmp(b))
            .map(|(zombie, _)| zombie);
        state.retarget_timer = AUTOPILOT_RETARGET_INTERVAL;
    }
}

// The actual "AI": turns the committed target (if any) into aim/move/fire
// intent. Writes FpsCamera.yaw/pitch directly (the same fields mouse_look/
// touch_look would otherwise write — this module runs in their place while
// on, see their own `if autopilot.0 { return; }` guards) and
// AutopilotIntent (read by send_player_input/shoot in place of real input).
pub(crate) fn autopilot_think(
    time: Res<Time>,
    autopilot: Res<Autopilot>,
    state: Res<AutopilotState>,
    mut intent: ResMut<AutopilotIntent>,
    player: Query<&Transform, (With<Player>, Without<NetPlayer>, Without<FpsCamera>)>,
    zombies: Query<(&Transform, &ZombieHitboxBones), With<Zombie>>,
    globals: Query<&GlobalTransform>,
    health: Query<&Health, With<Player>>,
    mut camera: Query<&mut FpsCamera>,
    mut decision_timer: Local<f32>,
) {
    if !autopilot.0 {
        return;
    }
    let Ok(player_transform) = player.single() else {
        return;
    };
    let Ok(mut cam) = camera.single_mut() else {
        return;
    };
    let dt = time.delta_secs();
    let turn_step = AUTOPILOT_TURN_RATE_DEG.to_radians() * dt;

    let Some((target_entity, target_transform, target_bones)) =
        state.target.and_then(|target| zombies.get(target).ok().map(|(t, b)| (target, t, b)))
    else {
        // Nothing in range — hold position, slowly scan for something to
        // find. Zombies already actively chase within their own
        // ZOMBIE_CHASE_RANGE, so a stationary scan is enough for v1; a
        // full wander/patrol behavior is a straightforward v2 add if this
        // proves insufficient.
        cam.yaw += AUTOPILOT_IDLE_SCAN_DEG_PER_SEC.to_radians() * dt;
        *intent = AutopilotIntent::default();
        return;
    };

    // Aim tracking and the fire gate are both O(1) (the single already-
    // committed target's position, nothing to scan) and stay smooth/
    // responsive every frame regardless of the decision throttle below. See
    // random_aim_point's own doc comment (crates/shared) — biased toward
    // center-mass, headshots rarest, committed per-target via
    // entity_phase(target_entity), not re-rolled every frame. Falls back to
    // the zombie's root/capsule-center translation for the brief window
    // before its bones resolve.
    let resolved_bones = resolve_bone_world_positions(target_bones, |bone| globals.get(bone).ok().map(|g| g.translation()));
    let aim_point = random_aim_point(&resolved_bones, target_entity).unwrap_or(target_transform.translation);
    let to_target = aim_point - player_transform.translation;
    let horizontal_distance = Vec2::new(to_target.x, to_target.z).length();
    let desired_yaw = (-to_target.x).atan2(-to_target.z);
    // Nudged down a fixed amount (AUTOPILOT_AIM_PITCH_LOWER) rather than
    // dynamically compensating for apply_view_recoil's own pitch_kick —
    // recoil kicks the actual fired direction up during a sustained spray,
    // so aiming a bit lower than dead-on-target to begin with keeps shots
    // landing closer to center over a spray instead of climbing off the
    // top of the target. Simple and good enough; not trying to track the
    // exact live kick value.
    let desired_pitch = (to_target.y.atan2(horizontal_distance.max(0.01)) - AUTOPILOT_AIM_PITCH_LOWER).clamp(-1.5, 1.5);
    cam.yaw = turn_towards_angle(cam.yaw, desired_yaw, turn_step);
    cam.pitch = turn_towards_angle(cam.pitch, desired_pitch, turn_step);

    let raw_aim_error = (cam.yaw - desired_yaw).rem_euclid(std::f32::consts::TAU);
    let aim_error = raw_aim_error.min(std::f32::consts::TAU - raw_aim_error);
    intent.fire =
        horizontal_distance <= AUTOPILOT_ENGAGE_RANGE + 1.0 && aim_error < AUTOPILOT_FIRE_AIM_TOLERANCE_DEG.to_radians();

    // Everything below is the actual "what should I do about it" decision
    // — including the O(zombie count) nearby-scan for swarm-retreat — which
    // doesn't need re-deciding every single frame. Throttled to
    // AUTOPILOT_DECISION_HZ; move_axis/sprint just keep their last decided
    // value on skipped frames (still held in the `intent` resource).
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

    // Swarm-aware retreat: back away from the average position of every
    // nearby zombie, not just the committed target — the concrete fix for
    // the "ignores clusters" failure mode described in this module's own
    // doc comment.
    let nearby: Vec<Vec3> = zombies
        .iter()
        .map(|(t, _)| t.translation)
        .filter(|p| player_transform.translation.distance(*p) <= retreat_range)
        .collect();

    let move_axis = if !nearby.is_empty() {
        let average = nearby.iter().copied().sum::<Vec3>() / nearby.len() as f32;
        let away = player_transform.translation - average;
        yaw_relative_axis(cam.yaw, away)
    } else if horizontal_distance > AUTOPILOT_ENGAGE_RANGE {
        Vec2::new(0.0, 1.0) // approach — forward is toward-target since yaw tracks it
    } else {
        // Comfortable band: brief side-step dodges, not a continuous sway.
        // A plain `sin(t)` here is never exactly zero, so LocalPlayerMoving/
        // NetTransform.moving (both a bare `move_axis != Vec2::ZERO` check,
        // see send_player_input/move_players) would read "moving" every
        // single frame — driving the Walk animation nonstop and making the
        // player model's legs visibly swing even while net displacement is
        // ~0 and the player looks like it's just standing there. Instead,
        // actually stand still (Vec2::ZERO — true Idle pose) most of the
        // time, and only step sideways during a short window once per
        // cycle, alternating direction each time.
        let phase = (time.elapsed_secs() * AUTOPILOT_DODGE_CYCLE_HZ).rem_euclid(1.0);
        if phase < AUTOPILOT_DODGE_DUTY_CYCLE {
            let direction = if (time.elapsed_secs() * AUTOPILOT_DODGE_CYCLE_HZ) as i32 % 2 == 0 { 1.0 } else { -1.0 };
            Vec2::new(direction, 0.0)
        } else {
            Vec2::ZERO
        }
    };

    intent.move_axis = move_axis;
    intent.sprint = nearby.is_empty() && horizontal_distance > AUTOPILOT_SPRINT_APPROACH_DISTANCE;
}

// Shortest-path angle interpolation, rate-limited — same reasoning as the
// server's turn_towards (crates/server/src/main.rs) for zombies/debug
// bots: a rate-limited turn looks like tracking, an instant snap looks
// like an aimbot.
fn turn_towards_angle(current: f32, target: f32, max_step: f32) -> f32 {
    let mut delta = (target - current) % std::f32::consts::TAU;
    if delta > std::f32::consts::PI {
        delta -= std::f32::consts::TAU;
    } else if delta < -std::f32::consts::PI {
        delta += std::f32::consts::TAU;
    }
    current + delta.clamp(-max_step, max_step)
}

// Converts a world-space direction into the same yaw-relative (x = right,
// y = forward) axis space ClientInput.move_axis/TouchMoveAxis already use.
fn yaw_relative_axis(yaw: f32, world_dir: Vec3) -> Vec2 {
    let forward = Vec3::new(-ops::sin(yaw), 0.0, -ops::cos(yaw));
    let right = Vec3::new(ops::cos(yaw), 0.0, -ops::sin(yaw));
    Vec2::new(world_dir.dot(right), world_dir.dot(forward)).normalize_or_zero()
}

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

use avian3d::prelude::{SpatialQuery, SpatialQueryFilter};
use bevy::math::ops;
use bevy::prelude::*;

use crate::{FpsCamera, Lean, Player, TouchAiming, LEAN_SIDE_OFFSET};
use shared::{random_aim_point, resolve_bone_world_positions, Health, NetPlayer, Zombie, ZombieHitboxBones, PLAYER_MAX_HEALTH};

/// Master on/off switch — ON by default, see the module doc comment. OFF
/// by default for scenarios where an AI target-seeking, auto-firing
/// player would fight the reason that scenario exists — see
/// `crate::autopilot_enabled_by_default`'s own doc comment for the full
/// list and why (mirrors Noclip's own scenario-conditional Default,
/// lib.rs).
#[derive(Resource)]
pub(crate) struct Autopilot(pub(crate) bool);
impl Default for Autopilot {
    fn default() -> Self {
        Autopilot(crate::autopilot_enabled_by_default(crate::current_scenario()))
    }
}

/// Runs ONLY autopilot's target-selection + camera-aim tracking (turns the
/// player's view toward the nearest zombie), without the movement/sprint/
/// fire decisions the full `Autopilot` toggle also drives — added so the
/// scope-camera-not-showing-zombies investigation (native_metal_probe.rs)
/// could get a zombie reliably centered in frame without needing to
/// actually aim by hand first. `intent.move_axis`/`intent.fire` still only
/// take effect downstream while `Autopilot.0` is true (see send_player_
/// input/shoot's own `if autopilot.0 { ... }` branches), so leaving this on
/// by itself is safe to combine with real touch/mouse movement — only the
/// camera direction gets taken over, same as `Autopilot` does for aim, but
/// with none of the rest.
#[derive(Resource)]
pub(crate) struct AutopilotLookOnly(pub(crate) bool);
impl Default for AutopilotLookOnly {
    // Same env-var-launch-override pattern as OPEN_SETTINGS_MENU/
    // FORCE_ADS_ON_START — starts the camera already aimed at whatever
    // zombie is nearest, so a screenshot taken moments after launch (no
    // interactive aiming needed) reliably has one in frame.
    fn default() -> Self {
        Self(std::env::var("FORCE_LOOK_AT_TARGET").is_ok())
    }
}

/// Which zombie autopilot has committed to, mirroring bot_pick_target's
/// commit-and-periodically-reevaluate pattern so aim doesn't flicker.
#[derive(Resource, Default)]
pub(crate) struct AutopilotState {
    pub(crate) target: Option<Entity>,
    retarget_timer: f32,
    /// Which body region random_aim_point picked for the current target —
    /// purely a HUD/debug label, never read for anything
    /// gameplay-affecting.
    pub(crate) aiming_region: Option<&'static str>,
    /// Counts down after `target` changes to a different zombie (see
    /// AUTOPILOT_TARGET_SWITCH_HOLD_MIN/MAX's own doc comment) — fire is
    /// gated on this hitting 0, so autopilot pauses a beat on acquiring a
    /// new target instead of instantly spraying it the same frame it swaps.
    hold_fire_timer: f32,
    pub(crate) time_since_swap: f32,
    /// Gates fire at long range (see AUTOPILOT_TAP_FIRE_DISTANCE) so
    /// autopilot taps single shots instead of holding the trigger down —
    /// counts down to 0, at which point one more tap is allowed and it's
    /// reset to AUTOPILOT_TAP_FIRE_INTERVAL.
    tap_fire_timer: f32,
}

/// Synthesized "virtual input" for this frame — read by send_player_input/
/// shoot in place of real touch/keyboard/mouse state while Autopilot is on.
#[derive(Resource, Default)]
pub(crate) struct AutopilotIntent {
    pub(crate) move_axis: Vec2,
    pub(crate) sprint: bool,
    pub(crate) fire: bool,
    pub(crate) aim: bool,
}

// Mirrors DEBUG_BOT_DETECT_RANGE (40.0, shared/src/lib.rs) — a bit tighter
// since the player (unlike a debug bot) also needs to react/retreat, not
// just approach.
const AUTOPILOT_DETECT_RANGE: f32 = 80.0;
// The "comfortable shooting distance" — a bit past DEBUG_BOT_ENGAGE_RANGE
// (10.0) since the player should shoot at range, not melee. Was 28.0.
const AUTOPILOT_ENGAGE_RANGE: f32 = 40.0;
// New: debug bots have no equivalent of this — below this distance,
// autopilot backs away instead of just stopping.
const AUTOPILOT_RETREAT_RANGE: f32 = 5.0;
const AUTOPILOT_RETARGET_INTERVAL: f32 = 1.2;
// Below this distance autopilot doesn't bother raising ADS — target's
// close enough hip-fire is fine.
const AUTOPILOT_ADS_MIN_DISTANCE: f32 = 25.0;
// A random pause (rolled once per target switch — see AutopilotState's own
// hold_fire_timer doc comment) before firing resumes on the new target, so
// autopilot doesn't keep spraying uninterrupted the instant it swaps to a
// different zombie — reads as a brief "target acquired" beat instead of a
// seamless aimbot hand-off.
const AUTOPILOT_TARGET_SWITCH_HOLD_MIN: f32 = 0.1;
const AUTOPILOT_TARGET_SWITCH_HOLD_MAX: f32 = 0.3;
// Beyond this range, spray (holding fire every eligible frame) climbs
// bullet spread (current_bullet_spread's "heat" from ViewRecoil.pitch_kick,
// lib.rs) faster than it can decay between shots at FIRE_RATE, so distant
// shots keep missing wider. Autopilot instead taps single shots at this
// range — see AUTOPILOT_TAP_FIRE_INTERVAL below — giving pitch_kick time to
// decay (RECOIL_RECOVERY_SPEED, lib.rs) between each one.
// Was 18.0 — raised so only genuinely far shots (30m+, out of
// AUTOPILOT_ENGAGE_RANGE's 40.0 comfortable-range) get tapped; closer
// shots stay full-auto spray.
const AUTOPILOT_TAP_FIRE_DISTANCE: f32 = 20.0;
// Gap enforced between taps at long range. RECOIL_RECOVERY_SPEED (5.0/sec)
// decays pitch_kick exponentially, so this gap lets most of a shot's kick
// bleed off before the next one instead of stacking on top of it. Was
// 0.35, then 0.22, then 0.14, then widened to 0.4 — tightened back down
// for a faster tap rate again.
const AUTOPILOT_TAP_FIRE_INTERVAL: f32 = 0.15;
// Rate-limited aim tracking — was 180.0, matching ZOMBIE_TURN_RATE_DEG
// (shared/src/lib.rs), but bumped faster for snappier target acquisition;
// the fade-in/distance easing below still keeps it from reading as an
// instant aimbot snap.
const AUTOPILOT_TURN_RATE_DEG: f32 = 260.0;
// Distance (m) at which the turn-rate distance-curve bottoms out at its
// slowest — widened from 40.0 so the fast→slow transition is spread over
// more distance (gentler falloff per meter) instead of compressed close in.
const AUTOPILOT_TURN_FAR_DISTANCE: f32 = 55.0;
// How long after acquiring a new target the turn rate takes to fade up to
// full (distance-curve) speed, instead of snapping to it the instant a
// target is committed — an ease-out sine (fast at first, smoothly
// decelerating as it nears full speed, never a hard stop) so the very
// first frames of tracking a freshly-swapped target read as a deliberate
// camera settling in, not an instant snap.
const AUTOPILOT_TURN_FADE_IN_DURATION: f32 = 0.4;
// Don't fire until aim has actually caught up to the target.
const AUTOPILOT_FIRE_AIM_TOLERANCE_DEG: f32 = 12.0;
const AUTOPILOT_SPRINT_APPROACH_DISTANCE: f32 = 22.0;
const AUTOPILOT_LOW_HEALTH_FRACTION: f32 = 0.3;
const AUTOPILOT_LOW_HEALTH_RETREAT_BONUS: f32 = 4.0;
const AUTOPILOT_IDLE_SCAN_DEG_PER_SEC: f32 = 20.0;
// Line-of-sight: a hit this much before the aim point still counts as
// "clear" (the aim point sits just inside the zombie mesh, so the ray
// naturally ends a hair short).
const AUTOPILOT_LOS_SLACK: f32 = 0.3;
// Only bother trying to lean around cover when the blocked target is at
// least this close — farther than this, repositioning is the better play.
const AUTOPILOT_PEEK_MAX_DISTANCE: f32 = 32.0;
// How fast autopilot eases its lean toward / away from a peek (per second,
// 0..1 units — matches LEAN_SPEED's feel).
const AUTOPILOT_LEAN_SPEED: f32 = 6.0;
// Fan of headings tried when steering toward an occluded target — degrees
// off the straight line to it. Mirrors the server's ZOMBIE_AVOID_ANGLES_DEG.
const AUTOPILOT_AVOID_ANGLES_DEG: [f32; 6] = [22.0, -22.0, 45.0, -45.0, 70.0, -70.0];
const AUTOPILOT_AVOID_PROBE_DISTANCE: f32 = 6.0;
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
// Target selection weights raw distance by how far off-camera-forward a
// zombie is, so a slightly-farther zombie already in front of the camera
// beats a slightly-closer one behind — without this, two similarly-close
// zombies on opposite sides flip the target (and the camera) 180 degrees
// back and forth every retarget. 0.0 = pure nearest-distance (no bias),
// 1.0 = a directly-behind zombie's effective distance is doubled.
const AUTOPILOT_TARGET_BEHIND_PENALTY: f32 = 0.6;

// Nearest-in-range, commit-and-reevaluate — same shape as the server's
// bot_pick_target, adapted to run purely client-side against zombies'
// already-replicated Transform (confirmed present client-side: shoot()
// builds its own predictive hitscan against this exact query shape).
pub(crate) fn autopilot_pick_target(
    time: Res<Time>,
    autopilot: Res<Autopilot>,
    look_only: Res<AutopilotLookOnly>,
    mut state: ResMut<AutopilotState>,
    player: Query<&Transform, (With<Player>, Without<NetPlayer>)>,
    camera: Query<&GlobalTransform, With<FpsCamera>>,
    zombies: Query<(Entity, &Transform), With<Zombie>>,
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
        // Horizontal-only (yaw plane) — pitch doesn't matter for "is this
        // behind the camera", and matches how turn_step/desired_yaw below
        // treat direction elsewhere in this module.
        let forward_xz = camera
            .single()
            .map(|g| {
                let f = g.forward();
                Vec2::new(f.x, f.z).normalize_or_zero()
            })
            .unwrap_or(Vec2::Y);
        let new_target = zombies
            .iter()
            .filter_map(|(zombie, transform)| {
                let offset = transform.translation - player_transform.translation;
                let distance = offset.length();
                if distance > AUTOPILOT_DETECT_RANGE {
                    return None;
                }
                let to_zombie_xz = Vec2::new(offset.x, offset.z).normalize_or_zero();
                // 1.0 directly in front, -1.0 directly behind.
                let facing = forward_xz.dot(to_zombie_xz).clamp(-1.0, 1.0);
                // 0.0 in front .. 1.0 behind.
                let behind = (1.0 - facing) * 0.5;
                let weighted_distance = distance * (1.0 + behind * AUTOPILOT_TARGET_BEHIND_PENALTY);
                Some((zombie, weighted_distance))
            })
            .min_by(|(_, a), (_, b)| a.total_cmp(b))
            .map(|(zombie, _)| zombie);
        if new_target != state.target {
            // Rolled once here, not re-rolled every frame the hold is
            // active — a single random duration in
            // [AUTOPILOT_TARGET_SWITCH_HOLD_MIN, _MAX), same sin-based
            // "randomness" used elsewhere in this codebase (no rand crate).
            let base = (ops::sin(time.elapsed_secs() * 947.0) + 1.0) * 0.5;
            state.hold_fire_timer =
                AUTOPILOT_TARGET_SWITCH_HOLD_MIN + base * (AUTOPILOT_TARGET_SWITCH_HOLD_MAX - AUTOPILOT_TARGET_SWITCH_HOLD_MIN);
            state.time_since_swap = 0.0;
        }
        state.target = new_target;
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
) {
    if !autopilot.0 {
        // Only clear on the frame autopilot actually turns off (not every
        // frame it stays off) — this used to unconditionally stomp
        // touch_aiming.0 to false every single frame while off, which
        // fought the real touch ADS button and made it look broken (it
        // never got to stay true past the next frame). is_changed() is
        // only true on the transition, so a genuine player-driven ADS
        // tap afterward is left alone.
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
        // Nothing in range — hold position, slowly scan for something to
        // find. Zombies already actively chase within their own
        // ZOMBIE_CHASE_RANGE, so a stationary scan is enough for v1; a
        // full wander/patrol behavior is a straightforward v2 add if this
        // proves insufficient.
        cam.yaw += AUTOPILOT_IDLE_SCAN_DEG_PER_SEC.to_radians() * dt;
        *intent = AutopilotIntent::default();
        if autopilot.0 {
            touch_aiming.0 = false;
        }
        state.aiming_region = None;
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
    let (aim_point, aiming_region) = random_aim_point(&resolved_bones, target_entity)
        .map_or((target_transform.translation, None), |(point, region)| (point, Some(region)));
    state.aiming_region = aiming_region;
    
    let camera_position = cam_global.translation();
    let to_target = aim_point - camera_position;
    // horizontal_distance (used below for both the pitch angle and the fire-
    // range gate) is deliberately measured off the TRUE target, not the
    // wobbled one — the wobble should sway the aim, not flicker the engage-
    // range/fire gate in and out near a boundary.
    let horizontal_distance = Vec2::new(to_target.x, to_target.z).length();

    // --- Line of sight to the target ----------------------------------------
    // A ray from the eye to the aim point that stops on level geometry before
    // reaching it means the zombie is behind cover. Zombies are excluded so
    // their own colliders never read as the blocker; walls and the parked
    // cars do.
    let occ_filter = SpatialQueryFilter::default().with_excluded_entities(zombies.iter().map(|(e, _, _)| e));
    let clear_to_aim_from = |eye: Vec3| -> bool {
        let delta = aim_point - eye;
        let dist = delta.length();
        match Dir3::new(delta) {
            Ok(dir) => spatial_query
                .cast_ray(eye, dir, (dist - AUTOPILOT_LOS_SLACK).max(0.0), true, &occ_filter)
                .is_none(),
            Err(_) => true,
        }
    };
    let straight_los = clear_to_aim_from(camera_position);

    // Corner peek: straight line blocked, but a ~shoulder-width sideways step
    // would clear it — lean that way so a shot opens up before fully rounding
    // the corner. Only worth it fairly close; farther out, repositioning wins.
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
    // Autopilot owns the lean while fully on (apply_lean_input early-returns
    // then), same as it owns yaw/pitch — ease toward the peek or back upright.
    // Once leaned, sync_camera_to_player shifts the eye, so next frame's
    // `straight_los` naturally flips true from the peek position and the fire
    // gate below opens.
    if autopilot.0 {
        if let Ok(mut lean) = lean_query.single_mut() {
            let step = (AUTOPILOT_LEAN_SPEED * dt).min(1.0);
            lean.current += (peek_dir - lean.current) * step;
        }
    }

    // Smooth cosine S-curve interpolation between 1.0 (close) and 0.15 (far,
    // >=AUTOPILOT_TURN_FAR_DISTANCE).
    let t = (horizontal_distance / AUTOPILOT_TURN_FAR_DISTANCE).clamp(0.0, 1.0);
    let ease = (1.0 + ops::cos(t * std::f32::consts::PI)) * 0.5;
    let distance_scale = 0.15 + (1.0 - 0.15) * ease;
    // Fade the turn rate in over the first AUTOPILOT_TURN_FADE_IN_DURATION
    // seconds after a target swap (see its own comment) — sine ease-out, so
    // it ramps up quickly at first then settles smoothly into full speed
    // rather than snapping straight to the distance curve above.
    state.time_since_swap += dt;
    let fade_t = (state.time_since_swap / AUTOPILOT_TURN_FADE_IN_DURATION).clamp(0.0, 1.0);
    let fade_in_scale = ops::sin(fade_t * std::f32::consts::FRAC_PI_2);
    let turn_rate_scale = distance_scale * fade_in_scale;
    let turn_step = AUTOPILOT_TURN_RATE_DEG.to_radians() * turn_rate_scale * dt;

    let wobbled_target = to_target;

    let desired_yaw = (-wobbled_target.x).atan2(-wobbled_target.z);
    let desired_pitch = wobbled_target.y.atan2(horizontal_distance.max(0.01)).clamp(-1.5, 1.5);
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
    // Far enough that spraying would climb spread faster than it decays —
    // tap instead of holding the trigger down (see AUTOPILOT_TAP_FIRE_
    // DISTANCE's own comment).
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
    intent.aim = horizontal_distance <= AUTOPILOT_ENGAGE_RANGE + 1.0 && horizontal_distance > AUTOPILOT_ADS_MIN_DISTANCE;
    if autopilot.0 {
        touch_aiming.0 = intent.aim;
    }

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
        .map(|(_, t, _)| t.translation)
        .filter(|p| player_transform.translation.distance(*p) <= retreat_range)
        .collect();

    let move_axis = if !nearby.is_empty() {
        let average = nearby.iter().copied().sum::<Vec3>() / nearby.len() as f32;
        let away = player_transform.translation - average;
        yaw_relative_axis(cam.yaw, away)
    } else if !straight_los && peek_dir == 0.0 {
        // Target behind cover and no quick peek available — close in on it,
        // steering the heading around whatever's blocking (walls, cars)
        // instead of walking face-first into it.
        let toward = target_transform.translation - player_transform.translation;
        let toward = Vec3::new(toward.x, 0.0, toward.z).normalize_or_zero();
        yaw_relative_axis(cam.yaw, steer_toward(&spatial_query, camera_position, toward, &occ_filter))
    } else if !straight_los {
        // Peeking around a corner — hold position, let the lean do the work.
        Vec2::ZERO
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
    intent.sprint = nearby.is_empty()
        && (horizontal_distance > AUTOPILOT_SPRINT_APPROACH_DISTANCE
            || (!straight_los && peek_dir == 0.0 && horizontal_distance > AUTOPILOT_RETREAT_RANGE));
}

// Raycast fan toward an occluded target: return the straight-line direction
// if the way is clear, otherwise the first angled heading (see
// AUTOPILOT_AVOID_ANGLES_DEG) that is. Mirrors the server's
// steer_around_obstacles for chase-AI zombies.
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

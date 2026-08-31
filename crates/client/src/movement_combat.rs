// --- Movement, camera, and combat pipeline ---
// Extracted from lib.rs (Phase 3 of the gameplay-code-organization pass —
// see chat/commit history). Every item here is `pub(crate)` so lib.rs's
// single master `.chain()`'d Update system list (unchanged — pure code
// motion, no execution-order change) can still reference these by name.
//
// `use crate::*;` pulls in every crate-root item this module needs
// (constants, other components/resources, helper fns) — root-private
// items ARE visible to a descendant module like this one per Rust's
// normal privacy rules, just not name-resolved without an explicit
// `use`/path, hence the glob rather than hunting down each one by hand.

use bevy::prelude::*;
use crate::*;

#[derive(Component)]
pub(crate) struct FpsCamera {
    pub(crate) yaw: f32,
    pub(crate) pitch: f32,
}

// Which of the two cameras spawned in setup() is actually rendering —
// FpsCamera stays the single source of look/aim truth in both modes (see
// this enum's own toggle_camera_mode/sync_third_person_camera), so shooting,
// the crosshair, and the gun viewmodel all keep working unmodified: only the
// *visual* camera swaps, not the aim math.
#[derive(Resource, Default, PartialEq, Eq, Clone, Copy)]
pub(crate) enum CameraMode {
    #[default]
    FirstPerson,
    ThirdPerson,
}

// Marks the third-person spectator camera spawned in setup() alongside the
// real FpsCamera — see CameraMode's own doc comment for why this is a
// separate entity rather than moving FpsCamera itself.
#[derive(Component)]
pub(crate) struct ThirdPersonCamera;

// Shared by shoot(), move_player_physics, and apply_aim_zoom so "aiming"
// means the same thing everywhere: cursor locked (actually playing, not
// paused/menu'd out) and right mouse button held — or, on touch, the
// TouchAimButton toggle (see touch_aim_button) is on. There's no touch
// equivalent of "cursor grab" gating this the way there is for mouse, since
// touch has no OS-level pointer lock concept — a touch player is just always
// considered "playing" for this check's purposes.
pub(crate) fn is_aiming(cursor: &CursorOptions, mouse: &ButtonInput<MouseButton>, touch_aiming: bool) -> bool {
    (cursor.grab_mode != CursorGrabMode::None && mouse.pressed(MouseButton::Right)) || touch_aiming
}

// Shared by shoot() (to actually offset the shot) and update_crosshair (to
// visually reflect it) so the reticle always shows the real spread the next
// shot would use — same recoil-heat/aim/stationary formula, just factored
// out so both stay in sync automatically instead of two copies drifting
// apart.
pub(crate) fn current_bullet_spread(pitch_kick: f32, aiming: bool, moving: bool, stance: PlayerStance, sprinting: bool) -> f32 {
    let heat = pitch_kick / RECOIL_PITCH_KICK_MAX;
    let mut spread = BULLET_SPREAD * (0.3 + heat);
    if aiming {
        spread *= AIM_SPREAD_MULTIPLIER;
    }
    if !moving {
        spread *= STATIONARY_SPREAD_MULTIPLIER;
    }
    // Sprinting is the least stable stance regardless of what PlayerStance
    // says (and move_player_physics only allows sprinting while Standing
    // anyway) — otherwise Prone is the most stable, Crouching in between.
    if sprinting {
        spread *= SPRINT_SPREAD_MULTIPLIER;
    } else {
        spread *= match stance {
            PlayerStance::Standing => 1.0,
            PlayerStance::Crouching => CROUCH_SPREAD_MULTIPLIER,
            PlayerStance::Prone => PRONE_SPREAD_MULTIPLIER,
        };
    }
    spread
}

#[derive(Component, Default)]
pub(crate) struct ViewRecoil {
    pub(crate) pitch_kick: f32,
    pub(crate) yaw_kick: f32,
}

// -1.0 (full lean left) .. 1.0 (full lean right), eased toward whichever of
// Q/E is held (or back to 0.0 if neither/both are) by apply_lean_input.
// apply_view_recoil turns this into camera roll, sync_camera_to_player turns
// it into a sideways offset.
#[derive(Component, Default)]
pub(crate) struct Lean {
    // Read/written by autopilot_think for corner-peek — hence pub(crate).
    pub(crate) current: f32,
}

// Debug/dev flight mode: toggled by KeyN. Sent to the server as part of
// ClientInput (see its own doc comment) and simulated by the shared
// simulate_player_step on both sides — bypasses MoveAndSlide (and thus
// gravity/collision) entirely, so the player can fly through walls to
// inspect the level. Not reset by restart_game (deliberately — no reason to
// drop out of noclip just because the player died).
#[derive(Resource, Default)]
pub(crate) struct Noclip(pub(crate) bool);

// Debug/dev toggles for the Cheats panel's "Gun"/"MSAA"/"Shadow" row — hide
// the viewmodel gun, or A/B the map's render cost the same way the
// CADisplayLink probe's own MSAA/SHADOW buttons do, just wired to the real
// game's own camera/sun instead of the probe's.
#[derive(Resource)]
pub(crate) struct GunVisible(pub(crate) bool);

// Client-side prediction + reconciliation for the client's pre-existing
// local Player entity (see setup()) — still what zombie AI/UI/damage all
// target, so this remains the one link back to it. Every frame: start from
// the server's last confirmed position (our own replicated NetPlayer's
// NetTransform), drop whichever buffered inputs the server has now
// confirmed (LastProcessedInput), then replay everything still
// unconfirmed through the exact same movement step the server itself
// runs (shared::movement::simulate_player_step) — including the input
// send_player_input just queued this frame, so movement is instant instead
// of waiting a full round trip.
//
// Known simplification: only translation is resynced from the server;
// vertical_velocity/grounded keep evolving from whatever this client last
// had locally rather than also being replicated and corrected. Good enough
// for ground movement (which is what player-vs-player visible drift would
// mostly show up as); a mispredicted jump/fall can drift slightly from the
// server's version until landing resets grounded.
// Reconciliation: only runs when LastProcessedInput actually advances (the
// server confirming a new input), not every render frame. Popping confirmed
// entries recovers the exact vertical_velocity/grounded the local
// simulation had right after applying the last-confirmed input (stashed in
// PredictedInputs by send_player_input) — resuming from that exact state,
// rather than whatever was left over from a previous reconciliation pass,
// is what avoids double-integrating gravity into vertical_velocity every
// frame. That old per-frame reset-and-replay-everything approach is exactly
// what caused the jump stutter this replaced: pending inputs stayed queued
// for several frames (RTT-sized), and each of those frames re-simulated
// the whole queue on top of already-advanced velocity instead of a
// consistent baseline, so gravity kept compounding until it visibly
// snapped/jittered.
pub(crate) fn predict_player_movement(
    local: Query<
        (&NetTransform, &LastProcessedInput),
        (With<NetPlayer>, With<LocalPlayer>, Without<Player>, Changed<LastProcessedInput>),
    >,
    mut player: Query<(&mut Transform, &mut Player, &PlayerStance), Without<NetPlayer>>,
    mut pending: ResMut<PredictedInputs>,
    mut smooth: ResMut<PredictionSmooth>,
    mover: MoveAndSlide,
) {
    let Ok((net_transform, last_processed)) = local.single() else {
        return;
    };
    let Ok((mut transform, mut player, stance)) = player.single_mut() else {
        return;
    };
    let stance = *stance;

    // Drop every input the server has now confirmed; the last one's trailing
    // (vertical_velocity, grounded) is the authoritative baseline to replay from.
    let mut baseline = None;
    while pending.0.front().is_some_and(|&(sequence, ..)| sequence <= last_processed.0) {
        let (_, _, _, vv, grounded) = pending.0.pop_front().unwrap();
        baseline = Some((vv, grounded));
    }
    let Some((base_vv, base_grounded)) = baseline else {
        return; // nothing new confirmed — leave the local prediction alone
    };

    // Full "snap to the server position + replay unconfirmed inputs" result,
    // computed on a scratch copy. The client sims per render frame while the
    // server sims once per 30Hz tick, so MoveAndSlide non-linearity leaves a
    // steady few-cm gap even with no real error — hard-snapping to it every
    // reconcile is the regular backward strafe jerk. Only commit the snap
    // past RECONCILE_POS_THRESHOLD; below that, trust the local prediction.
    let mut scratch_tf = *transform;
    scratch_tf.translation = net_transform.translation;
    let mut scratch_player = Player { vertical_velocity: base_vv, grounded: base_grounded };
    for (_, input, dt, ..) in &pending.0 {
        shared::movement::simulate_player_step(&mover, &mut scratch_tf, &mut scratch_player, stance, input, *dt);
    }

    if transform.translation.distance(scratch_tf.translation) > RECONCILE_POS_THRESHOLD {
        let pre = transform.translation;
        *transform = scratch_tf;
        player.vertical_velocity = scratch_player.vertical_velocity;
        player.grounded = scratch_player.grounded;
        // sync_camera_to_player (runs right after, same frame) reads
        // transform + smooth.offset, so camera == pre this frame; then
        // smooth.offset decays to 0 and the camera glides into the snap.
        smooth.offset += pre - transform.translation;
    }
}

// Any zombie actively chasing (has Chase) and within melee range continuously
// drains player health — no attack animation/cooldown, same simplicity
// tradeoff as the zombies' own one-shot-kill damage model in shoot().
pub(crate) fn zombies_damage_player(
    time: Res<Time>,
    game_over: Res<GameOver>,
    freeze: Res<FreezeZombies>,
    god_mode: Res<GodMode>,
    // Chase is server-only now (big-brain runs server-side) — this just
    // checks distance instead of requiring the zombie to also carry a
    // locally-nonexistent Chase marker.
    mut zombies: Query<(&Transform, &mut ZombieAttackTimer), With<Zombie>>,
    mut player: Query<(&Transform, &mut Health), With<Player>>,
    mut elapsed: Local<f32>,
) {
    if game_over.0 || freeze.0 || god_mode.0 {
        return;
    }
    // ZombieAttackTimer counts down in real wall-clock seconds, so unlike
    // update_zombie_lod's pure distance check, throttling this can't just
    // skip frames — it has to pass the FULL accumulated dt since the last
    // time this actually ran, not a single frame's delta, or the attack
    // cooldown would tick down slower than real time and damage would
    // land later than ZOMBIE_ATTACK_INTERVAL actually specifies.
    *elapsed += time.delta_secs();
    if *elapsed < 1.0 / INPUT_SEND_HZ {
        return;
    }
    let dt = *elapsed;
    *elapsed = 0.0;
    let Ok((player_transform, mut health)) = player.single_mut() else {
        return;
    };
    for (zombie_transform, mut attack_timer) in &mut zombies {
        let in_range = zombie_transform.translation.distance(player_transform.translation) <= ZOMBIE_STOP_DISTANCE;
        if !in_range {
            continue;
        }
        attack_timer.0 -= dt;
        if attack_timer.0 <= 0.0 {
            health.0 -= ZOMBIE_ATTACK_DAMAGE;
            attack_timer.0 = ZOMBIE_ATTACK_INTERVAL;
        }
    }
}

// Click into the window to lock/hide the cursor for mouse-look; Esc releases it.
pub(crate) fn grab_cursor(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut cursor: Query<&mut CursorOptions, With<PrimaryWindow>>,
    pending: Res<MapSelectionPending>,
    menu: Res<SettingsMenuVisible>,
    // `Option` because this resource only exists when EDITOR=1 actually
    // added bevy_editor_pls::EditorPlugin (see main()) — the client always
    // depends on the crate, but the plugin/resource is opt-in at runtime.
    editor: Option<Res<bevy_editor_pls::editor::Editor>>,
) {
    let Ok(mut cursor) = cursor.single_mut() else {
        return;
    };

    // Toggle cursor grab on backtick (`) key press
    if keys.just_pressed(KeyCode::Backquote) {
        if cursor.grab_mode == CursorGrabMode::None {
            cursor.grab_mode = CursorGrabMode::Locked;
            cursor.visible = false;
        } else {
            cursor.grab_mode = CursorGrabMode::None;
            cursor.visible = true;
        }
    }

    // While the dev editor (EDITOR=1, F2) is active, clicking its egui
    // panels (Hierarchy/Inspector/viewport gizmos/...) used to also count
    // as "click into the game window" below and re-grab+hide the cursor
    // mid-click — fighting the editor's own pointer handling and making it
    // feel like every click "swallows" the mouse. Suppress the game's own
    // auto-grab entirely while the editor is active; F2 to close it (or `)
    // still work to get mouse-look back.
    let editor_active = editor.is_some_and(|e| e.active());

    // Also cover the case where F2 opens the editor while the cursor was
    // already grabbed from mouse-look — force it back to a free, visible
    // cursor immediately rather than waiting for a manual ` press.
    if editor_active && cursor.grab_mode != CursorGrabMode::None {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    }

    // Left click only grabs the cursor if no menu is open/pending
    if mouse.just_pressed(MouseButton::Left)
        && cursor.grab_mode == CursorGrabMode::None
        && !pending.0
        && !menu.0
        && !editor_active
    {
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
    }

    // Escape releases it
    if keys.just_pressed(KeyCode::Escape) {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    }
}

// Updates the player's aim (yaw/pitch) from mouse movement. Doesn't touch the
// Transform directly — apply_view_recoil composes the final rotation from
// this aim plus the current recoil kick, every frame.
pub(crate) fn mouse_look(
    cursor: Query<&CursorOptions, With<PrimaryWindow>>,
    mouse: Res<ButtonInput<MouseButton>>,
    touch_aiming: Res<TouchAiming>,
    zoom: Res<ScopeZoom>,
    mut motion: MessageReader<MouseMotion>,
    mut look_delta: ResMut<MouseLookDelta>,
    sensitivity_scale: Res<SensitivityScale>,
    mut query: Query<&mut FpsCamera>,
    autopilot: Res<Autopilot>,
    look_only: Res<AutopilotLookOnly>,
) {
    // Real look input is ignored the whole time Autopilot (or its look-only
    // subset, AutopilotLookOnly) is on (see autopilot's own module doc
    // comment) — autopilot_think already wrote FpsCamera.yaw/pitch this
    // same tick.
    if autopilot.0 || look_only.0 {
        return;
    }
    let Ok(cursor) = cursor.single() else {
        look_delta.0 = Vec2::ZERO;
        return;
    };
    if cursor.grab_mode == CursorGrabMode::None {
        motion.clear();
        look_delta.0 = Vec2::ZERO;
        return;
    }

    let delta: Vec2 = motion.read().map(|e| e.delta).sum();
    look_delta.0 = delta;
    if delta == Vec2::ZERO {
        return;
    }

    // AIM_MOUSE_SENSITIVITY_MULTIPLIER is calibrated at SCOPE_DEFAULT_ZOOM;
    // scaled further by how far the current scope zoom (adjustable via
    // scroll wheel, see adjust_scope_zoom) is from that baseline, so a
    // given mouse move covers the same angular slice of the zoomed view no
    // matter the magnification — otherwise the same flat multiplier would
    // feel twitchy at max zoom and sluggish at min zoom.
    let sensitivity = if is_aiming(cursor, &mouse, touch_aiming.0) {
        MOUSE_SENSITIVITY * AIM_MOUSE_SENSITIVITY_MULTIPLIER * (SCOPE_DEFAULT_ZOOM / zoom.0)
    } else {
        MOUSE_SENSITIVITY
    } * sensitivity_scale.0;
    for mut cam in &mut query {
        cam.yaw -= delta.x * sensitivity;
        cam.pitch = (cam.pitch - delta.y * sensitivity).clamp(-PITCH_CLAMP, PITCH_CLAMP);
    }
}

// Composes the camera's final rotation from the mouse-driven aim plus the
// current recoil kick, and lets the kick spring back down over time.
pub(crate) fn apply_view_recoil(
    time: Res<Time>,
    mut query: Query<(&mut Transform, &FpsCamera, &mut ViewRecoil, &Lean)>,
) {
    for (mut transform, cam, mut recoil, lean) in &mut query {
        let decay = (RECOIL_RECOVERY_SPEED * time.delta_secs()).min(1.0);
        recoil.pitch_kick *= 1.0 - decay;
        recoil.yaw_kick *= 1.0 - decay;

        let pitch = (cam.pitch + recoil.pitch_kick).clamp(-1.54, 1.54);
        let yaw = cam.yaw + recoil.yaw_kick;
        // Negated: a positive Z rotation in Bevy's right-handed coords rolls
        // the camera the opposite way from what "lean left tilts view left"
        // intuitively expects (confirmed backwards when actually tested).
        let roll = -LEAN_ROLL_DEG.to_radians() * lean.current;
        transform.rotation = Quat::from_euler(EulerRot::YXZ, yaw, pitch, roll);
    }
}

// Q leans left, E leans right; releasing both (or holding both at once)
// eases back to upright. Gated on !game_over like the other player-facing
// input systems.
pub(crate) fn apply_lean_input(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    touch_lean: Res<TouchLeanHeld>,
    game_over: Res<GameOver>,
    autopilot: Res<Autopilot>,
    mut query: Query<&mut Lean>,
) {
    // While fully on, autopilot drives Lean.current directly (corner-peek —
    // see autopilot_think), same hands-off deal as mouse_look/touch_look.
    if autopilot.0 {
        return;
    }
    let target: f32 = if game_over.0 {
        0.0
    } else {
        let left = keys.pressed(KeyCode::KeyQ) || touch_lean.left;
        let right = keys.pressed(KeyCode::KeyE) || touch_lean.right;
        match (left, right) {
            (true, false) => -1.0,
            (false, true) => 1.0,
            _ => 0.0,
        }
    };
    for mut lean in &mut query {
        // Plain proportional lerp — reacts immediately to input, no routing
        // through center. The "fast on reversal" complaint turned out to be
        // about the gun mesh's shoulder swap, not this camera roll value;
        // see gun_view_animation's own shoulder_blend for that fix instead.
        let t = (LEAN_SPEED * time.delta_secs()).min(1.0);
        lean.current += (target - lean.current) * t;
    }
}

// Lerps the camera's FOV toward a zoomed-in value while aiming, and back to
// BASE_FOV otherwise — same lerp-toward-target shape as apply_view_recoil's
// decay, just easing up instead of down.
pub(crate) fn apply_aim_zoom(
    time: Res<Time>,
    mouse: Res<ButtonInput<MouseButton>>,
    touch_aiming: Res<TouchAiming>,
    cursor: Query<&CursorOptions, With<PrimaryWindow>>,
    mut camera: Query<&mut Projection, With<FpsCamera>>,
) {
    let Ok(cursor) = cursor.single() else {
        return;
    };
    let target_fov = if is_aiming(cursor, &mouse, touch_aiming.0) {
        BASE_FOV * AIM_FOV_MULTIPLIER
    } else {
        BASE_FOV
    };

    let Ok(mut projection) = camera.single_mut() else {
        return;
    };
    let Projection::Perspective(perspective) = projection.as_mut() else {
        return;
    };
    let t = (AIM_ZOOM_SPEED * time.delta_secs()).min(1.0);
    perspective.fov += (target_fov - perspective.fov) * t;
}

// Blurs the background while aiming (see AIM_DOF_FOCAL_DISTANCE) by
// inserting/removing DepthOfField on the main camera outright, rather than
// leaving it always attached — no blur at all outside ADS.
pub(crate) fn apply_aim_dof(
    mut commands: Commands,
    mouse: Res<ButtonInput<MouseButton>>,
    touch_aiming: Res<TouchAiming>,
    aim_blur: Res<AimBlurEnabled>,
    dof_focal_alt: Res<AimDofFocalDistanceAlt>,
    cursor: Query<&CursorOptions, With<PrimaryWindow>>,
    camera: Query<Entity, With<FpsCamera>>,
) {
    let (Ok(cursor), Ok(camera_entity)) = (cursor.single(), camera.single()) else {
        return;
    };
    if aim_blur.0 && is_aiming(cursor, &mouse, touch_aiming.0) {
        let focal_distance = if dof_focal_alt.0 {
            AIM_DOF_FOCAL_DISTANCE_B
        } else {
            AIM_DOF_FOCAL_DISTANCE_A
        };
        commands.entity(camera_entity).insert(DepthOfField {
            focal_distance,
            aperture_f_stops: AIM_DOF_APERTURE_F_STOPS,
            mode: DepthOfFieldMode::Gaussian,
            ..default()
        });
    } else {
        commands.entity(camera_entity).remove::<DepthOfField>();
    }
}

// Toggles Noclip on KeyN — separate system from send_player_input so the
// toggle itself still works even while game_over freezes real input sending.
pub(crate) fn toggle_noclip(keys: Res<ButtonInput<KeyCode>>, mut noclip: ResMut<Noclip>) {
    if keys.just_pressed(KeyCode::KeyN) {
        noclip.0 = !noclip.0;
    }
}

// Toggles CameraMode on KeyU by flipping which of the two cameras is
// Camera::is_active (never both/neither — same "swap the active one"
// pattern update_scope_lens uses for scope_camera vs. the main camera), and
// shows/hides PlayerModel to match: there's no point standing in third
// person looking at an invisible body. Also flips it back off on return to
// first person, on top of whatever KeyT (PlayerModelVisible's own debug
// toggle) last left it at — the two controls stepping on each other here is
// an accepted, documented quirk rather than plumbing a second bool through.
pub(crate) fn toggle_camera_mode(
    keys: Res<ButtonInput<KeyCode>>,
    mut mode: ResMut<CameraMode>,
    mut player_model_visible: ResMut<PlayerModelVisible>,
    mut fps_camera: Query<&mut Camera, (With<FpsCamera>, Without<ThirdPersonCamera>)>,
    mut third_person_camera: Query<&mut Camera, (With<ThirdPersonCamera>, Without<FpsCamera>)>,
) {
    if !keys.just_pressed(KeyCode::KeyU) {
        return;
    }
    *mode = match *mode {
        CameraMode::FirstPerson => CameraMode::ThirdPerson,
        CameraMode::ThirdPerson => CameraMode::FirstPerson,
    };
    let third_person = *mode == CameraMode::ThirdPerson;
    if let Ok(mut camera) = fps_camera.single_mut() {
        camera.is_active = !third_person;
    }
    if let Ok(mut camera) = third_person_camera.single_mut() {
        camera.is_active = third_person;
    }
    player_model_visible.0 = third_person;
}

// While in third person, orbits ThirdPersonCamera behind-and-to-one-side of
// wherever FpsCamera (still the real look/aim source — see CameraMode's doc
// comment) is currently facing, PUBG-style over-the-shoulder rather than
// dead-center behind the head. Orbits on yaw alone (not the full look
// rotation) so aiming straight up/down doesn't swing the camera to sit
// above/below the player — only the look-at tilt below follows pitch.
// Raycasts from the eye out to the desired spot (same SpatialQuery::cast_ray
// idiom shoot() uses for wall occlusion, movement_combat.rs's own
// wall_distance) and pulls the camera in to just short of anything solid it
// hits, so it never clips through a wall or prop behind the player.
pub(crate) fn sync_third_person_camera(
    mode: Res<CameraMode>,
    spatial_query: SpatialQuery,
    eye: Query<(Entity, &Transform, &FpsCamera, &Lean), Without<ThirdPersonCamera>>,
    mut camera: Query<&mut Transform, (With<ThirdPersonCamera>, Without<FpsCamera>)>,
) {
    if *mode != CameraMode::ThirdPerson {
        return;
    }
    let Ok((eye_entity, eye_transform, cam, lean)) = eye.single() else {
        return;
    };
    let Ok(mut camera_transform) = camera.single_mut() else {
        return;
    };
    let yaw_only = Quat::from_rotation_y(cam.yaw);
    let horizontal_forward = yaw_only * Vec3::NEG_Z;
    let horizontal_right = yaw_only * Vec3::X;
    // Bias toward the right shoulder at rest; Q (lean left, Lean.current
    // negative) pulls this past center to the left shoulder instead,
    // E (lean right) pushes further right — same shoulder-swap-on-lean feel
    // as PUBG/most cover shooters, riding Lean's existing ease rather than a
    // second hand-rolled lerp.
    let side_offset = THIRD_PERSON_SIDE_OFFSET * (lean.current + THIRD_PERSON_SHOULDER_BIAS).clamp(-1.0, 1.0);
    let desired = eye_transform.translation - horizontal_forward * THIRD_PERSON_DISTANCE
        + horizontal_right * side_offset
        + Vec3::Y * THIRD_PERSON_HEIGHT_OFFSET;
    let offset = desired - eye_transform.translation;
    let distance = offset.length();
    let Ok(direction) = Dir3::new(offset) else {
        return;
    };
    let filter = SpatialQueryFilter::from_excluded_entities([eye_entity]);
    let safe_distance = spatial_query
        .cast_ray(eye_transform.translation, direction, distance, true, &filter)
        .map(|hit| (hit.distance - THIRD_PERSON_COLLISION_MARGIN).max(0.0))
        .unwrap_or(distance);
    camera_transform.translation = eye_transform.translation + direction * safe_distance;
    // Looks toward a point out along the real aim ray (yaw+pitch, ignoring
    // roll) rather than the camera's own position — so pitching the mouse
    // up/down still tilts this camera (see THIRD_PERSON_LOOK_AHEAD_DISTANCE's
    // own doc comment), while the camera's position above only orbited on
    // yaw, keeping the framing this was tuned against stable.
    let aim_dir = Quat::from_euler(EulerRot::YXZ, cam.yaw, cam.pitch, 0.0) * Vec3::NEG_Z;
    let look_target = eye_transform.translation + aim_dir * THIRD_PERSON_LOOK_AHEAD_DISTANCE;
    camera_transform.look_at(look_target, Vec3::Y);
}

// KeyC toggles Standing<->Crouching; KeyX toggles Standing<->Prone. Pressing
// C while Prone (or X while Crouching) switches directly to the other
// stance rather than needing to stand up first.
pub(crate) fn toggle_player_stance(keys: Res<ButtonInput<KeyCode>>, mut player: Query<&mut PlayerStance>) {
    let Ok(mut stance) = player.single_mut() else {
        return;
    };
    if keys.just_pressed(KeyCode::KeyC) {
        *stance = if *stance == PlayerStance::Crouching {
            PlayerStance::Standing
        } else {
            PlayerStance::Crouching
        };
    }
    if keys.just_pressed(KeyCode::KeyX) {
        *stance = if *stance == PlayerStance::Prone {
            PlayerStance::Standing
        } else {
            PlayerStance::Prone
        };
    }
}

// Captures the new input and applies it to the local Player immediately —
// this is what makes movement feel instant instead of waiting a round trip.
// predict_player_movement only reconciles (rare, network-tick-rate
// frequency), it doesn't replay this per-frame application.
pub(crate) fn send_player_input(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    touch: TouchGameplayInputs,
    time: Res<Time>,
    game_over: Res<GameOver>,
    mut commands: Commands,
    mut pending: ResMut<PredictedInputs>,
    mut sequence: Local<u32>,
    camera_query: Query<&FpsCamera>,
    cursor: Query<&CursorOptions, With<PrimaryWindow>>,
    mut player_query: Query<(&mut Transform, &mut Player, &PlayerStance), Without<NetPlayer>>,
    noclip: Res<Noclip>,
    touch_move_axis: Res<TouchMoveAxis>,
    // Bundled into one nested-tuple param instead of flat ones — this fn
    // was already at Bevy's flat SystemParam tuple arity limit (16);
    // adding LocalPlayerSprinting as a 17th flat param broke `IntoSystem`
    // for the whole Update-schedule `.chain()` tuple this fn sits in.
    // Tuples of SystemParams are themselves valid SystemParams, so nesting
    // sidesteps the limit without restructuring anything else — Autopilot/
    // AutopilotIntent are folded into the same tuple for the same reason.
    (mut local_moving, mut local_sprinting, autopilot, intent, editor): (
        ResMut<LocalPlayerMoving>,
        ResMut<LocalPlayerSprinting>,
        Res<Autopilot>,
        Res<AutopilotIntent>,
        // Same `Option` reasoning as grab_cursor's own editor param — only
        // present when EDITOR=1 actually added bevy_editor_pls::EditorPlugin.
        Option<Res<bevy_editor_pls::editor::Editor>>,
    ),
    mover: MoveAndSlide,
    // Local prediction below still runs every single Update tick (up to
    // 120Hz on iOS via CADisplayLink) for smooth immediate feedback — only
    // the network SEND at the bottom of this fn is rate-limited, since
    // the server only ticks at SERVER_TICK_HZ (30Hz) and can't act on
    // input faster than that anyway; sending up to 4x more packets/sec
    // than the server can consume was pure wasted CPU/bandwidth on both
    // ends, not extra responsiveness.
    mut input_send_state: Local<InputSendThrottleState>,
) {
    if game_over.0 {
        return;
    }
    let Ok(cam) = camera_query.single() else {
        return;
    };
    let Ok((mut transform, mut player, stance)) = player_query.single_mut() else {
        return;
    };
    let stance = *stance;

    let aiming = cursor.single().is_ok_and(|cursor| is_aiming(cursor, &mouse, touch.aiming.0));

    // While the dev editor (EDITOR=1, F2) is active, WASD is bevy_editor_pls'
    // own free-fly camera input (camera_3d_free.rs, right-click + WASD) —
    // same reasoning grab_cursor already applies to mouse-look, just missed
    // here originally. Without this, WASD drove the real Player alongside
    // (and fighting) the editor's own camera, since this system reads raw
    // KeyCode presses unconditionally.
    let editor_active = editor.is_some_and(|e| e.active());

    // Real input, or this module's own AutopilotIntent while Autopilot is on.
    let (move_axis, sprinting) = if autopilot.0 {
        (intent.move_axis, intent.sprint)
    } else if editor_active {
        (Vec2::ZERO, false)
    } else {
        let sprinting = (keys.pressed(KeyCode::ShiftLeft) || touch.sprint.0) && stance == PlayerStance::Standing;
        let mut move_axis = Vec2::ZERO;
        if keys.pressed(KeyCode::KeyW) {
            move_axis.y += 1.0;
        }
        if keys.pressed(KeyCode::KeyS) {
            move_axis.y -= 1.0;
        }
        if keys.pressed(KeyCode::KeyD) {
            move_axis.x += 1.0;
        }
        if keys.pressed(KeyCode::KeyA) {
            move_axis.x -= 1.0;
        }
        // Harmless to add unconditionally: on real mobile there's no
        // keyboard, and on desktop (only reachable via TOUCH_UI_PREVIEW)
        // this is 0 unless actively testing the touch joystick.
        (
            (move_axis + touch_move_axis.0).clamp_length_max(1.0),
            sprinting,
        )
    };
    local_moving.0 = move_axis != Vec2::ZERO;
    local_sprinting.0 = sprinting;

    *sequence += 1;
    let dt = time.delta_secs();
    // While noclipping, Crouch flies down (see simulate_noclip_step's own
    // "vertical" match on input.stance) — touch.noclip_fly.down overrides
    // the *reported* stance for this input only, without touching the
    // real PlayerStance component (touch_crouch_button already skips its
    // own toggle during noclip for the same reason: don't leave the
    // player stuck Crouching once noclip turns back off).
    let effective_stance = if noclip.0 && touch.noclip_fly.down { PlayerStance::Crouching } else { stance };
    let input = ClientInput {
        sequence: *sequence,
        move_axis,
        yaw: cam.yaw,
        // Held while noclipping (hold Space, or touch.noclip_fly.up, to
        // fly up) vs. a one-shot press for a real jump —
        // simulate_player_step's two branches (noclip vs normal) never
        // both read this in the same frame, so it's safe to give it
        // different semantics per mode instead of adding a second field.
        jump: if noclip.0 {
            keys.pressed(KeyCode::Space) || touch.noclip_fly.up
        } else {
            keys.just_pressed(KeyCode::Space) || touch.jump.0
        },
        sprint: sprinting,
        aiming,
        stance: effective_stance,
        noclip: noclip.0,
    };
    // Predicted locally same as real input — while Autopilot is on,
    // intent.move_axis is this client's own decision, exactly as
    // authoritative-ish as real WASD.
    shared::movement::simulate_player_step(&mover, &mut transform, &mut player, stance, &input, dt);
    pending.0.push_back((*sequence, input, dt, player.vertical_velocity, player.grounded));

    input_send_state.jump_edge_pending |= input.jump;
    input_send_state.elapsed += dt;
    if input_send_state.elapsed < 1.0 / INPUT_SEND_HZ {
        return;
    }
    input_send_state.elapsed = 0.0;
    let mut sent_input = input;
    sent_input.jump = input_send_state.jump_edge_pending;
    input_send_state.jump_edge_pending = false;
    // Sent regardless of autopilot: while it's on this is a content-free
    // heartbeat (the server ignores/overwrites move_axis/yaw/sprint for
    // this player anyway), but it still needs to arrive so
    // LastProcessedInput keeps advancing — that's what triggers the
    // reconciliation snap above, autopilot or not.
    commands.client_trigger(sent_input);
}

// The camera doesn't move itself anymore — it just follows the physics body
// at eye height every frame. Its rotation is set separately by
// apply_view_recoil, which runs earlier in the chain.
pub(crate) fn sync_camera_to_player(
    time: Res<Time>,
    player_query: Query<(&Transform, &PlayerStance), (With<Player>, Without<FpsCamera>)>,
    mut camera_query: Query<(&mut Transform, &Lean), With<FpsCamera>>,
    mut smooth: ResMut<PredictionSmooth>,
) {
    let Ok((player_transform, stance)) = player_query.single() else {
        return;
    };
    let Ok((mut camera_transform, lean)) = camera_query.single_mut() else {
        return;
    };
    let eye_height_offset = match stance {
        PlayerStance::Standing => EYE_HEIGHT_OFFSET,
        PlayerStance::Crouching => CROUCH_EYE_HEIGHT_OFFSET,
        PlayerStance::Prone => PRONE_EYE_HEIGHT_OFFSET,
    };
    // predict_player_movement (runs just before, same frame) parked the
    // reconciliation delta here so the body could teleport without the
    // camera popping; ease it to zero.
    smooth.offset = smooth
        .offset
        .lerp(Vec3::ZERO, (time.delta_secs() / RECONCILE_SMOOTH_SECS).min(1.0));
    let eye = player_transform.translation + smooth.offset + Vec3::Y * eye_height_offset;
    // Uses the rotation apply_view_recoil already set this frame (it runs
    // earlier in the chain) so the peek offset follows whichever way the
    // camera is actually facing, including its current lean roll.
    let side_offset = *camera_transform.right() * LEAN_SIDE_OFFSET * lean.current;
    camera_transform.translation = eye + side_offset;
}

// Keeps PlayerModel (see its own doc comment) glued to the local player's
// position and look yaw every frame — not parented, so this is the
// equivalent of what parenting would do automatically, minus fighting the
// Player entity's own avian3d-owned Transform.
pub(crate) fn sync_player_model(
    visible: Res<PlayerModelVisible>,
    tuning: Res<PlayerModelTuning>,
    player: Query<&Transform, (With<Player>, Without<PlayerModel>)>,
    camera: Query<&FpsCamera>,
    mut model: Query<(&mut Transform, &mut Visibility), (With<PlayerModel>, Without<Player>)>,
) {
    let Ok((mut transform, mut visibility)) = model.single_mut() else {
        return;
    };
    *visibility = if visible.0 { Visibility::Visible } else { Visibility::Hidden };
    if !visible.0 {
        return;
    }
    let Ok(player_transform) = player.single() else {
        return;
    };
    let yaw = camera.single().map(|cam| cam.yaw).unwrap_or(0.0);
    // scale re-applied every frame (not just at spawn) so nudge_player_model_
    // tuning's live Comma/Period adjustments show up immediately instead of
    // only on the next restart.
    transform.scale = Vec3::splat(tuning.scale);
    transform.translation = player_transform.translation + Vec3::Y * tuning.y_offset;
    // + facing_deg: see PlayerModelTuning's own doc comment — this asset's
    // rig faces +Z, not Bevy's -Z-forward convention, so a bare
    // `Quat::from_rotation_y(yaw)` (matching every other yaw-driven rotation
    // in this file) rendered the model walking backward relative to where
    // it was actually headed.
    transform.rotation = Quat::from_rotation_y(yaw + tuning.facing_deg.to_radians());
}

const PLAYER_CHARACTER_RON: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/characters/soldier/character.ron");

// EDITOR=1-only: Comma/Period nudge PlayerModelTuning.scale, Minus/Equal
// nudge y_offset — see PlayerModelTuning's own doc comment. Meant to be
// used while looking at the local player in third-person (KeyU) so the
// effect (sync_player_model re-applies this resource every frame) is
// visible immediately; F9 (save_player_model_tuning) then writes whatever
// this settles on back to character.ron.
pub(crate) fn nudge_player_model_tuning(keys: Res<ButtonInput<KeyCode>>, mut tuning: ResMut<PlayerModelTuning>) {
    const SCALE_STEP: f32 = 0.02;
    const Y_OFFSET_STEP: f32 = 0.02;
    if keys.just_pressed(KeyCode::Comma) {
        tuning.scale = (tuning.scale - SCALE_STEP).max(0.05);
    }
    if keys.just_pressed(KeyCode::Period) {
        tuning.scale += SCALE_STEP;
    }
    if keys.just_pressed(KeyCode::Minus) {
        tuning.y_offset -= Y_OFFSET_STEP;
    }
    if keys.just_pressed(KeyCode::Equal) {
        tuning.y_offset += Y_OFFSET_STEP;
    }
}

// F9: writes PlayerModelTuning's current (possibly nudged) scale/y_offset/
// facing_deg back into character.ron, rewriting the WHOLE file (not just
// those 3 numbers) so id/display_name/lore_ref/mesh/animations/audio —
// carried unchanged in PlayerModelTuning purely for this — survive the
// round-trip instead of getting clobbered. Same "eyeball it live, F9
// saves" loop as save_zombie_meshy_showcase_transform (zombie_client.rs),
// just saving a live-nudged Resource instead of a live-dragged Transform:
// sync_player_model overwrites PlayerModel's own Transform every frame, so
// there'd be nothing meaningful to read a hand-dragged value back FROM
// there the way the zombie showcase (never overwritten per-frame) allows.
pub(crate) fn save_player_model_tuning(
    keys: Res<ButtonInput<KeyCode>>,
    tuning: Res<PlayerModelTuning>,
    mut toast: ResMut<F9SaveToast>,
) {
    if !keys.just_pressed(KeyCode::F9) {
        return;
    }
    // Reuses the same formatter player_anim_editor.rs's mesh-file-browser
    // Save button calls for any OTHER character's character.ron — see
    // format_character_ron's own doc comment (lib.rs).
    let out = format_character_ron(&CharacterRon {
        id: tuning.id.clone(),
        display_name: tuning.display_name.clone(),
        lore_ref: tuning.lore_ref.clone(),
        mesh: tuning.mesh.clone(),
        scale: tuning.scale,
        y_offset: tuning.y_offset,
        facing_deg: tuning.facing_deg,
        animations: tuning.animations.clone(),
        audio: tuning.audio.clone(),
    });
    match std::fs::write(PLAYER_CHARACTER_RON, out) {
        Ok(()) => {
            info!("saved soldier character.ron -> {PLAYER_CHARACTER_RON}");
            toast.ok_also("F9: saved soldier character.ron", " + soldier");
        }
        Err(e) => {
            error!("F9 soldier character.ron save failed writing {PLAYER_CHARACTER_RON}: {e}");
            toast.err(format!("F9: soldier character.ron save FAILED — {e}"));
        }
    }
}

// Holding left click (while the cursor is locked) fires automatically at
// FIRE_RATE: each shot kicks the gun model back, kicks the camera's view up
// with a bit of random jitter, and raycasts from the camera into the scene
// (with spread that grows the longer you hold the trigger). On a hit, it
// drops a small glowing marker at the impact point.
pub(crate) fn shoot(
    mut commands: Commands,
    time: Res<Time>,
    game_over: Res<GameOver>,
    mouse: Res<ButtonInput<MouseButton>>,
    touch_aiming: Res<TouchAiming>,
    cursor: Query<&CursorOptions, With<PrimaryWindow>>,
    mut camera: Query<(&Transform, &mut ViewRecoil), With<FpsCamera>>,
    mut gun: Query<&mut Gun>,
    mut vfx_assets: VfxAssets,
    zombies: Query<(Entity, &Transform, &ZombieHitboxBones), With<Zombie>>,
    globals: Query<&GlobalTransform>,
    spatial_query: SpatialQuery,
    touch_firing: Res<TouchFiring>,
    inspect_mode: Res<InspectMode>,
    mut cooldown: Local<f32>,
    (autopilot, intent, local_moving, local_sprinting, show_hit_markers, show_tracers): (
        Res<Autopilot>,
        Res<AutopilotIntent>,
        Res<LocalPlayerMoving>,
        Res<LocalPlayerSprinting>,
        Res<ShowHitMarkers>,
        Res<ShowBulletTracers>,
    ),
) {
    *cooldown = (*cooldown - time.delta_secs()).max(0.0);

    // Pure spectator camera while inspecting a map — see InspectMode's own
    // doc comment for why this can't just rely on the gun being hidden
    // (hidden ≠ can't still fire and burn ammo/trigger a reload).
    if game_over.0 || inspect_mode.0 {
        return;
    }
    let Ok(cursor) = cursor.single() else {
        return;
    };
    // Real mouse/touch input, or this module's own AutopilotIntent.fire
    // while Autopilot is on — same full-takeover reasoning mouse_look/
    // send_player_input already apply.
    let firing = if autopilot.0 {
        intent.fire
    } else {
        (cursor.grab_mode != CursorGrabMode::None && mouse.pressed(MouseButton::Left)) || touch_firing.0
    };
    if !firing || *cooldown > 0.0 {
        return;
    }
    let Ok(mut gun) = gun.single_mut() else {
        return;
    };
    // Mid-reload — click, no bang. Out-of-ammo can't happen here anymore:
    // the shot that empties the mag triggers a reload immediately below, so
    // ammo == 0 always implies reload_remaining > 0 too.
    if gun.ammo == 0 || gun.reload_remaining > 0.0 {
        return;
    }
    *cooldown = FIRE_RATE;
    gun.ammo -= 1;
    gun.recoil = RECOIL_DURATION;
    // Different frequency multiplier than the yaw jitter below so the two
    // don't stay correlated shot to shot.
    gun.recoil_roll = ops::sin(time.elapsed_secs() * 641.0);
    commands.spawn((AudioPlayer(vfx_assets.gunshot.0.clone()), PlaybackSettings::DESPAWN));
    if gun.ammo == 0 {
        // Auto-reload once the mag empties from firing — manual reload via
        // reload_gun (KeyR) is still how you top off early.
        gun.reload_remaining = RELOAD_DURATION;
        commands.spawn((AudioPlayer(vfx_assets.reload_sound.0.clone()), PlaybackSettings::DESPAWN));
    }

    let Ok((camera_transform, mut recoil)) = camera.single_mut() else {
        return;
    };

    recoil.pitch_kick = (recoil.pitch_kick + RECOIL_PITCH_KICK).min(RECOIL_PITCH_KICK_MAX);
    // Cheap deterministic "randomness" (no rand crate needed) so left/right
    // jitter varies shot to shot instead of always pulling the same way.
    let jitter = ops::sin(time.elapsed_secs() * 977.0);
    recoil.yaw_kick += jitter * RECOIL_YAW_JITTER;

    let stance = vfx_assets.stance.single().copied().unwrap_or_default();
    // LocalPlayerMoving/LocalPlayerSprinting instead of raw key checks —
    // see gun_view_animation's own comment on why (blind to touch input
    // and Autopilot's synthetic movement otherwise).
    let spread = current_bullet_spread(
        recoil.pitch_kick,
        is_aiming(cursor, &mouse, touch_aiming.0),
        local_moving.0,
        stance,
        local_sprinting.0,
    );
    let spread_x = ops::sin(time.elapsed_secs() * 613.0) * spread;
    let spread_y = ops::sin(time.elapsed_secs() * 811.0) * spread;
    let spread_dir = *camera_transform.forward()
        + *camera_transform.right() * spread_x
        + *camera_transform.up() * spread_y;
    let direction = Dir3::new(spread_dir).unwrap_or(camera_transform.forward());

    // Mirror the same shoulder swap gun_view_animation applies to the gun
    // mesh's X offset (reading the same eased gun.shoulder_blend it writes,
    // not Lean.current directly, so this stays in sync with the mesh's own
    // independently-eased swap instead of the raw lean value), so the
    // flash/smoke spawn at the muzzle instead of floating on the old
    // (right-shoulder) side while leaning left.
    let swapped_muzzle_offset = Vec3::new(
        MUZZLE_OFFSET.x * (1.0 - 2.0 * gun.shoulder_blend),
        MUZZLE_OFFSET.y,
        MUZZLE_OFFSET.z,
    );

    // World-space, not parented to the camera: a flash/puff should stay put
    // where it was fired, not swing around if the player turns right after.
    let muzzle_world_pos =
        camera_transform.translation + camera_transform.rotation * swapped_muzzle_offset;

    // Copying the camera's rotation onto the quad makes its face point back
    // at the camera (a cheap "billboard" — good enough since both effects
    // are gone within a fraction of a second).
    commands.spawn((
        Mesh3d(vfx_assets.muzzle_fx.flash_mesh.clone()),
        MeshMaterial3d(vfx_assets.muzzle_fx.flash_material.clone()),
        Transform::from_translation(muzzle_world_pos).with_rotation(camera_transform.rotation),
        PointLight {
            intensity: MUZZLE_LIGHT_INTENSITY,
            range: MUZZLE_LIGHT_RANGE,
            color: Color::srgb(1.0, 0.7, 0.3),
            shadows_enabled: false,
            ..default()
        },
        MuzzleFlash {
            remaining: MUZZLE_FLASH_LIFETIME,
        },
    ));

    commands.spawn((
        Mesh3d(vfx_assets.muzzle_fx.smoke_mesh.clone()),
        MeshMaterial3d(vfx_assets.muzzle_fx.smoke_material.clone()),
        Transform::from_translation(muzzle_world_pos)
            .with_rotation(camera_transform.rotation)
            .with_scale(Vec3::splat(SMOKE_START_SCALE)),
        Smoke {
            velocity: *camera_transform.forward() * 0.4 + Vec3::Y * 0.5,
            remaining: SMOKE_LIFETIME,
        },
    ));

    // Hit detection/damage is server-authoritative (see crates/server's
    // handle_fire_shot) — this sends the exact ray (already carrying this
    // shot's own spread/recoil jitter) for the server to resolve for real.
    // But waiting a full round trip just to show the shooter SOME response
    // reads as input lag, so this still does its own local prediction and,
    // on a hit, shows the hitmarker immediately — an honest "the raycast
    // physically stopped here" signal that doesn't depend on knowing real
    // damage, so it can't be wrong the way a locally-guessed damage number
    // could. The floating damage number and the blood-vs-spark choice (both
    // genuinely need to know the real damage) instead wait on the server's
    // own ShotConfirmed (see on_shot_confirmed) — deliberately NOT
    // predicted here anymore, so what the shooter sees always matches what
    // the zombie's health actually did, never a local guess that could
    // disagree with the server's own lag-compensated rewind.
    //
    // This casts against the REAL per-bone regions (zombie_hit_damage_ray),
    // not the wide outer broad-phase capsule (ZOMBIE_OUTER_RADIUS/
    // ZOMBIE_OUTER_HEIGHT) — that outer shell is only a cheap "did the shot
    // reach the zombie at all" pre-filter for the server's rewind lookup,
    // and stopping the visual tracer/hitmarker there reads as the shot
    // stopping short of the actual model. Every zombie is tested (not just
    // the nearest outer-shell hit) and the closest REAL bone hit across all
    // of them wins — the same "don't let one zombie's shell block the one
    // behind it" fix already applied to the server's rewound_hit.
    //
    // Wall occlusion: a raycast that excludes zombie colliders (only level
    // geometry / props block) — a bone hit farther than the nearest wall is
    // dropped, so the local hitmarker matches the server's own occlusion
    // check in handle_fire_shot instead of flashing through cover.
    let wall_filter = SpatialQueryFilter::from_excluded_entities(zombies.iter().map(|(e, ..)| e));
    let wall_distance = spatial_query
        .cast_ray(camera_transform.translation, direction, MAX_SHOT_DISTANCE, true, &wall_filter)
        .map(|hit| hit.distance);
    let predicted_hit = zombies
        .iter()
        .filter_map(|(entity, transform, bones)| {
            let bone_positions = resolve_bone_world_positions(bones, |bone| globals.get(bone).ok().map(GlobalTransform::translation));
            let forward = *transform.forward();
            zombie_hit_damage_ray(camera_transform.translation, direction, MAX_SHOT_DISTANCE, &bone_positions, forward)
                .map(|(point, normal, _damage)| (point.distance(camera_transform.translation), entity, point, normal))
        })
        .filter(|(distance, ..)| !wall_distance.is_some_and(|w| w < distance - 0.1))
        .min_by(|(a, ..), (b, ..)| a.total_cmp(b));
    if let Some((distance, entity, point, normal)) = predicted_hit {
        info!("[shot] local prediction: real bone hit zombie={entity} distance={distance:.2}");
        if show_hit_markers.0 {
            commands.spawn((
                Mesh3d(vfx_assets.hit_assets.mesh.clone()),
                MeshMaterial3d(vfx_assets.hit_assets.material.clone()),
                Transform::from_translation(point + normal.normalize_or_zero() * 0.01),
                HitMarker {
                    remaining: HIT_MARKER_LIFETIME,
                },
            ));
        }

        // Autopilot-only: real players keep the flat FIRE_RATE cooldown at
        // any range/ADS state — see autopilot.rs's own AUTOPILOT_TAP_FIRE_
        // DISTANCE/_INTERVAL for the (separate, distance-gated) mechanism
        // that already slows autopilot's own fire down at range.
        if autopilot.0 && is_aiming(cursor, &mouse, touch_aiming.0) {
            *cooldown = FIRE_RATE * (1.0 + (distance / 10.0).min(4.0));
        }

        // Mark this zombie as "hit by us, watch for it dying" — see
        // count_local_kills, which is the only place this entry is ever
        // read/removed. Overwrites any still-live entry from an earlier
        // shot with a fresh TTL, which is fine: either way a kill shortly
        // after is this player's.
        vfx_assets.recently_hit.0.insert(entity, RECENTLY_HIT_ZOMBIE_TTL);
    }

    // Tracer: from the muzzle (not the camera — the hitmarker/damage ray
    // above deliberately uses the camera as its origin for aim accuracy, but
    // a streak starting at eye level instead of the gun would look wrong) to
    // wherever this shot actually ended — the real hit point on a landed
    // shot, or all the way out to MAX_SHOT_DISTANCE on a miss (real tracers
    // fly on past whatever they didn't hit too).
    let tracer_end = predicted_hit.map_or(camera_transform.translation + *direction * MAX_SHOT_DISTANCE, |(_, _, point, _)| point);
    let tracer_start = muzzle_world_pos + Vec3::Y * TRACER_START_Y_OFFSET;
    spawn_bullet_tracer(&mut commands, &vfx_assets.tracer, time.elapsed_secs() * 733.0, tracer_start, tracer_end, show_tracers.0);

    info!("[shot] fired: origin={:.2?} direction={:.2?}", camera_transform.translation, *direction);
    commands.client_trigger(FireShot {
        origin: camera_transform.translation,
        direction: *direction,
    });
}

// Pressing R starts a reload unless the mag's already full or a reload is
// already running. The magazine refills instantly once RELOAD_DURATION
// elapses (see gun_view_animation) — no partial reloads, no spare-mag count
// (infinite reserve ammo), same simplicity tradeoff as the rest of the gun's
// systems.
pub(crate) fn reload_gun(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    touch_reload: Res<TouchReload>,
    game_over: Res<GameOver>,
    reload_sound: Res<ReloadSound>,
    mut gun: Query<&mut Gun>,
) {
    if game_over.0 || !(keys.just_pressed(KeyCode::KeyR) || touch_reload.0) {
        return;
    }
    let Ok(mut gun) = gun.single_mut() else {
        return;
    };
    if gun.ammo == MAGAZINE_SIZE || gun.reload_remaining > 0.0 {
        return;
    }
    gun.reload_remaining = RELOAD_DURATION;
    commands.spawn((AudioPlayer(reload_sound.0.clone()), PlaybackSettings::DESPAWN));
}


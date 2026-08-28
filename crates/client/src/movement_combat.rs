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
    current: f32,
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
    mover: MoveAndSlide,
) {
    let Ok((net_transform, last_processed)) = local.single() else {
        return;
    };
    let Ok((mut transform, mut player, stance)) = player.single_mut() else {
        return;
    };

    let mut confirmed_baseline = None;
    while pending.0.front().is_some_and(|&(sequence, ..)| sequence <= last_processed.0) {
        let (_, _, _, vertical_velocity, grounded) = pending.0.pop_front().unwrap();
        confirmed_baseline = Some((vertical_velocity, grounded));
    }
    if let Some((vertical_velocity, grounded)) = confirmed_baseline {
        player.vertical_velocity = vertical_velocity;
        player.grounded = grounded;
    }

    transform.translation = net_transform.translation;
    for (_, input, dt, ..) in &pending.0 {
        shared::movement::simulate_player_step(&mover, &mut transform, &mut player, *stance, input, *dt);
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
    // RenderCompare deliberately parks its one Bite-pose zombie within
    // ZOMBIE_STOP_DISTANCE of the player (see RENDER_COMPARE_ZOMBIE_
    // POSITIONS' own doc comment) purely to force that pose for visual
    // comparison — it's not a combat test, so real bite damage there would
    // just end the game before anyone gets to look at anything.
    if game_over.0 || freeze.0 || god_mode.0 || current_scenario() == Scenario::RenderCompare {
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
    mut query: Query<&mut Lean>,
) {
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
    (mut local_moving, mut local_sprinting, autopilot, intent): (
        ResMut<LocalPlayerMoving>,
        ResMut<LocalPlayerSprinting>,
        Res<Autopilot>,
        Res<AutopilotIntent>,
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

    // Real input, or this module's own AutopilotIntent while Autopilot is on.
    let (move_axis, sprinting) = if autopilot.0 {
        (intent.move_axis, intent.sprint)
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
    player_query: Query<(&Transform, &PlayerStance), (With<Player>, Without<FpsCamera>)>,
    mut camera_query: Query<(&mut Transform, &Lean), With<FpsCamera>>,
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
    let eye = player_transform.translation + Vec3::Y * eye_height_offset;
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
    transform.translation = player_transform.translation + Vec3::Y * PLAYER_MODEL_Y_OFFSET;
    // + PLAYER_MODEL_FACING_CORRECTION: see PLAYER_MODEL_SCALE's own doc
    // comment — this asset's rig faces +Z, not Bevy's -Z-forward
    // convention, so a bare `Quat::from_rotation_y(yaw)` (matching every
    // other yaw-driven rotation in this file) rendered the model walking
    // backward relative to where it was actually headed.
    transform.rotation = Quat::from_rotation_y(yaw + PLAYER_MODEL_FACING_CORRECTION);
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
    // behind it" fix already applied to the server's rewound_hit. No wall-
    // occlusion check against this predicted ray — acceptable for the
    // current open arena layout (NAVMESH_COLUMN_OBSTACLES is empty), but a
    // zombie visible through/behind level geometry could show a
    // mispredicted hitmarker if that ever changes.
    let predicted_hit = zombies
        .iter()
        .filter_map(|(entity, transform, bones)| {
            let bone_positions = resolve_bone_world_positions(bones, |bone| globals.get(bone).ok().map(GlobalTransform::translation));
            let forward = *transform.forward();
            zombie_hit_damage_ray(camera_transform.translation, direction, MAX_SHOT_DISTANCE, &bone_positions, forward)
                .map(|(point, normal, _damage)| (point.distance(camera_transform.translation), entity, point, normal))
        })
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


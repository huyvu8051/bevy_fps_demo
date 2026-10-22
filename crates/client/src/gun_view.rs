use bevy::prelude::*;

use crate::*;

#[derive(Resource)]
pub(crate) struct ScopeCameraEnabled(pub(crate) bool);
impl Default for ScopeCameraEnabled {
    fn default() -> Self {
        Self(true)
    }
}

#[derive(Component)]
pub(crate) struct ScopeCamera;

#[derive(Component)]
pub(crate) struct ScopeLens;

#[derive(Component)]
pub(crate) struct ScopeReticle;

/// One mark per kind of glass, built once and swapped on the single reticle
/// entity rather than respawned, so nothing has to be rewired on a weapon
/// change.
#[derive(Resource)]
pub(crate) struct ReticleAssets {
    pub(crate) chevron: (Handle<Mesh>, Handle<StandardMaterial>),
    pub(crate) duplex: (Handle<Mesh>, Handle<StandardMaterial>),
}

/// Hands the sighted weapon its own mark.
pub(crate) fn swap_scope_reticle(
    active: Res<ActiveWeapon>,
    // Built in setup_gameplay, whose insert_resource only lands at the next
    // sync point, so this runs at least one frame before it exists.
    assets: Option<Res<ReticleAssets>>,
    mut reticle: Query<(&mut Mesh3d, &mut MeshMaterial3d<StandardMaterial>), With<ScopeReticle>>,
    mut shown: Local<Option<WeaponId>>,
) {
    let Some(assets) = assets else { return };
    if *shown == Some(active.0) {
        return;
    }
    let Ok((mut mesh, mut material)) = reticle.single_mut() else { return };
    let (next_mesh, next_material) = match active.0 {
        WeaponId::M24 => &assets.duplex,
        _ => &assets.chevron,
    };
    mesh.0 = next_mesh.clone();
    material.0 = next_material.clone();
    *shown = Some(active.0);
}

/// Where the scope looks from and where its picture is drawn, both marked in
/// the mesh. They say what they are for rather than what they sit on: the first
/// is the front glass, because that is where light enters, but the second is
/// set down the tube rather than on the rear glass, so the glass draws over the
/// picture and the view reads as recessed.
///
/// Carrying that depth in the asset rather than in a constant here is the point
/// — it is a property of one scope's glass and tube, not of every weapon.
///
/// Both sockets' scale is the lens radius, so a unit circle under one is the
/// right width. A camera must take the position and the turn but never that
/// scale, or it would scale the whole world it renders.
///
/// Riding these means recoil, sway, bob and the shoulder swap come for free and
/// cannot drift out of step with the mesh.
#[derive(Component)]
pub(crate) struct ScopeCameraSocket;

#[derive(Component)]
pub(crate) struct ScopeViewSocket;

/// Pins the sights up so the scope can be looked at without waiting for the
/// autopilot to pick a target far enough away to want them.
pub(crate) fn force_ads() -> bool {
    std::env::var("FORCE_ADS").map(|value| value == "1").unwrap_or(false)
}

/// Where the eye lines up behind the sights: the rear glass on a scope, the
/// rear sight on irons. Distinct from ScopeViewSocket, which is where the
/// picture draws and sits further down the tube.
#[derive(Component)]
pub(crate) struct AimSocket;

/// The front post on a weapon that sights down irons. Paired with AimSocket it
/// gives the sight line, the same way the scope pair gives the optical axis.
#[derive(Component)]
pub(crate) struct FrontSightSocket;

/// The bore's end. Everything that leaves the barrel is placed and aimed from
/// here, so flash, smoke and casings follow the weapon through recoil and sway
/// without any of them holding their own copy of where the barrel is.
#[derive(Component)]
pub(crate) struct MuzzleSocket;

/// Caches where the bore ends inside the weapon.
///
/// The socket does not move within the weapon, so reparenting last frame's
/// world transforms yields a value that is exact rather than one frame late.
/// What it is composed with afterwards is the current weapon transform.
pub(crate) fn track_muzzle_socket(
    muzzle: Query<&GlobalTransform, With<MuzzleSocket>>,
    mut gun: Query<(&GlobalTransform, &mut Gun)>,
) {
    let Ok(socket) = muzzle.single() else { return };
    let Ok((gun_global, mut gun)) = gun.single_mut() else { return };
    gun.muzzle_local = socket.reparented_to(gun_global);
}

/// Reads the scope's optical axis off the two points the mesh marks along the
/// tube, and caches the turn that brings it onto the camera's.
///
/// Sighting only ever slid the weapon until Socket_Aim sat on the camera axis
/// and then assumed the glass was looking straight down the mesh. Nothing
/// checked it, so a scope mounted with any cant would have aimed off and no
/// weapon could have said so. Two marked points are enough to know instead of
/// assume — 304mm apart on the M24, which is plenty of baseline.
///
/// Neither point moves inside the weapon, so this runs off the propagated
/// transforms and the result is exact rather than a frame late.
pub(crate) fn track_optical_axis(
    glass: Query<&GlobalTransform, With<ScopeCameraSocket>>,
    post: Query<&GlobalTransform, With<FrontSightSocket>>,
    aim: Query<&GlobalTransform, (With<AimSocket>, Without<ScopeCameraSocket>, Without<FrontSightSocket>)>,
    mut gun: Query<(&GlobalTransform, &mut Gun)>,
) {
    let Ok((gun_global, mut gun)) = gun.single_mut() else { return };
    // Glass on a scope, the front post on irons: either way it is the far end
    // of the sight line, and either way the near end is where the eye goes.
    let front = glass.single().or(post.single());
    let (Ok(front), Ok(aim)) = (front, aim.single()) else {
        gun.optical_fix = Quat::IDENTITY;
        return;
    };
    let down_the_tube = front.reparented_to(gun_global).translation
        - aim.reparented_to(gun_global).translation;
    let Ok(axis) = Dir3::new(down_the_tube) else { return };
    gun.optical_fix = Quat::from_rotation_arc(*axis, Vec3::NEG_Z);
}

/// Hands the mesh's own named points to the systems that want them, so the
/// weapon table carries none of this geometry twice.
///
/// Only a weapon with glass marks the scope pair; one that sights down irons
/// leaves both unmarked and the whole scope view stays off for it.
pub(crate) fn wire_weapon_sockets(
    mut commands: Commands,
    child_of: Query<&ChildOf>,
    gun: Query<(), With<Gun>>,
    new_names: Query<(Entity, &Name), Added<Name>>,
) {
    for (entity, name) in &new_names {
        if !name.as_str().starts_with("Socket_") {
            continue;
        }
        let mut current = entity;
        let mut under_gun = false;
        for _ in 0..20 {
            if gun.contains(current) {
                under_gun = true;
                break;
            }
            let Ok(parent) = child_of.get(current) else { break };
            current = parent.0;
        }
        if !under_gun {
            continue;
        }
        let mut socket = commands.entity(entity);
        match name.as_str() {
            "Socket_ScopeCamera" => socket.insert(ScopeCameraSocket),
            "Socket_ScopeView" => socket.insert(ScopeViewSocket),
            "Socket_Aim" => socket.insert(AimSocket),
            "Socket_SightFront" => socket.insert(FrontSightSocket),
            "Socket_Muzzle" => socket.insert(MuzzleSocket),
            "Socket_RearGrip" => socket.insert(TriggerGripSocket),
            "Socket_FrontGrip" => socket.insert(ForeGripSocket),
            _ => continue,
        };
        info!("[socket] {name} wired");
    }
}

#[derive(Resource)]
pub(crate) struct ScopeZoom(pub(crate) f32);

pub(crate) fn update_scope_zoom_slider_visibility(
    mouse: Res<ButtonInput<MouseButton>>,
    touch_aiming: Res<TouchAiming>,
    cursor: Query<&CursorOptions, With<PrimaryWindow>>,
    mut track: Query<&mut Node, With<TouchScopeZoomSliderTrack>>,
) {
    let Ok(mut node) = track.single_mut() else {
        return;
    };
    let aiming = cursor.single().is_ok_and(|c| is_aiming(c, &mouse, touch_aiming.0));
    node.display = if aiming { Display::Flex } else { Display::None };
}

pub(crate) struct GunPoseTimings {
    recoil_t: f32,
    dip: f32,
    bob_x: f32,
    bob_y: f32,
}

pub(crate) fn gun_pose_timings(gun: &Gun) -> GunPoseTimings {
    let recoil_t = gun.recoil;
    let reload_t = gun.reload_remaining / RELOAD_DURATION;

    // A raised cosine rather than the parabola 4t(1-t) it replaces. Both peak
    // at 1 halfway through and both end at 0, but the parabola arrives and
    // leaves at full speed, so the weapon snapped into the dip and snapped
    // back out of it. This one is flat at both ends: it eases in and eases
    // out, which is the curve the sights and the kick already use.
    let dip = ops::sin(std::f32::consts::PI * reload_t).powi(2);
    let bob_aim_scale = 1.0 - gun.ads_blend * (1.0 - AIM_BOB_MULTIPLIER);
    let bob_x = ops::sin(gun.bob_phase) * GUN_BOB_AMOUNT_X * gun.bob_blend * bob_aim_scale;
    // abs() already doubles the rate: |sin| repeats every pi, not every tau.
    // One dip per footfall, against a full sway every two.
    let bob_y = ops::sin(gun.bob_phase).abs() * GUN_BOB_AMOUNT_Y * gun.bob_blend * bob_aim_scale;
    GunPoseTimings { recoil_t, dip, bob_x, bob_y }
}

/// Takes the scope picture from the eye rather than from the weapon.
///
/// Riding the weapon's own socket is what kills the parallax effect: the mark
/// hangs off the barrel, and a camera bolted to the same barrel moves with it,
/// so the two never part and the mark sits dead centre whatever the weapon
/// does. From the eye they move independently, which is the whole point —
/// a weapon held off the eye's line shows its mark off-centre, still over
/// what the barrel is pointing at.
pub(crate) fn sync_scope_camera(
    eye: Query<&Transform, (With<FpsCamera>, Without<ScopeCamera>)>,
    mut scope_camera: Query<&mut Transform, With<ScopeCamera>>,
) {
    let (Ok(eye), Ok(mut scope_transform)) = (eye.single(), scope_camera.single_mut()) else {
        return;
    };
    *scope_transform = Transform::from_translation(eye.translation).with_rotation(eye.rotation);
}

/// Turns the mark to point down the weapon's own sight axis.
///
/// It hangs off the scope camera, so it is always in front of the lens and
/// facing it; all the weapon contributes is the direction. That makes it a
/// perfectly parallax-free sight: the mark shows where the barrel points,
/// which is where the bullet goes, however the weapon sits against the eye.
/// The socket says which way that is, so no weapon carries its own copy.
pub(crate) fn place_scope_reticle(
    active: Res<ActiveWeapon>,
    aim: Query<&GlobalTransform, With<AimSocket>>,
    gun: Query<(&Transform, &GlobalTransform), (With<Gun>, Without<ScopeReticle>)>,
    zoom: Res<ScopeZoom>,
    mut reticle: Query<&mut Transform, With<ScopeReticle>>,
) {
    let (Ok(aim), Ok((gun_local, gun_global)), Ok(mut reticle)) =
        (aim.single(), gun.single(), reticle.single_mut())
    else {
        return;
    };
    // Both the camera and the weapon are placed from the eye this frame, so
    // working in the eye's frame cancels the eye out and leaves no stale term.
    // Only the socket's place *inside* the weapon comes from the propagated
    // transforms, and that never animates, so last frame's copy is exact.
    // Mixing a turn written this frame with one propagated last frame is what
    // shakes the mark in step with how fast the view is moving.
    let in_gun = aim.reparented_to(gun_global);
    // A unit throw, because only the direction survives: the mark hangs off
    // the camera, so pushing it further out moves it along the same ray and
    // changes nothing the player can see. There is no throw to tune.
    let down_the_barrel = gun_local.rotation * in_gun.rotation * Vec3::NEG_Z;
    reticle.translation = down_the_barrel;
    reticle.rotation = Quat::IDENTITY;
    // The mesh is built in radians at unit distance, so this is purely the
    // magnification: holding apparent thickness steady as the target grows is
    // what a second focal plane scope does.
    reticle.scale = Vec3::splat(active.def().scope.default_zoom / zoom.0);
}

pub(crate) fn adjust_scope_zoom(
    active: Res<ActiveWeapon>,
    mut wheel: MessageReader<MouseWheel>,
    mouse: Res<ButtonInput<MouseButton>>,
    touch_aiming: Res<TouchAiming>,
    cursor: Query<&CursorOptions, With<PrimaryWindow>>,
    mut zoom: ResMut<ScopeZoom>,
    camera: Query<&GlobalTransform, With<FpsCamera>>,
    autopilot: Res<Autopilot>,
    autopilot_state: Res<AutopilotState>,
    zombies: Query<&Transform, With<Zombie>>,
) {
    let Ok(cursor) = cursor.single() else {
        return;
    };
    if !is_aiming(cursor, &mouse, touch_aiming.0) {
        wheel.clear();
        return;
    }

    if autopilot.0 {
        if let Some(target_entity) = autopilot_state.target {
            if let (Ok(camera_global), Ok(zombie_transform)) =
                (camera.single(), zombies.get(target_entity))
            {
                let distance = camera_global.translation().distance(zombie_transform.translation);
                let t = ((distance - AUTO_ZOOM_NEAR_DISTANCE)
                    / (AUTO_ZOOM_FAR_DISTANCE - AUTO_ZOOM_NEAR_DISTANCE))
                    .clamp(0.0, 1.0);
                let scope = &active.def().scope;
                zoom.0 = scope.min_zoom + (scope.max_zoom - scope.min_zoom) * t;
                wheel.clear();
                return;
            }
        }
    }
    for event in wheel.read() {
        zoom.0 = (zoom.0 * SCOPE_ZOOM_STEP_FACTOR.powf(event.y.signum()))
            .clamp(active.def().scope.min_zoom, active.def().scope.max_zoom);
    }
}

pub(crate) fn update_scope_lens(
    gun: Query<&Gun>,
    zoom: Res<ScopeZoom>,
    scope_camera_enabled: Res<ScopeCameraEnabled>,
    mut scope_camera: Query<(&mut Camera, &mut Projection), With<ScopeCamera>>,
    mut lens: Query<(&mut Transform, &mut Visibility), (With<ScopeLens>, Without<ScopeReticle>)>,
    view_socket: Query<&GlobalTransform, With<ScopeViewSocket>>,
    gun_placed: Query<(&Transform, &GlobalTransform), (With<Gun>, Without<ScopeLens>, Without<ScopeReticle>)>,
) {
    let Ok(gun) = gun.single() else {
        return;
    };
    // No rear glass means no scope picture at all. The M4 aims down irons: the
    // gun comes up and that is the whole of it.
    let visible = !view_socket.is_empty()
        && scope_camera_enabled.0
        && gun.ads_blend > SCOPE_ACTIVE_ADS_THRESHOLD
        && gun.wall_raise < SCOPE_HIDDEN_WALL_RAISE_THRESHOLD;

    if let Ok((mut camera, mut projection)) = scope_camera.single_mut() {
        camera.is_active = visible;
        if let Projection::Perspective(perspective) = projection.as_mut() {
            perspective.fov = SCOPE_BASE_FOV / zoom.0;
        }
    }

    if let Ok((mut transform, mut visibility)) = lens.single_mut() {
        *visibility = if visible { Visibility::Visible } else { Visibility::Hidden };

        // What the player looks into is the rear glass, so sit on it. The disc
        // is a unit circle and the socket's scale is the lens radius, which
        // makes the picture exactly as wide as the real eyepiece.
        //
        // Set back down the tube rather than on the socket. Sat on it the disc
        // lands inside the 1.8mm the mesh's own glass occupies and the two
        // fight for the depth buffer; behind it the glass draws over the
        // picture, which is what looking through a scope does.
        //
        // The disc and the gun are both children of the eye, so the disc's
        // offset is the gun's own transform with the socket's place inside it
        // on the end. Only that inner part comes from the propagated
        // transforms, and it never animates; the gun's transform is this
        // frame's, so the disc cannot trail the mesh when the eye turns.
        if let (Ok(socket), Ok((gun_local, gun_global))) = (view_socket.single(), gun_placed.single()) {
            let in_gun = socket.reparented_to(gun_global);
            *transform = gun_local.mul_transform(in_gun);
        }
    }
}

pub(crate) fn gun_view_animation(
    time: Res<Time>,
    mouse: Res<ButtonInput<MouseButton>>,
    touch_aiming: Res<TouchAiming>,
    game_over: Res<GameOver>,
    look_delta: Res<MouseLookDelta>,
    cursor: Query<&CursorOptions, With<PrimaryWindow>>,
    camera: Query<(&Transform, &Lean), (With<FpsCamera>, Without<Gun>)>,
    spatial_query: SpatialQuery,
    mut query: Query<(&mut Transform, &GlobalTransform, &mut Gun), Without<FpsCamera>>,
    aim: Query<&GlobalTransform, (With<AimSocket>, Without<Gun>, Without<FpsCamera>)>,
    local_moving: Res<LocalPlayerMoving>,
    local_sprinting: Res<LocalPlayerSprinting>,
    active: Res<ActiveWeapon>,
) {
    let camera_single = camera.single().ok();
    let aiming = cursor.single().is_ok_and(|cursor| is_aiming(cursor, &mouse, touch_aiming.0));

    // Reads the requested lean, not the eased camera value: keying off the
    // eased one makes the gun wait for the camera to settle before it starts
    // moving, instead of travelling alongside it.
    let leaning = camera_single
        .is_some_and(|(_, lean)| lean.target.abs() > GUN_READY_LEAN_CUTOFF);

    // Keyed off the requested lean, not the eased camera value. The swap only
    // uses the negative half of the range, so coming out of a right lean it
    // stays pinned at zero until the camera crosses the middle and then picks
    // up on the slowest part of the camera ease. That is the gun lagging the
    // press. Reading the input instead starts both at the same instant.
    let shoulder_target = camera_single.map_or(0.0, |(_, lean)| (-lean.target).clamp(0.0, 1.0));

    let wall_target = camera_single.map_or(0.0, |(camera_transform, _)| {
        let filter = SpatialQueryFilter::default();
        spatial_query
            .cast_ray(
                camera_transform.translation,
                camera_transform.forward(),
                WALL_GUN_RAISE_DISTANCE,
                true,
                &filter,
            )
            .map_or(0.0, |hit| {
                1.0 - (hit.distance / WALL_GUN_RAISE_DISTANCE).clamp(0.0, 1.0)
            })
    });
    for (mut transform, gun_global, mut gun) in &mut query {
        gun.recoil *= ops::exp(-active.def().recoil.settle_speed * time.delta_secs());
        if gun.reload_remaining > 0.0 {
            gun.reload_remaining = (gun.reload_remaining - time.delta_secs()).max(0.0);
            if gun.reload_remaining == 0.0 {
                gun.ammo = active.def().magazine;
            }
        }

        let ads_target = if aiming || force_ads() { 1.0 } else { 0.0 };
        // Latched as the raise begins, and held for the whole of it. Reading
        // ready_blend live would not do: aiming drives it to zero, so a raise
        // that started from low ready would slow down halfway through it.
        // ready_blend is still last frame's here, which is what we want.
        if ads_target > 0.0 && gun.ads_blend <= f32::EPSILON {
            gun.ads_from_ready = gun.ready_blend;
        }
        let from_rest = ADS_FROM_REST_SCALE + (1.0 - ADS_FROM_REST_SCALE) * gun.ads_from_ready;
        // Dropping the sights is the weapon's own speed either way: what the
        // rest pose costs is the distance up, not the distance back down.
        let ads_rate = active.def().ads_speed * if ads_target > 0.0 { from_rest } else { 1.0 };
        let ads_step = (ads_rate * time.delta_secs()).min(1.0);
        gun.ads_blend += (ads_target - gun.ads_blend) * ads_step;

        // Having aimed once means the player is working a sight line and will
        // likely aim again, so the gun waits near the middle instead of
        // swinging all the way back to the hip. Breaking into a sprint says
        // they are done with that sight line, so it drops to the normal rest.
        if aiming {
            gun.ready_armed = true;
        } else if local_sprinting.0 || game_over.0 {
            gun.ready_armed = false;
        }
        // Leaning takes the gun back to its shoulder: the swap is a deliberate
        // move to the other side of cover, and a centred ready pose has nothing
        // to swap. Releasing the lean settles back to ready.
        let ready_target =
            if gun.ready_armed && !aiming && !leaning { 1.0 } else { 0.0 };
        let ready_step = (GUN_READY_SPEED * time.delta_secs()).min(1.0);
        gun.ready_blend += (ready_target - gun.ready_blend) * ready_step;

        // One roll per disturbance, so each return lands somewhere new.
        let disturbed = aiming || leaning || local_sprinting.0;
        if disturbed {
            if !gun.settle_rolled {
                gun.settle_rolled = true;
                let (offset, twist) = settle_jitter(time.elapsed_secs());
                gun.settle_jitter = offset;
                gun.settle_twist_jitter = twist;
            }
        } else {
            gun.settle_rolled = false;
        }

        // Chased rather than used raw. The offset lands straight on the final
        // translation with nothing else smoothing it, so swapping the value in
        // one frame moves the gun in one frame.
        let settle_step = (GUN_SETTLE_SPEED * time.delta_secs()).min(1.0);
        let settle_gap = gun.settle_jitter - gun.settle_current;
        gun.settle_current += settle_gap * settle_step;
        let twist_gap = gun.settle_twist_jitter - gun.settle_twist_current;
        gun.settle_twist_current += twist_gap * settle_step;

        let wall_raise_step = (WALL_GUN_RAISE_SPEED * time.delta_secs()).min(1.0);
        gun.wall_raise += (wall_target - gun.wall_raise) * wall_raise_step;

        let shoulder_step = (GUN_SHOULDER_SPEED * time.delta_secs()).min(1.0);
        gun.shoulder_blend += (shoulder_target - gun.shoulder_blend) * shoulder_step;

        let moving = local_moving.0;
        let bob_target = if moving { 1.0 } else { 0.0 };
        let bob_blend_step = (GUN_BOB_BLEND_SPEED * time.delta_secs()).min(1.0);
        gun.bob_blend += (bob_target - gun.bob_blend) * bob_blend_step;
        if moving {
            gun.bob_phase += GUN_BOB_SPEED * time.delta_secs();
        }

        // Late to a turn, not rolled by one. Both meshes put their origin on
        // the muzzle, so this pivots there: the barrel holds its aim and the
        // stock swings out. Negated because the weapon is still aimed back
        // where the view was, opposite the way it just moved.
        let lag = -look_delta.0;
        let sway_target = (lag * LOOK_SWAY_SENSITIVITY)
            .clamp(Vec2::splat(-LOOK_SWAY_MAX_DEG), Vec2::splat(LOOK_SWAY_MAX_DEG));
        let sway_step = (LOOK_SWAY_SPEED * time.delta_secs()).min(1.0);
        let sway_gap = sway_target - gun.look_sway;
        gun.look_sway += sway_gap * sway_step;

        let kick = &active.def().recoil;
        let timings = gun_pose_timings(&gun);

        // Faded out by ads_blend, the way the positional jitter already is
        // through resting: a weapon canted on its own bore would take the
        // sights off the camera axis. The optical correction fades the other
        // way, since it is only wanted while looking through the glass.
        let settle_twist = gun.settle_twist_current * (1.0 - gun.ads_blend);
        let base_rotation = Quat::from_rotation_z(settle_twist.to_radians())
            * Quat::from_rotation_x(RELOAD_TILT_DEG.to_radians() * timings.dip)
            * Quat::IDENTITY.slerp(gun.optical_fix, gun.ads_blend);

        let resting = gun.base_offset.lerp(active.def().ready_offset, gun.ready_blend) + gun.settle_current;
        // Sighted means Socket_Aim on the camera axis at this weapon.s eye
        // relief; the mesh says where that point is, so the offset is a
        // subtraction rather than a dialled-in vector. Where it sits inside
        // the gun never animates, so last frame.s copy of that is fine.
        //
        // Turned before it is subtracted. The socket sits 0.91m down the
        // weapon, so a straight subtraction only lands it on the axis while
        // the weapon is square to the camera -- the moment the pose turns it,
        // that lever throws the sight off by the turn times its length. Only
        // base_rotation is taken out: sway and recoil are meant to show.
        let sighted = aim
            .single()
            .map(|aim_global| {
                let in_gun = aim_global.reparented_to(gun_global);
                Vec3::new(0.0, 0.0, -active.def().ads_eye_relief) - base_rotation * in_gun.translation
            })
            .unwrap_or(resting);
        let base_pos = resting.lerp(sighted, gun.ads_blend) - Vec3::Z * view_pullback();

        let swapped_x = base_pos.x * (1.0 - 2.0 * gun.shoulder_blend);
        transform.translation = Vec3::new(swapped_x, base_pos.y, base_pos.z)
            + Vec3::new(
                timings.bob_x,
                kick.kick_up * timings.recoil_t - RELOAD_DIP * timings.dip - timings.bob_y
                    + WALL_GUN_RAISE_HEIGHT * gun.wall_raise,
                kick.kick_back * timings.recoil_t,
            );

        let wall_raise_tilt = WALL_GUN_RAISE_ROT_DEG.to_radians() * gun.wall_raise;
        transform.rotation = Quat::from_rotation_x(wall_raise_tilt)
            * Quat::from_rotation_y(gun.look_sway.x.to_radians())
            * Quat::from_rotation_x(gun.look_sway.y.to_radians())
            * Quat::from_rotation_y(kick.yaw_kick_deg.to_radians() * gun.recoil_roll * timings.recoil_t)
            * Quat::from_rotation_z(kick.roll_deg.to_radians() * gun.recoil_roll * timings.recoil_t)
            * Quat::from_rotation_x(kick.tilt_up_deg.to_radians() * timings.recoil_t)
            * base_rotation;
    }
}


/// Where the gun settles, nudged a little each time. Coming to rest on the
/// exact same millimetre after every aim is what makes the pose read as a
/// still image rather than a held weapon.
fn settle_jitter(seed: f32) -> (Vec3, f32) {
    let axis = |salt: f32| (ops::sin(seed * salt) * 43758.547).fract().abs() * 2.0 - 1.0;
    (
        Vec3::new(
            axis(12.9898) * GUN_SETTLE_JITTER.x,
            axis(78.233) * GUN_SETTLE_JITTER.y,
            axis(37.719) * GUN_SETTLE_JITTER.z,
        ),
        axis(94.673) * GUN_SETTLE_TWIST_DEG,
    )
}

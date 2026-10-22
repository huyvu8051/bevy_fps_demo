use bevy::prelude::*;

use crate::*;

/// Everything that belongs to the weapon rather than to the player holding it.
///
/// These are gathered in one place because every one of them is tuned against
/// a particular mesh: swap the mesh and all of them are wrong together. They
/// were global constants while there was only ever one gun.
pub(crate) struct WeaponDef {
    pub(crate) name: &'static str,
    pub(crate) mesh_path: &'static str,

    pub(crate) rest_offset: Vec3,
    pub(crate) ready_offset: Vec3,
    /// How far the eye sits behind Socket_Aim when sighted: the rear glass on
    /// a scope, the rear sight on irons. The gun is then placed so that point
    /// lands on the camera axis, which is the whole of ads_offset.
    pub(crate) ads_eye_relief: f32,
    /// How fast the sights come up, from the ready pose. Weight and length:
    /// a carbine snaps up, a scoped bolt gun does not.
    pub(crate) ads_speed: f32,
    /// How far the camera sits back off the eye while this weapon is held.
    /// See ViewPullback: smaller brings the view in on the weapon.
    pub(crate) view_pullback: f32,

    pub(crate) scope: Scope,
    pub(crate) sound: WeaponSound,
    /// How long the weapon takes to reload. A fifty round magazine and a five
    /// round bolt gun are not the same job.
    pub(crate) reload_duration: f32,
    pub(crate) fire_rate: f32,
    pub(crate) magazine: u32,
    pub(crate) recoil: Recoil,
}

/// What the weapon sounds like. A 7.62, a 5.56 and a bolt gun do not share a
/// report, and one file for all three was the last of the global settings.
///
/// Both files are cut by tools/make_weapon_sounds.sh. The reload is composed
/// against reload_duration below, so changing that number means re-running the
/// script or the mag hits land off the animation.
pub(crate) struct WeaponSound {
    pub(crate) fire: &'static str,
    pub(crate) reload: &'static str,
}

/// What the glass magnifies by. A short scope on a rifle and a sniper's optic
/// are not the same instrument, and a single global number made them so.
///
/// `default` is where a weapon starts and what the reticle is authored
/// against; the two bounds are what the wheel and the touch slider range over.
/// A weapon with no glass never shows a scope view, so its numbers only have
/// to be sane, not meaningful.
pub(crate) struct Scope {
    pub(crate) min_zoom: f32,
    pub(crate) default_zoom: f32,
    pub(crate) max_zoom: f32,
    /// The magnification the reticle mesh was drawn against. Its size on
    /// screen is proportional to this and to nothing else, so it is kept apart
    /// from default_zoom: winding a weapon up more powerful should not fatten
    /// its marks along with it.
    pub(crate) reticle_zoom: f32,
}

/// What a shot does to the view and to the weapon in the hands. The two halves
/// are separate numbers on purpose: how hard a weapon is to hold and how much
/// it visibly jumps are different things to tune.
pub(crate) struct Recoil {
    /// Radians of pitch per shot and the ceiling a burst climbs to. The
    /// ceiling also sets what bullet spread reads as heat.
    pub(crate) pitch_kick: f32,
    pub(crate) pitch_kick_max: f32,
    /// Sideways wander per shot, sign alternating so it wobbles rather than
    /// drifts.
    pub(crate) yaw_jitter: f32,
    /// Metres the eye slides along its own back per shot, and the ceiling a
    /// burst stacks to.
    pub(crate) push_back: f32,
    pub(crate) push_back_max: f32,
    /// Rate of the exponential both decay by.
    pub(crate) recovery_speed: f32,
    /// How hard the blow lands. The kick chases its mark at this rate, so it
    /// leaves fast and slows as it arrives rather than teleporting there --
    /// the impulse, and then the shoulder taking it. Well above the two
    /// recovery rates, or the strike reads as a drift.
    pub(crate) strike_speed: f32,

    /// How fast the weapon settles back after a shot. A rate, like
    /// recovery_speed above: the kick eases out rather than sliding home at a
    /// constant rate and stopping dead, which is the same curve the sights
    /// come up on.
    pub(crate) settle_speed: f32,
    pub(crate) kick_back: f32,
    pub(crate) kick_up: f32,
    /// Degrees the weapon turns. Roll and yaw flip sign per shot.
    pub(crate) tilt_up_deg: f32,
    pub(crate) roll_deg: f32,
    pub(crate) yaw_kick_deg: f32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) enum WeaponId {
    #[default]
    Ak47,
    M4a1,
    M24,
}

#[derive(Resource, Default)]
pub(crate) struct ActiveWeapon(pub(crate) WeaponId);

impl ActiveWeapon {
    pub(crate) fn def(&self) -> &'static WeaponDef {
        self.0.def()
    }
}

impl WeaponId {
    pub(crate) const ALL: [WeaponId; 3] = [WeaponId::Ak47, WeaponId::M4a1, WeaponId::M24];

    pub(crate) fn index(self) -> usize {
        match self {
            WeaponId::Ak47 => 0,
            WeaponId::M4a1 => 1,
            WeaponId::M24 => 2,
        }
    }

    pub(crate) fn def(self) -> &'static WeaponDef {
        match self {
            WeaponId::Ak47 => &AK47,
            WeaponId::M4a1 => &M4A1,
            WeaponId::M24 => &M24,
        }
    }

}

/// Standardised to match the M4: metres, barrel down -Z, origin on the bore at
/// the muzzle. The turn and the millimetres are baked into the asset's root
/// node, so the grips below are the Socket_RearGrip / Socket_FrontGrip the
/// mesh now ships with.
static AK47: WeaponDef = WeaponDef {
    name: "AK-47",
    mesh_path: "models/ak47_scope/raw/scene.gltf",
    rest_offset: Vec3::new(0.160, -0.0905, -0.7094),
    ready_offset: Vec3::new(0.045, -0.1905, -0.6194),
    ads_eye_relief: 0.032,
    ads_speed: 10.0,
    view_pullback: 0.13,
    // The floor is half the default, because that is where aim sensitivity,
    // which is default/zoom against a half speed base, comes back out at
    // exactly the hip fire rate.
    scope: Scope { min_zoom: 6.0, default_zoom: 12.0, max_zoom: 20.0, reticle_zoom: 4.0 },
    sound: WeaponSound { fire: "audio/weapons/ak47_fire.wav", reload: "audio/weapons/ak47_reload.wav" },
    reload_duration: 2.3,
    fire_rate: 0.1,
    magazine: 50,
    // 7.62: slower, heavier, looser at the top of a burst. Every axis above
    // the M4's.
    recoil: Recoil {
        pitch_kick: 0.0110,
        pitch_kick_max: 0.090,
        yaw_jitter: 0.0060,
        push_back: 0.009,
        push_back_max: 0.038,
        recovery_speed: 6.6,
        strike_speed: 60.0,

        settle_speed: 21.0,
        kick_back: 0.075,
        // Above the 12.8mm the 1.2 degree tilt drops the stock by over its own
        // 612mm lever, or the weapon reads as sinking at the shot rather than
        // rising: the tilt pivots on the muzzle, so it moves the tail.
        kick_up: 0.0180,
        tilt_up_deg: 1.2,
        roll_deg: 0.8,
        yaw_kick_deg: 0.7,
    },
};

/// No glass: a carry handle and a post, so it marks no scope sockets and the
/// scope view stays off for it.
///
/// rest_offset is anchored on Socket_RearGrip rather than on the origin,
/// because what holds still across a weapon swap is the firing hand. Anchoring
/// on the muzzle threw this 213mm-shorter rifle 208mm from the shoulder.
static M4A1: WeaponDef = WeaponDef {
    name: "M4A1",
    mesh_path: "models/m4a1/mesh/m4a1_rigged.glb",
    rest_offset: Vec3::new(0.160, -0.1268, -0.626),
    ready_offset: Vec3::new(0.071, -0.2268, -0.536),
    ads_eye_relief: 0.120,
    ads_speed: 12.5,
    view_pullback: 0.13,
    // Irons: the scope view never comes on, so these only have to be sane.
    scope: Scope { min_zoom: 1.0, default_zoom: 1.0, max_zoom: 1.0, reticle_zoom: 1.0 },
    sound: WeaponSound { fire: "audio/weapons/m4a1_fire.wav", reload: "audio/weapons/m4a1_reload.wav" },
    reload_duration: 1.9,
    fire_rate: 0.085,
    magazine: 30,
    // 5.56: about two thirds the AK's kick, settling faster. Cut by more than
    // a flat ratio because it also fires faster, and matching per shot would
    // have climbed harder for the same trigger hold.
    recoil: Recoil {
        pitch_kick: 0.0080,
        pitch_kick_max: 0.068,
        yaw_jitter: 0.0044,
        push_back: 0.0065,
        push_back_max: 0.028,
        recovery_speed: 7.4,
        strike_speed: 70.0,

        settle_speed: 23.0,
        kick_back: 0.055,
        kick_up: 0.0036,
        tilt_up_deg: 0.9,
        roll_deg: 0.6,
        yaw_kick_deg: 0.5,
    },
};

/// Standardised in Blender on import, where it arrived 29.8m long down +X with
/// non-uniform scale on a nested empty. The barrel radius after scaling came
/// out 10.5mm against the real rifle's 8.5mm, which is the check that the
/// 0.039 factor was right rather than merely plausible.
///
/// A bolt gun: it fires about as often as the others reload, so the recoil can
/// be heavy without a burst ever stacking it.
///
/// Anchored on the bore, not on Socket_RearGrip like the other two. Its grip
/// sits 68mm under the bore against the AK's 130mm, so matching hands dropped
/// the whole rifle 61mm and it read as carried low. A shouldered rifle holds
/// its bore at the eye, not its grip.
static M24: WeaponDef = WeaponDef {
    name: "M24",
    mesh_path: "models/m24/mesh/m24.glb",
    rest_offset: Vec3::new(0.160, -0.0905, -0.9770),
    ready_offset: Vec3::new(0.045, -0.1905, -0.8870),
    ads_eye_relief: 0.032,
    ads_speed: 6.5,
    // 268mm longer than the AK and it shows: the camera comes in 60mm to
    // frame it the way the shorter rifles already sit.
    view_pullback: 0.07,
    // Twice the AK at every stop: the glass it carries is the one thing this
    // rifle is for.
    scope: Scope { min_zoom: 12.0, default_zoom: 24.0, max_zoom: 40.0, reticle_zoom: 10.0 },
    sound: WeaponSound { fire: "audio/weapons/m24_fire.wav", reload: "audio/weapons/m24_reload.wav" },
    reload_duration: 3.1,
    fire_rate: 1.3,
    magazine: 5,
    recoil: Recoil {
        pitch_kick: 0.055,
        pitch_kick_max: 0.200,
        yaw_jitter: 0.012,
        push_back: 0.030,
        push_back_max: 0.075,
        recovery_speed: 2.6,
        strike_speed: 28.0,

        settle_speed: 5.5,
        kick_back: 0.115,
        kick_up: 0.014,
        tilt_up_deg: 6.0,
        roll_deg: 3.5,
        yaw_kick_deg: 3.0,
    },
};

/// Rebuilds the viewmodel for the weapon the player picked.
///
/// The mesh and the IK sockets are both built against one weapon's geometry,
/// so both are thrown away and made again rather than retargeted.
pub(crate) fn switch_weapon(
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut active: ResMut<ActiveWeapon>,
    mut zoom: ResMut<ScopeZoom>,
    mut gun: Query<(Entity, &mut Gun)>,
    mesh_parts: Query<Entity, With<GunMeshPart>>,
    sockets: Query<Entity, Or<(With<TriggerGripSocket>, With<ForeGripSocket>)>>,
) {
    let wanted = if keys.just_pressed(KeyCode::Digit1) {
        WeaponId::Ak47
    } else if keys.just_pressed(KeyCode::Digit2) {
        WeaponId::M4a1
    } else if keys.just_pressed(KeyCode::Digit3) {
        WeaponId::M24
    } else {
        return;
    };
    if wanted == active.0 {
        return;
    }

    let Ok((gun_entity, mut gun)) = gun.single_mut() else {
        return;
    };
    active.0 = wanted;
    let def = wanted.def();

    zoom.0 = def.scope.default_zoom;
    gun.base_offset = def.rest_offset;
    gun.ammo = def.magazine;
    gun.reload_remaining = 0.0;

    for entity in &mesh_parts {
        commands.entity(entity).despawn();
    }
    for entity in &sockets {
        commands.entity(entity).despawn();
    }
    spawn_gun_mesh_parts(&mut commands, &asset_server, &mut materials, gun_entity, def.mesh_path);
    info!("[weapon] switched to {}", def.name);
}

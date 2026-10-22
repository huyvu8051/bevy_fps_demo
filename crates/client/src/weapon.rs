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
    pub(crate) ads_speed: AdsSpeed,

    pub(crate) scope: Scope,
    pub(crate) sound: WeaponSound,
    /// How long the weapon takes to reload. A fifty round magazine and a five
    /// round bolt gun are not the same job.
    pub(crate) reload_duration: f32,
    pub(crate) fire_rate: f32,
    pub(crate) magazine: u32,
    pub(crate) recoil: Recoil,
}

/// How fast the sights come up. Weight and length: a carbine snaps up, a
/// scoped bolt gun does not.
///
/// Two speeds because the weapon starts from two places, and how much the
/// shorter trip saves is the weapon's own business -- a rifle held low and
/// wide gains more from being half way there than a carbine already tucked in.
/// It was one speed and a scale shared by all three.
///
/// A raise that begins part way between the two poses interpolates; dropping
/// the sights uses from_ready either way, because what the rest pose costs is
/// the distance up, not the distance back down.
pub(crate) struct AdsSpeed {
    pub(crate) from_ready: f32,
    pub(crate) from_rest: f32,
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

/// What a shot does, split by what it moves.
///
/// The weapon is a child of the camera, so the two halves never touch: nothing
/// under `view` shifts the gun on screen -- it drags the world past it -- and
/// nothing under `weapon` moves where the bullets go. One flat list of
/// thirteen numbers hid that, and `kick_back` sat next to `push_back` meaning
/// opposite halves of it.
pub(crate) struct Recoil {
    /// How hard the blow lands, on both halves. Every value below runs at the
    /// mark a shot set at this rate, so it leaves fast and slows as it
    /// arrives rather than teleporting there -- the impulse, and then the
    /// shoulder taking it. Well above either recovery rate, or the strike
    /// reads as a drift.
    pub(crate) strike_speed: f32,
    pub(crate) view: ViewKick,
    pub(crate) weapon: WeaponKick,
}

/// What a shot does to the camera: where the player is aiming afterwards.
pub(crate) struct ViewKick {
    /// Radians the aim climbs per shot, and the ceiling a burst stacks to.
    /// The ceiling also sets what bullet spread reads as heat.
    pub(crate) climb_per_shot: f32,
    pub(crate) climb_max: f32,
    /// Radians the aim wanders sideways per shot, sign alternating so it
    /// wobbles rather than drifts -- which is why it needs no ceiling.
    pub(crate) wander_per_shot: f32,
    /// Metres the camera slides along its own back per shot, and the ceiling.
    /// Its own back rather than a levelled one, so a climbing view is shoved
    /// up and back together.
    pub(crate) shove_per_shot: f32,
    pub(crate) shove_max: f32,
    /// How fast the marks above tend back to zero: the aim coming home.
    pub(crate) recover_speed: f32,
}

/// What a shot does to the weapon in the hands: how much it visibly jumps,
/// none of which moves the aim.
pub(crate) struct WeaponKick {
    /// Metres straight back towards the shoulder, and straight up. Plain
    /// translation, no pivot, so they move the whole weapon.
    pub(crate) slide_back: f32,
    pub(crate) lift: f32,
    /// Degrees about the three axes. Pitch turns about the mesh origin, which
    /// sits on the bore at the muzzle -- so it pins the nose and swings the
    /// tail down, rather than lifting the nose. Roll and yaw flip sign per
    /// shot.
    pub(crate) pitch_deg: f32,
    pub(crate) roll_deg: f32,
    pub(crate) yaw_deg: f32,
    /// How fast the weapon settles back after a shot. A rate, like
    /// recover_speed: the kick eases out rather than sliding home at a
    /// constant rate and stopping dead, which is the same curve the sights
    /// come up on.
    pub(crate) settle_speed: f32,
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
    ads_eye_relief: 0.01,
    ads_speed: AdsSpeed { from_ready: 8.0, from_rest: 5.5 },
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
        strike_speed: 60.0,
        view: ViewKick {
            climb_per_shot: 0.0110,
            climb_max: 0.090,
            wander_per_shot: 0.0060,
            shove_per_shot: 0.03,
            shove_max: 0.058,
            recover_speed: 6.6,
        },
        weapon: WeaponKick {
            slide_back: 0.055,
            lift: 0.0160,
            pitch_deg: 2.2,
            roll_deg: 0.8,
            yaw_deg: 0.7,
            settle_speed: 21.0,
        },
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
    ads_eye_relief: 0.10,
    ads_speed: AdsSpeed { from_ready: 16.0, from_rest: 9.0 },
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
        strike_speed: 70.0,
        view: ViewKick {
            climb_per_shot: 0.0080,
            climb_max: 0.068,
            wander_per_shot: 0.0044,
            shove_per_shot: 0.0095,
            shove_max: 0.038,
            recover_speed: 7.4,
        },
        weapon: WeaponKick {
            slide_back: 0.055,
            lift: 0.0170,
            pitch_deg: 2.4,
            roll_deg: 0.6,
            yaw_deg: 0.5,
            settle_speed: 23.0,
        },
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
    // Closest of the three by a wide margin: the eye comes right up behind the
    // rear glass, so the scope picture fills more of the view than either
    // carbine's sight. The glass itself sits 55mm further down the tube, so
    // there is still room before the near plane at 100mm.
    ads_eye_relief: -0.0,
    ads_speed: AdsSpeed { from_ready: 5.5, from_rest: 2.575 },
    // Twice the AK at every stop: the glass it carries is the one thing this
    // rifle is for.
    scope: Scope { min_zoom: 12.0, default_zoom: 24.0, max_zoom: 40.0, reticle_zoom: 10.0 },
    sound: WeaponSound { fire: "audio/weapons/m24_fire.wav", reload: "audio/weapons/m24_reload.wav" },
    reload_duration: 3.1,
    fire_rate: 1.3,
    magazine: 5,
    recoil: Recoil {
        strike_speed: 28.0,
        view: ViewKick {
            climb_per_shot: 0.055,
            climb_max: 0.200,
            wander_per_shot: 0.012,
            shove_per_shot: 0.4,
            shove_max: 0.925,
            recover_speed: 2.6,
        },
        weapon: WeaponKick {
            slide_back: 0.04,
            lift: 0.014,
            pitch_deg: 3.0,
            roll_deg: 3.5,
            yaw_deg: 3.0,
            settle_speed: 5.5,
        },
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

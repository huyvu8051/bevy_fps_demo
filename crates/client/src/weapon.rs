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
    /// Metres per unit of the mesh's own space, and the yaw that brings the
    /// mesh's length onto the camera's forward. Both are 1.0 and 0.0 for a
    /// mesh authored to the house convention: metres, barrel down -Z, origin
    /// on the bore at the muzzle. They exist for meshes that are not.
    pub(crate) mesh_scale: f32,
    pub(crate) mesh_yaw_deg: f32,

    pub(crate) rest_offset: Vec3,
    pub(crate) ready_offset: Vec3,
    /// How far the eye sits behind Socket_Aim when sighted: the rear glass on
    /// a scope, the rear sight on irons. The gun is then placed so that point
    /// lands on the camera axis, which is the whole of ads_offset.
    pub(crate) ads_eye_relief: f32,

    /// Camera relative, unlike everything else here. The muzzle is placed from
    /// the eye and the drift from rest_offset added on, so this has to be
    /// moved by hand whenever rest_offset is.
    pub(crate) muzzle_offset: Vec3,
    /// Added to the gun's own offset but along the camera's axes, not the
    /// gun's, so it tracks where the gun sits without taking its sway. Moving
    /// the mesh origin moves this with it.
    pub(crate) eject_port: Vec3,

    pub(crate) fire_rate: f32,
    pub(crate) magazine: u32,
    pub(crate) recoil: Recoil,
}

/// What a shot does to the view and to the weapon in the hands.
///
/// Kept apart from the rest of WeaponDef because it is the half a weapon is
/// felt by rather than seen by: two rifles can sit in exactly the same place
/// and still be told apart the moment the trigger is held.
///
/// The view half and the viewmodel half are deliberately separate numbers.
/// Driving both off one would tie how hard the gun is to hold to how much it
/// visibly jumps, and those are different things to tune.
pub(crate) struct Recoil {
    /// Radians of pitch added per shot, and the ceiling a held burst climbs
    /// to. The ceiling is also what bullet spread reads as heat, so raising
    /// it loosens the weapon's sustained accuracy as well as its climb.
    pub(crate) pitch_kick: f32,
    pub(crate) pitch_kick_max: f32,
    /// Sideways wander per shot. Its sign alternates, so it reads as a wobble
    /// rather than a drift off target.
    pub(crate) yaw_jitter: f32,
    /// Metres the eye is pushed back per shot, and its ceiling. Applied level
    /// rather than along the view, so pitch climb does not drag the eye down.
    pub(crate) push_back: f32,
    pub(crate) push_back_max: f32,
    /// Rate of the exponential all three decay by.
    pub(crate) recovery_speed: f32,

    /// Seconds one shot's weapon animation runs for.
    pub(crate) duration: f32,
    /// Metres the weapon travels back into the shoulder, and up.
    pub(crate) kick_back: f32,
    pub(crate) kick_up: f32,
    /// Degrees the weapon turns. Roll and yaw flip sign per shot so a burst
    /// never plays the same frame twice.
    pub(crate) tilt_up_deg: f32,
    pub(crate) roll_deg: f32,
    pub(crate) yaw_kick_deg: f32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) enum WeaponId {
    #[default]
    Ak47,
    M4a1,
}

#[derive(Resource, Default)]
pub(crate) struct ActiveWeapon(pub(crate) WeaponId);

impl ActiveWeapon {
    pub(crate) fn def(&self) -> &'static WeaponDef {
        self.0.def()
    }
}

impl WeaponId {
    pub(crate) fn def(self) -> &'static WeaponDef {
        match self {
            WeaponId::Ak47 => &AK47,
            WeaponId::M4a1 => &M4A1,
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
    mesh_scale: 1.0,
    mesh_yaw_deg: 0.0,
    rest_offset: Vec3::new(0.190, -0.054, -0.800),
    ready_offset: Vec3::new(0.045, -0.184, -0.710),
    ads_eye_relief: 0.195,
    muzzle_offset: Vec3::new(0.170, 0.005, -0.865),
    eject_port: Vec3::new(0.100, 0.009, 0.515),
    fire_rate: 0.1,
    magazine: 50,
    // The values the whole feel was originally tuned on, back when recoil was
    // global. A 7.62 rifle: slow, heavy, and loose at the top of a burst.
    recoil: Recoil {
        pitch_kick: 0.022,
        pitch_kick_max: 0.16,
        yaw_jitter: 0.012,
        push_back: 0.018,
        push_back_max: 0.07,
        recovery_speed: 5.0,

        duration: 0.12,
        kick_back: 0.12,
        kick_up: 0.01,
        tilt_up_deg: 4.0,
        roll_deg: 3.0,
        yaw_kick_deg: 3.0,
    },
};

/// Read off the sockets the mesh ships with rather than guessed:
///
///   Socket_Muzzle      (0.011,  0,      0    )
///   Socket_SightFront  (0,      0.058,  0.164)
///   Socket_FrontGrip   (0,      0,      0.276)
///   Socket_SightRear   (0,      0.057,  0.512)
///   Socket_RearGrip    (0,     -0.094,  0.531)
///
/// The muzzle sits at the origin and the stock runs back to +Z, so the mesh
/// already points down -Z and needs no turn at all. This is the convention the
/// AK was since standardised onto, so the two now read in the same frame.
///
/// This one carries no glass: a carry handle and a post, 350mm of sight radius
/// between them. There is no scope camera and no lens for it, only the gun
/// coming up — ads_offset lifts Socket_SightRear onto the camera axis at 0.35m
/// of eye relief.
///
/// rest_offset lands Socket_RearGrip exactly where the AK's sits, because the
/// thing that holds still across a weapon swap is the firing hand, not the
/// muzzle. Anchoring on the muzzle instead — which is what the origin happens
/// to be — threw this 213mm-shorter rifle 208mm away from the shoulder.
static M4A1: WeaponDef = WeaponDef {
    name: "M4A1",
    mesh_path: "models/m4a1/mesh/m4a1_rigged.glb",
    mesh_scale: 1.0,
    mesh_yaw_deg: 0.0,
    rest_offset: Vec3::new(0.190, -0.096, -0.686),
    ready_offset: Vec3::new(0.071, -0.226, -0.596),
    ads_eye_relief: 0.350,
    muzzle_offset: Vec3::new(0.201, -0.096, -0.686),
    eject_port: Vec3::new(0.06, 0.02, 0.33),
    fire_rate: 0.085,
    magazine: 30,
    // A 5.56 carbine against the AK's 7.62: roughly two thirds the kick in
    // every axis, and it settles faster. It also fires faster, so matching the
    // AK per shot would have climbed harder for the same trigger hold — the
    // point of the split is that a burst ends somewhere different, not just
    // that one shot looks different.
    recoil: Recoil {
        pitch_kick: 0.014,
        pitch_kick_max: 0.11,
        yaw_jitter: 0.007,
        push_back: 0.011,
        push_back_max: 0.045,
        recovery_speed: 6.5,

        duration: 0.10,
        kick_back: 0.075,
        kick_up: 0.006,
        tilt_up_deg: 2.5,
        roll_deg: 1.8,
        yaw_kick_deg: 1.6,
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
    mut gun: Query<(Entity, &mut Transform, &mut Gun)>,
    mesh_parts: Query<Entity, With<GunMeshPart>>,
    sockets: Query<Entity, Or<(With<TriggerGripSocket>, With<ForeGripSocket>)>>,
) {
    let wanted = if keys.just_pressed(KeyCode::Digit1) {
        WeaponId::Ak47
    } else if keys.just_pressed(KeyCode::Digit2) {
        WeaponId::M4a1
    } else {
        return;
    };
    if wanted == active.0 {
        return;
    }

    let Ok((gun_entity, mut transform, mut gun)) = gun.single_mut() else {
        return;
    };
    active.0 = wanted;
    let def = wanted.def();

    transform.rotation = Quat::from_rotation_y(def.mesh_yaw_deg.to_radians());
    transform.scale = Vec3::splat(def.mesh_scale);
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

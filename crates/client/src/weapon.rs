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

    /// Added to the gun's own offset but along the camera's axes, not the
    /// gun's, so it tracks where the gun sits without taking its sway. Moving
    /// the mesh origin moves this with it.
    pub(crate) eject_port: Vec3,

    pub(crate) fire_rate: f32,
    pub(crate) magazine: u32,
    pub(crate) recoil: Recoil,
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

    pub(crate) duration: f32,
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
    mesh_scale: 1.0,
    mesh_yaw_deg: 0.0,
    rest_offset: Vec3::new(0.160, -0.0905, -0.7094),
    ready_offset: Vec3::new(0.045, -0.1905, -0.6194),
    ads_eye_relief: 0.032,
    eject_port: Vec3::new(0.0410, 0.0086, 0.4905),
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

        duration: 0.11,
        kick_back: 0.054,
        kick_up: 0.0050,
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
    mesh_scale: 1.0,
    mesh_yaw_deg: 0.0,
    rest_offset: Vec3::new(0.160, -0.096, -0.626),
    ready_offset: Vec3::new(0.071, -0.196, -0.536),
    ads_eye_relief: 0.120,
    eject_port: Vec3::new(0.06, 0.02, 0.33),
    fire_rate: 0.085,
    magazine: 30,
    // 5.56: about two thirds the AK's kick, settling faster. Cut by more than
    // a flat ratio because it also fires faster, and matching per shot would
    // have climbed harder for the same trigger hold.
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
    mesh_scale: 1.0,
    mesh_yaw_deg: 0.0,
    rest_offset: Vec3::new(0.160, -0.0905, -0.9770),
    ready_offset: Vec3::new(0.045, -0.1905, -0.8870),
    ads_eye_relief: 0.032,
    // Eyeballed, like the other two. The mesh has no Socket_Eject yet.
    eject_port: Vec3::new(0.0525, 0.0158, 0.7560),
    fire_rate: 1.3,
    magazine: 5,
    recoil: Recoil {
        pitch_kick: 0.055,
        pitch_kick_max: 0.200,
        yaw_jitter: 0.012,
        push_back: 0.030,
        push_back_max: 0.075,
        recovery_speed: 4.0,

        duration: 0.25,
        kick_back: 0.160,
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
    mut gun: Query<(Entity, &mut Transform, &mut Gun)>,
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

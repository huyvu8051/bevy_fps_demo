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
    /// Metres per unit of the mesh's own space. The AK is modelled in
    /// millimetres, which is why its grip points read in the hundreds.
    pub(crate) mesh_scale: f32,
    /// Yaw that brings the mesh's own length onto the camera's forward.
    pub(crate) mesh_yaw_deg: f32,

    pub(crate) rest_offset: Vec3,
    pub(crate) ads_offset: Vec3,
    pub(crate) ready_offset: Vec3,

    /// Camera relative, unlike everything else here. The muzzle is placed from
    /// the eye and the drift from rest_offset added on, so this has to be
    /// moved by hand whenever rest_offset is.
    pub(crate) muzzle_offset: Vec3,
    pub(crate) scope_lens_offset: Vec3,
    /// Gun relative, so it follows the gun on its own.
    pub(crate) eject_port: Vec3,

    /// In the mesh's own units, which is the space the IK solver works in.
    pub(crate) trigger_grip_mesh: Vec3,
    pub(crate) fore_grip_mesh: Vec3,

    pub(crate) fire_rate: f32,
    pub(crate) magazine: u32,
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

/// Measured over many sessions against this mesh; see the comments on the
/// fields for what each one is relative to.
static AK47: WeaponDef = WeaponDef {
    name: "AK-47",
    mesh_path: "models/ak47_scope/raw/scene.gltf",
    mesh_scale: 0.001,
    mesh_yaw_deg: -90.0,
    rest_offset: Vec3::new(0.22, -0.32, -0.56),
    ads_offset: Vec3::new(0.0, -0.307, -0.35),
    ready_offset: Vec3::new(0.055, -0.40, -0.47),
    muzzle_offset: Vec3::new(0.2, -0.045, -1.11),
    scope_lens_offset: Vec3::new(0.0, 0.0, -0.2),
    eject_port: Vec3::new(0.10, 0.225, 0.03),
    trigger_grip_mesh: Vec3::new(160.0, 80.0, -130.0),
    fore_grip_mesh: Vec3::new(-50.0, 160.0, 100.0),
    fire_rate: 0.1,
    magazine: 50,
};

/// Read off the sockets the mesh ships with rather than guessed:
///
///   Socket_Muzzle     (0.011,  0,      0    )
///   Socket_FrontGrip  (0,      0,      0.276)
///   Socket_Sight      (0,      0.057,  0.512)
///   Socket_RearGrip   (0,     -0.094,  0.531)
///
/// The muzzle sits at the origin and the stock runs back to +Z, so the mesh
/// already points down -Z and needs no turn at all. It is in metres, so the
/// grips read in metres where the AK reads in hundreds.
///
/// rest_offset puts this muzzle exactly where the AK's tuned one is, and
/// ads_offset lifts Socket_Sight onto the camera axis at 0.35m of eye relief.
static M4A1: WeaponDef = WeaponDef {
    name: "M4A1",
    mesh_path: "models/m4a1/mesh/m4a1_rigged.glb",
    mesh_scale: 1.0,
    mesh_yaw_deg: 0.0,
    rest_offset: Vec3::new(0.189, -0.045, -1.110),
    ads_offset: Vec3::new(0.0, -0.057, -0.862),
    ready_offset: Vec3::new(0.05, -0.125, -1.020),
    muzzle_offset: Vec3::new(0.2, -0.045, -1.110),
    scope_lens_offset: Vec3::new(0.0, 0.0, -0.2),
    eject_port: Vec3::new(0.06, 0.02, 0.33),
    trigger_grip_mesh: Vec3::new(0.0, -0.094, 0.531),
    fore_grip_mesh: Vec3::new(0.0, 0.0, 0.276),
    fire_rate: 0.085,
    magazine: 30,
};

/// Rebuilds the viewmodel for the weapon the player picked.
///
/// The mesh and the IK sockets are both built against one weapon's geometry,
/// so both are thrown away and made again rather than retargeted: the grip
/// points live in the mesh's own units, and those units are not even the same
/// between these two.
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

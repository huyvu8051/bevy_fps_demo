use bevy::prelude::{EulerRot, Quat};

pub const ARENA_CAR_PATHS: [&str; 16] = [
    "props/car_sedan/mesh/car_sedan_light.glb",
    "props/car_van/mesh/car_van_light.glb",
    "props/car_pickup/mesh/car_pickup_light.glb",
    "props/car_vf3/mesh/car_vf3_light.glb",
    "props/car_suv/mesh/car_suv_light.glb",
    "props/car_hatchback/mesh/car_hatchback_light.glb",
    "props/car_sports/mesh/car_sports_light.glb",
    "props/car_taxi/mesh/car_taxi_light.glb",
    "props/car_police/mesh/car_police_light.glb",
    "props/car_ambulance/mesh/car_ambulance_light.glb",
    "props/car_delivery/mesh/car_delivery_light.glb",
    "props/car_towtruck/mesh/car_towtruck_light.glb",
    "props/car_wagon/mesh/car_wagon_light.glb",
    "props/car_motorcycle/mesh/car_motorcycle_light.glb",
    "props/car_muscle/mesh/car_muscle_light.glb",
    "props/car_minivan/mesh/car_minivan_light.glb",
];

pub fn arena_car_mesh_path(index: usize) -> &'static str {
    ARENA_CAR_PATHS[index]
}

/// `_q5` is the only tier that still ships; the decimated meshes and the
/// smaller textures were deleted.
pub fn arena_car_texture_paths(index: usize) -> (String, String, String) {
    let base = ARENA_CAR_PATHS[index];
    let dir = base.rsplit_once('/').map(|(dir, _)| dir).unwrap_or(base);
    let slot = |name: &str| format!("{dir}/textures/{name}_q5.jpg");
    (slot("base_color"), slot("normal"), slot("metallic_roughness"))
}

type CarPhysicsRow = (f32, f32, f32, f32, f32, f32, f32);

fn arena_car_physics_raw() -> &'static [CarPhysicsRow] {
    static DATA: std::sync::OnceLock<Vec<CarPhysicsRow>> = std::sync::OnceLock::new();
    DATA.get_or_init(|| {
        let text = crate::read_ron_data_file!("arena_car_physics.ron");
        ron::de::from_str(&text)
            .unwrap_or_else(|e| panic!("failed to parse arena car physics data: {e}"))
    })
    .as_slice()
}

pub struct ArenaCarPhysics {
    pub x: f32,
    pub z: f32,
    pub scale: f32,

    pub y: f32,

    pub rotation: Quat,
}

pub fn arena_car_physics() -> Vec<ArenaCarPhysics> {
    arena_car_physics_raw()
        .iter()
        .map(|&(scale, y, x, z, rx_deg, ry_deg, rz_deg)| ArenaCarPhysics {
            x,
            z,
            scale,
            y,
            rotation: Quat::from_euler(
                EulerRot::YXZ,
                ry_deg.to_radians(),
                rx_deg.to_radians(),
                rz_deg.to_radians(),
            ),
        })
        .collect()
}

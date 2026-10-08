
use bevy::prelude::{EulerRot, Quat, Vec3};

use crate::arena::ARENA_WALL_THICKNESS;
use crate::level_piece::{floor_panel, wall_panel, BoxPiece, PanelPiece};

pub const ITEM_GALLERY_ENTRIES: [(&str, Vec3); 10] = [
    ("Sedan (gara)", Vec3::new(1.9, 1.5, 4.6)),
    ("Ban lam viec don", Vec3::new(1.4, 0.75, 0.7)),
    ("1 o cubicle", Vec3::new(1.6, 1.2, 1.6)),
    ("Quay le tan", Vec3::new(2.2, 1.1, 0.6)),
    ("Tu do/locker", Vec3::new(0.4, 1.8, 0.5)),
    ("Cua don", Vec3::new(0.9, 2.1, 0.15)),
    ("Cua doi/kinh lon", Vec3::new(1.8, 2.4, 0.15)),
    ("Cau thang xoan oc (footprint)", Vec3::new(4.0, 3.0, 4.0)),
    ("Tuong ngoai (1 nhip 3m)", Vec3::new(3.0, 2.8, 1.0)),
    ("Tuong/vach noi that (1 nhip 3m)", Vec3::new(3.0, 2.8, 0.15)),
];

pub const ITEM_GALLERY_SPACING: f32 = 7.0;
pub const ITEM_GALLERY_HALF_SIZE_X: f32 =
    (ITEM_GALLERY_ENTRIES.len() as f32) * ITEM_GALLERY_SPACING / 2.0 + 5.0;

pub const ITEM_GALLERY_HALF_SIZE_Z: f32 = 20.0;
pub const ITEM_GALLERY_WALL_HEIGHT: f32 = 4.0;

pub fn item_gallery_positions() -> Vec<(f32, f32)> {
    let start_x = -(ITEM_GALLERY_ENTRIES.len() as f32 - 1.0) * ITEM_GALLERY_SPACING / 2.0;
    (0..ITEM_GALLERY_ENTRIES.len())
        .map(|i| (start_x + i as f32 * ITEM_GALLERY_SPACING, 0.0))
        .collect()
}

pub const ITEM_GALLERY_PLAYER_SPAWN: Vec3 = Vec3::new(0.0, 3.0, ITEM_GALLERY_HALF_SIZE_Z - 2.0);

pub fn item_gallery_player_spawn() -> Vec3 {
    let Ok(raw) = std::env::var("REVIEW_SPAWN") else {
        return ITEM_GALLERY_PLAYER_SPAWN;
    };
    let parts: Vec<f32> = raw.split(',').filter_map(|s| s.trim().parse().ok()).collect();
    match parts[..] {
        [x, y, z] => Vec3::new(x, y, z),
        _ => ITEM_GALLERY_PLAYER_SPAWN,
    }
}

pub fn item_gallery_level_pieces_computed() -> (Vec<BoxPiece>, Vec<PanelPiece>) {
    let half_x = ITEM_GALLERY_HALF_SIZE_X;
    let half_z = ITEM_GALLERY_HALF_SIZE_Z;
    let floor_color = [0.35, 0.35, 0.38];
    let wall_color = [0.5, 0.5, 0.52];

    let mut pieces: Vec<BoxPiece> = Vec::new();
    let mut panels: Vec<PanelPiece> = Vec::new();

    panels.push(floor_panel(Vec3::ZERO, half_x * 2.0, half_z * 2.0, ARENA_WALL_THICKNESS, floor_color, 0.9));

    let wall_span_x = half_x * 2.0 + ARENA_WALL_THICKNESS;
    let wall_span_z = half_z * 2.0 + ARENA_WALL_THICKNESS;
    let walls = [
        (Vec3::new(0.0, ITEM_GALLERY_WALL_HEIGHT / 2.0, -half_z), wall_span_x, ARENA_WALL_THICKNESS),
        (Vec3::new(0.0, ITEM_GALLERY_WALL_HEIGHT / 2.0, half_z), wall_span_x, ARENA_WALL_THICKNESS),
        (Vec3::new(-half_x, ITEM_GALLERY_WALL_HEIGHT / 2.0, 0.0), ARENA_WALL_THICKNESS, wall_span_z),
        (Vec3::new(half_x, ITEM_GALLERY_WALL_HEIGHT / 2.0, 0.0), ARENA_WALL_THICKNESS, wall_span_z),
    ];
    for (center, size_x, size_z) in walls {
        panels.push(wall_panel(center, size_x, size_z, ITEM_GALLERY_WALL_HEIGHT, wall_color, 0.8));
    }

    let ident = Quat::IDENTITY;
    for (i, (_label, size)) in ITEM_GALLERY_ENTRIES.iter().enumerate() {
        let (x, z) = item_gallery_positions()[i];

        let color = if i % 2 == 0 { [0.75, 0.55, 0.2] } else { [0.25, 0.45, 0.75] };
        pieces.push((Vec3::new(x, size.y / 2.0, z), ident, *size, color, 0.6));
    }

    (pieces, panels)
}

pub fn item_gallery_level_pieces() -> (Vec<BoxPiece>, Vec<PanelPiece>) {
    static DATA: std::sync::OnceLock<(Vec<BoxPiece>, Vec<PanelPiece>)> = std::sync::OnceLock::new();
    DATA.get_or_init(|| {
        let text = crate::read_ron_data_file!("map/3.ron");
        crate::level_piece::level_pieces_from_ron(&text)
    })
    .clone()
}

pub const ITEM_GALLERY_CAR_PATHS: [&str; 16] = [
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

pub fn item_gallery_car_mesh_path(index: usize, poly_quality: u8) -> String {
    let base = ITEM_GALLERY_CAR_PATHS[index];
    if poly_quality >= 5 {
        return base.to_string();
    }
    let poly_quality = poly_quality.max(1);
    let stem = base.strip_suffix(".glb").unwrap_or(base);
    format!("{stem}_p{poly_quality}.glb")
}

pub fn item_gallery_car_texture_paths(index: usize, texture_quality: u8) -> (String, String, String) {
    let base = ITEM_GALLERY_CAR_PATHS[index];
    let dir = base.rsplit_once('/').map(|(dir, _)| dir).unwrap_or(base);
    let quality = texture_quality.min(5);
    let slot = |name: &str| format!("{dir}/textures/{name}_q{quality}.jpg");
    (slot("base_color"), slot("normal"), slot("metallic_roughness"))
}

type CarPhysicsRow = (f32, f32, f32, f32, f32, f32, f32);

fn item_gallery_car_physics_raw() -> &'static [CarPhysicsRow] {
    static DATA: std::sync::OnceLock<Vec<CarPhysicsRow>> = std::sync::OnceLock::new();
    DATA.get_or_init(|| {
        let text = crate::read_ron_data_file!("item_gallery_car_physics.ron");
        ron::de::from_str(&text)
            .unwrap_or_else(|e| panic!("failed to parse item gallery car physics data: {e}"))
    })
    .as_slice()
}

pub struct ItemGalleryCarPhysics {
    pub x: f32,
    pub z: f32,
    pub scale: f32,

    pub y: f32,

    pub rotation: Quat,
}

pub fn item_gallery_car_physics() -> Vec<ItemGalleryCarPhysics> {
    item_gallery_car_physics_raw()
        .iter()
        .map(|&(scale, y, x, z, rx_deg, ry_deg, rz_deg)| ItemGalleryCarPhysics {
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

pub const ITEM_GALLERY_ZOMBIE_ROW_Z: [f32; 3] = [10.0, 5.0, 0.0];
pub const ITEM_GALLERY_ZOMBIE_SPACING: f32 = 1.8;

pub fn item_gallery_zombie_row(count: usize, z: f32) -> Vec<(f32, f32)> {
    let start_x = -(count.saturating_sub(1) as f32) * ITEM_GALLERY_ZOMBIE_SPACING / 2.0;
    (0..count)
        .map(|i| (start_x + i as f32 * ITEM_GALLERY_ZOMBIE_SPACING, z))
        .collect()
}

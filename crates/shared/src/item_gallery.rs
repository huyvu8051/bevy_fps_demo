// --- Scenario::ItemGallery — flat scale-reference rig, not a real map ---
//
// One of EVERY standard item size from `docs/maps/map01_dimensions.md`
// section 1 (the shared item-size library, not just Map 1's), laid out in
// a single grid row on a flat floor. Purpose: open this in the dev editor
// (EDITOR=1, F2 — see third_party/bevy_editor_pls) and eyeball each real-
// world-derived size next to the player capsule / a zombie BEFORE it gets
// committed into an actual room layout — cheaper to catch "that desk reads
// way too small in-engine" here than after a whole floor is built around
// wrong numbers.
//
// Each entry becomes 1 LevelBox (item_gallery_level_pieces below, called
// by both client and server — see level_piece.rs's module doc comment)
// — a plain cuboid at the item's stated (width, height, depth), no
// attempt at a real mesh/shape. Good enough to judge footprint and
// height; not meant to look like the real prop.

use bevy::prelude::{EulerRot, Quat, Vec3};

use crate::arena::ARENA_WALL_THICKNESS;
use crate::level_piece::{floor_panel, wall_panel, BoxPiece, PanelPiece};

/// (label, size) — label is for code-reading only (this crate has no text
/// rendering); it also becomes each box's line in the console log the
/// server prints on startup for this scenario, since there's no in-game
/// nameplate to read otherwise.
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

// Grid layout: 1 hàng ngang, cách nhau đều GALLERY_SPACING (đủ rộng hơn
// item to nhất — sedan dài 4.6m — để không đè lên nhau), tâm mỗi item đặt
// ngay trên mặt sàn (y = size.y / 2).
pub const ITEM_GALLERY_SPACING: f32 = 7.0;
pub const ITEM_GALLERY_HALF_SIZE_X: f32 =
    (ITEM_GALLERY_ENTRIES.len() as f32) * ITEM_GALLERY_SPACING / 2.0 + 5.0;
// 20 (was 10) — the flat floor needs to reach the 2nd row of meshy.ai
// test cars (client's spawn_item_gallery_test_props) at z=-14; 6m of
// margin left to the north wall.
pub const ITEM_GALLERY_HALF_SIZE_Z: f32 = 20.0;
pub const ITEM_GALLERY_WALL_HEIGHT: f32 = 4.0;

/// Toạ độ (x, z) tâm từng item — cùng thứ tự `ITEM_GALLERY_ENTRIES`, hàng
/// ngang qua Z=0, item đầu tiên ở cực Tây.
pub fn item_gallery_positions() -> Vec<(f32, f32)> {
    let start_x = -(ITEM_GALLERY_ENTRIES.len() as f32 - 1.0) * ITEM_GALLERY_SPACING / 2.0;
    (0..ITEM_GALLERY_ENTRIES.len())
        .map(|i| (start_x + i as f32 * ITEM_GALLERY_SPACING, 0.0))
        .collect()
}

// Player spawn: đứng lùi về cực Nam, nhìn dọc theo cả dãy item — đủ xa để
// thấy hết dãy khi mới vào, không kẹt giữa 2 item. Không spawn zombie/
// gameplay gì khác — rig này chỉ để đi lại quan sát bằng camera người
// chơi bình thường (+ editor F2 nếu cần), không phải 1 scenario chơi được.
pub const ITEM_GALLERY_PLAYER_SPAWN: Vec3 = Vec3::new(0.0, 3.0, ITEM_GALLERY_HALF_SIZE_Z - 2.0);

/// Quick-review override for ITEM_GALLERY_PLAYER_SPAWN — set env
/// `REVIEW_SPAWN="x,y,z"` (server-side, before starting `MAP_ID=5`) to
/// spawn directly next to whichever prop is being eyeballed instead of
/// walking/noclipping there from the default south-end spawn every time.
/// Falls back to ITEM_GALLERY_PLAYER_SPAWN if unset or malformed. See also
/// the client's REVIEW_YAW/REVIEW_PITCH (crates/client/src/lib.rs) for the
/// matching initial look-direction override.
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

/// Flat floor + 4 walls + 1 LevelBox per ITEM_GALLERY_ENTRIES — pure
/// data, no spawning (see map1_level_pieces's own doc comment for why:
/// same function, called by both client and server).
pub fn item_gallery_level_pieces() -> (Vec<BoxPiece>, Vec<PanelPiece>) {
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
        // Đổi màu xen kẽ theo chỉ số — không mang ý nghĩa gì, chỉ để dễ
        // phân biệt item liền kề nhau bằng mắt khi đứng xa.
        let color = if i % 2 == 0 { [0.75, 0.55, 0.2] } else { [0.25, 0.45, 0.75] };
        pieces.push((Vec3::new(x, size.y / 2.0, z), ident, *size, color, 0.6));
    }

    (pieces, panels)
}

// --- meshy.ai car test props — collision/physics layout -------------------
//
// Shared so client (visual mesh + collider, docs/PROP_IMPORT_CHECKLIST.md)
// and server (collider only — hit registration via SpatialQuery, movement
// blocking via MoveAndSlide) agree on where each car's collider actually
// is; drifting the two independently would silently break "bullets stop
// registering" or "player walks through" on whichever side didn't get the
// update.

/// glTF asset path per car — index i here is car i in
/// `item_gallery_car_physics()` / `item_gallery_car_positions()`. Lives in
/// `shared` (not client-only anymore) because the server now also loads
/// these meshes: both sides build the car collider with avian's
/// `ColliderConstructor::ConvexHullFromMesh` off the real geometry instead
/// of a hand-sized `Collider::cuboid`, so the path list has to be the one
/// source of truth. The first 4 (`car_sedan`..`car_vf3`) were exported
/// before Meshy's "Resize + Origin: Bottom" download option — that's why
/// their rows in the RON data file carry a scale/Y hack; everything after
/// `car_vf3` was exported with it already applied.
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

/// `(scale, y, x, z, rot_x_deg, rot_y_deg, rot_z_deg)` per car — the full
/// entity Transform placement (uniform scale + translation x/y/z + rotation
/// as X/Y/Z euler degrees, applied in `EulerRot::YXZ` order), same
/// order/count as `ITEM_GALLERY_CAR_PATHS` (index-matched). These match
/// each SceneRoot's own Transform exactly (see ITEM_GALLERY_CAR_PATHS's doc
/// for why the first 4 need a scale/Y hack). The collider is built from the
/// mesh (`ConvexHullFromMesh`), so there's no size data here — just
/// placement.
///
/// Loaded from `crates/client/assets/data/item_gallery_car_physics.ron`
/// (was a hardcoded const array here) instead of compiled in, so
/// hand-tuning a car's scale/position after eyeballing it in the dev
/// editor (F2) is "edit the file, rerun" instead of "edit code, rebuild".
/// Read once per process and cached — both client and server call
/// `item_gallery_car_physics()` at Startup, and the file never changes
/// mid-run.
///
/// Path is resolved from this crate's own `CARGO_MANIFEST_DIR` (baked in
/// at shared's compile time), NOT the process's CWD or either binary's
/// own Bevy `AssetPlugin.file_path` — those two differ between client and
/// server (see server/src/main.rs's own comment on why), but shared's
/// manifest dir is fixed regardless of which binary links it in, so this
/// one path works unchanged for both.
type CarPhysicsRow = (f32, f32, f32, f32, f32, f32, f32);

fn item_gallery_car_physics_raw() -> &'static [CarPhysicsRow] {
    static DATA: std::sync::OnceLock<Vec<CarPhysicsRow>> = std::sync::OnceLock::new();
    DATA.get_or_init(|| {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../client/assets/data/item_gallery_car_physics.ron");
        let text = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("failed to read item gallery car physics data at {path}: {e}"));
        ron::de::from_str(&text)
            .unwrap_or_else(|e| panic!("failed to parse item gallery car physics data at {path}: {e}"))
    })
    .as_slice()
}

// The cars' original layout was a computed 2×8 grid (start_x -21, 6m
// spacing, rows at z -6 / -14). That's now just the seed values baked into
// the RON file's x/z columns — nudge a car in the editor and F9 writes its
// new x/z straight back, no grid formula in the loop anymore.

/// Resolved per-car placement — `x`/`z`/`scale`/`y`/`rotation` all straight
/// from the RON file. Drives the client's visual `SceneRoot` and the
/// server's collider proxy; the collider shape itself comes from the mesh
/// (`ColliderConstructor::ConvexHullFromMesh` on both sides).
pub struct ItemGalleryCarPhysics {
    pub x: f32,
    pub z: f32,
    pub scale: f32,
    /// Rest-height Y translation for the entity itself (matches the
    /// SceneRoot's own Transform.y on the client).
    pub y: f32,
    /// Full entity rotation — RON stores it as X/Y/Z euler degrees, built
    /// here into a `Quat` (`EulerRot::YXZ`). Feed straight to
    /// `Transform::with_rotation`.
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

// --- Animation-showcase zombie rows (client-only, Scenario::ItemGallery) ---
//
// Two rows of stationary zombies between the player spawn (z ≈ +18) and the
// car rows (z −6 / −14), one row per zombie mesh kind, one zombie per
// animation clip of that kind. See crates/client's
// spawn_item_gallery_zombie_showcase.
pub const ITEM_GALLERY_ZOMBIE_ROW_Z: [f32; 2] = [10.0, 5.0];
pub const ITEM_GALLERY_ZOMBIE_SPACING: f32 = 1.8;

/// A single centered row of `count` evenly-spaced `(x, z)` slots.
pub fn item_gallery_zombie_row(count: usize, z: f32) -> Vec<(f32, f32)> {
    let start_x = -(count.saturating_sub(1) as f32) * ITEM_GALLERY_ZOMBIE_SPACING / 2.0;
    (0..count)
        .map(|i| (start_x + i as f32 * ITEM_GALLERY_ZOMBIE_SPACING, z))
        .collect()
}

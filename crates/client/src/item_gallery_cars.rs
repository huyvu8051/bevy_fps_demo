// --- meshy.ai car test props (Scenario::ItemGallery) — spawn + physics ---
//
// Self-contained slice of the item-gallery scale-reference rig
// (docs/PROP_IMPORT_CHECKLIST.md): spawns the meshy.ai car meshes as
// physics props (collision + gravity-settle + shake-on-hit) and nothing
// else. Pulled into its own plugin/module (rather than living inline in
// lib.rs/vfx.rs like it did originally) because it's genuinely orthogonal
// to the rest of gameplay — no other system depends on it, and nothing it
// does needs to run in a specific order relative to the master Update
// `.chain()` GamePlugin owns (spawn_item_gallery_test_props is called
// imperatively, not registered via add_systems; the shake-on-hit observer
// reacts to ShotFx independently of on_shot_fx's own observer — Bevy runs
// multiple observers per trigger type fine, no ordering promise needed
// here since the two don't touch the same components).

use bevy::prelude::*;
use bevy::render::render_resource::Face;
use crate::*;

/// Marks the 16 meshy.ai car test props spawned into Scenario::ItemGallery
/// (see spawn_item_gallery_test_props) so map_select_button can despawn
/// them on scenario switch same as LevelBox/LevelPanel — without this tag
/// they'd either never appear (menu-picked ItemGallery never runs setup()'s
/// Startup-only spawn) or linger forever after switching to another map.
#[derive(Component)]
pub(crate) struct ItemGalleryTestProp;

pub(crate) struct ItemGalleryCarsPlugin;

impl Plugin for ItemGalleryCarsPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(shake_item_gallery_car_on_hit)
            .add_systems(Update, force_backface_culling_on_car_materials);
        // Dev-only: F9 writes every car's live scale/Y back into
        // crates/client/assets/data/item_gallery_car_physics.ron so the
        // "nudge in the editor, eyeball, repeat" loop ends with a real
        // save instead of copying numbers out of the Inspector by hand.
        // Gated on EDITOR=1 like the bevy_editor_pls plugin itself
        // (crates/client/src/lib.rs) — no key handler at all in a normal
        // player build.
        if std::env::var("EDITOR").is_ok() {
            app.add_systems(Update, save_item_gallery_car_physics_on_key);
        }
    }
}

// Asset paths only — scale/Y/collider size all live in
// crates/client/assets/data/item_gallery_car_physics.ron now (loaded via
// shared::item_gallery_car_physics(); server needs those too, for its own
// collider-only copy; asset loading is client-only so the paths stay
// here). Same order/count as that data file — zipped by index below. The
// first 4 are early tests done BEFORE discovering the Meshy download
// dialog's own Resize+Origin:Bottom option (docs/PROP_IMPORT_CHECKLIST.md
// mục 1 step 4), hence their scale/Y hack in the shared array; every entry
// after car_vf3 was downloaded WITH Resize+Origin:Bottom already applied.
const ITEM_GALLERY_CAR_PATHS: [&str; 16] = [
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

// Spawned this much above each car's real rest height so gravity (see
// RigidBody::Dynamic below) drops it the rest of the way onto the floor
// collider itself, instead of trusting the hand-picked Y to be exactly
// right — fixes the "cars spawn sunk into the floor" bug that plain
// eyeballed Y offsets kept producing.
const ITEM_GALLERY_CAR_DROP_MARGIN: f32 = 0.15;

// Small enough that a single shot only rocks a car (translation/yaw
// unlocked, see spawn_item_gallery_test_props) rather than launching it —
// "rung lắc nhẹ nếu bị bắn trúng", not "propelled across the room". Tuned
// down from an initial 6.0 after rapid-fire testing showed impulses
// stacking faster than AngularDamping/LinearDamping could bleed them off,
// launching the car into the air after ~5 shots in under a second.
const ITEM_GALLERY_CAR_HIT_IMPULSE: f32 = 1.0;

/// Spawns the meshy.ai car test props (docs/PROP_IMPORT_CHECKLIST.md) into
/// Scenario::ItemGallery — called from both setup() (MAP_ID=5 at process
/// start) and map_select_button (ItemGallery picked from the in-game menu
/// after starting on a different map), so either path shows the same
/// props.
///
/// Each car is a RigidBody::Dynamic (gravity does the final drop onto the
/// floor, see ITEM_GALLERY_CAR_DROP_MARGIN) with rotation locked to X/Z
/// (never tips over) but not Y or translation, so a bullet impact (see
/// shake_item_gallery_car_on_hit below) can still visibly nudge/rock it —
/// "rung lắc nhẹ nếu bị bắn trúng". The collider itself is a child entity,
/// offset/sized from shared::item_gallery_car_physics() (already
/// pre-divided by scale — see that function's own doc comment for why),
/// matching the server's own collider-only copy
/// (spawn_item_gallery_car_colliders) so hit registration and movement
/// blocking agree with what's actually drawn here.
pub(crate) fn spawn_item_gallery_test_props(commands: &mut Commands, asset_server: &AssetServer) {
    for (path, physics) in ITEM_GALLERY_CAR_PATHS.iter().zip(shared::item_gallery_car_physics()) {
        // "props/car_sedan/mesh/car_sedan_light.glb" -> "car_sedan" — so the
        // dev editor's Hierarchy panel (bevy_editor_pls, F2) shows a real
        // label per car instead of an anonymous Entity id; picking-by-click
        // in the viewport is disabled in this vendored bevy_editor_pls
        // (hierarchy/picking.rs is commented out), so Name + scrolling the
        // Hierarchy list is the only way to find a specific car to focus
        // (select it, press F) without guessing entity ids.
        let name = path.split('/').nth(1).unwrap_or(path);
        commands
            .spawn((
                Name::new(name.to_string()),
                SceneRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(*path))),
                Transform::from_xyz(physics.x, physics.y + ITEM_GALLERY_CAR_DROP_MARGIN, physics.z)
                    .with_scale(Vec3::splat(physics.scale)),
                RigidBody::Dynamic,
                LockedAxes::new().lock_rotation_x().lock_rotation_z(),
                LinearDamping(8.0),
                AngularDamping(8.0),
                ItemGalleryTestProp,
            ))
            .with_children(|p| {
                p.spawn((
                    // Named for the same reason the parent is (see above) —
                    // otherwise the Hierarchy shows a bare "Entity (Nv0)" for
                    // the invisible collider box under each car.
                    Name::new(format!("{name}_collider")),
                    Transform::from_xyz(0.0, physics.collider_local_y, 0.0),
                    Collider::cuboid(physics.collider_size.x, physics.collider_size.y, physics.collider_size.z),
                ));
            });
    }
}

/// Independent observer on the same ShotFx trigger vfx.rs's on_shot_fx
/// reacts to (Bevy runs every observer registered for a trigger type, no
/// ordering promise needed since the two don't touch the same
/// components) — only cares about SurfaceHit, ignores ZombieHit entirely.
fn shake_item_gallery_car_on_hit(
    trigger: On<ShotFx>,
    spatial_query: SpatialQuery,
    collider_of: Query<&ColliderOf>,
    mut item_gallery_cars: Query<Forces, With<ItemGalleryTestProp>>,
) {
    let ShotFx::SurfaceHit { point, normal, .. } = *trigger.event() else {
        return;
    };
    // Nudge the query point slightly INTO the surface (off the exact
    // boundary, where point_intersections is unreliable) to find which
    // collider actually got hit, then walk ColliderOf up to the RigidBody
    // parent Forces lives on (the collider itself is a child entity, see
    // spawn_item_gallery_test_props above).
    let probe = point - normal.normalize_or_zero() * 0.05;
    let hit_bodies: Vec<Entity> = spatial_query
        .point_intersections(probe, &SpatialQueryFilter::default())
        .into_iter()
        .filter_map(|collider_entity| collider_of.get(collider_entity).ok().map(|c| c.body))
        .collect();
    for body in hit_bodies {
        if let Ok(mut forces) = item_gallery_cars.get_mut(body) {
            forces.apply_linear_impulse_at_point(-normal.normalize_or_zero() * ITEM_GALLERY_CAR_HIT_IMPULSE, point);
            break;
        }
    }
}

/// Meshy exports every car material with glTF's `doubleSided: true`, which
/// Bevy's glTF loader turns into `cull_mode: None` — every triangle gets
/// rasterized from both sides, including the far half of the mesh that's
/// always fully occluded by the near half on a closed car body anyway (no
/// open cabins/cockpits among these 16 — see PROP_IMPORT_CHECKLIST.md).
/// That's backface culling (a free ~2x fragment-shading win per car)
/// turned OFF for no benefit — this forces it back on for every material
/// that ends up on one of our car meshes specifically, without touching
/// doubleSided materials elsewhere (zombies/player/level geometry) that
/// might actually need both sides rendered.
///
/// Runs every frame (not once at spawn) because the glTF scene's mesh
/// entities — and their materials — don't exist yet the instant
/// spawn_item_gallery_test_props() returns; they stream in over the
/// following frames as the asset finishes loading, same reason
/// wire_zombie_hitbox_bones (crates/shared/src/zombie.rs) polls
/// Added<Name> instead of doing this once at spawn time.
fn force_backface_culling_on_car_materials(
    child_of: Query<&ChildOf>,
    item_gallery_cars: Query<(), With<ItemGalleryTestProp>>,
    new_materials: Query<(Entity, &MeshMaterial3d<StandardMaterial>), Added<MeshMaterial3d<StandardMaterial>>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    'material: for (mesh_entity, material_handle) in &new_materials {
        let mut current = mesh_entity;
        for _ in 0..20 {
            if item_gallery_cars.contains(current) {
                if let Some(material) = materials.get_mut(&material_handle.0) {
                    material.cull_mode = Some(Face::Back);
                }
                continue 'material;
            }
            let Ok(parent) = child_of.get(current) else {
                continue 'material;
            };
            current = parent.0;
        }
    }
}

// --- F9: save live car scale/Y back to the RON file (EDITOR=1 only) ------
//
// The RON tuple is (scale, y, width, height, depth). Only `scale` and `y`
// are things you tune by eyeballing a car in the editor, so those are the
// only two this reads off the live Transform; width/height/depth are the
// real-world collider size (shared::ItemGalleryCarPhysics::real_size) and
// get written straight back unchanged. Grid x/z isn't in the file at all
// (item_gallery_car_positions() computes it) — if a car has been dragged
// sideways in the editor that offset is logged and dropped, not saved.
//
// The file's header comment block is preserved verbatim (everything up to
// and including the lone `[` line); only the array body is regenerated,
// with the same `// name` trailing comments and column alignment the
// hand-written file had.

const ITEM_GALLERY_CAR_PHYSICS_RON: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/assets/data/item_gallery_car_physics.ron");

fn round4(v: f32) -> f32 {
    (v * 1.0e4).round() / 1.0e4
}

/// f32 -> RON float literal: shortest form, but always with a decimal
/// point (`1` is an int in RON and won't parse into an f32 field).
fn ron_f32(v: f32) -> String {
    let s = format!("{}", round4(v));
    if s.contains('.') { s } else { format!("{s}.0") }
}

fn save_item_gallery_car_physics_on_key(
    keys: Res<ButtonInput<KeyCode>>,
    cars: Query<(&Name, &Transform), With<ItemGalleryTestProp>>,
) {
    if !keys.just_pressed(KeyCode::F9) {
        return;
    }

    let live: std::collections::HashMap<&str, &Transform> =
        cars.iter().map(|(name, t)| (name.as_str(), t)).collect();

    // Same index order as the RON file and ITEM_GALLERY_CAR_PATHS.
    let defaults = shared::item_gallery_car_physics();
    let mut rows: Vec<(String, String)> = Vec::with_capacity(ITEM_GALLERY_CAR_PATHS.len());
    for (&path, phys) in ITEM_GALLERY_CAR_PATHS.iter().zip(&defaults) {
        let name = path.split('/').nth(1).unwrap_or(path);
        let Some(t) = live.get(name).copied() else {
            error!(
                "F9 save aborted: car '{name}' not in the scene \
                 (are you on Scenario::ItemGallery / MAP_ID=5?) — nothing written"
            );
            return;
        };

        let scale = t.scale.x;
        if (t.scale.y - scale).abs() > 1.0e-3 || (t.scale.z - scale).abs() > 1.0e-3 {
            warn!("car '{name}': non-uniform scale {:?} in editor — saving x ({scale}) only", t.scale);
        }
        if (t.translation.x - phys.x).abs() > 0.05 || (t.translation.z - phys.z).abs() > 0.05 {
            warn!(
                "car '{name}': moved to x/z ({:.2}, {:.2}) in editor, but grid x/z is \
                 computed (shared::item_gallery_car_positions), not stored — ignoring",
                t.translation.x, t.translation.z
            );
        }

        // Undo the spawn-time drop margin so the value round-trips: spawn
        // adds ITEM_GALLERY_CAR_DROP_MARGIN back on next run.
        let y = t.translation.y - ITEM_GALLERY_CAR_DROP_MARGIN;
        let sz = phys.real_size;
        let short = name.strip_prefix("car_").unwrap_or(name);
        rows.push((
            format!(
                "({}, {}, {}, {}, {}),",
                ron_f32(scale),
                ron_f32(y),
                ron_f32(sz.x),
                ron_f32(sz.y),
                ron_f32(sz.z),
            ),
            short.to_string(),
        ));
    }

    // Preserve the header: keep every line up to and including the lone `[`.
    let existing = match std::fs::read_to_string(ITEM_GALLERY_CAR_PHYSICS_RON) {
        Ok(s) => s,
        Err(e) => {
            error!("F9 save aborted: can't read {ITEM_GALLERY_CAR_PHYSICS_RON}: {e}");
            return;
        }
    };
    let header: String = existing
        .lines()
        .take_while(|l| l.trim() != "[")
        .map(|l| format!("{l}\n"))
        .collect();

    let width = rows.iter().map(|(core, _)| core.len()).max().unwrap_or(0);
    let mut out = header;
    out.push_str("[\n");
    for (core, short) in &rows {
        out.push_str(&format!("    {core:<width$} // {short}\n"));
    }
    out.push_str("]\n");

    match std::fs::write(ITEM_GALLERY_CAR_PHYSICS_RON, out) {
        Ok(()) => info!("saved {} cars -> {ITEM_GALLERY_CAR_PHYSICS_RON}", rows.len()),
        Err(e) => error!("F9 save failed writing {ITEM_GALLERY_CAR_PHYSICS_RON}: {e}"),
    }
}

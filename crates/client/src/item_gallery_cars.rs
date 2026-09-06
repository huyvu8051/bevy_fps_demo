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

// `i` into shared::ITEM_GALLERY_CAR_PATHS / shared::item_gallery_car_physics
// — spawn_item_gallery_test_props already needs this to build the right
// mesh/texture paths at spawn time; kept on the entity afterward so
// apply_item_gallery_car_distance_lod can rebuild those same paths at a
// DIFFERENT tier later without having to re-derive which car index this
// entity is (e.g. from Name, which is a display string, not meant to be
// parsed back).
#[derive(Component)]
pub(crate) struct ItemGalleryCarIndex(pub(crate) usize);

// The (texture, poly) tier this car's Mesh3d/MeshMaterial3d are CURRENTLY
// loaded at — lets apply_item_gallery_car_distance_lod skip re-issuing
// asset_server.load() every frame for a car whose band hasn't changed
// (only reload when the target tier actually differs from this).
#[derive(Component)]
pub(crate) struct ItemGalleryCarLodTier {
    pub(crate) texture: u8,
    pub(crate) poly: u8,
    // A texture swap in flight: handles already issued via asset_server.load,
    // not yet written into the material. Kept separate from `texture` above
    // (which only updates once the swap actually lands) so
    // apply_item_gallery_car_distance_lod can tell "still waiting on the
    // same target" apart from "target changed again, restart the load".
    pending_texture: Option<PendingCarTexture>,
}

struct PendingCarTexture {
    target: u8,
    base_color: Handle<Image>,
    normal: Handle<Image>,
    metallic_roughness: Handle<Image>,
}

// Settings-menu "Cars" cheat toggle — same "hide, don't despawn" shape as
// GunVisible (crate::movement_combat): a car still needs to exist (physics/
// nav obstacle/hit detection all stay live) for testing without it
// cluttering the view, unlike TextureQuality/PolyQuality which genuinely
// rebuild the entity at a different asset tier.
#[derive(Resource)]
pub(crate) struct CarsVisible(pub(crate) bool);
impl Default for CarsVisible {
    fn default() -> Self {
        Self(true)
    }
}

// Same resource-drives-a-component-every-frame shape as apply_map_render_
// toggles' own GunVisible handling — cheap enough (one Query, gated on
// Changed) not to need its own system split.
pub(crate) fn apply_cars_visible(
    visible: Res<CarsVisible>,
    mut cars: Query<&mut Visibility, With<ItemGalleryTestProp>>,
) {
    if !visible.is_changed() {
        return;
    }
    let target = if visible.0 { Visibility::Inherited } else { Visibility::Hidden };
    for mut visibility in &mut cars {
        if *visibility != target {
            *visibility = target;
        }
    }
}

pub(crate) struct ItemGalleryCarsPlugin;

impl Plugin for ItemGalleryCarsPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(shake_item_gallery_car_on_hit);
        // Dev-only: F9 writes every car's live scale/Y back into
        // crates/client/assets/data/item_gallery_car_physics.ron so the
        // "nudge in the editor, eyeball, repeat" loop ends with a real
        // save instead of copying numbers out of the Inspector by hand.
        // Gated on EDITOR=1 like the bevy_editor_pls plugin itself
        // (crates/client/src/lib.rs) — no key handler at all in a normal
        // player build. Also fully compiled out on wasm32: there's no
        // persistent filesystem to write back to in a browser sandbox (see
        // shared::read_ron_data_file!'s doc comment), so a web build ships
        // the RON files' contents frozen at build time regardless of
        // EDITOR — this system would just fail every save with an error
        // toast there, so it doesn't exist at all instead.
        #[cfg(not(target_arch = "wasm32"))]
        if std::env::var("EDITOR").is_ok() {
            app.init_resource::<F9SaveToast>()
                .add_systems(Startup, spawn_f9_toast_ui)
                .add_systems(
                    Update,
                    (
                        // .run_if(debug_keys_allowed) on all 4 key-reading
                        // ones — see that fn's own doc comment (lib.rs).
                        // Doesn't affect the normal F9-nudge workflow (done
                        // while just playing with EDITOR=1 set, F2 overlay
                        // closed); only actually disables these while the
                        // F2 overlay itself is open and could claim the
                        // same keys for something else later.
                        save_item_gallery_car_physics_on_key.run_if(debug_keys_allowed),
                        // Same F9 keypress also saves the soldier model's
                        // own tuning (movement_combat.rs) — Comma/Period/
                        // Minus/Equal nudge that one live first, see
                        // nudge_player_model_tuning's own comment. (Zombie
                        // meshes no longer have an F9-tunable RON transform
                        // — scale/offset/rotation get baked into the .glb
                        // instead, see scripts/bake_mesh_transform.py.)
                        nudge_player_model_tuning.run_if(debug_keys_allowed),
                        save_player_model_tuning.run_if(debug_keys_allowed),
                        tick_f9_toast,
                    ),
                );
        }
    }
}

// Car glTF paths moved to shared::ITEM_GALLERY_CAR_PATHS — the server now
// loads the same meshes to build its own ConvexHullFromMesh collider, so
// the list can't stay client-only. Index i there == car i in
// shared::item_gallery_car_physics() and in the RON data file.

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

// normal/metallic-roughness textures carry linear (not sRGB) data — every
// other texture load in this project goes through bevy_gltf, which
// already sets this correctly per glTF's own material role; loading these
// as plain standalone images instead needs it spelled out by hand or the
// normal map comes out with visibly wrong (gamma-shifted) lighting.
// Shared by spawn_item_gallery_test_props and
// apply_item_gallery_car_distance_lod — both build a car material by hand
// from the same 3 texture roles.
fn load_linear_car_texture(asset_server: &AssetServer, path: String) -> Handle<Image> {
    asset_server.load_with_settings(path, |settings: &mut bevy::image::ImageLoaderSettings| {
        settings.is_srgb = false;
    })
}

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
/// "rung lắc nhẹ nếu bị bắn trúng".
///
/// The collider is generated by avian from the car's real glTF geometry
/// (ColliderConstructor::ConvexHullFromMesh — one convex hull per mesh
/// primitive in the scene, attached once the asset finishes loading), not
/// a hand-sized box: it lands on the mesh child entities and avian scales
/// it by the root's Transform.scale, so it matches whatever's actually
/// drawn regardless of the per-car scale hacks. The server builds the same
/// hull off the same meshes (spawn_item_gallery_car_colliders) so hit
/// registration and movement blocking agree with the client.
// `poly_quality`/`texture_quality`: 1..=5 / 0..=5 — two INDEPENDENT axes
// (see shared::item_gallery_car_mesh_path's own doc comment for why
// they're separate files rather than a combined `_qN.glb`). The mesh is
// loaded as a bare `Handle<Mesh>` (GltfAssetLabel::Primitive, not
// GltfAssetLabel::Scene — a poly-tier file has no material/scene graph
// to speak of, see bake_poly_quality_levels.py) and the material is built
// by hand from the separately-tiered texture images, so this spawns one
// flat entity instead of a glTF scene subtree. `ColliderConstructor`
// (not …Hierarchy) attaches straight to this same entity's own Mesh3d —
// same collider regardless of either quality tier, since
// `ConvexHullFromMesh` only needs A geometry, not the highest-detail one.
pub(crate) fn spawn_item_gallery_test_props(
    commands: &mut Commands,
    asset_server: &AssetServer,
    materials: &mut Assets<StandardMaterial>,
    poly_quality: u8,
    texture_quality: u8,
) {
    for (i, (path, physics)) in shared::ITEM_GALLERY_CAR_PATHS.iter().zip(shared::item_gallery_car_physics()).enumerate() {
        // "props/car_sedan/mesh/car_sedan_light.glb" -> "car_sedan" — so the
        // dev editor's Hierarchy panel (bevy_editor_pls, F2) shows a real
        // label per car instead of an anonymous Entity id; picking-by-click
        // in the viewport is disabled in this vendored bevy_editor_pls
        // (hierarchy/picking.rs is commented out), so Name + scrolling the
        // Hierarchy list is the only way to find a specific car to focus
        // (select it, press F) without guessing entity ids.
        let name = path.split('/').nth(1).unwrap_or(path);
        let mesh_path = shared::item_gallery_car_mesh_path(i, poly_quality);
        let (base_color_path, normal_path, metallic_roughness_path) = shared::item_gallery_car_texture_paths(i, texture_quality);
        let material = materials.add(StandardMaterial {
            base_color_texture: Some(asset_server.load(base_color_path)),
            normal_map_texture: Some(load_linear_car_texture(asset_server, normal_path)),
            metallic_roughness_texture: Some(load_linear_car_texture(asset_server, metallic_roughness_path)),
            // Matches the old force_backface_culling_on_car_materials
            // system's override of the source glb's own doubleSided:true
            // material (which defaults to no culling) — kept as a
            // straight construction-time value now that this project
            // builds the material itself instead of importing the glTF's.
            cull_mode: Some(Face::Back),
            ..default()
        });
        commands.spawn((
            Name::new(name.to_string()),
            Mesh3d(asset_server.load(GltfAssetLabel::Primitive { mesh: 0, primitive: 0 }.from_asset(mesh_path))),
            MeshMaterial3d(material),
            Transform::from_xyz(physics.x, physics.y + ITEM_GALLERY_CAR_DROP_MARGIN, physics.z)
                .with_rotation(physics.rotation)
                .with_scale(Vec3::splat(physics.scale)),
            RigidBody::Dynamic,
            ColliderConstructor::ConvexHullFromMesh,
            LockedAxes::new().lock_rotation_x().lock_rotation_z(),
            LinearDamping(8.0),
            AngularDamping(8.0),
            ItemGalleryTestProp,
            ItemGalleryCarIndex(i),
            ItemGalleryCarLodTier { texture: texture_quality, poly: poly_quality, pending_texture: None },
        ));
    }
}

// Per-car distance-based auto LOD (see DistanceLodEnabled's own doc
// comment in lib.rs for the overall feature). Unlike
// respawn_item_gallery_cars_on_quality_change (which despawns/respawns
// every car at once for a manual settings-menu change), this swaps a
// single car's Mesh3d/material textures IN PLACE — the entity, its
// physics body, and its collider all stay exactly as they are (gravity-
// settled position, any bullet-impact rocking) since only the visual
// tier is changing, not the car itself. The collider (built once at
// spawn from ColliderConstructor::ConvexHullFromMesh) is deliberately
// left alone too: hit registration is server-authoritative (see
// spawn_item_gallery_test_props' own doc comment), so a slightly stale
// collision hull from a higher-poly tier is a non-issue, and regenerating
// it every band change would be pure churn for a purely cosmetic swap.
pub(crate) fn apply_item_gallery_car_distance_lod(
    enabled: Res<DistanceLodEnabled>,
    bands: Res<LodBandDistance>,
    texture_levels: Res<LodCarTextureLevel>,
    poly_levels: Res<LodCarPolyLevel>,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    player: Query<&Transform, With<Player>>,
    mut cars: Query<(&Transform, &ItemGalleryCarIndex, &mut ItemGalleryCarLodTier, &mut Mesh3d, &MeshMaterial3d<StandardMaterial>), With<ItemGalleryTestProp>>,
) {
    if !enabled.0 {
        return;
    }
    let Ok(player_transform) = player.single() else {
        return;
    };
    let player_pos = player_transform.translation.xz();
    for (transform, index, mut tier, mut mesh, material_handle) in &mut cars {
        let distance = (transform.translation.xz() - player_pos).length();
        let band = bands.band_of(distance);
        let target_texture = texture_levels.0[band];
        let target_poly = poly_levels.0[band];
        if tier.texture == target_texture && tier.poly == target_poly && tier.pending_texture.is_none() {
            continue;
        }
        // Poly (mesh) swap stays immediate — a hard cut in silhouette reads
        // as ordinary LOD popping. Texture swap does NOT: assigning a
        // freshly-issued (not-yet-loaded) Handle<Image> straight into the
        // material makes the whole car disappear for the ~0.5s the image
        // takes to decode, because Bevy's render extraction skips drawing a
        // mesh whose material references an image that isn't loaded yet —
        // it's not a visible placeholder, the car just vanishes. So the
        // texture swap below is staged through `pending_texture` and only
        // actually written into the material once every handle involved is
        // confirmed ready — the car keeps showing its CURRENT (old) texture
        // right up to that instant, no gap.
        if tier.poly != target_poly {
            mesh.0 = asset_server.load(GltfAssetLabel::Primitive { mesh: 0, primitive: 0 }.from_asset(shared::item_gallery_car_mesh_path(index.0, target_poly)));
            tier.poly = target_poly;
        }
        if tier.texture != target_texture {
            let already_loading = tier.pending_texture.as_ref().is_some_and(|p| p.target == target_texture);
            if !already_loading {
                let (base_color_path, normal_path, metallic_roughness_path) = shared::item_gallery_car_texture_paths(index.0, target_texture);
                tier.pending_texture = Some(PendingCarTexture {
                    target: target_texture,
                    base_color: asset_server.load(base_color_path),
                    normal: load_linear_car_texture(&asset_server, normal_path),
                    metallic_roughness: load_linear_car_texture(&asset_server, metallic_roughness_path),
                });
            }
        }
        if let Some(pending) = tier.pending_texture.take() {
            let ready = asset_server.is_loaded_with_dependencies(pending.base_color.id())
                && asset_server.is_loaded_with_dependencies(pending.normal.id())
                && asset_server.is_loaded_with_dependencies(pending.metallic_roughness.id());
            if ready {
                if let Some(material) = materials.get_mut(&material_handle.0) {
                    material.base_color_texture = Some(pending.base_color);
                    material.normal_map_texture = Some(pending.normal);
                    material.metallic_roughness_texture = Some(pending.metallic_roughness);
                }
                tier.texture = pending.target;
            } else {
                tier.pending_texture = Some(pending);
            }
        }
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

// force_backface_culling_on_car_materials used to live here — no longer
// needed (2026-09-04) now that spawn_item_gallery_test_props builds each
// car's StandardMaterial by hand instead of importing the glTF's own (see
// its own doc comment): cull_mode is just set directly at construction
// now, so there's no glTF-imported doubleSided:true material left to
// override after the fact.

// --- F9: save live car Transform back to the RON file (EDITOR=1 only) ----
//
// The RON tuple is (scale, y, x, z, rot_x_deg, rot_y_deg, rot_z_deg) — the
// whole entity placement, all read straight off the live Transform. Nudge /
// rotate / scale a car in the editor, press F9, done.
//
// The file's header comment block is preserved verbatim (everything up to
// and including the lone `[` line); only the array body is regenerated,
// with the same `// name` trailing comments and column alignment the
// hand-written file had.

// Everything below (the F9 EDITOR=1 save path) is wasm32-gated at
// definition, not just at ItemGalleryCarsPlugin::build's registration —
// see that fn's own doc comment for why a web build can't use any of this
// at all (no persistent filesystem to write back to).
#[cfg(not(target_arch = "wasm32"))]
const ITEM_GALLERY_CAR_PHYSICS_RON: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/assets/data/item_gallery_car_physics.ron");

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn round4(v: f32) -> f32 {
    (v * 1.0e4).round() / 1.0e4
}

/// f32 -> RON float literal: shortest form, but always with a decimal
/// point (`1` is an int in RON and won't parse into an f32 field). Shared
/// with save_player_model_tuning (movement_combat.rs).
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn ron_f32(v: f32) -> String {
    let s = format!("{}", round4(v));
    if s.contains('.') { s } else { format!("{s}.0") }
}

// --- On-screen F9-save feedback (EDITOR=1 only) --------------------------
//
// The F9 handlers already info!/error! log, but you're looking at the game
// window in the editor, not the terminal — so both handlers also drop a
// line here and tick_f9_toast flashes it bottom-centre for a couple of
// seconds.
#[derive(Resource, Default)]
#[cfg(not(target_arch = "wasm32"))]
pub(crate) struct F9SaveToast {
    text: String,
    ttl: f32,
    is_error: bool,
}

#[cfg(not(target_arch = "wasm32"))]
impl F9SaveToast {
    pub(crate) fn ok(&mut self, msg: impl Into<String>) {
        self.text = msg.into();
        self.ttl = 2.5;
        self.is_error = false;
    }
    pub(crate) fn err(&mut self, msg: impl Into<String>) {
        self.text = msg.into();
        self.ttl = 4.0;
        self.is_error = true;
    }
    /// Append `extra` to a success line still showing from earlier THIS
    /// frame (so "saved N cars" becomes "saved N cars + meshy zombie"),
    /// otherwise show `standalone`.
    pub(crate) fn ok_also(&mut self, standalone: &str, extra: &str) {
        if self.ttl > 0.0 && !self.is_error {
            self.text.push_str(extra);
            self.ttl = 2.5;
        } else {
            self.ok(standalone);
        }
    }
}

#[derive(Component)]
#[cfg(not(target_arch = "wasm32"))]
struct F9ToastText;

#[cfg(not(target_arch = "wasm32"))]
fn spawn_f9_toast_ui(mut commands: Commands) {
    commands.spawn((
        Text::new(""),
        TextFont { font_size: 18.0, ..default() },
        TextColor(Color::srgb(0.6, 1.0, 0.6)),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(48.0),
            left: Val::Percent(50.0),
            margin: UiRect::left(Val::Px(-160.0)),
            width: Val::Px(320.0),
            padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
            justify_content: JustifyContent::Center,
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.65)),
        Visibility::Hidden,
        F9ToastText,
    ));
}

#[cfg(not(target_arch = "wasm32"))]
fn tick_f9_toast(
    time: Res<Time>,
    mut toast: ResMut<F9SaveToast>,
    mut q: Query<(&mut Text, &mut Visibility, &mut TextColor), With<F9ToastText>>,
) {
    let Ok((mut text, mut vis, mut color)) = q.single_mut() else {
        return;
    };
    if toast.ttl > 0.0 {
        toast.ttl -= time.delta_secs();
        if text.0 != toast.text {
            text.0 = toast.text.clone();
        }
        color.0 = if toast.is_error {
            Color::srgb(1.0, 0.6, 0.5)
        } else {
            Color::srgb(0.6, 1.0, 0.6)
        };
        *vis = Visibility::Inherited;
    } else if *vis != Visibility::Hidden {
        *vis = Visibility::Hidden;
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn save_item_gallery_car_physics_on_key(
    keys: Res<ButtonInput<KeyCode>>,
    cars: Query<(&Name, &Transform), With<ItemGalleryTestProp>>,
    mut toast: ResMut<F9SaveToast>,
) {
    if !keys.just_pressed(KeyCode::F9) {
        return;
    }

    let live: std::collections::HashMap<&str, &Transform> =
        cars.iter().map(|(name, t)| (name.as_str(), t)).collect();

    // Same index order as the RON file and shared::ITEM_GALLERY_CAR_PATHS.
    let mut rows: Vec<(String, String)> = Vec::with_capacity(shared::ITEM_GALLERY_CAR_PATHS.len());
    for &path in &shared::ITEM_GALLERY_CAR_PATHS {
        let name = path.split('/').nth(1).unwrap_or(path);
        let Some(t) = live.get(name).copied() else {
            error!(
                "F9 save aborted: car '{name}' not in the scene \
                 (are you on Scenario::ItemGallery / MAP_ID=5?) — nothing written"
            );
            toast.err(format!("F9: car '{name}' not in scene — nothing saved"));
            return;
        };

        let scale = t.scale.x;
        if (t.scale.y - scale).abs() > 1.0e-3 || (t.scale.z - scale).abs() > 1.0e-3 {
            warn!("car '{name}': non-uniform scale {:?} in editor — saving x ({scale}) only", t.scale);
        }

        // Same EulerRot::YXZ split the spawn rebuilds from — returns
        // (y_angle, x_angle, z_angle).
        let (ry, rx, rz) = t.rotation.to_euler(EulerRot::YXZ);

        // Undo the spawn-time drop margin so Y round-trips: spawn adds
        // ITEM_GALLERY_CAR_DROP_MARGIN back on next run. x/z/rotation have
        // no such fudge — saved as-is.
        let y = t.translation.y - ITEM_GALLERY_CAR_DROP_MARGIN;
        let short = name.strip_prefix("car_").unwrap_or(name);
        rows.push((
            format!(
                "({}, {}, {}, {}, {}, {}, {}),",
                ron_f32(scale),
                ron_f32(y),
                ron_f32(t.translation.x),
                ron_f32(t.translation.z),
                ron_f32(rx.to_degrees()),
                ron_f32(ry.to_degrees()),
                ron_f32(rz.to_degrees()),
            ),
            short.to_string(),
        ));
    }

    // Preserve the header: keep every line up to and including the lone `[`.
    let existing = match std::fs::read_to_string(ITEM_GALLERY_CAR_PHYSICS_RON) {
        Ok(s) => s,
        Err(e) => {
            error!("F9 save aborted: can't read {ITEM_GALLERY_CAR_PHYSICS_RON}: {e}");
            toast.err(format!("F9: can't read car RON — {e}"));
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
        Ok(()) => {
            info!("saved {} cars -> {ITEM_GALLERY_CAR_PHYSICS_RON}", rows.len());
            toast.ok(format!("F9: saved {} cars", rows.len()));
        }
        Err(e) => {
            error!("F9 save failed writing {ITEM_GALLERY_CAR_PHYSICS_RON}: {e}");
            toast.err(format!("F9: car save FAILED — {e}"));
        }
    }
}

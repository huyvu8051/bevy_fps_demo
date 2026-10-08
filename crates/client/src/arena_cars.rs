
use bevy::prelude::*;
use bevy::render::render_resource::Face;
use crate::*;

#[derive(Component)]
pub(crate) struct ArenaCarProp;

#[derive(Component)]
pub(crate) struct ArenaCarIndex(pub(crate) usize);

#[derive(Component)]
pub(crate) struct ArenaCarLodTier {

    active_band: Option<usize>,

    pool_poly_levels: [u8; 3],
    pool_texture_levels: [u8; 3],
    pool_ready: bool,
    mesh_pool: [Handle<Mesh>; 3],
    texture_pool: [(Handle<Image>, Handle<Image>, Handle<Image>); 3],
}

impl ArenaCarLodTier {

    const UNBUILT: [u8; 3] = [u8::MAX; 3];

    fn new() -> Self {
        Self {
            active_band: None,
            pool_poly_levels: Self::UNBUILT,
            pool_texture_levels: Self::UNBUILT,
            pool_ready: false,
            mesh_pool: std::array::from_fn(|_| Handle::default()),
            texture_pool: std::array::from_fn(|_| (Handle::default(), Handle::default(), Handle::default())),
        }
    }
}

#[derive(Resource)]
pub(crate) struct CarsVisible(pub(crate) bool);
impl Default for CarsVisible {
    fn default() -> Self {
        Self(true)
    }
}

pub(crate) fn apply_cars_visible(
    visible: Res<CarsVisible>,
    mut cars: Query<&mut Visibility, With<ArenaCarProp>>,
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

pub(crate) struct ArenaCarsPlugin;

impl Plugin for ArenaCarsPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(shake_arena_car_on_hit);

        if std::env::var("EDITOR").is_ok() {
            app.init_resource::<F9SaveToast>()
                .add_systems(Startup, spawn_f9_toast_ui)
                .add_systems(
                    Update,
                    (

                        save_arena_car_physics_on_key.run_if(debug_keys_allowed),

                        nudge_player_model_tuning.run_if(debug_keys_allowed),
                        save_player_model_tuning.run_if(debug_keys_allowed),
                        tick_f9_toast,
                    ),
                );
        }
    }
}

const ARENA_CAR_DROP_MARGIN: f32 = 0.15;

const ARENA_CAR_HIT_IMPULSE: f32 = 1.0;

fn load_linear_car_texture(asset_server: &AssetServer, path: String) -> Handle<Image> {
    asset_server.load_with_settings(path, |settings: &mut bevy::image::ImageLoaderSettings| {
        settings.is_srgb = false;
    })
}

pub(crate) fn spawn_arena_car_props(
    commands: &mut Commands,
    asset_server: &AssetServer,
    materials: &mut Assets<StandardMaterial>,
    poly_quality: u8,
    texture_quality: u8,
) {
    for (i, (path, physics)) in shared::ARENA_CAR_PATHS.iter().zip(shared::arena_car_physics()).enumerate() {

        let name = path.split('/').nth(1).unwrap_or(path);
        let mesh_path = shared::arena_car_mesh_path(i, poly_quality);
        let (base_color_path, normal_path, metallic_roughness_path) = shared::arena_car_texture_paths(i, texture_quality);
        let material = materials.add(StandardMaterial {
            base_color_texture: Some(asset_server.load(base_color_path)),
            normal_map_texture: Some(load_linear_car_texture(asset_server, normal_path)),
            metallic_roughness_texture: Some(load_linear_car_texture(asset_server, metallic_roughness_path)),

            cull_mode: Some(Face::Back),
            ..default()
        });
        commands.spawn((
            Name::new(name.to_string()),
            Mesh3d(asset_server.load(GltfAssetLabel::Primitive { mesh: 0, primitive: 0 }.from_asset(mesh_path))),
            MeshMaterial3d(material),
            Transform::from_xyz(physics.x, physics.y + ARENA_CAR_DROP_MARGIN, physics.z)
                .with_rotation(physics.rotation)
                .with_scale(Vec3::splat(physics.scale)),
            RigidBody::Dynamic,
            ColliderConstructor::ConvexHullFromMesh,
            LockedAxes::new().lock_rotation_x().lock_rotation_z(),
            LinearDamping(8.0),
            AngularDamping(8.0),
            ArenaCarProp,
            ArenaCarIndex(i),
            ArenaCarLodTier::new(),
        ));
    }
}

pub(crate) fn apply_arena_car_distance_lod(
    enabled: Res<DistanceLodEnabled>,
    bands: Res<LodBandDistance>,
    texture_levels: Res<LodCarTextureLevel>,
    poly_levels: Res<LodCarPolyLevel>,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    player: Query<&Transform, With<Player>>,
    mut cars: Query<(&Transform, &ArenaCarIndex, &mut ArenaCarLodTier, &mut Mesh3d, &MeshMaterial3d<StandardMaterial>), With<ArenaCarProp>>,
) {
    if !enabled.0 {
        return;
    }
    let Ok(player_transform) = player.single() else {
        return;
    };
    let player_pos = player_transform.translation.xz();
    for (transform, index, mut tier, mut mesh, material_handle) in &mut cars {
        if tier.pool_poly_levels != poly_levels.0 || tier.pool_texture_levels != texture_levels.0 {

            tier.mesh_pool = std::array::from_fn(|band| {
                asset_server.load(GltfAssetLabel::Primitive { mesh: 0, primitive: 0 }.from_asset(shared::arena_car_mesh_path(index.0, poly_levels.0[band])))
            });
            tier.texture_pool = std::array::from_fn(|band| {
                let (base_color_path, normal_path, metallic_roughness_path) = shared::arena_car_texture_paths(index.0, texture_levels.0[band]);
                (
                    asset_server.load(base_color_path),
                    load_linear_car_texture(&asset_server, normal_path),
                    load_linear_car_texture(&asset_server, metallic_roughness_path),
                )
            });
            tier.pool_poly_levels = poly_levels.0;
            tier.pool_texture_levels = texture_levels.0;
            tier.pool_ready = false;
            tier.active_band = None;
        }
        if !tier.pool_ready {
            tier.pool_ready = tier.mesh_pool.iter().all(|h| asset_server.is_loaded_with_dependencies(h.id()))
                && tier.texture_pool.iter().all(|(base_color, normal, metallic_roughness)| {
                    asset_server.is_loaded_with_dependencies(base_color.id())
                        && asset_server.is_loaded_with_dependencies(normal.id())
                        && asset_server.is_loaded_with_dependencies(metallic_roughness.id())
                });
            if !tier.pool_ready {
                continue;
            }
        }
        let distance = (transform.translation.xz() - player_pos).length();
        let band = bands.band_of(distance);
        if tier.active_band == Some(band) {
            continue;
        }

        mesh.0 = tier.mesh_pool[band].clone();
        if let Some(mut material) = materials.get_mut(&material_handle.0) {
            let (base_color, normal, metallic_roughness) = &tier.texture_pool[band];
            material.base_color_texture = Some(base_color.clone());
            material.normal_map_texture = Some(normal.clone());
            material.metallic_roughness_texture = Some(metallic_roughness.clone());
        }
        tier.active_band = Some(band);
    }
}

fn shake_arena_car_on_hit(
    trigger: On<ShotFx>,
    spatial_query: SpatialQuery,
    collider_of: Query<&ColliderOf>,
    mut arena_cars: Query<Forces, With<ArenaCarProp>>,
) {
    let ShotFx::SurfaceHit { point, normal, .. } = *trigger.event() else {
        return;
    };

    let probe = point - normal.normalize_or_zero() * 0.05;
    let hit_bodies: Vec<Entity> = spatial_query
        .point_intersections(probe, &SpatialQueryFilter::default())
        .into_iter()
        .filter_map(|collider_entity| collider_of.get(collider_entity).ok().map(|c| c.body))
        .collect();
    for body in hit_bodies {
        if let Ok(mut forces) = arena_cars.get_mut(body) {
            forces.apply_linear_impulse_at_point(-normal.normalize_or_zero() * ARENA_CAR_HIT_IMPULSE, point);
            break;
        }
    }
}

const ARENA_CAR_PHYSICS_RON: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/assets/data/arena_car_physics.ron");

pub(crate) fn round4(v: f32) -> f32 {
    (v * 1.0e4).round() / 1.0e4
}

pub(crate) fn ron_f32(v: f32) -> String {
    let s = format!("{}", round4(v));
    if s.contains('.') { s } else { format!("{s}.0") }
}

#[derive(Resource, Default)]
pub(crate) struct F9SaveToast {
    text: String,
    ttl: f32,
    is_error: bool,
}

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
struct F9ToastText;

fn spawn_f9_toast_ui(mut commands: Commands) {
    commands.spawn((
        Text::new(""),
        TextFont { font_size: FontSize::Px(18.0), ..default() },
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

fn save_arena_car_physics_on_key(
    keys: Res<ButtonInput<KeyCode>>,
    cars: Query<(&Name, &Transform), With<ArenaCarProp>>,
    mut toast: ResMut<F9SaveToast>,
) {
    if !keys.just_pressed(KeyCode::F9) {
        return;
    }

    let live: std::collections::HashMap<&str, &Transform> =
        cars.iter().map(|(name, t)| (name.as_str(), t)).collect();

    let mut rows: Vec<(String, String)> = Vec::with_capacity(shared::ARENA_CAR_PATHS.len());
    for &path in &shared::ARENA_CAR_PATHS {
        let name = path.split('/').nth(1).unwrap_or(path);
        let Some(t) = live.get(name).copied() else {
            error!(
                "F9 save aborted: car '{name}' not in the scene \
                 — nothing written"
            );
            toast.err(format!("F9: car '{name}' not in scene — nothing saved"));
            return;
        };

        let scale = t.scale.x;
        if (t.scale.y - scale).abs() > 1.0e-3 || (t.scale.z - scale).abs() > 1.0e-3 {
            warn!("car '{name}': non-uniform scale {:?} in editor — saving x ({scale}) only", t.scale);
        }

        let (ry, rx, rz) = t.rotation.to_euler(EulerRot::YXZ);

        let y = t.translation.y - ARENA_CAR_DROP_MARGIN;
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

    let existing = match std::fs::read_to_string(ARENA_CAR_PHYSICS_RON) {
        Ok(s) => s,
        Err(e) => {
            error!("F9 save aborted: can't read {ARENA_CAR_PHYSICS_RON}: {e}");
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

    match std::fs::write(ARENA_CAR_PHYSICS_RON, out) {
        Ok(()) => {
            info!("saved {} cars -> {ARENA_CAR_PHYSICS_RON}", rows.len());
            toast.ok(format!("F9: saved {} cars", rows.len()));
        }
        Err(e) => {
            error!("F9 save failed writing {ARENA_CAR_PHYSICS_RON}: {e}");
            toast.err(format!("F9: car save FAILED — {e}"));
        }
    }
}

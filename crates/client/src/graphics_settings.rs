
use bevy::prelude::*;
use crate::*;

#[derive(Resource)]
pub(crate) struct SensitivityScale(pub(crate) f32);
impl Default for SensitivityScale {
    fn default() -> Self {
        Self(1.0)
    }
}

#[derive(Resource)]
pub(crate) struct FpsCap(pub(crate) u32);
impl Default for FpsCap {
    fn default() -> Self {
        let env_cap = std::env::var("FPS_CAP").ok().and_then(|s| s.parse::<u32>().ok()).filter(|c| *c > 0);
        Self(env_cap.unwrap_or(if cfg!(any(target_os = "ios", target_os = "android")) { 120 } else { 60 }))
    }
}

pub(crate) fn apply_fps_cap_desktop(cap: Res<FpsCap>, winit_settings: Option<ResMut<WinitSettings>>) {
    let Some(mut winit_settings) = winit_settings else { return };
    if !cap.is_changed() {
        return;
    }
    winit_settings.focused_mode = UpdateMode::reactive(std::time::Duration::from_secs_f32(1.0 / cap.0 as f32));
}

const RESOLUTION_SCALE_DEFAULT: f32 = 0.3;
pub(crate) const RESOLUTION_SCALE_MIN_TEXTURE_PX: u32 = 64;

fn resolution_scale_default() -> f32 {
    std::env::var("RENDER_SCALE")
        .ok()
        .and_then(|s| s.trim().parse::<f32>().ok())
        .map(|s| s.clamp(0.1, 1.0))
        .unwrap_or(RESOLUTION_SCALE_DEFAULT)
}

#[derive(Resource, Default)]
pub(crate) struct FreezeZombies(pub(crate) bool);

#[derive(Resource, Default)]
pub(crate) struct ShowHitboxes(pub(crate) bool);

#[derive(Resource, Default)]
pub(crate) struct ShowDistanceRings(pub(crate) bool);

#[derive(Resource)]
pub(crate) struct ShowAutopilotTarget(pub(crate) bool);

impl Default for ShowAutopilotTarget {
    fn default() -> Self {
        Self(std::env::var("SHOW_AUTOPILOT_TARGET").is_ok_and(|v| v == "1" || v.to_lowercase() == "true"))
    }
}

#[derive(Resource)]
pub(crate) struct ShowHitMarkers(pub(crate) bool);

impl Default for ShowHitMarkers {
    fn default() -> Self {
        if let Ok(val) = std::env::var("SHOW_HIT_MARKERS") {
            Self(val == "1" || val.to_lowercase() == "true")
        } else {
            Self(true)
        }
    }
}

#[derive(Resource)]
pub(crate) struct ShowFloatingDamage(pub(crate) bool);

impl Default for ShowFloatingDamage {
    fn default() -> Self {
        if let Ok(val) = std::env::var("SHOW_FLOATING_DAMAGE") {
            Self(val == "1" || val.to_lowercase() == "true")
        } else {
            Self(true)
        }
    }
}

#[derive(Resource)]
pub(crate) struct ShowBulletTracers(pub(crate) bool);

impl Default for ShowBulletTracers {
    fn default() -> Self {
        if let Ok(val) = std::env::var("SHOW_BULLET_TRACERS") {
            Self(val == "1" || val.to_lowercase() == "true")
        } else {
            Self(true)
        }
    }
}

#[derive(Resource, Default)]
pub(crate) struct GodMode(pub(crate) bool);

#[derive(Resource)]
pub(crate) struct RenderResolutionScale(pub(crate) f32);

impl Default for RenderResolutionScale {
    fn default() -> Self {
        Self(resolution_scale_default())
    }
}

#[derive(Resource)]
pub(crate) struct AimBlurEnabled(pub(crate) bool);

impl Default for AimBlurEnabled {
    fn default() -> Self {
        Self(true)
    }
}

#[derive(Resource, Default)]
pub(crate) struct AimDofFocalDistanceAlt(pub(crate) bool);

#[derive(Resource, Default)]
pub(crate) struct MapMsaaOn(pub(crate) bool);

#[derive(Resource)]
pub(crate) struct ShadowQuality(pub(crate) u8);
impl Default for ShadowQuality {
    fn default() -> Self {
        if cfg!(any(target_os = "ios", target_os = "android")) {
            Self(1)
        } else {
            Self(if light_shadows_enabled() { 5 } else { 0 })
        }
    }
}

#[derive(Resource)]
pub(crate) struct LightQuality(pub(crate) u8);
impl Default for LightQuality {
    fn default() -> Self {
        Self(if dynamic_lights_enabled() { 3 } else { 0 })
    }
}

#[derive(Resource)]
pub(crate) struct PointLightQuality(pub(crate) u8);
impl Default for PointLightQuality {
    fn default() -> Self {
        Self(if dynamic_lights_enabled() { 3 } else { 0 })
    }
}

#[derive(Component)]
pub(crate) struct PointLightBaseIntensity(pub(crate) f32);

pub(crate) fn texture_quality_default() -> u8 {
    std::env::var("TEXTURE_QUALITY")
        .ok()
        .and_then(|s| s.trim().parse::<u8>().ok())
        .map(|q| q.clamp(0, 5))
        .unwrap_or(if cfg!(any(target_os = "ios", target_os = "android")) { 2 } else { 5 })
}

#[derive(Resource)]
pub(crate) struct TextureQuality(pub(crate) u8);
impl Default for TextureQuality {
    fn default() -> Self {
        Self(texture_quality_default())
    }
}

pub(crate) fn zombie_texture_quality_default() -> u8 {
    std::env::var("ZOMBIE_TEXTURE_QUALITY")
        .ok()
        .and_then(|s| s.trim().parse::<u8>().ok())
        .map(|q| q.clamp(0, 5))
        .unwrap_or(if cfg!(any(target_os = "ios", target_os = "android")) { 2 } else { 5 })
}

#[derive(Resource)]
pub(crate) struct ZombieTextureQuality(pub(crate) u8);
impl Default for ZombieTextureQuality {
    fn default() -> Self {
        Self(zombie_texture_quality_default())
    }
}

pub(crate) fn poly_quality_default() -> u8 {
    std::env::var("POLY_QUALITY")
        .ok()
        .and_then(|s| s.trim().parse::<u8>().ok())
        .map(|q| q.clamp(1, 5))
        .unwrap_or(if cfg!(any(target_os = "ios", target_os = "android")) { 2 } else { 5 })
}

#[derive(Resource)]
pub(crate) struct PolyQuality(pub(crate) u8);
impl Default for PolyQuality {
    fn default() -> Self {
        Self(poly_quality_default())
    }
}

#[derive(Resource, Default)]
pub(crate) struct DistanceLodEnabled(pub(crate) bool);

#[derive(Resource)]
pub(crate) struct LodBandDistance(pub(crate) [f32; 3]);
impl Default for LodBandDistance {
    fn default() -> Self {
        Self([ZOMBIE_LOD_DISTANCE, 30.0, 50.0])
    }
}
impl LodBandDistance {

    pub(crate) fn band_of(&self, distance: f32) -> usize {
        if distance <= self.0[0] {
            0
        } else if distance <= self.0[1] {
            1
        } else {
            2
        }
    }
}
pub(crate) const LOD_DISTANCE_STEP: f32 = 1.0;
pub(crate) const LOD_DISTANCE_MIN: f32 = 1.0;
pub(crate) const LOD_DISTANCE_MAX: f32 = 200.0;

#[derive(Resource)]
pub(crate) struct LodCarTextureLevel(pub(crate) [u8; 3]);
impl Default for LodCarTextureLevel {
    fn default() -> Self {
        Self([5, 3, 1])
    }
}

#[derive(Resource)]
pub(crate) struct LodCarPolyLevel(pub(crate) [u8; 3]);
impl Default for LodCarPolyLevel {
    fn default() -> Self {
        Self([5, 3, 1])
    }
}

#[derive(Resource)]
pub(crate) struct LodZombieTextureLevel(pub(crate) [u8; 3]);
impl Default for LodZombieTextureLevel {
    fn default() -> Self {
        Self([5, 3, 1])
    }
}

pub(crate) fn gun_poly_quality_default() -> u8 {
    std::env::var("GUN_POLY_QUALITY")
        .ok()
        .and_then(|s| s.trim().parse::<u8>().ok())
        .map(|q| q.clamp(1, 5))
        .unwrap_or(if cfg!(any(target_os = "ios", target_os = "android")) { 2 } else { 5 })
}

#[derive(Resource)]
pub(crate) struct GunPolyQuality(pub(crate) u8);
impl Default for GunPolyQuality {
    fn default() -> Self {
        Self(gun_poly_quality_default())
    }
}

pub(crate) fn gun_texture_quality_default() -> u8 {
    std::env::var("GUN_TEXTURE_QUALITY")
        .ok()
        .and_then(|s| s.trim().parse::<u8>().ok())
        .map(|q| q.clamp(0, 5))
        .unwrap_or(if cfg!(any(target_os = "ios", target_os = "android")) { 2 } else { 5 })
}

#[derive(Resource)]
pub(crate) struct GunTextureQuality(pub(crate) u8);
impl Default for GunTextureQuality {
    fn default() -> Self {
        Self(gun_texture_quality_default())
    }
}

pub(crate) fn gun_mesh_path(poly_quality: u8) -> &'static str {
    match poly_quality {
        1 => "models/ak47_scope/mesh/ak47_scope_light_p1.glb",
        2 => "models/ak47_scope/mesh/ak47_scope_light_p2.glb",
        3 => "models/ak47_scope/mesh/ak47_scope_light_p3.glb",
        4 => "models/ak47_scope/mesh/ak47_scope_light_p4.glb",
        _ => "models/ak47_scope/mesh/ak47_scope_light.glb",
    }
}

pub(crate) fn gun_texture_paths(mat_index: usize, texture_quality: u8) -> (String, String, String) {
    let q = texture_quality.min(5);
    let base_ext = if mat_index == 1 { "png" } else { "jpg" };
    (
        format!("models/ak47_scope/mesh/textures/mat{mat_index}_base_color_q{q}.{base_ext}"),
        format!("models/ak47_scope/mesh/textures/mat{mat_index}_normal_q{q}.jpg"),
        format!("models/ak47_scope/mesh/textures/mat{mat_index}_metallic_roughness_q{q}.jpg"),
    )
}

pub(crate) fn spawn_gun_mesh_parts(
    commands: &mut Commands,
    asset_server: &AssetServer,
    materials: &mut Assets<StandardMaterial>,
    gun_entity: Entity,
    poly_quality: u8,
    texture_quality: u8,
) {
    let mesh_path = gun_mesh_path(poly_quality);
    let linear = |path: String| {
        asset_server.load_with_settings(path, |settings: &mut bevy::image::ImageLoaderSettings| {
            settings.is_srgb = false;
        })
    };
    for mat_index in 0..2usize {
        let (base_color_path, normal_path, metallic_roughness_path) = gun_texture_paths(mat_index, texture_quality);
        let metallic_roughness_handle = linear(metallic_roughness_path);
        let material = materials.add(StandardMaterial {
            base_color_texture: Some(asset_server.load(base_color_path)),
            normal_map_texture: Some(linear(normal_path)),
            metallic_roughness_texture: Some(metallic_roughness_handle.clone()),
            occlusion_texture: Some(metallic_roughness_handle),
            cull_mode: None,
            alpha_mode: if mat_index == 1 { AlphaMode::Blend } else { AlphaMode::Opaque },
            ..default()
        });
        commands.spawn((
            Name::new(if mat_index == 0 { "Gun body" } else { "Gun scope lens" }),
            Mesh3d(asset_server.load(GltfAssetLabel::Primitive { mesh: mat_index, primitive: 0 }.from_asset(mesh_path))),
            MeshMaterial3d(material),
            Transform::IDENTITY,
            GunMeshPart,
            ChildOf(gun_entity),
        ));
    }
}

pub(crate) fn respawn_gun_mesh_parts_on_quality_change(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    poly_quality: Res<GunPolyQuality>,
    texture_quality: Res<GunTextureQuality>,
    gun: Query<Entity, With<Gun>>,
    existing_parts: Query<Entity, With<GunMeshPart>>,
) {
    let poly_changed = poly_quality.is_changed() && !poly_quality.is_added();
    let texture_changed = texture_quality.is_changed() && !texture_quality.is_added();
    if !poly_changed && !texture_changed {
        return;
    }
    let Ok(gun_entity) = gun.single() else { return };
    for entity in &existing_parts {
        commands.entity(entity).despawn();
    }
    spawn_gun_mesh_parts(&mut commands, &asset_server, &mut materials, gun_entity, poly_quality.0, texture_quality.0);
}

pub(crate) fn dynamic_lights_enabled() -> bool {
    cfg!(any(target_os = "ios", target_os = "android")) || std::env::var("LIGHTS").map(|v| v != "0").unwrap_or(true)
}
pub(crate) fn light_shadows_enabled() -> bool {
    cfg!(any(target_os = "ios", target_os = "android")) || std::env::var("SHADOWS").map(|v| v != "0").unwrap_or(true)
}

pub(crate) fn ambient_brightness() -> f32 {
    if dynamic_lights_enabled() { 250.0 } else { 1800.0 }
}

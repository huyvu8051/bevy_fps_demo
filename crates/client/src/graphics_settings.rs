
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
        Self(env_cap.unwrap_or(if cfg!(target_os = "ios") { 120 } else { 60 }))
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
        if cfg!(target_os = "ios") {
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

/// Which variant of the zombie base-colour textures ships: mobile takes a
/// reduced one, desktop the original. The gun and the arena cars no longer
/// have tiers — only their sharpest meshes and textures are left on disk.
pub(crate) const ASSET_QUALITY_TIER: u8 = if cfg!(target_os = "ios") { 2 } else { 5 };

pub(crate) fn gun_mesh_path() -> &'static str {
    "models/ak47_scope/mesh/ak47_scope.glb"
}

pub(crate) fn gun_texture_paths(mat_index: usize) -> (String, String, String) {
    let base_ext = if mat_index == 1 { "png" } else { "jpg" };
    (
        format!("models/ak47_scope/mesh/textures/mat{mat_index}_base_color_q5.{base_ext}"),
        format!("models/ak47_scope/mesh/textures/mat{mat_index}_normal_q5.png"),
        format!("models/ak47_scope/mesh/textures/mat{mat_index}_metallic_roughness_q5.png"),
    )
}

pub(crate) fn spawn_gun_mesh_parts(
    commands: &mut Commands,
    asset_server: &AssetServer,
    materials: &mut Assets<StandardMaterial>,
    gun_entity: Entity,
) {
    let mesh_path = gun_mesh_path();
    let linear = |path: String| {
        asset_server.load_with_settings(path, |settings: &mut bevy::image::ImageLoaderSettings| {
            settings.is_srgb = false;
        })
    };
    for mat_index in 0..2usize {
        let (base_color_path, normal_path, metallic_roughness_path) = gun_texture_paths(mat_index);
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

pub(crate) fn dynamic_lights_enabled() -> bool {
    cfg!(target_os = "ios") || std::env::var("LIGHTS").map(|v| v != "0").unwrap_or(true)
}
pub(crate) fn light_shadows_enabled() -> bool {
    cfg!(target_os = "ios") || std::env::var("SHADOWS").map(|v| v != "0").unwrap_or(true)
}

pub(crate) fn ambient_brightness() -> f32 {
    if dynamic_lights_enabled() { 250.0 } else { 1800.0 }
}

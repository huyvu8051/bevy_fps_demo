use bevy::camera::primitives::Aabb;
use bevy::light::NotShadowCaster;

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

pub(crate) fn gun_mesh_path() -> &'static str {
    "models/ak47_scope/raw/scene.gltf"
}

pub(crate) fn spawn_gun_mesh_parts(
    commands: &mut Commands,
    asset_server: &AssetServer,
    _materials: &mut Assets<StandardMaterial>,
    gun_entity: Entity,
) {
    commands.spawn((
        Name::new("Gun mesh"),
        WorldAssetRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(gun_mesh_path()))),
        Transform::IDENTITY,
        GunMeshPart,
        ChildOf(gun_entity),
    ));
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

/// glTF leaves metallicFactor at its spec default of 1.0 when an exporter omits
/// it, which makes every surface a mirror with no diffuse term — unlit by the
/// sun and incapable of showing a shadow. Demote those on import; materials
/// built in code already default to 0.0 and are left alone.
const SHADOW_CASTER_SIZE_LIMIT: f32 = 60.0;

pub(crate) fn demote_fully_metallic_imports(
    mut events: MessageReader<AssetEvent<StandardMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for event in events.read() {
        let AssetEvent::Added { id } = event else {
            continue;
        };
        let Some(mut material) = materials.get_mut(*id) else {
            continue;
        };
        if material.metallic >= 1.0 && material.metallic_roughness_texture.is_none() {
            material.metallic = 0.0;
        }
    }
}

/// Bevy fits the directional light's shadow depth range around every shadow
/// caster in the scene. The maps ship a skybox-sized shell — the desert spans
/// 1349 units across — which stretches that range a thousandfold and destroys
/// the shadow map's depth precision, so nothing casts a visible shadow at all.
/// Backdrop geometry that large has no business casting shadows anyway.
pub(crate) fn exclude_oversized_shadow_casters(
    mut commands: Commands,
    meshes: Query<(Entity, &Aabb), (With<Mesh3d>, Without<NotShadowCaster>)>,
) {
    for (entity, aabb) in &meshes {
        if aabb.half_extents.max_element() * 2.0 > SHADOW_CASTER_SIZE_LIMIT {
            commands.entity(entity).insert(NotShadowCaster);
        }
    }
}

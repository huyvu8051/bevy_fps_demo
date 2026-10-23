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

pub(crate) fn spawn_gun_mesh_parts(
    commands: &mut Commands,
    asset_server: &AssetServer,
    _materials: &mut Assets<StandardMaterial>,
    gun_entity: Entity,
    mesh_path: &str,
) {
    commands.spawn((
        Name::new("Gun mesh"),
        WorldAssetRoot(load_gltf_scene(asset_server, mesh_path, RenderAssetUsages::RENDER_WORLD)),
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

const DESERT_VISIBILITY: f32 = 95.0;
const GARAGE_VISIBILITY: f32 = 38.0;

/// Depth fog, tuned per map. Visibility is the distance at which a dark object
/// stops being distinguishable from the haze, which is the number the
/// Koschmieder falloff is derived from, so it reads directly as "how far can
/// you see here".
pub(crate) fn distance_fog_for(scenario: Scenario) -> DistanceFog {
    match scenario {
        Scenario::Desert => DistanceFog {
            color: Color::srgb(0.74, 0.66, 0.54),
            directional_light_color: Color::srgb(1.0, 0.92, 0.76),
            directional_light_exponent: 24.0,
            falloff: FogFalloff::from_visibility(DESERT_VISIBILITY),
        },
        Scenario::Garage => DistanceFog {
            color: Color::srgb(0.10, 0.11, 0.13),
            directional_light_color: Color::srgb(0.4, 0.42, 0.5),
            directional_light_exponent: 8.0,
            falloff: FogFalloff::from_visibility(GARAGE_VISIBILITY),
        },
    }
}

/// Frame pacing, printed rather than drawn, so a run can be measured from the
/// log instead of from the Metal HUD, which the in-game capture cannot see.
pub(crate) fn log_frame_pacing(
    time: Res<Time<Real>>,
    cap: Res<FpsCap>,
    mut frames: Local<u32>,
    mut window: Local<f32>,
    mut worst: Local<f32>,
) {
    if std::env::var("FPS_LOG").is_err() {
        return;
    }
    let dt = time.delta_secs();
    *frames += 1;
    *window += dt;
    *worst = worst.max(dt);
    if *window < 1.0 {
        return;
    }
    let mean_ms = *window / *frames as f32 * 1000.0;
    info!(
        "[fps] {:.1} fps | khung {:.2}ms (te nhat {:.2}ms) | tran dat {} = {:.2}ms",
        *frames as f32 / *window,
        mean_ms,
        *worst * 1000.0,
        cap.0,
        1000.0 / cap.0 as f32,
    );
    (*frames, *window, *worst) = (0, 0.0, 0.0);
}

/// Per render graph node timings, behind RENDER_DIAG=1. CPU only on Metal.
pub(crate) fn render_diagnostics_if_asked() -> impl Plugin {
    #[derive(Default)]
    struct Maybe;
    impl Plugin for Maybe {
        fn build(&self, app: &mut App) {
            if std::env::var("RENDER_DIAG").is_err() {
                return;
            }
            app.add_plugins((
                bevy::render::diagnostic::RenderDiagnosticsPlugin,
                bevy::diagnostic::LogDiagnosticsPlugin {
                    wait_duration: std::time::Duration::from_secs(5),
                    ..default()
                },
            ));
        }
    }
    Maybe
}

/// Cascades the sun splits into. Each is a separate shadow view rendered per
/// camera, so it is the largest single cost in the frame. CASCADES=n overrides.
#[derive(Resource)]
pub(crate) struct ShadowCascades(pub(crate) u8);

impl Default for ShadowCascades {
    fn default() -> Self {
        Self(std::env::var("CASCADES").ok().and_then(|v| v.parse().ok()).unwrap_or(2))
    }
}

pub(crate) fn cascade_config(cascades: &ShadowCascades) -> bevy::light::CascadeShadowConfig {
    bevy::light::CascadeShadowConfigBuilder {
        num_cascades: cascades.0.max(1) as usize,
        ..default()
    }
    .build()
}

/// Keeps a camera out of the shadow pass, worth 1.47ms on a sighted frame --
/// most of the 2.25ms the scope view costs at all. Measured by removing this
/// component and nothing else: 10.70ms becomes 12.17ms.
///
/// per_view_shadow_pass is the only system that requires ViewLightEntities, so
/// removing it after prepare_lights skips that view. Its depth texture then
/// goes unwritten: reverse-Z reads the zeros as unshadowed, so the picture is
/// lit but has no shadows.
#[derive(Component, Clone, Copy, Default)]
pub(crate) struct NoShadowPass;

impl bevy::render::sync_component::SyncComponent for NoShadowPass {
    type Target = Self;
}

impl bevy::render::extract_component::ExtractComponent for NoShadowPass {
    type QueryData = ();
    type QueryFilter = With<NoShadowPass>;
    type Out = NoShadowPass;
    fn extract_component(_: ()) -> Option<NoShadowPass> {
        Some(NoShadowPass)
    }
}

/// Runs after prepare_lights has built the component, in the same set, so the
/// view is stripped before the graph walks it.
fn drop_shadow_views(
    mut commands: Commands,
    views: Query<Entity, (With<NoShadowPass>, With<bevy::pbr::ViewLightEntities>)>,
) {
    for view in &views {
        commands.entity(view).remove::<bevy::pbr::ViewLightEntities>();
    }
}

pub(crate) struct NoShadowPassPlugin;

impl Plugin for NoShadowPassPlugin {
    fn build(&self, app: &mut App) {
        // ExtractComponentPlugin brings SyncComponentPlugin with it; adding
        // both is a panic, not a no-op.
        app.add_plugins(bevy::render::extract_component::ExtractComponentPlugin::<NoShadowPass>::default());
        let Some(render_app) = app.get_sub_app_mut(bevy::render::RenderApp) else { return };
        render_app.add_systems(
            bevy::render::Render,
            drop_shadow_views
                .in_set(bevy::render::RenderSystems::CreateViews)
                .after(bevy::pbr::prepare_lights),
        );
    }
}

/// Whether the main camera glows its bright pixels.
///
/// Measured at ~635us a frame in the baseline profile, which is more than the
/// sun's fourth shadow cascade cost. It is also entirely a look, so it ships
/// on and is a setting rather than a decision made for the player.
#[derive(Resource)]
pub(crate) struct BloomEnabled(pub(crate) bool);

impl Default for BloomEnabled {
    fn default() -> Self {
        Self(true)
    }
}

/// Added and removed on the main camera to match the setting. The scope camera
/// never has it; see the note where that one is spawned.
pub(crate) fn apply_bloom(
    mut commands: Commands,
    enabled: Res<BloomEnabled>,
    camera: Query<(Entity, Has<Bloom>), (With<FpsCamera>, Without<ScopeCamera>)>,
) {
    if !enabled.is_changed() {
        return;
    }
    let Ok((camera, has_bloom)) = camera.single() else {
        return;
    };
    match (enabled.0, has_bloom) {
        (true, false) => {
            commands.entity(camera).insert(Bloom::default());
        }
        (false, true) => {
            // Hdr goes too. Bloom requires it, and a required component does
            // not leave when the thing that required it does -- so without
            // this the glow stops but the whole main pass keeps writing and
            // reading a 16 bit float target for nothing.
            commands.entity(camera).remove::<(Bloom, bevy::camera::Hdr)>();
        }
        _ => {}
    }
}

/// Counts what the opaque and shadow phases submit, behind DRAW_COUNT=1 --
/// after culling and batching, which an entity count cannot tell you.
pub(crate) struct DrawCountPlugin;

impl Plugin for DrawCountPlugin {
    fn build(&self, app: &mut App) {
        if std::env::var("DRAW_COUNT").is_err() {
            return;
        }
        let Some(render_app) = app.get_sub_app_mut(bevy::render::RenderApp) else { return };
        render_app.add_systems(
            bevy::render::Render,
            count_draws.in_set(bevy::render::RenderSystems::PhaseSort),
        );
    }
}

/// A closure cannot be generic over the phase item, so this is a fn.
fn tally<BPI: bevy::render::render_phase::BinnedPhaseItem>(
    phase: &bevy::render::render_phase::BinnedRenderPhase<BPI>,
) -> usize {
    phase.multidrawable_meshes.len()
        + phase.batchable_meshes.len()
        + phase.unbatchable_meshes.values().map(|bin| bin.entities.len()).sum::<usize>()
}

fn count_draws(
    opaque: Res<bevy::render::render_phase::ViewBinnedRenderPhases<bevy::core_pipeline::core_3d::Opaque3d>>,
    shadow: Res<bevy::render::render_phase::ViewBinnedRenderPhases<bevy::pbr::Shadow>>,
    mut every: Local<u32>,
) {
    *every += 1;
    if *every % 120 != 0 {
        return;
    }
    let opaque_views: Vec<usize> = opaque.values().map(tally).collect();
    let shadow_total: usize = shadow.values().map(tally).sum();
    info!(
        "[draws] opaque per view {:?} = {}, shadow {} across {} views, total {}",
        opaque_views,
        opaque_views.iter().sum::<usize>(),
        shadow_total,
        shadow.len(),
        opaque_views.iter().sum::<usize>() + shadow_total,
    );
}

/// Whether the cameras carry DistanceFog. Not a pass of its own but arithmetic
/// inside the PBR shader, and measured at 0.01ms.
#[derive(Resource, Default)]
pub(crate) struct FogEnabled(pub(crate) bool);

pub(crate) fn apply_fog(
    mut commands: Commands,
    enabled: Res<FogEnabled>,
    cameras: Query<(Entity, Has<DistanceFog>), With<Camera3d>>,
) {
    if !enabled.is_changed() {
        return;
    }
    for (camera, present) in &cameras {
        match (enabled.0, present) {
            (true, false) => {
                commands.entity(camera).insert(distance_fog_for(game_flow::current_scenario()));
            }
            (false, true) => {
                commands.entity(camera).remove::<DistanceFog>();
            }
            _ => {}
        }
    }
}

/// Loads a glTF scene, saying what is kept in RAM once it is on the GPU.
///
/// Only meshes can be dropped: load_materials governs the StandardMaterial
/// assets rather than their textures, and dropping their main world half
/// renders a black screen. The map keeps its CPU meshes for Avian's colliders.
pub(crate) fn load_gltf_scene(
    asset_server: &AssetServer,
    path: impl Into<String>,
    meshes: RenderAssetUsages,
) -> Handle<bevy::world_serialization::WorldAsset> {
    asset_server
        .load_builder()
        .with_settings(move |settings: &mut bevy::gltf::GltfLoaderSettings| {
            settings.load_meshes = meshes;
        })
        .load(GltfAssetLabel::Scene(0).from_asset(path.into()))
}

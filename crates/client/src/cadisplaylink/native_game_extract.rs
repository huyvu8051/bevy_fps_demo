//! Phase 6 Step 2 of the wgpu-replacement plan: per-frame extraction from
//! the headless real-game `App` (see `native_game_app.rs`) into a plain,
//! owned snapshot our Metal renderer can read — the seam between Bevy ECS
//! and the custom renderer, mirroring `native_metal_probe.rs`'s existing
//! `patrol_system` extract pattern (tick the World, run an ad-hoc
//! `SystemState`/`QueryState`, collect into an owned `Vec` rather than
//! keeping a live borrow).
//!
//! Deliberately data-only: no Metal/rendering types appear here at all, so
//! this module compiles and is testable independent of Step 4's actual
//! draw-call wiring.

use bevy::asset::{AssetId, Assets};
use bevy::camera::{Camera, Projection, RenderTargetInfo};
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::{With, Without};
use bevy::ecs::system::{Query, Res, SystemState};
use bevy::image::Image;
use bevy::mesh::skinning::SkinnedMesh;
use bevy::light::{DirectionalLight, PointLight};
use bevy::pbr::{MeshMaterial3d, StandardMaterial};
use bevy::render::alpha::AlphaMode;
use bevy::mesh::{Mesh, Mesh3d};
use bevy::render::render_resource::TextureFormat;
use bevy::text::{TextColor, TextFont};
use bevy::transform::components::{GlobalTransform, Transform};
use bevy::ui::widget::{Button, Text};
use bevy::ui::{BackgroundColor, ComputedNode, Display, Interaction, Node, UiGlobalTransform, Val};

use shared::{NetPlayer, Zombie, ZombieKind};

/// The headless real-game `App` has no `Window`/`RenderPlugin` at all (see
/// `native_game_app.rs`'s own plugin-set doc comment), so `Camera::
/// world_to_viewport` — which `update_zombie_health_bars` and
/// `update_floating_damage_texts` (crate::lib) both rely on to place their
/// bevy_ui `Node`s on screen — always failed: `logical_viewport_size()`
/// needs `Camera.computed.target_info`, which bevy_render's internal
/// `camera_system` normally fills in once per frame from the real Window's
/// size/scale factor, and that system never runs here. Both UI systems
/// treat a failed projection as "off-screen" and set `Display::None`, so
/// every floating damage number and zombie health bar silently stayed
/// hidden forever — not a rendering gap so much as the screen-space data
/// itself never being computed. Filling in the same fields by hand from
/// the Metal layer's own known point size (called right before
/// `app.update()`, every frame, since the view can resize) is far simpler
/// than pulling in enough of WindowPlugin/RenderPlugin for the real system
/// to apply, and is the same "extract/patch the minimum Bevy state a
/// system needs" approach the rest of this module already uses.
pub fn prime_camera_viewport(world: &mut bevy::ecs::world::World, width_pts: f32, height_pts: f32, scale_factor: f32) {
    let mut state: SystemState<Query<(&mut Camera, &mut Projection), With<crate::FpsCamera>>> = SystemState::new(world);
    let mut query = state.get_mut(world);
    let (w, h) = (width_pts.max(1.0), height_pts.max(1.0));
    // `scale_factor` MUST match the real `window.scale_factor()` (see
    // native_game_app.rs's WindowResolution::with_scale_factor_override) —
    // this used to be hardcoded to 1.0, which was harmless while the
    // Window itself also defaulted to scale_factor 1.0, but became a real
    // bug once that Window got a real scale_factor_override (device's
    // native scale, e.g. 3.0 on a 14 Pro Max): bevy_ui's own
    // `ui_focus_system` (and this project's own `multi_touch_button_
    // interaction`, see its doc comment) both convert a touch's LOGICAL
    // position to PHYSICAL by multiplying by `window.scale_factor()`
    // before hit-testing against `ComputedNode`/`UiGlobalTransform` — but
    // `ComputedNode` itself is sized against THIS camera's own
    // `target_info.scale_factor`, not the Window's. Leaving this at 1.0
    // while the Window carries a real 3x override meant every UI node was
    // laid out 3x smaller (in this "physical" space) than the touch
    // hit-test assumed, so taps landed nowhere near their visually-correct
    // button (confirmed via on-device logging: touch position exactly
    // inside the button's OWN logically-reported rect, yet Interaction
    // never changed — the physical-space rect bevy_ui actually hit-tests
    // against was 3x smaller and shifted). `logical_rect` (used to build
    // every on-screen draw snapshot) already divides back out by
    // `inverse_scale_factor`, so fixing this here doesn't move anything
    // visually — it only fixes hit-testing.
    let target_info = || {
        Some(RenderTargetInfo {
            physical_size: bevy::math::UVec2::new((w * scale_factor) as u32, (h * scale_factor) as u32),
            scale_factor,
        })
    };
    if let Ok((mut camera, mut projection)) = query.single_mut() {
        // No `camera.viewport` override — leaving it `None` means "the full
        // render target", which `target_info` below now defines.
        camera.computed.target_info = target_info();
        projection.update(w, h);
        camera.computed.clip_from_view = projection.get_clip_from_view();
    }

    // `present_camera` (Camera2d + IsDefaultUiCamera, crate::lib's setup())
    // is a SEPARATE camera entity from FpsCamera above — every UI node
    // without an explicit `UiTargetCamera` (which is all of them here)
    // sizes/positions itself off THIS camera's own `computed.target_info`,
    // not FpsCamera's, so touch-control buttons/joystick were being laid
    // out against whatever this camera's target_info happened to be —
    // `None`/stale, since the real `bevy_render` `camera_system` that
    // normally fills it in from the Window every frame never runs in this
    // headless app either — same root cause as FpsCamera's own target_info
    // above, just a second camera nobody had primed yet.
    let mut ui_camera_state: SystemState<Query<&mut Camera, With<bevy::ui::IsDefaultUiCamera>>> = SystemState::new(world);
    for mut ui_camera in ui_camera_state.get_mut(world).iter_mut() {
        ui_camera.computed.target_info = target_info();
    }

    // The headless App's own synthetic PrimaryWindow (native_game_app.rs)
    // is spawned once with a hardcoded placeholder resolution (1170x2532,
    // a portrait iPhone size picked purely to satisfy `With<PrimaryWindow>`
    // queries at startup — see its own doc comment) and NOTHING used to
    // keep it in sync with the real view size the way `camera.computed`
    // above already is. bevy_ui's own `ui_layout_system` sizes the UI root
    // node off THIS Window's resolution, not the camera — so every
    // touch-control button/joystick node was being laid out against a
    // fixed wrong-orientation size, landing it far outside the visible NDC
    // range once drawn (TouchButtonSnapshot/TouchJoystickSnapshot came back
    // populated, just with coordinates off-screen) rather than merely
    // mispositioned within it. Same "extract/patch the minimum Bevy state a
    // system needs, every frame since the view can resize" approach this
    // function already applies to the camera, now covering the Window too.
    // `scale_factor` stays at its default 1.0 (matching `target_info`
    // above), so points and physical pixels are the same number here.
    let mut window_state: SystemState<Query<&mut bevy::window::Window, With<bevy::window::PrimaryWindow>>> = SystemState::new(world);
    if let Ok(mut window) = window_state.get_mut(world).single_mut() {
        window.resolution.set(w, h);
    }
}

/// Raw RGBA8 pixel data for one mesh's base-color texture, extracted from
/// Bevy's `Image` asset — `None` on the `ExtractedMesh` when the material
/// has no texture (flat `base_color` factor only) or when its CPU-side
/// pixel format isn't one of the common RGBA8 variants glTF base-color
/// textures normally load as (extraction just skips it in that case,
/// falling back to a flat-tinted draw rather than failing).
pub struct ExtractedTexture {
    /// The source `Image` asset's own stable identity — used by the renderer
    /// to cache uploaded GPU textures keyed by WHICH image this is, not by
    /// its position in `RenderSnapshot::static_meshes` (see
    /// `NativeMetalProbe::real_game_textures`'s own doc comment for why a
    /// positional cache silently went stale/mismatched once transient VFX
    /// meshes — muzzle flash, smoke — started spawning/despawning every
    /// shot, shuffling that Vec's contents frame to frame).
    pub id: AssetId<Image>,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// One drawable static mesh — arena walls/floor (`spawn_test_arena`,
/// `crates/client/src/lib.rs:3915`) and the gun's glTF-scene-instantiated
/// child meshes (`GUN_PATH = "models/ak47_scope.glb"`) both show up here.
/// Both carry plain `Mesh3d`/`MeshMaterial3d<StandardMaterial>` once
/// Bevy's scene spawner has run; querying `GlobalTransform` (not local
/// `Transform`) is what makes the gun's meshes — parented to the gun
/// root, itself `ChildOf(camera)` — come out in correct world space
/// without any manual parent-chain math here. Requires
/// `bevy::transform::TransformPlugin` in the headless App (see
/// native_game_app.rs) to actually propagate `Transform` -> `GlobalTransform`
/// through the hierarchy each tick; first attempt at this used local
/// `Transform` directly and rendered the gun as a garbled giant mesh,
/// confirming the propagation step really is necessary, not optional.
pub struct ExtractedMesh {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    /// Empty if the source mesh has no UV attribute (e.g. the arena's
    /// `Cuboid` primitives, which do have one — but guard rather than
    /// assume) — the renderer falls back to `[0.0, 0.0]` per vertex, which
    /// combined with `texture: None` just shows the flat `base_color` tint.
    pub uvs: Vec<[f32; 2]>,
    /// Empty if the source `Mesh` isn't indexed (shouldn't happen for
    /// anything built via `meshes.add(Cuboid::new(..))` or loaded glTF, both
    /// of which are always indexed, but guard rather than assume).
    pub indices: Vec<u32>,
    pub base_color: [f32; 4],
    /// `StandardMaterial::emissive`, straight from `LinearRgba` (already
    /// linear, deliberately NOT `.to_srgba()`'d like `base_color` — these
    /// values are authored well above 1.0 on purpose, e.g. muzzle flash's
    /// `LinearRgba::rgb(6.0, 3.5, 1.0)` or the scope reticle's
    /// `rgb(35.0, 0.0, 0.0)`, meant to overdrive Bloom — see crate::lib's
    /// `MuzzleFxAssets`/`SparkAssets`/reticle material setup). Without this,
    /// every one of those effects rendered as flat unlit-adjacent
    /// `base_color` with no glow at all.
    pub emissive: [f32; 4],
    pub texture: Option<ExtractedTexture>,
    /// glTF tangents (xyz direction + w handedness sign) — needed to build
    /// a per-fragment TBN basis for `normal_texture` below. Empty when the
    /// source mesh has no `ATTRIBUTE_TANGENT` (most of this game's own
    /// procedural arena geometry has no normal map anyway, so there's
    /// nothing to build a basis for); `bake_static_mesh_vertices` (native_
    /// metal_probe.rs) falls back to a fixed `[1,0,0,1]` tangent in that
    /// case, same as it already falls back to `Vec3::Y` for missing
    /// normals — harmless since normal_texture is also None whenever
    /// tangents are missing (see below), so that fallback tangent is never
    /// actually sampled through.
    pub tangents: Vec<[f32; 4]>,
    /// `StandardMaterial::normal_map_texture` — the AK47's own real
    /// glTF material carries one (surface scratches/wear that only show up
    /// as a lighting response, not a flat color — see this field's own
    /// extraction site for the full story of why the renderer looked
    /// "textureless" on the gun's receiver despite base_color loading
    /// fine). `None` whenever the mesh has no tangents to build a TBN
    /// basis from, even if the material itself has one — sampling a
    /// normal map without a real tangent basis produces worse artifacts
    /// than just not perturbing the normal at all.
    pub normal_texture: Option<ExtractedTexture>,
    /// World-space transform at extraction time (from `GlobalTransform`,
    /// converted to a plain `Transform` for the renderer's convenience —
    /// see `bake_static_meshes` in native_metal_probe.rs).
    pub transform: Transform,
    /// The real `StandardMaterial::alpha_mode` — `true` for `Blend`/
    /// `Premultiplied`/`Add`/`Multiply` (anything that needs to composite
    /// with what's already drawn rather than fully replace it), `false`
    /// for `Opaque`/`Mask`. native_metal_probe.rs's draw_scene uses this
    /// (the real material property, not a guessed alpha threshold or an
    /// entity-hierarchy heuristic) to draw every blended mesh in the
    /// whole scene — arena or gun — in a separate
    /// late pass with depth-write disabled, so translucent geometry never
    /// depth-occludes whatever's behind it. See that pass's own doc
    /// comment for the full story of why an entity-hierarchy or alpha-
    /// value heuristic wasn't reliable enough.
    pub is_blend: bool,
}

/// Camera pose. `eye` is the `FpsCamera` entity's own world-space
/// translation. `rotation` is that entity's actual world-space rotation —
/// deliberately read straight off `GlobalTransform` rather than
/// reconstructed from `FpsCamera.yaw/.pitch`, because the *final* rotation
/// each tick is `apply_view_recoil`'s composition of yaw + pitch +
/// `ViewRecoil`'s kick + `Lean`'s roll (crates/client/src/lib.rs:5144) —
/// reconstructing from the raw component fields silently drops recoil kick
/// and lean roll, which is exactly why gun recoil was visible (baked into
/// the gun mesh's own transform) but the camera/crosshair never shook to
/// match. `yaw`/`pitch` are kept too (pre-recoil, straight off the
/// component) since they're still useful for e.g. UI that cares about the
/// player's intended look direction rather than its recoil-jittered one.
pub struct CameraSnapshot {
    pub eye: [f32; 3],
    pub rotation: [f32; 4],
    pub yaw: f32,
    pub pitch: f32,
    /// The real `Gun.ads_blend` (0 = hip fire, 1 = fully aimed) — drives
    /// whether/how strongly a renderer should show a scope-zoom overlay,
    /// instead of a probe-only guess at whether the player is aiming.
    pub ads_blend: f32,
    /// The real `ScopeZoom` resource value (magnification factor, e.g.
    /// 6..25 — see `SCOPE_MIN_ZOOM`/`SCOPE_MAX_ZOOM` in
    /// crates/client/src/lib.rs), so a scope overlay's FOV can be computed
    /// the same way the real game does (`SCOPE_BASE_FOV / zoom`) instead
    /// of a hardcoded guess.
    pub scope_zoom: f32,
    /// The real `Gun.wall_raise` (0 = normal, 1 = fully raised to clear a
    /// wall) — `update_scope_lens` (crates/client/src/lib.rs:6050) hides
    /// the scope even mid-ADS once this crosses
    /// `SCOPE_HIDDEN_WALL_RAISE_THRESHOLD`, so a renderer needs it too to
    /// match the real on/off condition exactly (`ads_blend` alone isn't
    /// enough).
    pub wall_raise: f32,
}

/// The real `ScopeCamera` entity's own state (crates/client/src/lib.rs,
/// `struct ScopeCamera` at line 1065) — reading this directly, instead of
/// reimplementing `sync_scope_camera`/`update_scope_lens`'s math
/// ourselves, is what makes the scope view's camera-shoulder-swap sweep,
/// recoil kick, reload dip/tilt, and walk-bob all "just work" without a
/// single line of that logic duplicated here: those two real systems
/// already compute the exact final `Transform`/`Camera.is_active`/
/// `Projection.fov` every tick inside this same headless App, we only
/// need to read the result. `desktop` (the real winit/wgpu build) shows
/// this exact system working correctly already, since it's 100% the same
/// `build_game_common` code — nothing about this needed reimplementing,
/// only extracting.
pub struct ScopeCameraSnapshot {
    pub eye: [f32; 3],
    pub rotation: [f32; 4],
    /// Real `Camera.is_active`, already gated on the exact
    /// `ads_blend > SCOPE_ACTIVE_ADS_THRESHOLD && wall_raise <
    /// SCOPE_HIDDEN_WALL_RAISE_THRESHOLD` condition by `update_scope_lens`
    /// — supersedes hand-reconstructing that condition from `ads_blend`/
    /// `wall_raise` alone.
    pub active: bool,
    /// Real `Projection::Perspective.fov` (radians), already
    /// `SCOPE_BASE_FOV / ScopeZoom` per `update_scope_lens`.
    pub fov: f32,
    /// The real `ScopeLens` entity's own `GlobalTransform.translation` —
    /// same gun-child quad `update_scope_lens` moves every tick with the
    /// shoulder-swap sweep, recoil kick, reload dip/tilt, and walk-bob
    /// (same math `sync_scope_camera` mirrors onto the camera itself, see
    /// this fn's own doc comment). The renderer projects this through the
    /// MAIN camera's own view_projection each frame to find where on
    /// screen the scope PIP should sit — previously hardcoded to a fixed
    /// screen-center position that never moved, so on recoil/reload the
    /// gun's real lens glass (and reticle) visibly kicked while the
    /// picture inside it stayed frozen dead center, instead of reading as
    /// one physical object like the desktop build (whose PIP is that same
    /// real 3D lens quad, not a separate screen-space overlay).
    pub lens_world_pos: [f32; 3],
    /// The real `ScopeLens` entity's own LOCAL `Transform.rotation` —
    /// `ChildOf(camera)`, so this is the lens's roll/tilt/sway RELATIVE
    /// to the camera (look_sway_roll + the recoil yaw-kick/roll-kick +
    /// reload tilt `update_scope_lens` stacks onto it), not its absolute
    /// world orientation. The renderer derives an on-screen 2D roll angle
    /// from this (rotate a reference up-vector by it, read off the
    /// resulting screen-space angle) and spins the PIP quad by that same
    /// angle, so the picture visibly rolls/tilts together with the real
    /// glass around it instead of only tracking its position.
    pub lens_local_rotation: [f32; 4],
}

#[derive(Default)]
pub struct RenderSnapshot {
    pub camera: Option<CameraSnapshot>,
    pub scope_camera: Option<ScopeCameraSnapshot>,
    pub zombies: Vec<SkinnedRenderState>,
    pub remote_players: Vec<SkinnedRenderState>,
    pub static_meshes: Vec<ExtractedMesh>,
    pub light: Option<LightSnapshot>,
    pub sun: Option<SunSnapshot>,
    pub floating_damage: Vec<FloatingDamageSnapshot>,
    pub health_bars: Vec<HealthBarSnapshot>,
    pub touch_buttons: Vec<TouchButtonSnapshot>,
    pub touch_joystick: Option<TouchJoystickSnapshot>,
    pub scope_zoom_slider: Option<ScopeZoomSliderSnapshot>,
    pub hud_texts: Vec<HudTextSnapshot>,
    pub crosshair_lines: Vec<CrosshairLineSnapshot>,
    pub settings_menu_panel: Option<SettingsMenuButtonSnapshot>,
    pub settings_menu_buttons: Vec<SettingsMenuButtonSnapshot>,
    pub settings_menu_texts: Vec<HudTextSnapshot>,
    pub game_over_panel: Option<SettingsMenuButtonSnapshot>,
    pub game_over_buttons: Vec<SettingsMenuButtonSnapshot>,
    pub game_over_texts: Vec<HudTextSnapshot>,
    pub map_select_panel: Option<SettingsMenuButtonSnapshot>,
    pub map_select_buttons: Vec<SettingsMenuButtonSnapshot>,
    pub map_select_texts: Vec<HudTextSnapshot>,
    /// The `MapMsaaOn` cheat-menu toggle (crate::lib) — previously only
    /// ever read by `apply_map_render_toggles` to sync bevy_render's own
    /// `Msaa` component (the desktop/winit build's render path), which
    /// this renderer never touches at all, so toggling it in-game visibly
    /// did nothing here. Extracted so native_metal_probe.rs can actually
    /// switch between its own MSAA-on/MSAA-off compiled pipeline pairs —
    /// same button, now with a real effect on both render paths instead of
    /// just one.
    pub map_msaa_on: bool,
    /// The `ShadowQuality` cheat-menu tier (crate::lib, 0..=5) — same
    /// "resource only ever wired to bevy_render's own component, dead on
    /// this renderer" gap as `map_msaa_on` above. `apply_map_render_toggles`
    /// still updates the real `MainSun` `DirectionalLight.shadows_enabled`
    /// every frame (desktop's render path reads it), but this renderer
    /// never touches that component — extracted separately so
    /// native_metal_probe.rs can pick its own shadow-map resolution (0 =
    /// skip the shadow-caster draw passes entirely) instead.
    pub shadow_quality: u8,
    /// The `LightQuality` cheat-menu tier (crate::lib, 0..=5) — same gap as
    /// `shadow_quality` above, for the sun's illuminance
    /// (`apply_map_render_toggles` scales `MainSun`'s
    /// `DirectionalLight.illuminance` to level/5, which only the desktop
    /// render path ever reads).
    pub light_quality: u8,
    /// The `RenderResolutionScale` settings-menu value (crate::lib,
    /// 0.1..=1.0) — same gap as `shadow_quality`/`light_quality` above.
    /// Desktop's `resize_scene_render_target` (crate::settings_menu) only
    /// ever resizes a bevy_render `Image` asset used as an offscreen
    /// camera render target; this renderer never touches bevy_render at
    /// all, so that resource previously had zero effect here regardless of
    /// what the settings-menu slider was set to. Extracted so
    /// native_metal_probe.rs can render its own offscreen scene target at
    /// this scale and upscale-blit it into the real drawable instead.
    pub render_resolution_scale: f32,
    /// The `ZombieTextureQuality` settings-menu tier (crate::lib, 0..=5,
    /// separate from cars' own `TextureQuality`) — this renderer's zombie/
    /// soldier skinned models were loaded once at startup from a fixed
    /// embedded texture and never touched it at all. Extracted so
    /// native_metal_probe.rs's apply_zombie_texture_quality can re-upload
    /// each zombie kind's base_color_texture from
    /// shared::zombie_kind_texture_path's tiered JPEGs when this changes.
    /// Cars don't need an equivalent snapshot field — their TextureQuality
    /// is already fully applied ECS-side (respawn_item_gallery_cars_on_
    /// quality_change bakes the tier's texture straight into a fresh
    /// StandardMaterial at spawn time), so by the time this extraction
    /// runs there's nothing left for the render layer to do.
    pub zombie_texture_quality: u8,
}

/// A still-alive floating damage number, already positioned/faded by
/// `update_floating_damage_texts` (crate::lib) — `screen_pos` is that
/// system's own `Node.left/.top` (logical points, top-left origin), read
/// straight off, not re-derived. Only entities left `Display::Flex` by
/// that system (on-screen, not yet expired) are extracted; see
/// `prime_camera_viewport`'s doc comment for why this used to always be
/// empty.
pub struct FloatingDamageSnapshot {
    pub screen_pos: [f32; 2],
    pub text: String,
    pub color: [f32; 4],
    pub font_size: f32,
}

/// A zombie health bar the game has already decided is worth showing —
/// `update_zombie_health_bars` (crate::lib) owns the "near + nearest
/// ZOMBIE_HEALTH_BAR_MAX_VISIBLE" selection and the `ZombieHealthBarsHidden`
/// cheat toggle; this only reads whatever that system left `Display::Flex`,
/// same as `FloatingDamageSnapshot`, instead of re-deriving that selection
/// here. `screen_pos`/`fraction` are its own `Node.left/.top` and the fill
/// child's `Node.width` (a `Val::Percent`), respectively.
pub struct HealthBarSnapshot {
    pub screen_pos: [f32; 2],
    pub fraction: f32,
}

/// One real touch-control button (any of JUMP/CROUCH/PRONE/SPRINT/LEAN
/// left+right/ADS/RELOAD/FIRE, plus the settings-menu "==" toggle) —
/// extracted generically via the `TouchControlButton` marker (crate::lib)
/// rather than one query per button type, since all 10 share the same
/// shape: a `Button` with a `BackgroundColor` and a static label. Unlike
/// `FloatingDamageSnapshot`/`HealthBarSnapshot`, `top_left`/`size` can't be
/// read straight off `Node.left/.top` — these buttons are positioned via a
/// mix of `left`/`right`/`top`/`bottom` anchors (see spawn_touch_controls),
/// so there's no single field that always holds "the" position. Instead
/// they come from the real computed layout (`ComputedNode`/
/// `UiGlobalTransform`, converted from physical to logical pixels via
/// `ComputedNode::inverse_scale_factor` — see `logical_rect`), the same
/// anchor-agnostic technique `multi_touch_button_interaction` (crate::lib)
/// already uses for touch hit-testing. `pressed` mirrors the button's own
/// `Interaction` so the renderer can draw a pressed-down visual without
/// re-deriving hit-testing separately.
pub struct TouchButtonSnapshot {
    pub top_left: [f32; 2],
    pub size: [f32; 2],
    pub color: [f32; 4],
    pub label: &'static str,
    pub pressed: bool,
}

/// The touch joystick's base + knob (crate::lib's `spawn_touch_controls`) —
/// not a `Button`/`TouchControlButton` (dragged, not tapped), so extracted
/// separately with the same `logical_rect` technique. `None` when
/// `TouchUiEnabled` is off (spawn_touch_controls never ran, so neither node
/// exists) — a fixed base + a knob that's already offset by
/// `update_touch_joystick_visual` (crate::lib) toward wherever the thumb
/// has dragged it, so drawing both each frame reproduces the exact same
/// drag visual the bevy_ui path already renders.
pub struct TouchJoystickSnapshot {
    pub base_top_left: [f32; 2],
    pub base_size: [f32; 2],
    pub knob_top_left: [f32; 2],
    pub knob_size: [f32; 2],
}

/// The scope-zoom slider's track + handle (crate::lib's `spawn_touch_
/// controls`) — same "dragged, not tapped, so extracted like the joystick
/// instead of TouchButtonSnapshot" reasoning as `TouchJoystickSnapshot`'s
/// own doc comment. `None` only when `TouchControlsEnabled` is off
/// (spawn_touch_controls never ran, so neither node exists) — while
/// simply not ADS'd, the track stays `Some` but zero-sized (the real
/// `Node.display: Display::None` toggle, `update_scope_zoom_slider_
/// visibility` in crate::lib, collapses it out of layout the same way a
/// `Display::None` `CrosshairRoot`'s children come back zero-sized — see
/// `CrosshairLineSnapshot`'s own extraction site), so the renderer just
/// naturally draws nothing visible rather than needing a separate
/// aiming-state field to check.
pub struct ScopeZoomSliderSnapshot {
    pub track_top_left: [f32; 2],
    pub track_size: [f32; 2],
    pub handle_top_left: [f32; 2],
    pub handle_size: [f32; 2],
}

/// One always-on HUD text readout (HP, ammo, or ping — crate::lib's
/// `HealthUi`/`AmmoUi`/`PingUi`) — generic over which one via the same
/// "read the real ComputedNode/UiGlobalTransform, not Node.left/.top"
/// technique `TouchButtonSnapshot` already uses, for the same reason: these
/// are anchored via a mix of `left`/`right`/`top`/`bottom` (HP is
/// left+top, ammo is right+bottom, ping is right+top), so there's no
/// single field that always holds "the" position. `text`/`color`/
/// `font_size` are read straight off the real `Text`/`TextColor`/
/// `TextFont` components each already carries — same values the desktop/
/// wgpu path renders, just re-drawn here with the bitmap font instead of
/// Bevy's own text pipeline (which this headless app never runs any text
/// layout/glyph-atlas machinery for — see UI_SHADER_SOURCE's own doc
/// comment on why a dedicated bitmap font exists in the first place).
/// `top_left`/`size` are the real node's own computed rect — the renderer
/// does NOT just draw at `top_left` directly, though: real Bevy text
/// (proportional font) and this bitmap font (fixed 5px-wide glyphs)
/// measure the same string to different widths, so whichever edge is the
/// text's real fixed anchor (left for HP, right for ammo/ping — see
/// spawn_health_ui/spawn_ammo_ui/spawn_ping_ui, crate::lib) needs to stay
/// fixed while the OTHER edge grows/shrinks to fit our font's own measured
/// width — the renderer infers which edge that is by checking which one
/// sits closer to its own screen edge (same idea as "nearest edge wins",
/// no extra data needed beyond this rect). Reused as-is for the settings
/// menu's own text (`settings_menu_texts` on `RenderSnapshot`) — same
/// shape, same reasoning, just a different source query (see
/// `collect_descendants`).
pub struct HudTextSnapshot {
    pub top_left: [f32; 2],
    pub size: [f32; 2],
    pub text: String,
    pub color: [f32; 4],
    pub font_size: f32,
}

/// One arm of the crosshair "+" (crate::lib's `CrosshairLine`) —
/// `update_crosshair` (crate::lib) repositions/resizes these 4 nodes every
/// frame (widening the gap with bullet spread, same real accuracy the next
/// shot will actually fire with), so reading their real computed layout
/// captures that math for free instead of re-deriving spread-to-gap
/// ourselves. Plain rects, same technique/limitation as everything else in
/// this file drawn via `ndc_ui_rect`/`draw_ui_quad` — see
/// `TouchButtonSnapshot`'s own doc comment.
pub struct CrosshairLineSnapshot {
    pub top_left: [f32; 2],
    pub size: [f32; 2],
    pub color: [f32; 4],
}

/// One button inside the settings menu (crate::lib's `SettingsMenuRoot`
/// subtree — page-nav `<`/`>`, resolution-scale presets, every cheat
/// toggle/action button, zombie-count +/-, sensitivity +/-, ...). Unlike
/// `TouchButtonSnapshot`, this is deliberately NOT keyed off a dedicated
/// marker component — the settings menu has dozens of distinct button
/// marker types (`CheatNoclipButton`, `ResolutionScaleButton`,
/// `SensitivityIncButton`, ...) and tagging every one individually the way
/// `TouchControlButton` does for the 10 touch-control buttons would mean a
/// new marker every time a settings row gets added. Instead
/// `collect_descendants` walks the real `Children` hierarchy from
/// `SettingsMenuRoot` once and this (plus `settings_menu_texts`, reusing
/// `HudTextSnapshot`) just asks "is this descendant a `Button`?"/"does it
/// carry `Text`?" — generic over content, the same way the settings menu
/// itself is generic over what page/row is spawned. No label field: labels
/// come from `settings_menu_texts`' own Text children, drawn as separate
/// quads exactly like every other text in this file.
pub struct SettingsMenuButtonSnapshot {
    pub top_left: [f32; 2],
    pub size: [f32; 2],
    pub color: [f32; 4],
}

/// Depth-first walk of the real `Children` hierarchy starting at (but not
/// including) `root`, collecting every descendant entity — used to scope
/// the settings-menu button/text extraction to "whatever is actually
/// inside `SettingsMenuRoot` this frame" without needing a dedicated
/// marker component on each one (see `SettingsMenuButtonSnapshot`'s own
/// doc comment). Small tree (a couple dozen nodes even with every cheat
/// row spawned), so plain recursion is fine — no need for
/// `find_first_skinned_mesh`'s iterative BFS queue.
fn collect_descendants(root: bevy::ecs::entity::Entity, children_query: &Query<&Children>, out: &mut Vec<bevy::ecs::entity::Entity>) {
    let Ok(children) = children_query.get(root) else { return };
    for &child in children.iter() {
        out.push(child);
        collect_descendants(child, children_query, out);
    }
}

/// Converts a node's real computed layout to a logical-points (matching
/// `Node.left/.top`'s own unit, see `ndc_ui_rect`'s doc comment) top-left +
/// size rect. `ComputedNode::size`/`UiGlobalTransform`'s translation are
/// both in physical pixels (see `multi_touch_button_interaction`'s own doc
/// comment); `inverse_scale_factor` is exactly the conversion factor bevy_ui
/// itself carries per-node for this, so no separate `Window` lookup is
/// needed here the way `multi_touch_button_interaction` needs one to go the
/// other direction (logical touch position -> physical, for hit-testing).
fn logical_rect(node: &ComputedNode, transform: &UiGlobalTransform) -> ([f32; 2], [f32; 2]) {
    let inv_scale = node.inverse_scale_factor();
    let size = node.size() * inv_scale;
    let center = transform.affine().translation * inv_scale;
    let top_left = center - size / 2.0;
    (top_left.to_array(), size.to_array())
}

/// Extracts a "panel root + everything inside it" UI subtree generically —
/// shared by the settings menu (`SettingsMenuRoot`) and the GAME OVER
/// overlay (`GameOverUi`), the same reasoning as `SettingsMenuButtonSnapshot`'s
/// own doc comment for why: neither wants a dedicated marker component on
/// every button/text spawned inside it (the settings menu has dozens of
/// distinct button types; GAME OVER only has one today, `RestartButton`,
/// but reusing the same generic walk means a future addition — a "Main
/// Menu" button, say — needs no new extraction code at all). Returns
/// `(panel, buttons, texts)`; all empty/`None` if no entity carries
/// `Root`, OR if `Root` exists but is currently `Display::None` (the
/// settings menu's own hide mechanism, toggled by `update_settings_menu`
/// — see `SettingsMenuVisible` — rather than despawned/respawned the way
/// `GameOverUi`/`MapSelectRoot` are; GAME OVER and map-select never hit
/// this branch since they simply don't exist while inactive). That check
/// matters: unlike a hidden CHILD (`CrosshairLine`, whose PARENT toggling
/// `Display::None` collapses it to a real zero-size `ComputedNode`), a
/// hidden ROOT's own subtree can retain a stale non-zero `ComputedNode`
/// from whenever it was last laid out (bevy_ui measures text size
/// independent of layout position) — extracting that produced this
/// project's own "closed settings menu's old page-0 text draws garbled
/// over the HP corner" bug, fixed by explicitly requiring `Display::Flex`
/// here instead of assuming a hidden subtree always reads back empty. The
/// panel itself (its own `BackgroundColor` — a dark translucent backdrop
/// for all three) isn't a `Button`, so it's extracted separately from the
/// descendant walk, same as `settings_menu_panel`'s own original doc
/// comment explained.
fn extract_panel<Root: bevy::ecs::component::Component>(
    world: &mut bevy::ecs::world::World,
) -> (Option<SettingsMenuButtonSnapshot>, Vec<SettingsMenuButtonSnapshot>, Vec<HudTextSnapshot>) {
    let mut root_state: SystemState<
        Query<(bevy::ecs::entity::Entity, &ComputedNode, &UiGlobalTransform, &BackgroundColor, &Node), With<Root>>,
    > = SystemState::new(world);
    let root_query = root_state.get(world);
    let Some((root, root_node, root_transform, root_bg, root_style)) = root_query.iter().next() else {
        return (None, Vec::new(), Vec::new());
    };
    if root_style.display != Display::Flex {
        return (None, Vec::new(), Vec::new());
    }
    let (top_left, size) = logical_rect(root_node, root_transform);
    let c = root_bg.0.to_srgba();
    let panel = Some(SettingsMenuButtonSnapshot { top_left, size, color: [c.red, c.green, c.blue, c.alpha] });

    let mut children_state: SystemState<Query<&Children>> = SystemState::new(world);
    let children_query = children_state.get(world);
    let mut descendants = Vec::new();
    collect_descendants(root, &children_query, &mut descendants);

    let mut button_state: SystemState<Query<(&ComputedNode, &UiGlobalTransform, &BackgroundColor), With<Button>>> = SystemState::new(world);
    let button_query = button_state.get(world);
    let buttons = descendants
        .iter()
        .filter_map(|&e| button_query.get(e).ok())
        .map(|(node, transform, bg)| {
            let (top_left, size) = logical_rect(node, transform);
            let c = bg.0.to_srgba();
            SettingsMenuButtonSnapshot { top_left, size, color: [c.red, c.green, c.blue, c.alpha] }
        })
        .collect();

    let mut text_state: SystemState<Query<(&ComputedNode, &UiGlobalTransform, &Text, &TextColor, &TextFont)>> = SystemState::new(world);
    let text_query = text_state.get(world);
    let texts = descendants
        .iter()
        .filter_map(|&e| text_query.get(e).ok())
        .map(|(node, transform, text, color, font)| {
            let (top_left, size) = logical_rect(node, transform);
            let c = color.0.to_srgba();
            HudTextSnapshot { top_left, size, text: text.0.clone(), color: [c.red, c.green, c.blue, c.alpha], font_size: font.font_size }
        })
        .collect();

    (panel, buttons, texts)
}

/// The scene's real `DirectionalLight` ("MainSun", crate::lib — spawned
/// unconditionally for both USE_TEST_ARENA and the Sponza scene, and per
/// its own doc comment there ("the arena instead just gets a brighter
/// ambient/directional light") is actually the *primary* light for the
/// test arena, with the single `PointLight` LightSnapshot already extracts
/// more of a secondary/fill light — missed on the first lighting-port pass,
/// which only extracted the point light.
pub struct SunSnapshot {
    /// The direction the light travels *toward* (the entity's own forward
    /// vector) — same convention native_metal_probe.rs's old hardcoded
    /// `light_dir` constant used, so `max(dot(normal, -sunDir), 0)` matches.
    pub direction: [f32; 3],
    pub color: [f32; 3],
}

/// The scene's real `PointLight`, extracted so the Metal renderer can
/// actually shade from where the light is instead of a hand-picked fixed
/// direction — lighting was never ported when the rest of the real game
/// moved onto this renderer (see native_metal_probe.rs's own fragment
/// shaders, which used a constant `light_dir` unrelated to any real light
/// in the scene). USE_TEST_ARENA (crate::lib) spawns exactly one overhead
/// `PointLight`, so `first()` is enough for now — if a scene with more
/// than one light ever needs supporting, this is the one place to extend
/// into a small array instead of a single `Option`.
pub struct LightSnapshot {
    pub position: [f32; 3],
    /// sRGB, 0..1 per channel — already in the same range the renderer's
    /// existing vertex/base-color colors use, unlike `PointLight::intensity`
    /// (lumens, can be in the millions) which this deliberately doesn't
    /// extract: the shader uses distance/range attenuation for falloff and
    /// this color purely as a tint, not a physically-based exposure value.
    pub color: [f32; 3],
    pub range: f32,
}

/// A skinned actor's (zombie or remote player) world transform plus its
/// already-animated skinning matrices — one per joint, ready to upload
/// straight into the renderer's joint buffer. Read directly from the real
/// `bevy_animation`/`TransformPlugin` state this headless App already
/// computes every tick (see `joint_matrices_for`'s own doc comment) instead
/// of re-sampling an animation clip by hand a second time — the same
/// "extract the real thing, don't reimplement it" principle this module
/// already applies to the camera/scope camera, now covering skinning too:
/// the old approach extracted a raw clip index + seek time and made the
/// RENDERER (skinned_gltf.rs's own hand-parsed copy of the glTF) re-sample
/// the clip independently — two separate implementations of the same
/// keyframe math that could silently drift apart from the desktop/wgpu
/// path's own `bevy_animation`. This extracts the actual per-joint result
/// instead, so both paths share the exact same animation computation.
/// Generic across gameplay type (used for both `snapshot.zombies` and
/// `snapshot.remote_players`) since `find_first_skinned_mesh`/
/// `joint_matrices_for` never cared what kind of entity `root` was either.
pub struct SkinnedRenderState {
    pub transform: Transform,
    pub joint_matrices: Vec<[[f32; 4]; 4]>,
    /// Which `SkinnedModelRegistry` entry this actor should draw as — only
    /// meaningful for zombies (`ZombieKind::Classic` for every non-zombie
    /// use of this struct, e.g. remote players, which pick their own fixed
    /// "soldier" model name at the call site instead of reading this).
    pub kind: ZombieKind,
}

/// Walks down from `root` (breadth-first, following `Children`) for the
/// first descendant carrying a `SkinnedMesh` — the mesh entity glTF's skin
/// import attaches it to, not necessarily `root` itself (which for a
/// zombie/soldier is the replicated/NetPlayer entity, several hierarchy
/// levels above the actual skinned glTF node). Mirrors the same
/// "walk the hierarchy looking for X" shape `find_animation_root`
/// (crates/shared) already uses, just top-down instead of bottom-up.
fn find_first_skinned_mesh(
    root: bevy::ecs::entity::Entity,
    children_query: &Query<&Children>,
    skinned_query: &Query<&SkinnedMesh>,
) -> Option<bevy::ecs::entity::Entity> {
    let mut queue = std::collections::VecDeque::from([root]);
    while let Some(entity) = queue.pop_front() {
        if skinned_query.contains(entity) {
            return Some(entity);
        }
        if let Ok(children) = children_query.get(entity) {
            queue.extend(children.iter());
        }
    }
    None
}

/// One already-animated skinning matrix per joint — `joint_matrix[i] =
/// worldTransform(joints[i]) * inverseBindMatrix[i]`, the standard GPU
/// skinning formula, computed here from data this headless App's own
/// `GltfPlugin`/`AnimationPlugin`/`TransformPlugin` already produce for
/// every real joint entity (not just the handful this codebase happens to
/// track by name for hit detection — see `SkinnedMesh.joints`'s own doc
/// comment for why this is fully generic across the whole skin, unlike
/// `ZombieHitboxBones`' 15 named bones). `inverse_bindposes_assets` comes
/// from the same glTF import as `skinned.joints` itself, so the two stay
/// index-aligned by construction — no separate file parse needed for
/// either half.
fn joint_matrices_for(
    skinned: &SkinnedMesh,
    globals: &Query<&GlobalTransform>,
    inverse_bindposes_assets: &Assets<bevy::mesh::skinning::SkinnedMeshInverseBindposes>,
) -> Vec<[[f32; 4]; 4]> {
    let Some(inverse_bindposes) = inverse_bindposes_assets.get(&skinned.inverse_bindposes) else {
        return Vec::new();
    };
    skinned
        .joints
        .iter()
        .zip(inverse_bindposes.iter())
        .map(|(&joint, inverse_bind)| {
            let world = globals.get(joint).map(GlobalTransform::to_matrix).unwrap_or_default();
            (world * *inverse_bind).to_cols_array_2d()
        })
        .collect()
}

impl Default for CameraSnapshot {
    fn default() -> Self {
        Self {
            eye: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            yaw: 0.0,
            pitch: 0.0,
            ads_blend: 0.0,
            scope_zoom: 8.0,
            wall_raise: 0.0,
        }
    }
}

/// Runs once per tick against the headless `App`'s `World` — cheap enough
/// (a handful of small queries plus re-reading two `Assets<T>` collections)
/// to call every frame rather than caching, matching
/// `native_metal_probe.rs`'s own per-frame extract call.
pub fn extract_render_snapshot(world: &mut bevy::ecs::world::World) -> RenderSnapshot {
    let mut snapshot = RenderSnapshot::default();

    // Camera: the FpsCamera entity's own world-space translation +
    // rotation (see CameraSnapshot's own doc comment for why the real
    // rotation, not a yaw/pitch reconstruction, is what carries
    // recoil/lean). `Gun.ads_blend`/`ScopeZoom` ride along here too since
    // a renderer needs them in lockstep with the camera pose to decide
    // whether/how to show a scope overlay.
    let mut camera_state: SystemState<(
        Query<(&GlobalTransform, &crate::FpsCamera)>,
        Query<&crate::Gun>,
        bevy::ecs::system::Res<crate::ScopeZoom>,
    )> = SystemState::new(world);
    let (camera_query, gun_query, scope_zoom) = camera_state.get(world);
    if let Some((transform, cam)) = camera_query.iter().next() {
        snapshot.camera = Some(CameraSnapshot {
            eye: transform.translation().to_array(),
            rotation: transform.rotation().to_array(),
            yaw: cam.yaw,
            pitch: cam.pitch,
            ads_blend: gun_query.iter().next().map(|g| g.ads_blend).unwrap_or(0.0),
            scope_zoom: scope_zoom.0,
            wall_raise: gun_query.iter().next().map(|g| g.wall_raise).unwrap_or(0.0),
        });
    }

    // Scope camera: read the real ScopeCamera entity's own final state
    // directly — see ScopeCameraSnapshot's own doc comment for why this
    // supersedes reimplementing sync_scope_camera/update_scope_lens.
    // ScopeLens's own GlobalTransform.translation rides along here too —
    // it's the real gun-mesh child quad update_scope_lens moves every
    // tick (shoulder-swap sweep, recoil kick, reload dip/tilt, walk-bob,
    // all the same math sync_scope_camera mirrors for the camera itself),
    // so a renderer needs its world position to know WHERE on screen the
    // scope picture should actually sit this frame — see lens_world_pos's
    // own doc comment on ScopeCameraSnapshot for why this replaced a
    // fixed-screen-position PIP. The LOCAL `Transform.rotation` (not
    // GlobalTransform — ScopeLens is `ChildOf(camera)`, see its own spawn
    // site in crate::lib's setup()) is the lens's own roll/tilt/sway
    // RELATIVE to the camera, i.e. exactly the on-screen 2D rotation the
    // PIP quad itself needs (see lens_local_rotation's own doc comment).
    let mut scope_state: SystemState<(
        Query<(&GlobalTransform, &bevy::camera::Camera, &bevy::camera::Projection), With<crate::ScopeCamera>>,
        Query<(&GlobalTransform, &Transform), With<crate::ScopeLens>>,
    )> = SystemState::new(world);
    let (scope_camera_query, scope_lens_query) = scope_state.get(world);
    if let Some((transform, camera, projection)) = scope_camera_query.iter().next() {
        let fov = match projection {
            bevy::camera::Projection::Perspective(p) => p.fov,
            _ => 0.0,
        };
        let (lens_world_pos, lens_local_rotation) = scope_lens_query
            .iter()
            .next()
            .map(|(global, local)| (global.translation().to_array(), local.rotation.to_array()))
            .unwrap_or((transform.translation().to_array(), [0.0, 0.0, 0.0, 1.0]));
        snapshot.scope_camera = Some(ScopeCameraSnapshot {
            eye: transform.translation().to_array(),
            rotation: transform.rotation().to_array(),
            active: camera.is_active,
            fov,
            lens_world_pos,
            lens_local_rotation,
        });
    }

    // Zombies: world-space transform plus real per-joint skinning matrices
    // — see SkinnedRenderState's own doc comment for why this reads the
    // already-animated skeleton instead of extracting a clip index/seek
    // time for the renderer to re-sample by hand. `entity` (the zombie
    // root) is where `find_first_skinned_mesh` starts walking down to find
    // the actual skinned glTF node — the zombie root itself is never the
    // skinned mesh entity (that's several hierarchy levels down, inside
    // the asynchronously-loaded glTF scene).
    let mut zombie_state: SystemState<(
        Query<(bevy::ecs::entity::Entity, &GlobalTransform, Option<&ZombieKind>), With<Zombie>>,
        Query<&Children>,
        Query<&SkinnedMesh>,
        Query<&GlobalTransform>,
        Res<Assets<bevy::mesh::skinning::SkinnedMeshInverseBindposes>>,
    )> = SystemState::new(world);
    let (zombie_query, children_query, skinned_mesh_query, joint_globals, inverse_bindposes_assets) = zombie_state.get(world);
    snapshot.zombies = zombie_query
        .iter()
        .map(|(entity, transform, kind)| {
            let joint_matrices = find_first_skinned_mesh(entity, &children_query, &skinned_mesh_query)
                .and_then(|mesh_entity| skinned_mesh_query.get(mesh_entity).ok())
                .map(|skinned| joint_matrices_for(skinned, &joint_globals, &inverse_bindposes_assets))
                .unwrap_or_default();
            SkinnedRenderState { transform: transform.compute_transform(), joint_matrices, kind: kind.copied().unwrap_or_default() }
        })
        .collect();

    // Remote players: NetPlayer entities that aren't this client's own.
    // `Without<LocalPlayer>` alone isn't reliable here — observed empirically
    // (debug log: camera at [0, 1.46, 14], the one "remote" player at
    // [0, 0.81, 14] — same X/Z as the camera, only offset in Y by the
    // ordinary eye-height-above-hip-pivot amount) meaning `mark_local_player`
    // (an observer on `Add<NetworkId>`, see crates/client/src/lib.rs:1299)
    // isn't tagging the local client's own entity in this headless setup,
    // root cause not yet found. Belt-and-suspenders fix: also drop any
    // NetPlayer whose horizontal (X/Z) position coincides with the camera's
    // — deliberately horizontal-only, since the camera sits above the
    // player's own pivot by a fixed eye-height offset even for the correct
    // entity, so a 3D distance check (tried first) missed this case. A
    // genuine remote player is never this close horizontally.
    // Same real-per-joint-matrices approach as zombies above (see
    // SkinnedRenderState's own doc comment) — `find_first_skinned_mesh`/
    // `joint_matrices_for` are generic over any root entity, so this is a
    // drop-in reuse, not a second implementation. `entity` here is the
    // NetPlayer root; the actual `SkinnedMesh` lives on a descendant inside
    // its `PlayerModelAnimRoot` child's asynchronously-loaded glTF scene
    // (see `on_net_player_replicated`/`wire_player_model_animations` in
    // crate::lib), same "root isn't the skinned entity" relationship as
    // zombies.
    const LOCAL_PLAYER_EXCLUSION_RADIUS_XZ: f32 = 1.0;
    let camera_eye_xz = snapshot.camera.as_ref().map(|c| bevy::math::Vec2::new(c.eye[0], c.eye[2]));
    let mut remote_state: SystemState<(
        Query<(bevy::ecs::entity::Entity, &GlobalTransform), (With<NetPlayer>, bevy::ecs::query::Without<crate::LocalPlayer>)>,
        Query<&Children>,
        Query<&SkinnedMesh>,
        Query<&GlobalTransform>,
        Res<Assets<bevy::mesh::skinning::SkinnedMeshInverseBindposes>>,
    )> = SystemState::new(world);
    let (remote_query, remote_children_query, remote_skinned_query, remote_joint_globals, remote_inverse_bindposes_assets) =
        remote_state.get(world);
    snapshot.remote_players = remote_query
        .iter()
        .map(|(entity, t)| (entity, t.compute_transform()))
        .filter(|(_, t)| match camera_eye_xz {
            Some(eye_xz) => {
                let pos_xz = bevy::math::Vec2::new(t.translation.x, t.translation.z);
                pos_xz.distance(eye_xz) > LOCAL_PLAYER_EXCLUSION_RADIUS_XZ
            }
            None => true,
        })
        .map(|(entity, transform)| {
            let joint_matrices = find_first_skinned_mesh(entity, &remote_children_query, &remote_skinned_query)
                .and_then(|mesh_entity| remote_skinned_query.get(mesh_entity).ok())
                .map(|skinned| joint_matrices_for(skinned, &remote_joint_globals, &remote_inverse_bindposes_assets))
                .unwrap_or_default();
            // `kind` is meaningless for a remote player (not a zombie) —
            // the call site that turns this into a SkinnedActorDraw always
            // picks the fixed "soldier" model name for entries from this
            // Vec, ignoring the field. Default (Classic) is just a filler.
            SkinnedRenderState { transform, joint_matrices, kind: ZombieKind::default() }
        })
        .collect();

    // Light: the scene's real PointLight — see LightSnapshot's own doc
    // comment for why only position/color/range (not raw lumen intensity)
    // are extracted.
    let mut light_state: SystemState<Query<(&PointLight, &GlobalTransform)>> = SystemState::new(world);
    snapshot.light = light_state.get(world).iter().next().map(|(light, transform)| {
        let c = light.color.to_srgba();
        LightSnapshot {
            position: transform.translation().to_array(),
            color: [c.red, c.green, c.blue],
            range: light.range,
        }
    });

    // Sun: the always-present DirectionalLight("MainSun") — see
    // SunSnapshot's own doc comment for why this is a separate extraction
    // from the point light above.
    let mut sun_state: SystemState<Query<(&DirectionalLight, &GlobalTransform)>> = SystemState::new(world);
    snapshot.sun = sun_state.get(world).iter().next().map(|(sun, transform)| {
        let c = sun.color.to_srgba();
        SunSnapshot { direction: transform.forward().to_array(), color: [c.red, c.green, c.blue] }
    });

    // MSAA toggle: see RenderSnapshot::map_msaa_on's own doc comment.
    let mut msaa_toggle_state: SystemState<Res<crate::MapMsaaOn>> = SystemState::new(world);
    snapshot.map_msaa_on = msaa_toggle_state.get(world).0;

    // Shadow/Light quality tiers: see RenderSnapshot::shadow_quality/
    // light_quality's own doc comments.
    let mut shadow_quality_state: SystemState<Res<crate::ShadowQuality>> = SystemState::new(world);
    snapshot.shadow_quality = shadow_quality_state.get(world).0;
    let mut light_quality_state: SystemState<Res<crate::LightQuality>> = SystemState::new(world);
    snapshot.light_quality = light_quality_state.get(world).0;

    // Resolution scale: see RenderSnapshot::render_resolution_scale's own
    // doc comment.
    let mut resolution_scale_state: SystemState<Res<crate::RenderResolutionScale>> = SystemState::new(world);
    snapshot.render_resolution_scale = resolution_scale_state.get(world).0;

    // Zombie texture quality: see RenderSnapshot::zombie_texture_quality's
    // own doc comment.
    let mut zombie_texture_quality_state: SystemState<Res<crate::ZombieTextureQuality>> = SystemState::new(world);
    snapshot.zombie_texture_quality = zombie_texture_quality_state.get(world).0;

    // Floating damage numbers: see FloatingDamageSnapshot's own doc comment.
    let mut damage_state: SystemState<
        Query<(&Node, &Text, &TextColor, &TextFont), With<crate::FloatingDamageText>>,
    > = SystemState::new(world);
    snapshot.floating_damage = damage_state
        .get(world)
        .iter()
        .filter(|(node, ..)| node.display == Display::Flex)
        .filter_map(|(node, text, color, font)| {
            let (Val::Px(x), Val::Px(y)) = (node.left, node.top) else { return None };
            let c = color.0.to_srgba();
            Some(FloatingDamageSnapshot {
                screen_pos: [x, y],
                text: text.0.clone(),
                color: [c.red, c.green, c.blue, c.alpha],
                font_size: font.font_size,
            })
        })
        .collect();

    // Zombie health bars: see HealthBarSnapshot's own doc comment. Both
    // queries live in one SystemState so the fill lookup below can borrow
    // World data for an arbitrary child entity without a second, separately
    // borrowed SystemState.
    let mut bar_state: SystemState<(
        Query<(&Node, &Children), With<crate::ZombieHealthBar>>,
        Query<&Node, With<crate::ZombieHealthBarFill>>,
    )> = SystemState::new(world);
    let (bars, fills) = bar_state.get(world);
    snapshot.health_bars = bars
        .iter()
        .filter(|(node, _)| node.display == Display::Flex)
        .filter_map(|(node, children)| {
            let (Val::Px(x), Val::Px(y)) = (node.left, node.top) else { return None };
            children.iter().find_map(|&child| match fills.get(child) {
                Ok(fill_node) => match fill_node.width {
                    Val::Percent(pct) => Some(HealthBarSnapshot { screen_pos: [x, y], fraction: (pct / 100.0).clamp(0.0, 1.0) }),
                    _ => None,
                },
                Err(_) => None,
            })
        })
        .collect();

    // Touch-control buttons: see TouchButtonSnapshot's own doc comment for
    // why ComputedNode/UiGlobalTransform (not Node.left/.top) is what's
    // read here.
    let mut touch_button_state: SystemState<
        Query<(&ComputedNode, &UiGlobalTransform, &BackgroundColor, &crate::TouchControlButton, &Interaction)>,
    > = SystemState::new(world);
    let touch_button_query = touch_button_state.get(world);
    snapshot.touch_buttons = touch_button_query
        .iter()
        .map(|(node, transform, bg, marker, interaction)| {
            let (top_left, size) = logical_rect(node, transform);
            let c = bg.0.to_srgba();
            TouchButtonSnapshot {
                top_left,
                size,
                color: [c.red, c.green, c.blue, c.alpha],
                label: marker.0,
                pressed: *interaction == Interaction::Pressed,
            }
        })
        .collect();

    // Touch joystick base + knob: see TouchJoystickSnapshot's own doc
    // comment for why this is a separate extraction from the buttons above.
    let mut joystick_state: SystemState<(
        Query<(&ComputedNode, &UiGlobalTransform), With<crate::TouchJoystickBase>>,
        Query<(&ComputedNode, &UiGlobalTransform), With<crate::TouchJoystickKnob>>,
    )> = SystemState::new(world);
    let (joystick_base_query, joystick_knob_query) = joystick_state.get(world);
    snapshot.touch_joystick = joystick_base_query.iter().next().zip(joystick_knob_query.iter().next()).map(
        |((base_node, base_transform), (knob_node, knob_transform))| {
            let (base_top_left, base_size) = logical_rect(base_node, base_transform);
            let (knob_top_left, knob_size) = logical_rect(knob_node, knob_transform);
            TouchJoystickSnapshot { base_top_left, base_size, knob_top_left, knob_size }
        },
    );

    // Scope-zoom slider track + handle: see ScopeZoomSliderSnapshot's own
    // doc comment.
    let mut zoom_slider_state: SystemState<(
        Query<(&ComputedNode, &UiGlobalTransform), With<crate::TouchScopeZoomSliderTrack>>,
        Query<(&ComputedNode, &UiGlobalTransform), With<crate::TouchScopeZoomSliderHandle>>,
    )> = SystemState::new(world);
    let (zoom_track_query, zoom_handle_query) = zoom_slider_state.get(world);
    snapshot.scope_zoom_slider = zoom_track_query.iter().next().zip(zoom_handle_query.iter().next()).map(
        |((track_node, track_transform), (handle_node, handle_transform))| {
            let (track_top_left, track_size) = logical_rect(track_node, track_transform);
            let (handle_top_left, handle_size) = logical_rect(handle_node, handle_transform);
            ScopeZoomSliderSnapshot { track_top_left, track_size, handle_top_left, handle_size }
        },
    );

    // HUD text readouts (HP/ammo/ping): see HudTextSnapshot's own doc
    // comment for why ComputedNode/UiGlobalTransform (not Node.left/.top)
    // is what's read here.
    let mut hud_text_state: SystemState<
        Query<
            (&ComputedNode, &UiGlobalTransform, &Text, &TextColor, &TextFont),
            bevy::ecs::query::Or<(With<crate::HealthUi>, With<crate::AmmoUi>, With<crate::PingUi>)>,
        >,
    > = SystemState::new(world);
    snapshot.hud_texts = hud_text_state
        .get(world)
        .iter()
        .map(|(node, transform, text, color, font)| {
            let (top_left, size) = logical_rect(node, transform);
            let c = color.0.to_srgba();
            HudTextSnapshot { top_left, size, text: text.0.clone(), color: [c.red, c.green, c.blue, c.alpha], font_size: font.font_size }
        })
        .collect();

    // Crosshair lines: see CrosshairLineSnapshot's own doc comment. A
    // Display::None CrosshairRoot (while ADS'd — see update_crosshair,
    // crate::lib) collapses its whole subtree out of layout, so its
    // CrosshairLine children naturally come back zero-sized here too — no
    // extra ADS check needed, same as touch controls' own idle-hide.
    let mut crosshair_state: SystemState<Query<(&ComputedNode, &UiGlobalTransform, &BackgroundColor), With<crate::CrosshairLine>>> =
        SystemState::new(world);
    snapshot.crosshair_lines = crosshair_state
        .get(world)
        .iter()
        .map(|(node, transform, bg)| {
            let (top_left, size) = logical_rect(node, transform);
            let c = bg.0.to_srgba();
            CrosshairLineSnapshot { top_left, size, color: [c.red, c.green, c.blue, c.alpha] }
        })
        .collect();

    // Settings menu + GAME OVER overlay: see extract_panel's own doc
    // comment for why both share this one generic "panel root + everything
    // inside it" extraction instead of a per-button-type marker. Each
    // leaves its Vecs empty/panel None if its own root doesn't exist this
    // frame — for the settings menu that's Display::None-collapsed (see
    // logical_rect's own zero-size behavior for hidden nodes), for GAME
    // OVER it's simply not spawned yet (a dynamic overlay, unlike
    // SettingsMenuRoot which always exists but hidden).
    let (settings_menu_panel, settings_menu_buttons, settings_menu_texts) = extract_panel::<crate::SettingsMenuRoot>(world);
    snapshot.settings_menu_panel = settings_menu_panel;
    snapshot.settings_menu_buttons = settings_menu_buttons;
    snapshot.settings_menu_texts = settings_menu_texts;

    let (game_over_panel, game_over_buttons, game_over_texts) = extract_panel::<crate::GameOverUi>(world);
    snapshot.game_over_panel = game_over_panel;
    snapshot.game_over_buttons = game_over_buttons;
    snapshot.game_over_texts = game_over_texts;

    let (map_select_panel, map_select_buttons, map_select_texts) = extract_panel::<crate::MapSelectRoot>(world);
    snapshot.map_select_panel = map_select_panel;
    snapshot.map_select_buttons = map_select_buttons;
    snapshot.map_select_texts = map_select_texts;

    // Static meshes: arena walls/floor + gun scene meshes, both plain
    // Mesh3d/MeshMaterial3d<StandardMaterial> entities — see this module's
    // doc comment on ExtractedMesh for why GlobalTransform (not local
    // Transform) is what makes the parented gun meshes come out correct.
    //
    // `Without<SkinnedMesh>` excludes zombie.glb/soldier.glb's own
    // Mesh3d/MeshMaterial3d child entities (Bevy's scene spawner
    // instantiates those from each Zombie/NetPlayer's SceneRoot, same as
    // it does for the gun's meshes here) — those already get drawn once,
    // correctly GPU-skinned, via `real_game_extracted`'s own
    // EntityKind::Zombie/Soldier skinned-model path (native_metal_probe.rs)
    // using the probe's separately-loaded zombie_model/soldier_model.
    // Without this filter, this query ALSO baked their raw bind-pose
    // vertex positions (no skinning applied — this query has no way to
    // apply joint transforms) as a second, oversized/unanimated "static"
    // mesh sitting on top of the correctly animated one — the exact same
    // "garbled giant mesh" failure mode as the gun's own GlobalTransform-
    // propagation bug noted above, just from skipping skinning instead of
    // skipping GlobalTransform.
    // Gun visibility: the mesh_query below has no notion of Bevy
    // `Visibility` at all — this headless App never runs the render
    // crate's visibility-propagation systems (no `InheritedVisibility`/
    // `ViewVisibility` to read even if it did), so without this the gun's
    // meshes always got baked into static_meshes regardless of the
    // `GunVisible` cheat toggle (crate::lib's `apply_map_render_toggles`
    // only ever updates the real `Gun` entity's `Visibility` component,
    // which only the desktop/wgpu render path actually reads) — same class
    // of bug `map_msaa_on` above was added to fix for MSAA. Only used to
    // skip drawing the gun entirely when the toggle is off — draw ORDER
    // (native_metal_probe.rs's draw_scene, opaque-vs-blend pass split) is
    // decided by the real material AlphaMode instead, see ExtractedMesh::
    // is_blend below, not by whether a mesh belongs to the gun.
    let mut gun_visible_state: SystemState<Res<crate::GunVisible>> = SystemState::new(world);
    let gun_visible = gun_visible_state.get(world).0;
    let gun_mesh_entities: std::collections::HashSet<bevy::ecs::entity::Entity> = {
        let mut gun_state: SystemState<(Query<bevy::ecs::entity::Entity, With<crate::Gun>>, Query<&Children>)> = SystemState::new(world);
        let (gun_query, children_query) = gun_state.get(world);
        let mut out = Vec::new();
        if let Some(gun_entity) = gun_query.iter().next() {
            out.push(gun_entity);
            collect_descendants(gun_entity, &children_query, &mut out);
        }
        out.into_iter().collect()
    };

    let mut mesh_state: SystemState<(
        Query<(bevy::ecs::entity::Entity, &Mesh3d, &MeshMaterial3d<StandardMaterial>, &GlobalTransform), Without<SkinnedMesh>>,
        Res<Assets<Mesh>>,
        Res<Assets<StandardMaterial>>,
        Res<Assets<Image>>,
    )> = SystemState::new(world);
    let (mesh_query, meshes, materials, images) = mesh_state.get(world);
    for (entity, mesh3d, material3d, transform) in mesh_query.iter() {
        let is_gun = gun_mesh_entities.contains(&entity);
        if is_gun && !gun_visible {
            continue;
        }
        let Some(mesh) = meshes.get(&mesh3d.0) else { continue };
        let Some(material) = materials.get(&material3d.0) else { continue };

        let positions = match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
            Some(bevy::mesh::VertexAttributeValues::Float32x3(p)) => p.clone(),
            _ => continue,
        };
        let normals = match mesh.attribute(Mesh::ATTRIBUTE_NORMAL) {
            Some(bevy::mesh::VertexAttributeValues::Float32x3(n)) => n.clone(),
            _ => Vec::new(),
        };
        let uvs = match mesh.attribute(Mesh::ATTRIBUTE_UV_0) {
            Some(bevy::mesh::VertexAttributeValues::Float32x2(uv)) => uv.clone(),
            _ => Vec::new(),
        };
        let tangents = match mesh.attribute(Mesh::ATTRIBUTE_TANGENT) {
            Some(bevy::mesh::VertexAttributeValues::Float32x4(t)) => t.clone(),
            _ => Vec::new(),
        };
        let indices = mesh
            .indices()
            .map(|i| i.iter().map(|v| v as u32).collect())
            .unwrap_or_default();

        // Only the common RGBA8 variants glTF base-color textures normally
        // decode to — anything else (compressed formats like KTX2/Basis,
        // etc.) this module has no reason to own a conversion for.
        let has_texture_handle = material.base_color_texture.is_some();
        let texture = material.base_color_texture.as_ref().and_then(|handle| {
            let image = images.get(handle)?;
            let data = image.data.as_ref()?;
            match image.texture_descriptor.format {
                TextureFormat::Rgba8UnormSrgb | TextureFormat::Rgba8Unorm => Some(ExtractedTexture {
                    id: handle.id(),
                    width: image.texture_descriptor.size.width,
                    height: image.texture_descriptor.size.height,
                    rgba: data.clone(),
                }),
                _ => None,
            }
        });
        // A material WITH a texture assigned that we couldn't decode (as
        // opposed to a material with no texture at all, which legitimately
        // renders via its flat base_color tint) previously fell back to an
        // opaque white placeholder — visible on the gun's scope lens as a
        // solid white/gray disk sitting inside the optic, since that
        // element's actual texture uses an unsupported format. Hiding the
        // mesh entirely is a deliberately temporary stand-in for real
        // format support until decoding whatever format that texture
        // actually is becomes worth doing.
        if has_texture_handle && texture.is_none() {
            continue;
        }

        // Same decode-or-skip rule as base_color_texture above, plus:
        // never surface a normal map without real tangents to build a TBN
        // basis from (see normal_texture's own doc comment on ExtractedMesh).
        let normal_texture = if tangents.is_empty() {
            None
        } else {
            material.normal_map_texture.as_ref().and_then(|handle| {
                let image = images.get(handle)?;
                let data = image.data.as_ref()?;
                match image.texture_descriptor.format {
                    TextureFormat::Rgba8UnormSrgb | TextureFormat::Rgba8Unorm => Some(ExtractedTexture {
                        id: handle.id(),
                        width: image.texture_descriptor.size.width,
                        height: image.texture_descriptor.size.height,
                        rgba: data.clone(),
                    }),
                    _ => None,
                }
            })
        };

        snapshot.static_meshes.push(ExtractedMesh {
            positions,
            normals,
            uvs,
            indices,
            base_color: {
                let s = material.base_color.to_srgba();
                [s.red, s.green, s.blue, s.alpha]
            },
            emissive: [
                material.emissive.red,
                material.emissive.green,
                material.emissive.blue,
                material.emissive.alpha,
            ],
            texture,
            tangents,
            normal_texture,
            transform: transform.compute_transform(),
            is_blend: matches!(
                material.alpha_mode,
                AlphaMode::Blend | AlphaMode::Premultiplied | AlphaMode::Add | AlphaMode::Multiply
            ),
        });
    }

    snapshot
}

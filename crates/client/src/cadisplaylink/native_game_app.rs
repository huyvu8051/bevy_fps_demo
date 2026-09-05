//! Boots the real game's gameplay App (physics, networking, zombie AI,
//! player prediction — everything in `build_game_common`/`setup()` from
//! `crate::lib`) headless, no window, no `WinitPlugin`, no
//! `RenderPlugin`/`PbrPlugin`/wgpu at all — this is what `native_metal_
//! probe.rs`'s real-game renderer runs and draws every frame on iOS.
//!
//! Our renderer never touched Bevy's own window/asset/GPU plumbing to
//! begin with (`native_metal_probe.rs` talks straight to the raw UIView's
//! CAMetalLayer, and reads input via direct FFI calls from Swift, not
//! Bevy's InputPlugin) — so the real gameplay App doesn't need
//! `WindowPlugin`'s `RawHandleWrapper`/`create_bevy_window` either. This
//! mirrors the project's own dedicated-server binary (also headless, also
//! runs the shared gameplay logic via a plain `App::update()` loop with no
//! window/render surface whatsoever).

use bevy::app::App;
use bevy::asset::AssetApp;

// `build_game_common` + `setup()` run fine with no window/render backend
// at all, once the right minimal plugin subset (below) is present —
// confirmed by actually running this headless; every plugin add below
// exists because leaving it out panicked at that exact point.
pub fn build_headless_game_ecs_app(physical_width: u32, physical_height: u32, scale_factor: f32) -> App {
    let mut app = App::new();
    // Proactive pass over bevy_internal's actual `DefaultPlugins` member
    // list (checked directly against
    // bevy_internal-0.18.1/src/default_plugins.rs) instead of continuing
    // to find gaps one panic at a time: every plugin in that list with no
    // window/GPU/render dependency, added here up front. Explicitly still
    // excluded, same reasoning as before: WindowPlugin, AccessibilityPlugin,
    // WinitPlugin, RenderPlugin, PipelinedRenderingPlugin, CorePipelinePlugin,
    // PostProcessPlugin (this is where Bloom/DepthOfField live — genuinely
    // can't run without a real GPU/RenderApp, not a headless-safe gap),
    // AntiAliasPlugin, UiRenderPlugin, PbrPlugin, GizmoRenderPlugin — every
    // one of those either creates an OS window/wgpu device directly or only
    // does render-world extraction work that assumes a RenderApp sub-app
    // already exists.
    app.add_plugins(bevy::app::PanicHandlerPlugin);
    app.add_plugins(bevy::log::LogPlugin::default());
    app.add_plugins(bevy::MinimalPlugins);
    app.add_plugins(bevy::diagnostic::DiagnosticsPlugin);
    // build_game_common calls init_state (ClientState), which needs
    // StatesPlugin's StateTransition schedule — normally pulled in for
    // free by DefaultPlugins. Confirmed as the first real gap by actually
    // running this headless: panicked with "The `StateTransition` schedule
    // is missing. Did you forget to add StatesPlugin..." before this line
    // was added.
    app.add_plugins(bevy::state::app::StatesPlugin);
    // Needed for correct `GlobalTransform` propagation (parent-relative
    // hierarchies like the gun's glTF scene, itself `ChildOf(camera)`) —
    // pure ECS hierarchy math, no window/GPU dependency. Without this,
    // `GlobalTransform` never gets computed from `Transform` + parent
    // chains, which is exactly what made the gun's meshes render garbled
    // when Step 4's extraction first tried using local `Transform` as if
    // it were world space for parented entities.
    app.add_plugins(bevy::transform::TransformPlugin);
    app.add_plugins(bevy::asset::AssetPlugin::default());
    // `AssetPlugin` only inserts `AssetServer`; it does NOT register the
    // `Mesh`/`Image` asset *types* themselves — that's normally done by
    // `MeshPlugin`/`ImagePlugin`, both bundled for free inside
    // `RenderPlugin`'s neighborhood in `DefaultPlugins`. Neither plugin
    // touches wgpu/RenderApp/GPU state in its `build()` (confirmed by
    // reading their source), so both are safe to add standalone here.
    // Found by running headless: `client::setup` panicked with
    // "Parameter `ResMut<Assets<Mesh>>` failed validation: Resource does
    // not exist" on the very first `app.update()`.
    app.add_plugins(bevy::mesh::MeshPlugin);
    app.add_plugins(bevy::image::ImagePlugin::default());
    // `ImagePlugin::build()` only *preregisters* `ImageLoader` (reserves the
    // jpg/png/... extension -> loader mapping so `asset_server.load()`
    // doesn't panic immediately). The actual loader instance only gets
    // registered via `register_asset_loader(ImageLoader::new(..))` inside
    // `bevy_render`'s `TexturePlugin::finish()` (bundled in `RenderPlugin`,
    // normally called by `App::run()`/`App::finish()` once a RenderApp
    // exists) — never added here. Root cause of cars/particle textures
    // never rendering on iOS: every `asset_server.load::<Image>()` call
    // just queues forever against a loader that was promised but never
    // instantiated, so `AssetServer::get_load_state()` reports
    // `LoadState::Loading` indefinitely, confirmed via on-device logging.
    // `.glb`-embedded images work because `GltfPlugin` decodes them itself
    // without going through this deferred registration path. Register the
    // loader ourselves with no compressed-texture support (we only load
    // plain jpg/png here, never basis/ktx2/dds).
    app.register_asset_loader(bevy::image::ImageLoader::new(
        bevy::image::CompressedImageFormats::NONE,
    ));
    // Also in DefaultPlugins' own list, also GPU-free (pure ECS data/
    // bookkeeping — the actual GPU work each does is entirely inside
    // render-world extraction systems that only run once a RenderApp
    // sub-app exists, same pattern as MeshPlugin/ImagePlugin above).
    app.add_plugins(bevy::camera::CameraPlugin);
    app.add_plugins(bevy::light::LightPlugin);
    app.add_plugins(bevy::text::TextPlugin);
    // UiPlugin's own image-node system (bevy_ui::widget::image::
    // update_image_content_size_system) reads Assets<TextureAtlasLayout>
    // unconditionally — that asset type normally comes from bevy_sprite's
    // SpritePlugin, which we don't want (its other systems assume a
    // RenderApp). Same "just register the asset type" fix as
    // StandardMaterial/AudioSource above.
    app.init_asset::<bevy::image::TextureAtlasLayout>();
    app.add_plugins(bevy::ui::UiPlugin::default());
    app.add_plugins(bevy::scene::ScenePlugin);
    app.add_plugins(bevy::gltf::GltfPlugin::default());
    // `StandardMaterial` is normally registered by `bevy_pbr::PbrPlugin` via
    // its internal `MaterialPlugin<StandardMaterial>`, but that plugin also
    // wires up render-world extraction systems that assume `RenderPlugin`
    // already stood up a `RenderApp` sub-app — not available headless. We
    // only need the asset *storage* (`Assets<StandardMaterial>`) so
    // `client::setup`'s `ResMut<Assets<StandardMaterial>>` param resolves;
    // call `init_asset` directly instead of pulling in all of `PbrPlugin`.
    // Found the same way as `MeshPlugin`/`ImagePlugin` above: running
    // headless panicked with "Parameter `ResMut<Assets<StandardMaterial>>`
    // failed validation: Resource does not exist".
    app.init_asset::<bevy::pbr::StandardMaterial>();
    // `AnimationGraph`/`AnimationClip` assets are normally registered by
    // `bevy_animation::AnimationPlugin`, which is GPU-free (pure ECS
    // animation sampling, no RenderApp/wgpu). Found the same way as the
    // above: headless run panicked on `ResMut<Assets<AnimationGraph>>`.
    app.add_plugins(bevy::animation::AnimationPlugin);
    // `setup()` loads sound effects via `asset_server.load::<AudioSource>`,
    // which needs the asset type registered — normally done by
    // `bevy_audio::AudioPlugin`. We don't need actual playback (no audio
    // device headless), just the asset registration so `.load()` doesn't
    // panic; `init_asset` alone is enough and avoids pulling in the real
    // audio backend (rodio/cpal) at all.
    app.init_asset::<bevy::audio::AudioSource>();
    // `client::multi_touch_button_interaction` (the on-screen joystick/
    // fire/ADS/reload button hit-testing) reads `Res<Touches>` — inserted
    // by `bevy::input::InputPlugin`, which is pure resource bookkeeping
    // (keyboard/mouse/touch input state), no window/GPU dependency at all
    // despite living conceptually next to WinitPlugin. Since our real
    // input path is direct FFI from Swift (see ffi.rs's touch/key/mouse
    // functions) rather than winit events, this resource just stays
    // permanently empty headless — fine, the system simply finds no
    // touches to process.
    app.add_plugins(bevy::input::InputPlugin);
    // `client::touch_move_axis` (joystick drag handling) reads
    // `Res<HoverMap>` — turns out that's inserted by `InteractionPlugin`,
    // not `PickingPlugin` itself (`PickingPlugin` alone wasn't enough, as
    // running headless with just that plugin still hit the same panic).
    // Both are pure ECS resource/event bookkeeping, no window/GPU
    // dependency, so pull in the whole `DefaultPickingPlugins` group
    // rather than hand-picking individual members.
    app.add_plugins(bevy::picking::DefaultPickingPlugins);
    // `DefaultPickingPlugins` includes a window-based mouse-picking backend
    // (`bevy_picking::input::mouse_pick_events`) that reads
    // `MessageReader<WindowEvent>` — that message type is normally
    // registered by `WindowPlugin`, which we deliberately don't add (no
    // window exists headless). Registering just the message type (no
    // actual `Window` entity, no `WinitPlugin`) is enough to satisfy the
    // reader; since no window ever exists, the message queue simply always
    // stays empty and this system is permanently a no-op — exactly the
    // "queries against a missing entity just don't iterate" gap the plan
    // called out.
    app.add_message::<bevy::window::WindowEvent>();
    // `client::draw_zombie_hitboxes` (debug gizmo drawing) reads
    // `Res<GizmoConfigStore>`, inserted by `bevy_gizmos::GizmoPlugin` —
    // pure config/ECS bookkeeping, the actual gizmo *rendering* only
    // happens in `bevy_gizmos`'s render-world extraction (part of
    // RenderPlugin, absent here), so gizmos are recorded but never drawn,
    // which is fine — this system just needs the resource to exist.
    app.add_plugins(bevy::gizmos::GizmoPlugin);
    // The game's own scene (`asset_server.load(SCENE_PATH)`, its Sponza
    // gltf, spawned via bevy_scene's `DynamicScene`/`scene_spawner_system`)
    // contains `MeshMaterial3d<StandardMaterial>` components, which
    // `scene_spawner_system` needs registered in the type registry to
    // reflect-insert them — normally done by `PbrPlugin`. Register just
    // this one type directly rather than pulling in all of `PbrPlugin`
    // (which assumes a `RenderApp` sub-app exists).
    app.register_type::<bevy::pbr::MeshMaterial3d<bevy::pbr::StandardMaterial>>();
    app.add_plugins(avian3d::prelude::PhysicsPlugins::default());
    app.add_plugins(bevy_replicon::prelude::RepliconPlugins);
    app.add_plugins(bevy_replicon_renet2::RepliconRenetPlugins);
    app.add_plugins(crate::GamePlugin);

    // Step 3 (input): several of the game's own input systems
    // (`touch_move_axis`, `touch_look`, `mouse_look`'s `CursorOptions` gate)
    // query `With<PrimaryWindow>`/`Query<&Window, ...>` and silently bail
    // every frame if no such entity exists — which is exactly the situation
    // pre-Step-3, since we deliberately never add `WindowPlugin`. Spawning
    // one plain `Window`+`PrimaryWindow`+`CursorOptions` entity (pure data,
    // no actual OS window, no `WinitPlugin` involved) is enough to make
    // those systems run unmodified against the real FFI-injected input
    // messages (see ffi.rs's new `native_metal_probe_touch_*`/`key_changed`/
    // `mouse_*` functions) exactly like the on-device/Simulator winit path
    // does. `CursorOptions.grab_mode` starts `Locked` — mirrors what a
    // connected Bluetooth mouse/keyboard would set via
    // `cadisplaylink_mouse_connected` on iOS (no on-screen cursor to
    // click-to-lock there either) — so `mouse_look` reads `MouseMotion`
    // immediately instead of early-returning.
    // Physical resolution + a real `scale_factor_override` (not the
    // WindowResolution default of 1.0) — MUST match Swift's real device
    // metrics (UIScreen.main.nativeScale + the Metal view's own point-size
    // bounds, threaded in via NativeMetalProbe::new/native_metal_probe_
    // create_real_game) or every touch-driven UI hit-test breaks: Swift
    // reports touch.location(in:) in POINTS (UIKit's own coordinate space),
    // while Bevy UI lays out Node/ComputedNode using window.width()/height()
    // (LOGICAL pixels = physical / scale_factor). A hardcoded physical size
    // with scale_factor stuck at 1.0 made "logical" 1170x2532 regardless of
    // the real device, while touches arrived in ~390x844-ish point space —
    // node.contains_point() then compared two different coordinate spaces,
    // so drag gestures (touch_scroll_settings_menu) silently never hit
    // their target node. Simple taps partly worked by coincidence wherever
    // the mismatched scales happened to still overlap, which is why this
    // surfaced as "scroll doesn't work" rather than "nothing works".
    app.world_mut().spawn((
        bevy::window::Window {
            resolution: bevy::window::WindowResolution::new(physical_width, physical_height)
                .with_scale_factor_override(scale_factor),
            ..Default::default()
        },
        bevy::window::PrimaryWindow,
        bevy::window::CursorOptions {
            grab_mode: bevy::window::CursorGrabMode::Locked,
            ..Default::default()
        },
    ));

    if app.plugins_state() != bevy::app::PluginsState::Ready {
        while app.plugins_state() == bevy::app::PluginsState::Adding {
            #[cfg(not(target_arch = "wasm32"))]
            bevy::tasks::tick_global_task_pools_on_main_thread();
        }
    }
    app.finish();
    app.cleanup();

    app
}

//! Phase 6 of the wgpu-replacement plan: the REAL game's gameplay App
//! (physics, networking, zombie AI, player prediction — everything in
//! `build_game_common`/`setup()` from `crate::lib`) running headless, no
//! window, no `WinitPlugin`, no `RenderPlugin`/`PbrPlugin`/wgpu at all.
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

// Step 1 (feasibility spike): does `build_game_common` + `setup()` run at
// all without any window/render backend? This was never tested before —
// every previous run of this code (device, Simulator, desktop) always had
// a full DefaultPlugins/WinitPlugin/RenderPlugin stack underneath it.
pub fn build_headless_game_ecs_app() -> App {
    let mut app = App::new();
    app.add_plugins(bevy::MinimalPlugins);
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
    crate::build_game_common(&mut app);

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

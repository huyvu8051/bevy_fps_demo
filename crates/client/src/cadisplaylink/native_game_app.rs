
use bevy::app::App;
use bevy::asset::AssetApp;

pub fn build_headless_game_ecs_app(physical_width: u32, physical_height: u32, scale_factor: f32) -> App {
    // Before anything spawns: it decides whether mesh and image data survives
    // on the CPU, which metal_renderer needs and bevy_render does not.
    crate::mark_native_renderer_in_use();

    let mut app = App::new();

    app.add_plugins(bevy::app::PanicHandlerPlugin);
    app.add_plugins(bevy::log::LogPlugin::default());
    app.add_plugins(bevy::MinimalPlugins);
    app.add_plugins(bevy::diagnostic::DiagnosticsPlugin);

    app.add_plugins(bevy::state::app::StatesPlugin);

    app.add_plugins(bevy::transform::TransformPlugin);
    app.add_plugins(bevy::asset::AssetPlugin::default());

    app.add_plugins(bevy::mesh::MeshPlugin);
    app.add_plugins(bevy::image::ImagePlugin::default());
    // Without this nothing registers a loader for .wav or .ogg.
    app.add_plugins(bevy::audio::AudioPlugin::default());

    // Every texture the game ships is ASTC in a KTX2 container. Without
    // RenderPlugin nothing tells Bevy what the GPU can take, so the loader
    // refuses them and the map draws untextured -- Apple silicon takes ASTC
    // natively, so say so.
    let formats = bevy::image::CompressedImageFormats::ASTC_LDR;
    app.register_asset_loader(bevy::image::ImageLoader::new(formats));
    app.insert_resource(bevy::image::CompressedImageFormatSupport(formats));

    app.add_plugins(bevy::camera::CameraPlugin);
    app.add_plugins(bevy::light::LightPlugin);
    app.add_plugins(bevy::text::TextPlugin);

    app.init_asset::<bevy::image::TextureAtlasLayout>();
    app.add_plugins(bevy::ui::UiPlugin::default());
    app.add_plugins(bevy::scene::ScenePlugin);
    app.add_plugins(bevy::world_serialization::WorldSerializationPlugin);
    app.add_plugins(bevy::gltf::GltfPlugin::default());
    metal_renderer::gltf_materials::register_gltf_standard_materials(&mut app);

    app.init_asset::<bevy::pbr::StandardMaterial>();

    app.add_plugins(bevy::animation::AnimationPlugin);

    app.init_asset::<bevy::audio::AudioSource>();

    app.add_plugins(bevy::input::InputPlugin);

    app.add_plugins(bevy::picking::DefaultPickingPlugins);

    app.add_message::<bevy::window::WindowEvent>();

    app.add_plugins(bevy::gizmos::GizmoPlugin);

    app.register_type::<bevy::pbr::MeshMaterial3d<bevy::pbr::StandardMaterial>>();
    app.add_plugins(avian3d::prelude::PhysicsPlugins::default());
    app.insert_resource(avian3d::prelude::SubstepCount(crate::PHYSICS_SUBSTEPS));
    // Without these the native path has physics and a player but no zombies,
    // no AI and no world_ready -- it has never actually run the game. They sit
    // beside PhysicsPlugins in main()'s plugin list for the windowed build and
    // were simply never carried across.
    app.add_plugins(crate::sim::SimPlugin);
    app.add_plugins(crate::zombie_nav::ZombieNavmeshPlugin);
    // No MaterialPlugin here: iOS draws through its own Metal renderer. The
    // asset collections still have to exist or the systems asking for them
    // panic, which across FFI aborts.
    app.init_asset::<crate::DamageOverlayMaterial>();
    app.init_asset::<crate::ScopeLensMaterial>();
    app.init_asset::<crate::MuzzleSmokeMaterial>();
    app.init_asset::<crate::MuzzleFlashMaterial>();

    app.add_plugins(crate::GamePlugin);

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
            bevy::tasks::tick_global_task_pools_on_main_thread();
        }
    }
    app.finish();
    app.cleanup();

    app
}


use bevy::app::App;
use bevy::asset::AssetApp;

pub fn build_headless_game_ecs_app(physical_width: u32, physical_height: u32, scale_factor: f32) -> App {
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
    // Without this nothing registers a loader for .wav or .ogg, and every
    // sound the game asks for fails: the footsteps have been failing this way
    // for as long as the hand-picked plugin list has existed.
    app.add_plugins(bevy::audio::AudioPlugin::default());

    app.register_asset_loader(bevy::image::ImageLoader::new(
        bevy::image::CompressedImageFormats::NONE,
    ));

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
    // The desktop build gets these from MaterialPlugin / UiMaterialPlugin,
    // which this list deliberately leaves out: iOS draws through its own
    // Metal renderer and has no Bevy render pipeline to attach them to. The
    // asset collections still have to exist, because GamePlugin's systems ask
    // for them by value and a missing resource is a panic, not a skip -- and a
    // panic inside the render frame callback crosses FFI and aborts.
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

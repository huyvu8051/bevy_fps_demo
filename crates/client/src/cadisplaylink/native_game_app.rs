
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

    app.register_asset_loader(bevy::image::ImageLoader::new(
        bevy::image::CompressedImageFormats::NONE,
    ));

    app.add_plugins(bevy::camera::CameraPlugin);
    app.add_plugins(bevy::light::LightPlugin);
    app.add_plugins(bevy::text::TextPlugin);

    app.init_asset::<bevy::image::TextureAtlasLayout>();
    app.add_plugins(bevy::ui::UiPlugin::default());
    app.add_plugins(bevy::scene::ScenePlugin);
    app.add_plugins(bevy::gltf::GltfPlugin::default());

    app.init_asset::<bevy::pbr::StandardMaterial>();

    app.add_plugins(bevy::animation::AnimationPlugin);

    app.init_asset::<bevy::audio::AudioSource>();

    app.add_plugins(bevy::input::InputPlugin);

    app.add_plugins(bevy::picking::DefaultPickingPlugins);

    app.add_message::<bevy::window::WindowEvent>();

    app.add_plugins(bevy::gizmos::GizmoPlugin);

    app.register_type::<bevy::pbr::MeshMaterial3d<bevy::pbr::StandardMaterial>>();
    app.add_plugins(avian3d::prelude::PhysicsPlugins::default());
    app.add_plugins(bevy_replicon::prelude::RepliconPlugins);
    app.add_plugins(bevy_replicon_renet2::RepliconRenetPlugins);
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

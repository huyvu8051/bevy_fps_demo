//! CADisplayLink-driven iOS entry point — an experiment to see whether
//! bypassing WinitPlugin's event loop reduces the ~20% idle CPU floor
//! measured on-device with the normal winit-driven MINIMAL_MAP_PROBE (see
//! bevyengine/bevy#5713 for the upstream discussion this is based on).
//! Ported/adapted from jinleili/bevy-in-app (MIT). Only ever reachable from
//! Swift via the extern "C" functions in ffi.rs — never touched by the
//! normal winit-driven app or by desktop builds.

use crate::build_game_common;
use bevy::app::{App, PluginGroup};
use bevy::ecs::{
    entity::Entity,
    system::{Commands, Query, SystemState},
    world::FromWorld,
};
use bevy::platform::collections::HashMap;
use bevy::window::{RawHandleWrapper, Window, WindowCreated, WindowWrapper};
use std::ops::Deref;
use uuid::Uuid;

mod ffi;
pub mod gltf_scene;
mod ios_view;
pub mod native_metal_probe;
pub mod skinned_gltf;
pub use ios_view::IosViewObj;

#[derive(Eq, Hash, PartialEq, Debug, Copy, Clone)]
struct WindowId(Uuid);

impl WindowId {
    fn new() -> Self {
        WindowId(Uuid::new_v4())
    }
}

#[derive(Debug)]
struct AppViewWindow(WindowWrapper<ios_view::AppView>);

impl Deref for AppViewWindow {
    type Target = ios_view::AppView;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Default)]
struct AppViews {
    views: HashMap<WindowId, AppViewWindow>,
    entity_to_window_id: HashMap<Entity, WindowId>,
}

impl AppViews {
    fn create_window(&mut self, view_obj: IosViewObj, entity: Entity) -> &AppViewWindow {
        let app_view = AppViewWindow(WindowWrapper::new(ios_view::AppView::new(view_obj)));
        let window_id = WindowId::new();
        self.entity_to_window_id.insert(entity, window_id);
        self.views.entry(window_id).insert(app_view).into_mut()
    }
}

// Called once, right after building the App and adding DefaultPlugins
// (with WinitPlugin disabled) — finds the Window entity WindowPlugin
// spawned on its own, attaches the raw UIView handle to it so
// bevy_render/wgpu can create a Metal surface, same shape as what winit
// would have done for us on the normal path.
fn create_bevy_window(app: &mut App, view_obj: IosViewObj) {
    app.init_non_send_resource::<AppViews>();

    let mut window_state: SystemState<Query<Entity, bevy::ecs::query::With<Window>>> = SystemState::from_world(app.world_mut());
    let Ok(entity) = window_state.get(app.world_mut()).single() else {
        return;
    };

    let (logical_res, scale_factor, raw_handle) = {
        let mut app_views = app.world_mut().non_send_resource_mut::<AppViews>();
        let app_view = app_views.create_window(view_obj, entity);
        (app_view.logical_resolution(), app_view.scale_factor, RawHandleWrapper::new(&app_view.0).unwrap())
    };

    let mut state: SystemState<(
        Commands,
        Query<&mut Window>,
        bevy::ecs::message::MessageWriter<WindowCreated>,
    )> = SystemState::from_world(app.world_mut());
    let (mut commands, mut windows, mut created_writer) = state.get_mut(app.world_mut());
    if let Ok(mut bevy_window) = windows.get_mut(entity) {
        bevy_window.resolution.set_scale_factor(scale_factor);
        bevy_window.resolution.set(logical_res.0, logical_res.1);
    }
    commands.entity(entity).insert(raw_handle);
    created_writer.write(WindowCreated { window: entity });
    state.apply(app.world_mut());
}

// Same plugin set main() adds (minus WinitPlugin/WinitSettings, which
// don't apply here), then build_game_common for everything else
// (networking, UI, zombies, debug tooling — all identical to the
// winit-driven app).
pub(crate) fn build_cadisplaylink_game_app(view_obj: IosViewObj) -> App {
    eprintln!("[cadisplaylink] build_cadisplaylink_game_app (REAL GAME) starting");
    use avian3d::prelude::PhysicsPlugins;
    use bevy::app::{PluginsState, TaskPoolOptions, TaskPoolPlugin};
    use bevy::dev_tools::fps_overlay::{FpsOverlayConfig, FpsOverlayPlugin};
    use bevy::picking::mesh_picking::MeshPickingPlugin;
    use bevy::window::WindowPlugin;
    use bevy::winit::WinitPlugin;
    use bevy::DefaultPlugins;
    use bevy_replicon::prelude::RepliconPlugins;
    use bevy_replicon_renet2::RepliconRenetPlugins;

    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins
            .build()
            .disable::<WinitPlugin>()
            .set(WindowPlugin::default())
            .set(TaskPoolPlugin {
                task_pool_options: TaskPoolOptions::with_num_threads(4),
                ..Default::default()
            }),
        MeshPickingPlugin,
        PhysicsPlugins::default(),
        FpsOverlayPlugin {
            config: FpsOverlayConfig {
                refresh_interval: std::time::Duration::from_secs_f32(1.0 / 60.0),
                ..Default::default()
            },
        },
        RepliconPlugins,
        RepliconRenetPlugins,
    ));
    build_game_common(&mut app);

    create_bevy_window(&mut app, view_obj);

    if app.plugins_state() != PluginsState::Ready {
        while app.plugins_state() == PluginsState::Adding {
            #[cfg(not(target_arch = "wasm32"))]
            bevy::tasks::tick_global_task_pools_on_main_thread();
        }
    }
    app.finish();
    app.cleanup();

    app
}

// See crate::fps_probe's own module doc comment — camera + a small static
// room, DefaultPlugins and nothing else (no picking, no physics, no
// networking, no FPS overlay, no game logic of any kind), to measure raw
// achievable FPS/CPU independent of everything else in this app.
pub(crate) fn build_cadisplaylink_fps_probe_app(view_obj: IosViewObj) -> App {
    eprintln!("[cadisplaylink] build_cadisplaylink_fps_probe_app (PROBE) starting");
    use bevy::app::{PluginsState, TaskPoolOptions, TaskPoolPlugin};
    use bevy::window::WindowPlugin;
    use bevy::winit::WinitPlugin;
    use bevy::DefaultPlugins;

    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .build()
            .disable::<WinitPlugin>()
            .set(WindowPlugin::default())
            .set(TaskPoolPlugin {
                task_pool_options: TaskPoolOptions::with_num_threads(4),
                ..Default::default()
            }),
    );
    app.add_systems(bevy::app::Startup, crate::fps_probe::spawn_arena_and_camera);
    app.add_systems(
        bevy::app::Update,
        (crate::fps_probe::fly_camera_move, crate::fps_probe::fly_camera_look),
    );

    create_bevy_window(&mut app, view_obj);

    if app.plugins_state() != PluginsState::Ready {
        while app.plugins_state() == PluginsState::Adding {
            #[cfg(not(target_arch = "wasm32"))]
            bevy::tasks::tick_global_task_pools_on_main_thread();
        }
    }
    app.finish();
    app.cleanup();

    app
}

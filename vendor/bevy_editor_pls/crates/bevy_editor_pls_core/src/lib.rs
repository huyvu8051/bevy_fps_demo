/// Editor systems, events and resources
pub mod editor;
/// Trait definition for new editor windows
pub mod editor_window;

use std::marker::PhantomData;

use bevy::prelude::*;
use bevy::camera::CameraUpdateSystems as CameraUpdateSystem;
use bevy::transform::TransformSystems as TransformSystem;
use bevy::window::{PrimaryWindow, WindowRef};
use bevy_inspector_egui::{
    bevy_egui::{EguiGlobalSettings, EguiPlugin, EguiPostUpdateSet as EguiSet, PrimaryEguiContext},
    DefaultInspectorConfigPlugin,
};
use editor::EditorInternalState;
use editor_window::EditorWindow;

pub use editor::{Editor, EditorEvent};

/// Re-export of [`egui_dock`]
pub use egui_dock;

/// Extension trait for [`App`] to add a new editor window type
pub trait AddEditorWindow {
    fn add_editor_window<W: EditorWindow>(&mut self) -> &mut Self;
}

struct WindowSetupPlugin<W>(PhantomData<fn() -> W>);
impl<W: EditorWindow> Plugin for WindowSetupPlugin<W> {
    fn build(&self, app: &mut App) {
        W::app_setup(app);
    }

    fn finish(&self, app: &mut App) {
        W::app_finish(app);
    }
}

impl AddEditorWindow for App {
    fn add_editor_window<W: EditorWindow>(&mut self) -> &mut Self {
        let mut editor = self.world_mut().get_resource_mut::<Editor>().expect("Editor resource missing. Make sure to add the `EditorPlugin` before calling `app.add_editor_window`.");
        editor.add_window::<W>();

        self.add_plugins(WindowSetupPlugin::<W>(PhantomData));

        self
    }
}

#[derive(SystemSet, Clone, Copy, Debug, Hash, Eq, PartialEq)]
pub enum EditorSet {
    /// In [`CoreSet::PostUpdate`]
    UI,
}

pub struct EditorPlugin {
    pub window: WindowRef,
}
impl Plugin for EditorPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<DefaultInspectorConfigPlugin>() {
            app.add_plugins(DefaultInspectorConfigPlugin);
        }
        if !app.is_plugin_added::<EguiPlugin>() {
            // bevy_egui 0.39 defaults to multipass mode
            // (enable_multipass_for_primary_context: true), which requires
            // every UI system to live in the EguiPrimaryContextPass
            // schedule — Editor::system (below) still draws in the old
            // Update schedule like every other system in this fork, so
            // under multipass its egui calls happen outside the pass
            // window and are silently dropped: state toggles correctly
            // (pressing E does flip `active`) but nothing ever paints,
            // reading as "nothing happens". Single-pass mode keeps the
            // old Update-schedule-anywhere behavior this whole crate is
            // built around.
            app.add_plugins(EguiPlugin {
                enable_multipass_for_primary_context: false,
                ..default()
            });
        }

        // bevy_egui's own auto_create_primary_context grabs whichever
        // Camera it happens to see added FIRST (setup_primary_egui_context_
        // system, PreStartup+PreUpdate, iterating an Added<Camera> query in
        // unspecified order) — with 4 cameras all spawned at PreStartup
        // here (this crate's own 3 editor cameras in
        // cameras::spawn_editor_cameras, plus whatever camera the host app
        // has), which one wins the primary EguiContext isn't guaranteed,
        // and 3 of those 4 start `is_active: false` (only the selected
        // ActiveEditorCamera gets turned on). If the context lands on an
        // inactive camera, its egui pass never renders at all — Editor::
        // system finds the context fine now (see its own fix) and runs
        // every frame, but the paint output goes to a camera that draws
        // nothing, so the editor UI stays invisible regardless of
        // Editor.active. Disabling auto-creation and owning exactly one
        // always-active, render-last (`order` very high) 2D camera
        // ourselves removes that race entirely.
        app.world_mut().resource_mut::<EguiGlobalSettings>().auto_create_primary_context = false;
        app.world_mut().spawn((
            Camera2d,
            Camera {
                order: i32::MAX as isize,
                // CameraOutputMode::Write's own doc comment: "If
                // [blend_state] is not set, the output will be written
                // as-is, ignoring clear_color and the existing data in
                // the final render target" — i.e. the DEFAULT output
                // mode REPLACES whatever the scene's own camera already
                // wrote instead of blending on top of it. With
                // clear_color: None this camera's intermediate texture
                // starts fully transparent, so writing it "as-is"
                // without alpha blending would have blanked the whole
                // window to transparent (not just where egui actually
                // painted) — every previous attempt at this fix kept the
                // scene visible and unchanged, meaning this camera's
                // pass was actually never compositing onto the window at
                // all. Explicit alpha blending is what makes an overlay
                // camera correctly show through everywhere it drew
                // nothing and paint everywhere it did.
                output_mode: bevy::camera::CameraOutputMode::Write {
                    blend_state: Some(bevy::render::render_resource::BlendState::ALPHA_BLENDING),
                    clear_color: ClearColorConfig::None,
                },
                ..default()
            },
            // RenderTarget is its own component now (not a Camera field —
            // see cameras::spawn_editor_cameras's own comment on this),
            // and NOT a required-component default of Camera — a camera
            // missing it renders to nothing at all, silently. This was
            // the actual final cause of "toggling flips Editor.active and
            // Editor::system finds the context and draws every frame, but
            // still nothing ever appears": this camera never had a
            // window to render into in the first place.
            bevy::camera::RenderTarget::Window(self.window),
            Name::new("bevy_editor_pls egui host camera"),
            PrimaryEguiContext,
        ));

        let (window_entity, always_active) = match self.window {
            WindowRef::Primary => {
                let entity = app
                    .world_mut()
                    .query_filtered::<Entity, With<PrimaryWindow>>()
                    .single(app.world())
                    .expect("no primary window found");
                (entity, false)
            }
            WindowRef::Entity(entity) => (entity, true),
        };

        app.insert_resource(Editor::new(window_entity, always_active))
            .init_resource::<EditorInternalState>()
            .add_message::<EditorEvent>()
            .configure_sets(PostUpdate, EditorSet::UI)
            .add_systems(
                Update,
                Editor::system
                    .in_set(EditorSet::UI)
                    .before(TransformSystem::Propagate)
                    .before(CameraUpdateSystem)
                    .before(EguiSet::ProcessOutput),
            );
    }
}

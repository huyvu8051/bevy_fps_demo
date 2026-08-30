use std::any::TypeId;

use super::add::{AddWindow, AddWindowState};
use super::hierarchy::HierarchyWindow;
use bevy::asset::UntypedAssetId;
use bevy::prelude::{AppTypeRegistry, Entity, Transform, Vec3, World};
use bevy::reflect::TypeRegistry;
use bevy_editor_pls_core::editor_window::{EditorWindow, EditorWindowContext};
use bevy_inspector_egui::bevy_inspector::hierarchy::SelectedEntities;
use bevy_inspector_egui::{bevy_inspector, egui};

#[derive(Eq, PartialEq)]
pub enum InspectorSelection {
    Entities,
    Resource(TypeId, String),
    Asset(TypeId, String, UntypedAssetId),
}

pub struct InspectorState {
    pub selected: InspectorSelection,
}

impl Default for InspectorState {
    fn default() -> Self {
        Self {
            selected: InspectorSelection::Entities,
        }
    }
}

pub struct InspectorWindow;
impl EditorWindow for InspectorWindow {
    type State = InspectorState;
    const NAME: &'static str = "Inspector";

    fn ui(world: &mut World, cx: EditorWindowContext, ui: &mut egui::Ui) {
        let type_registry = world.resource::<AppTypeRegistry>().0.clone();
        let type_registry = type_registry.read();

        let selected = &cx.state::<Self>().unwrap().selected;
        let selected_entities = &cx.state::<HierarchyWindow>().unwrap().selected;

        let add_window_state = cx.state::<AddWindow>();
        inspector(
            world,
            selected,
            selected_entities,
            ui,
            add_window_state,
            &type_registry,
        );
    }
}

fn inspector(
    world: &mut World,
    selected: &InspectorSelection,
    selected_entities: &SelectedEntities,
    ui: &mut egui::Ui,
    add_window_state: Option<&AddWindowState>,
    type_registry: &TypeRegistry,
) {
    egui::ScrollArea::vertical().show(ui, |ui| match *selected {
        InspectorSelection::Entities => match selected_entities.as_slice() {
            [] => {
                ui.label("No entity selected");
            }
            &[entity] => {
                bevy_inspector::ui_for_entity(world, entity, ui);
                copy_transform_ui(ui, entity, world);
                add_ui(ui, &[entity], world, add_window_state);
            }
            entities => {
                bevy_inspector::ui_for_entities_shared_components(world, entities, ui);
                add_ui(ui, entities, world, add_window_state);
            }
        },
        InspectorSelection::Resource(type_id, ref name) => {
            ui.label(name);
            bevy_inspector::by_type_id::ui_for_resource(world, type_id, ui, name, type_registry)
        }
        InspectorSelection::Asset(type_id, ref name, handle) => {
            ui.label(name);
            bevy_inspector::by_type_id::ui_for_asset(world, type_id, handle, ui, type_registry);
        }
    });
}

fn add_ui(
    ui: &mut egui::Ui,
    entities: &[Entity],
    world: &mut World,
    add_window_state: Option<&AddWindowState>,
) {
    if let Some(add_window_state) = add_window_state {
        let layout = egui::Layout::top_down(egui::Align::Center).with_cross_justify(true);
        ui.with_layout(layout, |ui| {
            ui.menu_button("+", |ui| {
                if let Some(add_item) = crate::add::add_ui(ui, add_window_state) {
                    for entity in entities {
                        add_item.add_to_entity(world, *entity);
                    }
                }
            });
        });
    }
}

// Requested workflow: this editor has no "export to code" step (edits are
// client-local, see EditorPlugin's own doc comments) — the intended
// process is "move it in the editor, read the numbers off the Inspector,
// type them into the level's .rs file by hand". This button just removes
// the error-prone "read digits off screen, retype" part: it formats the
// selected entity's current Transform as ready-to-paste Rust and puts it
// on the clipboard, so translation/rotation/scale round-trip exactly
// instead of via eyeballed decimals.
fn copy_transform_ui(ui: &mut egui::Ui, entity: Entity, world: &mut World) {
    let Some(transform) = world.get::<Transform>(entity) else {
        return;
    };
    let Vec3 { x, y, z } = transform.translation;
    let code = format!(
        "Transform {{\n    translation: Vec3::new({x:?}, {y:?}, {z:?}),\n    rotation: Quat::from_xyzw({:?}, {:?}, {:?}, {:?}),\n    scale: Vec3::new({:?}, {:?}, {:?}),\n}}",
        transform.rotation.x,
        transform.rotation.y,
        transform.rotation.z,
        transform.rotation.w,
        transform.scale.x,
        transform.scale.y,
        transform.scale.z,
    );
    if ui
        .button("📋 Copy Transform as Rust code")
        .on_hover_text("Copies this entity's current Transform, formatted as a Rust struct literal, to the clipboard")
        .clicked()
    {
        ui.ctx().copy_text(code);
    }
}

pub fn label_button(ui: &mut egui::Ui, text: &str, text_color: egui::Color32) -> bool {
    ui.add(egui::Button::new(egui::RichText::new(text).color(text_color)).frame(false))
        .clicked()
}

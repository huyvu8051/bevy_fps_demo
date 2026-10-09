use bevy::app::App;
use bevy::asset::{Handle, LoadContext};
use bevy::ecs::prelude::*;
use bevy::gltf::extensions::{
    ErasedGltfExtensionHandler, GltfExtensionHandler, GltfExtensionHandlers,
};
use bevy::gltf::{gltf, GltfAssetLabel, GltfLoaderSettings, GltfMaterial};
use bevy::pbr::{MeshMaterial3d, StandardMaterial};

/// glTF scenes arrive with `Mesh3d` but no material; `PbrPlugin` is what
/// normally turns each `GltfMaterial` into a `StandardMaterial` and attaches
/// it. The iOS app has no `PbrPlugin`, so without this every glTF map loads
/// invisible.
pub(crate) fn register_gltf_standard_materials(app: &mut App) {
    app.world_mut()
        .resource_mut::<GltfExtensionHandlers>()
        .0
        .write_blocking()
        .push(Box::new(StandardMaterialFromGltf));
}

#[derive(Clone)]
struct StandardMaterialFromGltf;

fn standard_material_label(material_label: &str) -> String {
    format!("{material_label}/std")
}

/// Only the fields the Metal renderer reads. `bevy_pbr` has a complete
/// conversion but keeps it private, and the renderer ignores everything else.
fn standard_material_from(material: &GltfMaterial) -> StandardMaterial {
    StandardMaterial {
        base_color: material.base_color,
        base_color_texture: material.base_color_texture.clone(),
        emissive: material.emissive,
        normal_map_texture: material.normal_map_texture.clone(),
        alpha_mode: material.alpha_mode,
        double_sided: material.double_sided,
        cull_mode: material.cull_mode,
        unlit: material.unlit,
        ..StandardMaterial::default()
    }
}

impl GltfExtensionHandler for StandardMaterialFromGltf {
    fn dyn_clone(&self) -> Box<dyn ErasedGltfExtensionHandler> {
        Box::new(self.clone())
    }

    fn on_root(
        &mut self,
        load_context: &mut LoadContext<'_>,
        _gltf: &gltf::Gltf,
        _settings: &GltfLoaderSettings,
    ) {
        load_context.add_labeled_asset(
            standard_material_label(&GltfAssetLabel::DefaultMaterial.to_string()),
            standard_material_from(&GltfMaterial::default()),
        );
    }

    fn on_material(
        &mut self,
        load_context: &mut LoadContext<'_>,
        _gltf_material: &gltf::Material,
        _material: Handle<GltfMaterial>,
        material_asset: &GltfMaterial,
        material_label: &str,
    ) {
        load_context.add_labeled_asset(
            standard_material_label(material_label),
            standard_material_from(material_asset),
        );
    }

    fn on_spawn_mesh_and_material(
        &mut self,
        load_context: &mut LoadContext<'_>,
        _primitive: &gltf::Primitive,
        _mesh: &gltf::Mesh,
        _material: &gltf::Material,
        entity: &mut EntityWorldMut,
        material_label: &str,
    ) {
        let handle = load_context
            .get_label_handle::<StandardMaterial>(standard_material_label(material_label));
        entity.insert(MeshMaterial3d(handle));
    }
}

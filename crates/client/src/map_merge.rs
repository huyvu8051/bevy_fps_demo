use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, MeshVertexAttribute, PrimitiveTopology, VertexAttributeValues};
use bevy::prelude::*;
use std::collections::HashMap;

use crate::map::MapRoot;

/// Whether the map's static geometry is welded after it loads. desert_city is
/// 2363 mesh primitives over 28 materials, each its own entity every frame.
#[derive(Resource, Default)]
pub(crate) struct MergeMapMeshes(pub(crate) bool);

#[derive(Component)]
pub(crate) struct MergedMapMesh;

/// Cells, not one mesh per material: a mesh spanning the map can never be
/// culled. Metres.
const MERGE_CELL_SIZE: f32 = 24.0;

/// Meshes only weld to others with the same attributes: a merged mesh cannot
/// have a colour channel on some vertices and not others.
#[derive(PartialEq, Eq, Hash, Clone, Copy)]
struct Layout {
    colour: bool,
}

#[derive(PartialEq, Eq, Hash)]
struct Group {
    material: AssetId<StandardMaterial>,
    cell: IVec3,
    layout: Layout,
}

#[derive(Default)]
struct Run {
    /// AssetId is the key, but a material cannot be rebuilt from an id.
    material: Handle<StandardMaterial>,
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    colours: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

fn float3(mesh: &Mesh, attribute: MeshVertexAttribute) -> Option<&Vec<[f32; 3]>> {
    match mesh.attribute(attribute)? {
        VertexAttributeValues::Float32x3(values) => Some(values),
        _ => None,
    }
}

/// Welds the map's static meshes into one per material per cell.
///
/// Runs once after world_ready, which is also the signal that Avian has built
/// the colliders. Any earlier and the map loses its collision.
pub(crate) fn merge_static_map_meshes(
    mut commands: Commands,
    enabled: Res<MergeMapMeshes>,
    mut meshes: ResMut<Assets<Mesh>>,
    map_root: Query<Entity, With<MapRoot>>,
    children: Query<&Children>,
    parts: Query<(Entity, &Mesh3d, &MeshMaterial3d<StandardMaterial>, &GlobalTransform)>,
    mut done: Local<bool>,
) {
    if *done || !enabled.0 {
        return;
    }
    let Ok(root) = map_root.single() else {
        return;
    };
    *done = true;

    let mut runs: HashMap<Group, Run> = HashMap::new();
    let mut welded = Vec::new();

    for entity in children.iter_descendants(root) {
        let Ok((entity, mesh_handle, material, global)) = parts.get(entity) else {
            continue;
        };
        let Some(mesh) = meshes.get(&mesh_handle.0) else {
            continue;
        };
        if mesh.primitive_topology() != PrimitiveTopology::TriangleList {
            continue;
        }
        let Some(positions) = float3(mesh, Mesh::ATTRIBUTE_POSITION) else {
            continue;
        };
        let normals = float3(mesh, Mesh::ATTRIBUTE_NORMAL);
        let uvs = match mesh.attribute(Mesh::ATTRIBUTE_UV_0) {
            Some(VertexAttributeValues::Float32x2(values)) => Some(values),
            _ => None,
        };
        let colours = match mesh.attribute(Mesh::ATTRIBUTE_COLOR) {
            Some(VertexAttributeValues::Float32x4(values)) => Some(values),
            _ => None,
        };

        let affine = global.affine();
        // Inverse transpose, not the matrix: non-uniform scale tilts normals.
        let normal_matrix = affine.matrix3.inverse().transpose();

        let centre = affine.transform_point3(
            positions
                .iter()
                .fold(Vec3::ZERO, |sum, p| sum + Vec3::from(*p))
                / positions.len().max(1) as f32,
        );
        let group = Group {
            material: material.0.id(),
            cell: (centre / MERGE_CELL_SIZE).floor().as_ivec3(),
            layout: Layout { colour: colours.is_some() },
        };
        let run = runs.entry(group).or_insert_with(|| Run {
            material: material.0.clone(),
            ..default()
        });
        let base = run.positions.len() as u32;

        for (i, position) in positions.iter().enumerate() {
            run.positions.push(affine.transform_point3(Vec3::from(*position)).to_array());
            let normal = normals
                .map(|n| (normal_matrix * Vec3::from(n[i])).normalize_or_zero())
                .unwrap_or(Vec3::Y);
            run.normals.push(normal.to_array());
            run.uvs.push(uvs.map(|u| u[i]).unwrap_or([0.0, 0.0]));
            if let Some(colours) = colours {
                run.colours.push(colours[i]);
            }
        }
        match mesh.indices() {
            Some(Indices::U32(indices)) => run.indices.extend(indices.iter().map(|i| base + i)),
            Some(Indices::U16(indices)) => {
                run.indices.extend(indices.iter().map(|i| base + *i as u32))
            }
            None => run.indices.extend(base..base + positions.len() as u32),
        }
        welded.push(entity);
    }

    let groups = runs.len();
    for (group, run) in runs {
        let material = run.material.clone();
        // RENDER_WORLD only: nothing reads this copy back.
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, run.positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, run.normals)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, run.uvs);
        if group.layout.colour {
            mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, run.colours);
        }
        mesh.insert_indices(Indices::U32(run.indices));

        commands.spawn((
            Name::new("Merged map mesh"),
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(material),
            Transform::IDENTITY,
            MergedMapMesh,
            ChildOf(root),
        ));
    }

    // The originals keep their collider and stop being drawn. SyncToRenderWorld
    // and the visibility set go too: Bevy does not remove required components
    // when the thing requiring them goes, so the entity would carry on being
    // copied into the render world every frame.
    for entity in &welded {
        commands.entity(*entity).remove::<(
            Mesh3d,
            MeshMaterial3d<StandardMaterial>,
            bevy::render::sync_world::SyncToRenderWorld,
            Visibility,
            InheritedVisibility,
            ViewVisibility,
            bevy::camera::primitives::Aabb,
        )>();
    }
    info!("[map] welded {} meshes into {} draws", welded.len(), groups);
}

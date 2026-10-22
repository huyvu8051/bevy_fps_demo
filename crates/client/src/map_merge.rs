use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, MeshVertexAttribute, PrimitiveTopology, VertexAttributeValues};
use bevy::prelude::*;
use std::collections::HashMap;

use crate::{map::MapRoot, *};

/// Whether the map's static geometry is welded together after it loads.
///
/// desert_city arrives as 4296 nodes and 2363 mesh primitives sharing 28
/// materials. Every one of those is an entity that must be transform
/// propagated, visibility checked, extracted into the render world, batched
/// and submitted, every frame, forever. Profiling put ~3.9ms a frame in the
/// render world sync alone, against 0.9ms for the whole of the gameplay.
#[derive(Resource, Default)]
pub(crate) struct MergeMapMeshes(pub(crate) bool);

#[derive(Component)]
pub(crate) struct MergedMapMesh;

/// Welding everything sharing a material into one mesh would be fewer entities
/// still, but a single mesh spanning the map can never be culled: look at a
/// wall and you draw the whole city behind it. Cells keep the count down and
/// the culling meaningful. Metres.
const MERGE_CELL_SIZE: f32 = 24.0;

/// Which attributes a run of vertices carries. Meshes are only welded to
/// others with the same set -- 2337 of the map's primitives have a colour
/// channel and 26 do not, and a merged mesh cannot have it both ways.
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
    /// One real handle per group. AssetId is the key, but a material cannot be
    /// rebuilt from an id any more, so the first part to land keeps its own.
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
/// Runs once, after world_ready -- which is already the signal that the scene
/// has loaded *and* that Avian has turned every ColliderConstructorHierarchy
/// into real colliders. That ordering is the whole safety of this: the
/// colliders are built from the original meshes and keep their own entities,
/// so only the drawing is moved. Take the meshes away any earlier and the map
/// loses its collision.
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
        // Normals need the inverse transpose, not the matrix: a non-uniform
        // scale tilts them the wrong way otherwise, and the map is full of
        // reused props stretched to fit.
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
        // RENDER_WORLD only: the colliders were built from the originals and
        // nothing asks this copy a question on the CPU again.
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

    // The originals keep their entity, their transform and their collider;
    // they just stop being drawn.
    //
    // SyncToRenderWorld has to go with them. It is a required component of
    // Mesh3d, and Bevy does not take required components away when the thing
    // that required them is removed -- so without this the entity stops being
    // drawn but carries on being copied into the render world every frame,
    // which is most of what it cost. Same for the visibility set: no mesh, but
    // check_visibility would still walk all 2363 of them.
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

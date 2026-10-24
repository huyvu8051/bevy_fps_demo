//! Loads the map's baked lightmap and attaches it to the map's meshes.
//!
//! `mapbake_editor` writes two files beside `scene.gltf`: `lightmap_uv.bin`,
//! which carries a second UV set, and `lightmap.png`, which carries ambient
//! occlusion in R and sun visibility in G. Neither can live in the glTF -- the
//! map is never rewritten -- so the pairing is done here, at extraction.
//!
//! Meshes are matched by geometry, not by name or by the order anything was
//! walked: a hash of every world-space vertex position and the index buffer.
//! Iteration order was the first attempt and Bevy makes no promise about it,
//! and a lightmap on the wrong mesh is not a subtle mistake.

use bevy::prelude::*;
use serde::Deserialize;
use std::collections::HashMap;

/// Mirrors `mapbake_editor::lightmap::LightmapMesh`. Two definitions of one
/// bincode layout: field order here has to match field order there.
#[derive(Deserialize)]
pub(crate) struct LightmapMesh {
    pub(crate) key: u64,
    pub(crate) xref: Vec<u32>,
    pub(crate) uv2: Vec<[f32; 2]>,
    pub(crate) indices: Vec<u32>,
}

#[derive(Deserialize)]
pub(crate) struct LightmapAtlasFile {
    pub(crate) size: u32,
    pub(crate) texels_per_unit: f32,
    pub(crate) meshes: Vec<LightmapMesh>,
}

#[derive(Resource, Default)]
pub(crate) struct Lightmap {
    /// Behind an Arc so the extractor can take a handle on it without copying
    /// seven megabytes, and without holding a borrow on the world while the
    /// mesh queries want one.
    pub(crate) by_key: std::sync::Arc<HashMap<u64, LightmapMesh>>,
    pub(crate) texture: Option<(Vec<u8>, u32)>,
}

/// The same hash `mapbake_editor` computes, over the same numbers.
///
/// A millimetre, not finer: both sides reach these positions by multiplying by
/// an affine and nothing promises they do it in the same order, and at a
/// kilometre from the origin an f32 only resolves 0.06mm.
pub(crate) fn instance_key(world_positions: &[Vec3], indices: &[u32]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut lo = Vec3::splat(f32::MAX);
    let mut hi = Vec3::splat(f32::MIN);
    for p in world_positions {
        lo = lo.min(*p);
        hi = hi.max(*p);
    }
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for value in [lo.x, lo.y, lo.z, hi.x, hi.y, hi.z] {
        ((value * 1000.0).round() as i64).hash(&mut hasher);
    }
    for p in world_positions {
        for value in [p.x, p.y, p.z] {
            ((value * 1000.0).round() as i64).hash(&mut hasher);
        }
    }
    world_positions.len().hash(&mut hasher);
    indices.hash(&mut hasher);
    hasher.finish()
}

pub(crate) fn load(map_dir: &std::path::Path) -> Lightmap {
    let uv_path = map_dir.join("lightmap_uv.bin");
    let png_path = map_dir.join("lightmap.png");

    let by_key = match std::fs::read(&uv_path) {
        Ok(bytes) => {
            match bincode::serde::decode_from_slice::<LightmapAtlasFile, _>(
                &bytes,
                bincode::config::standard(),
            ) {
                Ok((file, _)) => {
                    info!(
                        "[lightmap] {} meshes, {}x{} atlas at {} texels/m",
                        file.meshes.len(),
                        file.size,
                        file.size,
                        file.texels_per_unit
                    );
                    std::sync::Arc::new(file.meshes.into_iter().map(|m| (m.key, m)).collect())
                }
                Err(err) => {
                    warn!("[lightmap] could not decode {}: {err}", uv_path.display());
                    std::sync::Arc::new(HashMap::new())
                }
            }
        }
        Err(_) => std::sync::Arc::new(HashMap::new()),
    };

    // Read as RGBA even though only R and G carry anything: Metal has no
    // two-channel 8-bit format that samples as .rg without a swizzle, and the
    // atlas is small enough that the two wasted channels are not worth one.
    let texture = image::open(&png_path)
        .ok()
        .map(|image| {
            let rgba = image.to_rgba8();
            let size = rgba.width();
            (rgba.into_raw(), size)
        })
        .inspect(|(_, size)| info!("[lightmap] atlas texture {size}x{size}"));

    if by_key.is_empty() || texture.is_none() {
        warn!("[lightmap] no bake found beside the map -- falling back to flat ambient");
    }
    Lightmap { by_key, texture }
}

/// Rewrites a mesh onto its atlas vertices.
///
/// The bake splits charts, and splitting a chart duplicates the vertices along
/// the cut, so the atlas has its own vertex list and its own indices. `xref`
/// says where each atlas vertex came from; every other attribute is gathered
/// through it. For this map that is 1.02 vertices out for every one in -- the
/// glTF was already split by normals and the chart cuts mostly fall on seams
/// that were there.
pub(crate) fn apply(mesh: &mut metal_renderer::snapshot::ExtractedMesh, entry: &LightmapMesh) {
    let gather = |src: &Vec<[f32; 3]>| -> Vec<[f32; 3]> {
        entry.xref.iter().map(|i| src[*i as usize]).collect()
    };
    if mesh.positions.is_empty() {
        return;
    }
    let positions = gather(&mesh.positions);
    let normals = if mesh.normals.is_empty() { Vec::new() } else { gather(&mesh.normals) };
    let uvs: Vec<[f32; 2]> = if mesh.uvs.is_empty() {
        Vec::new()
    } else {
        entry.xref.iter().map(|i| mesh.uvs[*i as usize]).collect()
    };
    let tangents: Vec<[f32; 4]> = if mesh.tangents.is_empty() {
        Vec::new()
    } else {
        entry.xref.iter().map(|i| mesh.tangents[*i as usize]).collect()
    };

    mesh.positions = positions;
    mesh.normals = normals;
    mesh.uvs = uvs;
    mesh.tangents = tangents;
    mesh.indices = entry.indices.clone();
    mesh.uv2 = entry.uv2.clone();
}

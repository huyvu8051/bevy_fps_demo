//! Real glTF loading for the native Metal renderer (Phase 1 of the
//! wgpu-replacement plan) — replaces the tobj/pre-converted-.obj hack in
//! native_metal_probe.rs's original Sponza loader. Model I/O on iOS
//! doesn't support `.gltf` at all (confirmed on-device: `canImportFileExtension`
//! returns false), which is why that hack existed; the `gltf` crate is
//! pure Rust with no OS dependency, so it reads `.gltf`/`.glb` (including
//! the real models/Sponza/Sponza.gltf, and later soldier.glb/zombie.glb)
//! directly, no offline conversion step needed.
//!
//! Node transforms are baked into world-space vertex positions/normals at
//! load time (no per-draw model matrix yet) — same "static baked scene"
//! shape the box arena and OBJ-Sponza loaders already had. Per-node
//! instancing/animation is Phase 2+.

use metal::{Device, MTLPixelFormat, MTLRegion, MTLResourceOptions};

#[repr(C)]
#[derive(Copy, Clone)]
pub struct GltfVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
}

pub struct GltfMesh {
    pub vertex_buffer: metal::Buffer,
    pub index_buffer: metal::Buffer,
    pub index_count: usize,
    pub base_color_texture: Option<metal::Texture>,
    pub base_color_factor: [f32; 4],
}

type Mat4 = [[f32; 4]; 4];

fn mat4_identity() -> Mat4 {
    [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]]
}

fn mat4_mul(a: Mat4, b: Mat4) -> Mat4 {
    let mut out = mat4_identity();
    for col in 0..4 {
        for row in 0..4 {
            out[col][row] =
                a[0][row] * b[col][0] + a[1][row] * b[col][1] + a[2][row] * b[col][2] + a[3][row] * b[col][3];
        }
    }
    out
}

fn mat4_from_quat_translation_scale(t: [f32; 3], r: [f32; 4], s: [f32; 3]) -> Mat4 {
    let (x, y, z, w) = (r[0], r[1], r[2], r[3]);
    let (x2, y2, z2) = (x + x, y + y, z + z);
    let (xx, xy, xz) = (x * x2, x * y2, x * z2);
    let (yy, yz, zz) = (y * y2, y * z2, z * z2);
    let (wx, wy, wz) = (w * x2, w * y2, w * z2);
    [
        [(1.0 - (yy + zz)) * s[0], (xy + wz) * s[0], (xz - wy) * s[0], 0.0],
        [(xy - wz) * s[1], (1.0 - (xx + zz)) * s[1], (yz + wx) * s[1], 0.0],
        [(xz + wy) * s[2], (yz - wx) * s[2], (1.0 - (xx + yy)) * s[2], 0.0],
        [t[0], t[1], t[2], 1.0],
    ]
}

fn transform_point(m: Mat4, p: [f32; 3]) -> [f32; 3] {
    [
        m[0][0] * p[0] + m[1][0] * p[1] + m[2][0] * p[2] + m[3][0],
        m[0][1] * p[0] + m[1][1] * p[1] + m[2][1] * p[2] + m[3][1],
        m[0][2] * p[0] + m[1][2] * p[1] + m[2][2] * p[2] + m[3][2],
    ]
}

// Normals transform by the inverse-transpose in general, but since we only
// bake uniform-ish node scales in practice for this game's assets, using
// the upper-left 3x3 directly (then renormalizing) is a fine approximation
// — avoids a matrix inverse for a first pass.
fn transform_normal(m: Mat4, n: [f32; 3]) -> [f32; 3] {
    let v = [
        m[0][0] * n[0] + m[1][0] * n[1] + m[2][0] * n[2],
        m[0][1] * n[0] + m[1][1] * n[1] + m[2][1] * n[2],
        m[0][2] * n[0] + m[1][2] * n[1] + m[2][2] * n[2],
    ];
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len > 1e-6 { [v[0] / len, v[1] / len, v[2] / len] } else { [0.0, 1.0, 0.0] }
}

pub(crate) fn upload_texture(device: &Device, width: u32, height: u32, rgba: &[u8]) -> metal::Texture {
    let descriptor = metal::TextureDescriptor::new();
    descriptor.set_pixel_format(MTLPixelFormat::RGBA8Unorm);
    descriptor.set_width(width as u64);
    descriptor.set_height(height as u64);
    descriptor.set_storage_mode(metal::MTLStorageMode::Shared);
    descriptor.set_usage(metal::MTLTextureUsage::ShaderRead);
    let texture = device.new_texture(&descriptor);
    let region = MTLRegion::new_2d(0, 0, width as u64, height as u64);
    texture.replace_region(region, 0, rgba.as_ptr() as *const _, (width * 4) as u64);
    texture
}

// gltf::image::Data comes back in whatever channel layout the source image
// had (R8, RG8, RGB8, RGBA8, ...); Metal's texture upload here always wants
// RGBA8, so pad/expand as needed.
pub(crate) fn to_rgba8(image: &gltf::image::Data) -> Vec<u8> {
    use gltf::image::Format;
    let pixel_count = (image.width * image.height) as usize;
    let mut out = Vec::with_capacity(pixel_count * 4);
    match image.format {
        Format::R8 => {
            for &r in &image.pixels {
                out.extend_from_slice(&[r, r, r, 255]);
            }
        }
        Format::R8G8 => {
            for chunk in image.pixels.chunks_exact(2) {
                out.extend_from_slice(&[chunk[0], chunk[1], 0, 255]);
            }
        }
        Format::R8G8B8 => {
            for chunk in image.pixels.chunks_exact(3) {
                out.extend_from_slice(&[chunk[0], chunk[1], chunk[2], 255]);
            }
        }
        Format::R8G8B8A8 => {
            out.extend_from_slice(&image.pixels);
        }
        _ => {
            // 16-bit and float formats: not used by any asset this game
            // ships today (all glTF textures are 8-bit PNG/JPEG) — fall
            // back to a flat gray rather than crash on something exotic.
            out.resize(pixel_count * 4, 180);
        }
    }
    out
}

fn walk_node(
    node: &gltf::Node,
    parent_transform: Mat4,
    buffers: &[gltf::buffer::Data],
    textures: &[Option<metal::Texture>],
    device: &Device,
    meshes: &mut Vec<GltfMesh>,
) {
    let (t, r, s) = node.transform().decomposed();
    let local = mat4_from_quat_translation_scale(t, r, s);
    let world = mat4_mul(parent_transform, local);

    if let Some(mesh) = node.mesh() {
        for primitive in mesh.primitives() {
            let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));
            let Some(positions) = reader.read_positions() else { continue };
            let positions: Vec<[f32; 3]> = positions.collect();
            let normals: Vec<[f32; 3]> = reader
                .read_normals()
                .map(|n| n.collect())
                .unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; positions.len()]);
            let uvs: Vec<[f32; 2]> = reader
                .read_tex_coords(0)
                .map(|uv| uv.into_f32().collect())
                .unwrap_or_else(|| vec![[0.0, 0.0]; positions.len()]);
            let Some(indices) = reader.read_indices() else { continue };
            let indices: Vec<u32> = indices.into_u32().collect();
            if positions.is_empty() || indices.is_empty() {
                continue;
            }

            let vertices: Vec<GltfVertex> = (0..positions.len())
                .map(|i| GltfVertex {
                    position: transform_point(world, positions[i]),
                    normal: transform_normal(world, normals[i]),
                    uv: uvs[i],
                })
                .collect();

            let vertex_buffer = device.new_buffer_with_data(
                vertices.as_ptr() as *const _,
                (vertices.len() * std::mem::size_of::<GltfVertex>()) as u64,
                MTLResourceOptions::StorageModeShared,
            );
            let index_buffer = device.new_buffer_with_data(
                indices.as_ptr() as *const _,
                (indices.len() * std::mem::size_of::<u32>()) as u64,
                MTLResourceOptions::StorageModeShared,
            );

            let pbr = primitive.material().pbr_metallic_roughness();
            let base_color_texture = pbr
                .base_color_texture()
                .and_then(|info| textures.get(info.texture().source().index()).cloned().flatten());

            meshes.push(GltfMesh {
                vertex_buffer,
                index_buffer,
                index_count: indices.len(),
                base_color_texture,
                base_color_factor: pbr.base_color_factor(),
            });
        }
    }

    for child in node.children() {
        walk_node(&child, world, buffers, textures, device, meshes);
    }
}

pub fn load_gltf_scene(device: &Device, path: &std::path::Path) -> Vec<GltfMesh> {
    let (document, buffers, images) = match gltf::import(path) {
        Ok(result) => result,
        Err(err) => {
            eprintln!("[gltf_scene] failed to load {path:?}: {err}");
            return Vec::new();
        }
    };

    let textures: Vec<Option<metal::Texture>> = images
        .iter()
        .map(|image| Some(upload_texture(device, image.width, image.height, &to_rgba8(image))))
        .collect();

    let mut meshes = Vec::new();
    let Some(scene) = document.default_scene().or_else(|| document.scenes().next()) else {
        eprintln!("[gltf_scene] {path:?} has no scenes");
        return Vec::new();
    };
    for node in scene.nodes() {
        walk_node(&node, mat4_identity(), &buffers, &textures, device, &mut meshes);
    }
    eprintln!("[gltf_scene] loaded {} meshes ({} textures) from {path:?}", meshes.len(), textures.len());
    meshes
}

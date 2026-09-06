//! glTF-flavored helpers for the native Metal renderer: a vertex layout
//! (`GltfVertex`) shared by the real game's own per-frame mesh baking
//! (`bake_static_mesh_vertices` in native_metal_renderer.rs) and the skinned
//! zombie/soldier model loader (skinned_gltf.rs), plus `to_rgba8` for
//! normalizing whatever pixel format `gltf::image::Data` decoded to before
//! handing it to metal_texture.rs's `upload_texture`. Used to also own a
//! full node-tree glTF scene loader (`load_gltf_scene`/`walk_node`, Phase 1
//! of the wgpu-replacement plan) for this renderer's own standalone
//! Sponza-scene probe/demo, and `upload_texture` itself — removed/moved
//! once that demo path was confirmed unreachable in the shipped app
//! (Swift's ViewController.swift hardcodes USE_REAL_GAME_NATIVE_RENDERER =
//! true, USE_NATIVE_METAL_DEMO = false); see git history if the demo loader
//! is ever worth resurrecting for a standalone demo/probe build again.

#[repr(C)]
#[derive(Copy, Clone)]
pub struct GltfVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    // xyz = tangent direction, w = handedness sign (glTF's own convention)
    // — see native_metal_renderer.rs's SPONZA_SHADER_SOURCE for the TBN basis
    // this feeds. `[1,0,0,1]` (an arbitrary but valid unit tangent) when
    // the source mesh has no real ATTRIBUTE_TANGENT — never actually
    // sampled through in that case, since normal_texture is also None
    // whenever tangents are missing (see ExtractedMesh::normal_texture's
    // own doc comment).
    pub tangent: [f32; 4],
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

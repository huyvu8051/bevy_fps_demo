//! Rust-side control experiment, mirroring the Swift native-Metal demo
//! (see ViewController.swift's USE_NATIVE_METAL_DEMO): draws the same
//! 5-cuboid arena directly via the `metal` crate (classic
//! MTLCommandQueue/MTLRenderCommandEncoder, no bevy_render, no wgpu) to
//! check whether writing the renderer in Rust (instead of Swift) brings
//! wgpu's "high CPU encoding cost" overhead back, or whether it's specific
//! to wgpu's abstraction layer regardless of language. Entirely
//! self-contained — no Bevy App involved at all, since the question this
//! answers doesn't need ECS.

use core_graphics::geometry::{CGRect, CGSize};
use metal::foreign_types::ForeignType;
use metal::{
    CAMetalLayer, CommandQueue, CompileOptions, Device, MTLClearColor, MTLIndexType, MTLLoadAction, MTLPixelFormat,
    MTLPrimitiveType, MTLResourceOptions, MTLStoreAction, MetalLayer, RenderPassDescriptor, RenderPipelineDescriptor,
    RenderPipelineState,
};
use objc::{runtime::Object, *};

// Same models/Sponza/ the real game uses, converted once offline to .obj
// (see assets/models/SponzaObj — trimesh via Python, since Model I/O on
// iOS doesn't support .gltf; see ViewController.swift's own loadSponza
// comment for the full story). `tobj` here is the Rust-side equivalent of
// Swift's Model I/O OBJ import — no Bevy asset_server involved.
const SPONZA_OBJ_RELATIVE_PATH: &str = "assets/models/SponzaObj/Sponza.obj";

// Rust has no direct equivalent of Swift's `Bundle.main.resourcePath` —
// reach into Foundation via objc the same way the rest of this module
// already talks to UIKit/CAMetalLayer.
fn bundle_resource_path() -> Option<String> {
    unsafe {
        let bundle_class = class!(NSBundle);
        let bundle: *mut Object = msg_send![bundle_class, mainBundle];
        let path: *mut Object = msg_send![bundle, resourcePath];
        if path.is_null() {
            return None;
        }
        let utf8: *const std::os::raw::c_char = msg_send![path, UTF8String];
        if utf8.is_null() {
            return None;
        }
        Some(std::ffi::CStr::from_ptr(utf8).to_string_lossy().into_owned())
    }
}

const ARENA_HALF_SIZE: f32 = 10.0;
const ARENA_WALL_HEIGHT: f32 = 4.0;
const ARENA_WALL_THICKNESS: f32 = 0.3;

#[repr(C)]
#[derive(Copy, Clone)]
struct Vertex {
    position: [f32; 3],
    normal: [f32; 3],
    color: [f32; 3],
}

#[repr(C)]
#[derive(Copy, Clone)]
struct Uniforms {
    view_projection: [[f32; 4]; 4],
    light_dir: [f32; 3],
    _pad: f32,
}

const SHADER_SOURCE: &str = r#"
#include <metal_stdlib>
using namespace metal;

// packed_float3, not float3: MSL gives plain float3 struct members 16-byte
// alignment (12 bytes of data + 4 padding) even in the device address
// space, but Rust's #[repr(C)] Vertex below uses [f32; 3] fields, which
// are tightly packed at 12 bytes with no padding — reading a
// device-buffer array of that layout as plain float3 fields shifts every
// field after the first by 4 bytes per preceding vec3, which is exactly
// what produced the garbled rainbow/spike geometry the first time.
// No [[attribute(N)]] annotations here — those only apply to structs read
// via [[stage_in]]/a vertex descriptor. vertex_main below reads this
// straight out of a raw device buffer by index instead, so plain struct
// fields are all that's needed.
struct Vertex {
    packed_float3 position;
    packed_float3 normal;
    packed_float3 color;
};

struct Uniforms {
    float4x4 viewProjection;
    float3 lightDir;
};

struct VertexOut {
    float4 position [[position]];
    float3 normal;
    float3 color;
};

vertex VertexOut vertex_main(uint vid [[vertex_id]],
                              const device Vertex *verts [[buffer(0)]],
                              constant Uniforms &uniforms [[buffer(1)]]) {
    VertexOut out;
    Vertex v = verts[vid];
    out.position = uniforms.viewProjection * float4(float3(v.position), 1.0);
    out.normal = float3(v.normal);
    out.color = float3(v.color);
    return out;
}

fragment float4 fragment_main(VertexOut in [[stage_in]],
                               constant Uniforms &uniforms [[buffer(1)]]) {
    float diffuse = max(dot(normalize(in.normal), -normalize(uniforms.lightDir)), 0.0);
    float3 lit = in.color * (0.35 + 0.65 * diffuse);
    return float4(lit, 1.0);
}
"#;

fn append_box(verts: &mut Vec<Vertex>, center: [f32; 3], half: [f32; 3], color: [f32; 3]) {
    let (cx, cy, cz) = (center[0], center[1], center[2]);
    let (x, y, z) = (half[0], half[1], half[2]);
    let c = |dx: f32, dy: f32, dz: f32| [cx + dx * x, cy + dy * y, cz + dz * z];
    let c000 = c(-1.0, -1.0, -1.0);
    let c001 = c(-1.0, -1.0, 1.0);
    let c010 = c(-1.0, 1.0, -1.0);
    let c011 = c(-1.0, 1.0, 1.0);
    let c100 = c(1.0, -1.0, -1.0);
    let c101 = c(1.0, -1.0, 1.0);
    let c110 = c(1.0, 1.0, -1.0);
    let c111 = c(1.0, 1.0, 1.0);

    let mut face = |a: [f32; 3], b: [f32; 3], cc: [f32; 3], d: [f32; 3], n: [f32; 3]| {
        for p in [a, b, cc, a, cc, d] {
            verts.push(Vertex { position: p, normal: n, color });
        }
    };
    face(c001, c101, c111, c011, [0.0, 0.0, 1.0]);
    face(c100, c000, c010, c110, [0.0, 0.0, -1.0]);
    face(c101, c100, c110, c111, [1.0, 0.0, 0.0]);
    face(c000, c001, c011, c010, [-1.0, 0.0, 0.0]);
    face(c010, c011, c111, c110, [0.0, 1.0, 0.0]);
    face(c001, c000, c100, c101, [0.0, -1.0, 0.0]);
}

fn build_arena_vertices() -> Vec<Vertex> {
    let mut verts = Vec::new();
    let floor_color = [0.35, 0.35, 0.38];
    let wall_color = [0.55, 0.5, 0.45];
    let floor_size = ARENA_HALF_SIZE * 2.0;

    append_box(
        &mut verts,
        [0.0, -ARENA_WALL_THICKNESS / 2.0, 0.0],
        [floor_size / 2.0, ARENA_WALL_THICKNESS / 2.0, floor_size / 2.0],
        floor_color,
    );

    let wall_span = floor_size + ARENA_WALL_THICKNESS;
    let walls = [
        ([0.0, ARENA_WALL_HEIGHT / 2.0, -ARENA_HALF_SIZE], wall_span, ARENA_WALL_THICKNESS),
        ([0.0, ARENA_WALL_HEIGHT / 2.0, ARENA_HALF_SIZE], wall_span, ARENA_WALL_THICKNESS),
        ([-ARENA_HALF_SIZE, ARENA_WALL_HEIGHT / 2.0, 0.0], ARENA_WALL_THICKNESS, wall_span),
        ([ARENA_HALF_SIZE, ARENA_WALL_HEIGHT / 2.0, 0.0], ARENA_WALL_THICKNESS, wall_span),
    ];
    for (center, size_x, size_z) in walls {
        append_box(&mut verts, center, [size_x / 2.0, ARENA_WALL_HEIGHT / 2.0, size_z / 2.0], wall_color);
    }
    verts
}

fn perspective(fov_y_radians: f32, aspect: f32, near: f32, far: f32) -> [[f32; 4]; 4] {
    let y_scale = 1.0 / (fov_y_radians * 0.5).tan();
    let x_scale = y_scale / aspect;
    let z_range = far - near;
    let z_scale = -(far + near) / z_range;
    let wz_scale = -2.0 * far * near / z_range;
    [
        [x_scale, 0.0, 0.0, 0.0],
        [0.0, y_scale, 0.0, 0.0],
        [0.0, 0.0, z_scale, -1.0],
        [0.0, 0.0, wz_scale, 0.0],
    ]
}

fn look_at(eye: [f32; 3], center: [f32; 3], up: [f32; 3]) -> [[f32; 4]; 4] {
    let sub = |a: [f32; 3], b: [f32; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let cross = |a: [f32; 3], b: [f32; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let norm = |v: [f32; 3]| {
        let len = (dot(v, v)).sqrt();
        [v[0] / len, v[1] / len, v[2] / len]
    };
    let f = norm(sub(center, eye));
    let s = norm(cross(f, up));
    let u = cross(s, f);
    [
        [s[0], u[0], -f[0], 0.0],
        [s[1], u[1], -f[1], 0.0],
        [s[2], u[2], -f[2], 0.0],
        [-dot(s, eye), -dot(u, eye), dot(f, eye), 1.0],
    ]
}

fn mat_mul(a: [[f32; 4]; 4], b: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut out = [[0.0f32; 4]; 4];
    for col in 0..4 {
        for row in 0..4 {
            out[col][row] =
                a[0][row] * b[col][0] + a[1][row] * b[col][1] + a[2][row] * b[col][2] + a[3][row] * b[col][3];
        }
    }
    out
}

#[repr(C)]
#[derive(Copy, Clone)]
struct SponzaVertex {
    position: [f32; 3],
    normal: [f32; 3],
}

const SPONZA_SHADER_SOURCE: &str = r#"
#include <metal_stdlib>
using namespace metal;

struct Vertex {
    packed_float3 position;
    packed_float3 normal;
};

struct Uniforms {
    float4x4 viewProjection;
    float3 lightDir;
    float3 tintColor;
};

struct VertexOut {
    float4 position [[position]];
    float3 normal;
};

vertex VertexOut sponza_vertex(uint vid [[vertex_id]],
                                const device Vertex *verts [[buffer(0)]],
                                constant Uniforms &uniforms [[buffer(1)]]) {
    VertexOut out;
    Vertex v = verts[vid];
    out.position = uniforms.viewProjection * float4(float3(v.position), 1.0);
    out.normal = float3(v.normal);
    return out;
}

fragment float4 sponza_fragment(VertexOut in [[stage_in]],
                                 constant Uniforms &uniforms [[buffer(1)]]) {
    float diffuse = max(dot(normalize(in.normal), -normalize(uniforms.lightDir)), 0.0);
    float3 lit = uniforms.tintColor * (0.35 + 0.65 * diffuse);
    return float4(lit, 1.0);
}
"#;

#[repr(C)]
#[derive(Copy, Clone)]
struct SponzaUniforms {
    view_projection: [[f32; 4]; 4],
    light_dir: [f32; 3],
    _pad: f32,
    tint_color: [f32; 3],
    _pad2: f32,
}

struct SponzaMesh {
    vertex_buffer: metal::Buffer,
    index_buffer: metal::Buffer,
    index_count: usize,
}

// Parses assets/models/SponzaObj/Sponza.obj (see that dir's own history —
// converted offline from the real game's Sponza.gltf since Model I/O/tobj
// don't handle glTF here) via `tobj`, the Rust-side equivalent of what
// ViewController.swift's loadSponza() does with Model I/O. One MTLBuffer
// pair (vertex + u32 index) per OBJ "model" (usemtl group), same shape as
// the Swift version's per-submesh draw calls.
fn load_sponza(device: &Device) -> Vec<SponzaMesh> {
    let Some(resource_path) = bundle_resource_path() else {
        eprintln!("[native_metal_probe] no bundle resource path");
        return Vec::new();
    };
    let obj_path = std::path::Path::new(&resource_path).join(SPONZA_OBJ_RELATIVE_PATH);
    if !obj_path.exists() {
        eprintln!("[native_metal_probe] Sponza.obj not found at {obj_path:?}");
        return Vec::new();
    }

    let load_options = tobj::LoadOptions {
        triangulate: true,
        single_index: true,
        ..Default::default()
    };
    let (models, _materials) = match tobj::load_obj(&obj_path, &load_options) {
        Ok(result) => result,
        Err(err) => {
            eprintln!("[native_metal_probe] tobj load failed: {err}");
            return Vec::new();
        }
    };

    let mut meshes = Vec::new();
    for model in models {
        let mesh = model.mesh;
        if mesh.positions.is_empty() || mesh.indices.is_empty() {
            continue;
        }
        // This particular .obj (see assets/models/SponzaObj's own history —
        // trimesh's gltf->obj export) has no `vn` lines at all, so
        // mesh.normals is always empty here — every vertex would otherwise
        // get the exact same made-up normal, making every surface receive
        // identical lighting regardless of orientation (a solid mid-gray
        // blob with zero shading cues, no visible edges — exactly what
        // showed up on-device before this fix). Compute real per-vertex
        // normals by accumulating each triangle's face normal (cross
        // product of two edges) onto its 3 vertices, then normalizing —
        // the same thing MDLMesh.addNormals() did for us for free on the
        // Swift/Model I/O side.
        let has_normals = mesh.normals.len() == mesh.positions.len();
        let vertex_count = mesh.positions.len() / 3;
        let mut normals = vec![[0.0f32; 3]; vertex_count];
        if !has_normals {
            let pos_at = |i: u32| -> [f32; 3] {
                let i = i as usize;
                [mesh.positions[i * 3], mesh.positions[i * 3 + 1], mesh.positions[i * 3 + 2]]
            };
            let sub = |a: [f32; 3], b: [f32; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
            let cross =
                |a: [f32; 3], b: [f32; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
            for tri in mesh.indices.chunks_exact(3) {
                let (ia, ib, ic) = (tri[0], tri[1], tri[2]);
                let (pa, pb, pc) = (pos_at(ia), pos_at(ib), pos_at(ic));
                let face_normal = cross(sub(pb, pa), sub(pc, pa));
                for i in [ia, ib, ic] {
                    let n = &mut normals[i as usize];
                    n[0] += face_normal[0];
                    n[1] += face_normal[1];
                    n[2] += face_normal[2];
                }
            }
            for n in &mut normals {
                let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
                if len > 1e-6 {
                    n[0] /= len;
                    n[1] /= len;
                    n[2] /= len;
                } else {
                    *n = [0.0, 1.0, 0.0];
                }
            }
        }
        let mut vertices = Vec::with_capacity(vertex_count);
        for i in 0..vertex_count {
            let position = [mesh.positions[i * 3], mesh.positions[i * 3 + 1], mesh.positions[i * 3 + 2]];
            let normal = if has_normals { [mesh.normals[i * 3], mesh.normals[i * 3 + 1], mesh.normals[i * 3 + 2]] } else { normals[i] };
            vertices.push(SponzaVertex { position, normal });
        }
        let vertex_buffer = device.new_buffer_with_data(
            vertices.as_ptr() as *const _,
            (vertices.len() * std::mem::size_of::<SponzaVertex>()) as u64,
            MTLResourceOptions::StorageModeShared,
        );
        let index_buffer = device.new_buffer_with_data(
            mesh.indices.as_ptr() as *const _,
            (mesh.indices.len() * std::mem::size_of::<u32>()) as u64,
            MTLResourceOptions::StorageModeShared,
        );
        meshes.push(SponzaMesh { vertex_buffer, index_buffer, index_count: mesh.indices.len() });
    }
    eprintln!("[native_metal_probe] loaded {} Sponza meshes", meshes.len());
    meshes
}

pub struct NativeMetalProbe {
    device: Device,
    layer: MetalLayer,
    queue: CommandQueue,
    pipeline: RenderPipelineState,
    vertex_buffer: metal::Buffer,
    vertex_count: usize,
    depth_state: metal::DepthStencilState,
    depth_texture: Option<metal::Texture>,
    sponza_pipeline: Option<RenderPipelineState>,
    sponza_meshes: Vec<SponzaMesh>,
}

impl NativeMetalProbe {
    // `view` is the raw UIView* handed over from Swift — same FFI contract
    // as the existing cadisplaylink module (see ios_view.rs). We reach
    // into it via objc msg_send! to grab its CAMetalLayer, exactly what
    // MetalView.swift's `layerClass` override already made it.
    pub unsafe fn new(view: *mut Object) -> Self {
        let device = Device::system_default().expect("no Metal device");
        let raw_layer: *mut Object = msg_send![view, layer];
        // MetalLayer wraps a CAMetalLayer* Objective-C object the same way
        // MetalView.swift's `layerClass` override already made metalV's
        // backing layer — from_ptr takes ownership of a +1 retain count,
        // so bump the refcount here since `layer` (owned by the UIView)
        // shouldn't be released out from under Swift.
        let retained: *mut Object = msg_send![raw_layer, retain];
        let layer = MetalLayer::from_ptr(retained as *mut CAMetalLayer);
        layer.set_device(&device);
        layer.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
        // CAMetalLayer's drawableSize does NOT auto-track the view's
        // bounds when the layer is driven manually like this (unlike
        // MTKView, which keeps it in sync every layout pass) — has to be
        // set explicitly or nextDrawable()'s texture ends up 0x0, which
        // crashes the depth-texture descriptor validation. Set once here
        // from the view's current bounds * scale_factor; fine for this
        // diagnostic probe (no rotation handling needed).
        let bounds: CGRect = msg_send![view, bounds];
        let scale = layer.contents_scale();
        layer.set_drawable_size(CGSize::new(bounds.size.width * scale, bounds.size.height * scale));

        let queue = device.new_command_queue();

        let vertices = build_arena_vertices();
        let vertex_buffer = device.new_buffer_with_data(
            vertices.as_ptr() as *const _,
            (vertices.len() * std::mem::size_of::<Vertex>()) as u64,
            MTLResourceOptions::StorageModeShared,
        );

        let compile_options = CompileOptions::new();
        let library = device.new_library_with_source(SHADER_SOURCE, &compile_options).expect("shader compile failed");
        let vertex_fn = library.get_function("vertex_main", None).unwrap();
        let fragment_fn = library.get_function("fragment_main", None).unwrap();

        let pipeline_descriptor = RenderPipelineDescriptor::new();
        pipeline_descriptor.set_vertex_function(Some(&vertex_fn));
        pipeline_descriptor.set_fragment_function(Some(&fragment_fn));
        pipeline_descriptor.color_attachments().object_at(0).unwrap().set_pixel_format(MTLPixelFormat::BGRA8Unorm);
        pipeline_descriptor.set_depth_attachment_pixel_format(MTLPixelFormat::Depth32Float);
        let pipeline = device.new_render_pipeline_state(&pipeline_descriptor).expect("pipeline creation failed");

        let depth_descriptor = metal::DepthStencilDescriptor::new();
        depth_descriptor.set_depth_compare_function(metal::MTLCompareFunction::Less);
        depth_descriptor.set_depth_write_enabled(true);
        let depth_state = device.new_depth_stencil_state(&depth_descriptor);

        let sponza_pipeline = match device.new_library_with_source(SPONZA_SHADER_SOURCE, &compile_options) {
            Ok(sponza_library) => {
                let sponza_vertex_fn = sponza_library.get_function("sponza_vertex", None).unwrap();
                let sponza_fragment_fn = sponza_library.get_function("sponza_fragment", None).unwrap();
                let sponza_pipeline_descriptor = RenderPipelineDescriptor::new();
                sponza_pipeline_descriptor.set_vertex_function(Some(&sponza_vertex_fn));
                sponza_pipeline_descriptor.set_fragment_function(Some(&sponza_fragment_fn));
                sponza_pipeline_descriptor
                    .color_attachments()
                    .object_at(0)
                    .unwrap()
                    .set_pixel_format(MTLPixelFormat::BGRA8Unorm);
                sponza_pipeline_descriptor.set_depth_attachment_pixel_format(MTLPixelFormat::Depth32Float);
                device.new_render_pipeline_state(&sponza_pipeline_descriptor).ok()
            }
            Err(err) => {
                eprintln!("[native_metal_probe] sponza shader compile failed: {err}");
                None
            }
        };
        let sponza_meshes = load_sponza(&device);

        Self {
            device,
            layer,
            queue,
            pipeline,
            sponza_pipeline,
            sponza_meshes,
            vertex_buffer,
            vertex_count: vertices.len(),
            depth_state,
            depth_texture: None,
        }
    }

    fn ensure_depth_texture(&mut self, width: u64, height: u64) -> metal::Texture {
        let device = self.device.clone();
        let device = &device;
        if let Some(tex) = &self.depth_texture {
            if tex.width() == width && tex.height() == height {
                return tex.clone();
            }
        }
        let descriptor = metal::TextureDescriptor::new();
        descriptor.set_pixel_format(MTLPixelFormat::Depth32Float);
        descriptor.set_width(width);
        descriptor.set_height(height);
        descriptor.set_storage_mode(metal::MTLStorageMode::Private);
        descriptor.set_usage(metal::MTLTextureUsage::RenderTarget);
        let tex = device.new_texture(&descriptor);
        self.depth_texture = Some(tex.clone());
        tex
    }

    // Same per-frame work as the Swift demo: one render pass, one draw
    // call per mesh, camera fixed (no touch/camera movement in this Rust
    // version — it exists purely to measure encoding cost, not to be a
    // playable probe). Draws Sponza if it loaded, otherwise falls back to
    // the box arena — same "don't show a blank screen while diagnosing"
    // reasoning as ViewController.swift's own fallback.
    pub fn render_frame(&mut self) {
        let drawable_size = self.layer.drawable_size();
        let (width, height) = (drawable_size.width as u64, drawable_size.height as u64);
        let depth_texture = self.ensure_depth_texture(width, height);
        let Some(drawable) = self.layer.next_drawable() else { return };
        let texture = drawable.texture();

        let use_sponza = self.sponza_pipeline.is_some() && !self.sponza_meshes.is_empty();
        let aspect = width as f32 / height.max(1) as f32;
        let (eye, center, far) =
            if use_sponza { ([0.0, 3.0, 4.0], [0.0, 3.0, 0.0], 500.0) } else { ([0.0, 3.0, 10.0], [0.0, 0.0, 0.0], 100.0) };
        let proj = perspective(if use_sponza { 60.0_f32 } else { 45.0 }.to_radians(), aspect, 0.05, far);
        let view = look_at(eye, center, [0.0, 1.0, 0.0]);
        let light_dir = {
            let v = [-4.0f32, -8.0, -4.0];
            let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            [v[0] / len, v[1] / len, v[2] / len]
        };
        let view_projection = mat_mul(proj, view);

        let pass_descriptor = RenderPassDescriptor::new();
        let color_attachment = pass_descriptor.color_attachments().object_at(0).unwrap();
        color_attachment.set_texture(Some(texture));
        color_attachment.set_load_action(MTLLoadAction::Clear);
        color_attachment.set_store_action(MTLStoreAction::Store);
        color_attachment.set_clear_color(MTLClearColor::new(1.0, 0.0, 1.0, 1.0));

        let depth_attachment = pass_descriptor.depth_attachment().unwrap();
        depth_attachment.set_texture(Some(&depth_texture));
        depth_attachment.set_load_action(MTLLoadAction::Clear);
        depth_attachment.set_store_action(MTLStoreAction::DontCare);
        depth_attachment.set_clear_depth(1.0);

        let command_buffer = self.queue.new_command_buffer();
        let encoder = command_buffer.new_render_command_encoder(pass_descriptor);
        encoder.set_depth_stencil_state(&self.depth_state);

        if use_sponza {
            let sponza_uniforms = SponzaUniforms {
                view_projection,
                light_dir,
                _pad: 0.0,
                tint_color: [0.6, 0.58, 0.55],
                _pad2: 0.0,
            };
            encoder.set_render_pipeline_state(self.sponza_pipeline.as_ref().unwrap());
            encoder.set_vertex_bytes(
                1,
                std::mem::size_of::<SponzaUniforms>() as u64,
                &sponza_uniforms as *const _ as *const _,
            );
            encoder.set_fragment_bytes(
                1,
                std::mem::size_of::<SponzaUniforms>() as u64,
                &sponza_uniforms as *const _ as *const _,
            );
            for mesh in &self.sponza_meshes {
                encoder.set_vertex_buffer(0, Some(&mesh.vertex_buffer), 0);
                encoder.draw_indexed_primitives(
                    MTLPrimitiveType::Triangle,
                    mesh.index_count as u64,
                    MTLIndexType::UInt32,
                    &mesh.index_buffer,
                    0,
                );
            }
        } else {
            let uniforms = Uniforms { view_projection, light_dir, _pad: 0.0 };
            encoder.set_render_pipeline_state(&self.pipeline);
            encoder.set_vertex_buffer(0, Some(&self.vertex_buffer), 0);
            encoder.set_vertex_bytes(1, std::mem::size_of::<Uniforms>() as u64, &uniforms as *const _ as *const _);
            encoder.set_fragment_bytes(1, std::mem::size_of::<Uniforms>() as u64, &uniforms as *const _ as *const _);
            encoder.draw_primitives(MTLPrimitiveType::Triangle, 0, self.vertex_count as u64);
        }

        encoder.end_encoding();
        command_buffer.present_drawable(drawable);
        command_buffer.commit();
    }
}

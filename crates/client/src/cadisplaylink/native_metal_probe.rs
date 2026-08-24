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
    CAMetalLayer, CommandQueue, CompileOptions, Device, MTLClearColor, MTLLoadAction, MTLPixelFormat, MTLPrimitiveType,
    MTLResourceOptions, MTLStoreAction, MetalLayer, RenderPassDescriptor, RenderPipelineDescriptor, RenderPipelineState,
};
use objc::{runtime::Object, *};

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

pub struct NativeMetalProbe {
    device: Device,
    layer: MetalLayer,
    queue: CommandQueue,
    pipeline: RenderPipelineState,
    vertex_buffer: metal::Buffer,
    vertex_count: usize,
    depth_state: metal::DepthStencilState,
    depth_texture: Option<metal::Texture>,
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

        Self {
            device,
            layer,
            queue,
            pipeline,
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
    // call, camera fixed looking at the arena (no touch/camera movement in
    // this Rust version — it exists purely to measure encoding cost, not
    // to be a playable probe).
    pub fn render_frame(&mut self) {
        let drawable_size = self.layer.drawable_size();
        let (width, height) = (drawable_size.width as u64, drawable_size.height as u64);
        let depth_texture = self.ensure_depth_texture(width, height);
        let Some(drawable) = self.layer.next_drawable() else { return };
        let texture = drawable.texture();

        let aspect = width as f32 / height.max(1) as f32;
        let proj = perspective(45.0_f32.to_radians(), aspect, 0.1, 100.0);
        let view = look_at([0.0, 3.0, 10.0], [0.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        let uniforms = Uniforms {
            view_projection: mat_mul(proj, view),
            light_dir: {
                let v = [-4.0f32, -8.0, -4.0];
                let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
                [v[0] / len, v[1] / len, v[2] / len]
            },
            _pad: 0.0,
        };

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
        encoder.set_render_pipeline_state(&self.pipeline);
        encoder.set_depth_stencil_state(&self.depth_state);
        encoder.set_vertex_buffer(0, Some(&self.vertex_buffer), 0);
        encoder.set_vertex_bytes(1, std::mem::size_of::<Uniforms>() as u64, &uniforms as *const _ as *const _);
        encoder.set_fragment_bytes(1, std::mem::size_of::<Uniforms>() as u64, &uniforms as *const _ as *const _);
        encoder.draw_primitives(MTLPrimitiveType::Triangle, 0, self.vertex_count as u64);
        encoder.end_encoding();
        command_buffer.present_drawable(drawable);
        command_buffer.commit();
    }
}

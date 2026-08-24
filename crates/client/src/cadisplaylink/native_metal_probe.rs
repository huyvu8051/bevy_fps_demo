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

// The real game's own Sponza asset — loaded directly via the `gltf` crate
// (see gltf_scene.rs), no offline conversion needed. (Model I/O on iOS
// doesn't support .gltf at all — confirmed on-device via
// canImportFileExtension — which is why the Swift demo needed a
// pre-converted .obj; the pure-Rust `gltf` crate has no such OS
// dependency.)
const SPONZA_GLTF_RELATIVE_PATH: &str = "assets/models/Sponza/Sponza.gltf";

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

const SPONZA_SHADER_SOURCE: &str = r#"
#include <metal_stdlib>
using namespace metal;

struct Vertex {
    packed_float3 position;
    packed_float3 normal;
    packed_float2 uv;
};

struct Uniforms {
    float4x4 viewProjection;
    float3 lightDir;
    float4 baseColorFactor;
};

struct VertexOut {
    float4 position [[position]];
    float3 normal;
    float2 uv;
};

vertex VertexOut sponza_vertex(uint vid [[vertex_id]],
                                const device Vertex *verts [[buffer(0)]],
                                constant Uniforms &uniforms [[buffer(1)]]) {
    VertexOut out;
    Vertex v = verts[vid];
    out.position = uniforms.viewProjection * float4(float3(v.position), 1.0);
    out.normal = float3(v.normal);
    out.uv = float2(v.uv);
    return out;
}

fragment float4 sponza_fragment(VertexOut in [[stage_in]],
                                 constant Uniforms &uniforms [[buffer(1)]],
                                 texture2d<float> baseColorTexture [[texture(0)]],
                                 sampler texSampler [[sampler(0)]]) {
    float diffuse = max(dot(normalize(in.normal), -normalize(uniforms.lightDir)), 0.0);
    float4 texColor = baseColorTexture.sample(texSampler, in.uv);
    float4 baseColor = texColor * uniforms.baseColorFactor;
    float3 lit = baseColor.rgb * (0.35 + 0.65 * diffuse);
    return float4(lit, baseColor.a);
}
"#;

#[repr(C)]
#[derive(Copy, Clone)]
struct SponzaUniforms {
    view_projection: [[f32; 4]; 4],
    light_dir: [f32; 3],
    _pad: f32,
    base_color_factor: [f32; 4],
}

// Phase 2 of the wgpu-replacement plan: GPU skinning for
// models/soldier.glb (and later zombie.glb) — up to 4 joint influences
// per vertex, standard linear-blend skinning. jointMatrices is computed
// CPU-side once per frame (see skinned_gltf.rs's compute_joint_matrices,
// same keyframe-sampling + hierarchy-walk math bevy_animation does
// internally) and uploaded as a plain buffer the vertex shader indexes by
// joint id.
const SKINNED_SHADER_SOURCE: &str = r#"
#include <metal_stdlib>
using namespace metal;

struct Vertex {
    packed_float3 position;
    packed_float3 normal;
    packed_float2 uv;
    ushort4 joints;
    packed_float4 weights;
};

struct Uniforms {
    float4x4 viewProjection;
    float4x4 modelMatrix;
    float3 lightDir;
    float _pad;
    float4 baseColorFactor;
};

struct VertexOut {
    float4 position [[position]];
    float3 normal;
    float2 uv;
};

vertex VertexOut skinned_vertex(uint vid [[vertex_id]],
                                 const device Vertex *verts [[buffer(0)]],
                                 constant Uniforms &uniforms [[buffer(1)]],
                                 const device float4x4 *jointMatrices [[buffer(2)]]) {
    Vertex v = verts[vid];
    float4x4 skin =
        v.weights.x * jointMatrices[v.joints.x] +
        v.weights.y * jointMatrices[v.joints.y] +
        v.weights.z * jointMatrices[v.joints.z] +
        v.weights.w * jointMatrices[v.joints.w];

    float4 skinnedPos = skin * float4(float3(v.position), 1.0);
    float3 skinnedNormal = normalize((skin * float4(float3(v.normal), 0.0)).xyz);

    VertexOut out;
    out.position = uniforms.viewProjection * uniforms.modelMatrix * skinnedPos;
    out.normal = (uniforms.modelMatrix * float4(skinnedNormal, 0.0)).xyz;
    out.uv = float2(v.uv);
    return out;
}

fragment float4 skinned_fragment(VertexOut in [[stage_in]],
                                  constant Uniforms &uniforms [[buffer(1)]],
                                  texture2d<float> baseColorTexture [[texture(0)]],
                                  sampler texSampler [[sampler(0)]]) {
    float diffuse = max(dot(normalize(in.normal), -normalize(uniforms.lightDir)), 0.0);
    float4 texColor = baseColorTexture.sample(texSampler, in.uv);
    float4 baseColor = texColor * uniforms.baseColorFactor;
    // soldier.glb's materials have no base color texture at all, and some
    // of their baseColorFactor values are extremely dark (e.g. ~0.03) —
    // real gear colors (black straps/vest), but combined with this
    // shader's 0.35 ambient floor that multiplies straight through, they
    // rendered as an indistinguishable solid black silhouette on-device.
    // Floor the color itself (not just the lighting term) so dark
    // materials stay visible/distinguishable; brighter ones (e.g. skin
    // tone) are unaffected.
    float3 flooredColor = max(baseColor.rgb, float3(0.12));
    float3 lit = flooredColor * (0.35 + 0.65 * diffuse);
    return float4(lit, baseColor.a);
}
"#;

// Phase 3's other half: a second camera (narrow FOV, same
// position/orientation as the main one — simulates a rifle scope) rendered
// into an offscreen texture, then composited as a small picture-in-picture
// quad over the main view. Same offscreen-render-target-then-present-on-a-
// quad indirection the real game's own scope camera already uses (see
// setup()'s own comment there), reimplemented against this renderer
// instead of bevy_render.
const SCOPE_TEXTURE_SIZE: u64 = 512;

const QUAD_SHADER_SOURCE: &str = r#"
#include <metal_stdlib>
using namespace metal;

struct QuadVertex {
    packed_float2 position;
    packed_float2 uv;
};

struct QuadVertexOut {
    float4 position [[position]];
    float2 uv;
};

vertex QuadVertexOut quad_vertex(uint vid [[vertex_id]],
                                  const device QuadVertex *verts [[buffer(0)]]) {
    QuadVertexOut out;
    QuadVertex v = verts[vid];
    out.position = float4(v.position, 0.0, 1.0);
    out.uv = v.uv;
    return out;
}

fragment float4 quad_fragment(QuadVertexOut in [[stage_in]],
                               texture2d<float> tex [[texture(0)]],
                               sampler texSampler [[sampler(0)]]) {
    return tex.sample(texSampler, in.uv);
}
"#;

#[repr(C)]
#[derive(Copy, Clone)]
struct QuadVertex {
    position: [f32; 2],
    uv: [f32; 2],
}

#[repr(C)]
#[derive(Copy, Clone)]
struct SkinnedUniforms {
    view_projection: [[f32; 4]; 4],
    model_matrix: [[f32; 4]; 4],
    light_dir: [f32; 3],
    _pad: f32,
    base_color_factor: [f32; 4],
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
    sponza_meshes: Vec<super::gltf_scene::GltfMesh>,
    sponza_sampler: metal::SamplerState,
    white_texture: metal::Texture,
    camera_position: [f32; 3],
    camera_yaw: f32,
    camera_pitch: f32,
    skinned_pipeline: Option<RenderPipelineState>,
    soldier_model: Option<super::skinned_gltf::SkinnedModel>,
    zombie_model: Option<super::skinned_gltf::SkinnedModel>,
    scope_color_texture: metal::Texture,
    scope_depth_texture: metal::Texture,
    quad_pipeline: Option<RenderPipelineState>,
    quad_vertex_buffer: metal::Buffer,
    // Phase 3: entities now genuinely live in a Bevy ECS World (see
    // ecs_app below) instead of a hardcoded Vec — patrol movement runs as
    // a real Bevy system over real Transform components. Real
    // spawn/despawn and movement driven by actual networking is a later
    // step (this ECS App has no RepliconPlugins/game logic yet, just
    // MinimalPlugins + our own patrol system) — this proves the "extract
    // a draw list from ECS each frame" seam works end-to-end, which is
    // the actual architectural risk the plan calls out.
    ecs_app: bevy::app::App,
    start_time: std::time::Instant,
}

#[derive(bevy::ecs::component::Component, Clone, Copy)]
enum EntityKind {
    Soldier,
    Zombie,
}

#[derive(bevy::ecs::component::Component)]
struct Patrol {
    center: [f32; 2],
    radius: f32,
    speed: f32,
    phase: f32,
}

#[derive(bevy::ecs::component::Component)]
struct AnimOffset(f32);

// Real Bevy ECS system — walks every entity with a Patrol component and
// moves its Transform along a circle each tick, driven by Bevy's own Time
// resource (from MinimalPlugins' TimePlugin). This is the seam the plan
// calls "extract": render_frame() below queries the same World for
// (Transform, EntityKind, AnimOffset) after ticking this system, instead
// of reading a hand-maintained Vec.
fn patrol_system(time: bevy::ecs::system::Res<bevy::time::Time>, mut query: bevy::ecs::system::Query<(&mut bevy::transform::components::Transform, &Patrol)>) {
    let now = time.elapsed_secs();
    for (mut transform, patrol) in &mut query {
        let angle = patrol.phase + now * patrol.speed;
        let x = patrol.center[0] + angle.cos() * patrol.radius;
        let z = patrol.center[1] + angle.sin() * patrol.radius;
        let facing = angle + std::f32::consts::FRAC_PI_2;
        transform.translation = bevy::math::Vec3::new(x, 0.0, z);
        transform.rotation = bevy::math::Quat::from_rotation_y(facing);
    }
}

fn build_entity_ecs_app() -> bevy::app::App {
    let mut app = bevy::app::App::new();
    app.add_plugins(bevy::MinimalPlugins);
    app.add_systems(bevy::app::Update, patrol_system);
    app.world_mut().spawn((
        bevy::transform::components::Transform::IDENTITY,
        EntityKind::Zombie,
        Patrol { center: [-4.0, -3.0], radius: 2.5, speed: 0.4, phase: 0.0 },
        AnimOffset(0.0),
    ));
    app.world_mut().spawn((
        bevy::transform::components::Transform::IDENTITY,
        EntityKind::Zombie,
        Patrol { center: [3.0, -4.0], radius: 3.0, speed: 0.3, phase: 1.7 },
        AnimOffset(0.5),
    ));
    app.world_mut().spawn((
        bevy::transform::components::Transform::IDENTITY,
        EntityKind::Zombie,
        Patrol { center: [-2.0, -6.0], radius: 2.0, speed: 0.5, phase: 3.1 },
        AnimOffset(1.1),
    ));
    app.world_mut().spawn((
        bevy::transform::components::Transform::IDENTITY,
        EntityKind::Zombie,
        Patrol { center: [5.0, -2.0], radius: 2.2, speed: 0.35, phase: 4.6 },
        AnimOffset(1.8),
    ));
    app.world_mut().spawn((
        bevy::transform::components::Transform::IDENTITY,
        EntityKind::Soldier,
        Patrol { center: [0.0, -3.0], radius: 1.5, speed: 0.2, phase: 0.0 },
        AnimOffset(0.0),
    ));
    app
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
        let gltf_path = std::path::Path::new(&bundle_resource_path().unwrap_or_default()).join(SPONZA_GLTF_RELATIVE_PATH);
        let sponza_meshes = super::gltf_scene::load_gltf_scene(&device, &gltf_path);

        let sampler_descriptor = metal::SamplerDescriptor::new();
        sampler_descriptor.set_min_filter(metal::MTLSamplerMinMagFilter::Linear);
        sampler_descriptor.set_mag_filter(metal::MTLSamplerMinMagFilter::Linear);
        sampler_descriptor.set_address_mode_s(metal::MTLSamplerAddressMode::Repeat);
        sampler_descriptor.set_address_mode_t(metal::MTLSamplerAddressMode::Repeat);
        let sponza_sampler = device.new_sampler(&sampler_descriptor);

        // Fallback for meshes/materials with no base color texture (glTF
        // materials aren't required to have one, just a factor) — sample a
        // 1x1 white texture so baseColorFactor alone still comes through
        // correctly instead of needing a separate untextured shader path.
        let white_texture_descriptor = metal::TextureDescriptor::new();
        white_texture_descriptor.set_pixel_format(MTLPixelFormat::RGBA8Unorm);
        white_texture_descriptor.set_width(1);
        white_texture_descriptor.set_height(1);
        white_texture_descriptor.set_storage_mode(metal::MTLStorageMode::Shared);
        let white_texture = device.new_texture(&white_texture_descriptor);
        white_texture.replace_region(metal::MTLRegion::new_2d(0, 0, 1, 1), 0, [255u8, 255, 255, 255].as_ptr() as *const _, 4);

        let skinned_pipeline = match device.new_library_with_source(SKINNED_SHADER_SOURCE, &compile_options) {
            Ok(library) => {
                let vertex_fn = library.get_function("skinned_vertex", None).unwrap();
                let fragment_fn = library.get_function("skinned_fragment", None).unwrap();
                let descriptor = RenderPipelineDescriptor::new();
                descriptor.set_vertex_function(Some(&vertex_fn));
                descriptor.set_fragment_function(Some(&fragment_fn));
                descriptor.color_attachments().object_at(0).unwrap().set_pixel_format(MTLPixelFormat::BGRA8Unorm);
                descriptor.set_depth_attachment_pixel_format(MTLPixelFormat::Depth32Float);
                device.new_render_pipeline_state(&descriptor).ok()
            }
            Err(err) => {
                eprintln!("[native_metal_probe] skinned shader compile failed: {err}");
                None
            }
        };
        let resource_path = bundle_resource_path().unwrap_or_default();
        let soldier_path = std::path::Path::new(&resource_path).join("assets/models/soldier.glb");
        let soldier_model = super::skinned_gltf::load_skinned_gltf(&device, &soldier_path);
        let zombie_path = std::path::Path::new(&resource_path).join("assets/models/zombie.glb");
        let zombie_model = super::skinned_gltf::load_skinned_gltf(&device, &zombie_path);

        let ecs_app = build_entity_ecs_app();

        let scope_color_descriptor = metal::TextureDescriptor::new();
        scope_color_descriptor.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
        scope_color_descriptor.set_width(SCOPE_TEXTURE_SIZE);
        scope_color_descriptor.set_height(SCOPE_TEXTURE_SIZE);
        scope_color_descriptor.set_storage_mode(metal::MTLStorageMode::Private);
        scope_color_descriptor.set_usage(metal::MTLTextureUsage::RenderTarget | metal::MTLTextureUsage::ShaderRead);
        let scope_color_texture = device.new_texture(&scope_color_descriptor);

        let scope_depth_descriptor = metal::TextureDescriptor::new();
        scope_depth_descriptor.set_pixel_format(MTLPixelFormat::Depth32Float);
        scope_depth_descriptor.set_width(SCOPE_TEXTURE_SIZE);
        scope_depth_descriptor.set_height(SCOPE_TEXTURE_SIZE);
        scope_depth_descriptor.set_storage_mode(metal::MTLStorageMode::Private);
        scope_depth_descriptor.set_usage(metal::MTLTextureUsage::RenderTarget);
        let scope_depth_texture = device.new_texture(&scope_depth_descriptor);

        let quad_pipeline = match device.new_library_with_source(QUAD_SHADER_SOURCE, &compile_options) {
            Ok(library) => {
                let vertex_fn = library.get_function("quad_vertex", None).unwrap();
                let fragment_fn = library.get_function("quad_fragment", None).unwrap();
                let descriptor = RenderPipelineDescriptor::new();
                descriptor.set_vertex_function(Some(&vertex_fn));
                descriptor.set_fragment_function(Some(&fragment_fn));
                descriptor.color_attachments().object_at(0).unwrap().set_pixel_format(MTLPixelFormat::BGRA8Unorm);
                device.new_render_pipeline_state(&descriptor).ok()
            }
            Err(err) => {
                eprintln!("[native_metal_probe] quad shader compile failed: {err}");
                None
            }
        };
        // Picture-in-picture rectangle in the top-right corner, in NDC
        // (-1..1) space — no view/projection needed, it's drawn directly
        // over the finished main-camera frame. Two triangles, position +
        // uv interleaved per vertex (same "raw device buffer, no vertex
        // descriptor" reasoning as every other shader in this file).
        let quad_verts = [
            QuadVertex { position: [0.55, 0.55], uv: [0.0, 0.0] },
            QuadVertex { position: [0.95, 0.55], uv: [1.0, 0.0] },
            QuadVertex { position: [0.95, 0.95], uv: [1.0, 1.0] },
            QuadVertex { position: [0.55, 0.55], uv: [0.0, 0.0] },
            QuadVertex { position: [0.95, 0.95], uv: [1.0, 1.0] },
            QuadVertex { position: [0.55, 0.95], uv: [0.0, 1.0] },
        ];
        let quad_vertex_buffer = device.new_buffer_with_data(
            quad_verts.as_ptr() as *const _,
            (quad_verts.len() * std::mem::size_of::<QuadVertex>()) as u64,
            MTLResourceOptions::StorageModeShared,
        );

        Self {
            device,
            layer,
            queue,
            pipeline,
            sponza_pipeline,
            sponza_meshes,
            sponza_sampler,
            white_texture,
            vertex_buffer,
            vertex_count: vertices.len(),
            depth_state,
            depth_texture: None,
            camera_position: [0.0, 2.5, 2.0],
            camera_yaw: 0.0,
            camera_pitch: -0.25,
            skinned_pipeline,
            soldier_model,
            zombie_model,
            scope_color_texture,
            scope_depth_texture,
            quad_pipeline,
            quad_vertex_buffer,
            ecs_app,
            start_time: std::time::Instant::now(),
        }
    }

    // Same touch-driven free-fly camera as the Swift-only demo used to
    // have — Swift now tracks the raw touches (see ViewController.swift's
    // moveTouch/lookTouch) and passes the resulting per-frame deltas here
    // instead of moving its own camera fields, so this Rust probe can
    // actually be walked around like the real game for a feel of
    // real-world performance, not just a static shot.
    pub fn update_camera(&mut self, forward: f32, strafe: f32, look_dx: f32, look_dy: f32, dt: f32) {
        self.camera_yaw -= look_dx * 0.005;
        self.camera_pitch = (self.camera_pitch - look_dy * 0.005).clamp(-1.54, 1.54);

        if forward != 0.0 || strafe != 0.0 {
            let (sin_yaw, cos_yaw) = self.camera_yaw.sin_cos();
            let forward_dir = [-sin_yaw, 0.0, -cos_yaw];
            let right_dir = [cos_yaw, 0.0, -sin_yaw];
            let speed = 6.0 * dt;
            self.camera_position[0] += (forward_dir[0] * forward + right_dir[0] * strafe) * speed;
            self.camera_position[1] += (forward_dir[1] * forward + right_dir[1] * strafe) * speed;
            self.camera_position[2] += (forward_dir[2] * forward + right_dir[2] * strafe) * speed;
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
    // Re-reads the layer's own current `bounds` (in points) each frame and
    // updates drawableSize accordingly — CAMetalLayer.drawableSize doesn't
    // auto-track bounds changes when driven manually like this (unlike
    // MTKView, which redoes this every layout pass), and setting it only
    // once in `new()` used whatever bounds existed at that moment, which
    // may predate Auto Layout settling on the view's final frame — this
    // produced a torn/skewed image (part of the screen using stale
    // geometry from an earlier, wrong-sized drawable).
    fn sync_drawable_size(&self) {
        unsafe {
            let raw_layer = self.layer.as_ptr() as *mut Object;
            let bounds: CGRect = msg_send![raw_layer, bounds];
            let scale = self.layer.contents_scale();
            let target = CGSize::new(bounds.size.width * scale, bounds.size.height * scale);
            let current = self.layer.drawable_size();
            if (current.width - target.width).abs() > 0.5 || (current.height - target.height).abs() > 0.5 {
                self.layer.set_drawable_size(target);
            }
        }
    }

    // Shared between the main-camera pass and the scope pass — draws
    // Sponza's static meshes (or the box-arena fallback) plus every
    // extracted entity, against whatever view_projection/render target
    // the caller already bound on `encoder`. This is what actually makes
    // "two cameras" cheap to add: same draw calls, just re-run once per
    // camera with a different matrix and target texture.
    fn draw_scene(
        &self,
        encoder: &metal::RenderCommandEncoderRef,
        view_projection: [[f32; 4]; 4],
        light_dir: [f32; 3],
        extracted: &[(bevy::transform::components::Transform, EntityKind, f32)],
        now: f32,
        use_sponza: bool,
    ) {
        if use_sponza {
            encoder.set_render_pipeline_state(self.sponza_pipeline.as_ref().unwrap());
            encoder.set_fragment_sampler_state(0, Some(&self.sponza_sampler));
            for mesh in &self.sponza_meshes {
                let sponza_uniforms = SponzaUniforms {
                    view_projection,
                    light_dir,
                    _pad: 0.0,
                    base_color_factor: mesh.base_color_factor,
                };
                encoder.set_vertex_buffer(0, Some(&mesh.vertex_buffer), 0);
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
                let texture = mesh.base_color_texture.as_ref().unwrap_or(&self.white_texture);
                encoder.set_fragment_texture(0, Some(texture));
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

        let Some(pipeline) = &self.skinned_pipeline else { return };
        encoder.set_render_pipeline_state(pipeline);
        encoder.set_fragment_sampler_state(0, Some(&self.sponza_sampler));

        for (transform, kind, anim_offset) in extracted {
            let model = match kind {
                EntityKind::Soldier => self.soldier_model.as_ref(),
                EntityKind::Zombie => self.zombie_model.as_ref(),
            };
            let Some(model) = model else { continue };

            let [qx, qy, qz, qw] = transform.rotation.to_array();
            let (x2, y2, z2) = (qx + qx, qy + qy, qz + qz);
            let (xx, xy, xz) = (qx * x2, qx * y2, qx * z2);
            let (yy, yz, zz) = (qy * y2, qy * z2, qz * z2);
            let (wx, wy, wz) = (qw * x2, qw * y2, qw * z2);
            let model_matrix = [
                [1.0 - (yy + zz), xy + wz, xz - wy, 0.0],
                [xy - wz, 1.0 - (xx + zz), yz + wx, 0.0],
                [xz + wy, yz - wx, 1.0 - (xx + yy), 0.0],
                [transform.translation.x, transform.translation.y, transform.translation.z, 1.0],
            ];

            let joint_matrices = model.compute_joint_matrices(0, now + anim_offset);
            let joint_buffer = self.device.new_buffer_with_data(
                joint_matrices.as_ptr() as *const _,
                (joint_matrices.len() * std::mem::size_of::<[[f32; 4]; 4]>()) as u64,
                MTLResourceOptions::StorageModeShared,
            );

            for mesh in &model.meshes {
                let skinned_uniforms = SkinnedUniforms {
                    view_projection,
                    model_matrix,
                    light_dir,
                    _pad: 0.0,
                    base_color_factor: mesh.base_color_factor,
                };
                encoder.set_vertex_buffer(0, Some(&mesh.vertex_buffer), 0);
                encoder.set_vertex_buffer(2, Some(&joint_buffer), 0);
                encoder.set_vertex_bytes(
                    1,
                    std::mem::size_of::<SkinnedUniforms>() as u64,
                    &skinned_uniforms as *const _ as *const _,
                );
                encoder.set_fragment_bytes(
                    1,
                    std::mem::size_of::<SkinnedUniforms>() as u64,
                    &skinned_uniforms as *const _ as *const _,
                );
                let texture = mesh.base_color_texture.as_ref().unwrap_or(&self.white_texture);
                encoder.set_fragment_texture(0, Some(texture));
                encoder.draw_indexed_primitives(
                    MTLPrimitiveType::Triangle,
                    mesh.index_count as u64,
                    MTLIndexType::UInt32,
                    &mesh.index_buffer,
                    0,
                );
            }
        }
    }

    pub fn render_frame(&mut self) {
        self.sync_drawable_size();
        let drawable_size = self.layer.drawable_size();
        let (width, height) = (drawable_size.width as u64, drawable_size.height as u64);
        let depth_texture = self.ensure_depth_texture(width, height);
        let Some(drawable) = self.layer.next_drawable() else { return };
        let texture = drawable.texture();

        let use_sponza = self.sponza_pipeline.is_some() && !self.sponza_meshes.is_empty();
        let aspect = width as f32 / height.max(1) as f32;
        let far = if use_sponza { 500.0 } else { 100.0 };
        let (sin_pitch, cos_pitch) = self.camera_pitch.sin_cos();
        let (sin_yaw, cos_yaw) = self.camera_yaw.sin_cos();
        let look_dir = [-sin_yaw * cos_pitch, sin_pitch, -cos_yaw * cos_pitch];
        let center = [
            self.camera_position[0] + look_dir[0],
            self.camera_position[1] + look_dir[1],
            self.camera_position[2] + look_dir[2],
        ];
        let view = if use_sponza {
            look_at(self.camera_position, center, [0.0, 1.0, 0.0])
        } else {
            look_at([0.0, 3.0, 10.0], [0.0, 0.0, 0.0], [0.0, 1.0, 0.0])
        };
        let proj = perspective(if use_sponza { 60.0_f32 } else { 45.0 }.to_radians(), aspect, 0.05, far);
        let light_dir = {
            let v = [-4.0f32, -8.0, -4.0];
            let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            [v[0] / len, v[1] / len, v[2] / len]
        };
        let view_projection = mat_mul(proj, view);
        // Scope camera: same eye/orientation as the main camera, just a
        // much narrower FOV (12° vs 60°) — simulates a zoomed rifle scope
        // without needing separate camera-aim state yet.
        let scope_proj = perspective(12.0_f32.to_radians(), 1.0, 0.05, far);
        let scope_view_projection = mat_mul(scope_proj, view);

        let now = self.start_time.elapsed().as_secs_f32();
        // Tick the ECS world once (advances Time, runs patrol_system over
        // every Patrol+Transform entity), then extract — the actual
        // "extract" seam the plan calls for, decoupled from bevy_render
        // entirely. Collected into an owned Vec (not a live query/World
        // borrow) so draw_scene can be called twice — once per camera —
        // without re-ticking Time in between.
        self.ecs_app.update();
        let extracted: Vec<(bevy::transform::components::Transform, EntityKind, f32)> = {
            let mut query_state = self
                .ecs_app
                .world_mut()
                .query::<(&bevy::transform::components::Transform, &EntityKind, &AnimOffset)>();
            query_state.iter(self.ecs_app.world()).map(|(t, k, a)| (*t, *k, a.0)).collect()
        };

        // Pass 1: scope camera into the offscreen texture.
        let scope_pass_descriptor = RenderPassDescriptor::new();
        let scope_color = scope_pass_descriptor.color_attachments().object_at(0).unwrap();
        scope_color.set_texture(Some(&self.scope_color_texture));
        scope_color.set_load_action(MTLLoadAction::Clear);
        scope_color.set_store_action(MTLStoreAction::Store);
        scope_color.set_clear_color(MTLClearColor::new(0.05, 0.05, 0.05, 1.0));
        let scope_depth = scope_pass_descriptor.depth_attachment().unwrap();
        scope_depth.set_texture(Some(&self.scope_depth_texture));
        scope_depth.set_load_action(MTLLoadAction::Clear);
        scope_depth.set_store_action(MTLStoreAction::DontCare);
        scope_depth.set_clear_depth(1.0);

        let scope_command_buffer = self.queue.new_command_buffer();
        let scope_encoder = scope_command_buffer.new_render_command_encoder(scope_pass_descriptor);
        scope_encoder.set_depth_stencil_state(&self.depth_state);
        self.draw_scene(scope_encoder, scope_view_projection, light_dir, &extracted, now, use_sponza);
        scope_encoder.end_encoding();
        scope_command_buffer.commit();

        // Pass 2: main camera straight to the drawable, then the scope
        // texture composited as a picture-in-picture quad on top.
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
        self.draw_scene(encoder, view_projection, light_dir, &extracted, now, use_sponza);

        if let Some(quad_pipeline) = &self.quad_pipeline {
            encoder.set_render_pipeline_state(quad_pipeline);
            encoder.set_vertex_buffer(0, Some(&self.quad_vertex_buffer), 0);
            encoder.set_fragment_texture(0, Some(&self.scope_color_texture));
            encoder.set_fragment_sampler_state(0, Some(&self.sponza_sampler));
            encoder.draw_primitives(MTLPrimitiveType::Triangle, 0, 6);
        }

        encoder.end_encoding();
        command_buffer.present_drawable(drawable);
        command_buffer.commit();
    }
}

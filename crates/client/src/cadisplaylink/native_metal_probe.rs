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

// Phase 6 Step 4: draws the REAL game's own level (spawn_test_arena's walls
// + floor, the ak47_scope gun) instead of this probe's own hardcoded
// Sponza/box-arena — reusing every pipeline/shader built below unchanged,
// only the vertex data and camera pose source change. See
// native_game_app.rs (headless App) and native_game_extract.rs
// (extraction).
use super::native_game_app::build_headless_game_ecs_app;
use super::native_game_extract::{extract_render_snapshot, ExtractedMesh, LightSnapshot, SunSnapshot};

// Lighting was never actually ported when this renderer took over from
// bevy_render/wgpu — every fragment shader below used a single hardcoded
// "sun" direction unrelated to any real light in the scene (see
// render_frame's own former `light_dir` constant). Real-game mode now
// extracts BOTH real lights crate::lib actually spawns: the always-present
// `DirectionalLight`/"MainSun" (per its own doc comment, the *primary*
// light for the open USE_TEST_ARENA room) as an unattenuated directional
// term, and the arena's overhead `PointLight` (LightSnapshot) as a
// secondary term attenuated by real distance/range — missing the sun
// entirely was an oversight in the first lighting-port pass, which only
// grabbed the point light. Point-light position/color/range only (not raw
// lumen `intensity`, which can be in the millions) — see LightSnapshot's
// own doc comment for why; same reasoning for not extracting the sun's
// `illuminance` (lux, also not a 0..1 scalar this non-PBR shader could use
// directly) — it contributes at the same fixed weight the old hardcoded
// directional term always used, just with the real direction/color now.
#[derive(Copy, Clone)]
struct LightUniform {
    position: [f32; 3],
    range: f32,
    // [f32; 4] (not 3) with an unused trailing 0.0 — every Uniforms struct
    // below places a float4 (base_color_factor, or sun_color right after)
    // and MSL's float4 *always* self-aligns to a 16-byte offset inside a
    // struct regardless of what precedes it. Padding light_pos (12) +
    // light_range (4) = 16 exactly, then making light_color a full
    // 16-byte float4 too, keeps every subsequent field's offset landing
    // on a clean 16-byte boundary in both Rust's repr(C) layout and
    // MSL's — no manual `_pad` field needed, and no repeat of the exact
    // struct-layout mismatch bug documented on skinned_fragment below.
    color: [f32; 4],
    // Direction the sun's light travels *toward* (see SunSnapshot's own
    // doc comment) — same [f32;3]+f32-pad-to-16 pairing as position/range
    // above; the trailing f32 is unused (no per-sun range concept).
    sun_dir: [f32; 3],
    _sun_pad: f32,
    sun_color: [f32; 4],
}

impl LightUniform {
    fn from_snapshots(light: Option<&LightSnapshot>, sun: Option<&SunSnapshot>) -> Self {
        let (position, range, color) = match light {
            Some(l) => (l.position, l.range, [l.color[0], l.color[1], l.color[2], 0.0]),
            None => (FALLBACK_LIGHT.position, FALLBACK_LIGHT.range, FALLBACK_LIGHT.color),
        };
        let (sun_dir, sun_color) = match sun {
            Some(s) => (s.direction, [s.color[0], s.color[1], s.color[2], 0.0]),
            None => (FALLBACK_LIGHT.sun_dir, FALLBACK_LIGHT.sun_color),
        };
        LightUniform { position, range, color, sun_dir, _sun_pad: 0.0, sun_color }
    }
}

// Fallback for whenever there's no real light to extract (the toy demo/
// patrol probe world, which has no ECS lights at all) — a distant,
// huge-range point light plus a downward directional term approximates
// the old fixed-direction "sun" look well enough for that non-gameplay
// path.
const FALLBACK_LIGHT: LightUniform = LightUniform {
    position: [400.0, 800.0, 400.0],
    range: 4000.0,
    color: [1.0, 1.0, 1.0, 0.0],
    sun_dir: [-0.33, -0.67, -0.33],
    _sun_pad: 0.0,
    sun_color: [1.0, 1.0, 1.0, 0.0],
};

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
    light_pos: [f32; 3],
    light_range: f32,
    light_color: [f32; 4],
    sun_dir: [f32; 3],
    _sun_pad: f32,
    sun_color: [f32; 4],
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
// what produced the garbled rainbow/spike geometry the first time. Same
// reasoning applies to every float3 in Uniforms below (see skinned_fragment's
// own doc comment for the exact bug this caused once a struct had fields
// *after* a plain float3).
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
    packed_float3 lightPos;
    float lightRange;
    float4 lightColor;
    packed_float3 sunDir;
    float _sunPad;
    float4 sunColor;
};

struct VertexOut {
    float4 position [[position]];
    float3 worldPos;
    float3 normal;
    float3 color;
};

vertex VertexOut vertex_main(uint vid [[vertex_id]],
                              const device Vertex *verts [[buffer(0)]],
                              constant Uniforms &uniforms [[buffer(1)]]) {
    VertexOut out;
    Vertex v = verts[vid];
    out.position = uniforms.viewProjection * float4(float3(v.position), 1.0);
    out.worldPos = float3(v.position);
    out.normal = float3(v.normal);
    out.color = float3(v.color);
    return out;
}

fragment float4 fragment_main(VertexOut in [[stage_in]],
                               constant Uniforms &uniforms [[buffer(1)]]) {
    float sunDiffuse = max(dot(normalize(in.normal), -normalize(float3(uniforms.sunDir))), 0.0);
    float3 sunTerm = uniforms.sunColor.rgb * sunDiffuse;

    float3 toLight = float3(uniforms.lightPos) - in.worldPos;
    float dist = length(toLight);
    float atten = clamp(1.0 - dist / max(uniforms.lightRange, 0.0001), 0.0, 1.0);
    atten *= atten;
    float pointDiffuse = max(dot(normalize(in.normal), toLight / max(dist, 0.0001)), 0.0) * atten;
    float3 pointTerm = uniforms.lightColor.rgb * pointDiffuse;

    float3 lightTerm = clamp(sunTerm + pointTerm, 0.0, 1.0);
    float3 lit = in.color * (0.35 + 0.65 * lightTerm);
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
    packed_float3 lightPos;
    float lightRange;
    float4 lightColor;
    packed_float3 sunDir;
    float _sunPad;
    float4 sunColor;
    float4 baseColorFactor;
};

struct VertexOut {
    float4 position [[position]];
    float3 worldPos;
    float3 normal;
    float2 uv;
};

vertex VertexOut sponza_vertex(uint vid [[vertex_id]],
                                const device Vertex *verts [[buffer(0)]],
                                constant Uniforms &uniforms [[buffer(1)]]) {
    VertexOut out;
    Vertex v = verts[vid];
    out.position = uniforms.viewProjection * float4(float3(v.position), 1.0);
    out.worldPos = float3(v.position);
    out.normal = float3(v.normal);
    out.uv = float2(v.uv);
    return out;
}

fragment float4 sponza_fragment(VertexOut in [[stage_in]],
                                 constant Uniforms &uniforms [[buffer(1)]],
                                 texture2d<float> baseColorTexture [[texture(0)]],
                                 sampler texSampler [[sampler(0)]]) {
    float sunDiffuse = max(dot(normalize(in.normal), -normalize(float3(uniforms.sunDir))), 0.0);
    float3 sunTerm = uniforms.sunColor.rgb * sunDiffuse;

    float3 toLight = float3(uniforms.lightPos) - in.worldPos;
    float dist = length(toLight);
    float atten = clamp(1.0 - dist / max(uniforms.lightRange, 0.0001), 0.0, 1.0);
    atten *= atten;
    float pointDiffuse = max(dot(normalize(in.normal), toLight / max(dist, 0.0001)), 0.0) * atten;
    float3 pointTerm = uniforms.lightColor.rgb * pointDiffuse;

    float3 lightTerm = clamp(sunTerm + pointTerm, 0.0, 1.0);
    float4 texColor = baseColorTexture.sample(texSampler, in.uv);
    float4 baseColor = texColor * uniforms.baseColorFactor;
    float3 lit = baseColor.rgb * (0.35 + 0.65 * lightTerm);
    return float4(lit, baseColor.a);
}
"#;

#[repr(C)]
#[derive(Copy, Clone)]
struct SponzaUniforms {
    view_projection: [[f32; 4]; 4],
    light_pos: [f32; 3],
    light_range: f32,
    light_color: [f32; 4],
    sun_dir: [f32; 3],
    _sun_pad: f32,
    sun_color: [f32; 4],
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
    packed_float3 lightPos;
    float lightRange;
    float4 lightColor;
    packed_float3 sunDir;
    float _sunPad;
    float4 sunColor;
    float4 baseColorFactor;
};

struct VertexOut {
    float4 position [[position]];
    float3 worldPos;
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
    float4 worldPos4 = uniforms.modelMatrix * skinnedPos;
    out.position = uniforms.viewProjection * worldPos4;
    out.worldPos = worldPos4.xyz;
    out.normal = (uniforms.modelMatrix * float4(skinnedNormal, 0.0)).xyz;
    out.uv = float2(v.uv);
    return out;
}

fragment float4 skinned_fragment(VertexOut in [[stage_in]],
                                  constant Uniforms &uniforms [[buffer(1)]],
                                  texture2d<float> baseColorTexture [[texture(0)]],
                                  sampler texSampler [[sampler(0)]]) {
    // Root cause of an earlier version of this struct (kept for the next
    // person who hits this class of bug) found via a 3-step device/
    // Simulator console+shader bisection: (1) raw texColor sample alone →
    // colorful — rules out texture binding/decode; (2) texColor *
    // uniforms.baseColorFactor → solid black — isolates the bug to reading
    // baseColorFactor itself; (3) a lone `float3 lightDir` field (MSL
    // default alignment 16 per-member — NOT the GLSL std140 "vec3 forces
    // the next struct member to a 16-byte boundary" rule some docs
    // conflate this with, but the net effect here was the same) placed
    // whatever followed it at a different byte offset than Rust's
    // tightly-packed repr(C) struct, since a lone `float3` still consumes
    // a rounded-up 16-byte slot when a scalar or vec4 follows. Every
    // `packed_float3`/plain-float4 field below is chosen specifically so
    // each field's offset lands on the same byte in both Rust's repr(C)
    // layout and MSL's (see LightUniform's own doc comment) instead of
    // re-deriving this by trial and error again.
    float sunDiffuse = max(dot(normalize(in.normal), -normalize(float3(uniforms.sunDir))), 0.0);
    float3 sunTerm = uniforms.sunColor.rgb * sunDiffuse;

    float3 toLight = float3(uniforms.lightPos) - in.worldPos;
    float dist = length(toLight);
    float atten = clamp(1.0 - dist / max(uniforms.lightRange, 0.0001), 0.0, 1.0);
    atten *= atten;
    float pointDiffuse = max(dot(normalize(in.normal), toLight / max(dist, 0.0001)), 0.0) * atten;
    float3 pointTerm = uniforms.lightColor.rgb * pointDiffuse;

    float3 lightTerm = clamp(sunTerm + pointTerm, 0.0, 1.0);
    float4 texColor = baseColorTexture.sample(texSampler, in.uv);
    float4 baseColor = texColor * uniforms.baseColorFactor;
    float3 lit = baseColor.rgb * (0.35 + 0.65 * lightTerm);
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
    // Circular mask (real game's scope lens is round, fitting the scope
    // tube opening — see update_scope_lens/SCOPE_LENS_SIZE in
    // crates/client/src/lib.rs) instead of a square PIP box. Discarding
    // outside the inscribed circle just leaves whatever the main camera
    // already drew showing through — no blending needed for the cutout
    // itself.
    float2 centered = in.uv - float2(0.5, 0.5);
    if (length(centered) > 0.5) {
        discard_fragment();
    }
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
    light_pos: [f32; 3],
    light_range: f32,
    light_color: [f32; 4],
    sun_dir: [f32; 3],
    _sun_pad: f32,
    sun_color: [f32; 4],
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
    // Phase 6 Step 4 (temporary, alongside `ecs_app`'s toy patrol World):
    // `Some` only when constructed via `new(view, true)` — the headless
    // real game (build_game_common: physics/networking/AI/prediction),
    // ticked and extracted every frame instead of the toy `ecs_app`. When
    // present, `render_frame` draws its static geometry (arena+gun) and
    // drives the camera from its `FpsCamera` instead of touch input.
    real_game_app: Option<bevy::app::App>,
    // Cache of per-static-mesh textures (arena walls/floor, gun scene
    // meshes) — GPU texture upload is far more expensive than the small
    // per-frame vertex buffer rebuild, so unlike vertex data (which must be
    // rebaked every frame since Transform can change — gun sway/recoil),
    // textures are only (re)uploaded when the mesh count changes (i.e. new
    // assets finished loading), keyed positionally by index into
    // `RenderSnapshot::static_meshes` (stable frame-to-frame since the same
    // entities keep existing once spawned).
    real_game_textures: Vec<Option<metal::Texture>>,
    // Real-game mode only: the FpsCamera entity's actual world-space
    // rotation quaternion (xyzw), straight from GlobalTransform — includes
    // ViewRecoil's kick and Lean's roll, unlike camera_yaw/camera_pitch
    // (which only ever hold the pre-recoil base angles). Identity when not
    // yet set / not in real-game mode.
    real_game_camera_rotation: [f32; 4],
    // Real `Gun.ads_blend`/`ScopeZoom` — drive whether the scope PIP
    // overlay shows at all (only while actually aiming, not the demo
    // probe's always-on PIP) and its FOV (matching the real game's
    // `SCOPE_BASE_FOV / zoom` formula instead of a fixed guess).
    real_game_ads_blend: f32,
    real_game_scope_zoom: f32,
    real_game_wall_raise: f32,
    // The real ScopeCamera entity's own state (see ScopeCameraSnapshot's
    // doc comment on native_game_extract.rs) — using this directly instead
    // of the main camera's transform + our own reconstructed show/hide
    // condition is what gives the scope view its real shoulder-swap sweep,
    // recoil kick, reload dip, and walk-bob for free, and the real
    // Camera.is_active/Projection.fov instead of an approximation.
    real_game_scope_eye: [f32; 3],
    real_game_scope_rotation: [f32; 4],
    real_game_scope_active: bool,
    real_game_scope_fov: f32,
    // The scene's real PointLight, if any — see LightSnapshot's own doc
    // comment. `None` (falls back to FALLBACK_LIGHT) before the first
    // extraction, or for the toy demo/patrol probe world, which has no
    // ECS lights at all.
    real_game_light: LightUniform,
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

// Flattens ONE of the real game's extracted static meshes (arena
// walls/floor, or one of the gun's glTF-scene child meshes — see
// native_game_extract.rs) into a plain triangle list in the SAME
// `GltfVertex{position,normal,uv}` layout Sponza's own meshes already use
// (see gltf_scene.rs) — so drawing them with real textures needs no new
// shader/pipeline, just reusing `self.sponza_pipeline`/`self.sponza_sampler`
// unchanged, with a fresh `MTLBuffer` each frame (same "rebuild a small
// buffer per frame" precedent as the skinned-mesh joint buffer below).
// Bakes each mesh's `Transform` into world-space positions/normals on the
// CPU, matching how Sponza/box-arena vertices are already pre-baked at
// load time (no per-draw model matrix in this shader).
fn bake_static_mesh_vertices(mesh: &ExtractedMesh) -> Vec<super::gltf_scene::GltfVertex> {
    let mut verts = Vec::new();
    let vertex_at = |idx: usize| -> super::gltf_scene::GltfVertex {
        let local_pos = bevy::math::Vec3::from_array(mesh.positions[idx]);
        let local_normal = if mesh.normals.is_empty() {
            bevy::math::Vec3::Y
        } else {
            bevy::math::Vec3::from_array(mesh.normals[idx])
        };
        let uv = if mesh.uvs.is_empty() { [0.0, 0.0] } else { mesh.uvs[idx] };
        let world_pos = mesh.transform.transform_point(local_pos);
        let world_normal = (mesh.transform.rotation * local_normal).normalize_or_zero();
        super::gltf_scene::GltfVertex { position: world_pos.to_array(), normal: world_normal.to_array(), uv }
    };
    if mesh.indices.is_empty() {
        for idx in 0..mesh.positions.len() {
            verts.push(vertex_at(idx));
        }
    } else {
        for &idx in &mesh.indices {
            verts.push(vertex_at(idx as usize));
        }
    }
    verts
}

// One real-game static mesh (arena wall/floor, or a gun scene child mesh),
// ready to draw this frame — see bake_static_mesh_vertices and
// render_frame's texture-cache handling for how these get built.
// `texture: None` (untextured material, or an unsupported CPU pixel format
// — see native_game_extract.rs) falls back to `self.white_texture` in
// `draw_scene`, same fallback Sponza's own meshes already use.
// Real-game scope PIP: a small circle centered on screen (matching where
// the real game's lens quad sits, dead center in the scope tube — see
// SCOPE_LENS_OFFSET/update_scope_lens in crates/client/src/lib.rs) instead
// of the demo probe's fixed top-right box. `half_height` is in NDC
// (-1..1); `aspect` (width/height) corrects the X half-width so the quad
// is a true circle in screen space rather than an ellipse stretched by a
// non-square viewport — same reasoning perspective() already applies to
// the main projection's own aspect correction.
fn build_centered_quad(device: &Device, aspect: f32, half_height: f32) -> metal::Buffer {
    let half_w = half_height / aspect;
    let half_h = half_height;
    let verts = [
        QuadVertex { position: [-half_w, -half_h], uv: [0.0, 0.0] },
        QuadVertex { position: [half_w, -half_h], uv: [1.0, 0.0] },
        QuadVertex { position: [half_w, half_h], uv: [1.0, 1.0] },
        QuadVertex { position: [-half_w, -half_h], uv: [0.0, 0.0] },
        QuadVertex { position: [half_w, half_h], uv: [1.0, 1.0] },
        QuadVertex { position: [-half_w, half_h], uv: [0.0, 1.0] },
    ];
    device.new_buffer_with_data(
        verts.as_ptr() as *const _,
        (verts.len() * std::mem::size_of::<QuadVertex>()) as u64,
        MTLResourceOptions::StorageModeShared,
    )
}

struct RealGameMeshDraw<'a> {
    vertex_buffer: metal::Buffer,
    vertex_count: usize,
    base_color_factor: [f32; 4],
    texture: Option<&'a metal::Texture>,
}

impl NativeMetalProbe {
    // `view` is the raw UIView* handed over from Swift — same FFI contract
    // as the existing cadisplaylink module (see ios_view.rs). We reach
    // into it via objc msg_send! to grab its CAMetalLayer, exactly what
    // MetalView.swift's `layerClass` override already made it.
    // `real_game` selects Phase 6 Step 4's mode: draw the real game's own
    // level (arena+gun, camera driven by FpsCamera) instead of this
    // probe's own toy Sponza/box-arena/patrol World. Every pipeline/shader
    // below is still built either way — only which World gets ticked and
    // which vertex data feeds `self.pipeline` differs, in `render_frame`.
    pub unsafe fn new(view: *mut Object, real_game: bool) -> Self {
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
                let sponza_color_attachment = sponza_pipeline_descriptor.color_attachments().object_at(0).unwrap();
                sponza_color_attachment.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
                // Real-game materials with `AlphaMode::Blend` (e.g. the
                // scope lens glass on the ak47_scope model) carry alpha < 1
                // in their base_color/texture — without blending enabled
                // here, Metal ignores that alpha and paints every fragment
                // fully opaque regardless of what the fragment shader
                // returns, which is why the lens looked solid instead of
                // see-through once real textures/materials replaced the
                // earlier flat-color placeholder. Standard "source over"
                // blending: opaque (alpha=1) surfaces render identically to
                // unblended, so this is safe for the arena/gun's mostly-
                // opaque geometry too, not just the lens.
                sponza_color_attachment.set_blending_enabled(true);
                sponza_color_attachment.set_rgb_blend_operation(metal::MTLBlendOperation::Add);
                sponza_color_attachment.set_alpha_blend_operation(metal::MTLBlendOperation::Add);
                sponza_color_attachment.set_source_rgb_blend_factor(metal::MTLBlendFactor::SourceAlpha);
                sponza_color_attachment.set_source_alpha_blend_factor(metal::MTLBlendFactor::SourceAlpha);
                sponza_color_attachment.set_destination_rgb_blend_factor(metal::MTLBlendFactor::OneMinusSourceAlpha);
                sponza_color_attachment.set_destination_alpha_blend_factor(metal::MTLBlendFactor::OneMinusSourceAlpha);
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
        let real_game_app = if real_game { Some(build_headless_game_ecs_app()) } else { None };

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
            real_game_app,
            real_game_textures: Vec::new(),
            real_game_camera_rotation: [0.0, 0.0, 0.0, 1.0],
            real_game_ads_blend: 0.0,
            real_game_scope_zoom: 8.0,
            real_game_wall_raise: 0.0,
            real_game_scope_eye: [0.0, 0.0, 0.0],
            real_game_scope_rotation: [0.0, 0.0, 0.0, 1.0],
            real_game_scope_active: false,
            real_game_scope_fov: 12.0_f32.to_radians(),
            real_game_light: FALLBACK_LIGHT,
        }
    }

    // Phase 6 Step 3: exposes the headless real-game App to ffi.rs's
    // `native_metal_probe_touch_*`/`key_changed`/`mouse_*` functions so they
    // can write real Bevy input messages (`TouchInput`/`KeyboardInput`/
    // `MouseMotion`/`MouseButtonInput`) straight into it — the exact same
    // message-writing helpers `cadisplaylink_*` already uses against the
    // toy/real `cadisplaylink_create_game_app`, just re-targeted. `None`
    // when this probe wasn't constructed with `real_game: true`.
    pub fn real_game_app_mut(&mut self) -> Option<&mut bevy::app::App> {
        self.real_game_app.as_mut()
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
        light: LightUniform,
        // 4th field is `(clip_index, anim_time)` — an *absolute* time to
        // feed `compute_joint_matrices` directly, not an offset added to
        // `now` the way the toy demo's own AnimOffset component used to
        // work (that math now happens once at the call site instead, see
        // both places this is built below, so this fn doesn't need `now`
        // for animation at all anymore — real-game zombies use their
        // actual AnimationPlayer's seek_time, not wall-clock time).
        extracted: &[(bevy::transform::components::Transform, EntityKind, (usize, f32))],
        _now: f32,
        use_sponza: bool,
        real_game_meshes: Option<&[RealGameMeshDraw]>,
    ) {
        if let Some(meshes) = real_game_meshes {
            // Reuses Sponza's own pipeline/shader (position+normal+uv,
            // textured) unchanged — arena/gun geometry is textured exactly
            // like Sponza's, just rebaked into world space fresh each frame
            // (see bake_static_mesh_vertices) instead of once at load time.
            encoder.set_render_pipeline_state(self.sponza_pipeline.as_ref().unwrap());
            encoder.set_fragment_sampler_state(0, Some(&self.sponza_sampler));
            for m in meshes {
                let sponza_uniforms = SponzaUniforms {
                    view_projection,
                    light_pos: light.position,
                    light_range: light.range,
                    light_color: light.color,
                    sun_dir: light.sun_dir,
                    _sun_pad: 0.0,
                    sun_color: light.sun_color,
                    base_color_factor: m.base_color_factor,
                };
                encoder.set_vertex_buffer(0, Some(&m.vertex_buffer), 0);
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
                let texture = m.texture.unwrap_or(&self.white_texture);
                encoder.set_fragment_texture(0, Some(texture));
                encoder.draw_primitives(MTLPrimitiveType::Triangle, 0, m.vertex_count as u64);
            }
        } else if use_sponza {
            encoder.set_render_pipeline_state(self.sponza_pipeline.as_ref().unwrap());
            encoder.set_fragment_sampler_state(0, Some(&self.sponza_sampler));
            for mesh in &self.sponza_meshes {
                let sponza_uniforms = SponzaUniforms {
                    view_projection,
                    light_pos: light.position,
                    light_range: light.range,
                    light_color: light.color,
                    sun_dir: light.sun_dir,
                    _sun_pad: 0.0,
                    sun_color: light.sun_color,
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
            let uniforms = Uniforms {
                view_projection,
                light_pos: light.position,
                light_range: light.range,
                light_color: light.color,
                sun_dir: light.sun_dir,
                _sun_pad: 0.0,
                sun_color: light.sun_color,
            };
            encoder.set_render_pipeline_state(&self.pipeline);
            encoder.set_vertex_buffer(0, Some(&self.vertex_buffer), 0);
            encoder.set_vertex_bytes(1, std::mem::size_of::<Uniforms>() as u64, &uniforms as *const _ as *const _);
            encoder.set_fragment_bytes(1, std::mem::size_of::<Uniforms>() as u64, &uniforms as *const _ as *const _);
            encoder.draw_primitives(MTLPrimitiveType::Triangle, 0, self.vertex_count as u64);
        }

        let Some(pipeline) = &self.skinned_pipeline else { return };
        encoder.set_render_pipeline_state(pipeline);
        encoder.set_fragment_sampler_state(0, Some(&self.sponza_sampler));

        for (transform, kind, (anim_clip_index, anim_time)) in extracted {
            let model = match kind {
                EntityKind::Soldier => self.soldier_model.as_ref(),
                EntityKind::Zombie => self.zombie_model.as_ref(),
            };
            let Some(model) = model else { continue };

            // The real game never scales the Zombie entity's own Transform
            // (that's what native_game_extract.rs's GlobalTransform query
            // reads) — ZOMBIE_SCALE (0.25×) and ZOMBIE_MODEL_Y_OFFSET (feet-
            // to-capsule-center) are both applied only to the mesh's own
            // *child* entity, parented under the zombie (see
            // `zombie_mesh_bundle` in crate::lib). Omitting both here drew
            // zombie.glb at its raw, unscaled size — ~4x too big — floating
            // at the capsule's center instead of standing on its feet.
            //
            // PLAYER_MODEL_PATH (soldier.glb) was previously assumed to
            // need neither correction (spawned here at scale 1.0/offset
            // 0.0) — WRONG, confirmed by direct in-game report: the remote
            // player model rendered floating above the ground and facing
            // backward relative to its own travel direction. This is the
            // ONLY place that actually determines what's drawn on screen
            // for USE_REAL_GAME_NATIVE_RENDERER — the parallel Bevy-side
            // fix (on_net_player_replicated's child SceneRoot transform in
            // crate::lib, PLAYER_MODEL_SCALE/Y_OFFSET/FACING_CORRECTION)
            // only affects the desktop/non-native-renderer path, since this
            // custom Metal renderer never queries Bevy's own scene graph
            // for PlayerModel/PlayerModelAnimRoot at all. Reusing the same
            // constants here so both paths stay in sync.
            let (mesh_scale, y_offset) = match kind {
                EntityKind::Zombie => {
                    (shared::ZOMBIE_SCALE, -(shared::ZOMBIE_RADIUS + shared::ZOMBIE_LENGTH / 2.0))
                }
                EntityKind::Soldier => (crate::PLAYER_MODEL_SCALE, crate::PLAYER_MODEL_Y_OFFSET),
            };
            let rotation = match kind {
                EntityKind::Soldier => transform.rotation * bevy::math::Quat::from_rotation_y(crate::PLAYER_MODEL_FACING_CORRECTION),
                EntityKind::Zombie => transform.rotation,
            };

            let [qx, qy, qz, qw] = rotation.to_array();
            let (x2, y2, z2) = (qx + qx, qy + qy, qz + qz);
            let (xx, xy, xz) = (qx * x2, qx * y2, qx * z2);
            let (yy, yz, zz) = (qy * y2, qy * z2, qz * z2);
            let (wx, wy, wz) = (qw * x2, qw * y2, qw * z2);
            let model_matrix = [
                [(1.0 - (yy + zz)) * mesh_scale, (xy + wz) * mesh_scale, (xz - wy) * mesh_scale, 0.0],
                [(xy - wz) * mesh_scale, (1.0 - (xx + zz)) * mesh_scale, (yz + wx) * mesh_scale, 0.0],
                [(xz + wy) * mesh_scale, (yz - wx) * mesh_scale, (1.0 - (xx + yy)) * mesh_scale, 0.0],
                [
                    transform.translation.x,
                    transform.translation.y + y_offset,
                    transform.translation.z,
                    1.0,
                ],
            ];

            let joint_matrices = model.compute_joint_matrices(*anim_clip_index, *anim_time);
            let joint_buffer = self.device.new_buffer_with_data(
                joint_matrices.as_ptr() as *const _,
                (joint_matrices.len() * std::mem::size_of::<[[f32; 4]; 4]>()) as u64,
                MTLResourceOptions::StorageModeShared,
            );

            for mesh in &model.meshes {
                let skinned_uniforms = SkinnedUniforms {
                    view_projection,
                    model_matrix,
                    light_pos: light.position,
                    light_range: light.range,
                    light_color: light.color,
                    sun_dir: light.sun_dir,
                    _sun_pad: 0.0,
                    sun_color: light.sun_color,
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
        let is_real_game = self.real_game_app.is_some();
        // Moved up from further below (used to only be needed just before
        // the draw_scene calls) — the remote-player extraction just below
        // now needs it too, for its static PLAYER_MODEL_ANIM_WALK pose.
        let now = self.start_time.elapsed().as_secs_f32();

        // Phase 6 Step 4: tick the real headless game App (if this probe
        // was constructed with `real_game: true`) and extract a fresh
        // snapshot — camera pose comes from the game's own FpsCamera
        // instead of touch input, and the arena+gun geometry gets rebaked
        // into a plain vertex buffer each frame (see bake_static_meshes).
        let mut real_game_static_meshes: Vec<super::native_game_extract::ExtractedMesh> = Vec::new();
        // Real zombies/remote players, reusing the toy patrol World's own
        // `(Transform, EntityKind, (clip_index, anim_time))` shape so
        // draw_scene's existing skinned-mesh loop draws them unchanged —
        // zombies now carry their REAL AnimationPlayer's clip+seek_time
        // (see ZombieRenderState's own doc comment: the same "extract the
        // real thing" principle already applied to camera/scope camera),
        // not a hand-placed patrol dummy's phase offset.
        let mut real_game_extracted: Vec<(bevy::transform::components::Transform, EntityKind, (usize, f32))> =
            Vec::new();
        if let Some(app) = &mut self.real_game_app {
            app.update();
            let snapshot = extract_render_snapshot(app.world_mut());
            if let Some(cam) = &snapshot.camera {
                self.camera_position = cam.eye;
                self.camera_yaw = cam.yaw;
                self.camera_pitch = cam.pitch;
                self.real_game_camera_rotation = cam.rotation;
                self.real_game_ads_blend = cam.ads_blend;
                self.real_game_scope_zoom = cam.scope_zoom;
                self.real_game_wall_raise = cam.wall_raise;
            }
            if let Some(scope_cam) = &snapshot.scope_camera {
                self.real_game_scope_eye = scope_cam.eye;
                self.real_game_scope_rotation = scope_cam.rotation;
                self.real_game_scope_active = scope_cam.active;
                self.real_game_scope_fov = scope_cam.fov;
            } else {
                self.real_game_scope_active = false;
            }
            self.real_game_light = LightUniform::from_snapshots(snapshot.light.as_ref(), snapshot.sun.as_ref());
            real_game_static_meshes = snapshot.static_meshes;
            real_game_extracted.extend(
                snapshot
                    .zombies
                    .into_iter()
                    .map(|z| (z.transform, EntityKind::Zombie, (z.anim_clip_index, z.anim_time))),
            );
            // Remote players: no per-entity pose extraction wired up yet
            // (out of scope for this pass, which was specifically about
            // zombie.glb's 5 poses — see update_zombie_animation) — keep
            // playing PLAYER_MODEL_ANIM_WALK statically rather than the
            // previous hardcoded clip 0, which was correct for neither.
            real_game_extracted.extend(
                snapshot
                    .remote_players
                    .into_iter()
                    .map(|t| (t, EntityKind::Soldier, (crate::PLAYER_MODEL_ANIM_WALK, now))),
            );
        }
        // Texture cache: (re)upload when the mesh count changed OR when any
        // mesh's texture-presence flipped since the last check (see
        // `real_game_textures`'s own doc comment on the struct). Count
        // alone isn't enough: the gun's own texture loads asynchronously
        // (Bevy's asset server), so on the frame this list first stabilizes
        // at its final length, some meshes may still legitimately have
        // `texture: None` (still loading) rather than "no texture by
        // design" — caching that snapshot permanently left the gun stuck
        // rendering flat white (texture=None -> white_texture fallback)
        // for the rest of the app's life once its real texture DID finish
        // loading a moment later, since the count itself never changed
        // again to re-trigger a rebuild. Comparing the has-texture pattern
        // (not the pixels themselves — cheap) catches that transition.
        // Building a fresh `Device` clone rather than borrowing
        // `self.device` directly sidesteps a disjoint-field-borrow fight
        // with the `self.real_game_textures = ...` assignment right after.
        let texture_pattern_changed = self.real_game_textures.len() != real_game_static_meshes.len()
            || self
                .real_game_textures
                .iter()
                .zip(real_game_static_meshes.iter())
                .any(|(cached, mesh)| cached.is_some() != mesh.texture.is_some());
        if texture_pattern_changed {
            let device = self.device.clone();
            self.real_game_textures = real_game_static_meshes
                .iter()
                .map(|m| m.texture.as_ref().map(|t| super::gltf_scene::upload_texture(&device, t.width, t.height, &t.rgba)))
                .collect();
        }
        let real_game_meshes: Vec<RealGameMeshDraw> = real_game_static_meshes
            .iter()
            .zip(self.real_game_textures.iter())
            .map(|(mesh, texture)| {
                let verts = bake_static_mesh_vertices(mesh);
                let vertex_buffer = self.device.new_buffer_with_data(
                    verts.as_ptr() as *const _,
                    (verts.len() * std::mem::size_of::<super::gltf_scene::GltfVertex>()) as u64,
                    MTLResourceOptions::StorageModeShared,
                );
                RealGameMeshDraw {
                    vertex_buffer,
                    vertex_count: verts.len(),
                    base_color_factor: mesh.base_color,
                    texture: texture.as_ref(),
                }
            })
            .collect();
        let real_game_meshes_opt =
            if real_game_meshes.is_empty() { None } else { Some(real_game_meshes.as_slice()) };

        let use_camera_state = use_sponza || is_real_game;
        let aspect = width as f32 / height.max(1) as f32;
        let far = if use_camera_state { 500.0 } else { 100.0 };
        // Real-game mode: derive look direction + up from the actual
        // camera rotation quaternion (includes ViewRecoil kick + Lean
        // roll — see real_game_camera_rotation's own doc comment), not
        // reconstructed from yaw/pitch alone, so gun-fire recoil/lean
        // actually shakes/tilts the view like the real game — previously
        // the camera stayed perfectly steady through every shot even
        // though the gun mesh itself visibly kicked, since both are
        // baked from the same real transform data but only the gun's was
        // being read from the real rotation.
        let (look_dir, up) = if is_real_game {
            let rotation = bevy::math::Quat::from_array(self.real_game_camera_rotation);
            (
                (rotation * bevy::math::Vec3::NEG_Z).to_array(),
                (rotation * bevy::math::Vec3::Y).to_array(),
            )
        } else {
            let (sin_pitch, cos_pitch) = self.camera_pitch.sin_cos();
            let (sin_yaw, cos_yaw) = self.camera_yaw.sin_cos();
            ([-sin_yaw * cos_pitch, sin_pitch, -cos_yaw * cos_pitch], [0.0, 1.0, 0.0])
        };
        let center = [
            self.camera_position[0] + look_dir[0],
            self.camera_position[1] + look_dir[1],
            self.camera_position[2] + look_dir[2],
        ];
        let view = if use_camera_state {
            look_at(self.camera_position, center, up)
        } else {
            look_at([0.0, 3.0, 10.0], [0.0, 0.0, 0.0], [0.0, 1.0, 0.0])
        };
        let proj = perspective(if use_camera_state { 60.0_f32 } else { 45.0 }.to_radians(), aspect, 0.05, far);
        // The scene's real PointLight (see LightUniform's own doc comment)
        // — falls back to a fixed distant "sun" only for the toy demo/
        // patrol probe world, which has no ECS lights at all.
        let light = self.real_game_light;
        let view_projection = mat_mul(proj, view);
        // Scope camera: reads the REAL ScopeCamera entity's own state
        // (see ScopeCameraSnapshot's doc comment on native_game_extract.rs)
        // instead of reusing the main camera's transform + a hand-picked
        // FOV/threshold — `sync_scope_camera`/`update_scope_lens`
        // (crates/client/src/lib.rs) already compute the exact eye
        // position (with shoulder-swap sweep, recoil kick, reload dip,
        // walk-bob all baked in), `Camera.is_active`, and `Projection.fov`
        // every tick, unmodified, inside this same headless App — this is
        // just extracting that result, the same way the real desktop
        // build already shows it working. The demo probe (not real-game
        // mode) keeps its old always-on fixed-12°-FOV PIP reusing the main
        // camera's transform, since it has no real ScopeCamera to read.
        let show_scope = if is_real_game { self.real_game_scope_active } else { true };
        let (scope_view, scope_fov) = if is_real_game {
            let rotation = bevy::math::Quat::from_array(self.real_game_scope_rotation);
            let scope_forward = (rotation * bevy::math::Vec3::NEG_Z).to_array();
            let scope_up = (rotation * bevy::math::Vec3::Y).to_array();
            let scope_center = [
                self.real_game_scope_eye[0] + scope_forward[0],
                self.real_game_scope_eye[1] + scope_forward[1],
                self.real_game_scope_eye[2] + scope_forward[2],
            ];
            (look_at(self.real_game_scope_eye, scope_center, scope_up), self.real_game_scope_fov)
        } else {
            (view, 12.0_f32.to_radians())
        };
        let scope_proj = perspective(scope_fov, 1.0, 0.05, far);
        let scope_view_projection = mat_mul(scope_proj, scope_view);

        // Tick the ECS world once (advances Time, runs patrol_system over
        // every Patrol+Transform entity), then extract — the actual
        // "extract" seam the plan calls for, decoupled from bevy_render
        // entirely. Collected into an owned Vec (not a live query/World
        // borrow) so draw_scene can be called twice — once per camera —
        // without re-ticking Time in between.
        // In real-game mode the toy patrol World is irrelevant (its
        // hardcoded zombies/soldier would just float inside the real
        // arena) — skip ticking/extracting it and draw the real
        // zombies/remote players collected above instead.
        let extracted: Vec<(bevy::transform::components::Transform, EntityKind, (usize, f32))> = if is_real_game {
            real_game_extracted
        } else {
            self.ecs_app.update();
            let mut query_state = self
                .ecs_app
                .world_mut()
                .query::<(&bevy::transform::components::Transform, &EntityKind, &AnimOffset)>();
            query_state
                .iter(self.ecs_app.world())
                .map(|(t, k, a)| {
                    // No real AnimationPlayer to read for these hand-placed
                    // patrol dummies (see AnimOffset's own doc comment) —
                    // just keep them walking, staggered by AnimOffset's old
                    // per-entity phase, same visual as before this change.
                    let clip_index = match k {
                        EntityKind::Zombie => crate::ZOMBIE_ANIM_WALK,
                        EntityKind::Soldier => crate::PLAYER_MODEL_ANIM_WALK,
                    };
                    (*t, *k, (clip_index, now + a.0))
                })
                .collect()
        };

        // Pass 1: scope camera into the offscreen texture — skipped
        // entirely in real-game mode while not aiming (see `show_scope`),
        // both to avoid the wasted render pass and so the PIP quad below
        // doesn't composite a stale/irrelevant frame over the main view.
        if show_scope {
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
            self.draw_scene(
                scope_encoder,
                scope_view_projection,
                light,
                &extracted,
                now,
                use_sponza,
                real_game_meshes_opt,
            );
            scope_encoder.end_encoding();
            scope_command_buffer.commit();
        }

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
        self.draw_scene(
            encoder,
            view_projection,
            light,
            &extracted,
            now,
            use_sponza,
            real_game_meshes_opt,
        );

        if show_scope {
            if let Some(quad_pipeline) = &self.quad_pipeline {
                // Real-game mode: a circle dead center (matches the real
                // lens quad's screen position — see build_centered_quad's
                // own doc comment); the demo probe keeps its original
                // fixed top-right box, which has no real "is this actually
                // where the lens sits" state to match.
                let real_game_quad_buffer =
                    is_real_game.then(|| build_centered_quad(&self.device, aspect, 0.35));
                let quad_buffer = real_game_quad_buffer.as_ref().unwrap_or(&self.quad_vertex_buffer);
                encoder.set_render_pipeline_state(quad_pipeline);
                encoder.set_vertex_buffer(0, Some(quad_buffer), 0);
                encoder.set_fragment_texture(0, Some(&self.scope_color_texture));
                encoder.set_fragment_sampler_state(0, Some(&self.sponza_sampler));
                encoder.draw_primitives(MTLPrimitiveType::Triangle, 0, 6);
            }
        }

        encoder.end_encoding();
        command_buffer.present_drawable(drawable);
        command_buffer.commit();
    }
}

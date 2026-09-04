//! Rust-side control experiment, mirroring the Swift native-Metal demo
//! (see ViewController.swift's USE_NATIVE_METAL_DEMO): draws the same
//! 5-cuboid arena directly via the `metal` crate (classic
//! MTLCommandQueue/MTLRenderCommandEncoder, no bevy_render, no wgpu) to
//! check whether writing the renderer in Rust (instead of Swift) brings
//! wgpu's "high CPU encoding cost" overhead back, or whether it's specific
//! to wgpu's abstraction layer regardless of language. Entirely
//! self-contained — no Bevy App involved at all, since the question this
//! answers doesn't need ECS.

use bevy::asset::AssetId;
use bevy::image::Image;
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
use super::metal_math::{look_at, mat_mul, orthographic, perspective, transform_point};
use super::native_game_app::build_headless_game_ecs_app;
use super::native_game_extract::{
    extract_render_snapshot, prime_camera_viewport, ExtractedMesh, FloatingDamageSnapshot, HealthBarSnapshot,
    LightSnapshot, SunSnapshot,
};

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

// Rust has no direct equivalent of Swift's `Bundle.main.resourcePath` —
// reach into Foundation via objc the same way the rest of this module
// already talks to UIKit/CAMetalLayer.
//
// pub(crate): also called from lib.rs's load_character_ron_by_dir (real
// device only — see that fn's own doc comment for the crash this fixed:
// character.ron was being read via a CARGO_MANIFEST_DIR-relative path
// that only resolves on the dev machine that built the app, not a real
// device's sandboxed bundle).
pub(crate) fn bundle_resource_path() -> Option<String> {
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

const SPONZA_SHADER_SOURCE: &str = r#"
#include <metal_stdlib>
using namespace metal;

struct Vertex {
    packed_float3 position;
    packed_float3 normal;
    packed_float2 uv;
    // xyz = tangent direction, w = handedness — see GltfVertex's own doc
    // comment (gltf_scene.rs) for where this gets baked (CPU-side, world
    // space, same as position/normal above).
    packed_float4 tangent;
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
    float4 emissive;
    // Shadow-mapping/specular pass (see SHADOW_SHADER_SOURCE's own doc
    // comment) — lightViewProjection/pointLightViewProjection re-project
    // worldPos into the sun's and the point light's own shadow maps to
    // sample occlusion; cameraPos is needed to reconstruct a view
    // direction per-fragment for the Blinn-Phong specular term, since this
    // shader has no other way to know where the eye is.
    float4x4 lightViewProjection;
    float4x4 pointLightViewProjection;
    packed_float3 cameraPos;
    float _cameraPad;
};

struct VertexOut {
    float4 position [[position]];
    float3 worldPos;
    float3 normal;
    float2 uv;
    float4 tangent;
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
    out.tangent = float4(v.tangent);
    return out;
}

// Shininess exponent for the Blinn-Phong specular term — one fixed value
// for every material (this renderer has no per-material roughness/
// metallic map to drive it from, unlike bevy_render's real PBR path).
// Was 48 initially, which looked fine on the zombie's more irregular
// low-poly normals but washed the gun's large flat receiver faces into a
// smooth gray gradient wide enough to bury its own brushed-metal albedo
// texture underneath — those faces interpolate one near-constant normal
// across a much bigger area than the zombie's, so the same "reasonably
// tight" exponent produced a much wider hotspot there. 90 narrows the
// falloff enough to stay a small glint even on a big flat face.
constant float kShininess = 90.0;
// Specular intensity multiplier — see kShininess's own comment for why
// this also came down (0.6 -> 0.35): a wide AND bright highlight is what
// was burying the gun's texture detail, not just the width alone.
constant float kSpecularStrength = 0.35;

// Reads `shadowMap` (already-normalized-to-texture-space `lightViewProjection`)
// at `worldPos`, returning 1.0 (fully lit) .. 0.0 (fully shadowed) via
// Metal's hardware `sample_compare` (free bilinear = 2x2 PCF). Shared by
// both the sun and point-light shadow lookups below — same convention,
// different light_view_projection/texture per call.
static inline float sample_shadow(float4x4 light_view_projection, float3 worldPos, depth2d<float> shadowMap, sampler shadowSampler) {
    float4 lightClip = light_view_projection * float4(worldPos, 1.0);
    float3 lightNdc = lightClip.xyz / lightClip.w;
    float2 shadowUv = lightNdc.xy * 0.5 + 0.5;
    shadowUv.y = 1.0 - shadowUv.y;
    // A small constant bias (subtracted so a fragment reads slightly
    // nearer-to-light than it actually is) avoids shadow acne — self-
    // shadowing false positives from the shadow map's own limited
    // resolution. Doubled vs. a typical [0,1]-range bias since this
    // renderer's depth range spans [-1,1] (twice as wide).
    float shadowBias = 0.004;
    if (shadowUv.x < 0.0 || shadowUv.x > 1.0 || shadowUv.y < 0.0 || shadowUv.y > 1.0 || lightNdc.z > 1.0) {
        return 1.0;
    }
    return shadowMap.sample_compare(shadowSampler, shadowUv, lightNdc.z - shadowBias);
}

fragment float4 sponza_fragment(VertexOut in [[stage_in]],
                                 constant Uniforms &uniforms [[buffer(1)]],
                                 texture2d<float> baseColorTexture [[texture(0)]],
                                 sampler texSampler [[sampler(0)]],
                                 depth2d<float> shadowMap [[texture(1)]],
                                 sampler shadowSampler [[sampler(1)]],
                                 depth2d<float> pointShadowMap [[texture(2)]],
                                 texture2d<float> normalTexture [[texture(3)]]) {
    // Geometric (per-vertex-interpolated) normal, reoriented by the
    // sampled normal map below when this mesh actually has one — see
    // ExtractedMesh::normal_texture's own doc comment (native_game_
    // extract.rs) for why the AK47's receiver looked flat/textureless
    // without this: its "brushed metal" scratches are a lighting response
    // to a normal map, not a base-color pattern, and this renderer never
    // read normal maps at all before now. normalTexture is always bound
    // to SOMETHING (see flat_normal_texture's own doc comment) — a flat
    // "pointing straight up in tangent space" 1x1 fallback for meshes
    // with no real one, so this math runs unconditionally instead of
    // branching on an extra "has a normal map" uniform flag.
    float3 geometricNormal = normalize(in.normal);
    float3 tangent = normalize(in.tangent.xyz - geometricNormal * dot(in.tangent.xyz, geometricNormal));
    float3 bitangent = cross(geometricNormal, tangent) * in.tangent.w;
    float3x3 TBN = float3x3(tangent, bitangent, geometricNormal);
    float3 sampledNormal = normalTexture.sample(texSampler, in.uv).xyz * 2.0 - 1.0;
    float3 normal = normalize(TBN * sampledNormal);

    float3 viewDir = normalize(float3(uniforms.cameraPos) - in.worldPos);
    float3 toSun = -normalize(float3(uniforms.sunDir));

    // See SHADOW_SHADER_SOURCE's own doc comment for how these were
    // written — same lightViewProjection convention used to read them back
    // keeps the comparison self-consistent regardless of Metal's "native"
    // depth range (this renderer never uses it, see metal_math::
    // perspective's own doc comment). Both the sun (parallel rays, hence
    // an orthographic lightViewProjection) and the arena's overhead point
    // light (a real position, hence a perspective one — see render_frame's
    // "Pass 0b") cast real shadows here; bevy_render's own PBR path does
    // too for both, which is what an earlier version of this shader was
    // missing (only the sun was covered) — visible as a zombie/pillar
    // having two *different* real shadows on desktop that this renderer
    // only drew one of.
    float sunShadow = sample_shadow(uniforms.lightViewProjection, in.worldPos, shadowMap, shadowSampler);
    float pointShadowFactor = sample_shadow(uniforms.pointLightViewProjection, in.worldPos, pointShadowMap, shadowSampler);

    float sunDiffuse = max(dot(normal, toSun), 0.0) * sunShadow;
    float sunSpecular = sunDiffuse > 0.0 ? pow(max(dot(normal, normalize(toSun + viewDir)), 0.0), kShininess) * sunShadow : 0.0;

    float3 toLight = float3(uniforms.lightPos) - in.worldPos;
    float dist = length(toLight);
    float atten = clamp(1.0 - dist / max(uniforms.lightRange, 0.0001), 0.0, 1.0);
    atten *= atten;
    float3 pointDir = toLight / max(dist, 0.0001);
    float pointDiffuse = max(dot(normal, pointDir), 0.0) * atten * pointShadowFactor;
    float pointSpecular = pointDiffuse > 0.0 ? pow(max(dot(normal, normalize(pointDir + viewDir)), 0.0), kShininess) * atten * pointShadowFactor : 0.0;

    // Diffuse modulates the surface's own base color (as before); specular
    // is intentionally kept SEPARATE and added on top unmultiplied by
    // baseColor — a real (even non-PBR) specular highlight is the light's
    // own color reflecting off the surface, not tinted by its albedo, and
    // folding it into the same term that gets multiplied by baseColor.rgb
    // below crushed it to near-invisible on dark surfaces (the gun's black
    // metal) even though the math upstream was already computing it
    // correctly — this was the actual bug, not a missing specular term.
    float3 diffuseTerm = clamp(uniforms.sunColor.rgb * sunDiffuse + uniforms.lightColor.rgb * pointDiffuse, 0.0, 1.0);
    float3 specularTerm = (uniforms.sunColor.rgb * sunSpecular + uniforms.lightColor.rgb * pointSpecular) * kSpecularStrength;

    float4 texColor = baseColorTexture.sample(texSampler, in.uv);
    float4 baseColor = texColor * uniforms.baseColorFactor;
    float3 lit = baseColor.rgb * (0.35 + 0.65 * diffuseTerm) + specularTerm;
    // Additive — no real Bloom pass exists in this renderer (see
    // ExtractedMesh::emissive's own doc comment for why these values are
    // authored well above 1.0), but adding the raw emissive term straight
    // onto the lit result at least makes muzzle flash/reticle/sparks read
    // as bright/glowing instead of flat, matching bevy_render's own
    // emissive-is-additive-to-lit-color behavior minus the actual bloom
    // blur/threshold pass.
    lit += uniforms.emissive.rgb;
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
    emissive: [f32; 4],
    light_view_projection: [[f32; 4]; 4],
    point_light_view_projection: [[f32; 4]; 4],
    camera_pos: [f32; 3],
    _camera_pad: f32,
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
    // Shadow-mapping/specular pass — see SPONZA_SHADER_SOURCE's own
    // Uniforms doc comment, identical reasoning applies here.
    float4x4 lightViewProjection;
    float4x4 pointLightViewProjection;
    packed_float3 cameraPos;
    float _cameraPad;
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

// Same fixed shininess/specular-strength constants as SPONZA_SHADER_SOURCE
// — duplicated, not shared, since each shader source string in this file
// compiles as its own independent Metal library.
constant float kShininess = 48.0;
constant float kSpecularStrength = 0.6;

// Same shadow-sampling helper as SPONZA_SHADER_SOURCE — duplicated, not
// shared, since each shader source string in this file compiles as its
// own independent Metal library (see kShininess/kSpecularStrength's own
// "duplicated, not shared" comment above).
static inline float sample_shadow(float4x4 light_view_projection, float3 worldPos, depth2d<float> shadowMap, sampler shadowSampler) {
    float4 lightClip = light_view_projection * float4(worldPos, 1.0);
    float3 lightNdc = lightClip.xyz / lightClip.w;
    float2 shadowUv = lightNdc.xy * 0.5 + 0.5;
    shadowUv.y = 1.0 - shadowUv.y;
    float shadowBias = 0.004;
    if (shadowUv.x < 0.0 || shadowUv.x > 1.0 || shadowUv.y < 0.0 || shadowUv.y > 1.0 || lightNdc.z > 1.0) {
        return 1.0;
    }
    return shadowMap.sample_compare(shadowSampler, shadowUv, lightNdc.z - shadowBias);
}

fragment float4 skinned_fragment(VertexOut in [[stage_in]],
                                  constant Uniforms &uniforms [[buffer(1)]],
                                  texture2d<float> baseColorTexture [[texture(0)]],
                                  sampler texSampler [[sampler(0)]],
                                  depth2d<float> shadowMap [[texture(1)]],
                                  sampler shadowSampler [[sampler(1)]],
                                  depth2d<float> pointShadowMap [[texture(2)]]) {
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
    float3 normal = normalize(in.normal);
    float3 viewDir = normalize(float3(uniforms.cameraPos) - in.worldPos);
    float3 toSun = -normalize(float3(uniforms.sunDir));

    // See SHADOW_SHADER_SOURCE's own doc comment for how these were
    // written, and SPONZA_SHADER_SOURCE's matching block for why both the
    // sun AND the point light get their own real shadow here (not
    // repeated per-shader).
    float sunShadow = sample_shadow(uniforms.lightViewProjection, in.worldPos, shadowMap, shadowSampler);
    float pointShadowFactor = sample_shadow(uniforms.pointLightViewProjection, in.worldPos, pointShadowMap, shadowSampler);

    float sunDiffuse = max(dot(normal, toSun), 0.0) * sunShadow;
    float sunSpecular = sunDiffuse > 0.0 ? pow(max(dot(normal, normalize(toSun + viewDir)), 0.0), kShininess) * sunShadow : 0.0;

    float3 toLight = float3(uniforms.lightPos) - in.worldPos;
    float dist = length(toLight);
    float atten = clamp(1.0 - dist / max(uniforms.lightRange, 0.0001), 0.0, 1.0);
    atten *= atten;
    float3 pointDir = toLight / max(dist, 0.0001);
    float pointDiffuse = max(dot(normal, pointDir), 0.0) * atten * pointShadowFactor;
    float pointSpecular = pointDiffuse > 0.0 ? pow(max(dot(normal, normalize(pointDir + viewDir)), 0.0), kShininess) * atten * pointShadowFactor : 0.0;

    // See SPONZA_SHADER_SOURCE's matching block for why specular is kept
    // separate from baseColor's multiply instead of folded into the same
    // term (that crushed it to near-invisible on dark surfaces).
    float3 diffuseTerm = clamp(uniforms.sunColor.rgb * sunDiffuse + uniforms.lightColor.rgb * pointDiffuse, 0.0, 1.0);
    float3 specularTerm = (uniforms.sunColor.rgb * sunSpecular + uniforms.lightColor.rgb * pointSpecular) * kSpecularStrength;

    float4 texColor = baseColorTexture.sample(texSampler, in.uv);
    float4 baseColor = texColor * uniforms.baseColorFactor;
    float3 lit = baseColor.rgb * (0.35 + 0.65 * diffuseTerm) + specularTerm;
    return float4(lit, baseColor.a);
}
"#;

// Sun-only shadow map: renders every shadow-casting mesh (arena walls/
// floor/gun via shadow_vertex_sponza, zombies/remote players via
// shadow_vertex_skinned) from the light's own point of view into a
// depth-only render target — see NativeMetalProbe::shadow_map's own doc
// comment for the texture itself, and render_frame's "Pass 0" for how the
// resulting `lightViewProjection` matrix is built (an orthographic box
// centered on the camera, since a directional light has no single
// position/frustum of its own). No fragment function at all — a depth-
// only pass only needs position, and Metal explicitly supports a
// fragmentFunction-less pipeline for exactly this (`set_fragment_function
// (None)`, see this shader's two RenderPipelineDescriptors in `new()`).
// Position math is otherwise a stripped copy of sponza_vertex/
// skinned_vertex (same model/skinning transforms, just projected through
// the light's viewProjection instead of the camera's).
const SHADOW_SHADER_SOURCE: &str = r#"
#include <metal_stdlib>
using namespace metal;

struct SponzaVertex {
    packed_float3 position;
    packed_float3 normal;
    packed_float2 uv;
    // Unused here (this pass only needs position) but MUST stay present —
    // this struct's buffer(0) is the exact same GltfVertex-laid-out buffer
    // SPONZA_SHADER_SOURCE's own Vertex reads (bake_static_mesh_vertices
    // builds one buffer per mesh, shared by both the shadow and main
    // passes — see draw_shadow_casters/draw_scene). Metal computes this
    // struct's own stride from ITS OWN size to index verts[vid], so
    // dropping this field would silently read every vertex 16 bytes short
    // — position/normal/uv would land right, but every vertex AFTER the
    // first would already be reading from the wrong offset.
    packed_float4 tangent;
};

struct SponzaShadowUniforms {
    float4x4 lightViewProjection;
};

vertex float4 shadow_vertex_sponza(uint vid [[vertex_id]],
                                    const device SponzaVertex *verts [[buffer(0)]],
                                    constant SponzaShadowUniforms &uniforms [[buffer(1)]]) {
    SponzaVertex v = verts[vid];
    return uniforms.lightViewProjection * float4(float3(v.position), 1.0);
}

struct SkinnedVertex {
    packed_float3 position;
    packed_float3 normal;
    packed_float2 uv;
    ushort4 joints;
    packed_float4 weights;
};

struct SkinnedShadowUniforms {
    float4x4 lightViewProjection;
    float4x4 modelMatrix;
};

vertex float4 shadow_vertex_skinned(uint vid [[vertex_id]],
                                     const device SkinnedVertex *verts [[buffer(0)]],
                                     constant SkinnedShadowUniforms &uniforms [[buffer(1)]],
                                     const device float4x4 *jointMatrices [[buffer(2)]]) {
    SkinnedVertex v = verts[vid];
    float4x4 skin =
        v.weights.x * jointMatrices[v.joints.x] +
        v.weights.y * jointMatrices[v.joints.y] +
        v.weights.z * jointMatrices[v.joints.z] +
        v.weights.w * jointMatrices[v.joints.w];
    float4 skinnedPos = skin * float4(float3(v.position), 1.0);
    float4 worldPos = uniforms.modelMatrix * skinnedPos;
    return uniforms.lightViewProjection * worldPos;
}
"#;

#[repr(C)]
#[derive(Copy, Clone)]
struct SponzaShadowUniforms {
    light_view_projection: [[f32; 4]; 4],
}

#[repr(C)]
#[derive(Copy, Clone)]
struct SkinnedShadowUniforms {
    light_view_projection: [[f32; 4]; 4],
    model_matrix: [[f32; 4]; 4],
}

// Fixed square resolution for the sun's shadow map — no cascades, no
// resize-on-demand (unlike msaa_targets' screen-sized depth buffer): this
// renderer's whole shadow frustum is a fixed-size box recentered on the
// camera every frame (see render_frame's "Pass 0"), so its texture
// resolution has nothing to do with screen size. 2048 is generous for a
// single non-cascaded map covering a ~44-unit box — soft enough at the
// PCF-filtered edges (see SPONZA_SHADER_SOURCE's shadowBias) without the
// blocky look a smaller map would show up close.
const SHADOW_MAP_SIZE: u64 = 2048;
// Half-extent (world units) of the shadow frustum's ortho box, and how far
// back along the light direction its "eye" sits — see render_frame's
// "Pass 0" for the actual light_view_projection construction. Centered on
// the camera every frame rather than fit to the whole arena, so resolution
// stays concentrated on what's actually visible instead of being spread
// across geometry far outside the current view.
const SHADOW_HALF_EXTENT: f32 = 22.0;
const SHADOW_DISTANCE: f32 = 40.0;

// Phase 3's other half: a second camera (narrow FOV, same
// position/orientation as the main one — simulates a rifle scope) rendered
// into an offscreen texture, then composited as a small picture-in-picture
// quad over the main view. Same offscreen-render-target-then-present-on-a-
// quad indirection the real game's own scope camera already uses (see
// setup()'s own comment there), reimplemented against this renderer
// instead of bevy_render.
const SCOPE_TEXTURE_SIZE: u64 = 512;

// No anti-aliasing anywhere in this renderer at all — every model edge
// (zombies, walls, gun) rendered visibly jagged, unlike bevy_render's own
// Msaa::Off... wait, actually unlike this codebase's *other* rendering
// path, which defaults MSAA on (see crate::lib's Camera3d bundle — Msaa::
// Off is set there deliberately, but only because bevy_render's TAA/SSAO
// stack made MSAA redundant on that path; this renderer has neither, so
// there was nothing softening edges at all). 4x MSAA — the standard
// quality/cost tradeoff — applied uniformly to every pipeline used by
// either the main pass or the scope PIP pass, so a single compiled
// RenderPipelineState per shader still works in both (Metal requires a
// pipeline's rasterSampleCount to match whatever render pass it's bound
// to; keeping sample count uniform everywhere avoids needing two compiled
// variants of every shader).
const MSAA_SAMPLE_COUNT: u64 = 4;

const QUAD_SHADER_SOURCE: &str = r#"
#include <metal_stdlib>
using namespace metal;

struct QuadVertex {
    packed_float4 position;
    packed_float2 uv;
};

struct QuadVertexOut {
    float4 position [[position]];
    float2 uv;
};

// `position` is already real homogeneous clip space (not NDC) — see this
// shader's own Rust-side doc comment (build_projected_quad) for why: the
// real-game scope PIP now projects each of the lens quad's 4 real 3D
// world-space corners individually (full rotation + perspective), instead
// of building a flat always-facing-camera NDC quad and faking rotation/
// distance with 2D tricks, so each vertex needs its own `w` for Metal's
// rasterizer to interpolate `uv` perspective-correctly across the quad —
// passing it straight through here (not wrapping z=0/w=1 like before) is
// what makes that work. The demo-probe's still-flat fixed quad just
// carries z=0/w=1 baked into its own vertex data instead.
vertex QuadVertexOut quad_vertex(uint vid [[vertex_id]],
                                  const device QuadVertex *verts [[buffer(0)]]) {
    QuadVertexOut out;
    QuadVertex v = verts[vid];
    out.position = v.position;
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
    // already drew showing through.
    float2 centered = in.uv - float2(0.5, 0.5);
    if (length(centered) > 0.5) {
        discard_fragment();
    }
    float4 color = tex.sample(texSampler, in.uv);
    // Real game's ScopeLens is AlphaMode::Blend glass (see
    // build_sponza_descriptor's own comment) — the world behind the gun
    // still shows through it faintly, unlike a fully opaque disk. This
    // quad is drawn last, straight over the already-finished main-camera
    // frame (see render_frame), so blending it at less than full alpha
    // here lets that same "world already drawn underneath" bleed through
    // the scope picture the same way, instead of painting over it
    // completely opaque like before.
    color.a = 0.88;
    return color;
}
"#;

#[repr(C)]
#[derive(Copy, Clone)]
struct QuadVertex {
    position: [f32; 4],
    uv: [f32; 2],
}

// bevy_ui was never drawn by this renderer at all (it draws its own scene
// straight via the `metal` crate, never touching bevy_render's UI render
// graph) — zombie health bars and floating damage numbers looked simply
// "not ported" but were actually never screen-projected in the first place
// (see prime_camera_viewport's doc comment in native_game_extract.rs).
// Once that's fixed, this is the other half: actually draw them. Reuses
// the same "position/uv quad straight into NDC, no view/projection" shape
// as QUAD_SHADER_SOURCE's scope PIP above, plus a per-draw tint uniform so
// one pipeline covers both flat-colored rects (health bar background/fill,
// sampling `white_texture` so `texColor` is always (1,1,1,1) and the
// result is just `tint`) and glyph quads (sampling `font_atlas_texture`,
// whose texels are (1,1,1,coverage) — see build_font_atlas — so the result
// is `tint` masked by per-pixel glyph coverage).
const UI_SHADER_SOURCE: &str = r#"
#include <metal_stdlib>
using namespace metal;

struct UiVertex {
    packed_float2 position;
    packed_float2 uv;
};

struct UiVertexOut {
    float4 position [[position]];
    float2 uv;
};

struct UiUniforms {
    float4 tint;
};

vertex UiVertexOut ui_vertex(uint vid [[vertex_id]],
                              const device UiVertex *verts [[buffer(0)]]) {
    UiVertexOut out;
    UiVertex v = verts[vid];
    out.position = float4(v.position, 0.0, 1.0);
    out.uv = v.uv;
    return out;
}

fragment float4 ui_fragment(UiVertexOut in [[stage_in]],
                             constant UiUniforms &uniforms [[buffer(1)]],
                             texture2d<float> tex [[texture(0)]],
                             sampler texSampler [[sampler(0)]]) {
    float4 texColor = tex.sample(texSampler, in.uv);
    return texColor * uniforms.tint;
}
"#;

#[repr(C)]
#[derive(Copy, Clone)]
struct UiVertex {
    position: [f32; 2],
    uv: [f32; 2],
}

#[repr(C)]
#[derive(Copy, Clone)]
struct UiUniforms {
    tint: [f32; 4],
}

// A tiny built-in 5x7 dot-matrix font — just enough glyphs for floating
// damage numbers ("12.0") and health bar labels ("6.0/8"): digits, '.',
// and '/'. Generated procedurally into a small RGBA8 atlas texture at
// startup (build_font_atlas) instead of shipping a font asset/using
// CoreText, since this renderer already draws everything else via raw
// vertex data with no asset pipeline of its own. One row, GLYPH_COUNT
// cells wide, each cell GLYPH_W x GLYPH_H texels — see glyph_index for the
// character -> cell mapping and glyph_uv_rect for the per-glyph UV rect.
const GLYPH_W: usize = 5;
const GLYPH_H: usize = 7;
const GLYPH_COUNT: usize = 43;
// Row-major bits, MSB (bit 4) = leftmost pixel, 7 rows per glyph, in
// glyph_index order: '0'..'9', '.', '/', then the full A-Z uppercase
// alphabet (added incrementally as touch-control/HUD/settings-menu labels
// needed more of it — G/':' for HP/ammo/ping, then B/F/K/Q/V/W/X/Y/Z once
// the settings menu's own labels — HITBOXES, FREEZE, SHADOW, ... — showed
// missing letters simply get skipped by glyph_index, silently compacting
// the text instead of erroring), plus a handful of symbols (<, >, =).
const GLYPH_BITMAPS: [[u8; GLYPH_H]; GLYPH_COUNT] = [
    [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110], // 0
    [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110], // 1
    [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111], // 2
    [0b11111, 0b00010, 0b00100, 0b00010, 0b00001, 0b10001, 0b01110], // 3
    [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010], // 4
    [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110], // 5
    [0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110], // 6
    [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000], // 7
    [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110], // 8
    [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100], // 9
    [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b01100, 0b01100], // .
    [0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b00000, 0b00000], // /
    [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001], // A
    [0b01111, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b01111], // C
    [0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110], // D
    [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111], // E
    [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001], // H
    [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110], // I
    [0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100], // J
    [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111], // L
    [0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001], // M
    [0b10001, 0b11001, 0b10101, 0b10101, 0b10011, 0b10001, 0b10001], // N
    [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110], // O
    [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000], // P
    [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001], // R
    [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110], // S
    [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100], // T
    [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110], // U
    [0b00001, 0b00010, 0b00100, 0b01000, 0b00100, 0b00010, 0b00001], // <
    [0b10000, 0b01000, 0b00100, 0b00010, 0b00100, 0b01000, 0b10000], // >
    [0b00000, 0b00000, 0b11111, 0b00000, 0b11111, 0b00000, 0b00000], // =
    [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000], // space
    [0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111], // G
    [0b00000, 0b00000, 0b01100, 0b01100, 0b00000, 0b01100, 0b01100], // :
    [0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110], // B
    [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000], // F
    [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001], // K
    [0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101], // Q
    [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100], // V
    [0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b10101, 0b01010], // W
    [0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001], // X
    [0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100], // Y
    [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111], // Z
];

fn glyph_index(c: char) -> Option<usize> {
    match c {
        '0'..='9' => Some(c as usize - '0' as usize),
        '.' => Some(10),
        '/' => Some(11),
        'A' => Some(12),
        'C' => Some(13),
        'D' => Some(14),
        'E' => Some(15),
        'H' => Some(16),
        'I' => Some(17),
        'J' => Some(18),
        'L' => Some(19),
        'M' => Some(20),
        'N' => Some(21),
        'O' => Some(22),
        'P' => Some(23),
        'R' => Some(24),
        'S' => Some(25),
        'T' => Some(26),
        'U' => Some(27),
        '<' => Some(28),
        '>' => Some(29),
        '=' => Some(30),
        ' ' => Some(31),
        'G' => Some(32),
        ':' => Some(33),
        'B' => Some(34),
        'F' => Some(35),
        'K' => Some(36),
        'Q' => Some(37),
        'V' => Some(38),
        'W' => Some(39),
        'X' => Some(40),
        'Y' => Some(41),
        'Z' => Some(42),
        _ => None,
    }
}

// Cell N occupies texels [N*GLYPH_W, (N+1)*GLYPH_W) horizontally, the full
// GLYPH_H rows vertically — see build_font_atlas for how the texture
// itself is laid out.
fn glyph_uv_rect(index: usize) -> (f32, f32) {
    let u0 = index as f32 / GLYPH_COUNT as f32;
    let u1 = (index + 1) as f32 / GLYPH_COUNT as f32;
    (u0, u1)
}

// One-time RGBA8 atlas build: GLYPH_COUNT cells side by side, each
// GLYPH_W x GLYPH_H texels. Lit pixels are opaque white (1,1,1,1), unlit
// pixels fully transparent (0,0,0,0) — see UI_SHADER_SOURCE's own doc
// comment for why that makes `texColor * tint` do the right thing for
// both flat rects (sampling `white_texture` instead) and glyphs.
fn build_font_atlas(device: &Device) -> metal::Texture {
    let width = (GLYPH_W * GLYPH_COUNT) as u64;
    let height = GLYPH_H as u64;
    let mut pixels = vec![0u8; (width * height * 4) as usize];
    for (index, bitmap) in GLYPH_BITMAPS.iter().enumerate() {
        for (row, bits) in bitmap.iter().enumerate() {
            for col in 0..GLYPH_W {
                let lit = (bits >> (GLYPH_W - 1 - col)) & 1 != 0;
                let x = index * GLYPH_W + col;
                let y = row;
                let offset = (y * width as usize + x) * 4;
                let value = if lit { 255 } else { 0 };
                pixels[offset..offset + 4].copy_from_slice(&[value, value, value, value]);
            }
        }
    }
    let descriptor = metal::TextureDescriptor::new();
    descriptor.set_pixel_format(MTLPixelFormat::RGBA8Unorm);
    descriptor.set_width(width);
    descriptor.set_height(height);
    descriptor.set_storage_mode(metal::MTLStorageMode::Shared);
    let texture = device.new_texture(&descriptor);
    texture.replace_region(metal::MTLRegion::new_2d(0, 0, width, height), 0, pixels.as_ptr() as *const _, width * 4);
    texture
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
    light_view_projection: [[f32; 4]; 4],
    point_light_view_projection: [[f32; 4]; 4],
    camera_pos: [f32; 3],
    _camera_pad: f32,
}

pub struct NativeMetalProbe {
    device: Device,
    layer: MetalLayer,
    queue: CommandQueue,
    depth_state: metal::DepthStencilState,
    // Less-compare, write-DISABLED depth-stencil state for draw_scene's
    // blend pass — see its own construction comment (in `new()`).
    blend_depth_state: metal::DepthStencilState,
    // Always-pass, no-write depth-stencil state for the bevy_ui overlay —
    // see its own construction comment (in `new()`) for why the overlay
    // can't just inherit `depth_state` above.
    ui_depth_state: metal::DepthStencilState,
    // Main pass's depth/color render targets — see metal_msaa.rs's own doc
    // comment. The color one is resolved into the real drawable texture
    // each frame (MTLStoreAction::MultisampleResolve), never sampled
    // directly.
    msaa_targets: super::metal_msaa::MsaaTargets,
    // This frame's MapMsaaOn value (native_game_extract.rs), read each
    // frame the real game App ticks — decides which pipeline/texture twin
    // above render_frame binds. Defaults matching MapMsaaOn's own Default
    // (false/off) until the first extraction.
    msaa_enabled: bool,
    sponza_pipeline: Option<RenderPipelineState>,
    sponza_pipeline_no_msaa: Option<RenderPipelineState>,
    sponza_sampler: metal::SamplerState,
    // Sun shadow map — see SHADOW_SHADER_SOURCE's own doc comment.
    // Depth-only, fixed SHADOW_MAP_SIZE resolution, no MSAA twin needed
    // (never multisampled — shadow maps are sampled with PCF instead, see
    // shadow_sampler below).
    shadow_map: metal::Texture,
    // The arena's overhead PointLight's own shadow map — same shape/
    // pipelines as shadow_map above (render_frame's "Pass 0b" just runs
    // draw_shadow_casters a second time with a different light_view_
    // projection into this texture instead), kept as a separate texture
    // rather than a second layer/slice since Metal render targets are
    // simplest as whole textures. Without this, only the sun casts a
    // shadow — a real difference from bevy_render's own PBR path (which
    // shadows both), visible as one real shadow direction on this
    // renderer vs. two distinct overlapping ones on desktop.
    shadow_map_point: metal::Texture,
    shadow_pipeline_sponza: Option<RenderPipelineState>,
    shadow_pipeline_skinned: Option<RenderPipelineState>,
    // Depth-only state for Pass 0 (see render_frame) — write-enabled,
    // Less-compare, same convention as `depth_state` below (nearer wins);
    // a dedicated field only because `depth_state`'s pixel format is tied
    // to the main pass's own depth attachment format at creation time on
    // some platforms, and keeping them distinct avoids ever having to
    // reason about whether reusing one for the other is safe.
    shadow_depth_state: metal::DepthStencilState,
    // Comparison sampler — `set_compare_function(LessEqual)` makes
    // `shadowMap.sample_compare(...)` in the fragment shaders return
    // hardware-filtered (free bilinear, effectively a 2x2 PCF tap) shadow
    // visibility directly instead of a raw depth value to compare by hand.
    shadow_sampler: metal::SamplerState,
    white_texture: metal::Texture,
    // Fallback for meshes with no real normal map (most of them — see
    // ExtractedMesh::normal_texture's own doc comment) — a single tangent-
    // space "pointing straight up" texel (RGB 128,128,255 decodes to
    // roughly (0,0,1) once sponza_fragment's `* 2.0 - 1.0` unpacks it),
    // which leaves the TBN-transformed normal equal to the untouched
    // geometric normal. Same "always bind something, skip the branch"
    // reasoning as white_texture above.
    flat_normal_texture: metal::Texture,
    camera_position: [f32; 3],
    camera_yaw: f32,
    camera_pitch: f32,
    skinned_pipeline: Option<RenderPipelineState>,
    skinned_pipeline_no_msaa: Option<RenderPipelineState>,
    // See skinned_actor.rs's own doc comment — this project's zombie/soldier
    // models are just two entries registered here, not a special case the
    // renderer core knows about.
    skinned_models: super::skinned_actor::SkinnedModelRegistry,
    scope_color_texture: metal::Texture,
    // Multisampled scope-pass render target, resolved into
    // scope_color_texture above — see its own doc comment at construction.
    scope_color_texture_msaa: metal::Texture,
    scope_depth_texture: metal::Texture,
    // Sample-count-1 twin of scope_depth_texture, used when MapMsaaOn is
    // off.
    scope_depth_texture_no_msaa: metal::Texture,
    quad_pipeline: Option<RenderPipelineState>,
    quad_pipeline_no_msaa: Option<RenderPipelineState>,
    quad_vertex_buffer: metal::Buffer,
    start_time: std::time::Instant,
    // `Some` only when constructed via `new(view, true)` — the headless
    // real game (build_game_common: physics/networking/AI/prediction),
    // ticked and extracted every frame. When
    // present, `render_frame` draws its static geometry (arena+gun) and
    // drives the camera from its `FpsCamera` instead of touch input.
    real_game_app: Option<bevy::app::App>,
    // Cache of uploaded GPU textures, keyed by the source `Image` asset's
    // own stable AssetId — GPU texture upload is far more expensive than the
    // small per-frame vertex buffer rebuild, so unlike vertex data (which
    // must be rebaked every frame since Transform can change — gun sway/
    // recoil), each unique texture is uploaded once and reused after that.
    // Used to be keyed positionally by index into
    // `RenderSnapshot::static_meshes` instead — that relied on "the same
    // entities keep existing once spawned", which held for arena walls/gun
    // meshes but broke the moment transient VFX meshes (muzzle flash,
    // smoke — both textured) started spawning/despawning on every shot: the
    // Vec's length (and which mesh sat at which index) changed constantly,
    // so a cached texture from index N one frame could silently end up
    // bound to a completely different mesh at index N the next, or trigger
    // a full re-upload of every texture on the same frame a shot fired.
    // Identity-keyed avoids both: uploads happen exactly once per unique
    // image regardless of entity churn, and a lookup can never bind the
    // wrong texture to the wrong mesh.
    real_game_textures: std::collections::HashMap<AssetId<Image>, metal::Texture>,
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
    // Real ScopeLens entity's own world position — see
    // ScopeCameraSnapshot::lens_world_pos's own doc comment
    // (native_game_extract.rs). Projected through the main camera's
    // view_projection each frame to place the scope PIP quad where the
    // real 3D lens glass actually is, instead of a fixed screen center.
    real_game_scope_lens_world_pos: [f32; 3],
    // See ScopeCameraSnapshot::lens_local_rotation's own doc comment.
    real_game_scope_lens_local_rotation: [f32; 4],
    // The scene's real PointLight, if any — see LightSnapshot's own doc
    // comment. `None` (falls back to FALLBACK_LIGHT) before the first
    // extraction, or for the toy demo/patrol probe world, which has no
    // ECS lights at all.
    real_game_light: LightUniform,
    // bevy_ui overlay: pipeline/atlas/sampler for drawing zombie health
    // bars and floating damage numbers (see UI_SHADER_SOURCE's own doc
    // comment) plus this frame's already-extracted screen-space data —
    // `None` pipeline means the shader failed to compile, same
    // fail-soft pattern sponza/skinned/quad already use.
    ui_pipeline: Option<RenderPipelineState>,
    ui_pipeline_no_msaa: Option<RenderPipelineState>,
    ui_sampler: metal::SamplerState,
    font_atlas_texture: metal::Texture,
    ui_floating_damage: Vec<FloatingDamageSnapshot>,
    ui_health_bars: Vec<HealthBarSnapshot>,
    // Touch controls (HUD buttons + joystick) — see TouchButtonSnapshot/
    // TouchJoystickSnapshot's own doc comments (native_game_extract.rs).
    // Drawn with the exact same `ui_pipeline`/white_texture/font_atlas_texture
    // as the health bars/floating damage above, just a separate cached Vec
    // so this frame's set replaces last frame's wholesale like those do.
    ui_touch_buttons: Vec<super::native_game_extract::TouchButtonSnapshot>,
    ui_touch_joystick: Option<super::native_game_extract::TouchJoystickSnapshot>,
    ui_scope_zoom_slider: Option<super::native_game_extract::ScopeZoomSliderSnapshot>,
    // HP/ammo/ping readouts + crosshair — see HudTextSnapshot/
    // CrosshairLineSnapshot's own doc comments (native_game_extract.rs).
    ui_hud_texts: Vec<super::native_game_extract::HudTextSnapshot>,
    ui_crosshair_lines: Vec<super::native_game_extract::CrosshairLineSnapshot>,
    // Settings menu — see SettingsMenuButtonSnapshot's own doc comment
    // (native_game_extract.rs).
    ui_settings_menu_panel: Option<super::native_game_extract::SettingsMenuButtonSnapshot>,
    ui_settings_menu_buttons: Vec<super::native_game_extract::SettingsMenuButtonSnapshot>,
    ui_settings_menu_texts: Vec<super::native_game_extract::HudTextSnapshot>,
    // GAME OVER overlay — see extract_panel's own doc comment
    // (native_game_extract.rs).
    ui_game_over_panel: Option<super::native_game_extract::SettingsMenuButtonSnapshot>,
    ui_game_over_buttons: Vec<super::native_game_extract::SettingsMenuButtonSnapshot>,
    ui_game_over_texts: Vec<super::native_game_extract::HudTextSnapshot>,
    // Map-select overlay — see extract_panel's own doc comment
    // (native_game_extract.rs).
    ui_map_select_panel: Option<super::native_game_extract::SettingsMenuButtonSnapshot>,
    ui_map_select_buttons: Vec<super::native_game_extract::SettingsMenuButtonSnapshot>,
    ui_map_select_texts: Vec<super::native_game_extract::HudTextSnapshot>,
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
        // Only the xyz direction rotates into world space like the normal
        // above — w is glTF's handedness sign, not a spatial component, so
        // it carries straight through unchanged.
        let tangent = if mesh.tangents.is_empty() {
            [1.0, 0.0, 0.0, 1.0]
        } else {
            let [tx, ty, tz, tw] = mesh.tangents[idx];
            let world_tangent = (mesh.transform.rotation * bevy::math::Vec3::new(tx, ty, tz)).normalize_or_zero();
            [world_tangent.x, world_tangent.y, world_tangent.z, tw]
        };
        super::gltf_scene::GltfVertex { position: world_pos.to_array(), normal: world_normal.to_array(), uv, tangent }
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
// Real-game scope PIP: instead of a flat always-facing-camera NDC quad
// with position/roll faked on top (the previous two approaches — dead
// center, then a projected center + a single 2D roll angle), this builds
// the real ScopeLens circle mesh's own 4 corners in actual WORLD space
// (`lens_world_pos` + the lens's real world rotation, at its real
// `radius`) and projects each one individually through `view_projection`,
// exactly like any other 3D quad in the scene. That's what naturally
// picks up EVERY axis of update_scope_lens's real motion for free —
// translation, roll, the recoil yaw-kick (a true horizontal turn of the
// glass, not representable by a flat 2D quad + one roll angle), reload
// tilt, AND real perspective-correct scaling as the lens sweeps toward/
// away from the camera (ADS in/out, recoil kick along its own local Z) —
// instead of the fixed apparent size every earlier version used. Passing
// each corner's own homogeneous clip-space `[x, y, z, w]` straight to the
// GPU (not pre-divided by `w` on the CPU) is what makes Metal's
// rasterizer interpolate `uv` correctly across a quad that's no longer
// perfectly fronto-parallel to the screen — see quad_vertex's own MSL
// comment. `world_rotation` is `camera_rotation * lens_local_rotation`,
// mirroring exactly how Bevy composes a ChildOf(camera) entity's own
// GlobalTransform from its parent's.
fn build_projected_quad(
    device: &Device,
    view_projection: [[f32; 4]; 4],
    center: [f32; 3],
    world_rotation: bevy::math::Quat,
    radius: f32,
) -> metal::Buffer {
    let right = (world_rotation * bevy::math::Vec3::X) * radius;
    let up = (world_rotation * bevy::math::Vec3::Y) * radius;
    let center = bevy::math::Vec3::from_array(center);
    // V=0 is the texture's TOP row (image-space convention, same as
    // ndc_ui_rect's own doc comment) — so the TOP corners (+up) must
    // carry uv.y = 0.0, not the bottom ones. Had this backwards before
    // (bottom-of-quad -> uv.y=0), which sampled scope_color_texture's top
    // row at the screen's bottom and vice versa — a vertical flip, the
    // scope PIP rendering upside down.
    let corner = |signed_right: f32, signed_up: f32, uv: [f32; 2]| {
        let world = center + right * signed_right + up * signed_up;
        QuadVertex { position: transform_point(view_projection, world.to_array()), uv }
    };
    let (tl, tr, br, bl) = (
        corner(-1.0, 1.0, [0.0, 0.0]),
        corner(1.0, 1.0, [1.0, 0.0]),
        corner(1.0, -1.0, [1.0, 1.0]),
        corner(-1.0, -1.0, [0.0, 1.0]),
    );
    let verts = [bl, br, tr, bl, tr, tl];
    device.new_buffer_with_data(
        verts.as_ptr() as *const _,
        (verts.len() * std::mem::size_of::<QuadVertex>()) as u64,
        MTLResourceOptions::StorageModeShared,
    )
}

// Screen-space rect helper for the UI overlay: `x`/`y`/`w`/`h` are
// top-left-origin logical points (matching bevy_ui's `Node.left`/`.top`,
// per prime_camera_viewport's doc comment), converted straight to Metal
// NDC (-1..1, Y up) — no view/projection needed, same "straight into clip
// space" approach build_centered_quad/the scope PIP quad already use.
// `uv` is [bottom-left, bottom-right, top-right, top-left], V increasing
// downward (image-space convention) to match `build_font_atlas`.
fn ndc_ui_rect(x: f32, y: f32, w: f32, h: f32, view_w: f32, view_h: f32, uv: [[f32; 2]; 4]) -> [UiVertex; 6] {
    let ndc_x0 = (x / view_w) * 2.0 - 1.0;
    let ndc_x1 = ((x + w) / view_w) * 2.0 - 1.0;
    let ndc_y0 = 1.0 - (y / view_h) * 2.0;
    let ndc_y1 = 1.0 - ((y + h) / view_h) * 2.0;
    [
        UiVertex { position: [ndc_x0, ndc_y1], uv: uv[0] },
        UiVertex { position: [ndc_x1, ndc_y1], uv: uv[1] },
        UiVertex { position: [ndc_x1, ndc_y0], uv: uv[2] },
        UiVertex { position: [ndc_x0, ndc_y1], uv: uv[0] },
        UiVertex { position: [ndc_x1, ndc_y0], uv: uv[2] },
        UiVertex { position: [ndc_x0, ndc_y0], uv: uv[3] },
    ]
}

// `set_vertex_bytes`/`set_fragment_bytes` instead of a real `metal::Buffer`
// — each UI quad is tiny (96 bytes of vertex data) and drawn once, so a
// full buffer allocation per quad would be pure overhead; Metal's inline
// "bytes" path exists exactly for this.
fn draw_ui_quad(encoder: &metal::RenderCommandEncoderRef, verts: &[UiVertex; 6], tint: [f32; 4], texture: &metal::Texture) {
    encoder.set_vertex_bytes(0, std::mem::size_of::<[UiVertex; 6]>() as u64, verts.as_ptr() as *const _);
    let uniforms = UiUniforms { tint };
    encoder.set_fragment_bytes(1, std::mem::size_of::<UiUniforms>() as u64, &uniforms as *const _ as *const _);
    encoder.set_fragment_texture(0, Some(texture));
    encoder.draw_primitives(MTLPrimitiveType::Triangle, 0, 6);
}

// Draws a list of HudTextSnapshot-shaped text entries (HP/ammo/ping
// readouts, and the settings menu's own text — see HudTextSnapshot's own
// doc comment in native_game_extract.rs) with nearest-edge anchoring: the
// bitmap font (fixed 5px-wide glyphs) measures the same string to a
// different width/height than Bevy's real proportional font, so only
// whichever edge sits closer to its own screen edge — the text's real
// fixed anchor — stays put; the far edge grows/shrinks by our own
// measurement instead of the real layout's, or it'd drift off-screen.
// Uppercases before drawing — the bitmap atlas only has uppercase letters
// (see glyph_index's own doc comment), while the real Text these come
// from is mixed-case ("Ping: 34 ms").
fn draw_hud_text_list(
    encoder: &metal::RenderCommandEncoderRef,
    texts: &[super::native_game_extract::HudTextSnapshot],
    view_w: f32,
    view_h: f32,
    font_atlas_texture: &metal::Texture,
) {
    for hud in texts {
        let [left, top] = hud.top_left;
        let [w, h] = hud.size;
        let (right, bottom) = (left + w, top + h);
        let scale = hud.font_size / GLYPH_H as f32;
        let upper = hud.text.to_ascii_uppercase();
        let text_w: f32 = upper.chars().map(|_| GLYPH_W as f32 * scale + scale).sum();
        let text_h = GLYPH_H as f32 * scale;
        let mut cursor_x = if left < view_w - right { left } else { right - text_w };
        let y = if top < view_h - bottom { top } else { bottom - text_h };
        for ch in upper.chars() {
            let Some(index) = glyph_index(ch) else { continue };
            let (u0, u1) = glyph_uv_rect(index);
            let uv = [[u0, 1.0], [u1, 1.0], [u1, 0.0], [u0, 0.0]];
            let glyph_w = GLYPH_W as f32 * scale;
            let glyph_h = GLYPH_H as f32 * scale;
            let quad = ndc_ui_rect(cursor_x, y, glyph_w, glyph_h, view_w, view_h, uv);
            draw_ui_quad(encoder, &quad, hud.color, font_atlas_texture);
            cursor_x += glyph_w + scale;
        }
    }
}

// Draws a "panel + buttons + text" UI subtree — shared by the settings
// menu and the GAME OVER overlay (see extract_panel's own doc comment in
// native_game_extract.rs for why both extract into the same
// SettingsMenuButtonSnapshot/HudTextSnapshot shapes). The panel's own
// backdrop is drawn first, so the real buttons/text land on top of it;
// buttons are plain rects (no label — labels are `texts`' own Text
// children, drawn last via draw_hud_text_list).
fn draw_panel(
    encoder: &metal::RenderCommandEncoderRef,
    panel: &Option<super::native_game_extract::SettingsMenuButtonSnapshot>,
    buttons: &[super::native_game_extract::SettingsMenuButtonSnapshot],
    texts: &[super::native_game_extract::HudTextSnapshot],
    view_w: f32,
    view_h: f32,
    white_texture: &metal::Texture,
    font_atlas_texture: &metal::Texture,
) {
    const FULL_UV: [[f32; 2]; 4] = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];
    if let Some(panel) = panel {
        let [x, y] = panel.top_left;
        let [w, h] = panel.size;
        let quad = ndc_ui_rect(x, y, w, h, view_w, view_h, FULL_UV);
        draw_ui_quad(encoder, &quad, panel.color, white_texture);
    }
    for button in buttons {
        let [x, y] = button.top_left;
        let [w, h] = button.size;
        let quad = ndc_ui_rect(x, y, w, h, view_w, view_h, FULL_UV);
        draw_ui_quad(encoder, &quad, button.color, white_texture);
    }
    draw_hud_text_list(encoder, texts, view_w, view_h, font_atlas_texture);
}

struct RealGameMeshDraw<'a> {
    vertex_buffer: metal::Buffer,
    vertex_count: usize,
    base_color_factor: [f32; 4],
    emissive: [f32; 4],
    texture: Option<&'a metal::Texture>,
    normal_texture: Option<&'a metal::Texture>,
    // See ExtractedMesh::is_blend's own doc comment — this is what
    // draw_scene actually splits its two static-mesh passes on now.
    is_blend: bool,
    // Squared distance from the main camera's eye to this mesh's own
    // world-space transform.translation — used to sort the blend pass
    // back-to-front (farthest first) so overlapping translucent meshes
    // composite in the right order, same as any standard forward-
    // renderer transparency pass. Squared to skip a sqrt() per mesh per
    // frame; only the relative ordering it produces is ever used.
    camera_dist_sq: f32,
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

        let compile_options = CompileOptions::new();

        let depth_descriptor = metal::DepthStencilDescriptor::new();
        depth_descriptor.set_depth_compare_function(metal::MTLCompareFunction::Less);
        depth_descriptor.set_depth_write_enabled(true);
        let depth_state = device.new_depth_stencil_state(&depth_descriptor);

        // Same Less-compare as depth_state above, but write DISABLED — for
        // draw_scene's blend pass (translucent meshes, see ExtractedMesh::
        // is_blend). Still correctly hidden behind anything nearer that's
        // already in the depth buffer (opaque geometry, zombies), but
        // doesn't itself write depth, so it can never occlude anything —
        // the standard "opaque writes, transparent only tests" split any
        // forward renderer needs for correct transparency, instead of
        // relying on draw order alone (see draw_scene's own comment on
        // why draw order alone wasn't enough for two overlapping blended
        // meshes).
        let blend_depth_descriptor = metal::DepthStencilDescriptor::new();
        blend_depth_descriptor.set_depth_compare_function(metal::MTLCompareFunction::Less);
        blend_depth_descriptor.set_depth_write_enabled(false);
        let blend_depth_state = device.new_depth_stencil_state(&blend_depth_descriptor);

        // The bevy_ui overlay (health bars, floating damage, touch controls)
        // is drawn with the SAME `encoder` as the 3D scene right before it,
        // in the SAME render pass — Metal never resets depth-stencil state
        // between draw calls, so without an explicit override here the
        // overlay inherits `depth_state` above (Less-compare, write
        // enabled) and gets depth-tested against the 3D scene's own
        // just-written depth buffer. UI_SHADER_SOURCE's vertex shader
        // always outputs z=0.0 (nearest possible in Metal's [0,1] NDC
        // range), which should in principle always beat real scene
        // geometry — but real-device testing (screenshots of the same
        // frame data taken moments apart) showed the overlay intermittently
        // NOT rendering at all depending on what 3D geometry happened to be
        // behind it, which is exactly what a marginal/borderline depth
        // comparison produces. A screen-space overlay should never be
        // depth-tested against the 3D scene in the first place — same
        // reasoning any real renderer's HUD pass uses — so this is a
        // dedicated always-pass, no-write state instead of trying to tune
        // z=0.0 to reliably win.
        let ui_depth_descriptor = metal::DepthStencilDescriptor::new();
        ui_depth_descriptor.set_depth_compare_function(metal::MTLCompareFunction::Always);
        ui_depth_descriptor.set_depth_write_enabled(false);
        let ui_depth_state = device.new_depth_stencil_state(&ui_depth_descriptor);

        // Builds one Sponza RenderPipelineDescriptor (shared blend setup)
        // at the given sample count — used twice below (MSAA on/off) so the
        // two pipelines can't drift out of sync with each other.
        fn build_sponza_descriptor(
            vertex_fn: &metal::FunctionRef,
            fragment_fn: &metal::FunctionRef,
            sample_count: u64,
        ) -> RenderPipelineDescriptor {
            let descriptor = RenderPipelineDescriptor::new();
            descriptor.set_vertex_function(Some(vertex_fn));
            descriptor.set_fragment_function(Some(fragment_fn));
            let color_attachment = descriptor.color_attachments().object_at(0).unwrap();
            color_attachment.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
            // Real-game materials with `AlphaMode::Blend` (e.g. the scope
            // lens glass on the ak47_scope model) carry alpha < 1 in their
            // base_color/texture — without blending enabled here, Metal
            // ignores that alpha and paints every fragment fully opaque
            // regardless of what the fragment shader returns, which is why
            // the lens looked solid instead of see-through once real
            // textures/materials replaced the earlier flat-color
            // placeholder. Standard "source over" blending: opaque
            // (alpha=1) surfaces render identically to unblended, so this
            // is safe for the arena/gun's mostly-opaque geometry too, not
            // just the lens.
            color_attachment.set_blending_enabled(true);
            color_attachment.set_rgb_blend_operation(metal::MTLBlendOperation::Add);
            color_attachment.set_alpha_blend_operation(metal::MTLBlendOperation::Add);
            color_attachment.set_source_rgb_blend_factor(metal::MTLBlendFactor::SourceAlpha);
            color_attachment.set_source_alpha_blend_factor(metal::MTLBlendFactor::SourceAlpha);
            color_attachment.set_destination_rgb_blend_factor(metal::MTLBlendFactor::OneMinusSourceAlpha);
            color_attachment.set_destination_alpha_blend_factor(metal::MTLBlendFactor::OneMinusSourceAlpha);
            descriptor.set_depth_attachment_pixel_format(MTLPixelFormat::Depth32Float);
            descriptor.set_raster_sample_count(sample_count);
            descriptor
        }

        let (sponza_pipeline, sponza_pipeline_no_msaa) =
            match device.new_library_with_source(SPONZA_SHADER_SOURCE, &compile_options) {
                Ok(sponza_library) => {
                    let sponza_vertex_fn = sponza_library.get_function("sponza_vertex", None).unwrap();
                    let sponza_fragment_fn = sponza_library.get_function("sponza_fragment", None).unwrap();
                    let msaa_descriptor =
                        build_sponza_descriptor(&sponza_vertex_fn, &sponza_fragment_fn, MSAA_SAMPLE_COUNT);
                    let no_msaa_descriptor = build_sponza_descriptor(&sponza_vertex_fn, &sponza_fragment_fn, 1);
                    (
                        device.new_render_pipeline_state(&msaa_descriptor).ok(),
                        device.new_render_pipeline_state(&no_msaa_descriptor).ok(),
                    )
                }
                Err(err) => {
                    eprintln!("[native_metal_probe] sponza shader compile failed: {err}");
                    (None, None)
                }
            };

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

        // See flat_normal_texture's own field doc comment.
        let flat_normal_descriptor = metal::TextureDescriptor::new();
        flat_normal_descriptor.set_pixel_format(MTLPixelFormat::RGBA8Unorm);
        flat_normal_descriptor.set_width(1);
        flat_normal_descriptor.set_height(1);
        flat_normal_descriptor.set_storage_mode(metal::MTLStorageMode::Shared);
        let flat_normal_texture = device.new_texture(&flat_normal_descriptor);
        flat_normal_texture.replace_region(metal::MTLRegion::new_2d(0, 0, 1, 1), 0, [128u8, 128, 255, 255].as_ptr() as *const _, 4);

        // Sun + point-light shadow maps — see SHADOW_SHADER_SOURCE/
        // SHADOW_MAP_SIZE's own doc comments. Private storage (GPU-only,
        // never CPU-read) + RenderTarget (Pass 0/0b write them) |
        // ShaderRead (the main/scope passes' fragment shaders sample
        // them). Same descriptor shape reused for both — see
        // shadow_map_point's own field doc comment for why they're two
        // separate textures rather than one.
        let shadow_map_descriptor = metal::TextureDescriptor::new();
        shadow_map_descriptor.set_pixel_format(MTLPixelFormat::Depth32Float);
        shadow_map_descriptor.set_width(SHADOW_MAP_SIZE);
        shadow_map_descriptor.set_height(SHADOW_MAP_SIZE);
        shadow_map_descriptor.set_storage_mode(metal::MTLStorageMode::Private);
        shadow_map_descriptor.set_usage(metal::MTLTextureUsage::RenderTarget | metal::MTLTextureUsage::ShaderRead);
        let shadow_map = device.new_texture(&shadow_map_descriptor);
        let shadow_map_point = device.new_texture(&shadow_map_descriptor);

        let shadow_depth_descriptor = metal::DepthStencilDescriptor::new();
        shadow_depth_descriptor.set_depth_compare_function(metal::MTLCompareFunction::Less);
        shadow_depth_descriptor.set_depth_write_enabled(true);
        let shadow_depth_state = device.new_depth_stencil_state(&shadow_depth_descriptor);

        // Comparison sampler for `sample_compare` in the fragment shaders —
        // see shadow_sampler's own field doc comment (struct definition).
        // Clamp-to-edge (not Repeat, unlike sponza_sampler above): outside
        // the shadow frustum's box should read as "no occluder recorded"
        // (handled explicitly by the shadowUv bounds check in the shader),
        // not tile the shadow map's edge texels.
        let shadow_sampler_descriptor = metal::SamplerDescriptor::new();
        shadow_sampler_descriptor.set_min_filter(metal::MTLSamplerMinMagFilter::Linear);
        shadow_sampler_descriptor.set_mag_filter(metal::MTLSamplerMinMagFilter::Linear);
        shadow_sampler_descriptor.set_address_mode_s(metal::MTLSamplerAddressMode::ClampToEdge);
        shadow_sampler_descriptor.set_address_mode_t(metal::MTLSamplerAddressMode::ClampToEdge);
        shadow_sampler_descriptor.set_compare_function(metal::MTLCompareFunction::LessEqual);
        let shadow_sampler = device.new_sampler(&shadow_sampler_descriptor);

        // Depth-only pipelines — no fragment function, no color
        // attachments at all (see SHADOW_SHADER_SOURCE's own doc comment).
        // Single sample count only: the shadow map is never multisampled
        // (PCF via shadow_sampler above already softens its edges), so
        // unlike every other pipeline in this renderer there's no MSAA/
        // no-MSAA twin to build.
        let (shadow_pipeline_sponza, shadow_pipeline_skinned) =
            match device.new_library_with_source(SHADOW_SHADER_SOURCE, &compile_options) {
                Ok(shadow_library) => {
                    let sponza_shadow_vertex_fn = shadow_library.get_function("shadow_vertex_sponza", None).unwrap();
                    let sponza_descriptor = RenderPipelineDescriptor::new();
                    sponza_descriptor.set_vertex_function(Some(&sponza_shadow_vertex_fn));
                    sponza_descriptor.set_fragment_function(None);
                    sponza_descriptor.set_depth_attachment_pixel_format(MTLPixelFormat::Depth32Float);
                    let skinned_shadow_vertex_fn = shadow_library.get_function("shadow_vertex_skinned", None).unwrap();
                    let skinned_descriptor = RenderPipelineDescriptor::new();
                    skinned_descriptor.set_vertex_function(Some(&skinned_shadow_vertex_fn));
                    skinned_descriptor.set_fragment_function(None);
                    skinned_descriptor.set_depth_attachment_pixel_format(MTLPixelFormat::Depth32Float);
                    (
                        device.new_render_pipeline_state(&sponza_descriptor).ok(),
                        device.new_render_pipeline_state(&skinned_descriptor).ok(),
                    )
                }
                Err(err) => {
                    eprintln!("[native_metal_probe] shadow shader compile failed: {err}");
                    (None, None)
                }
            };

        fn build_skinned_descriptor(
            vertex_fn: &metal::FunctionRef,
            fragment_fn: &metal::FunctionRef,
            sample_count: u64,
        ) -> RenderPipelineDescriptor {
            let descriptor = RenderPipelineDescriptor::new();
            descriptor.set_vertex_function(Some(vertex_fn));
            descriptor.set_fragment_function(Some(fragment_fn));
            descriptor.color_attachments().object_at(0).unwrap().set_pixel_format(MTLPixelFormat::BGRA8Unorm);
            descriptor.set_depth_attachment_pixel_format(MTLPixelFormat::Depth32Float);
            descriptor.set_raster_sample_count(sample_count);
            descriptor
        }

        let (skinned_pipeline, skinned_pipeline_no_msaa) =
            match device.new_library_with_source(SKINNED_SHADER_SOURCE, &compile_options) {
                Ok(library) => {
                    let vertex_fn = library.get_function("skinned_vertex", None).unwrap();
                    let fragment_fn = library.get_function("skinned_fragment", None).unwrap();
                    let msaa_descriptor = build_skinned_descriptor(&vertex_fn, &fragment_fn, MSAA_SAMPLE_COUNT);
                    let no_msaa_descriptor = build_skinned_descriptor(&vertex_fn, &fragment_fn, 1);
                    (
                        device.new_render_pipeline_state(&msaa_descriptor).ok(),
                        device.new_render_pipeline_state(&no_msaa_descriptor).ok(),
                    )
                }
                Err(err) => {
                    eprintln!("[native_metal_probe] skinned shader compile failed: {err}");
                    (None, None)
                }
            };
        // Registers this project's two skinned actor types under the names
        // native_game_extract.rs's zombie/remote-player extraction tags its
        // SkinnedActorDraws with (see skinned_actor.rs's own doc comment) —
        // the renderer core itself has no idea these are "zombies" or
        // "soldiers", only that something registered these names.
        let resource_path = bundle_resource_path().unwrap_or_default();
        let mut skinned_models = super::skinned_actor::SkinnedModelRegistry::default();
        let soldier_path = std::path::Path::new(&resource_path).join("assets/characters/soldier_meshy/mesh/soldier_meshy.glb");
        if let Some(soldier_model) = super::skinned_gltf::load_skinned_gltf(&device, &soldier_path) {
            // Hand-copied from assets/characters/soldier_meshy/character.ron
            // — this custom Metal renderer never queries PlayerModelTuning
            // (the real game's dynamic RON-loaded source of truth on the
            // desktop/wgpu path) at all, only this hardcoded registration
            // determines what's drawn here, so these 3 numbers have to be
            // kept in sync BY HAND with that file. (This was already true
            // before switching to soldier_meshy — PLAYER_MODEL_SCALE/
            // _Y_OFFSET/_FACING_CORRECTION were themselves hand-copied
            // consts, now replaced by PlayerModelTuning; loading the RON
            // file dynamically here too would need bundle_resource_path(),
            // not PlayerModelTuning::load's CARGO_MANIFEST_DIR-relative
            // std::fs read, which only resolves correctly on the dev
            // machine that built the app, not a real device — untouched,
            // pre-existing gap, not introduced by this change.)
            const SCALE: f32 = 1.04;
            const Y_OFFSET: f32 = -0.76;
            const FACING_DEG: f32 = 0.0;
            skinned_models.register("soldier", soldier_model, SCALE, Y_OFFSET, FACING_DEG.to_radians());
        }
        // One registration per `ZombieKind`, under the name
        // `shared::zombie_kind_metal_model_name` picks — see that fn's own
        // doc comment for the real bug this replaced (a single Classic-
        // only "zombie" registration silently drawn for every kind). All
        // three glb files bake their own scale/offset/rotation into their
        // own scene root now (see the comment where
        // zombie_kind_mesh_transform used to live in crates/shared), so
        // every registration here is scale 1.0 / offset 0.0 / facing 0.0 —
        // the desktop/wgpu path's mesh-child Transform::IDENTITY, mirrored
        // here.
        for kind in [shared::ZombieKind::Classic, shared::ZombieKind::Meshy, shared::ZombieKind::Soldier] {
            let path = std::path::Path::new(&resource_path).join("assets").join(shared::zombie_kind_mesh_path(kind));
            if let Some(model) = super::skinned_gltf::load_skinned_gltf(&device, &path) {
                skinned_models.register(shared::zombie_kind_metal_model_name(kind), model, 1.0, 0.0, 0.0);
            }
        }

        // Real device metrics (bounds/scale computed above for the Metal
        // drawable size) threaded into the headless App's Window too — see
        // build_headless_game_ecs_app's own doc comment for why a mismatch
        // here breaks touch-driven UI hit-testing (drag-to-scroll silently
        // never engaging, taps landing on the wrong element).
        let real_game_app = Some(build_headless_game_ecs_app(
            (bounds.size.width * scale) as u32,
            (bounds.size.height * scale) as u32,
            scale as f32,
        ));

        // Resolve target — single-sample, still what quad_fragment (the PIP
        // composite) actually samples from; MSAA never touches this one
        // directly, see scope_color_texture_msaa below.
        let scope_color_descriptor = metal::TextureDescriptor::new();
        scope_color_descriptor.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
        scope_color_descriptor.set_width(SCOPE_TEXTURE_SIZE);
        scope_color_descriptor.set_height(SCOPE_TEXTURE_SIZE);
        scope_color_descriptor.set_storage_mode(metal::MTLStorageMode::Private);
        scope_color_descriptor.set_usage(metal::MTLTextureUsage::RenderTarget | metal::MTLTextureUsage::ShaderRead);
        let scope_color_texture = device.new_texture(&scope_color_descriptor);

        // The scope pass's actual render target — multisampled, resolved
        // into scope_color_texture above at the end of the pass (see
        // render_frame's MTLStoreAction::MultisampleResolve). Never sampled
        // itself, so no ShaderRead usage needed.
        let scope_color_msaa_descriptor = metal::TextureDescriptor::new();
        scope_color_msaa_descriptor.set_texture_type(metal::MTLTextureType::D2Multisample);
        scope_color_msaa_descriptor.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
        scope_color_msaa_descriptor.set_width(SCOPE_TEXTURE_SIZE);
        scope_color_msaa_descriptor.set_height(SCOPE_TEXTURE_SIZE);
        scope_color_msaa_descriptor.set_sample_count(MSAA_SAMPLE_COUNT);
        scope_color_msaa_descriptor.set_storage_mode(metal::MTLStorageMode::Private);
        scope_color_msaa_descriptor.set_usage(metal::MTLTextureUsage::RenderTarget);
        let scope_color_texture_msaa = device.new_texture(&scope_color_msaa_descriptor);

        let scope_depth_descriptor = metal::TextureDescriptor::new();
        scope_depth_descriptor.set_texture_type(metal::MTLTextureType::D2Multisample);
        scope_depth_descriptor.set_pixel_format(MTLPixelFormat::Depth32Float);
        scope_depth_descriptor.set_width(SCOPE_TEXTURE_SIZE);
        scope_depth_descriptor.set_height(SCOPE_TEXTURE_SIZE);
        scope_depth_descriptor.set_sample_count(MSAA_SAMPLE_COUNT);
        scope_depth_descriptor.set_storage_mode(metal::MTLStorageMode::Private);
        scope_depth_descriptor.set_usage(metal::MTLTextureUsage::RenderTarget);
        let scope_depth_texture = device.new_texture(&scope_depth_descriptor);

        // Sample-count-1 counterpart of scope_depth_texture, for the
        // MapMsaaOn-off path — the scope pass then renders straight into
        // scope_color_texture with no resolve step at all (see
        // render_frame), same as before MSAA existed in this renderer.
        let scope_depth_no_msaa_descriptor = metal::TextureDescriptor::new();
        scope_depth_no_msaa_descriptor.set_pixel_format(MTLPixelFormat::Depth32Float);
        scope_depth_no_msaa_descriptor.set_width(SCOPE_TEXTURE_SIZE);
        scope_depth_no_msaa_descriptor.set_height(SCOPE_TEXTURE_SIZE);
        scope_depth_no_msaa_descriptor.set_storage_mode(metal::MTLStorageMode::Private);
        scope_depth_no_msaa_descriptor.set_usage(metal::MTLTextureUsage::RenderTarget);
        let scope_depth_texture_no_msaa = device.new_texture(&scope_depth_no_msaa_descriptor);

        fn build_quad_descriptor(vertex_fn: &metal::FunctionRef, fragment_fn: &metal::FunctionRef, sample_count: u64) -> RenderPipelineDescriptor {
            let descriptor = RenderPipelineDescriptor::new();
            descriptor.set_vertex_function(Some(vertex_fn));
            descriptor.set_fragment_function(Some(fragment_fn));
            let color_attachment = descriptor.color_attachments().object_at(0).unwrap();
            color_attachment.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
            // Was opaque (no blending) — the scope PIP painted fully solid
            // over whatever the main camera already drew there, unlike the
            // real game's translucent ScopeLens glass (see
            // build_sponza_descriptor's comment). quad_fragment now returns
            // alpha < 1, so this needs the same standard "source over"
            // blend setup as that pipeline to actually show through.
            color_attachment.set_blending_enabled(true);
            color_attachment.set_rgb_blend_operation(metal::MTLBlendOperation::Add);
            color_attachment.set_alpha_blend_operation(metal::MTLBlendOperation::Add);
            color_attachment.set_source_rgb_blend_factor(metal::MTLBlendFactor::SourceAlpha);
            color_attachment.set_source_alpha_blend_factor(metal::MTLBlendFactor::SourceAlpha);
            color_attachment.set_destination_rgb_blend_factor(metal::MTLBlendFactor::OneMinusSourceAlpha);
            color_attachment.set_destination_alpha_blend_factor(metal::MTLBlendFactor::OneMinusSourceAlpha);
            // Missing on a real device (confirmed via crash log): the quad
            // PIP draws into the SAME render pass as the 3D scene (see
            // build_sponza_descriptor's own comment on why the UI overlay
            // shares 1 pass) — that pass has a Depth32Float attachment
            // bound, so every pipeline used in it needs to declare the
            // same depth format or Metal's validation layer rejects
            // set_render_pipeline_state with "pixelFormat
            // (MTLPixelFormatInvalid) does not match the framebuffer's
            // pixelFormat (MTLPixelFormatDepth32Float)" — fatal on device,
            // where that validation is enforced; happened to go unnoticed
            // on Simulator.
            descriptor.set_depth_attachment_pixel_format(MTLPixelFormat::Depth32Float);
            descriptor.set_raster_sample_count(sample_count);
            descriptor
        }

        let (quad_pipeline, quad_pipeline_no_msaa) =
            match device.new_library_with_source(QUAD_SHADER_SOURCE, &compile_options) {
                Ok(library) => {
                    let vertex_fn = library.get_function("quad_vertex", None).unwrap();
                    let fragment_fn = library.get_function("quad_fragment", None).unwrap();
                    let msaa_descriptor = build_quad_descriptor(&vertex_fn, &fragment_fn, MSAA_SAMPLE_COUNT);
                    let no_msaa_descriptor = build_quad_descriptor(&vertex_fn, &fragment_fn, 1);
                    (
                        device.new_render_pipeline_state(&msaa_descriptor).ok(),
                        device.new_render_pipeline_state(&no_msaa_descriptor).ok(),
                    )
                }
                Err(err) => {
                    eprintln!("[native_metal_probe] quad shader compile failed: {err}");
                    (None, None)
                }
            };
        // Picture-in-picture rectangle in the top-right corner, in NDC
        // (-1..1) space — no view/projection needed, it's drawn directly
        // over the finished main-camera frame. z=0/w=1 makes this plain
        // NDC-as-clip-space, same as before quad_vertex started taking
        // real per-vertex clip positions (see its own MSL doc comment) —
        // this demo-probe quad is still flat/fixed, unlike the real-game
        // scope PIP's build_projected_quad. Two triangles, position + uv
        // interleaved per vertex (same "raw device buffer, no vertex
        // descriptor" reasoning as every other shader in this file).
        let quad_verts = [
            QuadVertex { position: [0.55, 0.55, 0.0, 1.0], uv: [0.0, 0.0] },
            QuadVertex { position: [0.95, 0.55, 0.0, 1.0], uv: [1.0, 0.0] },
            QuadVertex { position: [0.95, 0.95, 0.0, 1.0], uv: [1.0, 1.0] },
            QuadVertex { position: [0.55, 0.55, 0.0, 1.0], uv: [0.0, 0.0] },
            QuadVertex { position: [0.95, 0.95, 0.0, 1.0], uv: [1.0, 1.0] },
            QuadVertex { position: [0.55, 0.95, 0.0, 1.0], uv: [0.0, 1.0] },
        ];
        let quad_vertex_buffer = device.new_buffer_with_data(
            quad_verts.as_ptr() as *const _,
            (quad_verts.len() * std::mem::size_of::<QuadVertex>()) as u64,
            MTLResourceOptions::StorageModeShared,
        );

        fn build_ui_descriptor(vertex_fn: &metal::FunctionRef, fragment_fn: &metal::FunctionRef, sample_count: u64) -> RenderPipelineDescriptor {
            let descriptor = RenderPipelineDescriptor::new();
            descriptor.set_vertex_function(Some(vertex_fn));
            descriptor.set_fragment_function(Some(fragment_fn));
            let color_attachment = descriptor.color_attachments().object_at(0).unwrap();
            color_attachment.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
            // Health bar backgrounds are translucent black and glyph quads
            // carry a coverage-shaped alpha (see build_font_atlas) — both
            // need real alpha blending, same as quad_pipeline's own scope
            // PIP above (also blended now, see build_quad_descriptor).
            color_attachment.set_blending_enabled(true);
            color_attachment.set_rgb_blend_operation(metal::MTLBlendOperation::Add);
            color_attachment.set_alpha_blend_operation(metal::MTLBlendOperation::Add);
            color_attachment.set_source_rgb_blend_factor(metal::MTLBlendFactor::SourceAlpha);
            color_attachment.set_source_alpha_blend_factor(metal::MTLBlendFactor::SourceAlpha);
            color_attachment.set_destination_rgb_blend_factor(metal::MTLBlendFactor::OneMinusSourceAlpha);
            color_attachment.set_destination_alpha_blend_factor(metal::MTLBlendFactor::OneMinusSourceAlpha);
            // Same missing-depth-format bug as build_quad_descriptor above
            // (see its own comment) — the UI overlay (health bars, floating
            // damage, touch controls) draws in the same Depth32Float-backed
            // render pass as the 3D scene.
            descriptor.set_depth_attachment_pixel_format(MTLPixelFormat::Depth32Float);
            descriptor.set_raster_sample_count(sample_count);
            descriptor
        }

        let (ui_pipeline, ui_pipeline_no_msaa) =
            match device.new_library_with_source(UI_SHADER_SOURCE, &compile_options) {
                Ok(library) => {
                    let vertex_fn = library.get_function("ui_vertex", None).unwrap();
                    let fragment_fn = library.get_function("ui_fragment", None).unwrap();
                    let msaa_descriptor = build_ui_descriptor(&vertex_fn, &fragment_fn, MSAA_SAMPLE_COUNT);
                    let no_msaa_descriptor = build_ui_descriptor(&vertex_fn, &fragment_fn, 1);
                    (
                        device.new_render_pipeline_state(&msaa_descriptor).ok(),
                        device.new_render_pipeline_state(&no_msaa_descriptor).ok(),
                    )
                }
                Err(err) => {
                    eprintln!("[native_metal_probe] ui shader compile failed: {err}");
                    (None, None)
                }
            };
        // Nearest, not linear — this is a crisp pixel font/flat-colored
        // rects, not a photo texture; linear filtering would just blur
        // glyph edges. Clamp instead of sponza_sampler's Repeat, since a
        // glyph quad's UV can legitimately sit exactly on a cell boundary.
        let ui_sampler_descriptor = metal::SamplerDescriptor::new();
        ui_sampler_descriptor.set_min_filter(metal::MTLSamplerMinMagFilter::Nearest);
        ui_sampler_descriptor.set_mag_filter(metal::MTLSamplerMinMagFilter::Nearest);
        ui_sampler_descriptor.set_address_mode_s(metal::MTLSamplerAddressMode::ClampToEdge);
        ui_sampler_descriptor.set_address_mode_t(metal::MTLSamplerAddressMode::ClampToEdge);
        let ui_sampler = device.new_sampler(&ui_sampler_descriptor);
        let font_atlas_texture = build_font_atlas(&device);

        Self {
            device,
            layer,
            queue,
            sponza_pipeline,
            sponza_pipeline_no_msaa,
            sponza_sampler,
            shadow_map,
            shadow_map_point,
            shadow_pipeline_sponza,
            shadow_pipeline_skinned,
            shadow_depth_state,
            shadow_sampler,
            white_texture,
            flat_normal_texture,
            depth_state,
            blend_depth_state,
            ui_depth_state,
            msaa_targets: super::metal_msaa::MsaaTargets::new(MSAA_SAMPLE_COUNT),
            msaa_enabled: false,
            camera_position: [0.0, 2.5, 2.0],
            camera_yaw: 0.0,
            camera_pitch: -0.25,
            skinned_pipeline,
            skinned_pipeline_no_msaa,
            skinned_models,
            scope_color_texture,
            scope_color_texture_msaa,
            scope_depth_texture,
            scope_depth_texture_no_msaa,
            quad_pipeline,
            quad_pipeline_no_msaa,
            quad_vertex_buffer,
            start_time: std::time::Instant::now(),
            real_game_app,
            real_game_textures: std::collections::HashMap::new(),
            real_game_camera_rotation: [0.0, 0.0, 0.0, 1.0],
            real_game_ads_blend: 0.0,
            real_game_scope_zoom: 8.0,
            real_game_wall_raise: 0.0,
            real_game_scope_eye: [0.0, 0.0, 0.0],
            real_game_scope_rotation: [0.0, 0.0, 0.0, 1.0],
            real_game_scope_active: false,
            real_game_scope_fov: 12.0_f32.to_radians(),
            real_game_scope_lens_world_pos: [0.0, 0.0, 0.0],
            real_game_scope_lens_local_rotation: [0.0, 0.0, 0.0, 1.0],
            real_game_light: FALLBACK_LIGHT,
            ui_pipeline,
            ui_pipeline_no_msaa,
            ui_sampler,
            font_atlas_texture,
            ui_floating_damage: Vec::new(),
            ui_health_bars: Vec::new(),
            ui_touch_buttons: Vec::new(),
            ui_touch_joystick: None,
            ui_scope_zoom_slider: None,
            ui_hud_texts: Vec::new(),
            ui_crosshair_lines: Vec::new(),
            ui_settings_menu_panel: None,
            ui_settings_menu_buttons: Vec::new(),
            ui_settings_menu_texts: Vec::new(),
            ui_game_over_panel: None,
            ui_game_over_buttons: Vec::new(),
            ui_game_over_texts: Vec::new(),
            ui_map_select_panel: None,
            ui_map_select_buttons: Vec::new(),
            ui_map_select_texts: Vec::new(),
        }
    }

    // Phase 6 Step 3: exposes the headless real-game App to ffi.rs's
    // `native_metal_probe_touch_*`/`key_changed`/`mouse_*` functions so they
    // can write real Bevy input messages (`TouchInput`/`KeyboardInput`/
    // `MouseMotion`/`MouseButtonInput`) straight into it — the exact same
    // message-writing helpers `cadisplaylink_*` already uses against the
    // toy/real `cadisplaylink_create_game_app`, just re-targeted. Always
    // `Some` in practice (every `NativeMetalProbe` builds one) — `Option`
    // mainly so callers don't need a separate liveness check.
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

    // The view's own size in points (not `drawable_size`'s physical
    // pixels — that's `sync_drawable_size`'s `bounds.size * contents_scale`).
    // Needed for prime_camera_viewport (which now also needs the real
    // contents_scale alongside this — see its own doc comment for why
    // both, not just points, are required for touch hit-testing to line
    // up with the visually-drawn UI) and the UI overlay's NDC math.
    fn view_size_points(&self) -> (f32, f32) {
        unsafe {
            let raw_layer = self.layer.as_ptr() as *mut Object;
            let bounds: CGRect = msg_send![raw_layer, bounds];
            (bounds.size.width as f32, bounds.size.height as f32)
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
        // anim_time is an *absolute* time to feed `compute_joint_matrices`
        // directly, not an offset added to `now` the way the toy demo's own
        // AnimOffset component used to work (that math now happens once at
        // the call site instead, see both places this is built below, so
        // this fn doesn't need `now` for animation at all anymore —
        // real-game zombies use their actual AnimationPlayer's seek_time,
        // not wall-clock time). `model` looks up self.skinned_models — see
        // skinned_actor.rs's own doc comment for why this is a name, not a
        // gameplay-type enum.
        extracted: &[super::skinned_actor::SkinnedActorDraw],
        _now: f32,
        real_game_meshes: Option<&[RealGameMeshDraw]>,
        // Selects which of each shader's two compiled pipelines (see
        // MSAA_SAMPLE_COUNT's own doc comment) to bind — must match
        // whatever sample count the caller's render pass attachments
        // actually have, or Metal rejects every draw call in this pass.
        msaa_enabled: bool,
        // Sun/point-light shadow map read-back — see render_frame's
        // "Pass 0"/"Pass 0b" for how `light_view_projection`/
        // `point_light_view_projection` are built; `camera_pos` feeds the
        // fragment shaders' Blinn-Phong specular term (see SPONZA_SHADER_
        // SOURCE's own Uniforms doc comment for why it needs the eye
        // position at all).
        light_view_projection: [[f32; 4]; 4],
        point_light_view_projection: [[f32; 4]; 4],
        camera_pos: [f32; 3],
    ) {
        let sponza_pipeline = if msaa_enabled { self.sponza_pipeline.as_ref() } else { self.sponza_pipeline_no_msaa.as_ref() };
        let skinned_pipeline = if msaa_enabled { self.skinned_pipeline.as_ref() } else { self.skinned_pipeline_no_msaa.as_ref() };
        encoder.set_fragment_texture(1, Some(&self.shadow_map));
        encoder.set_fragment_sampler_state(1, Some(&self.shadow_sampler));
        encoder.set_fragment_texture(2, Some(&self.shadow_map_point));

        // `real_game_meshes` is `None` only in the brief window before the
        // headless gameplay App's first Startup schedule has actually run
        // (spawn_test_arena hasn't spawned the floor/walls yet) — nothing to
        // draw here that frame, not an error. There used to be two more
        // fallback branches here (a hardcoded box-arena demo, and a
        // Sponza-scene "probe" demo) from earlier phases of this renderer's
        // own development, both unreachable in the shipped app (Swift's
        // ViewController.swift hardcodes USE_REAL_GAME_NATIVE_RENDERER =
        // true, USE_NATIVE_METAL_DEMO = false) — removed rather than kept
        // as dead weight; see git history if either is ever worth
        // resurrecting for a standalone demo/probe build again.
        // Draws one static mesh (arena wall/floor or a gun/lens child mesh)
        // — factored out so opaque and blended meshes can be drawn as two
        // separate passes below (see their own call sites' doc comment for
        // why order matters) without duplicating the uniform-building.
        let draw_static_mesh = |m: &RealGameMeshDraw| {
            let sponza_uniforms = SponzaUniforms {
                view_projection,
                light_pos: light.position,
                light_range: light.range,
                light_color: light.color,
                sun_dir: light.sun_dir,
                _sun_pad: 0.0,
                sun_color: light.sun_color,
                base_color_factor: m.base_color_factor,
                emissive: m.emissive,
                light_view_projection,
                point_light_view_projection,
                camera_pos,
                _camera_pad: 0.0,
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
            let normal_texture = m.normal_texture.unwrap_or(&self.flat_normal_texture);
            encoder.set_fragment_texture(3, Some(normal_texture));
            encoder.draw_primitives(MTLPrimitiveType::Triangle, 0, m.vertex_count as u64);
        };

        if let Some(meshes) = real_game_meshes {
            // Reuses Sponza's own pipeline/shader (position+normal+uv,
            // textured) unchanged — arena/gun geometry is textured exactly
            // like Sponza's, just rebaked into world space fresh each frame
            // (see bake_static_mesh_vertices) instead of once at load time.
            // Opaque/Mask only here (is_blend false — see ExtractedMesh::
            // is_blend) — real AlphaMode::Blend materials anywhere in the
            // scene (arena or gun) are drawn in a second pass below, AFTER
            // the skinned zombies/remote players, with depth WRITE
            // disabled — see that pass's own comment for the full story.
            encoder.set_render_pipeline_state(sponza_pipeline.unwrap());
            encoder.set_fragment_sampler_state(0, Some(&self.sponza_sampler));
            for m in meshes.iter().filter(|m| !m.is_blend) {
                draw_static_mesh(m);
            }
        }

        let Some(pipeline) = skinned_pipeline else { return };
        encoder.set_render_pipeline_state(pipeline);
        encoder.set_fragment_sampler_state(0, Some(&self.sponza_sampler));

        for actor in extracted {
            let Some(slot) = self.skinned_models.get(actor.model) else { continue };
            let (model, mesh_scale, y_offset) = (&slot.model, slot.scale, slot.y_offset);
            // This is the ONLY place that actually determines what's drawn
            // on screen for USE_REAL_GAME_NATIVE_RENDERER — this custom
            // Metal renderer never queries Bevy's own scene graph (the
            // mesh-child-entity scale/offset/facing corrections the
            // desktop/wgpu path applies there via PlayerModelTuning,
            // crate::lib) at all — see skinned_actor.rs's own doc comment
            // for why each registered model instead carries its own
            // scale/y_offset/facing_correction as plain data.
            // See SkinnedActorPose's own doc comment: JointMatrices (the
            // real-ECS-skeleton path) is used as-is; Clip is the fallback
            // for actors with no live skeleton wired up to extract from yet.
            //
            // model_matrix is IDENTITY for JointMatrices — those already
            // came out of the real ECS's own GlobalTransform, which already
            // includes the mesh child entity's own scale/y-offset (the same
            // `zombie_mesh_bundle`/ZOMBIE_SCALE transform the desktop path
            // applies) baked in. Applying mesh_scale/y_offset/facing AGAIN
            // here double-transformed it — scaled twice (ZOMBIE_SCALE² ≈
            // 1/16 size) and offset twice, which is exactly what shipped
            // briefly as "zombies render ant-sized" before this fix. Clip
            // still needs the old scale/offset/facing model_matrix, since
            // `compute_joint_matrices` samples the glTF's own raw
            // bind-pose-relative hierarchy — never placed into world space
            // by anything else.
            let (model_matrix, joint_matrices) = match &actor.pose {
                super::skinned_actor::SkinnedActorPose::JointMatrices(matrices) => {
                    ([[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]], matrices.clone())
                }
                super::skinned_actor::SkinnedActorPose::Clip { clip_index, time } => {
                    let rotation = if slot.facing_correction != 0.0 {
                        actor.transform.rotation * bevy::math::Quat::from_rotation_y(slot.facing_correction)
                    } else {
                        actor.transform.rotation
                    };
                    let [qx, qy, qz, qw] = rotation.to_array();
                    let (x2, y2, z2) = (qx + qx, qy + qy, qz + qz);
                    let (xx, xy, xz) = (qx * x2, qx * y2, qx * z2);
                    let (yy, yz, zz) = (qy * y2, qy * z2, qz * z2);
                    let (wx, wy, wz) = (qw * x2, qw * y2, qw * z2);
                    let matrix = [
                        [(1.0 - (yy + zz)) * mesh_scale, (xy + wz) * mesh_scale, (xz - wy) * mesh_scale, 0.0],
                        [(xy - wz) * mesh_scale, (1.0 - (xx + zz)) * mesh_scale, (yz + wx) * mesh_scale, 0.0],
                        [(xz + wy) * mesh_scale, (yz - wx) * mesh_scale, (1.0 - (xx + yy)) * mesh_scale, 0.0],
                        [actor.transform.translation.x, actor.transform.translation.y + y_offset, actor.transform.translation.z, 1.0],
                    ];
                    (matrix, model.compute_joint_matrices(*clip_index, *time))
                }
            };
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
                    light_view_projection,
                    point_light_view_projection,
                    camera_pos,
                    _camera_pad: 0.0,
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

        if let Some(meshes) = real_game_meshes {
            // Real AlphaMode::Blend/Premultiplied/Add/Multiply meshes
            // (is_blend — the scope lens glass, the red-dot sight's own
            // lens, or any future translucent material anywhere in the
            // scene, arena or gun) drawn LAST, after both the opaque pass
            // above and the skinned zombies/remote players just above
            // this — AND with self.blend_depth_state (test-only, write
            // disabled) instead of self.depth_state. Two distinct bugs
            // this fixes together:
            //   1. Draw order: self.depth_state (see its own construction
            //      comment) writes depth for every draw regardless of
            //      blending, so a blended mesh drawn BEFORE a zombie
            //      behind it wrote its own (much nearer) depth first —
            //      the zombie's later draw failed the depth test against
            //      that and got silently culled, invisible through any
            //      translucent geometry despite blending being enabled on
            //      this pipeline. Drawing translucent meshes after
            //      everything solid (opaque geometry + zombies) fixes
            //      this the same way any forward renderer's opaque-then-
            //      transparent pass split does — the shadow map (a wholly
            //      separate pass/depth buffer, see draw_shadow_casters)
            //      was never affected, which is why the zombie's shadow
            //      still showed even though the zombie itself didn't.
            //   2. Depth WRITE: even drawn last, a shared depth-write-
            //      enabled state would make two overlapping translucent
            //      meshes (e.g. the scope lens glass sitting just in
            //      front of the red-dot sight's own lens) occlude each
            //      other by draw order alone instead of correctly
            //      blending both — same underlying issue, just between
            //      two blended meshes instead of a blended mesh and a
            //      zombie. blend_depth_state still respects real
            //      occlusion from anything nearer already in the depth
            //      buffer (test stays on), it just never writes its own
            //      depth, so nothing drawn in this same pass can
            //      incorrectly block another blended mesh behind it.
            // Sorted back-to-front (farthest camera_dist_sq first) so
            // overlapping blended meshes still composite in the right
            // visual order even without a depth write to fall back on —
            // standard transparency-pass sorting, same reasoning as
            // camera_dist_sq's own doc comment on RealGameMeshDraw.
            let mut blended: Vec<&RealGameMeshDraw> = meshes.iter().filter(|m| m.is_blend).collect();
            blended.sort_by(|a, b| b.camera_dist_sq.total_cmp(&a.camera_dist_sq));
            if !blended.is_empty() {
                encoder.set_depth_stencil_state(&self.blend_depth_state);
                encoder.set_render_pipeline_state(sponza_pipeline.unwrap());
                encoder.set_fragment_sampler_state(0, Some(&self.sponza_sampler));
                for m in blended {
                    draw_static_mesh(m);
                }
                // Restore — nothing downstream in this same render pass
                // (e.g. render_frame's scope PIP quad, drawn with this
                // same encoder right after draw_scene returns) should
                // have to reason about which depth-stencil state
                // draw_scene happened to leave bound.
                encoder.set_depth_stencil_state(&self.depth_state);
            }
        }
    }

    // Pass 0 (see render_frame): renders the same geometry draw_scene does
    // into the sun's shadow map instead of the screen — depth-only, no
    // fragment shader, no lighting. The model_matrix/joint_matrices pose
    // resolution below is a deliberate copy of draw_scene's own (not
    // factored into a shared helper) — this renderer already accepts small
    // duplication like this for shader-adjacent logic (see kShininess/
    // kSpecularStrength's own "duplicated, not shared" comment) rather than
    // threading an extra owned Vec<(model_matrix, joint_buffer)> through
    // both call sites just to avoid it.
    fn draw_shadow_casters(
        &self,
        encoder: &metal::RenderCommandEncoderRef,
        light_view_projection: [[f32; 4]; 4],
        extracted: &[super::skinned_actor::SkinnedActorDraw],
        real_game_meshes: Option<&[RealGameMeshDraw]>,
    ) {
        if let Some(meshes) = real_game_meshes {
            if let Some(pipeline) = self.shadow_pipeline_sponza.as_ref() {
                encoder.set_render_pipeline_state(pipeline);
                for m in meshes {
                    let uniforms = SponzaShadowUniforms { light_view_projection };
                    encoder.set_vertex_buffer(0, Some(&m.vertex_buffer), 0);
                    encoder.set_vertex_bytes(1, std::mem::size_of::<SponzaShadowUniforms>() as u64, &uniforms as *const _ as *const _);
                    encoder.draw_primitives(MTLPrimitiveType::Triangle, 0, m.vertex_count as u64);
                }
            }
        }

        let Some(pipeline) = self.shadow_pipeline_skinned.as_ref() else { return };
        encoder.set_render_pipeline_state(pipeline);
        for actor in extracted {
            let Some(slot) = self.skinned_models.get(actor.model) else { continue };
            let (model, mesh_scale, y_offset) = (&slot.model, slot.scale, slot.y_offset);
            let (model_matrix, joint_matrices) = match &actor.pose {
                super::skinned_actor::SkinnedActorPose::JointMatrices(matrices) => {
                    ([[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]], matrices.clone())
                }
                super::skinned_actor::SkinnedActorPose::Clip { clip_index, time } => {
                    let rotation = if slot.facing_correction != 0.0 {
                        actor.transform.rotation * bevy::math::Quat::from_rotation_y(slot.facing_correction)
                    } else {
                        actor.transform.rotation
                    };
                    let [qx, qy, qz, qw] = rotation.to_array();
                    let (x2, y2, z2) = (qx + qx, qy + qy, qz + qz);
                    let (xx, xy, xz) = (qx * x2, qx * y2, qx * z2);
                    let (yy, yz, zz) = (qy * y2, qy * z2, qz * z2);
                    let (wx, wy, wz) = (qw * x2, qw * y2, qw * z2);
                    let matrix = [
                        [(1.0 - (yy + zz)) * mesh_scale, (xy + wz) * mesh_scale, (xz - wy) * mesh_scale, 0.0],
                        [(xy - wz) * mesh_scale, (1.0 - (xx + zz)) * mesh_scale, (yz + wx) * mesh_scale, 0.0],
                        [(xz + wy) * mesh_scale, (yz - wx) * mesh_scale, (1.0 - (xx + yy)) * mesh_scale, 0.0],
                        [actor.transform.translation.x, actor.transform.translation.y + y_offset, actor.transform.translation.z, 1.0],
                    ];
                    (matrix, model.compute_joint_matrices(*clip_index, *time))
                }
            };
            let joint_buffer = self.device.new_buffer_with_data(
                joint_matrices.as_ptr() as *const _,
                (joint_matrices.len() * std::mem::size_of::<[[f32; 4]; 4]>()) as u64,
                MTLResourceOptions::StorageModeShared,
            );
            for mesh in &model.meshes {
                let uniforms = SkinnedShadowUniforms { light_view_projection, model_matrix };
                encoder.set_vertex_buffer(0, Some(&mesh.vertex_buffer), 0);
                encoder.set_vertex_buffer(2, Some(&joint_buffer), 0);
                encoder.set_vertex_bytes(1, std::mem::size_of::<SkinnedShadowUniforms>() as u64, &uniforms as *const _ as *const _);
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

        let is_real_game = self.real_game_app.is_some();
        // Moved up from further below (used to only be needed just before
        // the draw_scene calls) — the remote-player extraction just below
        // now needs it too, for its static PLAYER_MODEL_ANIM_WALK pose.
        let now = self.start_time.elapsed().as_secs_f32();

        // Tick the real headless game App and extract a fresh snapshot —
        // camera pose comes from the game's own FpsCamera instead of touch
        // input, and the arena+gun geometry gets rebaked into a plain
        // vertex buffer each frame (see bake_static_meshes).
        let mut real_game_static_meshes: Vec<super::native_game_extract::ExtractedMesh> = Vec::new();
        // Real zombies/remote players, tagged with the same "zombie"/
        // "soldier" names registered in `new()` (see skinned_actor.rs) so
        // draw_scene's skinned-mesh loop draws them via that generic
        // lookup. Zombies carry their REAL AnimationPlayer's clip+seek_time
        // (see ZombieRenderState's own doc comment: the same "extract the
        // real thing" principle already applied to camera/scope camera).
        let mut real_game_extracted: Vec<super::skinned_actor::SkinnedActorDraw> = Vec::new();
        let (view_width_pts, view_height_pts) = self.view_size_points();
        if let Some(app) = &mut self.real_game_app {
            prime_camera_viewport(app.world_mut(), view_width_pts, view_height_pts, self.layer.contents_scale() as f32);
            app.update();
            let snapshot = extract_render_snapshot(app.world_mut());
            self.ui_floating_damage = snapshot.floating_damage;
            self.ui_health_bars = snapshot.health_bars;
            self.ui_touch_buttons = snapshot.touch_buttons;
            self.ui_touch_joystick = snapshot.touch_joystick;
            self.ui_scope_zoom_slider = snapshot.scope_zoom_slider;
            self.ui_hud_texts = snapshot.hud_texts;
            self.ui_crosshair_lines = snapshot.crosshair_lines;
            self.ui_settings_menu_panel = snapshot.settings_menu_panel;
            self.ui_settings_menu_buttons = snapshot.settings_menu_buttons;
            self.ui_settings_menu_texts = snapshot.settings_menu_texts;
            self.ui_game_over_panel = snapshot.game_over_panel;
            self.ui_game_over_buttons = snapshot.game_over_buttons;
            self.ui_game_over_texts = snapshot.game_over_texts;
            self.ui_map_select_panel = snapshot.map_select_panel;
            self.ui_map_select_buttons = snapshot.map_select_buttons;
            self.ui_map_select_texts = snapshot.map_select_texts;
            self.msaa_enabled = snapshot.map_msaa_on;
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
                self.real_game_scope_lens_world_pos = scope_cam.lens_world_pos;
                self.real_game_scope_lens_local_rotation = scope_cam.lens_local_rotation;
            } else {
                self.real_game_scope_active = false;
            }
            self.real_game_light = LightUniform::from_snapshots(snapshot.light.as_ref(), snapshot.sun.as_ref());
            real_game_static_meshes = snapshot.static_meshes;
            real_game_extracted.extend(snapshot.zombies.into_iter().map(|z| super::skinned_actor::SkinnedActorDraw {
                transform: z.transform,
                model: shared::zombie_kind_metal_model_name(z.kind),
                pose: super::skinned_actor::SkinnedActorPose::JointMatrices(z.joint_matrices),
            }));
            // Remote players: same real-per-joint-skeleton extraction as
            // zombies now — see native_game_extract.rs's remote-player
            // extraction (find_first_skinned_mesh/joint_matrices_for,
            // reading `PlayerModelAnimationPlayer`-driven AnimationPlayer
            // state that `update_remote_player_model_animation` already
            // plays for real, headless included) instead of the old static
            // "always PLAYER_MODEL_ANIM_WALK at wall-clock time" guess.
            real_game_extracted.extend(snapshot.remote_players.into_iter().map(|p| super::skinned_actor::SkinnedActorDraw {
                transform: p.transform,
                model: "soldier",
                pose: super::skinned_actor::SkinnedActorPose::JointMatrices(p.joint_matrices),
            }));
        }
        // Texture cache: upload once per unique `Image` asset, keyed by its
        // own AssetId (see `real_game_textures`'s own doc comment on the
        // struct for why identity-keyed, not positional). A texture asset
        // never changes its own pixels after loading — the only thing that
        // can happen is a mesh not having ITS texture ready yet (Bevy's
        // asset server loads the gun's texture asynchronously), which just
        // means `mesh.texture` is `None` for a frame or two and naturally
        // resolves to `Some` once loaded, no special-casing needed here.
        // Must happen here — after msaa_enabled is set by the extraction
        // above, but before real_game_meshes below borrows
        // self.real_game_textures (immutably, for the rest of this
        // function via RealGameMeshDraw's own lifetime), which would
        // otherwise conflict with the &mut self this needs.
        let msaa_enabled = self.msaa_enabled;
        let device = self.device.clone();
        let depth_texture = if msaa_enabled {
            self.msaa_targets.ensure_depth(&device, width, height)
        } else {
            self.msaa_targets.ensure_depth_no_msaa(&device, width, height)
        };
        let msaa_color_texture = msaa_enabled.then(|| self.msaa_targets.ensure_color(&device, width, height));
        let Some(drawable) = self.layer.next_drawable() else { return };
        let texture = drawable.texture();

        for mesh in &real_game_static_meshes {
            // Same identity-keyed upload-once cache for both — see
            // real_game_textures' own doc comment; normal maps are just
            // another Image asset to it, no special-casing needed.
            for t in [mesh.texture.as_ref(), mesh.normal_texture.as_ref()].into_iter().flatten() {
                if !self.real_game_textures.contains_key(&t.id) {
                    let uploaded = super::metal_texture::upload_texture(&self.device, t.width, t.height, &t.rgba);
                    self.real_game_textures.insert(t.id, uploaded);
                }
            }
        }
        let real_game_meshes: Vec<RealGameMeshDraw> = real_game_static_meshes
            .iter()
            .map(|mesh| {
                let verts = bake_static_mesh_vertices(mesh);
                let vertex_buffer = self.device.new_buffer_with_data(
                    verts.as_ptr() as *const _,
                    (verts.len() * std::mem::size_of::<super::gltf_scene::GltfVertex>()) as u64,
                    MTLResourceOptions::StorageModeShared,
                );
                let texture = mesh.texture.as_ref().and_then(|t| self.real_game_textures.get(&t.id));
                let normal_texture = mesh.normal_texture.as_ref().and_then(|t| self.real_game_textures.get(&t.id));
                let d = [
                    mesh.transform.translation.x - self.camera_position[0],
                    mesh.transform.translation.y - self.camera_position[1],
                    mesh.transform.translation.z - self.camera_position[2],
                ];
                RealGameMeshDraw {
                    vertex_buffer,
                    vertex_count: verts.len(),
                    base_color_factor: mesh.base_color,
                    emissive: mesh.emissive,
                    texture,
                    normal_texture,
                    is_blend: mesh.is_blend,
                    camera_dist_sq: d[0] * d[0] + d[1] * d[1] + d[2] * d[2],
                }
            })
            .collect();
        let real_game_meshes_opt =
            if real_game_meshes.is_empty() { None } else { Some(real_game_meshes.as_slice()) };

        let aspect = width as f32 / height.max(1) as f32;
        let far = if is_real_game { 500.0 } else { 100.0 };
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
        let view = if is_real_game {
            look_at(self.camera_position, center, up)
        } else {
            look_at([0.0, 3.0, 10.0], [0.0, 0.0, 0.0], [0.0, 1.0, 0.0])
        };
        let proj = perspective(if is_real_game { 60.0_f32 } else { 45.0 }.to_radians(), aspect, 0.05, far);
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

        // Collected into an owned Vec (not a live query/World borrow) so
        // draw_scene can be called twice — once per camera — without
        // re-extracting in between. Used to also have a toy "patrol" World
        // fallback here (a hand-placed ECS demo, ticked/extracted only when
        // NOT in real-game mode) — removed once real_game=false was
        // confirmed unreachable in the shipped app; see git history if it's
        // ever worth resurrecting for a standalone demo/probe build again.
        let extracted: Vec<super::skinned_actor::SkinnedActorDraw> = real_game_extracted;

        // Sun's shadow-map camera: an orthographic box (directional lights
        // have no single position/frustum of their own, unlike the point
        // light) recentered on the player every frame rather than fit to
        // the whole arena — see SHADOW_HALF_EXTENT/SHADOW_DISTANCE's own
        // doc comments. `sun_dir` is "direction the light travels toward"
        // (LightUniform's own doc comment), so the light's "eye" sits back
        // along `-sun_dir` from the camera (where the light is coming
        // from) looking forward along `+sun_dir` toward it.
        let sun_dir = {
            let d = bevy::math::Vec3::from_array(light.sun_dir);
            if d.length_squared() > 1e-6 { d.normalize() } else { bevy::math::Vec3::new(-0.33, -0.67, -0.33).normalize() }
        };
        // Avoid an (up parallel to forward) degenerate look_at when the sun
        // is nearly straight overhead/underfoot.
        let shadow_up = if sun_dir.y.abs() > 0.99 { [0.0, 0.0, 1.0] } else { [0.0, 1.0, 0.0] };
        let shadow_center = self.camera_position;
        let shadow_eye = [
            shadow_center[0] - sun_dir.x * SHADOW_DISTANCE,
            shadow_center[1] - sun_dir.y * SHADOW_DISTANCE,
            shadow_center[2] - sun_dir.z * SHADOW_DISTANCE,
        ];
        let shadow_view = look_at(shadow_eye, shadow_center, shadow_up);
        let shadow_proj = orthographic(
            -SHADOW_HALF_EXTENT,
            SHADOW_HALF_EXTENT,
            -SHADOW_HALF_EXTENT,
            SHADOW_HALF_EXTENT,
            0.1,
            SHADOW_DISTANCE * 2.0,
        );
        let light_view_projection = mat_mul(shadow_proj, shadow_view);

        // Pass 0: sun shadow map, depth-only — must run before Pass 1/2,
        // both of which sample `self.shadow_map` while shading. See
        // draw_shadow_casters' own doc comment.
        {
            let shadow_pass_descriptor = RenderPassDescriptor::new();
            let shadow_depth = shadow_pass_descriptor.depth_attachment().unwrap();
            shadow_depth.set_texture(Some(&self.shadow_map));
            shadow_depth.set_load_action(MTLLoadAction::Clear);
            shadow_depth.set_store_action(MTLStoreAction::Store);
            shadow_depth.set_clear_depth(1.0);

            let shadow_command_buffer = self.queue.new_command_buffer();
            let shadow_encoder = shadow_command_buffer.new_render_command_encoder(shadow_pass_descriptor);
            shadow_encoder.set_depth_stencil_state(&self.shadow_depth_state);
            self.draw_shadow_casters(shadow_encoder, light_view_projection, &extracted, real_game_meshes_opt);
            shadow_encoder.end_encoding();
            shadow_command_buffer.commit();
        }

        // The arena's overhead PointLight's own shadow camera — unlike the
        // sun, a point light has a REAL position (light.position), so this
        // is a genuine perspective frustum from that position toward the
        // player, not an orthographic box. A fixed wide FOV (90°) rather
        // than one fit tightly to SHADOW_HALF_EXTENT at the actual
        // light-to-player distance — simpler, and safely covers the
        // relevant area regardless of exactly how high the light sits.
        let point_light_pos = light.position;
        let point_to_center = (bevy::math::Vec3::from_array(shadow_center) - bevy::math::Vec3::from_array(point_light_pos)).normalize_or_zero();
        let point_up = if point_to_center.y.abs() > 0.99 { [0.0, 0.0, 1.0] } else { [0.0, 1.0, 0.0] };
        let point_light_view = look_at(point_light_pos, shadow_center, point_up);
        let point_light_proj = perspective(90.0_f32.to_radians(), 1.0, 0.1, light.range.max(SHADOW_DISTANCE * 2.0));
        let point_light_view_projection = mat_mul(point_light_proj, point_light_view);

        // Pass 0b: point light shadow map, depth-only — same shape as
        // Pass 0 above, into its own texture (see shadow_map_point's own
        // field doc comment for why the sun and point light need separate
        // shadow maps, not one).
        {
            let point_shadow_pass_descriptor = RenderPassDescriptor::new();
            let point_shadow_depth = point_shadow_pass_descriptor.depth_attachment().unwrap();
            point_shadow_depth.set_texture(Some(&self.shadow_map_point));
            point_shadow_depth.set_load_action(MTLLoadAction::Clear);
            point_shadow_depth.set_store_action(MTLStoreAction::Store);
            point_shadow_depth.set_clear_depth(1.0);

            let point_shadow_command_buffer = self.queue.new_command_buffer();
            let point_shadow_encoder = point_shadow_command_buffer.new_render_command_encoder(point_shadow_pass_descriptor);
            point_shadow_encoder.set_depth_stencil_state(&self.shadow_depth_state);
            self.draw_shadow_casters(point_shadow_encoder, point_light_view_projection, &extracted, real_game_meshes_opt);
            point_shadow_encoder.end_encoding();
            point_shadow_command_buffer.commit();
        }

        // Pass 1: scope camera into the offscreen texture — skipped
        // entirely in real-game mode while not aiming (see `show_scope`),
        // both to avoid the wasted render pass and so the PIP quad below
        // doesn't composite a stale/irrelevant frame over the main view.
        if show_scope {
            let scope_pass_descriptor = RenderPassDescriptor::new();
            let scope_color = scope_pass_descriptor.color_attachments().object_at(0).unwrap();
            if msaa_enabled {
                // Render into the multisampled texture, resolve straight
                // into scope_color_texture (what quad_fragment actually
                // samples) — see MSAA_SAMPLE_COUNT's own doc comment.
                scope_color.set_texture(Some(&self.scope_color_texture_msaa));
                scope_color.set_resolve_texture(Some(&self.scope_color_texture));
                scope_color.set_store_action(MTLStoreAction::MultisampleResolve);
            } else {
                scope_color.set_texture(Some(&self.scope_color_texture));
                scope_color.set_store_action(MTLStoreAction::Store);
            }
            scope_color.set_load_action(MTLLoadAction::Clear);
            scope_color.set_clear_color(MTLClearColor::new(0.05, 0.05, 0.05, 1.0));
            let scope_depth = scope_pass_descriptor.depth_attachment().unwrap();
            scope_depth.set_texture(Some(if msaa_enabled { &self.scope_depth_texture } else { &self.scope_depth_texture_no_msaa }));
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
                real_game_meshes_opt,
                msaa_enabled,
                light_view_projection,
                point_light_view_projection,
                self.camera_position,
            );
            scope_encoder.end_encoding();
            scope_command_buffer.commit();
        }

        // Pass 2: main camera straight to the drawable, then the scope
        // texture composited as a picture-in-picture quad on top.
        let pass_descriptor = RenderPassDescriptor::new();
        let color_attachment = pass_descriptor.color_attachments().object_at(0).unwrap();
        if let Some(msaa_color_texture) = &msaa_color_texture {
            // Render into the multisampled texture, resolve straight into
            // the real drawable — see MSAA_SAMPLE_COUNT's own doc comment.
            color_attachment.set_texture(Some(msaa_color_texture));
            color_attachment.set_resolve_texture(Some(texture));
            color_attachment.set_store_action(MTLStoreAction::MultisampleResolve);
        } else {
            color_attachment.set_texture(Some(texture));
            color_attachment.set_store_action(MTLStoreAction::Store);
        }
        color_attachment.set_load_action(MTLLoadAction::Clear);
        // No skybox in this renderer at all (see this comment's own history
        // — was a hardcoded debug magenta, meant to make any undrawn area
        // obvious during development, that ended up shipping as-is and
        // visibly showing through wherever the camera can see past the
        // arena's own geometry, e.g. over the walls or through any gap).
        // A flat neutral near-black instead reads as "shadow/void" rather
        // than a glaring debug color — not a real sky, but close enough to
        // the arena's own dim ambient tone (spawn_test_arena's wall/floor
        // base_color, crate::lib) that a stray gap doesn't stand out.
        color_attachment.set_clear_color(MTLClearColor::new(0.05, 0.055, 0.07, 1.0));

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
            real_game_meshes_opt,
            msaa_enabled,
            light_view_projection,
            point_light_view_projection,
            self.camera_position,
        );

        if show_scope {
            let quad_pipeline_ref = if msaa_enabled { &self.quad_pipeline } else { &self.quad_pipeline_no_msaa };
            if let Some(quad_pipeline) = quad_pipeline_ref {
                // Real-game mode: builds the real ScopeLens circle's own 4
                // world-space corners and projects each individually — see
                // build_projected_quad's own doc comment for the full
                // "this replaced two earlier flat-quad approximations"
                // story (dead-center fixed position, then a projected
                // center + one 2D roll angle — neither could represent a
                // true horizontal turn of the glass or real perspective-
                // correct near/far scaling, only build_projected_quad's
                // real per-corner projection can). SCOPE_LENS_RADIUS
                // mirrors crate::lib's own SCOPE_LENS_SIZE / 2.0 — same
                // "small duplicated constant, not shared across the wgpu/
                // Metal boundary" reasoning as kShininess/kSpecularStrength
                // above. world_rotation composes the real camera's own
                // world rotation with the lens's LOCAL one (ChildOf
                // (camera) — see lens_local_rotation's own doc comment on
                // ScopeCameraSnapshot), mirroring exactly how Bevy itself
                // derives a child's GlobalTransform. The demo probe keeps
                // its original fixed top-right box, which has no real "is
                // this actually where the lens sits" state to match.
                const SCOPE_LENS_RADIUS: f32 = 0.015;
                let real_game_quad_buffer = is_real_game.then(|| {
                    let camera_rotation = bevy::math::Quat::from_array(self.real_game_camera_rotation);
                    let lens_local_rotation = bevy::math::Quat::from_array(self.real_game_scope_lens_local_rotation);
                    let world_rotation = camera_rotation * lens_local_rotation;
                    build_projected_quad(
                        &self.device,
                        view_projection,
                        self.real_game_scope_lens_world_pos,
                        world_rotation,
                        SCOPE_LENS_RADIUS,
                    )
                });
                let quad_buffer = real_game_quad_buffer.as_ref().unwrap_or(&self.quad_vertex_buffer);
                encoder.set_render_pipeline_state(quad_pipeline);
                encoder.set_vertex_buffer(0, Some(quad_buffer), 0);
                encoder.set_fragment_texture(0, Some(&self.scope_color_texture));
                encoder.set_fragment_sampler_state(0, Some(&self.sponza_sampler));
                encoder.draw_primitives(MTLPrimitiveType::Triangle, 0, 6);
            }
        }

        // bevy_ui overlay: zombie health bars + floating damage numbers.
        // Drawn last, screen-space, straight over the finished 3D frame —
        // see UI_SHADER_SOURCE's and prime_camera_viewport's own doc
        // comments for why these were never visible before this pass.
        let ui_pipeline_ref = if msaa_enabled { &self.ui_pipeline } else { &self.ui_pipeline_no_msaa };
        if let Some(ui_pipeline) = ui_pipeline_ref {
            // See ui_depth_state's own construction comment (in `new()`) —
            // without this, the overlay inherits the 3D scene's own
            // Less-compare/write-enabled depth state from `self.depth_state`
            // (set once, above, for the whole render pass) and gets
            // intermittently depth-tested away depending on what real scene
            // geometry happens to be behind it.
            encoder.set_depth_stencil_state(&self.ui_depth_state);
            if !self.ui_health_bars.is_empty()
                || !self.ui_floating_damage.is_empty()
                || !self.ui_touch_buttons.is_empty()
                || self.ui_touch_joystick.is_some()
                || self.ui_scope_zoom_slider.is_some()
                || !self.ui_hud_texts.is_empty()
                || !self.ui_crosshair_lines.is_empty()
                || self.ui_settings_menu_panel.is_some()
                || !self.ui_settings_menu_buttons.is_empty()
                || !self.ui_settings_menu_texts.is_empty()
                || self.ui_game_over_panel.is_some()
                || !self.ui_game_over_buttons.is_empty()
                || !self.ui_game_over_texts.is_empty()
                || self.ui_map_select_panel.is_some()
                || !self.ui_map_select_buttons.is_empty()
                || !self.ui_map_select_texts.is_empty()
            {
                let (view_w, view_h) = self.view_size_points();
                encoder.set_render_pipeline_state(ui_pipeline);
                encoder.set_fragment_sampler_state(0, Some(&self.ui_sampler));

                const FULL_UV: [[f32; 2]; 4] = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];
                for bar in &self.ui_health_bars {
                    let [x, y] = bar.screen_pos;
                    let w = crate::ZOMBIE_HEALTH_BAR_WIDTH_PX;
                    let h = crate::ZOMBIE_HEALTH_BAR_HEIGHT_PX;
                    // Background — same translucent black
                    // spawn_zombie_health_bar (crate::lib) gives the real
                    // bevy_ui Node.
                    let bg = ndc_ui_rect(x, y, w, h, view_w, view_h, FULL_UV);
                    draw_ui_quad(encoder, &bg, [0.0, 0.0, 0.0, 0.5], &self.white_texture);
                    // Fill — same green ZombieHealthBarFill uses, scaled to
                    // `fraction` exactly like that Node's own Val::Percent
                    // width.
                    let fill = ndc_ui_rect(x, y, w * bar.fraction, h, view_w, view_h, FULL_UV);
                    draw_ui_quad(encoder, &fill, [0.2, 0.9, 0.2, 1.0], &self.white_texture);
                }

                for dmg in &self.ui_floating_damage {
                    let [x, y] = dmg.screen_pos;
                    let scale = dmg.font_size / GLYPH_H as f32;
                    let mut cursor_x = x;
                    for ch in dmg.text.chars() {
                        let Some(index) = glyph_index(ch) else { continue };
                        let (u0, u1) = glyph_uv_rect(index);
                        let uv = [[u0, 1.0], [u1, 1.0], [u1, 0.0], [u0, 0.0]];
                        let glyph_w = GLYPH_W as f32 * scale;
                        let glyph_h = GLYPH_H as f32 * scale;
                        let quad = ndc_ui_rect(cursor_x, y, glyph_w, glyph_h, view_w, view_h, uv);
                        draw_ui_quad(encoder, &quad, dmg.color, &self.font_atlas_texture);
                        cursor_x += glyph_w + scale;
                    }
                }

                // Touch controls: joystick first (drawn under the buttons,
                // matching the bevy_ui path's own spawn/z-order — see
                // spawn_touch_controls, crate::lib) then the buttons
                // themselves. Both flat rects, no rounded-corner mask (the
                // real bevy_ui nodes are circles/rounded-rects via
                // BorderRadius — UI_SHADER_SOURCE has no SDF corner
                // rounding yet, same simplification the health bars above
                // already accept for their own rounded corners).
                if let Some(joystick) = &self.ui_touch_joystick {
                    let [bx, by] = joystick.base_top_left;
                    let [bw, bh] = joystick.base_size;
                    let base = ndc_ui_rect(bx, by, bw, bh, view_w, view_h, FULL_UV);
                    draw_ui_quad(encoder, &base, [1.0, 1.0, 1.0, 0.3], &self.white_texture);
                    let [kx, ky] = joystick.knob_top_left;
                    let [kw, kh] = joystick.knob_size;
                    let knob = ndc_ui_rect(kx, ky, kw, kh, view_w, view_h, FULL_UV);
                    draw_ui_quad(encoder, &knob, [1.0, 1.0, 1.0, 0.55], &self.white_texture);
                }

                // Scope-zoom slider — same track-then-handle draw order/
                // colors as the joystick above (matching spawn_touch_
                // controls' own base-then-knob alpha values: 0.3 track,
                // 0.55 handle). Zero-sized (drawing nothing visible) while
                // not ADS'd — see ScopeZoomSliderSnapshot's own doc
                // comment for why that needs no separate visibility check
                // here.
                if let Some(slider) = &self.ui_scope_zoom_slider {
                    let [tx, ty] = slider.track_top_left;
                    let [tw, th] = slider.track_size;
                    let track = ndc_ui_rect(tx, ty, tw, th, view_w, view_h, FULL_UV);
                    draw_ui_quad(encoder, &track, [1.0, 1.0, 1.0, 0.3], &self.white_texture);
                    let [hx, hy] = slider.handle_top_left;
                    let [hw, hh] = slider.handle_size;
                    let handle = ndc_ui_rect(hx, hy, hw, hh, view_w, view_h, FULL_UV);
                    draw_ui_quad(encoder, &handle, [1.0, 1.0, 1.0, 0.55], &self.white_texture);
                }

                for button in &self.ui_touch_buttons {
                    let [x, y] = button.top_left;
                    let [w, h] = button.size;
                    // Pressed buttons brighten (alpha bumped, same "press
                    // feedback" idea a native UIButton highlight gives) —
                    // there's no separate pressed-state color to extract,
                    // so this just bumps whatever color BackgroundColor
                    // already carries.
                    let mut color = button.color;
                    if button.pressed {
                        color[3] = (color[3] * 1.6).min(1.0);
                    }
                    let quad = ndc_ui_rect(x, y, w, h, view_w, view_h, FULL_UV);
                    draw_ui_quad(encoder, &quad, color, &self.white_texture);

                    if !button.label.is_empty() {
                        let font_size = 10.0_f32.min(h * 0.3);
                        let scale = font_size / GLYPH_H as f32;
                        let text_w: f32 = button.label.chars().map(|_| GLYPH_W as f32 * scale + scale).sum();
                        let mut cursor_x = x + (w - text_w) / 2.0;
                        let text_y = y + (h - GLYPH_H as f32 * scale) / 2.0;
                        for ch in button.label.chars() {
                            let Some(index) = glyph_index(ch) else { continue };
                            let (u0, u1) = glyph_uv_rect(index);
                            let uv = [[u0, 1.0], [u1, 1.0], [u1, 0.0], [u0, 0.0]];
                            let glyph_w = GLYPH_W as f32 * scale;
                            let glyph_h = GLYPH_H as f32 * scale;
                            let glyph_quad = ndc_ui_rect(cursor_x, text_y, glyph_w, glyph_h, view_w, view_h, uv);
                            draw_ui_quad(encoder, &glyph_quad, [1.0, 1.0, 1.0, 1.0], &self.font_atlas_texture);
                            cursor_x += glyph_w + scale;
                        }
                    }
                }

                // Crosshair — 4 plain rects, no label. Same "collapses to a
                // zero-size quad when its real Node is Display::None"
                // free ride touch controls' idle-hide gets — see
                // CrosshairLineSnapshot's own doc comment.
                for line in &self.ui_crosshair_lines {
                    let [x, y] = line.top_left;
                    let [w, h] = line.size;
                    let quad = ndc_ui_rect(x, y, w, h, view_w, view_h, FULL_UV);
                    draw_ui_quad(encoder, &quad, line.color, &self.white_texture);
                }

                // HP/ammo/ping readouts + (below) the settings menu's own
                // text — see draw_hud_text_list's own doc comment.
                draw_hud_text_list(encoder, &self.ui_hud_texts, view_w, view_h, &self.font_atlas_texture);

                // Settings menu + GAME OVER + map-select overlay: same
                // generic "panel + buttons + text" draw for all three — see
                // draw_panel's own doc comment.
                draw_panel(
                    encoder,
                    &self.ui_settings_menu_panel,
                    &self.ui_settings_menu_buttons,
                    &self.ui_settings_menu_texts,
                    view_w,
                    view_h,
                    &self.white_texture,
                    &self.font_atlas_texture,
                );
                draw_panel(
                    encoder,
                    &self.ui_game_over_panel,
                    &self.ui_game_over_buttons,
                    &self.ui_game_over_texts,
                    view_w,
                    view_h,
                    &self.white_texture,
                    &self.font_atlas_texture,
                );
                draw_panel(
                    encoder,
                    &self.ui_map_select_panel,
                    &self.ui_map_select_buttons,
                    &self.ui_map_select_texts,
                    view_w,
                    view_h,
                    &self.white_texture,
                    &self.font_atlas_texture,
                );
            }
        }

        encoder.end_encoding();
        command_buffer.present_drawable(drawable);
        command_buffer.commit();
    }
}

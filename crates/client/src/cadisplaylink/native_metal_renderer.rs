
use bevy::asset::AssetId;
use bevy::image::Image;
use bevy::mesh::Mesh;
use core_graphics::geometry::{CGRect, CGSize};
use metal::foreign_types::ForeignType;
use metal::{
    CAMetalLayer, CommandQueue, CompileOptions, Device, MTLClearColor, MTLIndexType, MTLLoadAction, MTLPixelFormat,
    MTLPrimitiveType, MTLResourceOptions, MTLScissorRect, MTLStoreAction, MetalLayer, RenderPassDescriptor,
    RenderPipelineDescriptor, RenderPipelineState,
};
use objc::{runtime::Object, *};

use super::metal_math::{look_at, mat_mul, orthographic, perspective, transform_point};
use super::native_game_app::build_headless_game_ecs_app;
use super::native_game_extract::{extract_render_snapshot, prime_camera_viewport, ExtractedMesh, LightSnapshot, SunSnapshot};

#[derive(Copy, Clone)]
struct LightUniform {
    position: [f32; 3],
    range: f32,

    color: [f32; 4],

    sun_dir: [f32; 3],
    _sun_pad: f32,
    sun_color: [f32; 4],
}

impl LightUniform {

    fn from_snapshots(light: Option<&LightSnapshot>, sun: Option<&SunSnapshot>, sun_fraction: f32, point_fraction: f32) -> Self {
        let (position, range, color_base) = match light {
            Some(l) => (l.position, l.range, [l.color[0], l.color[1], l.color[2], 0.0]),
            None => (FALLBACK_LIGHT.position, FALLBACK_LIGHT.range, FALLBACK_LIGHT.color),
        };
        let color = [color_base[0] * point_fraction, color_base[1] * point_fraction, color_base[2] * point_fraction, color_base[3]];
        let (sun_dir, sun_color_base) = match sun {
            Some(s) => (s.direction, [s.color[0], s.color[1], s.color[2], 0.0]),
            None => (FALLBACK_LIGHT.sun_dir, FALLBACK_LIGHT.sun_color),
        };
        let sun_color = [
            sun_color_base[0] * sun_fraction,
            sun_color_base[1] * sun_fraction,
            sun_color_base[2] * sun_fraction,
            sun_color_base[3],
        ];
        LightUniform { position, range, color, sun_dir, _sun_pad: 0.0, sun_color }
    }
}

const FALLBACK_LIGHT: LightUniform = LightUniform {
    position: [400.0, 800.0, 400.0],
    range: 4000.0,
    color: [1.0, 1.0, 1.0, 0.0],
    sun_dir: [-0.33, -0.67, -0.33],
    _sun_pad: 0.0,
    sun_color: [1.0, 1.0, 1.0, 0.0],
};

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
    // Every mesh caches its vertex buffer in LOCAL space, so this places the
    // instance in the world each draw.
    float4x4 modelMatrix;
    // Inverse transpose of modelMatrix: normals skew under non-uniform scale,
    // which 140 of the desert's nodes carry.
    float4x4 normalMatrix;
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
    float4 worldPos4 = uniforms.modelMatrix * float4(float3(v.position), 1.0);
    out.position = uniforms.viewProjection * worldPos4;
    out.worldPos = worldPos4.xyz;
    out.normal = (uniforms.normalMatrix * float4(float3(v.normal), 0.0)).xyz;
    out.uv = float2(v.uv);
    // xyz rotates like the normal above (w=0 drops translation); w itself
    // is glTF's handedness sign, not a spatial component, so it carries
    // straight through unchanged — same convention bake_static_mesh_
    // vertices/bake_local_mesh_vertices already use for tangent.w.
    float3 rotatedTangent = (uniforms.modelMatrix * float4(v.tangent.xyz, 0.0)).xyz;
    out.tangent = float4(rotatedTangent, v.tangent.w);
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

    model_matrix: [[f32; 4]; 4],
    normal_matrix: [[f32; 4]; 4],
}

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

const SHADOW_SHADER_SOURCE: &str = r#"
#include <metal_stdlib>
using namespace metal;

struct SponzaVertex {
    packed_float3 position;
    packed_float3 normal;
    packed_float2 uv;
    // Unused here (this pass only needs position) but MUST stay present —
    // this struct's buffer(0) is the exact same GltfVertex-laid-out buffer
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
    // Same role as SPONZA_SHADER_SOURCE's own Uniforms::modelMatrix —
    // identity for CPU-baked-to-world-space meshes (walls/gun), the real
    // per-instance transform for cars (cached LOCAL-space vertex buffer).
    // SkinnedShadowUniforms above already has this exact field for the
    // same reason (zombies' shadow pass).
    float4x4 modelMatrix;
};

vertex float4 shadow_vertex_sponza(uint vid [[vertex_id]],
                                    const device SponzaVertex *verts [[buffer(0)]],
                                    constant SponzaShadowUniforms &uniforms [[buffer(1)]]) {
    SponzaVertex v = verts[vid];
    return uniforms.lightViewProjection * uniforms.modelMatrix * float4(float3(v.position), 1.0);
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
    model_matrix: [[f32; 4]; 4],
}

#[repr(C)]
#[derive(Copy, Clone)]
struct SkinnedShadowUniforms {
    light_view_projection: [[f32; 4]; 4],
    model_matrix: [[f32; 4]; 4],
}

const SHADOW_MAP_SIZE: u64 = 2048;

const SHADOW_MAP_SIZES: [u64; 6] = [512, 512, 1024, 1536, 2048, 3072];

const RENDER_SCALE_MIN_PX: u64 = 64;

const SHADOW_HALF_EXTENT: f32 = 22.0;
const SHADOW_DISTANCE: f32 = 40.0;

const SCOPE_TEXTURE_SIZE: u64 = 512;

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

const BLIT_SHADER_SOURCE: &str = r#"
#include <metal_stdlib>
using namespace metal;

struct BlitVertex {
    packed_float2 position;
    packed_float2 uv;
};

struct BlitVertexOut {
    float4 position [[position]];
    float2 uv;
};

vertex BlitVertexOut blit_vertex(uint vid [[vertex_id]],
                                  const device BlitVertex *verts [[buffer(0)]]) {
    BlitVertexOut out;
    BlitVertex v = verts[vid];
    out.position = float4(v.position, 0.0, 1.0);
    out.uv = v.uv;
    return out;
}

fragment float4 blit_fragment(BlitVertexOut in [[stage_in]],
                               texture2d<float> tex [[texture(0)]],
                               sampler texSampler [[sampler(0)]]) {
    return tex.sample(texSampler, in.uv);
}
"#;

#[repr(C)]
#[derive(Copy, Clone)]
struct BlitVertex {
    position: [f32; 2],
    uv: [f32; 2],
}

const GLYPH_W: usize = 5;
const GLYPH_H: usize = 7;
const GLYPH_COUNT: usize = 43;

const GLYPH_BITMAPS: [[u8; GLYPH_H]; GLYPH_COUNT] = [
    [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110],
    [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
    [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111],
    [0b11111, 0b00010, 0b00100, 0b00010, 0b00001, 0b10001, 0b01110],
    [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
    [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110],
    [0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110],
    [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
    [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
    [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100],
    [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b01100, 0b01100],
    [0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b00000, 0b00000],
    [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
    [0b01111, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b01111],
    [0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110],
    [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111],
    [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
    [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
    [0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100],
    [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
    [0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001],
    [0b10001, 0b11001, 0b10101, 0b10101, 0b10011, 0b10001, 0b10001],
    [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
    [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000],
    [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001],
    [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110],
    [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
    [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
    [0b00001, 0b00010, 0b00100, 0b01000, 0b00100, 0b00010, 0b00001],
    [0b10000, 0b01000, 0b00100, 0b00010, 0b00100, 0b01000, 0b10000],
    [0b00000, 0b00000, 0b11111, 0b00000, 0b11111, 0b00000, 0b00000],
    [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000],
    [0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111],
    [0b00000, 0b00000, 0b01100, 0b01100, 0b00000, 0b01100, 0b01100],
    [0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110],
    [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000],
    [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001],
    [0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101],
    [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100],
    [0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b10101, 0b01010],
    [0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001],
    [0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100],
    [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111],
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

fn glyph_uv_rect(index: usize) -> (f32, f32) {
    let u0 = index as f32 / GLYPH_COUNT as f32;
    let u1 = (index + 1) as f32 / GLYPH_COUNT as f32;
    (u0, u1)
}

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

pub struct NativeMetalRenderer {
    device: Device,
    layer: MetalLayer,
    queue: CommandQueue,
    depth_state: metal::DepthStencilState,

    blend_depth_state: metal::DepthStencilState,

    ui_depth_state: metal::DepthStencilState,

    msaa_targets: super::metal_msaa::MsaaTargets,

    msaa_enabled: bool,

    shadow_quality: u8,

    render_resolution_scale: f32,

    zombie_texture_quality: u8,

    scene_color_size: (u64, u64),
    scene_color_texture: metal::Texture,

    blit_pipeline: Option<RenderPipelineState>,
    blit_vertex_buffer: metal::Buffer,
    sponza_pipeline: Option<RenderPipelineState>,
    sponza_pipeline_no_msaa: Option<RenderPipelineState>,
    sponza_sampler: metal::SamplerState,

    shadow_map: metal::Texture,

    shadow_map_point: metal::Texture,
    shadow_pipeline_sponza: Option<RenderPipelineState>,
    shadow_pipeline_skinned: Option<RenderPipelineState>,

    shadow_depth_state: metal::DepthStencilState,

    shadow_sampler: metal::SamplerState,
    white_texture: metal::Texture,

    flat_normal_texture: metal::Texture,
    camera_position: [f32; 3],
    camera_yaw: f32,
    camera_pitch: f32,
    skinned_pipeline: Option<RenderPipelineState>,
    skinned_pipeline_no_msaa: Option<RenderPipelineState>,

    skinned_models: super::skinned_actor::SkinnedModelRegistry,
    scope_color_texture: metal::Texture,

    scope_color_texture_msaa: metal::Texture,
    scope_depth_texture: metal::Texture,

    scope_depth_texture_no_msaa: metal::Texture,
    quad_pipeline: Option<RenderPipelineState>,
    quad_pipeline_no_msaa: Option<RenderPipelineState>,
    quad_vertex_buffer: metal::Buffer,
    start_time: std::time::Instant,


    real_game_app: Option<bevy::app::App>,

    real_game_textures: std::collections::HashMap<AssetId<Image>, metal::Texture>,

    gpu_transformed_mesh_cache: std::collections::HashMap<AssetId<Mesh>, (metal::Buffer, usize)>,

    real_game_camera_rotation: [f32; 4],

    real_game_ads_blend: f32,
    real_game_scope_zoom: f32,
    real_game_wall_raise: f32,

    real_game_scope_eye: [f32; 3],
    real_game_scope_rotation: [f32; 4],
    real_game_scope_active: bool,
    real_game_scope_fov: f32,

    real_game_scope_lens_world_pos: [f32; 3],

    real_game_scope_lens_local_rotation: [f32; 4],

    real_game_light: LightUniform,

    ui_pipeline: Option<RenderPipelineState>,
    ui_pipeline_no_msaa: Option<RenderPipelineState>,
    ui_sampler: metal::SamplerState,
    font_atlas_texture: metal::Texture,


    ui_generic_nodes: Vec<super::native_game_extract::GenericUiNode>,
}

fn normal_matrix_for(model: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    bevy::math::Mat4::from_cols_array_2d(&model).inverse().transpose().to_cols_array_2d()
}

fn bake_local_mesh_vertices(mesh: &ExtractedMesh) -> Vec<super::gltf_scene::GltfVertex> {
    let mut verts = Vec::new();
    let vertex_at = |idx: usize| -> super::gltf_scene::GltfVertex {
        let position = mesh.positions[idx];
        let normal = if mesh.normals.is_empty() { [0.0, 1.0, 0.0] } else { mesh.normals[idx] };
        let uv = if mesh.uvs.is_empty() { [0.0, 0.0] } else { mesh.uvs[idx] };
        let tangent = if mesh.tangents.is_empty() { [1.0, 0.0, 0.0, 1.0] } else { mesh.tangents[idx] };
        super::gltf_scene::GltfVertex { position, normal, uv, tangent }
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

fn ndc_ui_rect_rotated(center_x: f32, center_y: f32, w: f32, h: f32, rotation: f32, view_w: f32, view_h: f32, uv: [[f32; 2]; 4]) -> [UiVertex; 6] {
    let (sin, cos) = rotation.sin_cos();
    let hw = w / 2.0;
    let hh = h / 2.0;
    let to_ndc = |lx: f32, ly: f32| -> [f32; 2] {
        let px = center_x + lx * cos - ly * sin;
        let py = center_y + lx * sin + ly * cos;
        [(px / view_w) * 2.0 - 1.0, 1.0 - (py / view_h) * 2.0]
    };
    let bottom_left = to_ndc(-hw, hh);
    let bottom_right = to_ndc(hw, hh);
    let top_right = to_ndc(hw, -hh);
    let top_left = to_ndc(-hw, -hh);
    [
        UiVertex { position: bottom_left, uv: uv[0] },
        UiVertex { position: bottom_right, uv: uv[1] },
        UiVertex { position: top_right, uv: uv[2] },
        UiVertex { position: bottom_left, uv: uv[0] },
        UiVertex { position: top_right, uv: uv[2] },
        UiVertex { position: top_left, uv: uv[3] },
    ]
}

fn draw_ui_quad(encoder: &metal::RenderCommandEncoderRef, verts: &[UiVertex; 6], tint: [f32; 4], texture: &metal::Texture) {
    encoder.set_vertex_bytes(0, std::mem::size_of::<[UiVertex; 6]>() as u64, verts.as_ptr() as *const _);
    let uniforms = UiUniforms { tint };
    encoder.set_fragment_bytes(1, std::mem::size_of::<UiUniforms>() as u64, &uniforms as *const _ as *const _);
    encoder.set_fragment_texture(0, Some(texture));
    encoder.draw_primitives(MTLPrimitiveType::Triangle, 0, 6);
}

fn draw_ui_verts(encoder: &metal::RenderCommandEncoderRef, verts: &[UiVertex], primitive: MTLPrimitiveType, tint: [f32; 4], texture: &metal::Texture) {
    if verts.is_empty() {
        return;
    }
    encoder.set_vertex_bytes(0, std::mem::size_of_val(verts) as u64, verts.as_ptr() as *const _);
    let uniforms = UiUniforms { tint };
    encoder.set_fragment_bytes(1, std::mem::size_of::<UiUniforms>() as u64, &uniforms as *const _ as *const _);
    encoder.set_fragment_texture(0, Some(texture));
    encoder.draw_primitives(primitive, 0, verts.len() as u64);
}

/// Angle 0 = +X, increasing clockwise (+Y is down). `0.0, TAU` sweeps a full circle.
fn arc_fill_verts(center: [f32; 2], radius: f32, start_angle: f32, sweep: f32, view_w: f32, view_h: f32, segments: u32) -> Vec<UiVertex> {
    let to_ndc = |x: f32, y: f32| -> [f32; 2] { [(x / view_w) * 2.0 - 1.0, 1.0 - (y / view_h) * 2.0] };
    let center_ndc = to_ndc(center[0], center[1]);
    let point_at = |theta: f32| {
        let (sin, cos) = theta.sin_cos();
        to_ndc(center[0] + cos * radius, center[1] + sin * radius)
    };
    let mut verts = Vec::with_capacity(segments as usize * 3);
    let mut prev = point_at(start_angle);
    for i in 1..=segments {
        let edge = point_at(start_angle + sweep * (i as f32 / segments as f32));
        verts.push(UiVertex { position: center_ndc, uv: [0.5, 0.5] });
        verts.push(UiVertex { position: prev, uv: [0.5, 0.5] });
        verts.push(UiVertex { position: edge, uv: [0.5, 0.5] });
        prev = edge;
    }
    verts
}

fn arc_stroke_verts(center: [f32; 2], outer_radius: f32, inner_radius: f32, start_angle: f32, sweep: f32, view_w: f32, view_h: f32, segments: u32) -> Vec<UiVertex> {
    let to_ndc = |x: f32, y: f32| -> [f32; 2] { [(x / view_w) * 2.0 - 1.0, 1.0 - (y / view_h) * 2.0] };
    let mut verts = Vec::with_capacity((segments as usize + 1) * 2);
    for i in 0..=segments {
        let theta = start_angle + sweep * (i as f32 / segments as f32);
        let (sin, cos) = theta.sin_cos();
        let outer = to_ndc(center[0] + cos * outer_radius, center[1] + sin * outer_radius);
        let inner = to_ndc(center[0] + cos * inner_radius, center[1] + sin * inner_radius);
        verts.push(UiVertex { position: outer, uv: [0.5, 0.5] });
        verts.push(UiVertex { position: inner, uv: [0.5, 0.5] });
    }
    verts
}

/// Corner (arc-center, start-angle) pairs, top-left/top-right/bottom-right/bottom-left, each a quarter turn.
fn rounded_rect_corners(x: f32, y: f32, w: f32, h: f32, r: f32) -> [([f32; 2], f32); 4] {
    use std::f32::consts::PI;
    [
        ([x + r, y + r], PI),
        ([x + w - r, y + r], -PI / 2.0),
        ([x + w - r, y + h - r], 0.0),
        ([x + r, y + h - r], PI / 2.0),
    ]
}

fn draw_rounded_rect_fill(encoder: &metal::RenderCommandEncoderRef, center: [f32; 2], size: [f32; 2], radius: f32, color: [f32; 4], view_w: f32, view_h: f32, white_texture: &metal::Texture) {
    const FULL_UV: [[f32; 2]; 4] = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];
    const CORNER_SEGMENTS: u32 = 10;
    let [cx, cy] = center;
    let [w, h] = size;
    let r = radius.min(w / 2.0).min(h / 2.0);
    let x = cx - w / 2.0;
    let y = cy - h / 2.0;

    let middle = ndc_ui_rect(x + r, y, w - 2.0 * r, h, view_w, view_h, FULL_UV);
    draw_ui_quad(encoder, &middle, color, white_texture);
    let left = ndc_ui_rect(x, y + r, r, h - 2.0 * r, view_w, view_h, FULL_UV);
    draw_ui_quad(encoder, &left, color, white_texture);
    let right = ndc_ui_rect(x + w - r, y + r, r, h - 2.0 * r, view_w, view_h, FULL_UV);
    draw_ui_quad(encoder, &right, color, white_texture);

    for (corner_center, start_angle) in rounded_rect_corners(x, y, w, h, r) {
        let verts = arc_fill_verts(corner_center, r, start_angle, std::f32::consts::FRAC_PI_2, view_w, view_h, CORNER_SEGMENTS);
        draw_ui_verts(encoder, &verts, MTLPrimitiveType::Triangle, color, white_texture);
    }
}

fn draw_rounded_rect_border(
    encoder: &metal::RenderCommandEncoderRef,
    center: [f32; 2],
    size: [f32; 2],
    radius: f32,
    border_width: f32,
    color: [f32; 4],
    view_w: f32,
    view_h: f32,
    white_texture: &metal::Texture,
) {
    const FULL_UV: [[f32; 2]; 4] = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];
    const CORNER_SEGMENTS: u32 = 10;
    let [cx, cy] = center;
    let [w, h] = size;
    let r = radius.min(w / 2.0).min(h / 2.0);
    let bw = border_width.min(r).max(0.0);
    let x = cx - w / 2.0;
    let y = cy - h / 2.0;

    for (ex, ey, ew, eh) in [(x + r, y, w - 2.0 * r, bw), (x + r, y + h - bw, w - 2.0 * r, bw), (x, y + r, bw, h - 2.0 * r), (x + w - bw, y + r, bw, h - 2.0 * r)] {
        let quad = ndc_ui_rect(ex, ey, ew.max(0.0), eh.max(0.0), view_w, view_h, FULL_UV);
        draw_ui_quad(encoder, &quad, color, white_texture);
    }
    for (corner_center, start_angle) in rounded_rect_corners(x, y, w, h, r) {
        let verts = arc_stroke_verts(corner_center, r, (r - bw).max(0.0), start_angle, std::f32::consts::FRAC_PI_2, view_w, view_h, CORNER_SEGMENTS);
        draw_ui_verts(encoder, &verts, MTLPrimitiveType::TriangleStrip, color, white_texture);
    }
}

fn draw_rect_border(
    encoder: &metal::RenderCommandEncoderRef,
    center: [f32; 2],
    size: [f32; 2],
    border_width: f32,
    color: [f32; 4],
    view_w: f32,
    view_h: f32,
    white_texture: &metal::Texture,
) {
    const FULL_UV: [[f32; 2]; 4] = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];
    let [cx, cy] = center;
    let [w, h] = size;
    let x = cx - w / 2.0;
    let y = cy - h / 2.0;
    let bw = border_width.min(w / 2.0).min(h / 2.0);
    for (ex, ey, ew, eh) in [(x, y, w, bw), (x, y + h - bw, w, bw), (x, y, bw, h), (x + w - bw, y, bw, h)] {
        let quad = ndc_ui_rect(ex, ey, ew, eh, view_w, view_h, FULL_UV);
        draw_ui_quad(encoder, &quad, color, white_texture);
    }
}

/// `canvas_w`/`canvas_h` must be this pass's actual bound-texture size (e.g.
/// `render_width`/`render_height`), not the device's native resolution — this
/// pass may render into a `render_resolution_scale`-downscaled intermediate.
fn generic_ui_scissor_rect(clip: Option<[f32; 4]>, canvas_w: f32, canvas_h: f32, view_w: f32, view_h: f32) -> MTLScissorRect {
    let phys_w = canvas_w.max(1.0);
    let phys_h = canvas_h.max(1.0);
    let Some([cx, cy, cw, ch]) = clip else {
        return MTLScissorRect { x: 0, y: 0, width: phys_w as u64, height: phys_h as u64 };
    };
    let to_px_x = |v: f32| (v / view_w) * phys_w;
    let to_px_y = |v: f32| (v / view_h) * phys_h;
    let x = to_px_x(cx).clamp(0.0, phys_w - 1.0);
    let y = to_px_y(cy).clamp(0.0, phys_h - 1.0);
    let width = to_px_x(cw).clamp(1.0, phys_w - x);
    let height = to_px_y(ch).clamp(1.0, phys_h - y);
    MTLScissorRect { x: x as u64, y: y as u64, width: width as u64, height: height as u64 }
}

fn draw_generic_ui_nodes(
    encoder: &metal::RenderCommandEncoderRef,
    nodes: &[super::native_game_extract::GenericUiNode],
    canvas_w: f32,
    canvas_h: f32,
    view_w: f32,
    view_h: f32,
    white_texture: &metal::Texture,
    font_atlas_texture: &metal::Texture,
    images: &std::collections::HashMap<AssetId<Image>, metal::Texture>,
) {
    const FULL_UV: [[f32; 2]; 4] = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];
    const CIRCLE_SEGMENTS: u32 = 40;
    for node in nodes {
        encoder.set_scissor_rect(generic_ui_scissor_rect(node.clip_rect, canvas_w, canvas_h, view_w, view_h));

        let [cx, cy] = node.center;
        let [w, h] = node.size;
        // Only a square node with a fully-rounded radius is a circle; a non-square one
        // (Bevy clamps its radius to half the *shorter* side) is a pill, which the
        // rounded-rect path already draws correctly.
        let is_circle = node.corner_radius > 0.0 && node.corner_radius >= w.min(h) / 2.0 - 0.5 && (w - h).abs() <= 0.5;
        let is_rounded = !is_circle && node.corner_radius > 0.5;

        if is_circle {
            let radius = w.min(h) / 2.0;
            if let Some(color) = node.background_color {
                let verts = arc_fill_verts([cx, cy], radius, 0.0, std::f32::consts::TAU, view_w, view_h, CIRCLE_SEGMENTS);
                draw_ui_verts(encoder, &verts, MTLPrimitiveType::Triangle, color, white_texture);
            }
            if let Some(border_color) = node.border_color {
                let inner = (radius - node.border_width).max(0.0);
                let verts = arc_stroke_verts([cx, cy], radius, inner, 0.0, std::f32::consts::TAU, view_w, view_h, CIRCLE_SEGMENTS);
                draw_ui_verts(encoder, &verts, MTLPrimitiveType::TriangleStrip, border_color, white_texture);
            }
        } else if is_rounded {
            if let Some(color) = node.background_color {
                draw_rounded_rect_fill(encoder, [cx, cy], [w, h], node.corner_radius, color, view_w, view_h, white_texture);
            }
            if let Some(border_color) = node.border_color {
                draw_rounded_rect_border(encoder, [cx, cy], [w, h], node.corner_radius, node.border_width, border_color, view_w, view_h, white_texture);
            }
        } else {
            if let Some(color) = node.background_color {
                let quad = ndc_ui_rect_rotated(cx, cy, w, h, node.rotation, view_w, view_h, FULL_UV);
                draw_ui_quad(encoder, &quad, color, white_texture);
            }
            if let Some(border_color) = node.border_color {
                draw_rect_border(encoder, [cx, cy], [w, h], node.border_width, border_color, view_w, view_h, white_texture);
            }
        }

        if let Some(image) = node.image.as_ref().and_then(|image| images.get(&image.id)) {
            let quad = ndc_ui_rect_rotated(cx, cy, w, h, node.rotation, view_w, view_h, FULL_UV);
            draw_ui_quad(encoder, &quad, [1.0, 1.0, 1.0, 1.0], image);
        }
        if let Some(text) = &node.text {
            let scale = text.font_size / GLYPH_H as f32;
            let upper = text.content.to_ascii_uppercase();
            let mut cursor_x = cx - w / 2.0;
            let y = cy - h / 2.0;
            for ch in upper.chars() {
                let Some(index) = glyph_index(ch) else { continue };
                let (u0, u1) = glyph_uv_rect(index);
                let uv = [[u0, 1.0], [u1, 1.0], [u1, 0.0], [u0, 0.0]];
                let glyph_w = GLYPH_W as f32 * scale;
                let glyph_h = GLYPH_H as f32 * scale;
                let quad = ndc_ui_rect(cursor_x, y, glyph_w, glyph_h, view_w, view_h, uv);
                draw_ui_quad(encoder, &quad, text.color, font_atlas_texture);
                cursor_x += glyph_w + scale;
            }
        }
    }

    encoder.set_scissor_rect(generic_ui_scissor_rect(None, canvas_w, canvas_h, view_w, view_h));
}

struct RealGameMeshDraw<'a> {
    vertex_buffer: metal::Buffer,
    vertex_count: usize,
    base_color_factor: [f32; 4],
    emissive: [f32; 4],
    texture: Option<&'a metal::Texture>,
    normal_texture: Option<&'a metal::Texture>,

    is_blend: bool,

    camera_dist_sq: f32,

    model_matrix: [[f32; 4]; 4],
}


impl NativeMetalRenderer {

    pub unsafe fn new(view: *mut Object) -> Self {
        let device = Device::system_default().expect("no Metal device");
        let raw_layer: *mut Object = msg_send![view, layer];

        let retained: *mut Object = msg_send![raw_layer, retain];
        let layer = MetalLayer::from_ptr(retained as *mut CAMetalLayer);
        layer.set_device(&device);
        layer.set_pixel_format(MTLPixelFormat::BGRA8Unorm);

        let bounds: CGRect = msg_send![view, bounds];
        let scale = layer.contents_scale();
        layer.set_drawable_size(CGSize::new(bounds.size.width * scale, bounds.size.height * scale));

        let queue = device.new_command_queue();

        let compile_options = CompileOptions::new();

        let depth_descriptor = metal::DepthStencilDescriptor::new();
        depth_descriptor.set_depth_compare_function(metal::MTLCompareFunction::Less);
        depth_descriptor.set_depth_write_enabled(true);
        let depth_state = device.new_depth_stencil_state(&depth_descriptor);

        let blend_depth_descriptor = metal::DepthStencilDescriptor::new();
        blend_depth_descriptor.set_depth_compare_function(metal::MTLCompareFunction::Less);
        blend_depth_descriptor.set_depth_write_enabled(false);
        let blend_depth_state = device.new_depth_stencil_state(&blend_depth_descriptor);

        let ui_depth_descriptor = metal::DepthStencilDescriptor::new();
        ui_depth_descriptor.set_depth_compare_function(metal::MTLCompareFunction::Always);
        ui_depth_descriptor.set_depth_write_enabled(false);
        let ui_depth_state = device.new_depth_stencil_state(&ui_depth_descriptor);

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
                    eprintln!("[native_metal_renderer] sponza shader compile failed: {err}");
                    (None, None)
                }
            };

        let sampler_descriptor = metal::SamplerDescriptor::new();
        sampler_descriptor.set_min_filter(metal::MTLSamplerMinMagFilter::Linear);
        sampler_descriptor.set_mag_filter(metal::MTLSamplerMinMagFilter::Linear);
        sampler_descriptor.set_address_mode_s(metal::MTLSamplerAddressMode::Repeat);
        sampler_descriptor.set_address_mode_t(metal::MTLSamplerAddressMode::Repeat);
        let sponza_sampler = device.new_sampler(&sampler_descriptor);

        let white_texture_descriptor = metal::TextureDescriptor::new();
        white_texture_descriptor.set_pixel_format(MTLPixelFormat::RGBA8Unorm);
        white_texture_descriptor.set_width(1);
        white_texture_descriptor.set_height(1);
        white_texture_descriptor.set_storage_mode(metal::MTLStorageMode::Shared);
        let white_texture = device.new_texture(&white_texture_descriptor);
        white_texture.replace_region(metal::MTLRegion::new_2d(0, 0, 1, 1), 0, [255u8, 255, 255, 255].as_ptr() as *const _, 4);


        let flat_normal_descriptor = metal::TextureDescriptor::new();
        flat_normal_descriptor.set_pixel_format(MTLPixelFormat::RGBA8Unorm);
        flat_normal_descriptor.set_width(1);
        flat_normal_descriptor.set_height(1);
        flat_normal_descriptor.set_storage_mode(metal::MTLStorageMode::Shared);
        let flat_normal_texture = device.new_texture(&flat_normal_descriptor);
        flat_normal_texture.replace_region(metal::MTLRegion::new_2d(0, 0, 1, 1), 0, [128u8, 128, 255, 255].as_ptr() as *const _, 4);

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

        let shadow_sampler_descriptor = metal::SamplerDescriptor::new();
        shadow_sampler_descriptor.set_min_filter(metal::MTLSamplerMinMagFilter::Linear);
        shadow_sampler_descriptor.set_mag_filter(metal::MTLSamplerMinMagFilter::Linear);
        shadow_sampler_descriptor.set_address_mode_s(metal::MTLSamplerAddressMode::ClampToEdge);
        shadow_sampler_descriptor.set_address_mode_t(metal::MTLSamplerAddressMode::ClampToEdge);
        shadow_sampler_descriptor.set_compare_function(metal::MTLCompareFunction::LessEqual);
        let shadow_sampler = device.new_sampler(&shadow_sampler_descriptor);

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
                    eprintln!("[native_metal_renderer] shadow shader compile failed: {err}");
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
                    eprintln!("[native_metal_renderer] skinned shader compile failed: {err}");
                    (None, None)
                }
            };

        let resource_path = bundle_resource_path().unwrap_or_default();
        let mut skinned_models = super::skinned_actor::SkinnedModelRegistry::default();
        let soldier_path = std::path::Path::new(&resource_path).join("assets/characters/soldier_meshy/mesh/soldier_meshy.glb");
        if let Some(soldier_model) = super::skinned_gltf::load_skinned_gltf(&device, &soldier_path) {

            const SCALE: f32 = 1.04;
            const Y_OFFSET: f32 = -0.76;
            const FACING_DEG: f32 = 0.0;
            skinned_models.register("soldier", soldier_model, SCALE, Y_OFFSET, FACING_DEG.to_radians());
        }

        for kind in [shared::ZombieKind::Classic, shared::ZombieKind::Meshy, shared::ZombieKind::Soldier] {
            let path = std::path::Path::new(&resource_path).join("assets").join(shared::zombie_kind_mesh_path(kind));
            if let Some(model) = super::skinned_gltf::load_skinned_gltf(&device, &path) {
                skinned_models.register(shared::zombie_kind_metal_model_name(kind), model, 1.0, 0.0, 0.0);
            }
        }

        let real_game_app = Some(build_headless_game_ecs_app(
            (bounds.size.width * scale) as u32,
            (bounds.size.height * scale) as u32,
            scale as f32,
        ));

        let scope_color_descriptor = metal::TextureDescriptor::new();
        scope_color_descriptor.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
        scope_color_descriptor.set_width(SCOPE_TEXTURE_SIZE);
        scope_color_descriptor.set_height(SCOPE_TEXTURE_SIZE);
        scope_color_descriptor.set_storage_mode(metal::MTLStorageMode::Private);
        scope_color_descriptor.set_usage(metal::MTLTextureUsage::RenderTarget | metal::MTLTextureUsage::ShaderRead);
        let scope_color_texture = device.new_texture(&scope_color_descriptor);

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
                    eprintln!("[native_metal_renderer] quad shader compile failed: {err}");
                    (None, None)
                }
            };

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

        let blit_pipeline = match device.new_library_with_source(BLIT_SHADER_SOURCE, &compile_options) {
            Ok(library) => {
                let vertex_fn = library.get_function("blit_vertex", None).unwrap();
                let fragment_fn = library.get_function("blit_fragment", None).unwrap();
                let descriptor = RenderPipelineDescriptor::new();
                descriptor.set_vertex_function(Some(&vertex_fn));
                descriptor.set_fragment_function(Some(&fragment_fn));
                descriptor.color_attachments().object_at(0).unwrap().set_pixel_format(MTLPixelFormat::BGRA8Unorm);
                device.new_render_pipeline_state(&descriptor).ok()
            }
            Err(err) => {
                eprintln!("[native_metal_renderer] blit shader compile failed: {err}");
                None
            }
        };

        let blit_verts = [
            BlitVertex { position: [-1.0, -1.0], uv: [0.0, 1.0] },
            BlitVertex { position: [1.0, -1.0], uv: [1.0, 1.0] },
            BlitVertex { position: [1.0, 1.0], uv: [1.0, 0.0] },
            BlitVertex { position: [-1.0, -1.0], uv: [0.0, 1.0] },
            BlitVertex { position: [1.0, 1.0], uv: [1.0, 0.0] },
            BlitVertex { position: [-1.0, 1.0], uv: [0.0, 0.0] },
        ];
        let blit_vertex_buffer = device.new_buffer_with_data(
            blit_verts.as_ptr() as *const _,
            (blit_verts.len() * std::mem::size_of::<BlitVertex>()) as u64,
            MTLResourceOptions::StorageModeShared,
        );

        let scene_color_placeholder_descriptor = metal::TextureDescriptor::new();
        scene_color_placeholder_descriptor.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
        scene_color_placeholder_descriptor.set_width(1);
        scene_color_placeholder_descriptor.set_height(1);
        scene_color_placeholder_descriptor.set_storage_mode(metal::MTLStorageMode::Private);
        scene_color_placeholder_descriptor.set_usage(metal::MTLTextureUsage::RenderTarget | metal::MTLTextureUsage::ShaderRead);
        let scene_color_texture = device.new_texture(&scene_color_placeholder_descriptor);

        fn build_ui_descriptor(vertex_fn: &metal::FunctionRef, fragment_fn: &metal::FunctionRef, sample_count: u64) -> RenderPipelineDescriptor {
            let descriptor = RenderPipelineDescriptor::new();
            descriptor.set_vertex_function(Some(vertex_fn));
            descriptor.set_fragment_function(Some(fragment_fn));
            let color_attachment = descriptor.color_attachments().object_at(0).unwrap();
            color_attachment.set_pixel_format(MTLPixelFormat::BGRA8Unorm);

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
                    eprintln!("[native_metal_renderer] ui shader compile failed: {err}");
                    (None, None)
                }
            };

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
            shadow_quality: 5,
            render_resolution_scale: 1.0,
            zombie_texture_quality: 255,
            scene_color_size: (0, 0),
            scene_color_texture,
            blit_pipeline,
            blit_vertex_buffer,
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
            gpu_transformed_mesh_cache: std::collections::HashMap::new(),
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
            ui_generic_nodes: Vec::new(),
        }
    }

    pub fn real_game_app_mut(&mut self) -> Option<&mut bevy::app::App> {
        self.real_game_app.as_mut()
    }

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

    fn view_size_points(&self) -> (f32, f32) {
        unsafe {
            let raw_layer = self.layer.as_ptr() as *mut Object;
            let bounds: CGRect = msg_send![raw_layer, bounds];
            (bounds.size.width as f32, bounds.size.height as f32)
        }
    }

    fn draw_scene(
        &self,
        encoder: &metal::RenderCommandEncoderRef,
        view_projection: [[f32; 4]; 4],
        light: LightUniform,

        extracted: &[super::skinned_actor::SkinnedActorDraw],
        _now: f32,
        real_game_meshes: Option<&[RealGameMeshDraw]>,

        msaa_enabled: bool,

        light_view_projection: [[f32; 4]; 4],
        point_light_view_projection: [[f32; 4]; 4],
        camera_pos: [f32; 3],
    ) {
        let sponza_pipeline = if msaa_enabled { self.sponza_pipeline.as_ref() } else { self.sponza_pipeline_no_msaa.as_ref() };
        let skinned_pipeline = if msaa_enabled { self.skinned_pipeline.as_ref() } else { self.skinned_pipeline_no_msaa.as_ref() };
        encoder.set_fragment_texture(1, Some(&self.shadow_map));
        encoder.set_fragment_sampler_state(1, Some(&self.shadow_sampler));
        encoder.set_fragment_texture(2, Some(&self.shadow_map_point));

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
                model_matrix: m.model_matrix,
                normal_matrix: normal_matrix_for(m.model_matrix),
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

            if joint_matrices.is_empty() {
                continue;
            }
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

            let mut blended: Vec<&RealGameMeshDraw> = meshes.iter().filter(|m| m.is_blend).collect();
            blended.sort_by(|a, b| b.camera_dist_sq.total_cmp(&a.camera_dist_sq));
            if !blended.is_empty() {
                encoder.set_depth_stencil_state(&self.blend_depth_state);
                encoder.set_render_pipeline_state(sponza_pipeline.unwrap());
                encoder.set_fragment_sampler_state(0, Some(&self.sponza_sampler));
                for m in blended {
                    draw_static_mesh(m);
                }

                encoder.set_depth_stencil_state(&self.depth_state);
            }
        }
    }

    fn make_shadow_texture(device: &metal::Device, size: u64) -> metal::Texture {
        let descriptor = metal::TextureDescriptor::new();
        descriptor.set_pixel_format(MTLPixelFormat::Depth32Float);
        descriptor.set_width(size);
        descriptor.set_height(size);
        descriptor.set_storage_mode(metal::MTLStorageMode::Private);
        descriptor.set_usage(metal::MTLTextureUsage::RenderTarget | metal::MTLTextureUsage::ShaderRead);
        device.new_texture(&descriptor)
    }

    fn ensure_shadow_map_size(&mut self, quality: u8) {
        let quality = quality.min(5);
        if quality == self.shadow_quality {
            return;
        }
        let size = SHADOW_MAP_SIZES[quality as usize];
        self.shadow_map = Self::make_shadow_texture(&self.device, size);
        self.shadow_map_point = Self::make_shadow_texture(&self.device, size);
        self.shadow_quality = quality;
    }

    fn ensure_scene_color_texture(&mut self, width: u64, height: u64) -> metal::Texture {
        if (width, height) == self.scene_color_size {
            return self.scene_color_texture.clone();
        }
        let descriptor = metal::TextureDescriptor::new();
        descriptor.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
        descriptor.set_width(width);
        descriptor.set_height(height);
        descriptor.set_storage_mode(metal::MTLStorageMode::Private);
        descriptor.set_usage(metal::MTLTextureUsage::RenderTarget | metal::MTLTextureUsage::ShaderRead);
        let tex = self.device.new_texture(&descriptor);
        self.scene_color_texture = tex.clone();
        self.scene_color_size = (width, height);
        tex
    }

    fn apply_zombie_texture_quality(&mut self, tier: u8) {
        let tier = tier.min(5);
        if tier == self.zombie_texture_quality {
            return;
        }
        let resource_path = bundle_resource_path().unwrap_or_default();
        for kind in [shared::ZombieKind::Classic, shared::ZombieKind::Meshy, shared::ZombieKind::Soldier] {
            let rel_path = shared::zombie_kind_texture_path(kind, tier);
            let path = std::path::Path::new(&resource_path).join("assets").join(&rel_path);
            let bytes = match std::fs::read(&path) {
                Ok(bytes) => bytes,
                Err(err) => {
                    eprintln!("[native_metal_renderer] zombie texture read failed {path:?}: {err}");
                    continue;
                }
            };
            let decoded = match image::load_from_memory(&bytes) {
                Ok(decoded) => decoded,
                Err(err) => {
                    eprintln!("[native_metal_renderer] zombie texture decode failed {path:?}: {err}");
                    continue;
                }
            };
            let rgba = decoded.to_rgba8();
            let texture = super::metal_texture::upload_texture(&self.device, rgba.width(), rgba.height(), rgba.as_raw());
            let Some(slot) = self.skinned_models.get_mut(shared::zombie_kind_metal_model_name(kind)) else { continue };
            for mesh in &mut slot.model.meshes {
                mesh.base_color_texture = Some(texture.clone());
            }
        }
        self.zombie_texture_quality = tier;
    }

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
                    let uniforms = SponzaShadowUniforms { light_view_projection, model_matrix: m.model_matrix };
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

            if joint_matrices.is_empty() {
                continue;
            }
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

        let now = self.start_time.elapsed().as_secs_f32();

        let mut real_game_static_meshes: Vec<super::native_game_extract::ExtractedMesh> = Vec::new();

        let mut real_game_extracted: Vec<super::skinned_actor::SkinnedActorDraw> = Vec::new();
        let (view_width_pts, view_height_pts) = self.view_size_points();
        if let Some(app) = &mut self.real_game_app {
            prime_camera_viewport(app.world_mut(), view_width_pts, view_height_pts, self.layer.contents_scale() as f32);
            app.update();
            let uploaded = &self.gpu_transformed_mesh_cache;
            let snapshot =
                extract_render_snapshot(app.world_mut(), |id| uploaded.contains_key(&id));
            self.ui_generic_nodes = snapshot.generic_ui_nodes;
            self.msaa_enabled = snapshot.map_msaa_on;
            self.ensure_shadow_map_size(snapshot.shadow_quality);
            self.render_resolution_scale = snapshot.render_resolution_scale.clamp(0.1, 1.0);
            self.apply_zombie_texture_quality(snapshot.zombie_texture_quality);
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
            self.real_game_light = LightUniform::from_snapshots(
                snapshot.light.as_ref(),
                snapshot.sun.as_ref(),
                snapshot.light_quality.min(5) as f32 / 5.0,
                snapshot.point_light_quality.min(5) as f32 / 5.0,
            );
            real_game_static_meshes = snapshot.static_meshes;
            real_game_extracted.extend(snapshot.skinned_actors.into_iter().map(|actor| {
                super::skinned_actor::SkinnedActorDraw {
                    transform: actor.transform,
                    model: actor.model,
                    pose: super::skinned_actor::SkinnedActorPose::JointMatrices(actor.joint_matrices),
                }
            }));
        }

        let msaa_enabled = self.msaa_enabled;
        let device = self.device.clone();

        let render_width = ((width as f64 * self.render_resolution_scale as f64) as u64).max(RENDER_SCALE_MIN_PX);
        let render_height = ((height as f64 * self.render_resolution_scale as f64) as u64).max(RENDER_SCALE_MIN_PX);
        let depth_texture = if msaa_enabled {
            self.msaa_targets.ensure_depth(&device, render_width, render_height)
        } else {
            self.msaa_targets.ensure_depth_no_msaa(&device, render_width, render_height)
        };
        let msaa_color_texture = msaa_enabled.then(|| self.msaa_targets.ensure_color(&device, render_width, render_height));

        let texture = self.ensure_scene_color_texture(render_width, render_height);
        let Some(drawable) = self.layer.next_drawable() else { return };
        let drawable_texture = drawable.texture();

        let used_texture_ids: std::collections::HashSet<AssetId<Image>> = real_game_static_meshes
            .iter()
            .flat_map(|mesh| [mesh.texture.as_ref(), mesh.normal_texture.as_ref()].into_iter().flatten())
            .chain(self.ui_generic_nodes.iter().filter_map(|node| node.image.as_ref()))
            .map(|t| t.id)
            .collect();
        self.real_game_textures.retain(|id, _| used_texture_ids.contains(id));
        for image in self.ui_generic_nodes.iter().filter_map(|node| node.image.as_ref()) {
            if !self.real_game_textures.contains_key(&image.id) {
                let uploaded =
                    super::metal_texture::upload_texture(&self.device, image.width, image.height, &image.rgba);
                self.real_game_textures.insert(image.id, uploaded);
            }
        }
        let used_mesh_ids: std::collections::HashSet<AssetId<Mesh>> =
            real_game_static_meshes.iter().map(|mesh| mesh.mesh_id).collect();
        self.gpu_transformed_mesh_cache.retain(|id, _| used_mesh_ids.contains(id));

        for mesh in &real_game_static_meshes {

            for t in [mesh.texture.as_ref(), mesh.normal_texture.as_ref()].into_iter().flatten() {
                if !self.real_game_textures.contains_key(&t.id) {
                    let uploaded = super::metal_texture::upload_texture(&self.device, t.width, t.height, &t.rgba);
                    self.real_game_textures.insert(t.id, uploaded);
                }
            }
        }
        let real_game_meshes: Vec<RealGameMeshDraw> = real_game_static_meshes
            .iter()
            .filter(|mesh| mesh.visible)
            .filter_map(|mesh| {

                let (vertex_buffer, vertex_count) = match self.gpu_transformed_mesh_cache.get(&mesh.mesh_id) {
                    Some(cached) => cached.clone(),
                    None => {
                        let verts = bake_local_mesh_vertices(mesh);
                        if verts.is_empty() {
                            bevy::log::warn!(
                                "empty mesh skipped: positions={} normals={} uvs={} indices={}",
                                mesh.positions.len(),
                                mesh.normals.len(),
                                mesh.uvs.len(),
                                mesh.indices.len()
                            );
                            return None;
                        }
                        let buffer = self.device.new_buffer_with_data(
                            verts.as_ptr() as *const _,
                            (verts.len() * std::mem::size_of::<super::gltf_scene::GltfVertex>()) as u64,
                            MTLResourceOptions::StorageModeShared,
                        );
                        let entry = (buffer, verts.len());
                        self.gpu_transformed_mesh_cache.insert(mesh.mesh_id, entry.clone());
                        entry
                    }
                };
                let model_matrix = mesh.transform.to_matrix().to_cols_array_2d();
                let texture = mesh.texture.as_ref().and_then(|t| self.real_game_textures.get(&t.id));
                let normal_texture = mesh.normal_texture.as_ref().and_then(|t| self.real_game_textures.get(&t.id));
                let d = [
                    mesh.transform.translation.x - self.camera_position[0],
                    mesh.transform.translation.y - self.camera_position[1],
                    mesh.transform.translation.z - self.camera_position[2],
                ];
                Some(RealGameMeshDraw {
                    vertex_buffer,
                    vertex_count,
                    base_color_factor: mesh.base_color,
                    emissive: mesh.emissive,
                    texture,
                    normal_texture,
                    is_blend: mesh.is_blend,
                    camera_dist_sq: d[0] * d[0] + d[1] * d[1] + d[2] * d[2],
                    model_matrix,
                })
            })
            .collect();
        let real_game_meshes_opt =
            if real_game_meshes.is_empty() { None } else { Some(real_game_meshes.as_slice()) };

        let aspect = width as f32 / height.max(1) as f32;
        let far = if is_real_game { 500.0 } else { 100.0 };

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

        let light = self.real_game_light;
        let view_projection = mat_mul(proj, view);

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

        let extracted: Vec<super::skinned_actor::SkinnedActorDraw> = real_game_extracted;

        let sun_dir = {
            let d = bevy::math::Vec3::from_array(light.sun_dir);
            if d.length_squared() > 1e-6 { d.normalize() } else { bevy::math::Vec3::new(-0.33, -0.67, -0.33).normalize() }
        };

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

            if self.shadow_quality > 0 {
                self.draw_shadow_casters(shadow_encoder, light_view_projection, &extracted, real_game_meshes_opt);
            }
            shadow_encoder.end_encoding();
            shadow_command_buffer.commit();
        }

        let point_light_pos = light.position;
        let point_to_center = (bevy::math::Vec3::from_array(shadow_center) - bevy::math::Vec3::from_array(point_light_pos)).normalize_or_zero();
        let point_up = if point_to_center.y.abs() > 0.99 { [0.0, 0.0, 1.0] } else { [0.0, 1.0, 0.0] };
        let point_light_view = look_at(point_light_pos, shadow_center, point_up);
        let point_light_proj = perspective(90.0_f32.to_radians(), 1.0, 0.1, light.range.max(SHADOW_DISTANCE * 2.0));
        let point_light_view_projection = mat_mul(point_light_proj, point_light_view);

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
            if self.shadow_quality > 0 {
                self.draw_shadow_casters(point_shadow_encoder, point_light_view_projection, &extracted, real_game_meshes_opt);
            }
            point_shadow_encoder.end_encoding();
            point_shadow_command_buffer.commit();
        }

        if show_scope {
            let scope_pass_descriptor = RenderPassDescriptor::new();
            let scope_color = scope_pass_descriptor.color_attachments().object_at(0).unwrap();
            if msaa_enabled {

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

        let pass_descriptor = RenderPassDescriptor::new();
        let color_attachment = pass_descriptor.color_attachments().object_at(0).unwrap();
        if let Some(msaa_color_texture) = &msaa_color_texture {

            color_attachment.set_texture(Some(msaa_color_texture));
            color_attachment.set_resolve_texture(Some(&texture));
            color_attachment.set_store_action(MTLStoreAction::MultisampleResolve);
        } else {
            color_attachment.set_texture(Some(&texture));
            color_attachment.set_store_action(MTLStoreAction::Store);
        }
        color_attachment.set_load_action(MTLLoadAction::Clear);

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

        let ui_pipeline_ref = if msaa_enabled { &self.ui_pipeline } else { &self.ui_pipeline_no_msaa };
        if let Some(ui_pipeline) = ui_pipeline_ref {

            encoder.set_depth_stencil_state(&self.ui_depth_state);
            if !self.ui_generic_nodes.is_empty() {
                let (view_w, view_h) = self.view_size_points();
                encoder.set_render_pipeline_state(ui_pipeline);
                encoder.set_fragment_sampler_state(0, Some(&self.ui_sampler));

                draw_generic_ui_nodes(
                    encoder,
                    &self.ui_generic_nodes,
                    render_width as f32,
                    render_height as f32,
                    view_w,
                    view_h,
                    &self.white_texture,
                    &self.font_atlas_texture,
                    &self.real_game_textures,
                );
            }
        }

        encoder.end_encoding();
        command_buffer.commit();

        let blit_command_buffer = self.queue.new_command_buffer();
        if let Some(blit_pipeline) = &self.blit_pipeline {
            let blit_pass_descriptor = RenderPassDescriptor::new();
            let blit_color_attachment = blit_pass_descriptor.color_attachments().object_at(0).unwrap();
            blit_color_attachment.set_texture(Some(drawable_texture));
            blit_color_attachment.set_load_action(MTLLoadAction::DontCare);
            blit_color_attachment.set_store_action(MTLStoreAction::Store);
            let blit_encoder = blit_command_buffer.new_render_command_encoder(blit_pass_descriptor);
            blit_encoder.set_render_pipeline_state(blit_pipeline);
            blit_encoder.set_vertex_buffer(0, Some(&self.blit_vertex_buffer), 0);
            blit_encoder.set_fragment_texture(0, Some(&texture));
            blit_encoder.set_fragment_sampler_state(0, Some(&self.sponza_sampler));
            blit_encoder.draw_primitives(MTLPrimitiveType::Triangle, 0, 6);
            blit_encoder.end_encoding();
        }
        blit_command_buffer.present_drawable(drawable);
        blit_command_buffer.commit();
    }
}

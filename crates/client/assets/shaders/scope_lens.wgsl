// Kept as .wgsl, not .wesl: a .wesl file goes through the WESL parser, which
// rejects both naga_oil's #import and the #{...} substitution below.
// VertexOutput cannot be redeclared locally because its fields are
// #ifdef-gated, and the material bind group index has to come from Bevy.
#import bevy_pbr::forward_io::VertexOutput

// x = barrel distortion, y = vignette start, z = chromatic fringing, w = edge darkness
@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> settings: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var scope_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var scope_sampler: sampler;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let centred = in.uv - vec2<f32>(0.5);
    let radius = length(centred) * 2.0;

    // outside the glass: opaque black rather than a hard cut
    if (radius > 1.0) {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }

    // barrel distortion pulls the image outward toward the rim
    let bulge = 1.0 + settings.x * radius * radius;
    let warped = vec2<f32>(0.5) + centred * bulge;

    // the glass disperses colour slightly more at the edge
    let offset = centred * settings.z * radius * radius;
    let r = textureSample(scope_texture, scope_sampler, warped + offset).r;
    let g = textureSample(scope_texture, scope_sampler, warped).g;
    let b = textureSample(scope_texture, scope_sampler, warped - offset).b;

    // darken toward the rim, then fade the last sliver into the housing
    let vignette = 1.0 - settings.w * smoothstep(settings.y, 1.0, radius);
    let rim = 1.0 - smoothstep(0.965, 1.0, radius);

    return vec4<f32>(vec3<f32>(r, g, b) * vignette * rim, 1.0);
}

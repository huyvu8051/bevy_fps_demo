// Tier 1: the only coupling is VertexOutput, whose fields are #ifdef-gated and
// so cannot be redeclared locally. On 0.20 this import becomes
// `import bevy_pbr::render::forward_io::VertexOutput;`
#import bevy_pbr::forward_io::VertexOutput

struct ScopeLens {
    // x = barrel distortion, y = vignette start, z = chromatic fringing, w = edge darkness
    settings: vec4<f32>,
};

@group(2) @binding(0)
var<uniform> lens: ScopeLens;
@group(2) @binding(1)
var scope_texture: texture_2d<f32>;
@group(2) @binding(2)
var scope_sampler: sampler;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let centred = in.uv - vec2<f32>(0.5);
    let radius = length(centred) * 2.0;

    // outside the glass: opaque black ring rather than a hard cut
    if (radius > 1.0) {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }

    // barrel distortion pulls the image outward toward the rim
    let bulge = 1.0 + lens.settings.x * radius * radius;
    let warped = vec2<f32>(0.5) + centred * bulge;

    // the glass disperses colour slightly more at the edge
    let fringe = lens.settings.z * radius * radius;
    let offset = centred * fringe;
    let r = textureSample(scope_texture, scope_sampler, warped + offset).r;
    let g = textureSample(scope_texture, scope_sampler, warped).g;
    let b = textureSample(scope_texture, scope_sampler, warped - offset).b;

    // darken toward the rim, then fade the last sliver into the housing
    let vignette = 1.0 - lens.settings.w * smoothstep(lens.settings.y, 1.0, radius);
    let rim = 1.0 - smoothstep(0.965, 1.0, radius);

    return vec4<f32>(vec3<f32>(r, g, b) * vignette * rim, 1.0);
}

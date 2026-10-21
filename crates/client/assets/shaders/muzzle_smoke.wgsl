// Kept as .wgsl, not .wesl: the WESL parser rejects naga_oil's #import and the
// #{...} substitution below. See scope_lens.wgsl for the same constraint.
#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::prepass_utils::prepass_depth
#import bevy_pbr::view_transformations::depth_ndc_to_view_z

// x = life progress 0..1, y = per-puff turn in radians
@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> settings: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var atlas: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var atlas_sampler: sampler;

// Sprite sheet baked from the raymarch this shader used to run. R holds the
// lit term, A the coverage. Must match the baker's layout.
const COLS: f32 = 8.0;
const ROWS: f32 = 8.0;
const TILE_TEXELS: f32 = 128.0;

/// Over how many metres of depth difference the quad fades out where it meets
/// solid geometry. Without this a puff against a wall shows the hard line of
/// the quad cutting into it.
const SOFT_FADE_METRES: f32 = 0.35;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let progress = clamp(settings.x, 0.0, 1.0);

    // One bake serves every puff, so each is turned about its own centre to
    // keep repeats from reading as the same cloud twice.
    let turn = settings.y;
    let centred = in.uv - vec2<f32>(0.5);
    let c = cos(turn);
    let s = sin(turn);
    let spun = vec2<f32>(centred.x * c - centred.y * s, centred.x * s + centred.y * c)
        + vec2<f32>(0.5);
    let inside = f32(spun.x >= 0.0 && spun.x <= 1.0 && spun.y >= 0.0 && spun.y <= 1.0);

    // Half a texel in from each edge, or the filter reaches into the
    // neighbouring frame and drags the wrong cloud in with it.
    let inset = 0.5 / TILE_TEXELS;
    let within = inset + clamp(spun, vec2<f32>(0.0), vec2<f32>(1.0)) * (1.0 - 2.0 * inset);

    let frame = floor(progress * (COLS * ROWS - 1.0));
    let cell = vec2<f32>(frame % COLS, floor(frame / COLS));
    // Explicit level, not textureSample: the discard below puts this in
    // non-uniform control flow, where implicit derivatives are undefined and
    // the hardware is free to pick any mip. It picked the smallest, which
    // sampled one average colour over the whole quad and drew it as a flat
    // rectangle.
    let baked = textureSampleLevel(
        atlas,
        atlas_sampler,
        (cell + within) / vec2<f32>(COLS, ROWS),
        0.0,
    ) * inside;

    // How far the solid world is behind this fragment. Both are negative
    // (view -z is forward), so the subtraction gives a positive gap.
    let scene_z = depth_ndc_to_view_z(prepass_depth(in.position, 0u));
    let quad_z = depth_ndc_to_view_z(in.position.z);
    let gap = clamp((quad_z - scene_z) / SOFT_FADE_METRES, 0.0, 1.0);

    let alpha = baked.a * (1.0 - progress * progress) * gap;
    if (alpha <= 0.001) {
        discard;
    }
    let colour = mix(vec3<f32>(0.34, 0.33, 0.32), vec3<f32>(0.86, 0.86, 0.84), baked.r);
    return vec4<f32>(colour, alpha);
}

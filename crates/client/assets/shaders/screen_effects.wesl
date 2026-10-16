// Plain WGSL, no directives: FullscreenVertexOutput has no #ifdef-gated fields
// so it is declared here instead of imported, which keeps this file valid for
// both naga_oil on 0.19 and WESL on 0.20.
struct FullscreenVertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

struct ScreenEffects {
    grain: f32,
    seconds: f32,
    desaturation: f32,
    pad: f32,
};

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var screen_sampler: sampler;
@group(0) @binding(2) var<uniform> effects: ScreenEffects;

fn hash(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.547);
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    var colour = textureSample(screen_texture, screen_sampler, in.uv).rgb;

    // colour drains toward grey as the player nears death
    let luma = dot(colour, vec3<f32>(0.2126, 0.7152, 0.0722));
    colour = mix(colour, vec3<f32>(luma), clamp(effects.desaturation, 0.0, 1.0));

    // grain reseeds every frame and bites harder in the shadows, the way
    // sensor noise does
    let seed = in.uv + vec2<f32>(fract(effects.seconds * 37.0), fract(effects.seconds * 53.0));
    let noise = hash(seed) - 0.5;
    let shadow_bias = 1.0 - smoothstep(0.0, 0.6, luma);
    colour = colour + noise * effects.grain * (0.35 + 0.65 * shadow_bias);

    return vec4<f32>(max(colour, vec3<f32>(0.0)), 1.0);
}

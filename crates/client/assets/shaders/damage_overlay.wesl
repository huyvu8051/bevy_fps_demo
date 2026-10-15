// Plain WGSL with no preprocessor directives, so it survives both the
// naga_oil pipeline on 0.19 and WESL on 0.20. UiVertexOutput is declared
// here rather than imported for the same reason.
struct UiVertexOutput {
    @location(0) uv: vec2<f32>,
    @location(1) border_widths: vec4<f32>,
    @location(2) border_radius: vec4<f32>,
    @location(3) @interpolate(flat) size: vec2<f32>,
    @builtin(position) position: vec4<f32>,
};

struct DamageOverlay {
    // x = hurt amount 0..1, y = seconds, z = recent hit flash 0..1, w = unused
    settings: vec4<f32>,
};

@group(1) @binding(0)
var<uniform> overlay: DamageOverlay;

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let hurt = clamp(overlay.settings.x, 0.0, 1.0);
    let seconds = overlay.settings.y;
    let flash = clamp(overlay.settings.z, 0.0, 1.0);

    // distance from centre, corrected so the vignette stays round
    let centred = (in.uv - vec2<f32>(0.5)) * vec2<f32>(in.size.x / max(in.size.y, 1.0), 1.0);
    let radius = length(centred) / 0.72;

    // the ring tightens as health drops
    let inner = mix(0.95, 0.30, hurt);
    let vignette = smoothstep(inner, 1.05, radius);

    // slow pulse, stronger when nearly dead
    let pulse = 0.82 + 0.18 * sin(seconds * mix(1.8, 5.2, hurt));
    let edge = vignette * hurt * pulse;

    // a brief whole-screen wash on the frame a hit lands
    let hit = flash * (0.22 + 0.5 * vignette);

    let alpha = clamp(edge * 0.85 + hit, 0.0, 0.95);
    let colour = mix(vec3<f32>(0.42, 0.0, 0.0), vec3<f32>(0.75, 0.05, 0.03), flash);
    return vec4<f32>(colour, alpha);
}

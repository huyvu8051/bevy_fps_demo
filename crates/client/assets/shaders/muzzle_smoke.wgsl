// Kept as .wgsl, not .wesl: the WESL parser rejects naga_oil's #import and the
// #{...} substitution below. See scope_lens.wgsl for the same constraint.
#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::prepass_utils::prepass_depth
#import bevy_pbr::view_transformations::depth_ndc_to_view_z

// x = life progress 0..1, y = per-puff seed, z = density, w = billow speed
@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> settings: vec4<f32>;

const STEPS: i32 = 16;

/// Over how many metres of depth difference the quad fades out where it meets
/// solid geometry. Without this a puff against a wall shows the hard line of
/// the quad cutting into it.
const SOFT_FADE_METRES: f32 = 0.35;
const SPHERE_RADIUS: f32 = 0.5;

fn hash(p: vec3<f32>) -> f32 {
    let q = fract(p * 0.3183099 + vec3<f32>(0.1, 0.1, 0.1));
    let r = q * 17.0;
    return fract(r.x * r.y * r.z * (r.x + r.y + r.z));
}

// value noise: trilinear blend of the 8 lattice corners
fn value_noise(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);

    let n000 = hash(i + vec3<f32>(0.0, 0.0, 0.0));
    let n100 = hash(i + vec3<f32>(1.0, 0.0, 0.0));
    let n010 = hash(i + vec3<f32>(0.0, 1.0, 0.0));
    let n110 = hash(i + vec3<f32>(1.0, 1.0, 0.0));
    let n001 = hash(i + vec3<f32>(0.0, 0.0, 1.0));
    let n101 = hash(i + vec3<f32>(1.0, 0.0, 1.0));
    let n011 = hash(i + vec3<f32>(0.0, 1.0, 1.0));
    let n111 = hash(i + vec3<f32>(1.0, 1.0, 1.0));

    let x00 = mix(n000, n100, u.x);
    let x10 = mix(n010, n110, u.x);
    let x01 = mix(n001, n101, u.x);
    let x11 = mix(n011, n111, u.x);
    return mix(mix(x00, x10, u.y), mix(x01, x11, u.y), u.z);
}

fn fbm(p: vec3<f32>) -> f32 {
    var sum = 0.0;
    var amplitude = 0.5;
    var point = p;
    for (var octave = 0; octave < 4; octave = octave + 1) {
        sum = sum + amplitude * value_noise(point);
        point = point * 2.02;
        amplitude = amplitude * 0.5;
    }
    return sum;
}

// Smoke density inside a soft sphere. The puff rises, expands and tears apart
// as `progress` runs 0 -> 1.
fn density_at(p: vec3<f32>, progress: f32, seed: f32, billow: f32) -> f32 {
    let falloff = 1.0 - smoothstep(0.0, SPHERE_RADIUS, length(p));
    if (falloff <= 0.0) {
        return 0.0;
    }

    // drift up and churn; domain warp gives the curling, non-uniform edge
    let drift = vec3<f32>(0.0, -progress * billow, 0.0);
    let base = p * 3.4 + drift + vec3<f32>(seed, seed * 1.7, seed * 2.3);
    let warp = vec3<f32>(fbm(base * 0.7), fbm(base * 0.7 + 11.3), fbm(base * 0.7 + 23.9));
    let noise = fbm(base + warp * 1.6);

    // erode the cloud over its life so it dissolves instead of fading flat
    let erosion = mix(0.22, 0.86, progress);
    return max(noise - erosion, 0.0) * falloff;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let progress = clamp(settings.x, 0.0, 1.0);
    let seed = settings.y;
    let strength = settings.z;
    let billow = settings.w;

    // March straight through the quad's local box — the quad is billboarded by
    // the CPU, so -Z is always the direction away from the viewer.
    let centred = in.uv - vec2<f32>(0.5);
    let entry = vec3<f32>(centred.x, -centred.y, SPHERE_RADIUS);
    let step = vec3<f32>(0.0, 0.0, -2.0 * SPHERE_RADIUS / f32(STEPS));

    var transmittance = 1.0;
    var lit = 0.0;
    var position = entry;

    for (var i = 0; i < STEPS; i = i + 1) {
        let d = density_at(position, progress, seed, billow) * strength;
        if (d > 0.0) {
            let absorbed = 1.0 - exp(-d * 0.35);
            // cheap self-shadowing: the top of the puff catches more light
            let shade = mix(0.45, 1.0, clamp(position.y / SPHERE_RADIUS * 0.5 + 0.5, 0.0, 1.0));
            lit = lit + transmittance * absorbed * shade;
            transmittance = transmittance * (1.0 - absorbed);
            if (transmittance < 0.01) {
                break;
            }
        }
        position = position + step;
    }

    // How far the solid world is behind this fragment. Both are negative
    // (view -z is forward), so the subtraction gives a positive gap.
    let scene_z = depth_ndc_to_view_z(prepass_depth(in.position, 0u));
    let quad_z = depth_ndc_to_view_z(in.position.z);
    let gap = clamp((quad_z - scene_z) / SOFT_FADE_METRES, 0.0, 1.0);

    let alpha = (1.0 - transmittance) * (1.0 - progress * progress) * gap;
    if (alpha <= 0.001) {
        discard;
    }
    let colour = mix(vec3<f32>(0.34, 0.33, 0.32), vec3<f32>(0.86, 0.86, 0.84), clamp(lit, 0.0, 1.0));
    return vec4<f32>(colour, alpha);
}

// Kept as .wgsl, not .wesl: the WESL parser rejects naga_oil's #import and the
// #{...} substitution below. See scope_lens.wgsl for the same constraint.
#import bevy_pbr::forward_io::VertexOutput

// x = life progress 0..1, y = seed, z = ray scale, w = core size
@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> settings: vec4<f32>;
// x = 0 radial core / 1 directional blade, y = facing (CPU-side fresnel), zw unused
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var<uniform> shape: vec4<f32>;

fn hash2(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

fn vnoise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash2(i);
    let b = hash2(i + vec2<f32>(1.0, 0.0));
    let c = hash2(i + vec2<f32>(0.0, 1.0));
    let d = hash2(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn fbm(p: vec2<f32>) -> f32 {
    var sum = 0.0;
    var amplitude = 0.5;
    var point = p;
    for (var octave = 0; octave < 4; octave = octave + 1) {
        sum = sum + amplitude * vnoise(point);
        point = point * 2.03;
        amplitude = amplitude * 0.5;
    }
    return sum;
}

// Deep red fringe through orange to a core that blows out white. The HDR
// multiplier is what makes bloom bite.
fn flame_colour(intensity: f32) -> vec4<f32> {
    let t = clamp(intensity, 0.0, 1.0);
    let deep = vec3<f32>(1.0, 0.20, 0.03);
    let mid = vec3<f32>(1.0, 0.60, 0.16);
    let white = vec3<f32>(1.0, 0.97, 0.90);
    var colour = mix(deep, mid, smoothstep(0.0, 0.45, t));
    colour = mix(colour, white, smoothstep(0.45, 0.95, t));
    return vec4<f32>(colour * (0.9 + intensity * 9.0), clamp(intensity * 1.35, 0.0, 1.0));
}

// The tongue of flame ejected along the barrel. Its quad is axis-locked to the
// bore and only spun to face the eye, so `shape.y` carries how edge-on it is.
fn blade(in: VertexOutput, progress: f32, seed: f32) -> vec4<f32> {
    let across = in.uv.x - 0.5;
    let along = 1.0 - in.uv.y;

    let taper = pow(clamp(1.0 - along, 0.0, 1.0), 0.65);
    let edge = abs(across) / max(taper * 0.5, 0.001);
    if (edge >= 1.0) {
        discard;
    }

    let body = pow(1.0 - edge, 1.5);
    let churn = fbm(vec2<f32>(across * 7.0 + seed * 13.0, along * 5.0 - seed * 9.0));
    let eroded = clamp(body * (0.6 + churn * 0.9) - along * 0.22, 0.0, 1.0);

    let decay = pow(1.0 - progress, 1.4);
    let intensity = eroded * 2.8 * decay * clamp(shape.y, 0.0, 1.0);
    if (intensity <= 0.004) {
        discard;
    }
    return flame_colour(intensity);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let progress = clamp(settings.x, 0.0, 1.0);
    if (shape.x > 0.5) {
        return blade(in, progress, settings.y);
    }
    let seed = settings.y;
    let ray_scale = settings.z;
    let core_size = settings.w;

    let centred = (in.uv - vec2<f32>(0.5)) * 2.0;
    let radius = length(centred);
    if (radius > 1.0) {
        discard;
    }

    // Sample the noise on the unit circle rather than against the raw angle:
    // that wraps continuously, so there is no seam at +/-pi.
    let dir = centred / max(radius, 0.0001);

    // Coarse lobes decide where gas escapes; the fine term, which also walks
    // along the radius, breaks those lobes into ragged streaks instead of
    // leaving clean petals.
    let lobes = fbm(dir * ray_scale + vec2<f32>(seed * 11.0, seed * 5.0));
    let streaks = fbm(dir * (ray_scale * 3.6) + vec2<f32>(radius * 4.5 + seed * 3.0, seed * 7.0));
    let rays = pow(clamp(lobes * 1.2 + streaks * 0.55 - 0.3, 0.0, 1.0), 2.4);

    // spikes stretch out, then the plume is pulled back in as it burns out
    let reach = mix(1.8, 6.8, progress);
    let plume = rays * exp(-radius * reach);

    let core_falloff = radius / max(core_size, 0.02);
    let core = exp(-core_falloff * core_falloff * 4.0);

    // real flashes stutter rather than fading smoothly
    let flicker = 0.84 + 0.16 * sin(progress * 41.0 + seed * 23.0);
    let decay = pow(1.0 - progress, 1.6);

    // exp() never reaches zero, so without this the quad boundary shows up as a
    // hard-edged circle wherever the plume is still bright at radius 1
    let rim = smoothstep(1.0, 0.72, radius);
    let intensity = (plume * 3.0 + core * 4.4) * decay * flicker * rim;
    if (intensity <= 0.004) {
        discard;
    }

    return flame_colour(intensity);
}

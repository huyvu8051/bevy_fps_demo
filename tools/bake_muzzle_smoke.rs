// Bakes the muzzle smoke sprite sheet that crates/client/assets/shaders/
// muzzle_smoke.wgsl reads. The shader used to run this raymarch per fragment,
// at 960 noise hashes a pixel, which made a sustained burst fill-rate bound.
//
// Stores the lit term in RGB and coverage (1 - transmittance) in A. The
// soft-particle depth fade is NOT baked: it needs the scene's depth buffer, so
// it stays in the shader.
//
// Regenerate (the layout constants here must match the shader's):
//
//   rustc -O tools/bake_muzzle_smoke.rs -o /tmp/bake && /tmp/bake /tmp/raw.rgba
//
// then wrap /tmp/raw.rgba as a 1024x1024 RGBA PNG at
// crates/client/assets/textures/muzzle_smoke_atlas.png.
//
// Frames run progress 0 -> 1 left to right, top to bottom. The puff erodes to
// nothing by progress 0.85, so the last few frames are deliberately empty.
const STEPS: i32 = 12;
const SPHERE_RADIUS: f32 = 0.5;
const DENSITY: f32 = 4.7;
const BILLOW: f32 = 1.5;
const SEED: f32 = 0.37;

const TILE: usize = 128;
const COLS: usize = 8;
const ROWS: usize = 8;

fn fr(x: f32) -> f32 { x - x.floor() }
fn mix(a: f32, b: f32, t: f32) -> f32 { a + (b - a) * t }

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn hash(p: [f32; 3]) -> f32 {
    let q = [
        fr(p[0] * 0.3183099 + 0.1),
        fr(p[1] * 0.3183099 + 0.1),
        fr(p[2] * 0.3183099 + 0.1),
    ];
    let r = [q[0] * 17.0, q[1] * 17.0, q[2] * 17.0];
    fr(r[0] * r[1] * r[2] * (r[0] + r[1] + r[2]))
}

fn value_noise(p: [f32; 3]) -> f32 {
    let i = [p[0].floor(), p[1].floor(), p[2].floor()];
    let f = [p[0] - i[0], p[1] - i[1], p[2] - i[2]];
    let u = [
        f[0] * f[0] * (3.0 - 2.0 * f[0]),
        f[1] * f[1] * (3.0 - 2.0 * f[1]),
        f[2] * f[2] * (3.0 - 2.0 * f[2]),
    ];
    let c = |dx: f32, dy: f32, dz: f32| hash([i[0] + dx, i[1] + dy, i[2] + dz]);
    let x00 = mix(c(0.0, 0.0, 0.0), c(1.0, 0.0, 0.0), u[0]);
    let x10 = mix(c(0.0, 1.0, 0.0), c(1.0, 1.0, 0.0), u[0]);
    let x01 = mix(c(0.0, 0.0, 1.0), c(1.0, 0.0, 1.0), u[0]);
    let x11 = mix(c(0.0, 1.0, 1.0), c(1.0, 1.0, 1.0), u[0]);
    mix(mix(x00, x10, u[1]), mix(x01, x11, u[1]), u[2])
}

fn fbm_octaves(p: [f32; 3], octaves: i32) -> f32 {
    let mut sum = 0.0;
    let mut amplitude = 0.5;
    let mut point = p;
    for _ in 0..octaves {
        sum += amplitude * value_noise(point);
        point = [point[0] * 2.02, point[1] * 2.02, point[2] * 2.02];
        amplitude *= 0.5;
    }
    sum
}

fn fbm(p: [f32; 3]) -> f32 { fbm_octaves(p, 4) }
fn fbm_warp(p: [f32; 3]) -> f32 { fbm_octaves(p, 2) }

fn density_at(p: [f32; 3], progress: f32) -> f32 {
    let len = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
    let falloff = 1.0 - smoothstep(0.0, SPHERE_RADIUS, len);
    if falloff <= 0.0 { return 0.0; }
    let base = [
        p[0] * 3.4 + SEED,
        p[1] * 3.4 - progress * BILLOW + SEED * 1.7,
        p[2] * 3.4 + SEED * 2.3,
    ];
    let b7 = [base[0] * 0.7, base[1] * 0.7, base[2] * 0.7];
    let warp = [
        fbm_warp(b7),
        fbm_warp([b7[0] + 11.3, b7[1] + 11.3, b7[2] + 11.3]),
        fbm_warp([b7[0] + 23.9, b7[1] + 23.9, b7[2] + 23.9]),
    ];
    let noise = fbm([
        base[0] + warp[0] * 1.6,
        base[1] + warp[1] * 1.6,
        base[2] + warp[2] * 1.6,
    ]);
    let erosion = mix(0.22, 0.86, progress);
    (noise - erosion).max(0.0) * falloff
}

fn main() {
    let frames = COLS * ROWS;
    let width = COLS * TILE;
    let height = ROWS * TILE;
    let mut out = vec![0u8; width * height * 4];

    for frame in 0..frames {
        let progress = frame as f32 / (frames - 1) as f32;
        let (tile_x, tile_y) = ((frame % COLS) * TILE, (frame / COLS) * TILE);
        for py in 0..TILE {
            for px in 0..TILE {
                let uv = [
                    (px as f32 + 0.5) / TILE as f32,
                    (py as f32 + 0.5) / TILE as f32,
                ];
                let entry = [uv[0] - 0.5, -(uv[1] - 0.5), SPHERE_RADIUS];
                let step = -2.0 * SPHERE_RADIUS / STEPS as f32;

                let mut transmittance = 1.0f32;
                let mut lit = 0.0f32;
                let mut position = entry;
                for _ in 0..STEPS {
                    let d = density_at(position, progress) * DENSITY;
                    if d > 0.0 {
                        let absorbed = 1.0 - (-d * 0.35).exp();
                        let shade = mix(
                            0.45,
                            1.0,
                            (position[1] / SPHERE_RADIUS * 0.5 + 0.5).clamp(0.0, 1.0),
                        );
                        lit += transmittance * absorbed * shade;
                        transmittance *= 1.0 - absorbed;
                        if transmittance < 0.01 { break; }
                    }
                    position[2] += step;
                }

                let l = (lit.clamp(0.0, 1.0) * 255.0).round() as u8;
                let a = ((1.0 - transmittance).clamp(0.0, 1.0) * 255.0).round() as u8;
                let o = ((tile_y + py) * width + tile_x + px) * 4;
                out[o] = l;
                out[o + 1] = l;
                out[o + 2] = l;
                out[o + 3] = a;
            }
        }
    }
    std::fs::write(std::env::args().nth(1).unwrap(), &out).unwrap();
    eprintln!("{}x{} raw RGBA, {} frames", width, height, frames);
}

//! Plain 4x4-matrix math for the native Metal renderer — zero dependency on
//! this project's own gameplay types (no `bevy`, no `shared`, no
//! `ExtractedMesh`/`Zombie`/etc.), so this is the first piece pulled out on
//! the way toward a genuinely reusable "native Metal renderer for Bevy on
//! iOS" plugin (see native_metal_probe.rs's own module doc comment for the
//! wider context this experiment exists in). Column-major `[[f32; 4]; 4]`,
//! matching Metal's own convention.

/// Right-handed perspective projection, Metal's clip-space convention
/// (Z in [-1, 1] via the standard OpenGL-style derivation — this renderer
/// doesn't use Metal's native [0, 1] depth range).
pub fn perspective(fov_y_radians: f32, aspect: f32, near: f32, far: f32) -> [[f32; 4]; 4] {
    let y_scale = 1.0 / (fov_y_radians * 0.5).tan();
    let x_scale = y_scale / aspect;
    let z_range = far - near;
    let z_scale = -(far + near) / z_range;
    let wz_scale = -2.0 * far * near / z_range;
    [
        [x_scale, 0.0, 0.0, 0.0],
        [0.0, y_scale, 0.0, 0.0],
        [0.0, 0.0, z_scale, -1.0],
        [0.0, 0.0, wz_scale, 0.0],
    ]
}

/// Standard right-handed look-at view matrix.
pub fn look_at(eye: [f32; 3], center: [f32; 3], up: [f32; 3]) -> [[f32; 4]; 4] {
    let sub = |a: [f32; 3], b: [f32; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let cross = |a: [f32; 3], b: [f32; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let norm = |v: [f32; 3]| {
        let len = (dot(v, v)).sqrt();
        [v[0] / len, v[1] / len, v[2] / len]
    };
    let f = norm(sub(center, eye));
    let s = norm(cross(f, up));
    let u = cross(s, f);
    [
        [s[0], u[0], -f[0], 0.0],
        [s[1], u[1], -f[1], 0.0],
        [s[2], u[2], -f[2], 0.0],
        [-dot(s, eye), -dot(u, eye), dot(f, eye), 1.0],
    ]
}

/// Column-major 4x4 matrix multiply (`a * b`).
pub fn mat_mul(a: [[f32; 4]; 4], b: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut out = [[0.0f32; 4]; 4];
    for col in 0..4 {
        for row in 0..4 {
            out[col][row] = a[0][row] * b[col][0] + a[1][row] * b[col][1] + a[2][row] * b[col][2] + a[3][row] * b[col][3];
        }
    }
    out
}

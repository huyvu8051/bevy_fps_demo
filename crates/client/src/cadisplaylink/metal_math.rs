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

/// Right-handed orthographic projection, same "OpenGL-style" Z in [-1, 1]
/// convention as `perspective` above (see its own doc comment for why this
/// renderer deliberately doesn't remap into Metal's native [0, 1] depth
/// range — self-consistent Less-compare depth testing doesn't actually
/// need that remap). Used for the shadow-mapping light camera (native_
/// metal_probe.rs) — a directional light has no perspective falloff, so
/// its shadow frustum is a plain box, not perspective's view-frustum
/// pyramid.
pub fn orthographic(left: f32, right: f32, bottom: f32, top: f32, near: f32, far: f32) -> [[f32; 4]; 4] {
    let rl = right - left;
    let tb = top - bottom;
    let fmn = far - near;
    [
        [2.0 / rl, 0.0, 0.0, 0.0],
        [0.0, 2.0 / tb, 0.0, 0.0],
        [0.0, 0.0, -2.0 / fmn, 0.0],
        [-(right + left) / rl, -(top + bottom) / tb, -(far + near) / fmn, 1.0],
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

/// Transforms a point (implicit w=1) by a column-major 4x4 matrix,
/// returning the full homogeneous clip-space `[x, y, z, w]` — callers that
/// need NDC (native_metal_probe.rs's scope-PIP screen projection, to place
/// the PIP circle where the real 3D lens quad's world position actually
/// projects to instead of a fixed screen position) must divide by `w`
/// themselves, and check `w > 0` first: a point behind the camera has
/// `w <= 0`, and dividing by it would fling the projected point to the
/// wrong side of the screen instead of correctly reporting "not visible".
pub fn transform_point(m: [[f32; 4]; 4], p: [f32; 3]) -> [f32; 4] {
    [
        m[0][0] * p[0] + m[1][0] * p[1] + m[2][0] * p[2] + m[3][0],
        m[0][1] * p[0] + m[1][1] * p[1] + m[2][1] * p[2] + m[3][1],
        m[0][2] * p[0] + m[1][2] * p[1] + m[2][2] * p[2] + m[3][2],
        m[0][3] * p[0] + m[1][3] * p[1] + m[2][3] * p[2] + m[3][3],
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

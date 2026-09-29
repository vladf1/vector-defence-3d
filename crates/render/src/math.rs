//! Plain 4x4 matrix helpers (column-major, WGSL layout) and a small `Vec3`.
//!
//! Matrices are stored as `f32` (what the GPU reads); products and inverses are evaluated in
//! `f64`, like the JavaScript original did over its `Float32Array`s.

/// Column-major 4x4 matrix (WGSL layout).
pub type Mat4 = [f32; 16];

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

pub const fn vec3(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3 { x, y, z }
}

/// `Math.hypot(x, y, z) || 1`: a length that is safe to divide by.
pub(crate) fn safe_length(x: f32, y: f32, z: f32) -> f32 {
    let length = (x * x + y * y + z * z).sqrt();
    if length > 0.0 { length } else { 1.0 }
}

pub const fn mat4_identity() -> Mat4 {
    let mut m = [0.0; 16];
    m[0] = 1.0;
    m[5] = 1.0;
    m[10] = 1.0;
    m[15] = 1.0;
    m
}

/// out = a * b (both column-major).
pub fn mat4_multiply(out: &mut Mat4, a: &Mat4, b: &Mat4) {
    for column in 0..4 {
        let b0 = b[column * 4] as f64;
        let b1 = b[column * 4 + 1] as f64;
        let b2 = b[column * 4 + 2] as f64;
        let b3 = b[column * 4 + 3] as f64;
        for row in 0..4 {
            out[column * 4 + row] =
                (a[row] as f64 * b0 + a[4 + row] as f64 * b1 + a[8 + row] as f64 * b2 + a[12 + row] as f64 * b3) as f32;
        }
    }
}

/// Writes the inverse of `m` into `out`; returns false (leaving `out` untouched) when `m` is singular.
pub fn mat4_invert(out: &mut Mat4, m: &Mat4) -> bool {
    let a = m.map(f64::from);
    let (a00, a01, a02, a03) = (a[0], a[1], a[2], a[3]);
    let (a10, a11, a12, a13) = (a[4], a[5], a[6], a[7]);
    let (a20, a21, a22, a23) = (a[8], a[9], a[10], a[11]);
    let (a30, a31, a32, a33) = (a[12], a[13], a[14], a[15]);
    let b00 = a00 * a11 - a01 * a10;
    let b01 = a00 * a12 - a02 * a10;
    let b02 = a00 * a13 - a03 * a10;
    let b03 = a01 * a12 - a02 * a11;
    let b04 = a01 * a13 - a03 * a11;
    let b05 = a02 * a13 - a03 * a12;
    let b06 = a20 * a31 - a21 * a30;
    let b07 = a20 * a32 - a22 * a30;
    let b08 = a20 * a33 - a23 * a30;
    let b09 = a21 * a32 - a22 * a31;
    let b10 = a21 * a33 - a23 * a31;
    let b11 = a22 * a33 - a23 * a32;
    let determinant = b00 * b11 - b01 * b10 + b02 * b09 + b03 * b08 - b04 * b07 + b05 * b06;
    if determinant.abs() < 1e-12 {
        return false;
    }
    let inverse = 1.0 / determinant;
    let values = [
        (a11 * b11 - a12 * b10 + a13 * b09) * inverse,
        (a02 * b10 - a01 * b11 - a03 * b09) * inverse,
        (a31 * b05 - a32 * b04 + a33 * b03) * inverse,
        (a22 * b04 - a21 * b05 - a23 * b03) * inverse,
        (a12 * b08 - a10 * b11 - a13 * b07) * inverse,
        (a00 * b11 - a02 * b08 + a03 * b07) * inverse,
        (a32 * b02 - a30 * b05 - a33 * b01) * inverse,
        (a20 * b05 - a22 * b02 + a23 * b01) * inverse,
        (a10 * b10 - a11 * b08 + a13 * b06) * inverse,
        (a01 * b08 - a00 * b10 - a03 * b06) * inverse,
        (a30 * b04 - a31 * b02 + a33 * b00) * inverse,
        (a21 * b02 - a20 * b04 - a23 * b00) * inverse,
        (a11 * b07 - a10 * b09 - a12 * b06) * inverse,
        (a00 * b09 - a01 * b07 + a02 * b06) * inverse,
        (a31 * b01 - a30 * b03 - a32 * b00) * inverse,
        (a20 * b03 - a21 * b01 + a22 * b00) * inverse,
    ];
    for (target, value) in out.iter_mut().zip(values) {
        *target = value as f32;
    }
    true
}

/// WebGPU clip-space (depth 0..1) perspective projection.
pub fn mat4_perspective(out: &mut Mat4, vertical_fov_radians: f32, aspect: f32, near: f32, far: f32) {
    let f = 1.0 / (vertical_fov_radians / 2.0).tan();
    *out = [0.0; 16];
    out[0] = f / aspect;
    out[5] = f;
    out[10] = far / (near - far);
    out[11] = -1.0;
    out[14] = (far * near) / (near - far);
}

/// World matrix of a camera at `eye` looking at `target` (the camera looks down its local -Z).
pub fn mat4_look_at_world(out: &mut Mat4, eye: Vec3, target: Vec3, up: Vec3) {
    let (mut zx, mut zy, mut zz) = (eye.x - target.x, eye.y - target.y, eye.z - target.z);
    let z_length = safe_length(zx, zy, zz);
    zx /= z_length;
    zy /= z_length;
    zz /= z_length;
    let mut xx = up.y * zz - up.z * zy;
    let mut xy = up.z * zx - up.x * zz;
    let mut xz = up.x * zy - up.y * zx;
    let x_length = safe_length(xx, xy, xz);
    xx /= x_length;
    xy /= x_length;
    xz /= x_length;
    *out = [
        xx,
        xy,
        xz,
        0.0,
        zy * xz - zz * xy,
        zz * xx - zx * xz,
        zx * xy - zy * xx,
        0.0,
        zx,
        zy,
        zz,
        0.0,
        eye.x,
        eye.y,
        eye.z,
        1.0,
    ];
}

/// Transforms a point and divides by w (for projection matrices).
pub fn transform_point_projective(m: &Mat4, x: f32, y: f32, z: f32) -> Vec3 {
    let (x, y, z) = (x as f64, y as f64, z as f64);
    let m = m.map(f64::from);
    let w = m[3] * x + m[7] * y + m[11] * z + m[15];
    let inverse_w = if w != 0.0 { 1.0 / w } else { 1.0 };
    vec3(
        ((m[0] * x + m[4] * y + m[8] * z + m[12]) * inverse_w) as f32,
        ((m[1] * x + m[5] * y + m[9] * z + m[13]) * inverse_w) as f32,
        ((m[2] * x + m[6] * y + m[10] * z + m[14]) * inverse_w) as f32,
    )
}

/// Translation, rotation from XYZ Euler angles (R = Rx * Ry * Rz), and axis scale.
#[allow(clippy::too_many_arguments)]
pub fn mat4_compose(
    x: f32,
    y: f32,
    z: f32,
    rotation_x: f32,
    rotation_y: f32,
    rotation_z: f32,
    scale_x: f32,
    scale_y: f32,
    scale_z: f32,
) -> Mat4 {
    let (b, a) = rotation_x.sin_cos();
    let (d, c) = rotation_y.sin_cos();
    let (f, e) = rotation_z.sin_cos();
    let ae = a * e;
    let af = a * f;
    let be = b * e;
    let bf = b * f;
    [
        c * e * scale_x,
        (af + be * d) * scale_x,
        (bf - ae * d) * scale_x,
        0.0,
        -c * f * scale_y,
        (ae - bf * d) * scale_y,
        (be + af * d) * scale_y,
        0.0,
        d * scale_z,
        -b * c * scale_z,
        a * c * scale_z,
        0.0,
        x,
        y,
        z,
        1.0,
    ]
}

/// Inverse transpose of the upper 3x3, as row-major values: n' = N * n. Used to carry
/// normals through non-uniform scale while building geometry.
pub fn normal_matrix(m: &Mat4) -> [f32; 9] {
    let (a00, a01, a02) = (m[0], m[4], m[8]);
    let (a10, a11, a12) = (m[1], m[5], m[9]);
    let (a20, a21, a22) = (m[2], m[6], m[10]);
    let c00 = a11 * a22 - a12 * a21;
    let c01 = a12 * a20 - a10 * a22;
    let c02 = a10 * a21 - a11 * a20;
    let determinant = a00 * c00 + a01 * c01 + a02 * c02;
    let inverse = if determinant.abs() < 1e-12 { 0.0 } else { 1.0 / determinant };
    [
        c00 * inverse,
        c01 * inverse,
        c02 * inverse,
        (a02 * a21 - a01 * a22) * inverse,
        (a00 * a22 - a02 * a20) * inverse,
        (a01 * a20 - a00 * a21) * inverse,
        (a01 * a12 - a02 * a11) * inverse,
        (a02 * a10 - a00 * a12) * inverse,
        (a00 * a11 - a01 * a10) * inverse,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: &Mat4, b: &Mat4, tolerance: f32) -> bool {
        a.iter().zip(b).all(|(x, y)| (x - y).abs() <= tolerance)
    }

    #[test]
    fn invert_times_original_is_identity() {
        let m = mat4_compose(12.0, -4.0, 30.0, 0.3, -1.1, 2.2, 1.5, 0.7, 2.0);
        let mut inverse = mat4_identity();
        assert!(mat4_invert(&mut inverse, &m));
        let mut product = mat4_identity();
        mat4_multiply(&mut product, &m, &inverse);
        assert!(close(&product, &mat4_identity(), 1e-5), "{product:?}");
        mat4_multiply(&mut product, &inverse, &m);
        assert!(close(&product, &mat4_identity(), 1e-5), "{product:?}");
    }

    #[test]
    fn singular_matrix_is_rejected() {
        let mut out = mat4_identity();
        assert!(!mat4_invert(&mut out, &[0.0; 16]));
        assert_eq!(out, mat4_identity());
    }

    #[test]
    fn multiply_matches_manual_composition() {
        let translate = mat4_compose(5.0, 6.0, 7.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0);
        let scale = mat4_compose(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 2.0, 3.0, 4.0);
        let mut out = mat4_identity();
        mat4_multiply(&mut out, &translate, &scale);
        let point = transform_point_projective(&out, 1.0, 1.0, 1.0);
        assert_eq!(point, vec3(7.0, 9.0, 11.0));
    }

    #[test]
    fn perspective_maps_near_and_far_to_clip_depth() {
        let mut projection = mat4_identity();
        mat4_perspective(&mut projection, 1.0, 1.5, 60.0, 4000.0);
        let near = transform_point_projective(&projection, 0.0, 0.0, -60.0);
        let far = transform_point_projective(&projection, 0.0, 0.0, -4000.0);
        assert!(near.z.abs() < 1e-5 && (far.z - 1.0).abs() < 1e-5);
    }

    #[test]
    fn normal_matrix_undoes_non_uniform_scale() {
        let m = mat4_compose(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 2.0, 1.0, 0.5);
        let n = normal_matrix(&m);
        assert!((n[0] - 0.5).abs() < 1e-6 && (n[4] - 1.0).abs() < 1e-6 && (n[8] - 2.0).abs() < 1e-6);
    }
}

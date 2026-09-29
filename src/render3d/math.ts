/** Column-major 4x4 matrix (WGSL layout). */
export type Mat4 = Float32Array;

export interface Vec3 {
  x: number;
  y: number;
  z: number;
}

export function vec3(x: number, y: number, z: number): Vec3 {
  return { x, y, z };
}

export function mat4Identity(): Mat4 {
  const m = new Float32Array(16);
  m[0] = 1;
  m[5] = 1;
  m[10] = 1;
  m[15] = 1;
  return m;
}

/** out = a * b (both column-major). `out` must not alias either input. */
export function mat4Multiply(out: Mat4, a: Mat4, b: Mat4): Mat4 {
  for (let column = 0; column < 4; column += 1) {
    const b0 = b[column * 4];
    const b1 = b[(column * 4) + 1];
    const b2 = b[(column * 4) + 2];
    const b3 = b[(column * 4) + 3];
    for (let row = 0; row < 4; row += 1) {
      out[(column * 4) + row] = (a[row] * b0) + (a[4 + row] * b1) + (a[8 + row] * b2) + (a[12 + row] * b3);
    }
  }
  return out;
}

export function mat4Invert(out: Mat4, m: Mat4): Mat4 | null {
  const a00 = m[0], a01 = m[1], a02 = m[2], a03 = m[3];
  const a10 = m[4], a11 = m[5], a12 = m[6], a13 = m[7];
  const a20 = m[8], a21 = m[9], a22 = m[10], a23 = m[11];
  const a30 = m[12], a31 = m[13], a32 = m[14], a33 = m[15];
  const b00 = (a00 * a11) - (a01 * a10);
  const b01 = (a00 * a12) - (a02 * a10);
  const b02 = (a00 * a13) - (a03 * a10);
  const b03 = (a01 * a12) - (a02 * a11);
  const b04 = (a01 * a13) - (a03 * a11);
  const b05 = (a02 * a13) - (a03 * a12);
  const b06 = (a20 * a31) - (a21 * a30);
  const b07 = (a20 * a32) - (a22 * a30);
  const b08 = (a20 * a33) - (a23 * a30);
  const b09 = (a21 * a32) - (a22 * a31);
  const b10 = (a21 * a33) - (a23 * a31);
  const b11 = (a22 * a33) - (a23 * a32);
  const determinant = (b00 * b11) - (b01 * b10) + (b02 * b09) + (b03 * b08) - (b04 * b07) + (b05 * b06);
  if (Math.abs(determinant) < 1e-12) {
    return null;
  }
  const inverse = 1 / determinant;
  out[0] = ((a11 * b11) - (a12 * b10) + (a13 * b09)) * inverse;
  out[1] = ((a02 * b10) - (a01 * b11) - (a03 * b09)) * inverse;
  out[2] = ((a31 * b05) - (a32 * b04) + (a33 * b03)) * inverse;
  out[3] = ((a22 * b04) - (a21 * b05) - (a23 * b03)) * inverse;
  out[4] = ((a12 * b08) - (a10 * b11) - (a13 * b07)) * inverse;
  out[5] = ((a00 * b11) - (a02 * b08) + (a03 * b07)) * inverse;
  out[6] = ((a32 * b02) - (a30 * b05) - (a33 * b01)) * inverse;
  out[7] = ((a20 * b05) - (a22 * b02) + (a23 * b01)) * inverse;
  out[8] = ((a10 * b10) - (a11 * b08) + (a13 * b06)) * inverse;
  out[9] = ((a01 * b08) - (a00 * b10) - (a03 * b06)) * inverse;
  out[10] = ((a30 * b04) - (a31 * b02) + (a33 * b00)) * inverse;
  out[11] = ((a21 * b02) - (a20 * b04) - (a23 * b00)) * inverse;
  out[12] = ((a11 * b07) - (a10 * b09) - (a12 * b06)) * inverse;
  out[13] = ((a00 * b09) - (a01 * b07) + (a02 * b06)) * inverse;
  out[14] = ((a31 * b01) - (a30 * b03) - (a32 * b00)) * inverse;
  out[15] = ((a20 * b03) - (a21 * b01) + (a22 * b00)) * inverse;
  return out;
}

/** WebGPU clip-space (depth 0..1) perspective projection. */
export function mat4Perspective(out: Mat4, verticalFovRadians: number, aspect: number, near: number, far: number): Mat4 {
  const f = 1 / Math.tan(verticalFovRadians / 2);
  out.fill(0);
  out[0] = f / aspect;
  out[5] = f;
  out[10] = far / (near - far);
  out[11] = -1;
  out[14] = (far * near) / (near - far);
  return out;
}

/** World matrix of a camera at `eye` looking at `target` (the camera looks down its local -Z). */
export function mat4LookAtWorld(out: Mat4, eye: Vec3, target: Vec3, up: Vec3): Mat4 {
  let zx = eye.x - target.x;
  let zy = eye.y - target.y;
  let zz = eye.z - target.z;
  const zLength = Math.hypot(zx, zy, zz) || 1;
  zx /= zLength;
  zy /= zLength;
  zz /= zLength;
  let xx = (up.y * zz) - (up.z * zy);
  let xy = (up.z * zx) - (up.x * zz);
  let xz = (up.x * zy) - (up.y * zx);
  const xLength = Math.hypot(xx, xy, xz) || 1;
  xx /= xLength;
  xy /= xLength;
  xz /= xLength;
  out[0] = xx;
  out[1] = xy;
  out[2] = xz;
  out[3] = 0;
  out[4] = (zy * xz) - (zz * xy);
  out[5] = (zz * xx) - (zx * xz);
  out[6] = (zx * xy) - (zy * xx);
  out[7] = 0;
  out[8] = zx;
  out[9] = zy;
  out[10] = zz;
  out[11] = 0;
  out[12] = eye.x;
  out[13] = eye.y;
  out[14] = eye.z;
  out[15] = 1;
  return out;
}

/** Transforms a point and divides by w (for projection matrices). */
export function transformPointProjective(m: Mat4, x: number, y: number, z: number, out: Vec3): Vec3 {
  const w = (m[3] * x) + (m[7] * y) + (m[11] * z) + m[15];
  const inverseW = w !== 0 ? 1 / w : 1;
  out.x = ((m[0] * x) + (m[4] * y) + (m[8] * z) + m[12]) * inverseW;
  out.y = ((m[1] * x) + (m[5] * y) + (m[9] * z) + m[13]) * inverseW;
  out.z = ((m[2] * x) + (m[6] * y) + (m[10] * z) + m[14]) * inverseW;
  return out;
}

/** Translation, rotation from XYZ Euler angles (R = Rx * Ry * Rz), and axis scale. */
export function mat4Compose(
  x: number,
  y: number,
  z: number,
  rotationX: number,
  rotationY: number,
  rotationZ: number,
  scaleX: number,
  scaleY: number,
  scaleZ: number,
): Mat4 {
  const a = Math.cos(rotationX);
  const b = Math.sin(rotationX);
  const c = Math.cos(rotationY);
  const d = Math.sin(rotationY);
  const e = Math.cos(rotationZ);
  const f = Math.sin(rotationZ);
  const ae = a * e;
  const af = a * f;
  const be = b * e;
  const bf = b * f;
  const m = new Float32Array(16);
  m[0] = c * e * scaleX;
  m[1] = (af + (be * d)) * scaleX;
  m[2] = (bf - (ae * d)) * scaleX;
  m[4] = -c * f * scaleY;
  m[5] = (ae - (bf * d)) * scaleY;
  m[6] = (be + (af * d)) * scaleY;
  m[8] = d * scaleZ;
  m[9] = -b * c * scaleZ;
  m[10] = a * c * scaleZ;
  m[12] = x;
  m[13] = y;
  m[14] = z;
  m[15] = 1;
  return m;
}

/**
 * Inverse transpose of the upper 3x3, as row-major values: n' = N * n. Used to carry
 * normals through non-uniform scale while building geometry.
 */
export function normalMatrix(m: Mat4): number[] {
  const a00 = m[0], a01 = m[4], a02 = m[8];
  const a10 = m[1], a11 = m[5], a12 = m[9];
  const a20 = m[2], a21 = m[6], a22 = m[10];
  const c00 = (a11 * a22) - (a12 * a21);
  const c01 = (a12 * a20) - (a10 * a22);
  const c02 = (a10 * a21) - (a11 * a20);
  const determinant = (a00 * c00) + (a01 * c01) + (a02 * c02);
  const inverse = Math.abs(determinant) < 1e-12 ? 0 : 1 / determinant;
  return [
    c00 * inverse, c01 * inverse, c02 * inverse,
    ((a02 * a21) - (a01 * a22)) * inverse, ((a00 * a22) - (a02 * a20)) * inverse, ((a01 * a20) - (a00 * a21)) * inverse,
    ((a01 * a12) - (a02 * a11)) * inverse, ((a02 * a10) - (a00 * a12)) * inverse, ((a00 * a11) - (a01 * a10)) * inverse,
  ];
}

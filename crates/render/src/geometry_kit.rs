//! Procedural low-poly geometry: primitives, outline extrusion, and part assembly into
//! non-indexed triangle soups.
use std::f32::consts::{PI, SQRT_2};

use crate::math::{Mat4, Vec3, mat4_compose, normal_matrix, safe_length};

/// A 2D field-space point (x forward, y toward screen-bottom) for model outlines.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct P2 {
    pub x: f32,
    pub y: f32,
}

pub const fn p2(x: f32, y: f32) -> P2 {
    P2 { x, y }
}

/// Non-indexed triangle soup: three floats per vertex for positions and normals.
#[derive(Clone, Debug, Default)]
pub struct Primitive {
    pub positions: Vec<f32>,
    pub normals: Vec<f32>,
}

/// A primitive tagged with its glow mask, ready to merge into a neon part.
#[derive(Clone, Debug, Default)]
pub struct Part {
    pub positions: Vec<f32>,
    pub normals: Vec<f32>,
    pub glow: Vec<f32>,
}

/// Interleaved vertex data for one batch: `position(3) normal(3) glow(1)` for the neon
/// pipeline, `position(3) uv(2)` for flat effect quads.
#[derive(Clone, Debug)]
pub struct Mesh {
    pub vertices: Vec<f32>,
    pub vertex_count: u32,
}

pub const NEON_VERTEX_FLOATS: usize = 7;

impl Primitive {
    fn push_vertex(&mut self, x: f32, y: f32, z: f32, nx: f32, ny: f32, nz: f32) {
        self.positions.extend([x, y, z]);
        self.normals.extend([nx, ny, nz]);
    }

    fn quad(&mut self, corners: [[f32; 3]; 4], normals: [[f32; 3]; 4]) {
        for index in [0, 1, 2, 0, 2, 3] {
            let [x, y, z] = corners[index];
            let [nx, ny, nz] = normals[index];
            self.push_vertex(x, y, z, nx, ny, nz);
        }
    }
}

/// Swaps the second and third vertex of the triangle starting at float `offset`.
fn swap_bc(values: &mut [f32], offset: usize) {
    for component in 0..3 {
        values.swap(offset + 3 + component, offset + 6 + component);
    }
}

fn face_normal(p: &[f32], offset: usize) -> (f32, f32, f32) {
    let abx = p[offset + 3] - p[offset];
    let aby = p[offset + 4] - p[offset + 1];
    let abz = p[offset + 5] - p[offset + 2];
    let acx = p[offset + 6] - p[offset];
    let acy = p[offset + 7] - p[offset + 1];
    let acz = p[offset + 8] - p[offset + 2];
    (aby * acz - abz * acy, abz * acx - abx * acz, abx * acy - aby * acx)
}

/// Flips any triangle whose winding disagrees with its vertex normals (keeps front faces outward).
fn orient_triangles(mut primitive: Primitive) -> Primitive {
    for offset in (0..primitive.positions.len()).step_by(9) {
        let (fx, fy, fz) = face_normal(&primitive.positions, offset);
        let n = &primitive.normals;
        let nx = n[offset] + n[offset + 3] + n[offset + 6];
        let ny = n[offset + 1] + n[offset + 4] + n[offset + 7];
        let nz = n[offset + 2] + n[offset + 5] + n[offset + 8];
        if fx * nx + fy * ny + fz * nz < 0.0 {
            swap_bc(&mut primitive.positions, offset);
            swap_bc(&mut primitive.normals, offset);
        }
    }
    primitive
}

/// Recomputes per-face (flat) normals from triangle winding.
pub fn recompute_flat_normals(mut primitive: Primitive) -> Primitive {
    let count = primitive.positions.len();
    primitive.normals.resize(count, 0.0);
    for offset in (0..count).step_by(9) {
        let (fx, fy, fz) = face_normal(&primitive.positions, offset);
        let length = safe_length(fx, fy, fz);
        for vertex in 0..3 {
            let base = offset + vertex * 3;
            primitive.normals[base] = fx / length;
            primitive.normals[base + 1] = fy / length;
            primitive.normals[base + 2] = fz / length;
        }
    }
    primitive
}

// ---------------------------------------------------------------- primitives

#[inline(never)]
pub fn box3(width: f32, height: f32, depth: f32) -> Primitive {
    let (hx, hy, hz) = (width / 2.0, height / 2.0, depth / 2.0);
    let mut primitive = Primitive::default();
    let faces: [([f32; 3], [[f32; 3]; 4]); 6] = [
        ([1.0, 0.0, 0.0], [[hx, -hy, -hz], [hx, hy, -hz], [hx, hy, hz], [hx, -hy, hz]]),
        ([-1.0, 0.0, 0.0], [[-hx, -hy, hz], [-hx, hy, hz], [-hx, hy, -hz], [-hx, -hy, -hz]]),
        ([0.0, 1.0, 0.0], [[-hx, hy, -hz], [-hx, hy, hz], [hx, hy, hz], [hx, hy, -hz]]),
        ([0.0, -1.0, 0.0], [[-hx, -hy, hz], [-hx, -hy, -hz], [hx, -hy, -hz], [hx, -hy, hz]]),
        ([0.0, 0.0, 1.0], [[hx, -hy, hz], [hx, hy, hz], [-hx, hy, hz], [-hx, -hy, hz]]),
        ([0.0, 0.0, -1.0], [[-hx, -hy, -hz], [-hx, hy, -hz], [hx, hy, -hz], [hx, -hy, -hz]]),
    ];
    for (normal, corners) in faces {
        primitive.quad(corners, [normal; 4]);
    }
    primitive
}

/// Cylinder along Y centered at the origin (a cone when `radius_top` is 0). Segment 0 sits
/// on +Z, matching the convention the models' rotations were tuned for.
pub fn cylinder(radius_top: f32, radius_bottom: f32, height: f32, segments: u32) -> Primitive {
    let mut primitive = Primitive::default();
    let half_height = height / 2.0;
    let slope = (radius_bottom - radius_top) / height;
    let ring = |index: u32| (index as f32 / segments as f32 * PI * 2.0).sin_cos();
    for index in 0..segments {
        let (a_sin, a_cos) = ring(index);
        let (b_sin, b_cos) = ring(index + 1);
        let length_a = safe_length(a_sin, slope, a_cos);
        let length_b = safe_length(b_sin, slope, b_cos);
        let normal_a = [a_sin / length_a, slope / length_a, a_cos / length_a];
        let normal_b = [b_sin / length_b, slope / length_b, b_cos / length_b];
        primitive.quad(
            [
                [radius_bottom * a_sin, -half_height, radius_bottom * a_cos],
                [radius_bottom * b_sin, -half_height, radius_bottom * b_cos],
                [radius_top * b_sin, half_height, radius_top * b_cos],
                [radius_top * a_sin, half_height, radius_top * a_cos],
            ],
            [normal_a, normal_b, normal_b, normal_a],
        );
        if radius_top > 0.0 {
            primitive.push_vertex(0.0, half_height, 0.0, 0.0, 1.0, 0.0);
            primitive.push_vertex(radius_top * a_sin, half_height, radius_top * a_cos, 0.0, 1.0, 0.0);
            primitive.push_vertex(radius_top * b_sin, half_height, radius_top * b_cos, 0.0, 1.0, 0.0);
        }
        if radius_bottom > 0.0 {
            primitive.push_vertex(0.0, -half_height, 0.0, 0.0, -1.0, 0.0);
            primitive.push_vertex(radius_bottom * b_sin, -half_height, radius_bottom * b_cos, 0.0, -1.0, 0.0);
            primitive.push_vertex(radius_bottom * a_sin, -half_height, radius_bottom * a_cos, 0.0, -1.0, 0.0);
        }
    }
    orient_triangles(primitive)
}

pub fn cone(radius: f32, height: f32, segments: u32) -> Primitive {
    cylinder(0.0, radius, height, segments)
}

/// UV sphere; `phi_start`/`phi_length` sweep around Y from -X (phi = 0) toward +Z, so a
/// half sweep from pi covers the z <= 0 hemisphere.
pub fn sphere(radius: f32, width_segments: u32, height_segments: u32, phi_start: f32, phi_length: f32) -> Primitive {
    let mut primitive = Primitive::default();
    let point = |u: f32, v: f32| {
        let phi = phi_start + u * phi_length;
        let theta = v * PI;
        let x = -phi.cos() * theta.sin();
        let y = theta.cos();
        let z = phi.sin() * theta.sin();
        [x, y, z]
    };
    let push = |primitive: &mut Primitive, [x, y, z]: [f32; 3]| {
        primitive.push_vertex(x * radius, y * radius, z * radius, x, y, z);
    };
    let (columns, rows) = (width_segments as f32, height_segments as f32);
    for row in 0..height_segments {
        for column in 0..width_segments {
            let (column, row_f) = (column as f32, row as f32);
            let a = point(column / columns, row_f / rows);
            let b = point((column + 1.0) / columns, row_f / rows);
            let c = point((column + 1.0) / columns, (row_f + 1.0) / rows);
            let d = point(column / columns, (row_f + 1.0) / rows);
            if row != 0 {
                for vertex in [a, b, d] {
                    push(&mut primitive, vertex);
                }
            }
            if row != height_segments - 1 {
                for vertex in [b, c, d] {
                    push(&mut primitive, vertex);
                }
            }
        }
    }
    orient_triangles(primitive)
}

/// Torus lying in the XY plane (rotate a quarter turn about X to lay it flat).
pub fn torus(radius: f32, tube: f32, radial_segments: u32, tubular_segments: u32) -> Primitive {
    let mut primitive = Primitive::default();
    let point = |i: u32, j: u32| {
        let u = j as f32 / tubular_segments as f32 * PI * 2.0;
        let v = i as f32 / radial_segments as f32 * PI * 2.0;
        let center_x = radius * u.cos();
        let center_y = radius * u.sin();
        let x = (radius + tube * v.cos()) * u.cos();
        let y = (radius + tube * v.cos()) * u.sin();
        let z = tube * v.sin();
        let nx = x - center_x;
        let ny = y - center_y;
        let length = safe_length(nx, ny, z);
        ([x, y, z], [nx / length, ny / length, z / length])
    };
    for i in 0..radial_segments {
        for j in 0..tubular_segments {
            let a = point(i, j);
            let b = point(i, j + 1);
            let c = point(i + 1, j + 1);
            let d = point(i + 1, j);
            primitive.quad([a.0, b.0, c.0, d.0], [a.1, b.1, c.1, d.1]);
        }
    }
    orient_triangles(primitive)
}

pub fn octahedron(radius: f32) -> Primitive {
    let r = radius;
    let px = [r, 0.0, 0.0];
    let nx = [-r, 0.0, 0.0];
    let py = [0.0, r, 0.0];
    let ny = [0.0, -r, 0.0];
    let pz = [0.0, 0.0, r];
    let nz = [0.0, 0.0, -r];
    let faces = [
        [px, py, pz],
        [px, pz, ny],
        [px, ny, nz],
        [px, nz, py],
        [nx, pz, py],
        [nx, ny, pz],
        [nx, nz, ny],
        [nx, py, nz],
    ];
    let mut primitive = Primitive::default();
    for face in faces {
        for [x, y, z] in face {
            primitive.push_vertex(x, y, z, 0.0, 0.0, 0.0);
        }
    }
    orient_outward(recompute_flat_normals(primitive))
}

/// Flips flat-shaded triangles so their normals point away from the origin.
fn orient_outward(mut primitive: Primitive) -> Primitive {
    for offset in (0..primitive.positions.len()).step_by(9) {
        let p = &primitive.positions;
        let n = &primitive.normals;
        let cx = p[offset] + p[offset + 3] + p[offset + 6];
        let cy = p[offset + 1] + p[offset + 4] + p[offset + 7];
        let cz = p[offset + 2] + p[offset + 5] + p[offset + 8];
        if cx * n[offset] + cy * n[offset + 1] + cz * n[offset + 2] < 0.0 {
            swap_bc(&mut primitive.positions, offset);
        }
    }
    recompute_flat_normals(primitive)
}

/// Disc in the XY plane facing +Z.
pub fn circle(radius: f32, segments: u32) -> Primitive {
    let mut primitive = Primitive::default();
    for index in 0..segments {
        let a = index as f32 / segments as f32 * PI * 2.0;
        let b = (index + 1) as f32 / segments as f32 * PI * 2.0;
        primitive.push_vertex(0.0, 0.0, 0.0, 0.0, 0.0, 1.0);
        primitive.push_vertex(a.cos() * radius, a.sin() * radius, 0.0, 0.0, 0.0, 1.0);
        primitive.push_vertex(b.cos() * radius, b.sin() * radius, 0.0, 0.0, 0.0, 1.0);
    }
    orient_triangles(primitive)
}

// ---------------------------------------------------------------- outlines

fn signed_area(outline: &[P2]) -> f32 {
    let mut area = 0.0;
    for (index, a) in outline.iter().enumerate() {
        let b = outline[(index + 1) % outline.len()];
        area += a.x * b.y - b.x * a.y;
    }
    area / 2.0
}

fn cross(a: P2, b: P2, c: P2) -> f32 {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}

/// Ear-clipping triangulation of a simple polygon; returns index triplets.
fn triangulate(outline: &[P2]) -> Vec<usize> {
    let counter_clockwise = signed_area(outline) > 0.0;
    let mut remaining: Vec<usize> = (0..outline.len()).collect();
    let mut triangles = Vec::new();
    let inside = |p: P2, a: P2, b: P2, c: P2| {
        let d1 = cross(a, b, p);
        let d2 = cross(b, c, p);
        let d3 = cross(c, a, p);
        if counter_clockwise { d1 > 0.0 && d2 > 0.0 && d3 > 0.0 } else { d1 < 0.0 && d2 < 0.0 && d3 < 0.0 }
    };
    let mut guard = 0;
    while remaining.len() > 3 && guard < 1000 {
        guard += 1;
        let mut clipped = false;
        for index in 0..remaining.len() {
            let previous = remaining[(index + remaining.len() - 1) % remaining.len()];
            let current = remaining[index];
            let next = remaining[(index + 1) % remaining.len()];
            let (a, b, c) = (outline[previous], outline[current], outline[next]);
            let turn = cross(a, b, c);
            if if counter_clockwise { turn <= 0.0 } else { turn >= 0.0 } {
                continue;
            }
            if remaining
                .iter()
                .any(|&other| other != previous && other != current && other != next && inside(outline[other], a, b, c))
            {
                continue;
            }
            triangles.extend([previous, current, next]);
            remaining.remove(index);
            clipped = true;
            break;
        }
        if !clipped {
            break;
        }
    }
    if remaining.len() == 3 {
        triangles.extend([remaining[0], remaining[1], remaining[2]]);
    }
    triangles
}

/// Offsets every vertex along its mitered outward normal.
fn offset_outline(outline: &[P2], distance: f32) -> Vec<P2> {
    let orientation = if signed_area(outline) > 0.0 { 1.0 } else { -1.0 };
    let length_or_one = |x: f32, y: f32| {
        let length = x.hypot(y);
        if length > 0.0 { length } else { 1.0 }
    };
    outline
        .iter()
        .enumerate()
        .map(|(index, point)| {
            let previous = outline[(index + outline.len() - 1) % outline.len()];
            let next = outline[(index + 1) % outline.len()];
            let (in_x, in_y) = (point.x - previous.x, point.y - previous.y);
            let (out_x, out_y) = (next.x - point.x, next.y - point.y);
            let length_in = length_or_one(in_x, in_y);
            let length_out = length_or_one(out_x, out_y);
            // Outward normals of the two adjacent edges (right-hand side for CCW outlines).
            let normal_in = p2(in_y / length_in * orientation, -in_x / length_in * orientation);
            let normal_out = p2(out_y / length_out * orientation, -out_x / length_out * orientation);
            let mut mx = normal_in.x + normal_out.x;
            let mut my = normal_in.y + normal_out.y;
            let m_length = length_or_one(mx, my);
            mx /= m_length;
            my /= m_length;
            // Exact offset-line intersection, capped at sqrt(2) like three.js ExtrudeGeometry bevels.
            let miter = SQRT_2.min(1.0 / (mx * normal_in.x + my * normal_in.y).max(1e-6));
            p2(point.x + mx * distance * miter, point.y + my * distance * miter)
        })
        .collect()
}

/// Extrudes a 2D field-space outline (x forward, y toward screen-bottom) upward along +Y
/// from y = 0, so the footprint matches the 2D silhouette seen from above. A nonzero bevel
/// widens the walls by `bevel` and chamfers the rims with one step top and bottom.
pub fn extrude_outline(outline: &[P2], height: f32, bevel: f32) -> Primitive {
    let mut primitive = Primitive::default();
    let expanded = if bevel > 0.0 { offset_outline(outline, bevel) } else { outline.to_vec() };
    let rings: Vec<(&[P2], f32)> = if bevel > 0.0 {
        vec![(outline, 0.0), (&expanded, bevel), (&expanded, bevel + height), (outline, height + bevel * 2.0)]
    } else {
        vec![(outline, 0.0), (outline, height)]
    };

    // Field (x, y) maps to world (x, z). A cap triangle faces +Y when it is clockwise in
    // field coordinates; walls face outward when wound against the outline's orientation.
    let positive_area = signed_area(outline) > 0.0;
    let triangles = triangulate(outline);
    let (bottom, bottom_y) = rings[0];
    let (top, top_y) = rings[rings.len() - 1];
    for &[a, b, c] in triangles.as_chunks::<3>().0 {
        let clockwise = cross(outline[a], outline[b], outline[c]) < 0.0;
        let upward = if clockwise { [a, b, c] } else { [a, c, b] };
        for vertex in upward {
            primitive.push_vertex(top[vertex].x, top_y, top[vertex].y, 0.0, 0.0, 0.0);
        }
        for vertex in [upward[0], upward[2], upward[1]] {
            primitive.push_vertex(bottom[vertex].x, bottom_y, bottom[vertex].y, 0.0, 0.0, 0.0);
        }
    }
    for pair in rings.windows(2) {
        let (lower, lower_y) = pair[0];
        let (upper, upper_y) = pair[1];
        for index in 0..outline.len() {
            let next = (index + 1) % outline.len();
            let lower_a = [lower[index].x, lower_y, lower[index].y];
            let lower_b = [lower[next].x, lower_y, lower[next].y];
            let upper_b = [upper[next].x, upper_y, upper[next].y];
            let upper_a = [upper[index].x, upper_y, upper[index].y];
            let corners =
                if positive_area { [lower_b, lower_a, upper_a, upper_b] } else { [lower_a, lower_b, upper_b, upper_a] };
            primitive.quad(corners, [[0.0; 3]; 4]);
        }
    }
    recompute_flat_normals(primitive)
}

pub fn scale_outline(outline: &[P2], scale: f32) -> Vec<P2> {
    outline.iter().map(|point| p2(point.x * scale, point.y * scale)).collect()
}

/// Axis-aligned box turned by a 2D field angle and centered at a field point.
#[inline(never)]
pub fn bar(center_x: f32, y: f32, center_z: f32, field_angle: f32, length: f32, height: f32, width: f32) -> Primitive {
    transform(box3(length, height, width), &place(center_x, y, center_z, 0.0, -field_angle, 0.0, 1.0, 1.0, 1.0))
}

/// Thin glowing bars along each outline edge at a fixed height: the 3D stand-in for the 2D stroke.
pub fn outline_trim(outline: &[P2], y: f32, thickness: f32, height: f32) -> Vec<Primitive> {
    let mut bars = Vec::with_capacity(outline.len());
    for (index, start) in outline.iter().enumerate() {
        let end = outline[(index + 1) % outline.len()];
        let dx = end.x - start.x;
        let dy = end.y - start.y;
        let length = dx.hypot(dy);
        if length < 0.01 {
            continue;
        }
        bars.push(bar(start.x + dx / 2.0, y, start.y + dy / 2.0, dy.atan2(dx), length + thickness, height, thickness));
    }
    bars
}

/// Flat-shaded solid from explicit triangles.
pub fn faceted_solid(triangles: &[[Vec3; 3]]) -> Primitive {
    let mut primitive = Primitive::default();
    for triangle in triangles {
        for vertex in triangle {
            primitive.push_vertex(vertex.x, vertex.y, vertex.z, 0.0, 0.0, 0.0);
        }
    }
    recompute_flat_normals(primitive)
}

/// Closed bipyramid over a field-space outline with apexes above and below, wound outward.
pub fn bipyramid(outline: &[P2], top_apex: Vec3, bottom_apex: Vec3) -> Primitive {
    let ring: Vec<Vec3> = outline.iter().map(|point| Vec3 { x: point.x, y: 0.0, z: point.y }).collect();
    let count = ring.len() as f32;
    let center_x = ring.iter().map(|vertex| vertex.x).sum::<f32>() / count;
    let center_z = ring.iter().map(|vertex| vertex.z).sum::<f32>() / count;
    let mut triangles = Vec::with_capacity(ring.len() * 2);
    let mut push_outward = |a: Vec3, b: Vec3, c: Vec3| {
        let (abx, aby, abz) = (b.x - a.x, b.y - a.y, b.z - a.z);
        let (acx, acy, acz) = (c.x - a.x, c.y - a.y, c.z - a.z);
        let fx = aby * acz - abz * acy;
        let fy = abz * acx - abx * acz;
        let fz = abx * acy - aby * acx;
        let px = (a.x + b.x + c.x) / 3.0 - center_x;
        let py = (a.y + b.y + c.y) / 3.0;
        let pz = (a.z + b.z + c.z) / 3.0 - center_z;
        triangles.push(if fx * px + fy * py + fz * pz >= 0.0 { [a, b, c] } else { [a, c, b] });
    };
    for (index, current) in ring.iter().enumerate() {
        let next = ring[(index + 1) % ring.len()];
        push_outward(*current, next, top_apex);
        push_outward(next, *current, bottom_apex);
    }
    faceted_solid(&triangles)
}

// ---------------------------------------------------------------- assembly

#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub fn place(
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
    mat4_compose(x, y, z, rotation_x, rotation_y, rotation_z, scale_x, scale_y, scale_z)
}

#[inline(never)]
pub fn translate(x: f32, y: f32, z: f32) -> Mat4 {
    mat4_compose(x, y, z, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0)
}

pub fn transform(mut primitive: Primitive, matrix: &Mat4) -> Primitive {
    let normals = normal_matrix(matrix);
    let positions = primitive.positions.as_chunks_mut::<3>().0.iter_mut();
    for (position, normal) in positions.zip(primitive.normals.as_chunks_mut::<3>().0.iter_mut()) {
        let [x, y, z] = *position;
        position[0] = matrix[0] * x + matrix[4] * y + matrix[8] * z + matrix[12];
        position[1] = matrix[1] * x + matrix[5] * y + matrix[9] * z + matrix[13];
        position[2] = matrix[2] * x + matrix[6] * y + matrix[10] * z + matrix[14];
        let [nx, ny, nz] = *normal;
        let tx = normals[0] * nx + normals[1] * ny + normals[2] * nz;
        let ty = normals[3] * nx + normals[4] * ny + normals[5] * nz;
        let tz = normals[6] * nx + normals[7] * ny + normals[8] * nz;
        let length = safe_length(tx, ty, tz);
        normal[0] = tx / length;
        normal[1] = ty / length;
        normal[2] = tz / length;
    }
    primitive
}

/// Tags a primitive with a constant glow mask, optionally transforming it first.
#[inline(never)]
pub fn solid(primitive: Primitive, glow: f32, matrix: Option<&Mat4>) -> Part {
    let transformed = match matrix {
        Some(matrix) => transform(primitive, matrix),
        None => primitive,
    };
    let vertex_count = transformed.positions.len() / 3;
    Part { positions: transformed.positions, normals: transformed.normals, glow: vec![glow; vertex_count] }
}

#[inline(never)]
pub fn merge(parts: Vec<Part>) -> Part {
    let mut merged = Part::default();
    for part in parts {
        merged.positions.extend(part.positions);
        merged.normals.extend(part.normals);
        merged.glow.extend(part.glow);
    }
    merged
}

pub fn to_neon_mesh(part: &Part) -> Mesh {
    let vertex_count = part.glow.len();
    let mut vertices = Vec::with_capacity(vertex_count * NEON_VERTEX_FLOATS);
    for vertex in 0..vertex_count {
        vertices.extend_from_slice(&part.positions[vertex * 3..vertex * 3 + 3]);
        vertices.extend_from_slice(&part.normals[vertex * 3..vertex * 3 + 3]);
        vertices.push(part.glow[vertex]);
    }
    Mesh { vertices, vertex_count: vertex_count as u32 }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_outward(primitive: &Primitive) {
        for offset in (0..primitive.positions.len()).step_by(9) {
            let (fx, fy, fz) = face_normal(&primitive.positions, offset);
            let n = &primitive.normals;
            let dot = fx * (n[offset] + n[offset + 3] + n[offset + 6])
                + fy * (n[offset + 1] + n[offset + 4] + n[offset + 7])
                + fz * (n[offset + 2] + n[offset + 5] + n[offset + 8]);
            assert!(dot >= -1e-6, "triangle at {offset} winds against its normals");
        }
    }

    #[test]
    fn primitive_vertex_counts() {
        assert_eq!(box3(1.0, 2.0, 3.0).positions.len() / 3, 36);
        assert_eq!(cylinder(1.0, 2.0, 3.0, 8).positions.len() / 3, 8 * 12);
        assert_eq!(cone(1.0, 2.0, 6).positions.len() / 3, 6 * 9);
        assert_eq!(sphere(1.0, 10, 8, 0.0, PI * 2.0).positions.len() / 3, 10 * (8 * 2 - 2) * 3);
        assert_eq!(torus(1.0, 0.1, 4, 6).positions.len() / 3, 4 * 6 * 6);
        assert_eq!(octahedron(1.0).positions.len() / 3, 24);
        assert_eq!(circle(1.0, 12).positions.len() / 3, 36);
        for primitive in [box3(1.0, 2.0, 3.0), cylinder(1.0, 2.0, 3.0, 8), sphere(1.0, 10, 8, 0.0, PI * 2.0)] {
            assert_outward(&primitive);
        }
    }

    #[test]
    fn triangulates_concave_outlines() {
        let arrow = [p2(1.8, 0.0), p2(0.28, -0.86), p2(-1.35, -0.58), p2(-0.92, 0.0), p2(-1.35, 0.58), p2(0.28, 0.86)];
        let triangles = triangulate(&arrow);
        assert_eq!(triangles.len(), (arrow.len() - 2) * 3);
        let area: f32 =
            triangles.as_chunks::<3>().0.iter().map(|&[a, b, c]| cross(arrow[a], arrow[b], arrow[c]).abs() / 2.0).sum();
        assert!((area - signed_area(&arrow).abs()).abs() < 1e-4);
    }

    #[test]
    fn extruded_caps_face_up_and_down() {
        let square = [p2(-1.0, -1.0), p2(1.0, -1.0), p2(1.0, 1.0), p2(-1.0, 1.0)];
        let solid = extrude_outline(&square, 1.0, 0.1);
        // Two caps of two triangles plus three wall bands of four quads.
        assert_eq!(solid.positions.len() / 3, 2 * 2 * 3 + 3 * 4 * 6);
        assert!((solid.normals[1] - 1.0).abs() < 1e-6, "first cap triangle faces up");
        let top = solid.positions.iter().skip(1).step_by(3).fold(0.0f32, |a, &b| a.max(b));
        assert!((top - 1.2).abs() < 1e-6);
    }
}

import type { Point } from "../types";
import { mat4Compose, normalMatrix, type Mat4, type Vec3 } from "./math";

/** Non-indexed triangle soup: three floats per vertex for positions and normals. */
export interface Primitive {
  positions: number[];
  normals: number[];
}

/** A primitive tagged with its glow mask, ready to merge into a neon part. */
export interface Part extends Primitive {
  glow: number[];
}

/** Interleaved `position(3) normal(3) glow(1)` vertices for the neon pipeline. */
export interface NeonMesh {
  readonly vertices: Float32Array;
  readonly vertexCount: number;
}

export const NEON_VERTEX_FLOATS = 7;

function pushVertex(primitive: Primitive, x: number, y: number, z: number, nx: number, ny: number, nz: number): void {
  primitive.positions.push(x, y, z);
  primitive.normals.push(nx, ny, nz);
}

/** Flips any triangle whose winding disagrees with its vertex normals (keeps front faces outward). */
function orientTriangles(primitive: Primitive): Primitive {
  const p = primitive.positions;
  const n = primitive.normals;
  for (let offset = 0; offset < p.length; offset += 9) {
    const abx = p[offset + 3] - p[offset];
    const aby = p[offset + 4] - p[offset + 1];
    const abz = p[offset + 5] - p[offset + 2];
    const acx = p[offset + 6] - p[offset];
    const acy = p[offset + 7] - p[offset + 1];
    const acz = p[offset + 8] - p[offset + 2];
    const fx = (aby * acz) - (abz * acy);
    const fy = (abz * acx) - (abx * acz);
    const fz = (abx * acy) - (aby * acx);
    const nx = n[offset] + n[offset + 3] + n[offset + 6];
    const ny = n[offset + 1] + n[offset + 4] + n[offset + 7];
    const nz = n[offset + 2] + n[offset + 5] + n[offset + 8];
    if ((fx * nx) + (fy * ny) + (fz * nz) < 0) {
      for (let component = 0; component < 3; component += 1) {
        const bIndex = offset + 3 + component;
        const cIndex = offset + 6 + component;
        [p[bIndex], p[cIndex]] = [p[cIndex], p[bIndex]];
        [n[bIndex], n[cIndex]] = [n[cIndex], n[bIndex]];
      }
    }
  }
  return primitive;
}

/** Recomputes per-face (flat) normals from triangle winding. */
export function recomputeFlatNormals(primitive: Primitive): Primitive {
  return flatNormals(primitive);
}

function flatNormals(primitive: Primitive): Primitive {
  const p = primitive.positions;
  const n = primitive.normals;
  n.length = p.length;
  for (let offset = 0; offset < p.length; offset += 9) {
    const abx = p[offset + 3] - p[offset];
    const aby = p[offset + 4] - p[offset + 1];
    const abz = p[offset + 5] - p[offset + 2];
    const acx = p[offset + 6] - p[offset];
    const acy = p[offset + 7] - p[offset + 1];
    const acz = p[offset + 8] - p[offset + 2];
    let fx = (aby * acz) - (abz * acy);
    let fy = (abz * acx) - (abx * acz);
    let fz = (abx * acy) - (aby * acx);
    const length = Math.hypot(fx, fy, fz) || 1;
    fx /= length;
    fy /= length;
    fz /= length;
    for (let vertex = 0; vertex < 3; vertex += 1) {
      n[offset + (vertex * 3)] = fx;
      n[offset + (vertex * 3) + 1] = fy;
      n[offset + (vertex * 3) + 2] = fz;
    }
  }
  return primitive;
}

function quad(primitive: Primitive, corners: number[][], normals: number[][]): void {
  for (const index of [0, 1, 2, 0, 2, 3]) {
    const [x, y, z] = corners[index];
    const [nx, ny, nz] = normals[index];
    pushVertex(primitive, x, y, z, nx, ny, nz);
  }
}

// ---------------------------------------------------------------- primitives

export function box(width: number, height: number, depth: number): Primitive {
  const hx = width / 2;
  const hy = height / 2;
  const hz = depth / 2;
  const primitive: Primitive = { positions: [], normals: [] };
  const faces: Array<[number[], number[][]]> = [
    [[1, 0, 0], [[hx, -hy, -hz], [hx, hy, -hz], [hx, hy, hz], [hx, -hy, hz]]],
    [[-1, 0, 0], [[-hx, -hy, hz], [-hx, hy, hz], [-hx, hy, -hz], [-hx, -hy, -hz]]],
    [[0, 1, 0], [[-hx, hy, -hz], [-hx, hy, hz], [hx, hy, hz], [hx, hy, -hz]]],
    [[0, -1, 0], [[-hx, -hy, hz], [-hx, -hy, -hz], [hx, -hy, -hz], [hx, -hy, hz]]],
    [[0, 0, 1], [[hx, -hy, hz], [hx, hy, hz], [-hx, hy, hz], [-hx, -hy, hz]]],
    [[0, 0, -1], [[-hx, -hy, -hz], [-hx, hy, -hz], [hx, hy, -hz], [hx, -hy, -hz]]],
  ];
  for (const [normal, corners] of faces) {
    quad(primitive, corners, [normal, normal, normal, normal]);
  }
  return primitive;
}

/**
 * Cylinder along Y centered at the origin (a cone when `radiusTop` is 0). Segment 0 sits
 * on +Z, matching the convention the models' rotations were tuned for.
 */
export function cylinder(radiusTop: number, radiusBottom: number, height: number, segments: number): Primitive {
  const primitive: Primitive = { positions: [], normals: [] };
  const halfHeight = height / 2;
  const slope = (radiusBottom - radiusTop) / height;
  const ring = (index: number) => {
    const theta = (index / segments) * Math.PI * 2;
    return { sin: Math.sin(theta), cos: Math.cos(theta) };
  };
  for (let index = 0; index < segments; index += 1) {
    const a = ring(index);
    const b = ring(index + 1);
    const normalA = [a.sin, slope, a.cos];
    const normalB = [b.sin, slope, b.cos];
    const normalizedA = normalA.map((value) => value / Math.hypot(...normalA));
    const normalizedB = normalB.map((value) => value / Math.hypot(...normalB));
    quad(
      primitive,
      [
        [radiusBottom * a.sin, -halfHeight, radiusBottom * a.cos],
        [radiusBottom * b.sin, -halfHeight, radiusBottom * b.cos],
        [radiusTop * b.sin, halfHeight, radiusTop * b.cos],
        [radiusTop * a.sin, halfHeight, radiusTop * a.cos],
      ],
      [normalizedA, normalizedB, normalizedB, normalizedA],
    );
    if (radiusTop > 0) {
      pushVertex(primitive, 0, halfHeight, 0, 0, 1, 0);
      pushVertex(primitive, radiusTop * a.sin, halfHeight, radiusTop * a.cos, 0, 1, 0);
      pushVertex(primitive, radiusTop * b.sin, halfHeight, radiusTop * b.cos, 0, 1, 0);
    }
    if (radiusBottom > 0) {
      pushVertex(primitive, 0, -halfHeight, 0, 0, -1, 0);
      pushVertex(primitive, radiusBottom * b.sin, -halfHeight, radiusBottom * b.cos, 0, -1, 0);
      pushVertex(primitive, radiusBottom * a.sin, -halfHeight, radiusBottom * a.cos, 0, -1, 0);
    }
  }
  return orientTriangles(primitive);
}

export function cone(radius: number, height: number, segments: number): Primitive {
  return cylinder(0, radius, height, segments);
}

/**
 * UV sphere; `phiStart`/`phiLength` sweep around Y from -X (phi = 0) toward +Z, so a
 * half sweep from pi covers the z <= 0 hemisphere.
 */
export function sphere(radius: number, widthSegments: number, heightSegments: number, phiStart: number, phiLength: number): Primitive {
  const primitive: Primitive = { positions: [], normals: [] };
  const point = (u: number, v: number) => {
    const phi = phiStart + (u * phiLength);
    const theta = v * Math.PI;
    const x = -Math.cos(phi) * Math.sin(theta);
    const y = Math.cos(theta);
    const z = Math.sin(phi) * Math.sin(theta);
    return { position: [x * radius, y * radius, z * radius], normal: [x, y, z] };
  };
  for (let row = 0; row < heightSegments; row += 1) {
    for (let column = 0; column < widthSegments; column += 1) {
      const a = point(column / widthSegments, row / heightSegments);
      const b = point((column + 1) / widthSegments, row / heightSegments);
      const c = point((column + 1) / widthSegments, (row + 1) / heightSegments);
      const d = point(column / widthSegments, (row + 1) / heightSegments);
      if (row !== 0) {
        for (const vertex of [a, b, d]) {
          pushVertex(primitive, vertex.position[0], vertex.position[1], vertex.position[2], vertex.normal[0], vertex.normal[1], vertex.normal[2]);
        }
      }
      if (row !== heightSegments - 1) {
        for (const vertex of [b, c, d]) {
          pushVertex(primitive, vertex.position[0], vertex.position[1], vertex.position[2], vertex.normal[0], vertex.normal[1], vertex.normal[2]);
        }
      }
    }
  }
  return orientTriangles(primitive);
}

/** Torus lying in the XY plane (rotate a quarter turn about X to lay it flat). */
export function torus(radius: number, tube: number, radialSegments: number, tubularSegments: number): Primitive {
  const primitive: Primitive = { positions: [], normals: [] };
  const point = (i: number, j: number) => {
    const u = (j / tubularSegments) * Math.PI * 2;
    const v = (i / radialSegments) * Math.PI * 2;
    const centerX = radius * Math.cos(u);
    const centerY = radius * Math.sin(u);
    const x = (radius + (tube * Math.cos(v))) * Math.cos(u);
    const y = (radius + (tube * Math.cos(v))) * Math.sin(u);
    const z = tube * Math.sin(v);
    const nx = x - centerX;
    const ny = y - centerY;
    const length = Math.hypot(nx, ny, z) || 1;
    return { position: [x, y, z], normal: [nx / length, ny / length, z / length] };
  };
  for (let i = 0; i < radialSegments; i += 1) {
    for (let j = 0; j < tubularSegments; j += 1) {
      const a = point(i, j);
      const b = point(i, j + 1);
      const c = point(i + 1, j + 1);
      const d = point(i + 1, j);
      quad(primitive, [a.position, b.position, c.position, d.position], [a.normal, b.normal, c.normal, d.normal]);
    }
  }
  return orientTriangles(primitive);
}

export function octahedron(radius: number): Primitive {
  const r = radius;
  const px = [r, 0, 0];
  const nx = [-r, 0, 0];
  const py = [0, r, 0];
  const ny = [0, -r, 0];
  const pz = [0, 0, r];
  const nz = [0, 0, -r];
  const faces = [
    [px, py, pz], [px, pz, ny], [px, ny, nz], [px, nz, py],
    [nx, pz, py], [nx, ny, pz], [nx, nz, ny], [nx, py, nz],
  ];
  const primitive: Primitive = { positions: [], normals: [] };
  for (const face of faces) {
    for (const vertex of face) {
      pushVertex(primitive, vertex[0], vertex[1], vertex[2], 0, 0, 0);
    }
  }
  flatNormals(primitive);
  return orientOutward(primitive);
}

/** Flips flat-shaded triangles so their normals point away from the origin. */
function orientOutward(primitive: Primitive): Primitive {
  const p = primitive.positions;
  const n = primitive.normals;
  for (let offset = 0; offset < p.length; offset += 9) {
    const cx = p[offset] + p[offset + 3] + p[offset + 6];
    const cy = p[offset + 1] + p[offset + 4] + p[offset + 7];
    const cz = p[offset + 2] + p[offset + 5] + p[offset + 8];
    if ((cx * n[offset]) + (cy * n[offset + 1]) + (cz * n[offset + 2]) < 0) {
      for (let component = 0; component < 3; component += 1) {
        const bIndex = offset + 3 + component;
        const cIndex = offset + 6 + component;
        [p[bIndex], p[cIndex]] = [p[cIndex], p[bIndex]];
      }
    }
  }
  return flatNormals(primitive);
}

/** Disc in the XY plane facing +Z. */
export function circle(radius: number, segments: number): Primitive {
  const primitive: Primitive = { positions: [], normals: [] };
  for (let index = 0; index < segments; index += 1) {
    const a = (index / segments) * Math.PI * 2;
    const b = ((index + 1) / segments) * Math.PI * 2;
    pushVertex(primitive, 0, 0, 0, 0, 0, 1);
    pushVertex(primitive, Math.cos(a) * radius, Math.sin(a) * radius, 0, 0, 0, 1);
    pushVertex(primitive, Math.cos(b) * radius, Math.sin(b) * radius, 0, 0, 0, 1);
  }
  return orientTriangles(primitive);
}

// ---------------------------------------------------------------- outlines

function signedArea(outline: readonly Point[]): number {
  let area = 0;
  for (let index = 0; index < outline.length; index += 1) {
    const a = outline[index];
    const b = outline[(index + 1) % outline.length];
    area += (a.x * b.y) - (b.x * a.y);
  }
  return area / 2;
}

/** Ear-clipping triangulation of a simple polygon; returns index triplets. */
function triangulate(outline: readonly Point[]): number[] {
  const counterClockwise = signedArea(outline) > 0;
  const remaining = outline.map((_, index) => index);
  const triangles: number[] = [];
  const cross = (a: Point, b: Point, c: Point) => ((b.x - a.x) * (c.y - a.y)) - ((b.y - a.y) * (c.x - a.x));
  const inside = (p: Point, a: Point, b: Point, c: Point) => {
    const d1 = cross(a, b, p);
    const d2 = cross(b, c, p);
    const d3 = cross(c, a, p);
    return counterClockwise ? d1 > 0 && d2 > 0 && d3 > 0 : d1 < 0 && d2 < 0 && d3 < 0;
  };
  let guard = 0;
  while (remaining.length > 3 && guard < 1000) {
    guard += 1;
    let clipped = false;
    for (let index = 0; index < remaining.length; index += 1) {
      const previous = remaining[(index + remaining.length - 1) % remaining.length];
      const current = remaining[index];
      const next = remaining[(index + 1) % remaining.length];
      const a = outline[previous];
      const b = outline[current];
      const c = outline[next];
      const turn = cross(a, b, c);
      if (counterClockwise ? turn <= 0 : turn >= 0) {
        continue;
      }
      if (remaining.some((other) => other !== previous && other !== current && other !== next && inside(outline[other], a, b, c))) {
        continue;
      }
      triangles.push(previous, current, next);
      remaining.splice(index, 1);
      clipped = true;
      break;
    }
    if (!clipped) {
      break;
    }
  }
  if (remaining.length === 3) {
    triangles.push(remaining[0], remaining[1], remaining[2]);
  }
  return triangles;
}

/** Offsets every vertex along its mitered outward normal. */
function offsetOutline(outline: readonly Point[], distance: number): Point[] {
  const orientation = signedArea(outline) > 0 ? 1 : -1;
  return outline.map((point, index) => {
    const previous = outline[(index + outline.length - 1) % outline.length];
    const next = outline[(index + 1) % outline.length];
    const edgeIn = { x: point.x - previous.x, y: point.y - previous.y };
    const edgeOut = { x: next.x - point.x, y: next.y - point.y };
    const lengthIn = Math.hypot(edgeIn.x, edgeIn.y) || 1;
    const lengthOut = Math.hypot(edgeOut.x, edgeOut.y) || 1;
    // Outward normals of the two adjacent edges (right-hand side for CCW outlines).
    const normalIn = { x: (edgeIn.y / lengthIn) * orientation, y: (-edgeIn.x / lengthIn) * orientation };
    const normalOut = { x: (edgeOut.y / lengthOut) * orientation, y: (-edgeOut.x / lengthOut) * orientation };
    let mx = normalIn.x + normalOut.x;
    let my = normalIn.y + normalOut.y;
    const mLength = Math.hypot(mx, my) || 1;
    mx /= mLength;
    my /= mLength;
    // Exact offset-line intersection, capped at sqrt(2) like three.js ExtrudeGeometry bevels.
    const miter = Math.min(Math.SQRT2, 1 / Math.max(1e-6, (mx * normalIn.x) + (my * normalIn.y)));
    return { x: point.x + (mx * distance * miter), y: point.y + (my * distance * miter) };
  });
}

/**
 * Extrudes a 2D field-space outline (x forward, y toward screen-bottom) upward along +Y
 * from y = 0, so the footprint matches the 2D silhouette seen from above. A nonzero bevel
 * widens the walls by `bevel` and chamfers the rims with one step top and bottom.
 */
export function extrudeOutline(outline: readonly Point[], height: number, bevel: number): Primitive {
  const primitive: Primitive = { positions: [], normals: [] };
  const expanded = bevel > 0 ? offsetOutline(outline, bevel) : [...outline];
  const rings: Array<{ points: readonly Point[]; y: number }> = bevel > 0
    ? [
      { points: outline, y: 0 },
      { points: expanded, y: bevel },
      { points: expanded, y: bevel + height },
      { points: outline, y: height + (bevel * 2) },
    ]
    : [
      { points: outline, y: 0 },
      { points: outline, y: height },
    ];

  // Field (x, y) maps to world (x, z). A cap triangle faces +Y when it is clockwise in
  // field coordinates; walls face outward when wound against the outline's orientation.
  const positiveArea = signedArea(outline) > 0;
  const triangles = triangulate(outline);
  const bottom = rings[0];
  const top = rings[rings.length - 1];
  for (let index = 0; index < triangles.length; index += 3) {
    const a = triangles[index];
    const b = triangles[index + 1];
    const c = triangles[index + 2];
    const clockwise = ((outline[b].x - outline[a].x) * (outline[c].y - outline[a].y)) - ((outline[b].y - outline[a].y) * (outline[c].x - outline[a].x)) < 0;
    const upward = clockwise ? [a, b, c] : [a, c, b];
    for (const vertex of upward) {
      pushVertex(primitive, top.points[vertex].x, top.y, top.points[vertex].y, 0, 0, 0);
    }
    for (const vertex of [upward[0], upward[2], upward[1]]) {
      pushVertex(primitive, bottom.points[vertex].x, bottom.y, bottom.points[vertex].y, 0, 0, 0);
    }
  }
  for (let ring = 0; ring < rings.length - 1; ring += 1) {
    const lower = rings[ring];
    const upper = rings[ring + 1];
    for (let index = 0; index < outline.length; index += 1) {
      const next = (index + 1) % outline.length;
      const lowerA = [lower.points[index].x, lower.y, lower.points[index].y];
      const lowerB = [lower.points[next].x, lower.y, lower.points[next].y];
      const upperB = [upper.points[next].x, upper.y, upper.points[next].y];
      const upperA = [upper.points[index].x, upper.y, upper.points[index].y];
      const corners = positiveArea ? [lowerB, lowerA, upperA, upperB] : [lowerA, lowerB, upperB, upperA];
      quad(primitive, corners, [[0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0]]);
    }
  }
  return flatNormals(primitive);
}

export function scaleOutline(outline: readonly Point[], scale: number): Point[] {
  return outline.map((point) => ({ x: point.x * scale, y: point.y * scale }));
}

/** Axis-aligned box turned by a 2D field angle and centered at a field point. */
export function bar(
  centerX: number,
  y: number,
  centerZ: number,
  fieldAngle: number,
  length: number,
  height: number,
  width: number,
): Primitive {
  return transform(box(length, height, width), place(centerX, y, centerZ, 0, -fieldAngle, 0, 1, 1, 1));
}

/** Thin glowing bars along each outline edge at a fixed height: the 3D stand-in for the 2D stroke. */
export function outlineTrim(outline: readonly Point[], y: number, thickness: number, height: number): Primitive[] {
  const bars: Primitive[] = [];
  for (let index = 0; index < outline.length; index += 1) {
    const start = outline[index];
    const end = outline[(index + 1) % outline.length];
    const dx = end.x - start.x;
    const dy = end.y - start.y;
    const length = Math.hypot(dx, dy);
    if (length < 0.01) {
      continue;
    }
    bars.push(bar(start.x + (dx / 2), y, start.y + (dy / 2), Math.atan2(dy, dx), length + thickness, height, thickness));
  }
  return bars;
}

/** Flat-shaded solid from explicit triangles. */
export function facetedSolid(triangles: readonly (readonly [Vec3, Vec3, Vec3])[]): Primitive {
  const primitive: Primitive = { positions: [], normals: [] };
  for (const triangle of triangles) {
    for (const vertex of triangle) {
      pushVertex(primitive, vertex.x, vertex.y, vertex.z, 0, 0, 0);
    }
  }
  return flatNormals(primitive);
}

/** Closed bipyramid over a field-space outline with apexes above and below, wound outward. */
export function bipyramid(outline: readonly Point[], topApex: Vec3, bottomApex: Vec3): Primitive {
  const ring = outline.map((point) => ({ x: point.x, y: 0, z: point.y }));
  let centerX = 0;
  let centerZ = 0;
  for (const vertex of ring) {
    centerX += vertex.x;
    centerZ += vertex.z;
  }
  centerX /= ring.length;
  centerZ /= ring.length;
  const triangles: Array<[Vec3, Vec3, Vec3]> = [];
  const pushOutward = (a: Vec3, b: Vec3, c: Vec3): void => {
    const abx = b.x - a.x, aby = b.y - a.y, abz = b.z - a.z;
    const acx = c.x - a.x, acy = c.y - a.y, acz = c.z - a.z;
    const fx = (aby * acz) - (abz * acy);
    const fy = (abz * acx) - (abx * acz);
    const fz = (abx * acy) - (aby * acx);
    const px = ((a.x + b.x + c.x) / 3) - centerX;
    const py = (a.y + b.y + c.y) / 3;
    const pz = ((a.z + b.z + c.z) / 3) - centerZ;
    triangles.push((fx * px) + (fy * py) + (fz * pz) >= 0 ? [a, b, c] : [a, c, b]);
  };
  for (let index = 0; index < ring.length; index += 1) {
    const current = ring[index];
    const next = ring[(index + 1) % ring.length];
    pushOutward(current, next, topApex);
    pushOutward(next, current, bottomApex);
  }
  return facetedSolid(triangles);
}

// ---------------------------------------------------------------- assembly

export function place(
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
  return mat4Compose(x, y, z, rotationX, rotationY, rotationZ, scaleX, scaleY, scaleZ);
}

export function translate(x: number, y: number, z: number): Mat4 {
  return mat4Compose(x, y, z, 0, 0, 0, 1, 1, 1);
}

export function transform(primitive: Primitive, matrix: Mat4): Primitive {
  const normals = normalMatrix(matrix);
  const p = primitive.positions;
  const n = primitive.normals;
  for (let offset = 0; offset < p.length; offset += 3) {
    const x = p[offset];
    const y = p[offset + 1];
    const z = p[offset + 2];
    p[offset] = (matrix[0] * x) + (matrix[4] * y) + (matrix[8] * z) + matrix[12];
    p[offset + 1] = (matrix[1] * x) + (matrix[5] * y) + (matrix[9] * z) + matrix[13];
    p[offset + 2] = (matrix[2] * x) + (matrix[6] * y) + (matrix[10] * z) + matrix[14];
    const nx = n[offset];
    const ny = n[offset + 1];
    const nz = n[offset + 2];
    let tx = (normals[0] * nx) + (normals[1] * ny) + (normals[2] * nz);
    let ty = (normals[3] * nx) + (normals[4] * ny) + (normals[5] * nz);
    let tz = (normals[6] * nx) + (normals[7] * ny) + (normals[8] * nz);
    const length = Math.hypot(tx, ty, tz) || 1;
    tx /= length;
    ty /= length;
    tz /= length;
    n[offset] = tx;
    n[offset + 1] = ty;
    n[offset + 2] = tz;
  }
  return primitive;
}

/** Tags a primitive with a constant glow mask, optionally transforming it first. */
export function solid(primitive: Primitive, glow: number, matrix: Mat4 | null): Part {
  const transformed = matrix ? transform(primitive, matrix) : primitive;
  return {
    positions: transformed.positions,
    normals: transformed.normals,
    glow: new Array<number>(transformed.positions.length / 3).fill(glow),
  };
}

export function merge(parts: readonly Part[]): Part {
  const merged: Part = { positions: [], normals: [], glow: [] };
  for (const part of parts) {
    for (const value of part.positions) {
      merged.positions.push(value);
    }
    for (const value of part.normals) {
      merged.normals.push(value);
    }
    for (const value of part.glow) {
      merged.glow.push(value);
    }
  }
  return merged;
}

export function toNeonMesh(part: Part): NeonMesh {
  const vertexCount = part.positions.length / 3;
  const vertices = new Float32Array(vertexCount * NEON_VERTEX_FLOATS);
  for (let vertex = 0; vertex < vertexCount; vertex += 1) {
    const offset = vertex * NEON_VERTEX_FLOATS;
    vertices[offset] = part.positions[vertex * 3];
    vertices[offset + 1] = part.positions[(vertex * 3) + 1];
    vertices[offset + 2] = part.positions[(vertex * 3) + 2];
    vertices[offset + 3] = part.normals[vertex * 3];
    vertices[offset + 4] = part.normals[(vertex * 3) + 1];
    vertices[offset + 5] = part.normals[(vertex * 3) + 2];
    vertices[offset + 6] = part.glow[vertex];
  }
  return { vertices, vertexCount };
}

import type { Point } from "../types";
import {
  bar,
  bipyramid,
  box,
  circle,
  cone,
  cylinder,
  extrudeOutline,
  merge,
  octahedron,
  outlineTrim,
  place,
  scaleOutline,
  solid,
  sphere,
  torus,
  transform,
  translate,
  recomputeFlatNormals,
  type Part,
} from "./geometry-kit";
import { vec3, type Vec3 } from "./math";

/*
 * Procedural low-poly models. Monster models are built at unit radius and scaled per
 * instance by the monster's gameplay radius; tower models use field units (tower radius 12).
 * Local +X is "forward" (2D angle 0) and +Z maps to field +Y.
 */

const HALF_TURN = Math.PI;
const QUARTER_TURN = Math.PI / 2;

function glowTrim(outline: readonly Point[], y: number, thickness: number, height: number, glow: number): Part[] {
  return outlineTrim(outline, y, thickness, height).map((primitive) => solid(primitive, glow, null));
}

/** A glowing bar from `from` to `to`: a lit ridge line along a faceted body's edge. */
function glowRidge(from: Vec3, to: Vec3, thickness: number, glow: number): Part {
  const dx = to.x - from.x;
  const dy = to.y - from.y;
  const dz = to.z - from.z;
  const horizontal = Math.hypot(dx, dz);
  return solid(
    box(Math.hypot(horizontal, dy) + thickness, thickness, thickness),
    glow,
    // Rz lifts the bar's X axis to the ridge's slope, then Ry turns it to the ridge's heading.
    place(from.x + (dx / 2), from.y + (dy / 2), from.z + (dz / 2), 0, -Math.atan2(dz, dx), Math.atan2(dy, horizontal), 1, 1, 1),
  );
}

function cylinderAlongX(radius: number, length: number, segments: number, glow: number, startX: number, y: number): Part {
  return solid(
    cylinder(radius, radius, length, segments),
    glow,
    place(startX + (length / 2), y, 0, 0, 0, -QUARTER_TURN, 1, 1, 1),
  );
}

// ---------------------------------------------------------------- monsters

const PACKMAN_TRIM_SEGMENTS = 16;
const PACKMAN_TRIM_RADIUS = 1.02;
const PACKMAN_TRIM_THICKNESS = 0.13;

export function createPackManJaw(): Part {
  // The z <= 0 hemisphere (field "up" side) with its cut face capped; the second jaw is
  // this mesh flipped about X, which hides its eye underneath like the 2D single eye.
  const parts = [
    solid(sphere(1, 22, 14, HALF_TURN, HALF_TURN), 0, null),
    solid(circle(1, 22), 0.32, null),
    solid(sphere(0.17, 10, 8, 0, Math.PI * 2), 1, translate(0.18, 0.8, -0.44)),
  ];
  // The original's outline from above: the jaw's half of the equator, plus the mouth edge
  // from the center to the front (the back half would show between the jaws).
  for (let segment = 0; segment < PACKMAN_TRIM_SEGMENTS; segment += 1) {
    const start = (segment / PACKMAN_TRIM_SEGMENTS) * Math.PI;
    const end = ((segment + 1) / PACKMAN_TRIM_SEGMENTS) * Math.PI;
    const startX = Math.cos(start) * PACKMAN_TRIM_RADIUS;
    const startZ = -Math.sin(start) * PACKMAN_TRIM_RADIUS;
    const endX = Math.cos(end) * PACKMAN_TRIM_RADIUS;
    const endZ = -Math.sin(end) * PACKMAN_TRIM_RADIUS;
    const length = Math.hypot(endX - startX, endZ - startZ);
    parts.push(solid(bar((startX + endX) / 2, 0, (startZ + endZ) / 2, Math.atan2(endZ - startZ, endX - startX), length + PACKMAN_TRIM_THICKNESS, PACKMAN_TRIM_THICKNESS, PACKMAN_TRIM_THICKNESS), 1, null));
  }
  parts.push(solid(bar(PACKMAN_TRIM_RADIUS / 2, 0, 0, 0, PACKMAN_TRIM_RADIUS + PACKMAN_TRIM_THICKNESS, PACKMAN_TRIM_THICKNESS, PACKMAN_TRIM_THICKNESS), 1, null));
  return merge(parts);
}

export function createSquareBody(): Part {
  const parts = [solid(box(2, 2, 2), 0, null)];
  const edge = 2.12;
  const thickness = 0.16;
  for (const a of [-1, 1]) {
    for (const b of [-1, 1]) {
      parts.push(solid(box(edge, thickness, thickness), 1, translate(0, a, b)));
      parts.push(solid(box(thickness, edge, thickness), 1, translate(a, 0, b)));
      parts.push(solid(box(thickness, thickness, edge), 1, translate(a, b, 0)));
    }
  }
  return merge(parts);
}

const TRIANGLE_OUTLINE: Point[] = [{ x: 1, y: 0 }, { x: -1, y: -1 }, { x: -1, y: 1 }];
const TRIANGLE_TOP_APEX = vec3(-0.3, 0.62, 0);

export function createTriangleBody(): Part {
  return merge([
    solid(bipyramid(TRIANGLE_OUTLINE, TRIANGLE_TOP_APEX, vec3(-0.3, -0.34, 0)), 0, null),
    ...glowTrim(TRIANGLE_OUTLINE, 0, 0.11, 0.11, 1),
    // Glowing ridges from the peak to each corner, so the pyramid reads from straight above.
    ...TRIANGLE_OUTLINE.map((corner) => glowRidge(TRIANGLE_TOP_APEX, vec3(corner.x, 0, corner.y), 0.07, 1)),
    solid(sphere(0.13, 8, 6, 0, Math.PI * 2), 1, translate(0.25, 0.28, 0)),
  ]);
}

const TANK_HULL_TOP = 0.82;

export function createTankHull(): Part {
  const hullOutline: Point[] = [
    { x: -1, y: -0.72 },
    { x: 1.1, y: -0.72 },
    { x: 1.1, y: 0.72 },
    { x: -1, y: 0.72 },
  ];
  const parts = [
    solid(box(2.1, 0.5, 1.44), 0, translate(0.05, 0.57, 0)),
    solid(box(1.7, 0.1, 1.1), 0.08, translate(0, TANK_HULL_TOP - 0.02, 0)),
    ...glowTrim(hullOutline, TANK_HULL_TOP - 0.03, 0.07, 0.07, 1),
  ];
  for (const side of [-1, 1]) {
    parts.push(solid(box(2.26, 0.5, 0.34), 0.04, translate(0.05, 0.26, side * 0.86)));
    parts.push(solid(box(2.2, 0.06, 0.06), 1, translate(0.05, 0.53, side * 1.03)));
    for (let tread = 0; tread < 7; tread += 1) {
      parts.push(solid(box(0.1, 0.52, 0.36), 0.12, translate(-0.95 + (tread * 0.33), 0.26, side * 0.86)));
    }
  }
  return merge(parts);
}

/** Turret mesh with its origin at the turret pivot; also used by the flying-turret debris. */
export function createTankTurret(): Part {
  return merge([
    solid(cylinder(0.44, 0.5, 0.34, 16), 0, translate(0, 0.17, 0)),
    solid(torus(0.46, 0.035, 4, 24), 1, place(0, 0.33, 0, QUARTER_TURN, 0, 0, 1, 1, 1)),
    cylinderAlongX(0.085, 1.2, 8, 0, 0.4, 0.2),
    cylinderAlongX(0.12, 0.16, 8, 1, 1.5, 0.2),
  ]);
}

const RUNNER_OUTLINE: Point[] = [
  { x: 1.8, y: 0 },
  { x: 0.28, y: -0.86 },
  { x: -1.35, y: -0.58 },
  { x: -0.92, y: 0 },
  { x: -1.35, y: 0.58 },
  { x: 0.28, y: 0.86 },
];

export function createRunnerBody(): Part {
  return merge([
    solid(extrudeOutline(RUNNER_OUTLINE, 0.32, 0.1), 0, null),
    ...glowTrim(scaleOutline(RUNNER_OUTLINE, 1.05), 0.26, 0.09, 0.09, 1),
    solid(sphere(1, 10, 8, 0, Math.PI * 2), 1, place(0.35, 0.5, 0, 0, 0, 0, 0.55, 0.2, 0.28)),
  ]);
}

const SPLITTER_OUTLINE: Point[] = Array.from({ length: 6 }, (_, index) => {
  const angle = (Math.PI / 3) * index;
  const radius = index % 2 === 0 ? 1.15 : 0.72;
  return { x: Math.cos(angle) * radius, y: Math.sin(angle) * radius };
});

export function createSplitterBody(): Part {
  return merge([
    solid(bipyramid(SPLITTER_OUTLINE, vec3(0, 0.66, 0), vec3(0, -0.42, 0)), 0, null),
    ...glowTrim(SPLITTER_OUTLINE, 0, 0.1, 0.1, 1),
    solid(bar(-0.28, 0.42, 0.02, Math.atan2(0.35, 0.55), 0.68, 0.07, 0.08), 1, null),
    solid(bar(0.29, 0.42, 0.01, Math.atan2(-0.38, 0.58), 0.7, 0.07, 0.08), 1, null),
  ]);
}

const BERSERKER_OUTLINE: Point[] = [
  { x: 1.55, y: 0 },
  { x: 0.4, y: -0.8 },
  { x: -0.1, y: -1.08 },
  { x: -1.28, y: -0.44 },
  { x: -0.72, y: 0 },
  { x: -1.28, y: 0.44 },
  { x: -0.1, y: 1.08 },
  { x: 0.4, y: 0.8 },
];

export function createBerserkerBody(): Part {
  const parts = [
    solid(extrudeOutline(BERSERKER_OUTLINE, 0.42, 0.12), 0, null),
    ...glowTrim(scaleOutline(BERSERKER_OUTLINE, 1.08), 0.33, 0.09, 0.09, 1),
  ];
  for (const side of [-1, 1]) {
    parts.push(solid(cone(0.16, 0.9, 6), 0.25, place(0.75, 0.55, side * 0.72, 0, side * 0.5, -QUARTER_TURN, 1, 1, 1)));
    parts.push(solid(box(0.28, 0.08, 0.1), 1, place(0.98, 0.67, side * 0.2, 0, side * -0.45, 0, 1, 1, 1)));
  }
  return merge(parts);
}

export function createBerserkerSpikes(): Part {
  return merge([-0.42, 0, 0.42].map((z) =>
    solid(cone(0.13, 0.62, 5), 1, place(-0.6, 0.72, z, 0, 0, 0.9, 1, 1, 1)),
  ));
}

const BULWARK_SHELL_OUTLINE: Point[] = [
  { x: 1.35, y: 0 },
  { x: 0.82, y: -0.8 },
  { x: -0.2, y: -0.98 },
  { x: -1.08, y: -0.8 },
  { x: -1.32, y: 0 },
  { x: -1.08, y: 0.8 },
  { x: -0.2, y: 0.98 },
  { x: 0.82, y: 0.8 },
];

const BULWARK_CORE_OUTLINE: Point[] = [
  { x: 0.98, y: 0 },
  { x: 0.42, y: -0.46 },
  { x: -0.3, y: -0.46 },
  { x: -0.72, y: 0 },
  { x: -0.3, y: 0.46 },
  { x: 0.42, y: 0.46 },
];

const BULWARK_FRONT_PLATE_OUTLINE: Point[] = [
  { x: 1.08, y: 0 },
  { x: 0.76, y: -0.28 },
  { x: 0.16, y: -0.28 },
  { x: 0.16, y: 0.28 },
  { x: 0.76, y: 0.28 },
];

const BULWARK_UPPER_TIER_Y = 0.62;

export function createBulwarkShell(): Part {
  const upperTier = transform(extrudeOutline(scaleOutline(BULWARK_SHELL_OUTLINE, 0.8), 0.16, 0.06), translate(0, BULWARK_UPPER_TIER_Y - 0.02, 0));
  const frontPlate = transform(extrudeOutline(BULWARK_FRONT_PLATE_OUTLINE, 0.08, 0.03), translate(0, BULWARK_UPPER_TIER_Y + 0.2, 0));
  return merge([
    solid(extrudeOutline(BULWARK_SHELL_OUTLINE, 0.34, 0.14), 0, null),
    solid(upperTier, 0, null),
    solid(frontPlate, 0.5, null),
    ...glowTrim(scaleOutline(BULWARK_SHELL_OUTLINE, 1.09), 0.3, 0.07, 0.1, 0.9),
  ]);
}

/** Pulsing armor core and the glowing armor seams, tinted separately from the shell. */
export function createBulwarkCore(): Part {
  const core = transform(extrudeOutline(BULWARK_CORE_OUTLINE, 0.05, 0.02), translate(0, BULWARK_UPPER_TIER_Y + 0.2, 0));
  return merge([
    solid(core, 1, null),
    solid(bar(0.13, BULWARK_UPPER_TIER_Y + 0.22, -0.52, Math.atan2(0.6, 0.7), 0.9, 0.05, 0.06), 1, null),
    solid(bar(0.13, BULWARK_UPPER_TIER_Y + 0.22, 0.52, Math.atan2(-0.6, 0.7), 0.9, 0.05, 0.06), 1, null),
  ]);
}

/** Irregular crystalline chunk used for every death shard. */
export function createShard(): Part {
  const shard = octahedron(1);
  const jitter = [0.82, 1.18, 0.9, 1.05, 0.7, 1.12];
  const p = shard.positions;
  for (let offset = 0; offset < p.length; offset += 3) {
    const x = p[offset];
    const y = p[offset + 1];
    const z = p[offset + 2];
    const axis = Math.abs(x) > 0.5 ? (x > 0 ? 0 : 1) : Math.abs(y) > 0.5 ? (y > 0 ? 2 : 3) : (z > 0 ? 4 : 5);
    const scale = jitter[axis];
    p[offset] = x * scale * 1.1;
    p[offset + 1] = y * scale * 0.55;
    p[offset + 2] = z * scale;
  }
  const faceted = solid(recomputeFlatNormals(shard), 0.3, null);
  for (let vertex = 0; vertex < faceted.glow.length; vertex += 1) {
    faceted.glow[vertex] = Math.floor(vertex / 3) % 2 === 0 ? 0.55 : 0.12;
  }
  return faceted;
}

// ---------------------------------------------------------------- towers

export const TOWER_BASE_TOP = 5;

/** The dark plinth every tower stands on (tinted by the tower's accent). */
export function createTowerBase(): Part {
  return merge([
    solid(cylinder(11.2, 12.6, 4.2, 28), 0, translate(0, 2.1, 0)),
    solid(cylinder(10.2, 11.2, 0.8, 28), 0, translate(0, 4.6, 0)),
  ]);
}

/** The glowing rim on the plinth's top edge: the original's white base stroke. */
export function createTowerRim(): Part {
  return solid(torus(11.35, 0.68, 6, 48), 1, place(0, 4.25, 0, QUARTER_TURN, 0, 0, 1, 1, 1));
}

/** Unit-radius ring on the ground; instances scale it to each level's upgrade halo. */
export function createUpgradeRing(): Part {
  return solid(torus(1, 0.035, 4, 56), 1, place(0, 0, 0, QUARTER_TURN, 0, 0, 1, 1, 1));
}

export function createLevelPip(): Part {
  return solid(box(1.6, 1.1, 1.3), 1, null);
}

export function createGunHead(): Part {
  return merge([
    solid(cylinder(6.1, 7, 4.2, 6), 0, place(0, TOWER_BASE_TOP + 2.1, 0, 0, HALF_TURN / 6, 0, 1, 1, 1)),
    solid(torus(6.35, 0.32, 4, 6), 1, place(0, TOWER_BASE_TOP + 3.3, 0, QUARTER_TURN, 0, HALF_TURN / 6, 1, 1, 1)),
    solid(box(3.2, 2.6, 5.4), 0.05, translate(-5.2, TOWER_BASE_TOP + 2.7, 0)),
    solid(box(0.5, 0.5, 4.2), 1, translate(-6.9, TOWER_BASE_TOP + 3.2, 0)),
  ]);
}

export const GUN_BARREL_Y = TOWER_BASE_TOP + 3.1;

/** Unit-length barrel along +X; instances scale X by length and Y/Z by radius. */
export function createGunBarrel(): Part {
  return merge([
    // Glows like the original's thick white barrel stroke.
    cylinderAlongX(1, 1, 10, 0.6, 0, 0),
    solid(box(1, 0.35, 0.3), 1, translate(0.5, 1.02, 0)),
  ]);
}

export function createGunMuzzle(): Part {
  return cylinderAlongX(1.35, 1.8, 10, 1, -0.9, 0);
}

/** Unit-length glowing rail along +X. */
export function createRail(): Part {
  return solid(box(1, 0.7, 0.7), 1, translate(0.5, 0, 0));
}

export const LASER_CRYSTAL_Y = TOWER_BASE_TOP + 4.4;

/** Unit diamond; instances scale it to each level's crystal length and girth. */
export function createLaserCrystal(): Part {
  return solid(octahedron(1), 0.62, null);
}

export function createLaserCradle(): Part {
  const parts = [
    solid(cylinder(3.4, 4, 2.2, 16), 0, translate(0, TOWER_BASE_TOP + 1.1, 0)),
  ];
  for (const side of [-1, 1]) {
    parts.push(solid(box(7, 3.4, 1.3), 0, translate(-1, TOWER_BASE_TOP + 3.2, side * 4.4)));
    parts.push(solid(box(6.4, 0.4, 0.4), 1, translate(-1, TOWER_BASE_TOP + 5, side * 4.4)));
  }
  return merge(parts);
}

export const MISSILE_RACK_Y = TOWER_BASE_TOP + 2.6;

export function createMissileLauncher(): Part {
  const parts = [
    solid(box(19, 1.4, 9.4), 0, translate(0.5, TOWER_BASE_TOP + 0.9, 0)),
    solid(box(2.6, 4.4, 9.4), 0.04, translate(-10.3, TOWER_BASE_TOP + 2.4, 0)),
    solid(box(0.4, 3.6, 7.4), 1, translate(-8.9, TOWER_BASE_TOP + 2.6, 0)),
  ];
  for (const side of [-1, 1]) {
    parts.push(solid(box(19, 3.4, 1.2), 0, translate(0.5, TOWER_BASE_TOP + 2.8, side * 4.1)));
    parts.push(solid(box(18, 0.36, 0.5), 1, translate(0.8, TOWER_BASE_TOP + 4.6, side * 4.1)));
  }
  return merge(parts);
}

/** Missile centered near its midpoint, nose along +X. */
export function createMissile(): Part {
  const parts = [
    cylinderAlongX(1.65, 13.4, 10, 0.1, -7.4, 0),
    solid(cone(1.9, 5.2, 10), 1, place(8.6, 0, 0, 0, 0, -QUARTER_TURN, 1, 1, 1)),
    cylinderAlongX(1.35, 2.2, 10, 0.55, -9.6, 0),
  ];
  for (let fin = 0; fin < 4; fin += 1) {
    const angle = (fin * QUARTER_TURN) + (QUARTER_TURN / 2);
    parts.push(solid(box(3.2, 0.3, 2.6), 0, place(-6.4, Math.sin(angle) * 1.9, Math.cos(angle) * 1.9, angle, 0, 0, 1, 1, 1)));
  }
  return merge(parts);
}

export const SLOW_CORE_Y = TOWER_BASE_TOP + 7;

export function createSlowCore(): Part {
  return merge([
    solid(cylinder(1.6, 3, 6, 12), 0, translate(0, TOWER_BASE_TOP + 3 - SLOW_CORE_Y, 0)),
    solid(sphere(4, 20, 14, 0, Math.PI * 2), 1, null),
    solid(torus(6.2, 0.45, 6, 36), 0.75, place(0, 0, 0, QUARTER_TURN + 0.5, 0, 0, 1, 1, 1)),
    solid(torus(5.4, 0.3, 6, 36), 0.55, place(0, 0, 0, QUARTER_TURN - 0.7, 0, 0.4, 1, 1, 1)),
  ]);
}

export function createOrbNode(): Part {
  return solid(sphere(1, 10, 8, 0, Math.PI * 2), 1, null);
}

export const DRONE_PAD_TOP = TOWER_BASE_TOP + 1.5;

export function createDronePad(): Part {
  return merge([
    solid(cylinder(9.4, 10.2, 1.5, 6), 0, place(0, TOWER_BASE_TOP + 0.75, 0, 0, HALF_TURN / 6, 0, 1, 1, 1)),
    solid(torus(8.4, 0.36, 4, 6), 1, place(0, DRONE_PAD_TOP, 0, QUARTER_TURN, 0, HALF_TURN / 6, 1, 1, 1)),
    solid(box(0.7, 0.2, 7), 0.8, translate(-2.6, DRONE_PAD_TOP, 0)),
    solid(box(0.7, 0.2, 7), 0.8, translate(2.6, DRONE_PAD_TOP, 0)),
    solid(box(5.2, 0.2, 0.7), 0.8, translate(0, DRONE_PAD_TOP, 0)),
  ]);
}

/** The coil (not the plinth) is scaled up from its base on the plinth top. */
export const TESLA_COIL_SCALE = 1.2;
export const TESLA_TOP_Y = TOWER_BASE_TOP + (17.5 * TESLA_COIL_SCALE);

export function createTeslaCoil(): Part {
  const s = TESLA_COIL_SCALE;
  const ring = (radius: number, tube: number, height: number): Part => (
    solid(torus(radius * s, tube * s, 6, 24), 1, place(0, TOWER_BASE_TOP + (height * s), 0, QUARTER_TURN, 0, 0, 1, 1, 1))
  );
  return merge([
    solid(cylinder(1.5 * s, 2.6 * s, 15 * s, 12), 0, translate(0, TOWER_BASE_TOP + (7.5 * s), 0)),
    ring(4.6, 0.55, 3.5),
    ring(3.9, 0.5, 7.3),
    ring(3.2, 0.45, 11),
    solid(sphere(3.3 * s, 16, 12, 0, Math.PI * 2), 1, translate(0, TESLA_TOP_Y, 0)),
  ]);
}

// ---------------------------------------------------------------- flyers & board

export function createDroneBody(): Part {
  const parts = [
    solid(box(7.8, 2.2, 7.8), 0, null),
    solid(box(3.8, 0.5, 1.8), 1, translate(0, 1.2, 0)),
    solid(bar(0, 0, 0, Math.PI / 4, 19.5, 1, 1.2), 0, null),
    solid(bar(0, 0, 0, -Math.PI / 4, 19.5, 1, 1.2), 0, null),
  ];
  for (const x of [-6.9, 6.9]) {
    for (const z of [-6.9, 6.9]) {
      parts.push(solid(cylinder(1.5, 1.5, 1.8, 10), 0.8, translate(x, 0.2, z)));
    }
  }
  return merge(parts);
}

export function createPortal(): Part {
  const parts = [
    solid(cylinder(0.96, 1, 0.035, 40), 0.05, translate(0, 0.02, 0)),
    solid(torus(1, 0.06, 6, 48), 1, place(0, 0.05, 0, QUARTER_TURN, 0, 0, 1, 1, 1)),
    solid(torus(0.7, 0.028, 4, 40), 0.8, place(0, 0.05, 0, QUARTER_TURN, 0, 0, 1, 1, 1)),
  ];
  for (let pylon = 0; pylon < 6; pylon += 1) {
    const angle = (pylon / 6) * Math.PI * 2;
    parts.push(solid(box(0.09, 0.24, 0.16), 0.5, place(Math.cos(angle) * 1.1, 0.12, Math.sin(angle) * 1.1, 0, -angle, 0, 1, 1, 1)));
  }
  return merge(parts);
}

export function createSpawnGate(roadWidth: number): Part {
  const halfSpan = (roadWidth / 2) + 3.2;
  return merge([
    solid(box(2.6, 15, 2.6), 0.05, translate(0, 7.5, -halfSpan)),
    solid(box(2.6, 15, 2.6), 0.05, translate(0, 7.5, halfSpan)),
    solid(box(2.4, 1.8, (halfSpan * 2) + 2.6), 0.1, translate(0, 15.6, 0)),
    solid(box(0.6, 0.6, halfSpan * 2), 1, translate(0, 14.4, 0)),
    solid(box(0.6, 14, 0.6), 1, translate(0, 7, -halfSpan + 1.4)),
    solid(box(0.6, 14, 0.6), 1, translate(0, 7, halfSpan - 1.4)),
  ]);
}

// ---------------------------------------------------------------- effect geometry

/** Flat quad vertices for the effect pipelines: interleaved `position(3) uv(2)`. */
export interface FlatMesh {
  readonly vertices: Float32Array;
  readonly vertexCount: number;
}

/**
 * A flat quad on the ground plane (XZ) spanning [minX, maxX] x [-halfDepth, halfDepth];
 * uv.x runs along +X and uv.y from +Z (0) to -Z (1).
 */
function flatQuad(minX: number, maxX: number, halfDepth: number): FlatMesh {
  const corners = [
    [minX, halfDepth, 0, 0],
    [maxX, halfDepth, 1, 0],
    [maxX, -halfDepth, 1, 1],
    [minX, -halfDepth, 0, 1],
  ];
  const vertices: number[] = [];
  for (const index of [0, 1, 2, 0, 2, 3]) {
    const [x, z, u, v] = corners[index];
    vertices.push(x, 0, z, u, v);
  }
  return { vertices: new Float32Array(vertices), vertexCount: 6 };
}

/** Flat unit quad on the ground plane, centered. */
export function createFlatQuad(): FlatMesh {
  return flatQuad(-0.5, 0.5, 0.5);
}

/** Flat unit ribbon spanning x = 0..1 with uv.y across its width. */
export function createRibbonQuad(): FlatMesh {
  return flatQuad(0, 1, 0.5);
}

/** Flat disc quad of radius 1 whose uv spans the full square (shaders draw the circle). */
export function createRangeQuad(): FlatMesh {
  return flatQuad(-1, 1, 1);
}

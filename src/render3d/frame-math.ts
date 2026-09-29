import type { Vec3 } from "./math";

/** Per-frame values shared by every 3D view. */
export interface FrameContext {
  /** Presentation seconds since the previous frame; 0 while the simulation is frozen. */
  readonly deltaSeconds: number;
  /** Presentation clock (simulated seconds). */
  readonly time: number;
  readonly frame: number;
  /** Normalized world-space camera forward vector, for camera-facing ribbons. */
  readonly viewDirection: Vec3;
}

/** Minimal mutable quaternion used to compose instance rotations without allocations. */
export class Quat {
  x = 0;
  y = 0;
  z = 0;
  w = 1;

  set(x: number, y: number, z: number, w: number): this {
    this.x = x;
    this.y = y;
    this.z = z;
    this.w = w;
    return this;
  }

  /** Rotation about world up; pass `-fieldAngle` to face a 2D heading. */
  setYaw(angle: number): this {
    const half = angle / 2;
    return this.set(0, Math.sin(half), 0, Math.cos(half));
  }

  setAxisAngle(axisX: number, axisY: number, axisZ: number, angle: number): this {
    const half = angle / 2;
    const s = Math.sin(half);
    return this.set(axisX * s, axisY * s, axisZ * s, Math.cos(half));
  }

  /** this = this * other */
  multiply(other: Quat): this {
    return this.multiplyComponents(other.x, other.y, other.z, other.w);
  }

  multiplyComponents(bx: number, by: number, bz: number, bw: number): this {
    const { x: ax, y: ay, z: az, w: aw } = this;
    return this.set(
      (aw * bx) + (ax * bw) + (ay * bz) - (az * by),
      (aw * by) - (ax * bz) + (ay * bw) + (az * bx),
      (aw * bz) + (ax * by) - (ay * bx) + (az * bw),
      (aw * bw) - (ax * bx) - (ay * by) - (az * bz),
    );
  }

  /** this = this * rotationAboutLocalZ(angle); pitches a +X-forward model nose-up. */
  pitch(angle: number): this {
    const half = angle / 2;
    return this.multiplyComponents(0, 0, Math.sin(half), Math.cos(half));
  }

  /** this = this * rotationAboutLocalX(angle); banks a +X-forward model. */
  roll(angle: number): this {
    const half = angle / 2;
    return this.multiplyComponents(Math.sin(half), 0, 0, Math.cos(half));
  }

  /** Heading from a 2D field angle, then pitch and roll in the model's local frame. */
  setHeadingPitchRoll(fieldAngle: number, pitchAngle: number, rollAngle: number): this {
    return this.setYaw(-fieldAngle).pitch(pitchAngle).roll(rollAngle);
  }
}

export function smoothTowards(current: number, target: number, ratePerSecond: number, deltaSeconds: number): number {
  return target + ((current - target) * Math.exp(-ratePerSecond * deltaSeconds));
}

/** Cheap deterministic 0..1 hash for per-entity animation phase offsets. */
export function hash01(seed: number): number {
  const value = Math.sin(seed * 12.9898) * 43758.5453;
  return value - Math.floor(value);
}

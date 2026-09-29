import { INSTANCE_FLOATS, type ScenePipelines } from "./gpu-pipelines";
import { BufferUsage } from "./gpu-flags";

/** Per-instance layout: column-major transform (16), linear RGB tint + pad (4), extras (4). */
export const INSTANCE_STRIDE = INSTANCE_FLOATS;
const TINT_OFFSET = 16;
const EXTRA_OFFSET = 20;
const FLOAT_BYTES = 4;

/** Static vertex data a batch draws per instance. */
export interface BatchGeometry {
  readonly buffer: GPUBuffer;
  readonly vertexCount: number;
}

export function createGeometryBuffer(device: GPUDevice, label: string, vertices: Float32Array, vertexCount: number): BatchGeometry {
  const buffer = device.createBuffer({ label, size: vertices.byteLength, usage: BufferUsage.VERTEX | BufferUsage.COPY_DST });
  device.queue.writeBuffer(buffer, 0, vertices);
  return { buffer, vertexCount };
}

/**
 * A fixed-capacity instanced draw refilled every frame. Instances are written straight
 * into one typed array and uploaded with a single write of the used range, so
 * steady-state drawing allocates nothing.
 */
export class InstancedBatch {
  readonly capacity: number;
  private readonly data: Float32Array;
  private readonly instances: GPUBuffer;
  private count = 0;
  private uploaded = 0;

  /**
   * `shaderMode` is written to every instance's tint w: the neon or effect module's look
   * selector (see `NeonMode` and `EffectMode`).
   */
  constructor(
    private readonly device: GPUDevice,
    readonly name: string,
    private readonly geometry: BatchGeometry,
    readonly pipeline: keyof ScenePipelines,
    private readonly shaderMode: number,
    capacity: number,
  ) {
    this.capacity = capacity;
    this.data = new Float32Array(capacity * INSTANCE_STRIDE);
    this.instances = device.createBuffer({
      label: name,
      size: this.data.byteLength,
      usage: BufferUsage.VERTEX | BufferUsage.COPY_DST,
    });
  }

  get size(): number {
    return this.count;
  }

  begin(): void {
    this.count = 0;
  }

  /** Translation, rotation about world up (+Y), and axis scale. */
  pushYaw(
    x: number,
    y: number,
    z: number,
    yaw: number,
    scaleX: number,
    scaleY: number,
    scaleZ: number,
    red: number,
    green: number,
    blue: number,
  ): number {
    const index = this.count;
    if (index >= this.capacity) {
      return -1;
    }

    const cos = Math.cos(yaw);
    const sin = Math.sin(yaw);
    const m = this.data;
    const o = index * INSTANCE_STRIDE;
    m[o] = cos * scaleX;
    m[o + 1] = 0;
    m[o + 2] = -sin * scaleX;
    m[o + 3] = 0;
    m[o + 4] = 0;
    m[o + 5] = scaleY;
    m[o + 6] = 0;
    m[o + 7] = 0;
    m[o + 8] = sin * scaleZ;
    m[o + 9] = 0;
    m[o + 10] = cos * scaleZ;
    m[o + 11] = 0;
    m[o + 12] = x;
    m[o + 13] = y;
    m[o + 14] = z;
    m[o + 15] = 1;
    this.finishInstance(o, red, green, blue);
    this.count = index + 1;
    return index;
  }

  /** Translation, unit-quaternion rotation, and axis scale. */
  pushQuaternion(
    x: number,
    y: number,
    z: number,
    qx: number,
    qy: number,
    qz: number,
    qw: number,
    scaleX: number,
    scaleY: number,
    scaleZ: number,
    red: number,
    green: number,
    blue: number,
  ): number {
    const index = this.count;
    if (index >= this.capacity) {
      return -1;
    }

    const x2 = qx + qx;
    const y2 = qy + qy;
    const z2 = qz + qz;
    const xx = qx * x2;
    const xy = qx * y2;
    const xz = qx * z2;
    const yy = qy * y2;
    const yz = qy * z2;
    const zz = qz * z2;
    const wx = qw * x2;
    const wy = qw * y2;
    const wz = qw * z2;
    const m = this.data;
    const o = index * INSTANCE_STRIDE;
    m[o] = (1 - (yy + zz)) * scaleX;
    m[o + 1] = (xy + wz) * scaleX;
    m[o + 2] = (xz - wy) * scaleX;
    m[o + 3] = 0;
    m[o + 4] = (xy - wz) * scaleY;
    m[o + 5] = (1 - (xx + zz)) * scaleY;
    m[o + 6] = (yz + wx) * scaleY;
    m[o + 7] = 0;
    m[o + 8] = (xz + wy) * scaleZ;
    m[o + 9] = (yz - wx) * scaleZ;
    m[o + 10] = (1 - (xx + yy)) * scaleZ;
    m[o + 11] = 0;
    m[o + 12] = x;
    m[o + 13] = y;
    m[o + 14] = z;
    m[o + 15] = 1;
    this.finishInstance(o, red, green, blue);
    this.count = index + 1;
    return index;
  }

  /**
   * Arbitrary orthogonal basis: columns are the world-space images of the local X, Y,
   * and Z axes (lengths are the axis scales).
   */
  pushBasis(
    x: number,
    y: number,
    z: number,
    axisXx: number,
    axisXy: number,
    axisXz: number,
    axisYx: number,
    axisYy: number,
    axisYz: number,
    axisZx: number,
    axisZy: number,
    axisZz: number,
    red: number,
    green: number,
    blue: number,
  ): number {
    const index = this.count;
    if (index >= this.capacity) {
      return -1;
    }

    const m = this.data;
    const o = index * INSTANCE_STRIDE;
    m[o] = axisXx;
    m[o + 1] = axisXy;
    m[o + 2] = axisXz;
    m[o + 3] = 0;
    m[o + 4] = axisYx;
    m[o + 5] = axisYy;
    m[o + 6] = axisYz;
    m[o + 7] = 0;
    m[o + 8] = axisZx;
    m[o + 9] = axisZy;
    m[o + 10] = axisZz;
    m[o + 11] = 0;
    m[o + 12] = x;
    m[o + 13] = y;
    m[o + 14] = z;
    m[o + 15] = 1;
    this.finishInstance(o, red, green, blue);
    this.count = index + 1;
    return index;
  }

  setExtra(index: number, component: number, value: number): void {
    if (index >= 0) {
      this.data[(index * INSTANCE_STRIDE) + EXTRA_OFFSET + component] = value;
    }
  }

  finish(): void {
    this.uploaded = this.count;
    if (this.count > 0) {
      this.device.queue.writeBuffer(this.instances, 0, this.data.buffer, 0, this.count * INSTANCE_STRIDE * FLOAT_BYTES);
    }
  }

  /** Records the draw; the caller has already bound this batch's pipeline. */
  draw(pass: GPURenderPassEncoder): void {
    if (this.uploaded === 0) {
      return;
    }
    pass.setVertexBuffer(0, this.geometry.buffer);
    pass.setVertexBuffer(1, this.instances, 0, this.uploaded * INSTANCE_STRIDE * FLOAT_BYTES);
    pass.draw(this.geometry.vertexCount, this.uploaded);
  }

  dispose(): void {
    this.instances.destroy();
  }

  private finishInstance(offset: number, red: number, green: number, blue: number): void {
    const m = this.data;
    m[offset + TINT_OFFSET] = red;
    m[offset + TINT_OFFSET + 1] = green;
    m[offset + TINT_OFFSET + 2] = blue;
    m[offset + TINT_OFFSET + 3] = this.shaderMode;
    m[offset + EXTRA_OFFSET] = 0;
    m[offset + EXTRA_OFFSET + 1] = 0;
    m[offset + EXTRA_OFFSET + 2] = 0;
    m[offset + EXTRA_OFFSET + 3] = 0;
  }
}

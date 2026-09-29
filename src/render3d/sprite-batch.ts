import { SPRITE_FLOATS, type ScenePipelines } from "./gpu-pipelines";
import { BufferUsage } from "./gpu-flags";

const FLOAT_BYTES = 4;
const QUAD_VERTICES = 6;

/**
 * Camera-facing quads drawn as one instanced draw call. Each sprite is
 * `x, y, z, rotation | width, height, shape, - | r, g, b, a` (linear color).
 */
export class SpriteBatch {
  readonly capacity: number;
  private readonly data: Float32Array;
  private readonly instances: GPUBuffer;
  private count = 0;
  private uploaded = 0;

  /** `shaderMode` is written to every sprite's shape w: the sprite module's look selector (see `SpriteMode`). */
  constructor(
    private readonly device: GPUDevice,
    readonly name: string,
    capacity: number,
    readonly pipeline: keyof ScenePipelines,
    private readonly shaderMode: number,
  ) {
    this.capacity = capacity;
    this.data = new Float32Array(capacity * SPRITE_FLOATS);
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

  push(
    x: number,
    y: number,
    z: number,
    rotation: number,
    width: number,
    height: number,
    shape: number,
    red: number,
    green: number,
    blue: number,
    alpha: number,
  ): void {
    const index = this.count;
    if (index >= this.capacity) {
      return;
    }

    const d = this.data;
    const o = index * SPRITE_FLOATS;
    d[o] = x;
    d[o + 1] = y;
    d[o + 2] = z;
    d[o + 3] = rotation;
    d[o + 4] = width;
    d[o + 5] = height;
    d[o + 6] = shape;
    d[o + 7] = this.shaderMode;
    d[o + 8] = red;
    d[o + 9] = green;
    d[o + 10] = blue;
    d[o + 11] = alpha;
    this.count = index + 1;
  }

  finish(): void {
    this.uploaded = this.count;
    if (this.count > 0) {
      this.device.queue.writeBuffer(this.instances, 0, this.data.buffer, 0, this.count * SPRITE_FLOATS * FLOAT_BYTES);
    }
  }

  draw(pass: GPURenderPassEncoder): void {
    if (this.uploaded === 0) {
      return;
    }
    pass.setVertexBuffer(0, this.instances, 0, this.uploaded * SPRITE_FLOATS * FLOAT_BYTES);
    pass.draw(QUAD_VERTICES, this.uploaded);
  }

  dispose(): void {
    this.instances.destroy();
  }
}

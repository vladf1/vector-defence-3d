import { randomRange } from "../utils";
import type { CameraRig } from "./camera-rig";
import { linearColor, type LinearColor } from "./palette";
import type { RenderBatches } from "./render-batches";

const FxKind = {
  Spark: 0,
  Ember: 1,
  Fireball: 2,
  Smoke: 3,
  Ring: 4,
  Flash: 5,
  Pillar: 6,
  Halo: 7,
} as const;

type FxKind = typeof FxKind[keyof typeof FxKind];

const GROUND_BOUNCE_RESTITUTION = 0.38;
const GROUND_FRICTION = 0.55;
const FIREBALL_HOT = linearColor("#fff4d0");
const FIREBALL_WARM = linearColor("#ffb04a");
const FIREBALL_COOL = linearColor("#d8452a");
const SMOKE_COLOR = linearColor("#2a2f2c");
const EXPLOSION_LIGHT = linearColor("#ffae5c");
const ESCAPE_COLOR = linearColor("#b0ffe1");
const ESCAPE_ACCENT = linearColor("#ffe36f");
const ZAP_COLOR = linearColor("#9fe8ff");

/** Struct-of-arrays particle pool; dead particles are swapped out so updates stay dense. */
class FxParticlePool {
  private count = 0;
  private readonly kind: Uint8Array;
  private readonly x: Float32Array;
  private readonly y: Float32Array;
  private readonly z: Float32Array;
  private readonly vx: Float32Array;
  private readonly vy: Float32Array;
  private readonly vz: Float32Array;
  private readonly age: Float32Array;
  private readonly life: Float32Array;
  private readonly startSize: Float32Array;
  private readonly endSize: Float32Array;
  private readonly gravity: Float32Array;
  private readonly drag: Float32Array;
  private readonly red: Float32Array;
  private readonly green: Float32Array;
  private readonly blue: Float32Array;
  private readonly spin: Float32Array;

  constructor(private readonly capacity: number) {
    this.kind = new Uint8Array(capacity);
    this.x = new Float32Array(capacity);
    this.y = new Float32Array(capacity);
    this.z = new Float32Array(capacity);
    this.vx = new Float32Array(capacity);
    this.vy = new Float32Array(capacity);
    this.vz = new Float32Array(capacity);
    this.age = new Float32Array(capacity);
    this.life = new Float32Array(capacity);
    this.startSize = new Float32Array(capacity);
    this.endSize = new Float32Array(capacity);
    this.gravity = new Float32Array(capacity);
    this.drag = new Float32Array(capacity);
    this.red = new Float32Array(capacity);
    this.green = new Float32Array(capacity);
    this.blue = new Float32Array(capacity);
    this.spin = new Float32Array(capacity);
  }

  get size(): number {
    return this.count;
  }

  spawn(
    kind: FxKind,
    x: number,
    y: number,
    z: number,
    vx: number,
    vy: number,
    vz: number,
    life: number,
    startSize: number,
    endSize: number,
    color: LinearColor,
    gravity: number,
    drag: number,
  ): void {
    if (this.count >= this.capacity) {
      return;
    }
    const i = this.count;
    this.kind[i] = kind;
    this.x[i] = x;
    this.y[i] = y;
    this.z[i] = z;
    this.vx[i] = vx;
    this.vy[i] = vy;
    this.vz[i] = vz;
    this.age[i] = 0;
    this.life[i] = life;
    this.startSize[i] = startSize;
    this.endSize[i] = endSize;
    this.gravity[i] = gravity;
    this.drag[i] = drag;
    this.red[i] = color.r;
    this.green[i] = color.g;
    this.blue[i] = color.b;
    this.spin[i] = randomRange(-2, 2);
    this.count = i + 1;
  }

  clear(): void {
    this.count = 0;
  }

  update(deltaSeconds: number): void {
    if (deltaSeconds <= 0) {
      return;
    }
    let index = 0;
    while (index < this.count) {
      this.age[index] += deltaSeconds;
      if (this.age[index] >= this.life[index]) {
        this.swapRemove(index);
        continue;
      }
      const damping = Math.exp(-this.drag[index] * deltaSeconds);
      this.vx[index] *= damping;
      this.vz[index] *= damping;
      this.vy[index] = (this.vy[index] * damping) - (this.gravity[index] * deltaSeconds);
      this.x[index] += this.vx[index] * deltaSeconds;
      this.y[index] += this.vy[index] * deltaSeconds;
      this.z[index] += this.vz[index] * deltaSeconds;
      if (this.gravity[index] > 0 && this.y[index] < 0.4) {
        this.y[index] = 0.4;
        if (this.vy[index] < 0) {
          this.vy[index] = -this.vy[index] * GROUND_BOUNCE_RESTITUTION;
          this.vx[index] *= GROUND_FRICTION;
          this.vz[index] *= GROUND_FRICTION;
        }
      }
      index += 1;
    }
  }

  write(batches: RenderBatches): void {
    const glow = batches.glow;
    const smoke = batches.smoke;
    for (let i = 0; i < this.count; i += 1) {
      const t = this.age[i] / this.life[i];
      const kind = this.kind[i];
      const r = this.red[i];
      const g = this.green[i];
      const b = this.blue[i];
      if (kind === FxKind.Smoke) {
        const fade = Math.min(1, t * 5) * (1 - t);
        const size = this.startSize[i] + ((this.endSize[i] - this.startSize[i]) * Math.sqrt(t));
        smoke.push(this.x[i], this.y[i], this.z[i], this.spin[i] * this.age[i], size, size, i % 4, r, g, b, fade * 0.62);
      } else if (kind === FxKind.Fireball) {
        const grow = 1 - ((1 - t) * (1 - t) * (1 - t));
        const size = this.startSize[i] + ((this.endSize[i] - this.startSize[i]) * grow);
        const heat = 1 - t;
        const warmMix = Math.min(1, t * 2.2);
        const coolMix = Math.max(0, (t - 0.45) / 0.55);
        const fr = mix3(FIREBALL_HOT.r, FIREBALL_WARM.r, FIREBALL_COOL.r, warmMix, coolMix);
        const fg = mix3(FIREBALL_HOT.g, FIREBALL_WARM.g, FIREBALL_COOL.g, warmMix, coolMix);
        const fb = mix3(FIREBALL_HOT.b, FIREBALL_WARM.b, FIREBALL_COOL.b, warmMix, coolMix);
        const intensity = 4 * heat * Math.sqrt(heat);
        glow.push(this.x[i], this.y[i], this.z[i], this.spin[i], size, size, 0, fr * intensity, fg * intensity, fb * intensity, heat);
      } else if (kind === FxKind.Ring || kind === FxKind.Halo) {
        const grow = 1 - ((1 - t) * (1 - t));
        const size = this.startSize[i] + ((this.endSize[i] - this.startSize[i]) * grow);
        const fade = (1 - t) * (1 - t);
        if (kind === FxKind.Ring) {
          batches.pushGroundGlow(this.x[i], this.y[i], this.z[i], size / 2, 1, r * 1.5, g * 1.5, b * 1.5, fade);
        } else {
          glow.push(this.x[i], this.y[i], this.z[i], 0, size, size, 1, r * 1.3, g * 1.3, b * 1.3, fade);
        }
      } else if (kind === FxKind.Ember) {
        const speed = Math.hypot(this.vx[i], this.vz[i]);
        const length = this.startSize[i] + Math.min(18, speed * 0.05);
        const fade = 1 - t;
        glow.push(this.x[i], this.y[i], this.z[i], Math.atan2(-this.vz[i], this.vx[i]), length, this.startSize[i] * 0.45, 0, r * 2, g * 2, b * 2, fade);
      } else if (kind === FxKind.Pillar) {
        const fade = (1 - t) * (1 - t);
        const width = this.startSize[i] * (1 + (t * 0.8));
        glow.push(this.x[i], this.y[i], this.z[i], 0, width, this.endSize[i], 0, r * 1.8, g * 1.8, b * 1.8, fade);
      } else {
        const size = this.startSize[i] + ((this.endSize[i] - this.startSize[i]) * t);
        const fade = kind === FxKind.Flash ? (1 - t) * (1 - t) * (1 - t) : Math.pow(1 - t, 1.4);
        const intensity = kind === FxKind.Flash ? 2.2 : 1.8;
        glow.push(this.x[i], this.y[i], this.z[i], 0, size, size, 0, r * intensity, g * intensity, b * intensity, fade);
      }
    }
  }

  private swapRemove(index: number): void {
    const last = this.count - 1;
    if (index !== last) {
      this.kind[index] = this.kind[last];
      this.x[index] = this.x[last];
      this.y[index] = this.y[last];
      this.z[index] = this.z[last];
      this.vx[index] = this.vx[last];
      this.vy[index] = this.vy[last];
      this.vz[index] = this.vz[last];
      this.age[index] = this.age[last];
      this.life[index] = this.life[last];
      this.startSize[index] = this.startSize[last];
      this.endSize[index] = this.endSize[last];
      this.gravity[index] = this.gravity[last];
      this.drag[index] = this.drag[last];
      this.red[index] = this.red[last];
      this.green[index] = this.green[last];
      this.blue[index] = this.blue[last];
      this.spin[index] = this.spin[last];
    }
    this.count = last;
  }
}

function mix3(hot: number, warm: number, cool: number, warmMix: number, coolMix: number): number {
  const first = hot + ((warm - hot) * warmMix);
  return first + ((cool - first) * coolMix);
}

const FLASH_LIGHT_DISTANCE = 190;
const FLASH_LIGHT_DECAY = 1.6;

interface PooledLight {
  x: number;
  y: number;
  z: number;
  red: number;
  green: number;
  blue: number;
  intensity: number;
  peak: number;
  age: number;
  duration: number;
}

/** Point-light slot layout the shaders read: position + cutoff, color x intensity + decay. */
export interface LightSink {
  readonly data: Float32Array;
  readonly positionOffset: number;
  readonly colorOffset: number;
  readonly slots: number;
}

/**
 * A fixed set of point lights reused for explosion flashes. The shaders always loop over
 * the same slot count; idle lights just sit at zero intensity.
 */
class FlashLightPool {
  private readonly lights: PooledLight[] = [];

  constructor(count: number) {
    for (let index = 0; index < count; index += 1) {
      this.lights.push({ x: 0, y: -500, z: 0, red: 1, green: 1, blue: 1, intensity: 0, peak: 0, age: 0, duration: 1 });
    }
  }

  flash(x: number, y: number, z: number, color: LinearColor, peak: number, duration: number): void {
    let chosen: PooledLight | undefined;
    let weakest = Infinity;
    for (const pooled of this.lights) {
      if (pooled.intensity < weakest) {
        weakest = pooled.intensity;
        chosen = pooled;
      }
    }
    if (!chosen || weakest > peak) {
      return;
    }
    chosen.x = x;
    chosen.y = y;
    chosen.z = z;
    chosen.red = color.r;
    chosen.green = color.g;
    chosen.blue = color.b;
    chosen.peak = peak;
    chosen.age = 0;
    chosen.duration = duration;
    chosen.intensity = peak;
  }

  update(deltaSeconds: number): void {
    for (const pooled of this.lights) {
      if (pooled.intensity <= 0) {
        continue;
      }
      pooled.age += deltaSeconds;
      const t = pooled.age / pooled.duration;
      pooled.intensity = t >= 1 ? 0 : pooled.peak * (1 - t) * (1 - t);
    }
  }

  write(sink: LightSink): void {
    const { data, positionOffset, colorOffset } = sink;
    for (let slot = 0; slot < sink.slots; slot += 1) {
      const light = this.lights[slot];
      const p = positionOffset + (slot * 4);
      const c = colorOffset + (slot * 4);
      const intensity = light?.intensity ?? 0;
      data[p] = light?.x ?? 0;
      data[p + 1] = light?.y ?? -500;
      data[p + 2] = light?.z ?? 0;
      data[p + 3] = FLASH_LIGHT_DISTANCE;
      data[c] = (light?.red ?? 0) * intensity;
      data[c + 1] = (light?.green ?? 0) * intensity;
      data[c + 2] = (light?.blue ?? 0) * intensity;
      data[c + 3] = FLASH_LIGHT_DECAY;
    }
  }

  clear(): void {
    for (const pooled of this.lights) {
      pooled.intensity = 0;
    }
  }
}

const SCORCH_CAPACITY = 48;
const SCORCH_LIFETIME_SECONDS = 9;

class ScorchMarks {
  private readonly x = new Float32Array(SCORCH_CAPACITY);
  private readonly z = new Float32Array(SCORCH_CAPACITY);
  private readonly size = new Float32Array(SCORCH_CAPACITY);
  private readonly rotation = new Float32Array(SCORCH_CAPACITY);
  private readonly age = new Float32Array(SCORCH_CAPACITY).fill(SCORCH_LIFETIME_SECONDS);
  private next = 0;

  add(x: number, z: number, size: number): void {
    const index = this.next;
    this.next = (this.next + 1) % SCORCH_CAPACITY;
    this.x[index] = x;
    this.z[index] = z;
    this.size[index] = size;
    this.rotation[index] = randomRange(0, Math.PI * 2);
    this.age[index] = 0;
  }

  update(deltaSeconds: number): void {
    for (let index = 0; index < SCORCH_CAPACITY; index += 1) {
      this.age[index] += deltaSeconds;
    }
  }

  write(batches: RenderBatches): void {
    for (let index = 0; index < SCORCH_CAPACITY; index += 1) {
      const t = this.age[index] / SCORCH_LIFETIME_SECONDS;
      if (t >= 1) {
        continue;
      }
      const fadeIn = Math.min(1, this.age[index] * 6);
      const slot = batches.decal.pushYaw(this.x[index], 0.55, this.z[index], this.rotation[index], this.size[index], 1, this.size[index], 0.004, 0.006, 0.005);
      batches.decal.setExtra(slot, 0, 0.72 * fadeIn * (1 - (t * t)));
      batches.decal.setExtra(slot, 1, 0);
    }
  }

  clear(): void {
    this.age.fill(SCORCH_LIFETIME_SECONDS);
  }
}

export interface FxBudget {
  particles: number;
  lights: number;
}

/** Renderer-only spectacle layered on top of the simulation's own effect particles. */
export class FxSystem {
  private readonly particles: FxParticlePool;
  private readonly lights: FlashLightPool;
  private readonly scorch = new ScorchMarks();

  constructor(private readonly rig: CameraRig, budget: FxBudget) {
    this.particles = new FxParticlePool(budget.particles);
    this.lights = new FlashLightPool(budget.lights);
  }

  get activeParticles(): number {
    return this.particles.size;
  }

  clear(): void {
    this.particles.clear();
    this.lights.clear();
    this.scorch.clear();
  }

  update(deltaSeconds: number): void {
    this.particles.update(deltaSeconds);
    this.lights.update(deltaSeconds);
    this.scorch.update(deltaSeconds);
  }

  write(batches: RenderBatches): void {
    this.scorch.write(batches);
    this.particles.write(batches);
  }

  writeLights(sink: LightSink): void {
    this.lights.write(sink);
  }

  /** Missile blast: fireball, smoke column, sparks, scorch, light flash, and shake. */
  explosion(x: number, z: number, scale: number): void {
    const p = this.particles;
    this.lights.flash(x, 22, z, EXPLOSION_LIGHT, 5200 * scale, 0.45);
    p.spawn(FxKind.Flash, x, 16, z, 0, 0, 0, 0.18, 30 * scale, 64 * scale, FIREBALL_HOT, 0, 0);
    for (let index = 0; index < 7; index += 1) {
      const angle = randomRange(0, Math.PI * 2);
      const spread = randomRange(0, 7) * scale;
      p.spawn(
        FxKind.Fireball,
        x + (Math.cos(angle) * spread),
        randomRange(12, 22),
        z + (Math.sin(angle) * spread),
        Math.cos(angle) * randomRange(20, 50),
        randomRange(30, 70),
        Math.sin(angle) * randomRange(20, 50),
        randomRange(0.42, 0.7),
        randomRange(8, 12) * scale,
        randomRange(26, 40) * scale,
        FIREBALL_WARM,
        0,
        3,
      );
    }
    p.spawn(FxKind.Pillar, x, 18, z, 0, 0, 0, 0.28, 10 * scale, 46 * scale, FIREBALL_HOT, 0, 0);
    for (let index = 0; index < 7; index += 1) {
      p.spawn(
        FxKind.Smoke,
        x + randomRange(-9, 9) * scale,
        randomRange(6, 14),
        z + randomRange(-9, 9) * scale,
        randomRange(-14, 14),
        randomRange(16, 34),
        randomRange(-14, 14),
        randomRange(1.1, 1.9),
        randomRange(10, 16) * scale,
        randomRange(30, 46) * scale,
        SMOKE_COLOR,
        0,
        1.4,
      );
    }
    for (let index = 0; index < 12; index += 1) {
      const angle = randomRange(0, Math.PI * 2);
      const speed = randomRange(90, 230) * scale;
      p.spawn(
        FxKind.Ember,
        x,
        randomRange(4, 10),
        z,
        Math.cos(angle) * speed,
        randomRange(70, 190),
        Math.sin(angle) * speed,
        randomRange(0.5, 0.95),
        randomRange(3, 5),
        0,
        index % 3 === 0 ? FIREBALL_HOT : FIREBALL_WARM,
        520,
        1.2,
      );
    }
    p.spawn(FxKind.Ring, x, 1.2, z, 0, 0, 0, 0.42, 12 * scale, 110 * scale, FIREBALL_WARM, 0, 0);
    this.scorch.add(x, z, randomRange(30, 40) * scale);
    this.rig.addTrauma(0.2 * scale);
  }

  /** Death pop in the monster's color; heavy monsters add smoke, a scorch, and shake. */
  monsterDeath(x: number, y: number, z: number, css: string, radius: number, heavy: boolean): void {
    const color = linearColor(css);
    const p = this.particles;
    this.lights.flash(x, y + 10, z, color, heavy ? 2600 : 1300, heavy ? 0.4 : 0.26);
    p.spawn(FxKind.Flash, x, y, z, 0, 0, 0, 0.14, radius * 2.4, radius * 6.5, color, 0, 0);
    p.spawn(FxKind.Ring, x, 1, z, 0, 0, 0, 0.36, radius * 1.6, radius * 7, color, 0, 0);
    const sparkCount = heavy ? 16 : 9;
    for (let index = 0; index < sparkCount; index += 1) {
      const angle = randomRange(0, Math.PI * 2);
      const speed = randomRange(60, 170);
      p.spawn(
        FxKind.Spark,
        x,
        y,
        z,
        Math.cos(angle) * speed,
        randomRange(60, 200),
        Math.sin(angle) * speed,
        randomRange(0.4, 0.85),
        randomRange(2.6, 4.2),
        0.6,
        color,
        480,
        0.9,
      );
    }
    if (heavy) {
      for (let index = 0; index < 4; index += 1) {
        p.spawn(
          FxKind.Smoke,
          x + randomRange(-6, 6),
          y,
          z + randomRange(-6, 6),
          randomRange(-10, 10),
          randomRange(14, 26),
          randomRange(-10, 10),
          randomRange(1, 1.6),
          radius * 1.2,
          radius * 3.6,
          SMOKE_COLOR,
          0,
          1.2,
        );
      }
      this.scorch.add(x, z, radius * 3);
      this.rig.addTrauma(0.12);
    }
  }

  /** Base breach: towering light pillar, double shockwave, and heavy shake. */
  escapeBlast(x: number, z: number): void {
    const p = this.particles;
    this.lights.flash(x, 30, z, ESCAPE_COLOR, 9000, 0.8);
    p.spawn(FxKind.Pillar, x, 60, z, 0, 0, 0, 0.9, 20, 150, ESCAPE_COLOR, 0, 0);
    p.spawn(FxKind.Pillar, x, 50, z, 0, 0, 0, 0.6, 8, 130, ESCAPE_ACCENT, 0, 0);
    p.spawn(FxKind.Flash, x, 12, z, 0, 0, 0, 0.3, 40, 120, ESCAPE_COLOR, 0, 0);
    p.spawn(FxKind.Ring, x, 1.5, z, 0, 0, 0, 0.7, 20, 190, ESCAPE_COLOR, 0, 0);
    p.spawn(FxKind.Ring, x, 1.5, z, 0, 0, 0, 0.5, 10, 110, ESCAPE_ACCENT, 0, 0);
    this.rig.addTrauma(0.55);
  }

  spawnPulse(x: number, z: number, css: string, radius: number): void {
    this.particles.spawn(FxKind.Ring, x, 1, z, 0, 0, 0, 0.45, radius * 1.5, radius * 5, linearColor(css), 0, 0);
  }

  shieldHit(x: number, y: number, z: number, radius: number, css: string): void {
    this.particles.spawn(FxKind.Halo, x, y, z, 0, 0, 0, 0.22, radius * 2.4, radius * 3.4, linearColor(css), 0, 0);
  }

  ember(x: number, y: number, z: number, vx: number, vz: number, css: string): void {
    this.particles.spawn(FxKind.Ember, x, y, z, vx, randomRange(10, 40), vz, randomRange(0.25, 0.45), randomRange(2.4, 3.6), 0, linearColor(css), 60, 2);
  }

  zap(x: number, y: number, z: number): void {
    this.lights.flash(x, y + 6, z, ZAP_COLOR, 1100, 0.14);
  }
}

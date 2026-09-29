import type { UpdateContext } from "../../game-engine/update-context";
import { randomRange } from "../../utils";
import { Particle } from "./particle";

const MISSILE_EXPLOSION_SCALE_BASE = 0.8;
const MISSILE_EXPLOSION_SCALE_PER_LEVEL = 0.06;

function getMissileExplosionScale(level: number): number {
  return MISSILE_EXPLOSION_SCALE_BASE + (MISSILE_EXPLOSION_SCALE_PER_LEVEL * level);
}

export class SmokeParticle extends Particle {
  private readonly maxSize: number;
  private readonly growthPerSecond: number;

  constructor(x: number, y: number, blastAngle: number, level: number) {
    const scale = getMissileExplosionScale(level);
    const angle = blastAngle + Math.PI + randomRange(-1.1, 1.1);
    const size = randomRange(3.2, 6.8) * scale;
    super(x, y, size, "#7d7b72", randomRange(0.7, 1.15), {
      speedPerSecond: randomRange(22, 78) * scale,
      offset: randomRange(1, 7) * scale,
      angle,
    });
    this.alpha = randomRange(0.32, 0.58);
    this.maxSize = size + (randomRange(4, 8) * scale);
    this.growthPerSecond = 12 * scale;
  }

  override update(context: UpdateContext): void {
    super.update(context);
    this.size = Math.min(this.maxSize, this.size + (this.growthPerSecond * context.deltaSeconds));
  }
}

export class EmberStreakParticle extends Particle {
  constructor(x: number, y: number, blastAngle: number, level: number) {
    const scale = getMissileExplosionScale(level);
    const angle = blastAngle + Math.PI + randomRange(-1.8, 1.8);
    super(x, y, randomRange(2.1, 3.6) * scale, randomRange(0, 1) > 0.45 ? "#ff8f45" : "#fff0a8", randomRange(3.5, 5.4), {
      speedPerSecond: randomRange(175, 385) * scale,
      offset: randomRange(2, 6) * scale,
      angle,
    });
  }
}

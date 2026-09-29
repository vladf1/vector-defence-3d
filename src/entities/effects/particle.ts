import type { UpdateContext } from "../../game-engine/update-context";
import { CalibratedExponentialDecay, isOutsideBounds, randomRange } from "../../utils";

const VELOCITY_DECAY = new CalibratedExponentialDecay(2.4, 60);

interface ParticleOptions {
  speedPerSecond?: number;
  offset?: number;
  angle?: number;
}

export class Particle {
  x: number;
  y: number;
  velocityXPerSecond: number;
  velocityYPerSecond: number;
  size: number;
  color: string;
  alpha = 1;
  alphaFadePerSecond: number;
  removed = false;

  constructor(
    x: number,
    y: number,
    size: number,
    color: string,
    alphaFadePerSecond: number,
    options: ParticleOptions = {},
  ) {
    const speedPerSecond = options.speedPerSecond ?? randomRange(120, 420);
    const offset = options.offset ?? randomRange(4, 6);
    const angle = options.angle ?? randomRange(-Math.PI, Math.PI);
    this.velocityXPerSecond = Math.cos(angle) * speedPerSecond;
    this.velocityYPerSecond = Math.sin(angle) * speedPerSecond;
    this.x = x + (Math.cos(angle) * offset);
    this.y = y + (Math.sin(angle) * offset);
    this.size = size;
    this.color = color;
    this.alphaFadePerSecond = alphaFadePerSecond;
  }

  update(context: UpdateContext): void {
    VELOCITY_DECAY.apply(this, context.deltaSeconds);
    this.alpha -= this.alphaFadePerSecond * context.deltaSeconds;
    if (this.alpha <= 0 || isOutsideBounds(this, context.fieldBounds, 20)) {
      this.removed = true;
    }
  }
}

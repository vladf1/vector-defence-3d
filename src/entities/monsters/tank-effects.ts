import type { UpdateContext } from "../../game-engine/update-context";
import { CalibratedExponentialDecay, isOutsideBounds, randomRange } from "../../utils";
import { Particle } from "../effects/particle";

const PRINT_COLOR = "#86ad99";
const PRINT_ALPHA = 0.46;
const PRINT_FADE_PER_SECOND = 0.234;
const DRIFT_VELOCITY_DECAY = new CalibratedExponentialDecay(0.34, 60);

export class TankTrackPrintParticle extends Particle {
  readonly angle: number;

  constructor(x: number, y: number, angle: number) {
    super(x, y, 1, PRINT_COLOR, PRINT_FADE_PER_SECOND, {
      speedPerSecond: 0,
      offset: 0,
      angle: 0,
    });
    this.alpha = PRINT_ALPHA;
    this.angle = angle;
  }

  override update(context: UpdateContext): void {
    this.alpha -= this.alphaFadePerSecond * context.deltaSeconds;
    if (this.alpha <= 0) {
      this.removed = true;
    }
  }
}

export class TankTurretParticle extends Particle {
  alpha = 1;
  alphaFadePerSecond: number;
  radius: number;
  rotation: number;
  barrelRotation: number;
  angularVelocityPerSecond: number;

  constructor(
    x: number,
    y: number,
    radius: number,
    color: string,
    rotation: number,
    barrelRotation: number,
  ) {
    super(x, y, radius * 2, color, 0, { speedPerSecond: 0, offset: 0 });
    const travelAngle = randomRange(-Math.PI, Math.PI);
    const speedPerSecond = randomRange(115, 185);
    this.velocityXPerSecond = Math.cos(travelAngle) * speedPerSecond;
    this.velocityYPerSecond = Math.sin(travelAngle) * speedPerSecond;
    this.radius = radius;
    this.rotation = rotation;
    this.barrelRotation = barrelRotation;
    this.angularVelocityPerSecond = randomRange(-12.5, 12.5);
    this.alphaFadePerSecond = randomRange(0.45, 0.78);
  }

  override update(context: UpdateContext): void {
    DRIFT_VELOCITY_DECAY.apply(this, context.deltaSeconds);
    this.rotation += this.angularVelocityPerSecond * context.deltaSeconds;
    this.alpha = Math.max(0, this.alpha - (this.alphaFadePerSecond * context.deltaSeconds));
    if (this.alpha <= 0 || isOutsideBounds(this, context.fieldBounds, 34)) {
      this.removed = true;
    }
  }
}

export function getTankTurretCenterOffsetX(tankRadius: number): number {
  return tankRadius * 0.08;
}

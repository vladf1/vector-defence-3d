import type { UpdateContext } from "../../game-engine/update-context";
import type { Point } from "../../types";
import { CalibratedExponentialDecay, isOutsideBounds, randomRange } from "../../utils";
import { Particle } from "./particle";

const DRIFT_VELOCITY_DECAY = new CalibratedExponentialDecay(0.58, 60);

export class EscapeFragmentParticle extends Particle {
  alpha = 1;
  alphaFadePerSecond: number;
  angularVelocityPerSecond: number;
  rotation: number;
  vertices: Point[];

  constructor(
    x: number,
    y: number,
    color: string,
    angle: number,
    speedPerSecond: number,
    length: number,
    width: number,
    initialSeparation: number,
  ) {
    super(x, y, Math.max(length, width), color, 0, {
      speedPerSecond,
      offset: initialSeparation,
      angle,
    });
    this.rotation = angle + randomRange(-0.6, 0.6);
    this.angularVelocityPerSecond = randomRange(-13.5, 13.5);
    this.alphaFadePerSecond = randomRange(0.95, 1.55);
    this.vertices = createFragmentVertices(length, width);
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

function createFragmentVertices(length: number, width: number): Point[] {
  const pointCount = 4 + Math.floor(randomRange(0, 4));
  const vertices: Point[] = [];
  const angleOffset = randomRange(-0.26, 0.26);

  for (let index = 0; index < pointCount; index += 1) {
    const angle = angleOffset + ((Math.PI * 2 * index) / pointCount) + randomRange(-0.2, 0.2);
    const lengthRadius = (length / 2) * (Math.cos(angle) > 0 ? randomRange(0.72, 1.22) : randomRange(0.44, 0.95));
    const widthRadius = (width / 2) * randomRange(0.58, 1.28);
    vertices.push({
      x: (Math.cos(angle) * lengthRadius) + randomRange(length * -0.08, length * 0.08),
      y: (Math.sin(angle) * widthRadius) + randomRange(width * -0.18, width * 0.18),
    });
  }

  return vertices;
}

import type { UpdateContext } from "../../game-engine/update-context";
import type { Point } from "../../types";
import { CalibratedExponentialDecay, isOutsideBounds, randomRange } from "../../utils";
import { Particle } from "./particle";

const ANGULAR_VELOCITY_MAX_PER_SECOND = 9.9;
const DRIFT_VELOCITY_DECAY = new CalibratedExponentialDecay(0.42, 60);

export class GlassShardParticle extends Particle {
  alpha = 1;
  alphaFadePerSecond: number;
  rotation: number;
  angularVelocityPerSecond: number;
  vertices: Point[];

  constructor(
    x: number,
    y: number,
    color: string,
    vertices: Point[],
    origin: Point,
    rotation: number,
    speedPerSecond: number,
    initialSeparation: number,
  ) {
    super(x, y, 1, color, 0, { speedPerSecond, offset: 0 });
    const centroid = getCentroid(vertices);
    const travelAngle =
      Math.atan2(centroid.y - origin.y, centroid.x - origin.x) +
      rotation +
      randomRange(-0.22, 0.22);
    this.velocityXPerSecond = Math.cos(travelAngle) * speedPerSecond;
    this.velocityYPerSecond = Math.sin(travelAngle) * speedPerSecond;
    this.x += Math.cos(travelAngle) * initialSeparation;
    this.y += Math.sin(travelAngle) * initialSeparation;
    this.vertices = vertices;
    this.rotation = rotation;
    this.angularVelocityPerSecond = randomRange(-ANGULAR_VELOCITY_MAX_PER_SECOND, ANGULAR_VELOCITY_MAX_PER_SECOND);
    this.alphaFadePerSecond = randomRange(0.8, 1.8);
  }

  update(context: UpdateContext): void {
    DRIFT_VELOCITY_DECAY.apply(this, context.deltaSeconds);
    this.rotation += this.angularVelocityPerSecond * context.deltaSeconds;
    this.alpha = Math.max(0, this.alpha - (this.alphaFadePerSecond * context.deltaSeconds));
    if (this.alpha <= 0 || isOutsideBounds(this, context.fieldBounds, 28)) {
      this.removed = true;
    }
  }
}

function getCentroid(vertices: Point[]): Point {
  let sumX = 0;
  let sumY = 0;
  for (const vertex of vertices) {
    sumX += vertex.x;
    sumY += vertex.y;
  }
  return {
    x: sumX / vertices.length,
    y: sumY / vertices.length,
  };
}

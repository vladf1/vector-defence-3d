import type { UpdateContext } from "../../game-engine/update-context";
import { Particle } from "./particle";

export class HitRingEffect extends Particle {
  constructor(x: number, y: number, readonly ringColor: string, readonly maxRadius: number) {
    super(x, y, 0, ringColor, 1, { speedPerSecond: 0, offset: 0, angle: 0 });
    this.alpha = 0.85;
  }

  override update(context: UpdateContext): void {
    this.alpha = Math.max(0, this.alpha - (5.2 * context.deltaSeconds));
    if (this.alpha <= 0) {
      this.removed = true;
    }
  }
}

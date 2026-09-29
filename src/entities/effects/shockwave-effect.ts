import type { UpdateContext } from "../../game-engine/update-context";
import { Particle } from "./particle";

export class ShockwaveEffect extends Particle {
  private ageSeconds = 0;

  constructor(x: number, y: number, readonly scale: number) {
    super(x, y, 0, "#fff0a8", 1, { speedPerSecond: 0, offset: 0, angle: 0 });
    this.alpha = 1;
  }

  override update(context: UpdateContext): void {
    this.ageSeconds += context.deltaSeconds;
    this.alpha = Math.max(0, 1 - (this.ageSeconds * 4.15));
    if (this.alpha <= 0) {
      this.removed = true;
    }
  }
}

import type { UpdateContext } from "../../game-engine/update-context";
import type { Monster } from "../monsters/monster";

interface LightningSource {
  x: number;
  y: number;
  readonly visualX?: number;
  readonly visualY?: number;
  level?: number;
  removed?: boolean;
}

export class LightningLinkEffect {
  source: LightningSource;
  target: Monster;
  color: string;
  alpha = 0.92;
  ageSeconds = 0;
  removed = false;

  constructor(source: LightningSource, target: Monster, color: string) {
    this.source = source;
    this.target = target;
    this.color = color;
  }

  update(context: UpdateContext): void {
    if (this.target.removed || this.source.removed) {
      this.alpha = 0;
    } else {
      this.ageSeconds += context.deltaSeconds;
      this.alpha -= 3.8 * context.deltaSeconds;
    }

    if (this.alpha <= 0) {
      this.removed = true;
    }
  }
}

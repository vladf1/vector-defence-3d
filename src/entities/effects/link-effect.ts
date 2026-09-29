import type { UpdateContext } from "../../game-engine/update-context";
import type { Monster } from "../monsters/monster";

interface LinkSource {
  x: number;
  y: number;
  readonly visualX?: number;
  readonly visualY?: number;
  level?: number;
  removed?: boolean;
}

export class LinkEffect {
  source: LinkSource;
  target: Monster;
  color: string;
  alpha: number;
  alphaFadePerSecond: number;
  ageSeconds = 0;
  removed = false;

  constructor(target: Monster, color: string, alphaFadePerSecond: number, source: LinkSource) {
    this.target = target;
    this.color = color;
    this.alphaFadePerSecond = alphaFadePerSecond;
    this.source = source;
    this.alpha = color === "#d8ff4f" ? 0.8 : 0.7;
  }

  update(context: UpdateContext): void {
    if (this.target.removed || this.source.removed) {
      this.alpha = 0;
    } else {
      this.ageSeconds += context.deltaSeconds;
      this.alpha -= this.alphaFadePerSecond * context.deltaSeconds;
    }
    if (this.alpha <= 0) {
      this.removed = true;
    }
  }
}

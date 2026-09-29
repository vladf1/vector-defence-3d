import { AudioCue } from "../../audio-manifest";
import type { UpdateContext, UpdateResult } from "../../game-engine/update-context";
import { TowerKind } from "../../types";
import { withinDistance } from "../../utils";
import { LinkEffect } from "../effects/link-effect";
import { Tower } from "./tower";

const SLOW_FACTOR = 0.5;
const RECOVERY_SPEED_PER_SECOND = 36;

export class SlowTower extends Tower {
  static readonly kind = TowerKind.Slow;
  static readonly label = "Slow";
  static readonly summary = "Freezes clusters so the rest can clean up.";
  static readonly baseCost = 3;
  static readonly baseRange = 70;
  static readonly shortcuts = ["4", "s"] as const;

  pulse = 0;
  orbit = 0;

  protected updateTower(context: UpdateContext, result: UpdateResult): void {
    this.pulse += 4.8 * context.deltaSeconds;
    this.orbit += this.getOrbitSpeedPerSecond() * context.deltaSeconds;
    if (!this.ready()) {
      return;
    }

    let affected = 0;
    const maxTargets = this.level + 2;
    for (const monster of context.activeMonsters) {
      if (!this.isMonsterActive(monster)) {
        continue;
      }
      if (!withinDistance(this, monster, this.range)) {
        continue;
      }
      monster.slowDown(SLOW_FACTOR, RECOVERY_SPEED_PER_SECOND);
      if (result.remainingLinkCapacity > 0) {
        result.addLink(new LinkEffect(monster, "#d8ff4f", 1, this));
      }
      affected += 1;
      if (affected === maxTargets) {
        break;
      }
    }

    if (affected === 0) {
      return;
    }

    this.resetCooldown(1);
    result.playSound(AudioCue.SlowPulse, this.x, 0.85 + (affected * 0.1));
  }

  private getOrbitSpeedPerSecond(): number {
    return 4.8 / (1 + (this.level * 0.7));
  }
}

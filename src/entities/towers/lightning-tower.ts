import { AudioCue } from "../../audio-manifest";
import type { UpdateContext, UpdateResult } from "../../game-engine/update-context";
import { TowerKind } from "../../types";
import { withinDistance } from "../../utils";
import { LightningLinkEffect } from "../effects/lightning-link-effect";
import type { Monster } from "../monsters/monster";
import { Tower } from "./tower";

export const LIGHTNING_COLORS = [
  "#8ff7ff",
  "#7fe5ff",
  "#71d0ff",
  "#9fb8ff",
  "#c9a7ff",
  "#f09cff",
  "#f5fbff",
] as const;

const SLOW_FACTOR = 0.15;
const RECOVERY_SPEED_PER_SECOND = 100;

export class LightningTower extends Tower {
  static readonly kind = TowerKind.Lightning;
  static readonly label = "Lightning";
  static readonly summary = "Chains shocks that damage and heavily slow monsters.";
  static readonly baseCost = 5;
  static readonly baseRange = 74;
  static readonly shortcuts = ["5", "e"] as const;

  protected updateTower(context: UpdateContext, result: UpdateResult): void {
    if (!this.ready()) {
      return;
    }

    const firstTarget = this.findClosestMonsterInContext(context);
    if (!firstTarget) {
      return;
    }

    const targets = this.collectChainTargets(context, firstTarget);
    const damage = this.getDamage();
    const color = this.getColor();
    let source: Tower | Monster = this;

    for (const target of targets) {
      target.takeDamage(damage);
      target.shakeFromHit();
      target.slowDown(SLOW_FACTOR, RECOVERY_SPEED_PER_SECOND);
      if (result.remainingLinkCapacity > 0) {
        result.addLink(new LightningLinkEffect(source, target, color));
      }
      source = target;
    }

    this.resetCooldown(this.getCooldownSeconds());
    result.playSound(AudioCue.LightningShock, this.x, 0.95 + (this.level * 0.09));
  }

  private collectChainTargets(context: UpdateContext, firstTarget: Monster): Monster[] {
    const targets = [firstTarget];
    let source = firstTarget;
    const maxTargets = Math.min(6, 2 + Math.floor(this.level / 2));
    const chainRange = 58 + (this.level * 5);

    while (targets.length < maxTargets) {
      let nextTarget: Monster | undefined;
      let closestDistanceSquared = Number.POSITIVE_INFINITY;

      for (const monster of context.activeMonsters) {
        if (!this.isMonsterActive(monster) || targets.includes(monster)) {
          continue;
        }
        if (!withinDistance(source, monster, chainRange)) {
          continue;
        }

        const dx = monster.x - source.x;
        const dy = monster.y - source.y;
        const distanceSquared = (dx * dx) + (dy * dy);
        if (distanceSquared < closestDistanceSquared) {
          closestDistanceSquared = distanceSquared;
          nextTarget = monster;
        }
      }

      if (!nextTarget) {
        break;
      }

      targets.push(nextTarget);
      source = nextTarget;
    }

    return targets;
  }

  private getDamage(): number {
    return 12 + (this.level * 4.8);
  }

  private getCooldownSeconds(): number {
    return Math.max(0.72, 1.12 - (this.level * 0.055));
  }

  private getColor(): string {
    return LIGHTNING_COLORS[Math.min(this.level, LIGHTNING_COLORS.length - 1)];
  }
}

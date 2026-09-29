import { AudioCue } from "../../audio-manifest";
import type { UpdateContext, UpdateResult } from "../../game-engine/update-context";
import { TowerKind, type Point } from "../../types";
import { angleBetween, calculateIntercept, randomRange, turnAngleTowards } from "../../utils";
import { GUN_PROJECTILE_SPEED_PER_SECOND, GunProjectile } from "../projectiles/gun-projectile";
import { Tower } from "./tower";

export const MUZZLE_FLASH_DURATION_SECONDS = 0.055;
export const GUN_PROJECTILE_SOURCE_OFFSET = 16;

export class GunTower extends Tower {
  static readonly kind = TowerKind.Gun;
  static readonly label = "Gun";
  static readonly summary = "Fast, cheap, accurate lead shots.";
  static readonly baseCost = 2;
  static readonly baseRange = 60;
  static readonly shortcuts = ["1", "g"] as const;

  angle = randomRange(-Math.PI, Math.PI);
  turnSpeedPerSecond = 9.6;
  muzzleFlashSeconds = 0;

  protected updateTower(context: UpdateContext, result: UpdateResult): void {
    this.muzzleFlashSeconds = Math.max(0, this.muzzleFlashSeconds - context.deltaSeconds);
    const tracked = this.findTrackedMonsterInContext(context);
    if (!tracked) {
      return;
    }

    const source = this.getProjectileSource();
    const target = calculateIntercept(tracked, GUN_PROJECTILE_SPEED_PER_SECOND, source);
    const targetAngle = angleBetween(this, target);
    this.angle = turnAngleTowards(this.angle, targetAngle, this.turnSpeedPerSecond * context.deltaSeconds);

    const alignedToTarget = this.isAimedAtTarget(this.angle, targetAngle);
    if (alignedToTarget && this.ready()) {
      const actualSource = this.getProjectileSource();
      this.muzzleFlashSeconds = MUZZLE_FLASH_DURATION_SECONDS;
      this.resetCooldown(0.2);
      result.addProjectile(new GunProjectile(actualSource, target, this.level));
      result.playSound(AudioCue.GunFire, actualSource.x, 0.92 + (this.level * 0.08));
    }
  }

  private getProjectileSource(): Point {
    return {
      x: this.x + (Math.cos(this.angle) * GUN_PROJECTILE_SOURCE_OFFSET),
      y: this.y + (Math.sin(this.angle) * GUN_PROJECTILE_SOURCE_OFFSET),
    };
  }
}

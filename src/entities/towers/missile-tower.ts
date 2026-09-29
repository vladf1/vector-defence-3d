import { AudioCue } from "../../audio-manifest";
import type { UpdateContext, UpdateResult } from "../../game-engine/update-context";
import { TowerKind } from "../../types";
import { angleBetween, clamp, randomRange, turnAngleTowards } from "../../utils";
import { Missile } from "../projectiles/missile";
import { createMissileVisual } from "../projectiles/missile-visuals";
import { Tower } from "./tower";

const MISSILE_FIRING_ANGLE_TOLERANCE = Math.PI / 12;
const MISSILE_RACK_CENTER_X = 0;
export const MISSILE_POWERBANK_COLORS = [
  "#9dffd7",
  "#d8ff4f",
  "#ff9d5c",
  "#ff6d8c",
  "#b58cff",
  "#78a7ff",
  "#f6f0ff",
] as const;

export class MissileTower extends Tower {
  static readonly kind = TowerKind.Missile;
  static readonly label = "Missile";
  static readonly summary = "Slow launcher with splash damage.";
  static readonly baseCost = 5;
  static readonly baseRange = 150;
  static readonly shortcuts = ["3", "r"] as const;

  angle = randomRange(-Math.PI, Math.PI);
  turnSpeedPerSecond = 3.6;
  private visual = createMissileVisual(this.level);

  protected onUpgrade(): void {
    this.visual = createMissileVisual(this.level);
  }

  protected updateTower(context: UpdateContext, result: UpdateResult): void {
    const tracked = this.findTrackedMonsterInContext(context);
    let alignedToTarget = false;

    if (tracked) {
      const targetAngle = angleBetween(this, tracked);
      this.angle = turnAngleTowards(this.angle, targetAngle, this.turnSpeedPerSecond * context.deltaSeconds);
      alignedToTarget = this.isAimedAtTarget(this.angle, targetAngle, MISSILE_FIRING_ANGLE_TOLERANCE);
    }

    if (tracked && alignedToTarget && this.ready()) {
      const cos = Math.cos(this.angle);
      const sin = Math.sin(this.angle);
      const source = {
        x: this.x + (cos * MISSILE_RACK_CENTER_X),
        y: this.y + (sin * MISSILE_RACK_CENTER_X),
      };
      this.resetCooldown(this.getCooldownDurationSeconds());
      result.addMissile(new Missile(source, tracked, this.level, this.visual, this.angle));
      result.playSound(AudioCue.MissileLaunch, source.x, 1 + (this.level * 0.09));
    }
  }

  private getCooldownDurationSeconds(): number {
    return 2 - (0.2 * this.level);
  }

  /** 0 right after launch, 1 once the next missile is seated. */
  getReloadProgress(): number {
    return this.ready()
      ? 1
      : 1 - clamp(this.cooldownSeconds / this.getCooldownDurationSeconds(), 0, 1);
  }
}

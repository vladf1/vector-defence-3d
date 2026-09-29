import { AudioCue } from "../../audio-manifest";
import type { UpdateContext, UpdateResult } from "../../game-engine/update-context";
import { TowerKind } from "../../types";
import { Drone } from "../projectiles/drone";
import { Tower } from "./tower";

const DRONE_COOLDOWN_SECONDS = 30;
export class DroneTower extends Tower {
  static readonly kind = TowerKind.Drone;
  static readonly label = "Drone";
  static readonly summary = "Launches one autonomous hunter drone on a long cooldown.";
  static readonly baseCost = 5;
  static readonly baseRange = 115;
  static readonly shortcuts = ["5", "d"] as const;

  protected updateTower(context: UpdateContext, result: UpdateResult): void {
    if (!this.ready()) {
      return;
    }

    const target = this.findClosestMonsterInContext(context);
    if (!target) {
      return;
    }

    result.addDrone(new Drone({ x: this.x, y: this.y }, this.level));
    this.resetCooldown(DRONE_COOLDOWN_SECONDS);
    result.playSound(AudioCue.GunFire, this.x, 0.22 + (this.level * 0.025));
  }

  /** 0 right after launch, 1 when a drone is docked and ready. */
  getLaunchReadiness(): number {
    return this.ready() ? 1 : 1 - Math.min(1, this.cooldownSeconds / DRONE_COOLDOWN_SECONDS);
  }
}

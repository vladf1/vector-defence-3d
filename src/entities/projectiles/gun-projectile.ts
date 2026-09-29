import type { Point } from "../../types";
import { Projectile } from "./projectile";

const PROJECTILE_DAMAGE_BASE = 10;
const PROJECTILE_SIZE_BASE = 3;
const PROJECTILE_SIZE_PER_LEVEL = 0.5;
export const GUN_PROJECTILE_SPEED_PER_SECOND = 420;

export class GunProjectile extends Projectile {
  readonly visualLevel: number;

  constructor(source: Point, target: Point, level: number) {
    super(
      source,
      target,
      GUN_PROJECTILE_SPEED_PER_SECOND,
      PROJECTILE_DAMAGE_BASE + level,
      (PROJECTILE_SIZE_BASE + (level * PROJECTILE_SIZE_PER_LEVEL)) / 2,
      "#9fffe4",
    );
    this.visualLevel = level;
  }
}

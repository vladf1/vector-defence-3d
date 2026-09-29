import {
  MAX_TOWER_LEVEL,
  TOWER_RANGE_UPGRADE_STEP,
  UPGRADE_COST,
  TIMER_EPSILON_SECONDS,
} from "../../constants";
import type { TowerKind } from "../../types";
import { normalizeAngle } from "../../utils";
import type { UpdateContext, UpdateResult } from "../../game-engine/update-context";
import type { Monster } from "../monsters/monster";

const DEFAULT_FIRING_ANGLE_TOLERANCE = 0.08;

export interface TowerClass<T extends Tower = Tower> {
  new (x: number, y: number): T;
  readonly kind: TowerKind;
  readonly label: string;
  readonly summary: string;
  readonly baseCost: number;
  readonly baseRange: number;
  readonly shortcuts: readonly string[];
}

export abstract class Tower {
  kind: TowerKind;
  x: number;
  y: number;
  range: number;
  cost: number;
  currentTarget?: Monster;
  level = 0;
  cooldownSeconds = 0;
  removed = false;

  constructor(x: number, y: number) {
    // Reading this.towerClass invokes the getter below, which reflects static metadata from the concrete tower constructor.
    const towerClass = this.towerClass;
    this.kind = towerClass.kind;
    this.x = x;
    this.y = y;
    this.range = towerClass.baseRange;
    this.cost = towerClass.baseCost;
  }

  get towerClass(): TowerClass {
    return this.constructor as TowerClass;
  }

  get upgradeCost(): number {
    return UPGRADE_COST;
  }

  get resaleValue(): number {
    return Math.round(this.cost * 0.75);
  }

  canUpgrade(): boolean {
    return this.level < MAX_TOWER_LEVEL;
  }

  update(context: UpdateContext, result: UpdateResult): void {
    if (this.cooldownSeconds > 0) {
      this.cooldownSeconds -= context.deltaSeconds;
    }
    this.updateTower(context, result);
    // Keep overshoot for a shot fired this step, but never bank idle shots.
    this.cooldownSeconds = Math.max(0, this.cooldownSeconds);
  }

  upgrade(): void {
    if (!this.canUpgrade()) {
      return;
    }
    this.level += 1;
    this.cost += UPGRADE_COST;
    this.range += this.level * TOWER_RANGE_UPGRADE_STEP;
    this.onUpgrade();
  }

  protected findTrackedMonsterInContext(context: UpdateContext): Monster | undefined {
    if (this.currentTarget && this.canTrackMonster(this.currentTarget)) {
      return this.currentTarget;
    }

    this.currentTarget = this.findClosestMonsterInContext(context);
    return this.currentTarget;
  }

  protected findClosestMonsterInContext(context: UpdateContext): Monster | undefined {
    let closest: Monster | undefined;
    let smallestDistanceSquared = Number.POSITIVE_INFINITY;

    for (const monster of context.activeMonsters) {
      if (!this.isMonsterActive(monster)) {
        continue;
      }
      const distanceSquared = this.getDistanceSquaredInRange(monster);
      if (distanceSquared === null) {
        continue;
      }

      if (distanceSquared < smallestDistanceSquared) {
        smallestDistanceSquared = distanceSquared;
        closest = monster;
      }
    }
    return closest;
  }

  protected canTrackMonster(monster: Monster): boolean {
    return this.isMonsterActive(monster) && this.getDistanceSquaredInRange(monster) !== null;
  }

  protected isMonsterActive(monster: Monster): boolean {
    return !monster.removed && monster.hitPoints > 0;
  }

  private getDistanceSquaredInRange(monster: Monster): number | null {
    const dx = monster.x - this.x;
    if (Math.abs(dx) > this.range) {
      return null;
    }

    const dy = monster.y - this.y;
    if (Math.abs(dy) > this.range) {
      return null;
    }

    const distanceSquared = (dx * dx) + (dy * dy);
    if (distanceSquared > this.range * this.range) {
      return null;
    }

    return distanceSquared;
  }

  protected resetCooldown(seconds: number): void {
    this.cooldownSeconds += seconds;
  }

  protected ready(): boolean {
    return this.cooldownSeconds <= TIMER_EPSILON_SECONDS;
  }

  protected isAimedAtTarget(currentAngle: number, targetAngle: number, tolerance = DEFAULT_FIRING_ANGLE_TOLERANCE): boolean {
    return Math.abs(normalizeAngle(targetAngle - currentAngle)) <= tolerance;
  }

  protected abstract updateTower(context: UpdateContext, result: UpdateResult): void;
  protected onUpgrade(): void {
  }
}

import type { LightningLinkEffect } from "./entities/effects/lightning-link-effect";
import type { LinkEffect } from "./entities/effects/link-effect";
import type { Particle } from "./entities/effects/particle";
import type { Monster } from "./entities/monsters/monster";
import type { Drone } from "./entities/projectiles/drone";
import type { Missile } from "./entities/projectiles/missile";
import type { Projectile } from "./entities/projectiles/projectile";
import type { Tower } from "./entities/towers/tower";
import { createRouteMotionPath, type RouteMotionPath } from "./route-path";
import type { LevelData, Point, TowerKind, WaveData } from "./types";
import { compactInPlace } from "./utils";

export type RuntimeLinkEffect = LinkEffect | LightningLinkEffect;

export class LevelRuntime {
  readonly level?: LevelData;
  readonly routePath?: RouteMotionPath;
  money: number;
  escapesLeft: number;
  spawnDelay: number;
  spawnCooldown: number;
  spawnIndex = 0;
  spawnedMonsters = 0;
  currentWaveIndex = 0;
  waveSpawnedMonsters = 0;
  towers: Tower[] = [];
  monsters: Monster[] = [];
  drones: Drone[] = [];
  projectiles: Projectile[] = [];
  missiles: Missile[] = [];
  particles: Particle[] = [];
  links: RuntimeLinkEffect[] = [];
  selectedTower?: Tower;
  placingTower?: TowerKind;
  pointer?: Point;
  winDelay = 0;

  constructor();
  constructor(level: LevelData, roadTurnRadius: number, routeCurveSampleStep: number);
  constructor(level?: LevelData, roadTurnRadius?: number, routeCurveSampleStep?: number) {
    this.level = level;
    if (!level) {
      this.routePath = undefined;
    } else {
      if (roadTurnRadius === undefined || routeCurveSampleStep === undefined) {
        throw new Error("Route geometry is required when creating a level runtime.");
      }
      this.routePath = createRouteMotionPath(level.points, roadTurnRadius, routeCurveSampleStep);
    }
    this.money = level?.startingMoney ?? 0;
    this.escapesLeft = level?.allowEscape ?? 0;
    this.spawnDelay = level?.waves[0]?.buildTime ?? 0;
    this.spawnCooldown = level ? 0.2 : 0;
  }

  get activeWave(): WaveData | undefined {
    return this.level?.waves[this.currentWaveIndex];
  }

  get waveTotal(): number {
    return this.level?.waves.length ?? 0;
  }

  *getActiveMonsters(): IterableIterator<Monster> {
    for (const monster of this.monsters) {
      if (!monster.removed) {
        yield monster;
      }
    }
  }

  compactRemoved(): void {
    compactInPlace(this.monsters);
    compactInPlace(this.drones);
    compactInPlace(this.projectiles);
    compactInPlace(this.missiles);
    compactInPlace(this.particles);
    compactInPlace(this.links);
  }
}

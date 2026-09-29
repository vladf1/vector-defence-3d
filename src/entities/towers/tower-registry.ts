import { TowerKind } from "../../types";
import { DroneTower } from "./drone-tower";
import { GunTower } from "./gun-tower";
import { LaserTower } from "./laser-tower";
import { LightningTower } from "./lightning-tower";
import { MissileTower } from "./missile-tower";
import { SlowTower } from "./slow-tower";
import type { TowerClass } from "./tower";

/** Registration order is the toolbar order. */
export const TOWER_CLASSES: readonly TowerClass[] = [
  GunTower,
  LaserTower,
  MissileTower,
  SlowTower,
  DroneTower,
  LightningTower,
];

const TOWER_CLASS_BY_KIND = TOWER_CLASSES.reduce<Record<TowerKind, TowerClass>>((classes, towerClass) => {
  classes[towerClass.kind] = towerClass;
  return classes;
}, {} as Record<TowerKind, TowerClass>);

export function getTowerClass(kind: TowerKind): TowerClass {
  return TOWER_CLASS_BY_KIND[kind];
}

export function findTowerShortcut(key: string, availableTowers: readonly TowerKind[]): TowerKind | undefined {
  const normalizedKey = key.toLowerCase();
  for (const towerClass of TOWER_CLASSES) {
    if (availableTowers.includes(towerClass.kind) && (towerClass.shortcuts as readonly string[]).includes(normalizedKey)) {
      return towerClass.kind;
    }
  }
  return undefined;
}

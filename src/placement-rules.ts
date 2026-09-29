import type { Tower } from "./entities/towers/tower";
import type { RouteMotionPath } from "./route-path";
import type { FieldBounds, Point } from "./types";
import { isWithinDistanceToSegment, withinDistance } from "./utils";

export interface PlacementGeometry {
  bounds: FieldBounds;
  towerRadius: number;
  towerSelectionPadding: number;
  minDistanceToOtherTowers: number;
  minDistanceToRoad: number;
}

export function canPlaceTower(
  point: Point,
  routePath: RouteMotionPath | undefined,
  towers: readonly Tower[],
  geometry: PlacementGeometry,
): boolean {
  if (!routePath) {
    return false;
  }

  const { minX, minY, maxX, maxY } = geometry.bounds;
  const outsideBounds =
    point.x < minX + geometry.towerRadius ||
    point.y < minY + geometry.towerRadius ||
    point.x > maxX - geometry.towerRadius ||
    point.y > maxY - geometry.towerRadius;

  if (outsideBounds) {
    return false;
  }

  for (const tower of towers) {
    if (withinDistance(point, tower, geometry.minDistanceToOtherTowers)) {
      return false;
    }
  }

  for (let index = 0; index < routePath.entries.length - 1; index += 1) {
    const start = routePath.entries[index];
    const end = routePath.entries[index + 1];
    if (isWithinDistanceToSegment(point, start, end, geometry.minDistanceToRoad)) {
      return false;
    }
  }

  return true;
}

export function findTowerAtPoint(
  point: Point,
  towers: readonly Tower[],
  towerRadius: number,
  towerSelectionPadding: number,
): Tower | undefined {
  for (let index = towers.length - 1; index >= 0; index -= 1) {
    const tower = towers[index];
    if (withinDistance(point, tower, towerRadius + towerSelectionPadding)) {
      return tower;
    }
  }

  return undefined;
}

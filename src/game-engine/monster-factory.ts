import { PackManMonster } from "../entities/monsters/packman-monster";
import { BerserkerMonster } from "../entities/monsters/berserker-monster";
import { BulwarkMonster } from "../entities/monsters/bulwark-monster";
import type { Monster } from "../entities/monsters/monster";
import { RunnerMonster } from "../entities/monsters/runner-monster";
import { SplitterMonster } from "../entities/monsters/splitter-monster";
import { SquareMonster } from "../entities/monsters/square-monster";
import { TankMonster } from "../entities/monsters/tank-monster";
import { TriangleMonster } from "../entities/monsters/triangle-monster";
import { createPathEntriesFromDistance, type PathEntry } from "../route-path";
import { MonsterKind } from "../types";
import { randomRange } from "../utils";

const MIN_SPLITTER_CHILD_OFFSET_DISTANCE = 10;
const MAX_SPLITTER_CHILD_OFFSET_DISTANCE = 18;
const HIT_POINT_BONUS_PER_LEVEL = 0.06;

export function createMonster(kind: MonsterKind, path: PathEntry[], speedScale: number, levelIndex: number): Monster {
  const monster = createBaseMonster(kind, path, speedScale);
  const levelHitPointMultiplier = getLevelHitPointMultiplier(levelIndex);
  monster.hitPoints *= levelHitPointMultiplier;
  monster.maxHitPoints *= levelHitPointMultiplier;
  return monster;
}

export function createSplitterChildren(monster: Monster, speedScale: number, levelIndex: number): Monster[] {
  const children: Monster[] = [];
  const childCount = 2;
  const splitAngle = monster.angle;
  const pathLength = monster.path[monster.path.length - 1]?.totalDistance ?? 0;
  const minSpeedMultiplier = 0.89;
  const maxSpeedMultiplier = 0.97;

  for (let index = 0; index < childCount; index += 1) {
    const spawnDistance = monster.distanceAlongPath + createSplitterChildPathOffset(monster.distanceAlongPath, pathLength, index);
    const childPath = createPathEntriesFromDistance(monster.path, spawnDistance);
    const child = createMonster(MonsterKind.Runner, childPath, speedScale, levelIndex);
    const speedMultiplier = randomRange(minSpeedMultiplier, maxSpeedMultiplier);
    child.angle = splitAngle + randomRange(-0.12, 0.12);
    child.maxSpeedPerSecond *= speedMultiplier;
    child.speedPerSecond = child.maxSpeedPerSecond;
    child.velocityXPerSecond = Math.cos(child.angle) * child.speedPerSecond;
    child.velocityYPerSecond = Math.sin(child.angle) * child.speedPerSecond;
    child.hitPoints = Math.round(child.maxHitPoints * 0.72);
    child.maxHitPoints = child.hitPoints;
    child.bounty = Math.max(1, Math.round(child.bounty * 0.55));
    child.radius *= 0.86;
    children.push(child);
  }

  return children;
}

function createSplitterChildPathOffset(distanceAlongPath: number, pathLength: number, childIndex: number): number {
  const preferredDirection = childIndex % 2 === 0 ? -1 : 1;
  return createRandomOffsetInDirection(distanceAlongPath, pathLength, preferredDirection)
    ?? createRandomOffsetInDirection(distanceAlongPath, pathLength, -preferredDirection)
    ?? 0;
}

function createRandomOffsetInDirection(distanceAlongPath: number, pathLength: number, direction: number): number | undefined {
  const availableDistance = direction < 0
    ? distanceAlongPath
    : Math.max(0, pathLength - distanceAlongPath);

  if (availableDistance < MIN_SPLITTER_CHILD_OFFSET_DISTANCE) {
    return undefined;
  }

  const offsetDistance = randomRange(
    MIN_SPLITTER_CHILD_OFFSET_DISTANCE,
    Math.min(MAX_SPLITTER_CHILD_OFFSET_DISTANCE, availableDistance),
  );
  return offsetDistance * direction;
}

function createBaseMonster(kind: MonsterKind, path: PathEntry[], speedScale: number): Monster {
  switch (kind) {
    case MonsterKind.PackMan:
      return new PackManMonster(path, speedScale);
    case MonsterKind.Berserker:
      return new BerserkerMonster(path, speedScale);
    case MonsterKind.Bulwark:
      return new BulwarkMonster(path, speedScale);
    case MonsterKind.Square:
      return new SquareMonster(path, speedScale);
    case MonsterKind.Triangle:
      return new TriangleMonster(path, speedScale);
    case MonsterKind.Tank:
      return new TankMonster(path, speedScale);
    case MonsterKind.Splitter:
      return new SplitterMonster(path, speedScale);
    default:
      return new RunnerMonster(path, speedScale);
  }
}

function getLevelHitPointMultiplier(levelIndex: number): number {
  const levelOffset = Math.max(0, levelIndex);
  return 1 + (levelOffset * HIT_POINT_BONUS_PER_LEVEL);
}

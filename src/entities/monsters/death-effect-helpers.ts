import { GlassShardParticle } from "../effects/glass-shard-particle";
import type { UpdateResult } from "../../game-engine/update-context";
import type { Point } from "../../types";
import { randomRange } from "../../utils";
import type { PolygonShardSplitter } from "./polygon-shard-splitter";

export function rotatePoint(point: Point, angle: number): Point {
  return {
    x: point.x * Math.cos(angle) - point.y * Math.sin(angle),
    y: point.x * Math.sin(angle) + point.y * Math.cos(angle),
  };
}

export function pointOnRadius(angle: number, radius: number): Point {
  return {
    x: Math.cos(angle) * radius,
    y: Math.sin(angle) * radius,
  };
}

export function createDeathEffectOrigin(
  radius: number,
  xMinRatio: number,
  xMaxRatio: number,
  yMinRatio: number,
  yMaxRatio: number,
): Point {
  return {
    x: randomRange(radius * xMinRatio, radius * xMaxRatio),
    y: randomRange(radius * yMinRatio, radius * yMaxRatio),
  };
}

export function createPolygonShardParticles(
  result: UpdateResult,
  source: { visualX: number; visualY: number; color: string },
  outline: readonly Point[],
  origin: Point,
  rotation: number,
  speedMinPerSecond: number,
  speedMaxPerSecond: number,
  initialSeparation: number,
  splitter: PolygonShardSplitter,
): void {
  if (result.remainingParticleCapacity === 0) {
    return;
  }
  for (const shard of splitter.splitIntoShards(outline)) {
    if (result.remainingParticleCapacity === 0) {
      break;
    }
    result.addParticle(new GlassShardParticle(
      source.visualX,
      source.visualY,
      source.color,
      shard.vertices,
      origin,
      rotation,
      randomRange(speedMinPerSecond, speedMaxPerSecond),
      initialSeparation,
    ));
  }
}

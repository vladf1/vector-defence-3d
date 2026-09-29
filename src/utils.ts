import type { FieldBounds, Point } from "./types";

interface MovingPoint extends Point {
  velocityXPerSecond: number;
  velocityYPerSecond: number;
}

/** Predicts the earliest reachable intercept; aim at the current position if none exists. */
export function calculateIntercept(target: MovingPoint, projectileSpeedPerSecond: number, from: Point): Point {
  const dx = target.x - from.x;
  const dy = target.y - from.y;
  const a = projectileSpeedPerSecond ** 2 - target.velocityXPerSecond ** 2 - target.velocityYPerSecond ** 2;
  const b = dx * target.velocityXPerSecond + dy * target.velocityYPerSecond;
  const c = dx * dx + dy * dy;
  const discriminant = b * b + a * c;
  // Rationalizing the smaller positive root also handles equal projectile/target speeds.
  const denominator = discriminant >= 0 ? Math.sqrt(discriminant) - b : 0;
  const timeSeconds = denominator > 0 ? c / denominator : 0;
  return {
    x: target.x + target.velocityXPerSecond * timeSeconds,
    y: target.y + target.velocityYPerSecond * timeSeconds,
  };
}

/**
 * Converts legacy per-update linear slowdown into continuous decay while
 * preserving its velocity and displacement at the chosen reference rate.
 */
export class CalibratedExponentialDecay {
  private readonly decayRatePerSecond: number;
  private readonly displacementScale: number;
  private cachedDeltaSeconds = Number.NaN;
  private cachedDisplacementSeconds = 0;
  private cachedVelocityFactor = 1;

  constructor(linearSlowdownPerSecond: number, referenceUpdatesPerSecond: number) {
    if (
      !Number.isFinite(linearSlowdownPerSecond)
      || !Number.isFinite(referenceUpdatesPerSecond)
      || linearSlowdownPerSecond < 0
      || referenceUpdatesPerSecond <= 0
      || linearSlowdownPerSecond >= referenceUpdatesPerSecond
    ) {
      throw new RangeError("Velocity decay requires 0 <= slowdown < reference update rate.");
    }

    if (linearSlowdownPerSecond === 0) {
      this.decayRatePerSecond = 0;
      this.displacementScale = 1;
      return;
    }

    const referenceDeltaSeconds = 1 / referenceUpdatesPerSecond;
    const referenceVelocityFactor = 1 - (linearSlowdownPerSecond * referenceDeltaSeconds);
    this.decayRatePerSecond = -Math.log(referenceVelocityFactor) * referenceUpdatesPerSecond;
    this.displacementScale = (
      referenceVelocityFactor
      * referenceDeltaSeconds
      * this.decayRatePerSecond
    ) / (1 - referenceVelocityFactor);
  }

  apply(target: MovingPoint, deltaSeconds: number): void {
    if (deltaSeconds <= 0) {
      return;
    }

    if (this.cachedDeltaSeconds !== deltaSeconds) {
      this.cachedDeltaSeconds = deltaSeconds;
      if (this.decayRatePerSecond === 0) {
        this.cachedVelocityFactor = 1;
        this.cachedDisplacementSeconds = deltaSeconds;
      } else {
        this.cachedVelocityFactor = Math.exp(-this.decayRatePerSecond * deltaSeconds);
        this.cachedDisplacementSeconds = (
          this.displacementScale
          * (1 - this.cachedVelocityFactor)
        ) / this.decayRatePerSecond;
      }
    }

    const velocityXPerSecond = target.velocityXPerSecond;
    const velocityYPerSecond = target.velocityYPerSecond;
    target.x += velocityXPerSecond * this.cachedDisplacementSeconds;
    target.y += velocityYPerSecond * this.cachedDisplacementSeconds;
    target.velocityXPerSecond = velocityXPerSecond * this.cachedVelocityFactor;
    target.velocityYPerSecond = velocityYPerSecond * this.cachedVelocityFactor;
  }
}

export function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

export function easeInOutCubic(progress: number): number {
  // Progress is normalized from 0 to 1.
  const clamped = clamp(progress, 0, 1);
  return clamped < 0.5
    ? 4 * clamped * clamped * clamped
    : 1 - (Math.pow((-2 * clamped) + 2, 3) / 2);
}

export function easeInOutSine(progress: number): number {
  // Progress is normalized from 0 to 1.
  const clamped = clamp(progress, 0, 1);
  return -(Math.cos(Math.PI * clamped) - 1) / 2;
}

export function easeOutCubic(progress: number): number {
  // Progress is normalized from 0 to 1.
  const clamped = clamp(progress, 0, 1);
  return 1 - Math.pow(1 - clamped, 3);
}

export function calculateDistance(source: Point, target: Point): number {
  return Math.hypot(source.x - target.x, source.y - target.y);
}

export function withinDistance(source: Point, target: Point, maxDistance: number): boolean {
  const dx = target.x - source.x;
  if (Math.abs(dx) > maxDistance) {
    return false;
  }

  const dy = target.y - source.y;
  if (Math.abs(dy) > maxDistance) {
    return false;
  }

  return (dx * dx) + (dy * dy) <= (maxDistance * maxDistance);
}

export function isOutsideBounds(point: Point, bounds: FieldBounds, margin: number): boolean {
  return point.x < bounds.minX - margin || point.y < bounds.minY - margin
    || point.x > bounds.maxX + margin || point.y > bounds.maxY + margin;
}

export function angleBetween(source: Point, target: Point): number {
  return Math.atan2(target.y - source.y, target.x - source.x);
}

export function normalizeAngle(angle: number): number {
  return Math.atan2(Math.sin(angle), Math.cos(angle));
}

export function turnAngleTowards(current: number, target: number, maxStep: number): number {
  const delta = normalizeAngle(target - current);
  const step = clamp(delta, -maxStep, maxStep);
  return normalizeAngle(current + step);
}

export function closestPointOnSegment(point: Point, start: Point, end: Point): Point {
  const segmentX = end.x - start.x;
  const segmentY = end.y - start.y;
  const segmentLengthSquared = (segmentX * segmentX) + (segmentY * segmentY);
  if (segmentLengthSquared === 0) {
    return start;
  }

  const projection = projectPointOntoSegment(point, start, segmentX, segmentY, segmentLengthSquared);
  return {
    x: start.x + (projection * segmentX),
    y: start.y + (projection * segmentY),
  };
}

export function isWithinDistanceToSegment(point: Point, start: Point, end: Point, maxDistance: number): boolean {
  if (
    point.x < Math.min(start.x, end.x) - maxDistance ||
    point.x > Math.max(start.x, end.x) + maxDistance ||
    point.y < Math.min(start.y, end.y) - maxDistance ||
    point.y > Math.max(start.y, end.y) + maxDistance
  ) {
    return false;
  }

  const segmentX = end.x - start.x;
  const segmentY = end.y - start.y;
  const segmentLengthSquared = (segmentX * segmentX) + (segmentY * segmentY);
  if (segmentLengthSquared === 0) {
    return withinDistance(point, start, maxDistance);
  }

  const projection = projectPointOntoSegment(point, start, segmentX, segmentY, segmentLengthSquared);
  const closestX = start.x + (projection * segmentX);
  const closestY = start.y + (projection * segmentY);
  const distanceX = closestX - point.x;
  const distanceY = closestY - point.y;
  return (distanceX * distanceX) + (distanceY * distanceY) <= (maxDistance * maxDistance);
}

function projectPointOntoSegment(
  point: Point,
  start: Point,
  segmentX: number,
  segmentY: number,
  segmentLengthSquared: number,
): number {
  const dotProduct = ((point.x - start.x) * segmentX) + ((point.y - start.y) * segmentY);
  return clamp(dotProduct / segmentLengthSquared, 0, 1);
}

export function randomRange(min: number, max: number): number {
  return min + (Math.random() * (max - min));
}

export function formatMoney(value: number): string {
  return `$${Math.round(value)}`;
}

export function compactInPlace<T extends { removed: boolean }>(items: T[]): void {
  const itemCount = items.length;
  let writeIndex = 0;

  while (writeIndex < itemCount && !items[writeIndex].removed) {
    writeIndex += 1;
  }

  if (writeIndex === itemCount) {
    return;
  }

  for (let readIndex = writeIndex + 1; readIndex < itemCount; readIndex += 1) {
    const item = items[readIndex];
    if (!item.removed) {
      items[writeIndex] = item;
      writeIndex += 1;
    }
  }
  items.length = writeIndex;
}

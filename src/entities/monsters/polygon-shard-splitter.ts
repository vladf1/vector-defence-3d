import type { Point } from "../../types";

export interface PolygonShardSplitterConfig {
  minShardCount: number;
  maxShardCount: number;
  maxConsecutiveSplitFailures: number;
  crackAttemptsPerShard: number;
  minBoundarySeparationRatio: number;
  boundaryEndpointInsetRatio: number;
  minShardAreaRatio: number;
  maxShardAreaRatio: number;
  maxSplitChildAreaRatio: number;
  maxShardVertices: number;
  kinkOffsetRatio: number;
  pointMergeDistance: number;
  areaToleranceRatio: number;
  random: () => number;
}

export type PolygonShardSplitterConfigOptions = {
  minShardCount: number;
  maxShardCount: number;
  random?: () => number;
} & Partial<Omit<
  PolygonShardSplitterConfig,
  "minShardCount" | "maxShardCount" | "random"
>>;

export interface Shard {
  vertices: Point[];
}

interface WorkingShard extends Shard {
  area: number;
}

interface BoundarySample {
  point: Point;
  edgeIndex: number;
  distance: number;
  ratio: number;
}

interface EdgeMetric {
  length: number;
  distanceAtStart: number;
}

export function createPolygonShardSplitterConfig(
  options: PolygonShardSplitterConfigOptions,
): PolygonShardSplitterConfig {
  return {
    minShardCount: options.minShardCount,
    maxShardCount: options.maxShardCount,
    maxConsecutiveSplitFailures: options.maxConsecutiveSplitFailures ?? 120,
    crackAttemptsPerShard: options.crackAttemptsPerShard ?? 12,
    minBoundarySeparationRatio: options.minBoundarySeparationRatio ?? 0.2,
    boundaryEndpointInsetRatio: options.boundaryEndpointInsetRatio ?? 0.055,
    minShardAreaRatio: options.minShardAreaRatio ?? 0.034,
    maxShardAreaRatio: options.maxShardAreaRatio ?? 0.56,
    maxSplitChildAreaRatio: options.maxSplitChildAreaRatio ?? 0.72,
    maxShardVertices: options.maxShardVertices ?? 11,
    kinkOffsetRatio: options.kinkOffsetRatio ?? 0.18,
    pointMergeDistance: options.pointMergeDistance ?? 0.025,
    areaToleranceRatio: options.areaToleranceRatio ?? 0.035,
    random: options.random ?? Math.random,
  };
}

export function createPolygonShardSplitter(
  options: PolygonShardSplitterConfigOptions,
): PolygonShardSplitter {
  return new PolygonShardSplitter(createPolygonShardSplitterConfig(options));
}

export class PolygonShardSplitter {
  constructor(private readonly config: PolygonShardSplitterConfig) {}

  splitIntoShards(outline: readonly Point[]): Shard[] {
    const sourcePolygon = outline.map(clonePoint);
    if (sourcePolygon.length < 3) {
      return [];
    }

    const sourceArea = polygonArea(sourcePolygon);
    if (sourceArea === 0) {
      return [];
    }

    const targetShardCount = randomInteger(
      this.config.minShardCount,
      this.config.maxShardCount,
      this.config.random,
    );
    const shards: WorkingShard[] = [{ vertices: sourcePolygon, area: sourceArea }];
    let consecutiveFailures = 0;

    while (
      (
        shards.length < targetShardCount ||
        maxShardAreaRatio(shards, sourceArea) > this.config.maxShardAreaRatio
      ) &&
      shards.length < this.config.maxShardCount &&
      consecutiveFailures < this.config.maxConsecutiveSplitFailures
    ) {
      const shardIndex = this.chooseShardIndex(shards, sourceArea);
      if (shardIndex === -1) {
        break;
      }

      const split = this.trySplitShard(shards[shardIndex], sourceArea);
      if (split === null) {
        consecutiveFailures += 1;
        continue;
      }

      shards.splice(shardIndex, 1, split[0], split[1]);
      consecutiveFailures = 0;
    }

    // Split construction owns these points; shard consumers treat them as immutable.
    return shards.map((shard) => ({
      vertices: shard.vertices,
    }));
  }

  private chooseShardIndex(shards: WorkingShard[], sourceArea: number): number {
    const minCandidateArea = sourceArea * this.config.minShardAreaRatio * 2.15;
    const oversizedArea = sourceArea * this.config.maxShardAreaRatio;
    let hasOversizedCandidate = false;
    let totalWeight = 0;
    let lastCandidateIndex = -1;

    for (let index = 0; index < shards.length; index += 1) {
      const area = shards[index].area;
      if (area < minCandidateArea) {
        continue;
      }

      const isOversized = area > oversizedArea;
      if (isOversized && !hasOversizedCandidate) {
        hasOversizedCandidate = true;
        totalWeight = 0;
        lastCandidateIndex = -1;
      }
      if (hasOversizedCandidate && !isOversized) {
        continue;
      }

      totalWeight += area * area;
      lastCandidateIndex = index;
    }

    if (lastCandidateIndex === -1) {
      return -1;
    }

    let threshold = this.config.random() * totalWeight;
    for (let index = 0; index < shards.length; index += 1) {
      const area = shards[index].area;
      if (
        area < minCandidateArea ||
        (hasOversizedCandidate && area <= oversizedArea)
      ) {
        continue;
      }

      threshold -= area * area;
      if (threshold <= 0) {
        return index;
      }
    }

    return lastCandidateIndex;
  }

  private trySplitShard(
    shard: WorkingShard,
    sourceArea: number,
  ): [WorkingShard, WorkingShard] | null {
    const polygon = shard.vertices;
    const edgeMetrics = createEdgeMetrics(polygon);
    if (edgeMetrics.perimeter === 0) {
      return null;
    }

    for (let attempt = 0; attempt < this.config.crackAttemptsPerShard; attempt += 1) {
      const boundaryPair = this.sampleBoundaryPair(polygon, edgeMetrics);
      if (boundaryPair === null) {
        continue;
      }

      const kinkPoint = this.createKinkPoint(
        boundaryPair.start.point,
        boundaryPair.end.point,
      );
      const kinkedSplit = this.tryCrackPath(
        shard,
        sourceArea,
        boundaryPair,
        kinkPoint,
      );
      if (kinkedSplit !== null) {
        return kinkedSplit;
      }

      const straightSplit = this.tryCrackPath(
        shard,
        sourceArea,
        boundaryPair,
        null,
      );
      if (straightSplit !== null) {
        return straightSplit;
      }
    }

    return null;
  }

  private tryCrackPath(
    shard: WorkingShard,
    sourceArea: number,
    boundaryPair: { start: BoundarySample; end: BoundarySample },
    kinkPoint: Point | null,
  ): [WorkingShard, WorkingShard] | null {
    const polygon = shard.vertices;
    if (!isValidCrackPath(
      polygon,
      boundaryPair.start,
      boundaryPair.end,
      kinkPoint,
      this.config.pointMergeDistance,
    )) {
      return null;
    }

    const split = splitPolygonWithCrack(
      polygon,
      boundaryPair.start,
      boundaryPair.end,
      kinkPoint,
    );
    if (split === null) {
      return null;
    }

    const firstArea = polygonArea(split[0]);
    const secondArea = polygonArea(split[1]);
    const areaDeltaRatio = Math.abs(firstArea + secondArea - shard.area) / shard.area;
    if (areaDeltaRatio > this.config.areaToleranceRatio) {
      return null;
    }

    const largestChildAreaRatio = Math.max(firstArea, secondArea) / shard.area;
    if (largestChildAreaRatio > this.config.maxSplitChildAreaRatio) {
      return null;
    }

    if (
      !isReadableShard(split[0], firstArea, sourceArea, this.config) ||
      !isReadableShard(split[1], secondArea, sourceArea, this.config)
    ) {
      return null;
    }

    return [
      { vertices: split[0], area: firstArea },
      { vertices: split[1], area: secondArea },
    ];
  }

  private sampleBoundaryPair(
    polygon: Point[],
    edgeMetrics: { edges: EdgeMetric[]; perimeter: number },
  ): { start: BoundarySample; end: BoundarySample } | null {
    const maxAttempts = this.config.crackAttemptsPerShard * 3;
    for (let attempt = 0; attempt < maxAttempts; attempt += 1) {
      const start = sampleBoundaryAtDistance(
        polygon,
        edgeMetrics.edges,
        this.config.random() * edgeMetrics.perimeter,
      );
      const end = sampleBoundaryAtDistance(
        polygon,
        edgeMetrics.edges,
        this.config.random() * edgeMetrics.perimeter,
      );
      if (start.edgeIndex === end.edgeIndex) {
        continue;
      }
      if (
        start.ratio < this.config.boundaryEndpointInsetRatio ||
        start.ratio > 1 - this.config.boundaryEndpointInsetRatio ||
        end.ratio < this.config.boundaryEndpointInsetRatio ||
        end.ratio > 1 - this.config.boundaryEndpointInsetRatio
      ) {
        continue;
      }

      const directDistance = Math.abs(start.distance - end.distance);
      const separation = Math.min(
        directDistance,
        edgeMetrics.perimeter - directDistance,
      );
      if (separation < edgeMetrics.perimeter * this.config.minBoundarySeparationRatio) {
        continue;
      }

      return { start, end };
    }

    return null;
  }

  private createKinkPoint(start: Point, end: Point): Point {
    const dx = end.x - start.x;
    const dy = end.y - start.y;
    const length = Math.hypot(dx, dy);
    const normalX = length === 0 ? 0 : -dy / length;
    const normalY = length === 0 ? 0 : dx / length;
    const ratio = 0.5 + ((this.config.random() - 0.5) * 0.18);
    const offset = (this.config.random() - 0.5) * length * this.config.kinkOffsetRatio;
    return {
      x: start.x + (dx * ratio) + (normalX * offset),
      y: start.y + (dy * ratio) + (normalY * offset),
    };
  }
}

function isValidCrackPath(
  polygon: Point[],
  start: BoundarySample,
  end: BoundarySample,
  kinkPoint: Point | null,
  epsilon: number,
): boolean {
  if (kinkPoint === null) {
    return isValidCrackSegment(
      start.point,
      end.point,
      polygon,
      start.edgeIndex,
      end.edgeIndex,
      epsilon,
    );
  }

  return (
    pointInsidePolygon(kinkPoint.x, kinkPoint.y, polygon, epsilon) &&
    isValidCrackSegment(
      start.point,
      kinkPoint,
      polygon,
      start.edgeIndex,
      -1,
      epsilon,
    ) &&
    isValidCrackSegment(
      kinkPoint,
      end.point,
      polygon,
      -1,
      end.edgeIndex,
      epsilon,
    )
  );
}

function isValidCrackSegment(
  start: Point,
  end: Point,
  polygon: Point[],
  skippedEdgeIndexA: number,
  skippedEdgeIndexB: number,
  epsilon: number,
): boolean {
  if (!pointInsidePolygon(
    (start.x + end.x) / 2,
    (start.y + end.y) / 2,
    polygon,
    epsilon,
  )) {
    return false;
  }

  for (let edgeIndex = 0; edgeIndex < polygon.length; edgeIndex += 1) {
    if (edgeIndex === skippedEdgeIndexA || edgeIndex === skippedEdgeIndexB) {
      continue;
    }
    if (segmentsIntersect(
      start,
      end,
      polygon[edgeIndex],
      polygon[(edgeIndex + 1) % polygon.length],
      epsilon,
    )) {
      return false;
    }
  }

  return true;
}

function splitPolygonWithCrack(
  polygon: Point[],
  start: BoundarySample,
  end: BoundarySample,
  kinkPoint: Point | null,
): [Point[], Point[]] | null {
  const startToEndBoundary = boundaryChain(polygon, start, end);
  const endToStartBoundary = boundaryChain(polygon, end, start);
  const firstShard = startToEndBoundary;
  if (kinkPoint !== null) {
    firstShard.push(kinkPoint);
  }

  const secondShard = [start.point];
  if (kinkPoint !== null) {
    secondShard.push(kinkPoint);
  }
  secondShard.push(end.point);
  for (let index = 1; index < endToStartBoundary.length - 1; index += 1) {
    secondShard.push(endToStartBoundary[index]);
  }

  if (firstShard.length < 3 || secondShard.length < 3) {
    return null;
  }

  return [firstShard, secondShard];
}

function boundaryChain(
  polygon: Point[],
  start: BoundarySample,
  end: BoundarySample,
): Point[] {
  const chain = [start.point];
  let index = (start.edgeIndex + 1) % polygon.length;
  const stopIndex = (end.edgeIndex + 1) % polygon.length;
  let guard = 0;

  while (index !== stopIndex && guard <= polygon.length) {
    chain.push(polygon[index]);
    index = (index + 1) % polygon.length;
    guard += 1;
  }

  chain.push(end.point);
  return chain;
}

function createEdgeMetrics(
  polygon: Point[],
): { edges: EdgeMetric[]; perimeter: number } {
  const edges: EdgeMetric[] = [];
  let perimeter = 0;
  for (let index = 0; index < polygon.length; index += 1) {
    const start = polygon[index];
    const end = polygon[(index + 1) % polygon.length];
    const length = Math.hypot(end.x - start.x, end.y - start.y);
    edges.push({ length, distanceAtStart: perimeter });
    perimeter += length;
  }

  return { edges, perimeter };
}

function sampleBoundaryAtDistance(
  polygon: Point[],
  edgeMetrics: EdgeMetric[],
  distance: number,
): BoundarySample {
  for (let index = 0; index < edgeMetrics.length; index += 1) {
    const edge = edgeMetrics[index];
    if (distance <= edge.distanceAtStart + edge.length) {
      const start = polygon[index];
      const end = polygon[(index + 1) % polygon.length];
      const ratio = edge.length === 0
        ? 0
        : (distance - edge.distanceAtStart) / edge.length;
      return {
        point: {
          x: start.x + ((end.x - start.x) * ratio),
          y: start.y + ((end.y - start.y) * ratio),
        },
        edgeIndex: index,
        distance,
        ratio,
      };
    }
  }

  return {
    point: clonePoint(polygon[0]),
    edgeIndex: 0,
    distance: 0,
    ratio: 0,
  };
}

function isReadableShard(
  shard: Point[],
  area: number,
  sourceArea: number,
  config: PolygonShardSplitterConfig,
): boolean {
  return (
    shard.length >= 3 &&
    shard.length <= config.maxShardVertices &&
    area >= sourceArea * config.minShardAreaRatio
  );
}

function maxShardAreaRatio(shards: WorkingShard[], sourceArea: number): number {
  let maxArea = 0;
  for (const shard of shards) {
    maxArea = Math.max(maxArea, shard.area);
  }
  return maxArea / sourceArea;
}

function polygonArea(polygon: readonly Point[]): number {
  let total = 0;
  for (let index = 0; index < polygon.length; index += 1) {
    const current = polygon[index];
    const next = polygon[(index + 1) % polygon.length];
    total += (current.x * next.y) - (next.x * current.y);
  }
  return Math.abs(total) / 2;
}

function pointInsidePolygon(
  pointX: number,
  pointY: number,
  polygon: readonly Point[],
  epsilon: number,
): boolean {
  let inside = false;
  for (
    let index = 0, previousIndex = polygon.length - 1;
    index < polygon.length;
    previousIndex = index, index += 1
  ) {
    const current = polygon[index];
    const previous = polygon[previousIndex];
    if (pointOnSegmentCoordinates(pointX, pointY, previous, current, epsilon)) {
      return true;
    }
    if (
      (current.y > pointY) !== (previous.y > pointY) &&
      pointX < ((previous.x - current.x) * (pointY - current.y)) /
        (previous.y - current.y) + current.x
    ) {
      inside = !inside;
    }
  }

  return inside;
}

function segmentsIntersect(
  a: Point,
  b: Point,
  c: Point,
  d: Point,
  epsilon: number,
): boolean {
  const directionA = orientation(a, b, c);
  const directionB = orientation(a, b, d);
  const directionC = orientation(c, d, a);
  const directionD = orientation(c, d, b);

  if (Math.abs(directionA) <= epsilon && pointOnSegment(c, a, b, epsilon)) {
    return true;
  }
  if (Math.abs(directionB) <= epsilon && pointOnSegment(d, a, b, epsilon)) {
    return true;
  }
  if (Math.abs(directionC) <= epsilon && pointOnSegment(a, c, d, epsilon)) {
    return true;
  }
  if (Math.abs(directionD) <= epsilon && pointOnSegment(b, c, d, epsilon)) {
    return true;
  }

  return (
    (directionA > 0) !== (directionB > 0) &&
    (directionC > 0) !== (directionD > 0)
  );
}

function orientation(a: Point, b: Point, c: Point): number {
  return ((b.x - a.x) * (c.y - a.y)) - ((b.y - a.y) * (c.x - a.x));
}

function pointOnSegment(
  point: Point,
  start: Point,
  end: Point,
  epsilon: number,
): boolean {
  return (
    Math.abs(orientation(start, end, point)) <= epsilon &&
    point.x >= Math.min(start.x, end.x) - epsilon &&
    point.x <= Math.max(start.x, end.x) + epsilon &&
    point.y >= Math.min(start.y, end.y) - epsilon &&
    point.y <= Math.max(start.y, end.y) + epsilon
  );
}

function pointOnSegmentCoordinates(
  pointX: number,
  pointY: number,
  start: Point,
  end: Point,
  epsilon: number,
): boolean {
  return (
    Math.abs(
      ((end.x - start.x) * (pointY - start.y)) -
      ((end.y - start.y) * (pointX - start.x)),
    ) <= epsilon &&
    pointX >= Math.min(start.x, end.x) - epsilon &&
    pointX <= Math.max(start.x, end.x) + epsilon &&
    pointY >= Math.min(start.y, end.y) - epsilon &&
    pointY <= Math.max(start.y, end.y) + epsilon
  );
}

function randomInteger(
  minValue: number,
  maxValue: number,
  random: () => number,
): number {
  return minValue + Math.floor(random() * (maxValue - minValue + 1));
}

function clonePoint(point: Point): Point {
  return { x: point.x, y: point.y };
}

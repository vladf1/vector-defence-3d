import type { Point } from "./types";
import { angleBetween, calculateDistance } from "./utils";

export type RoutePathCommand =
  | { kind: "line"; point: Point }
  | { kind: "quadratic"; control: Point; point: Point };

export interface PathEntry extends Point {
  totalDistance: number;
  heading?: PathEntryHeading;
}

export interface RouteMotionPath {
  start: Point;
  commands: RoutePathCommand[];
  entries: PathEntry[];
}

interface RoundedTurn {
  index: number;
  entry: Point;
  control: Point;
  exit: Point;
}

type PathEntryHeading =
  | { kind: "line"; angle: number }
  | { kind: "quadratic"; start: Point; control: Point; end: Point; tStart: number; tEnd: number };

export function createRouteMotionPath(points: readonly Point[], roadTurnRadius: number, routeCurveSampleStep: number): RouteMotionPath {
  const start = points[0] ?? { x: 0, y: 0 };
  const entries: PathEntry[] = [{ x: start.x, y: start.y, totalDistance: 0 }];

  if (points.length < 2) {
    return {
      start,
      commands: [],
      entries,
    };
  }

  const turns = createRoundedTurns(points, roadTurnRadius);
  const commands: RoutePathCommand[] = [];
  let current = start;
  let turnIndex = 0;

  for (let segmentIndex = 0; segmentIndex < points.length - 1; segmentIndex += 1) {
    const nextTurn = turns[turnIndex];
    const segmentEnd = nextTurn?.index === segmentIndex + 1
      ? nextTurn.entry
      : points[segmentIndex + 1];

    appendLine(commands, entries, current, segmentEnd);
    current = segmentEnd;

    if (nextTurn?.index === segmentIndex + 1) {
      appendQuadratic(commands, entries, nextTurn.entry, nextTurn.control, nextTurn.exit, routeCurveSampleStep);
      current = nextTurn.exit;
      turnIndex += 1;
    }
  }

  return {
    start,
    commands,
    entries,
  };
}

export function createPathEntries(points: Point[]): PathEntry[] {
  const entries: PathEntry[] = [];
  let totalDistance = 0;

  for (let index = 0; index < points.length; index += 1) {
    const point = points[index];
    if (index > 0) {
      const previous = points[index - 1];
      totalDistance += calculateDistance(previous, point);
    }
    entries.push({
      x: point.x,
      y: point.y,
      totalDistance,
    });
  }

  return entries;
}

export function createPathEntriesFromDistance(source: readonly PathEntry[], startDistance: number): PathEntry[] {
  const pathLength = source[source.length - 1]?.totalDistance ?? 0;
  const clampedDistance = Math.min(Math.max(0, startDistance), pathLength);
  const spawnPoint = getPointAtDistance(source, clampedDistance);
  const entries: PathEntry[] = [{
    x: spawnPoint.x,
    y: spawnPoint.y,
    totalDistance: 0,
  }];

  const firstTailIndex = findPathIndexAtDistance(source, clampedDistance);
  if (firstTailIndex >= source.length) {
    return entries;
  }

  for (let index = firstTailIndex; index < source.length; index += 1) {
    const entry = source[index];
    const totalDistance = entry.totalDistance - clampedDistance;
    if (totalDistance <= 0 && calculateDistance(spawnPoint, entry) <= 0) {
      continue;
    }
    entries.push({
      x: entry.x,
      y: entry.y,
      totalDistance: Math.max(0, totalDistance),
      heading: index === firstTailIndex
        ? createTailHeading(source, entry, clampedDistance, firstTailIndex)
        : cloneHeading(entry.heading),
    });
  }

  return entries;
}

export function getPointAtDistance(source: readonly PathEntry[], distance: number): Point {
  if (source.length === 0) {
    return { x: 0, y: 0 };
  }

  const first = source[0];
  if (distance <= first.totalDistance) {
    return { x: first.x, y: first.y };
  }

  for (let index = 1; index < source.length; index += 1) {
    const end = source[index];
    if (distance > end.totalDistance) {
      continue;
    }

    const start = source[index - 1];
    const span = end.totalDistance - start.totalDistance;
    const ratio = span > 0 ? (distance - start.totalDistance) / span : 1;
    return {
      x: start.x + ((end.x - start.x) * ratio),
      y: start.y + ((end.y - start.y) * ratio),
    };
  }

  const last = source[source.length - 1];
  return { x: last.x, y: last.y };
}

function findPathIndexAtDistance(source: readonly PathEntry[], distance: number): number {
  for (let index = 0; index < source.length; index += 1) {
    if (source[index].totalDistance >= distance) {
      return index;
    }
  }

  return source.length;
}

export function getPathHeadingAngle(path: readonly PathEntry[], distance: number, targetIndex: number): number {
  if (path.length < 2) {
    return 0;
  }

  const endIndex = findLocalPathIndexAtDistance(path, distance, targetIndex);
  const start = path[endIndex - 1] ?? path[0];
  const end = path[endIndex] ?? path[path.length - 1];
  const heading = end.heading;
  if (!heading) {
    return angleBetween(start, end);
  }

  if (heading.kind === "line") {
    return heading.angle;
  }

  const span = end.totalDistance - start.totalDistance;
  const ratio = span > 0 ? (distance - start.totalDistance) / span : 1;
  const t = heading.tStart + ((heading.tEnd - heading.tStart) * ratio);
  return getQuadraticAngle(heading.start, heading.control, heading.end, t);
}

function createTailHeading(
  source: readonly PathEntry[],
  entry: PathEntry,
  clampedDistance: number,
  firstTailIndex: number,
): PathEntryHeading | undefined {
  const heading = entry.heading;
  if (!heading || heading.kind === "line") {
    return cloneHeading(heading);
  }

  const start = source[firstTailIndex - 1];
  const span = entry.totalDistance - (start?.totalDistance ?? entry.totalDistance);
  const ratio = span > 0 ? (clampedDistance - (start?.totalDistance ?? clampedDistance)) / span : 1;
  return {
    ...heading,
    tStart: heading.tStart + ((heading.tEnd - heading.tStart) * ratio),
  };
}

function cloneHeading(heading: PathEntryHeading | undefined): PathEntryHeading | undefined {
  if (!heading) {
    return undefined;
  }

  if (heading.kind === "line") {
    return { ...heading };
  }

  return {
    kind: "quadratic",
    start: { ...heading.start },
    control: { ...heading.control },
    end: { ...heading.end },
    tStart: heading.tStart,
    tEnd: heading.tEnd,
  };
}

function findLocalPathIndexAtDistance(path: readonly PathEntry[], distance: number, targetIndex: number): number {
  let index = Math.min(Math.max(1, targetIndex), path.length - 1);
  while (index > 1 && path[index - 1].totalDistance >= distance) {
    index -= 1;
  }
  while (index < path.length - 1 && path[index].totalDistance < distance) {
    index += 1;
  }
  return index;
}

function createRoundedTurns(points: readonly Point[], roadTurnRadius: number): RoundedTurn[] {
  const turns: RoundedTurn[] = [];

  for (let index = 1; index < points.length - 1; index += 1) {
    const previous = points[index - 1];
    const control = points[index];
    const next = points[index + 1];
    const previousLength = calculateDistance(previous, control);
    const nextLength = calculateDistance(control, next);
    const turnDistance = Math.min(roadTurnRadius, previousLength * 0.45, nextLength * 0.45);

    if (turnDistance <= 1) {
      continue;
    }

    turns.push({
      index,
      entry: pointToward(control, previous, turnDistance),
      control,
      exit: pointToward(control, next, turnDistance),
    });
  }

  return turns;
}

function pointToward(source: Point, target: Point, distance: number): Point {
  const total = calculateDistance(source, target);
  if (total === 0) {
    return { x: source.x, y: source.y };
  }

  const ratio = distance / total;
  return {
    x: source.x + ((target.x - source.x) * ratio),
    y: source.y + ((target.y - source.y) * ratio),
  };
}

function appendLine(commands: RoutePathCommand[], entries: PathEntry[], start: Point, end: Point): void {
  const length = calculateDistance(start, end);
  if (length <= 0) {
    return;
  }

  commands.push({ kind: "line", point: end });
  appendPoint(entries, end, {
    kind: "line",
    angle: angleBetween(start, end),
  });
}

function appendQuadratic(
  commands: RoutePathCommand[],
  entries: PathEntry[],
  start: Point,
  control: Point,
  end: Point,
  routeCurveSampleStep: number,
): void {
  commands.push({ kind: "quadratic", control, point: end });
  const estimatedLength = calculateDistance(start, control)
    + calculateDistance(control, end);
  const steps = Math.max(4, Math.ceil(estimatedLength / routeCurveSampleStep));

  for (let step = 1; step <= steps; step += 1) {
    const t = step / steps;
    const tStart = (step - 1) / steps;
    const inverse = 1 - t;
    appendPoint(entries, {
      x: (inverse * inverse * start.x) + (2 * inverse * t * control.x) + (t * t * end.x),
      y: (inverse * inverse * start.y) + (2 * inverse * t * control.y) + (t * t * end.y),
    }, {
      kind: "quadratic",
      start,
      control,
      end,
      tStart,
      tEnd: t,
    });
  }
}

function appendPoint(entries: PathEntry[], point: Point, heading: PathEntryHeading): void {
  const previous = entries[entries.length - 1];
  const segmentLength = calculateDistance(previous, point);
  if (segmentLength <= 0) {
    return;
  }

  entries.push({
    x: point.x,
    y: point.y,
    totalDistance: previous.totalDistance + segmentLength,
    heading,
  });
}

function getQuadraticAngle(start: Point, control: Point, end: Point, t: number): number {
  const inverse = 1 - t;
  const dx = (2 * inverse * (control.x - start.x)) + (2 * t * (end.x - control.x));
  const dy = (2 * inverse * (control.y - start.y)) + (2 * t * (end.y - control.y));
  return Math.atan2(dy, dx);
}

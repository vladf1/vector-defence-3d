export interface CircleSweep {
  readonly previousX: number;
  readonly previousY: number;
  readonly x: number;
  readonly y: number;
  readonly radius: number;
}

export interface ActiveCircleSweep extends CircleSweep {
  readonly removed: boolean;
  readonly hitPoints: number;
}

export interface CircleSweepCollision<T extends CircleSweep> {
  readonly target: T;
  readonly time: number;
  readonly x: number;
  readonly y: number;
}

export interface ActiveCircleSweepCollisionQuery<T extends ActiveCircleSweep> {
  findEarliestCollision(source: CircleSweep): CircleSweepCollision<T> | undefined;
}

export class LinearActiveCircleSweepCollisionIndex<T extends ActiveCircleSweep>
implements ActiveCircleSweepCollisionQuery<T> {
  constructor(private readonly targets: readonly T[]) {
  }

  findEarliestCollision(source: CircleSweep): CircleSweepCollision<T> | undefined {
    return findEarliestActiveCircleSweepCollision(source, this.targets);
  }
}

export class ActiveCircleSweepCollisionIndex<T extends ActiveCircleSweep>
implements ActiveCircleSweepCollisionQuery<T> {
  private readonly cells = new Map<number, T[]>();
  private readonly populatedCells: T[][] = [];
  private readonly queryMarks = new WeakMap<T, number>();
  private readonly targetOrder = new WeakMap<T, number>();
  private queryId = 0;

  constructor(private readonly cellSize: number) {
  }

  rebuild(targets: readonly T[]): void {
    for (const cell of this.populatedCells) {
      cell.length = 0;
    }
    this.populatedCells.length = 0;

    for (let targetIndex = 0; targetIndex < targets.length; targetIndex += 1) {
      const target = targets[targetIndex];
      if (target.removed || target.hitPoints <= 0) {
        continue;
      }
      this.targetOrder.set(target, targetIndex);

      const minCellX = this.toCell(Math.min(target.previousX, target.x) - target.radius);
      const maxCellX = this.toCell(Math.max(target.previousX, target.x) + target.radius);
      const minCellY = this.toCell(Math.min(target.previousY, target.y) - target.radius);
      const maxCellY = this.toCell(Math.max(target.previousY, target.y) + target.radius);
      for (let cellY = minCellY; cellY <= maxCellY; cellY += 1) {
        for (let cellX = minCellX; cellX <= maxCellX; cellX += 1) {
          const key = this.getCellKey(cellX, cellY);
          let cell = this.cells.get(key);
          if (!cell) {
            cell = [];
            this.cells.set(key, cell);
          }
          if (cell.length === 0) {
            this.populatedCells.push(cell);
          }
          cell.push(target);
        }
      }
    }
  }

  findEarliestCollision(source: CircleSweep): CircleSweepCollision<T> | undefined {
    this.queryId += 1;
    let hitTarget: T | undefined;
    let hitTime = Number.POSITIVE_INFINITY;
    let hitOrder = Number.POSITIVE_INFINITY;
    const minCellX = this.toCell(Math.min(source.previousX, source.x) - source.radius);
    const maxCellX = this.toCell(Math.max(source.previousX, source.x) + source.radius);
    const minCellY = this.toCell(Math.min(source.previousY, source.y) - source.radius);
    const maxCellY = this.toCell(Math.max(source.previousY, source.y) + source.radius);

    for (let cellY = minCellY; cellY <= maxCellY; cellY += 1) {
      for (let cellX = minCellX; cellX <= maxCellX; cellX += 1) {
        const cell = this.cells.get(this.getCellKey(cellX, cellY));
        if (!cell) {
          continue;
        }

        for (const target of cell) {
          if (this.queryMarks.get(target) === this.queryId) {
            continue;
          }
          this.queryMarks.set(target, this.queryId);
          if (target.removed || target.hitPoints <= 0) {
            continue;
          }

          const collisionTime = getSweptCircleCollisionTime(source, target);
          const order = this.targetOrder.get(target) ?? Number.POSITIVE_INFINITY;
          if (collisionTime !== undefined && (collisionTime < hitTime || (collisionTime === hitTime && order < hitOrder))) {
            hitTarget = target;
            hitTime = collisionTime;
            hitOrder = order;
          }
        }
      }
    }

    return hitTarget ? createCollision(source, hitTarget, hitTime) : undefined;
  }

  private toCell(value: number): number {
    return Math.floor(value / this.cellSize);
  }

  private getCellKey(cellX: number, cellY: number): number {
    return ((cellX * 73_856_093) ^ (cellY * 19_349_663));
  }
}

export function findEarliestActiveCircleSweepCollision<T extends ActiveCircleSweep>(
  source: CircleSweep,
  targets: readonly T[],
): CircleSweepCollision<T> | undefined {
  let hitTarget: T | undefined;
  let hitTime = Number.POSITIVE_INFINITY;

  for (const target of targets) {
    if (target.removed || target.hitPoints <= 0) {
      continue;
    }

    const collisionTime = getSweptCircleCollisionTime(source, target);
    if (collisionTime !== undefined && collisionTime < hitTime) {
      hitTarget = target;
      hitTime = collisionTime;
    }
  }

  if (!hitTarget) {
    return undefined;
  }

  return createCollision(source, hitTarget, hitTime);
}

function createCollision<T extends CircleSweep>(
  source: CircleSweep,
  target: T,
  time: number,
): CircleSweepCollision<T> {
  return {
    target,
    time,
    x: source.previousX + ((source.x - source.previousX) * time),
    y: source.previousY + ((source.y - source.previousY) * time),
  };
}

/** Returns the first contact time from 0 to 1, or undefined when the sweeps do not collide. */
export function getSweptCircleCollisionTime(source: CircleSweep, target: CircleSweep): number | undefined {
  // Make the target stationary by subtracting its frame movement from the source movement.
  const startX = source.previousX - target.previousX;
  const startY = source.previousY - target.previousY;
  const endX = source.x - target.x;
  const endY = source.y - target.y;
  const combinedRadius = source.radius + target.radius;
  if (!relativeSweepBoundsOverlap(startX, startY, endX, endY, combinedRadius)) {
    return undefined;
  }

  const movementX = endX - startX;
  const movementY = endY - startY;
  const c = (startX * startX) + (startY * startY) - (combinedRadius * combinedRadius);

  if (c <= 0) {
    return 0;
  }

  const a = (movementX * movementX) + (movementY * movementY);
  if (a === 0) {
    return undefined;
  }

  const b = (startX * movementX) + (startY * movementY);
  const discriminant = (b * b) - (a * c);
  if (discriminant < 0) {
    return undefined;
  }

  const time = (-b - Math.sqrt(discriminant)) / a;
  return time >= 0 && time <= 1 ? time : undefined;
}

function relativeSweepBoundsOverlap(startX: number, startY: number, endX: number, endY: number, radius: number): boolean {
  return !(
    (startX < -radius && endX < -radius)
    || (startX > radius && endX > radius)
    || (startY < -radius && endY < -radius)
    || (startY > radius && endY > radius)
  );
}

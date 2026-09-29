const BASE_TAIL_X = -7.8;
const BASE_NOSE_TIP_X = 8.9;
const ROCKET_OFFSET_X = 1.8;
const TAIL_CAP_LENGTH = 2.4;
const MISSILE_LENGTH_SCALE_PER_LEVEL = 0.03;

/** Missile silhouette extents in unscaled missile space; they set the missile's collision radius. */
export interface MissileVisual {
  readonly noseTipX: number;
  readonly tailCapLeftX: number;
}

export function getMissileScale(level: number): number {
  return 1 + (0.05 * level);
}

export function createMissileVisual(level: number): MissileVisual {
  const levelScale = getMissileScale(level);
  const coordinateScaleX = (1 + (MISSILE_LENGTH_SCALE_PER_LEVEL * level)) / levelScale;
  const scaleX = (x: number) => (x + ROCKET_OFFSET_X) * coordinateScaleX;
  return {
    noseTipX: scaleX(BASE_NOSE_TIP_X + 3.2),
    tailCapLeftX: scaleX(BASE_TAIL_X - TAIL_CAP_LENGTH),
  };
}

export function getMissileHalfLength(visual: MissileVisual): number {
  return Math.max(
    Math.abs(visual.tailCapLeftX),
    Math.abs(visual.noseTipX),
  );
}

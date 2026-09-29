import { AudioCue } from "../../audio-manifest";
import type { UpdateContext, UpdateResult } from "../../game-engine/update-context";
import type { PathEntry } from "../../route-path";
import type { Point } from "../../types";
import { randomRange } from "../../utils";
import { createDeathEffectOrigin, createPolygonShardParticles } from "./death-effect-helpers";
import { Monster } from "./monster";
import { createPolygonShardSplitter } from "./polygon-shard-splitter";

const BASE_COLOR = "#ff7a4f";
const ENRAGED_COLOR = "#ff5a36";
const FRENZIED_COLOR = "#ff3158";
const BASE_SPEED_PER_SECOND = 62;
const ENRAGED_SPEED_PER_SECOND = 100;
const FRENZIED_SPEED_PER_SECOND = 138;
const HIT_POINTS = 343;
const BOUNTY = 4;
const RADIUS = 8;
const RAGE_ANIMATION_DURATION_SECONDS = 1.05;
const BODY_SURGE_MIN_SCALE = 0.015;
const BODY_SURGE_MAX_SCALE = 0.085;
const OUTLINE = [
  { x: RADIUS * 1.55, y: 0 },
  { x: RADIUS * 0.4, y: -RADIUS * 0.8 },
  { x: -RADIUS * 0.1, y: -RADIUS * 1.08 },
  { x: -RADIUS * 1.28, y: -RADIUS * 0.44 },
  { x: -RADIUS * 0.72, y: 0 },
  { x: -RADIUS * 1.28, y: RADIUS * 0.44 },
  { x: -RADIUS * 0.1, y: RADIUS * 1.08 },
  { x: RADIUS * 0.4, y: RADIUS * 0.8 },
];
const SHARD_SPLITTER = createPolygonShardSplitter({
  minShardCount: 5,
  maxShardCount: 11,
});

export class BerserkerMonster extends Monster {
  private rageStage = 0;
  private rageAnimationElapsedSeconds = randomRange(0, RAGE_ANIMATION_DURATION_SECONDS);

  constructor(path: PathEntry[], private readonly speedScale: number) {
    super(path, BASE_COLOR, BASE_SPEED_PER_SECOND * speedScale, HIT_POINTS, BOUNTY, RADIUS);
  }

  get currentRageStage(): number {
    return this.rageStage;
  }

  protected override updateSpecial(context: UpdateContext): void {
    const nextStage = this.hitPoints <= this.maxHitPoints * 0.2
      ? 2
      : (this.hitPoints <= this.maxHitPoints * 0.5 ? 1 : 0);

    if (nextStage !== this.rageStage) {
      this.rageStage = nextStage;
      const burstFloor = this.getStageSpeedPerSecond() * (0.72 + (this.rageStage * 0.08));
      this.speedPerSecond = Math.max(this.speedPerSecond, burstFloor);
    }

    this.rageAnimationElapsedSeconds += context.deltaSeconds;
    this.maxSpeedPerSecond = this.getStageSpeedPerSecond();
    this.color = this.getStageColor();

    if (this.speedPerSecond < this.maxSpeedPerSecond) {
      this.speedPerSecond = Math.min(
        this.maxSpeedPerSecond,
        this.speedPerSecond + ((50.4 + (this.rageStage * 43.2)) * this.speedScale * context.deltaSeconds),
      );
    } else if (this.speedPerSecond > this.maxSpeedPerSecond) {
      this.speedPerSecond = this.maxSpeedPerSecond;
    }

    this.velocityXPerSecond = Math.cos(this.angle) * this.speedPerSecond;
    this.velocityYPerSecond = Math.sin(this.angle) * this.speedPerSecond;
  }

  override addDeathEffect(result: UpdateResult): void {
    const motion = this.getRageMotion();
    const origin = createDeathEffectOrigin(this.radius, -0.15, 0.22, -0.15, 0.15);
    createPolygonShardParticles(
      result,
      this,
      this.createAnimatedOutline(motion),
      transformPoint(origin, motion),
      this.angle,
      140,
      230,
      0,
      SHARD_SPLITTER,
    );
    result.playSound(AudioCue.MonsterHeavyDeath, this.x, 1.05);
  }

  private getStageColor(): string {
    if (this.rageStage === 2) {
      return FRENZIED_COLOR;
    }
    if (this.rageStage === 1) {
      return ENRAGED_COLOR;
    }
    return BASE_COLOR;
  }

  private getStageSpeedPerSecond(): number {
    if (this.rageStage === 2) {
      return FRENZIED_SPEED_PER_SECOND * this.speedScale;
    }
    if (this.rageStage === 1) {
      return ENRAGED_SPEED_PER_SECOND * this.speedScale;
    }
    return BASE_SPEED_PER_SECOND * this.speedScale;
  }

  getRageMotion(): RageMotion {
    const stageIntensity = this.rageStage === 2 ? 1 : (this.rageStage === 1 ? 0.68 : 0.36);
    const cycle = this.rageAnimationElapsedSeconds / RAGE_ANIMATION_DURATION_SECONDS;
    const surge = (1 - Math.cos(cycle * Math.PI * 2)) / 2;
    const scaleAmount = BODY_SURGE_MIN_SCALE + ((BODY_SURGE_MAX_SCALE - BODY_SURGE_MIN_SCALE) * stageIntensity);

    return {
      scaleX: 1 + (surge * scaleAmount),
      scaleY: 1 - (surge * scaleAmount * 0.45),
      emberAlpha: 0.24 + (stageIntensity * 0.42),
    };
  }

  private createAnimatedOutline(motion: RageMotion): Point[] {
    return OUTLINE.map((point) => transformPoint(point, motion));
  }

}

export interface RageMotion {
  scaleX: number;
  scaleY: number;
  emberAlpha: number;
}

function transformPoint(point: Point, motion: RageMotion): Point {
  return {
    x: point.x * motion.scaleX,
    y: point.y * motion.scaleY,
  };
}

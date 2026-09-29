import { AudioCue } from "../../audio-manifest";
import type { UpdateContext, UpdateResult } from "../../game-engine/update-context";
import type { PathEntry } from "../../route-path";
import type { Point } from "../../types";
import { easeInOutCubic, easeInOutSine, randomRange } from "../../utils";
import { createDeathEffectOrigin, createPolygonShardParticles, pointOnRadius } from "./death-effect-helpers";
import { Monster } from "./monster";
import { createPolygonShardSplitter } from "./polygon-shard-splitter";

const COLOR = "#5df2ef";
const SPEED_PER_SECOND = 81;
const HIT_POINTS = 264;
const BOUNTY = 2;
const RADIUS = 7.5;
const MOUTH_OPEN_ANGLE = Math.PI * 0.18;
const MOUTH_CLOSED_ANGLE = Math.PI * 0.035;
const IDLE_ANIMATION_INTERVAL_MIN_SECONDS = 2;
const IDLE_ANIMATION_INTERVAL_MAX_SECONDS = 5;
const MOUTH_ANIMATION_DURATION_SECONDS = 0.5;
const ROTATION_ANIMATION_DURATION_SECONDS = 0.9;
const FULL_ROTATION = Math.PI * 2;
const SHARD_SPLITTER = createPolygonShardSplitter({
  minShardCount: 5,
  maxShardCount: 11,
  maxShardVertices: 26,
});

export class PackManMonster extends Monster {
  private mouthAngle = MOUTH_OPEN_ANGLE;
  private mouthAnimationElapsedSeconds = 0;
  private bodyRotation = 0;
  private bodyRotationDirection = 1;
  private rotationAnimationElapsedSeconds = 0;
  private secondsUntilIdleAnimation = randomRange(IDLE_ANIMATION_INTERVAL_MIN_SECONDS, IDLE_ANIMATION_INTERVAL_MAX_SECONDS);

  constructor(path: PathEntry[], speedScale: number) {
    super(path, COLOR, SPEED_PER_SECOND * speedScale, HIT_POINTS, BOUNTY, RADIUS);
  }

  get currentMouthAngle(): number {
    return this.mouthAngle;
  }

  get currentBodyRotation(): number {
    return this.bodyRotation;
  }

  protected override updateSpecial(context: UpdateContext): void {
    if (this.mouthAnimationElapsedSeconds > 0) {
      this.advanceMouthAnimation(context.deltaSeconds);
      return;
    }

    if (this.rotationAnimationElapsedSeconds > 0) {
      this.advanceRotationAnimation(context.deltaSeconds);
      return;
    }

    this.secondsUntilIdleAnimation -= context.deltaSeconds;
    if (this.secondsUntilIdleAnimation <= 0) {
      this.startIdleAnimation(context.deltaSeconds);
    }
  }

  createOutline(arcVertexCount: number): Point[] {
    return createPackManOutline(this.radius, this.mouthAngle, arcVertexCount);
  }

  override addDeathEffect(result: UpdateResult): void {
    createPolygonShardParticles(
      result,
      this,
      this.createOutline(18),
      createDeathEffectOrigin(this.radius, -0.12, 0.12, -0.12, 0.12),
      this.angle + this.bodyRotation,
      125,
      205,
      0,
      SHARD_SPLITTER,
    );
    result.playSound(AudioCue.MonsterShatter, this.x);
  }

  private advanceMouthAnimation(deltaSeconds: number): void {
    this.mouthAnimationElapsedSeconds += deltaSeconds;
    const progress = Math.min(1, this.mouthAnimationElapsedSeconds / MOUTH_ANIMATION_DURATION_SECONDS);
    this.mouthAngle = getPackManMouthAngle(progress);

    if (progress === 1) {
      this.mouthAngle = MOUTH_OPEN_ANGLE;
      this.mouthAnimationElapsedSeconds = 0;
      this.resetIdleAnimationInterval();
    }
  }

  private advanceRotationAnimation(deltaSeconds: number): void {
    this.rotationAnimationElapsedSeconds += deltaSeconds;
    const progress = Math.min(1, this.rotationAnimationElapsedSeconds / ROTATION_ANIMATION_DURATION_SECONDS);
    this.bodyRotation = this.bodyRotationDirection * FULL_ROTATION * easeInOutCubic(progress);

    if (progress === 1) {
      this.bodyRotation = 0;
      this.rotationAnimationElapsedSeconds = 0;
      this.resetIdleAnimationInterval();
    }
  }

  private resetIdleAnimationInterval(): void {
    this.secondsUntilIdleAnimation = randomRange(IDLE_ANIMATION_INTERVAL_MIN_SECONDS, IDLE_ANIMATION_INTERVAL_MAX_SECONDS);
  }

  private startIdleAnimation(deltaSeconds: number): void {
    if (randomRange(0, 1) < 0.5) {
      this.advanceMouthAnimation(deltaSeconds);
      return;
    }

    this.bodyRotationDirection = randomRange(0, 1) < 0.5 ? -1 : 1;
    this.advanceRotationAnimation(deltaSeconds);
  }
}

function getPackManMouthAngle(animationProgress: number): number {
  const clampedProgress = Math.max(0, Math.min(1, animationProgress));
  const snapProgress = clampedProgress < 1 ? (clampedProgress * 2) % 1 : 1;
  const mirroredProgress = snapProgress <= 0.5 ? snapProgress * 2 : (1 - snapProgress) * 2;
  const closeThenOpen = easeInOutSine(mirroredProgress);
  return MOUTH_OPEN_ANGLE - ((MOUTH_OPEN_ANGLE - MOUTH_CLOSED_ANGLE) * closeThenOpen);
}

function createPackManOutline(radius: number, mouthAngle: number, arcVertexCount: number): Point[] {
  const bodySweepAngle = (Math.PI * 2) - (mouthAngle * 2);
  const vertexCount = Math.max(2, Math.floor(arcVertexCount));
  const outline = [{ x: 0, y: 0 }];

  for (let index = 0; index < vertexCount; index += 1) {
    const ratio = vertexCount === 1 ? 0 : index / (vertexCount - 1);
    const angle = mouthAngle + (bodySweepAngle * ratio);
    outline.push(pointOnRadius(angle, radius));
  }

  return outline;
}

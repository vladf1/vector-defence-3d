import { AudioCue } from "../../audio-manifest";
import { createMissileExplosionParticles } from "../../game-engine/combat-effects";
import type { UpdateContext, UpdateResult } from "../../game-engine/update-context";
import type { Point } from "../../types";
import { angleBetween, calculateDistance, clamp, isOutsideBounds, randomRange, turnAngleTowards } from "../../utils";
import { Particle } from "../effects/particle";
import type { Monster } from "../monsters/monster";
import { getMissileHalfLength, getMissileScale, type MissileVisual } from "./missile-visuals";

const MISSILE_DAMAGE_BASE = 50;
const MISSILE_DAMAGE_PER_LEVEL = 4;
const MISSILE_EFFECT_RADIUS_BASE = 60;
const MISSILE_EFFECT_RADIUS_PER_LEVEL = 5;
const MISSILE_SPEED_BASE_PER_SECOND = 108;
const MISSILE_SPEED_PER_LEVEL_PER_SECOND = 30;
const MISSILE_HIT_SHAKE_MIN_DURATION_SECONDS = 0.08;
const MISSILE_HIT_SHAKE_DURATION_RANGE_SECONDS = 0.035;
const MISSILE_HIT_SHAKE_MIN_DISTANCE = 0.45;
const MISSILE_HIT_SHAKE_DISTANCE_RANGE = 0.8;
const MISSILE_TURN_SPEED_PER_SECOND = 7.2;
const MISSILE_LAUNCH_BLOOM_SECONDS = 0.28;

function getShakeStrengthFromSplashRatio(ratio: number): number {
  return clamp(ratio, 0.15, 1);
}

export class Missile {
  x: number;
  y: number;
  previousX: number;
  previousY: number;
  radius: number;
  angle: number;
  speedPerSecond: number;
  damage: number;
  effectRadius: number;
  scale: number;
  level: number;
  trackedMonster?: Monster;
  removed = false;
  trailTimer = 0;
  launchBloomSeconds = MISSILE_LAUNCH_BLOOM_SECONDS;

  constructor(source: Point, trackedMonster: Monster, level: number, visual: MissileVisual, initialAngle?: number) {
    this.x = source.x;
    this.y = source.y;
    this.previousX = source.x;
    this.previousY = source.y;
    this.trackedMonster = trackedMonster;
    this.level = level;
    this.damage = MISSILE_DAMAGE_BASE + (MISSILE_DAMAGE_PER_LEVEL * level);
    this.effectRadius = MISSILE_EFFECT_RADIUS_BASE + (MISSILE_EFFECT_RADIUS_PER_LEVEL * level);
    this.speedPerSecond = MISSILE_SPEED_BASE_PER_SECOND + (MISSILE_SPEED_PER_LEVEL_PER_SECOND * level);
    this.scale = getMissileScale(level);
    this.radius = getMissileHalfLength(visual) * this.scale;
    this.angle = initialAngle ?? angleBetween(source, trackedMonster);
  }

  update(context: UpdateContext, result: UpdateResult): void {
    this.launchBloomSeconds = Math.max(0, this.launchBloomSeconds - context.deltaSeconds);
    this.speedPerSecond += 180 * context.deltaSeconds;
    if (this.trackedMonster && this.trackedMonster.removed) {
      this.trackedMonster = undefined;
    }
    if (this.trackedMonster) {
      const targetAngle = angleBetween(this, this.trackedMonster);
      this.angle = turnAngleTowards(this.angle, targetAngle, MISSILE_TURN_SPEED_PER_SECOND * context.deltaSeconds);
    }

    this.previousX = this.x;
    this.previousY = this.y;
    this.x += Math.cos(this.angle) * this.speedPerSecond * context.deltaSeconds;
    this.y += Math.sin(this.angle) * this.speedPerSecond * context.deltaSeconds;

    const collision = context.monsterCollisionIndex.findEarliestCollision(this);
    if (collision) {
      this.x = collision.x;
      this.y = collision.y;
      this.explode(context, result);
      return;
    }

    this.addTrail(context.deltaSeconds, result);
    if (isOutsideBounds(this, context.fieldBounds, 20)) {
      this.removed = true;
    }
  }

  private addTrail(deltaSeconds: number, result: UpdateResult): void {
    this.trailTimer += deltaSeconds;
    if (this.trailTimer < 0.02) {
      return;
    }

    this.trailTimer -= 0.02;
    if (result.remainingParticleCapacity === 0) {
      return;
    }
    const trailX = this.x + randomRange(-3, 3) - (Math.cos(this.angle) * 9);
    const trailY = this.y + randomRange(-3, 3) - (Math.sin(this.angle) * 9);
    const exhaustAngle = this.angle + Math.PI + randomRange(-0.35, 0.35);
    result.addParticle(new Particle(trailX, trailY, randomRange(0.6, 1.2), "#fff0a8", 5.5, {
      speedPerSecond: randomRange(36, 82),
      offset: 0,
      angle: exhaustAngle,
    }));
    result.addParticle(new Particle(trailX, trailY, randomRange(0.8, 1.5), "#ff8f45", 3.8, {
      speedPerSecond: randomRange(28, 68),
      offset: 1,
      angle: exhaustAngle,
    }));
    result.addParticle(new Particle(trailX, trailY, 1, "#7e858c", 1.4, {
      speedPerSecond: randomRange(22, 50),
      offset: 2,
      angle: exhaustAngle,
    }));
  }

  private explode(context: UpdateContext, result: UpdateResult): void {
    this.removed = true;
    for (const particle of createMissileExplosionParticles(this.x, this.y, this.angle, this.level, result.remainingParticleCapacity)) {
      result.addParticle(particle);
    }
    for (const nearby of context.activeMonsters) {
      if (nearby.removed || nearby.hitPoints <= 0) {
        continue;
      }

      const distance = calculateDistance(this, nearby);
      if (distance > this.effectRadius) {
        continue;
      }

      const ratio = (this.effectRadius - distance) / this.effectRadius;
      const shakeStrength = getShakeStrengthFromSplashRatio(ratio);
      nearby.shake(
        MISSILE_HIT_SHAKE_MIN_DURATION_SECONDS + (MISSILE_HIT_SHAKE_DURATION_RANGE_SECONDS * shakeStrength),
        MISSILE_HIT_SHAKE_MIN_DISTANCE + (MISSILE_HIT_SHAKE_DISTANCE_RANGE * shakeStrength),
      );
      nearby.takeDamage(this.damage * ratio);
    }
    result.playSound(AudioCue.MissileExplosion, this.x, 1.1);
  }
}

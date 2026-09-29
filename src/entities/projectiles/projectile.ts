import { AudioCue } from "../../audio-manifest";
import { createHitImpactParticles } from "../../game-engine/combat-effects";
import type { UpdateContext, UpdateResult } from "../../game-engine/update-context";
import type { Point } from "../../types";
import { angleBetween, isOutsideBounds } from "../../utils";

export abstract class Projectile {
  x: number;
  y: number;
  previousX: number;
  previousY: number;
  velocityXPerSecond: number;
  velocityYPerSecond: number;
  damage: number;
  radius: number;
  angle: number;
  removed = false;

  protected constructor(
    source: Point,
    target: Point,
    speedPerSecond: number,
    damage: number,
    radius: number,
    private readonly impactColor: string,
    private readonly impactSoundIntensity?: number,
  ) {
    this.angle = angleBetween(source, target);
    this.x = source.x;
    this.y = source.y;
    this.previousX = source.x;
    this.previousY = source.y;
    this.velocityXPerSecond = Math.cos(this.angle) * speedPerSecond;
    this.velocityYPerSecond = Math.sin(this.angle) * speedPerSecond;
    this.damage = damage;
    this.radius = radius;
  }

  update(context: UpdateContext, result: UpdateResult): void {
    this.previousX = this.x;
    this.previousY = this.y;
    this.x += this.velocityXPerSecond * context.deltaSeconds;
    this.y += this.velocityYPerSecond * context.deltaSeconds;

    const collision = context.monsterCollisionIndex.findEarliestCollision(this);
    if (collision) {
      this.x = collision.x;
      this.y = collision.y;
      collision.target.takeDamage(this.damage);
      this.removed = true;
      for (const particle of createHitImpactParticles(this.x, this.y, this.impactColor, this.angle, result.remainingParticleCapacity)) {
        result.addParticle(particle);
      }
      result.playSound(AudioCue.ProjectileImpact, this.x, this.impactSoundIntensity);
      return;
    }

    if (isOutsideBounds(this, context.fieldBounds, 20)) {
      this.removed = true;
    }
  }
}

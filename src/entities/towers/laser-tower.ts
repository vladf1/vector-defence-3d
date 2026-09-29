import { AudioCue } from "../../audio-manifest";
import { createLaserImpactParticles } from "../../game-engine/combat-effects";
import type { UpdateContext, UpdateResult } from "../../game-engine/update-context";
import { type Point, TowerKind } from "../../types";
import { angleBetween, clamp, isWithinDistanceToSegment, randomRange, turnAngleTowards, withinDistance } from "../../utils";
import { Tower } from "./tower";

export const LASER_COLORS = [
  { body: "#5bf4ff", accent: "#9dffd7", beam: "110, 255, 152" },
  { body: "#6dff9c", accent: "#d8ff4f", beam: "185, 255, 105" },
  { body: "#ffe36f", accent: "#ff9d5c", beam: "255, 227, 111" },
  { body: "#ffad4f", accent: "#ff6d8c", beam: "255, 157, 92" },
  { body: "#ff8edb", accent: "#b58cff", beam: "255, 142, 219" },
  { body: "#b58cff", accent: "#78a7ff", beam: "181, 140, 255" },
  { body: "#4f8cff", accent: "#f6f0ff", beam: "79, 140, 255" },
] as const;
const BEAM_FADE_PER_SECOND = 0.9;

export class LaserTower extends Tower {
  static readonly kind = TowerKind.Laser;
  static readonly label = "Laser";
  static readonly summary = "Piercing beam that melts lines of enemies.";
  static readonly baseCost = 3;
  static readonly baseRange = 100;
  static readonly shortcuts = ["2", "z"] as const;

  angle = randomRange(-Math.PI, Math.PI);
  beamAlpha = 0;
  beamTarget = { x: 0, y: 0 };
  damagePerSecond = 60;
  directionLocked = false;
  laserSparkCooldownSeconds = 0;
  turnSpeedPerSecond = 6.72;
  private readonly beamSource = { x: 0, y: 0 };

  protected updateTower(context: UpdateContext, result: UpdateResult): void {
    this.laserSparkCooldownSeconds = Math.max(0, this.laserSparkCooldownSeconds - context.deltaSeconds);

    let alignedToTarget = false;
    if (!this.directionLocked) {
      const tracked = this.findTrackedMonsterInContext(context);
      if (tracked) {
        const targetAngle = angleBetween(this, tracked);
        this.angle = turnAngleTowards(this.angle, targetAngle, this.turnSpeedPerSecond * context.deltaSeconds);
        alignedToTarget = this.isAimedAtTarget(this.angle, targetAngle);
      }
    }

    const directionX = Math.cos(this.angle), directionY = Math.sin(this.angle);
    const source = this.getBeamSource(directionX, directionY);
    this.beamTarget.x = this.x + (directionX * 1000);
    this.beamTarget.y = this.y + (directionY * 1000);
    if (this.ready() && (this.directionLocked ? this.hasMonsterInBeam(context, source) : alignedToTarget)) {
      this.fire(result);
    }

    const integratedBeamStrengthSeconds = this.advanceBeam(context.deltaSeconds);
    if (integratedBeamStrengthSeconds <= 0) {
      return;
    }

    const shouldCreateSparks = this.laserSparkCooldownSeconds <= 0 && result.remainingParticleCapacity > 0;
    let sparkBurstsCreated = 0;
    const colors = this.getLaserColors();
    // The beam is fixed during this update; reuse its geometry and each hit's spark position.
    const minX = Math.min(source.x, this.beamTarget.x), maxX = Math.max(source.x, this.beamTarget.x);
    const minY = Math.min(source.y, this.beamTarget.y), maxY = Math.max(source.y, this.beamTarget.y);
    const beamX = this.beamTarget.x - source.x, beamY = this.beamTarget.y - source.y;
    const inverseLengthSquared = 1 / ((beamX * beamX) + (beamY * beamY));

    for (const monster of context.activeMonsters) {
      if (!this.isMonsterActive(monster)) {
        continue;
      }
      if (monster.x < minX - monster.radius || monster.x > maxX + monster.radius
        || monster.y < minY - monster.radius || monster.y > maxY + monster.radius) continue;
      const dot = ((monster.x - source.x) * beamX) + ((monster.y - source.y) * beamY);
      const projection = clamp(dot * inverseLengthSquared, 0, 1);
      const impactX = source.x + (projection * beamX), impactY = source.y + (projection * beamY);
      const dx = monster.x - impactX, dy = monster.y - impactY;
      if ((dx * dx) + (dy * dy) <= monster.radius * monster.radius) {
        monster.takeContinuousDamage(this.damagePerSecond * integratedBeamStrengthSeconds);
        if (shouldCreateSparks && sparkBurstsCreated < 2) {
          for (const particle of createLaserImpactParticles(impactX, impactY, this.angle, colors.accent, result.remainingParticleCapacity)) {
            result.addParticle(particle);
          }
          sparkBurstsCreated += 1;
        }
      }
    }

    if (sparkBurstsCreated > 0) {
      this.laserSparkCooldownSeconds = 0.055;
    }
  }

  private advanceBeam(deltaSeconds: number): number {
    if (this.beamAlpha <= 0 || deltaSeconds <= 0) {
      return 0;
    }

    const startingAlpha = this.beamAlpha;
    const activeSeconds = Math.min(deltaSeconds, startingAlpha / BEAM_FADE_PER_SECOND);
    const endingAlpha = Math.max(0, startingAlpha - (BEAM_FADE_PER_SECOND * activeSeconds));
    this.beamAlpha = endingAlpha;
    return activeSeconds * ((startingAlpha + endingAlpha) / 2);
  }

  protected onUpgrade(): void {
    this.damagePerSecond = 60 + (15 * this.level);
  }

  toggleDirectionLock(): void {
    this.directionLocked = !this.directionLocked;
  }

  private getLaserColors(): typeof LASER_COLORS[number] {
    return LASER_COLORS[Math.min(this.level, LASER_COLORS.length - 1)];
  }

  getMuzzleOffset(): number {
    return 8.5 + (this.level * 0.78);
  }

  private fire(result: UpdateResult): void {
    this.beamAlpha = 1;
    this.resetCooldown(1.5);
    result.playSound(AudioCue.LaserFire, this.x, 0.9 + (this.level * 0.1));
  }

  private hasMonsterInBeam(context: UpdateContext, source: Point): boolean {
    for (const monster of context.activeMonsters) {
      if (!this.isMonsterActive(monster)) {
        continue;
      }
      if (!withinDistance(this, monster, this.range)) {
        continue;
      }

      if (isWithinDistanceToSegment(monster, source, this.beamTarget, monster.radius)) {
        return true;
      }
    }
    return false;
  }

  private getBeamSource(directionX: number, directionY: number): Point {
    const muzzleOffset = this.getMuzzleOffset();
    this.beamSource.x = this.x + (directionX * muzzleOffset);
    this.beamSource.y = this.y + (directionY * muzzleOffset);
    return this.beamSource;
  }
}

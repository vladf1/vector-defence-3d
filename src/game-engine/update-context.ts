import type { AudioCue as AudioCueValue } from "../audio-manifest";
import type { Particle } from "../entities/effects/particle";
import type { Monster } from "../entities/monsters/monster";
import type { Drone } from "../entities/projectiles/drone";
import type { Missile } from "../entities/projectiles/missile";
import type { Projectile } from "../entities/projectiles/projectile";
import type { RuntimeLinkEffect } from "../level-runtime";
import type { ActiveCircleSweepCollisionQuery } from "./collision-detection";
import type { FieldBounds } from "../types";

export interface UpdateContext {
  deltaSeconds: number;
  readonly fieldWidth: number;
  readonly fieldHeight: number;
  fieldBounds: FieldBounds;
  readonly activeMonsters: readonly Monster[];
  readonly monsterCollisionIndex: ActiveCircleSweepCollisionQuery<Monster>;
  activeDrones: readonly Drone[];
  readonly droneAssignments: ReadonlyMap<Monster, number>;
}

export interface UpdateSound {
  cue: AudioCueValue;
  panX?: number;
  intensity?: number;
}

export class UpdateResult {
  particleLimit = Number.POSITIVE_INFINITY;
  linkLimit = Number.POSITIVE_INFINITY;
  readonly killedMonsters: Monster[] = [];
  readonly escapedMonsters: Monster[] = [];
  readonly particles: Particle[] = [];
  readonly links: RuntimeLinkEffect[] = [];
  readonly drones: Drone[] = [];
  readonly projectiles: Projectile[] = [];
  readonly missiles: Missile[] = [];
  readonly sounds: UpdateSound[] = [];

  get remainingParticleCapacity(): number {
    return Math.max(0, this.particleLimit - this.particles.length);
  }

  get remainingLinkCapacity(): number {
    return Math.max(0, this.linkLimit - this.links.length);
  }

  addKilledMonster(monster: Monster): void {
    this.killedMonsters.push(monster);
  }

  addEscapedMonster(monster: Monster): void {
    this.escapedMonsters.push(monster);
  }

  addParticle(particle: Particle): void {
    if (this.particles.length < this.particleLimit) {
      this.particles.push(particle);
    }
  }

  addLink(link: RuntimeLinkEffect): void {
    if (this.remainingLinkCapacity > 0) {
      this.links.push(link);
    }
  }

  addDrone(drone: Drone): void {
    this.drones.push(drone);
  }

  addProjectile(projectile: Projectile): void {
    this.projectiles.push(projectile);
  }

  addMissile(missile: Missile): void {
    this.missiles.push(missile);
  }

  playSound(cue: AudioCueValue, panX?: number, intensity?: number): void {
    this.sounds.push({ cue, panX, intensity });
  }

  clear(): void {
    this.particleLimit = Number.POSITIVE_INFINITY;
    this.linkLimit = Number.POSITIVE_INFINITY;
    this.killedMonsters.length = 0;
    this.escapedMonsters.length = 0;
    this.particles.length = 0;
    this.links.length = 0;
    this.drones.length = 0;
    this.projectiles.length = 0;
    this.missiles.length = 0;
    this.sounds.length = 0;
  }
}

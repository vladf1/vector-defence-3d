import { DRONE_ACCENT_COLORS } from "../entities/drone-visuals";
import type { Drone } from "../entities/projectiles/drone";
import { DroneProjectile } from "../entities/projectiles/drone-projectile";
import { GunProjectile } from "../entities/projectiles/gun-projectile";
import type { Missile } from "../entities/projectiles/missile";
import type { Projectile } from "../entities/projectiles/projectile";
import type { LevelRuntime } from "../level-runtime";
import { clamp, normalizeAngle } from "../utils";
import { Quat, smoothTowards, type FrameContext } from "./frame-math";
import { GUN_BARREL_Y, MISSILE_RACK_Y } from "./models";
import type { MonsterView } from "./monster-view";
import { linearColor } from "./palette";
import type { RenderBatches } from "./render-batches";

const GUN_TRACER = linearColor("#d9fff3");
const GUN_TRACER_GLOW = linearColor("#9fffe4");
const MISSILE_BODY = linearColor("#ff9d5c");
const MISSILE_FLAME = linearColor("#ffb04a");
const MISSILE_FLAME_CORE = linearColor("#fff0a8");
const ROTOR_COLOR = linearColor("#e8fff6");
const DRONE_CRUISE_ALTITUDE = 36;
const DRONE_EXIT_CLIMB_PER_SECOND = 60;
const MISSILE_CRUISE_ALTITUDE = 46;
const MISSILE_CLIMB_DISTANCE = 70;
const DRONE_SHOT_DROP_DISTANCE = 52;
const ROTOR_OFFSETS = [[-6.9, -6.9], [6.9, -6.9], [-6.9, 6.9], [6.9, 6.9]] as const;

interface MissileFlight {
  frame: number;
  launchX: number;
  launchY: number;
  altitude: number;
  climbAngle: number;
}

interface DroneFlight {
  frame: number;
  altitude: number;
  bank: number;
  pitch: number;
  previousX: number;
  previousY: number;
  previousAngle: number;
}

interface ShotFlight {
  frame: number;
  startX: number;
  startY: number;
  startAltitude: number;
}

const rotation = new Quat();

/**
 * Gives flat simulation trajectories a third dimension: missiles climb and dive,
 * drones cruise, bank, and climb away, and drone rounds drop from altitude.
 */
export class ProjectileView {
  private readonly missiles = new Map<Missile, MissileFlight>();
  private readonly drones = new Map<Drone, DroneFlight>();
  private readonly shots = new Map<Projectile, ShotFlight>();
  private runtime?: LevelRuntime;

  constructor(private readonly monsters: MonsterView) {}

  reset(): void {
    this.missiles.clear();
    this.drones.clear();
    this.shots.clear();
    this.runtime = undefined;
  }

  /** Altitude of the missile closest to a field point, used to lift its trail particles. */
  getMissileAltitudeNear(x: number, y: number): number | undefined {
    let best: number | undefined;
    let bestDistance = 26 * 26;
    for (const [missile, flight] of this.missiles) {
      const dx = missile.x - x;
      const dy = missile.y - y;
      const distance = (dx * dx) + (dy * dy);
      if (distance < bestDistance) {
        bestDistance = distance;
        best = flight.altitude;
      }
    }
    return best;
  }

  getDroneAltitudeNear(x: number, y: number): number | undefined {
    let best: number | undefined;
    let bestDistance = 20 * 20;
    for (const [drone, flight] of this.drones) {
      const dx = drone.x - x;
      const dy = drone.y - y;
      const distance = (dx * dx) + (dy * dy);
      if (distance < bestDistance) {
        bestDistance = distance;
        best = flight.altitude;
      }
    }
    return best;
  }

  write(runtime: LevelRuntime, batches: RenderBatches, frame: FrameContext): void {
    if (runtime !== this.runtime) {
      this.reset();
      this.runtime = runtime;
    }
    this.writeDrones(runtime.drones, batches, frame);
    this.writeShots(runtime.projectiles, batches, frame);
    this.writeMissiles(runtime.missiles, batches, frame);
  }

  private writeShots(projectiles: readonly Projectile[], batches: RenderBatches, frame: FrameContext): void {
    for (const projectile of projectiles) {
      if (projectile.removed) {
        continue;
      }
      let flight = this.shots.get(projectile);
      if (!flight) {
        const fromDrone = projectile instanceof DroneProjectile;
        flight = {
          frame: 0,
          startX: projectile.previousX,
          startY: projectile.previousY,
          startAltitude: fromDrone
            ? (this.getDroneAltitudeNear(projectile.previousX, projectile.previousY) ?? DRONE_CRUISE_ALTITUDE)
            : GUN_BARREL_Y,
        };
        this.shots.set(projectile, flight);
      }
      flight.frame = frame.frame;

      const travelled = Math.hypot(projectile.x - flight.startX, projectile.y - flight.startY);
      const targetAltitude = 7;
      const drop = projectile instanceof DroneProjectile ? clamp(travelled / DRONE_SHOT_DROP_DISTANCE, 0, 1) : clamp(travelled / 90, 0, 1);
      const altitude = flight.startAltitude + ((targetAltitude - flight.startAltitude) * drop);
      const yaw = -projectile.angle;

      if (projectile instanceof GunProjectile) {
        const length = (9.8 + (projectile.visualLevel * 1.45)) * 1.25;
        const width = 2.6 + (projectile.visualLevel * 0.35);
        const tail = Math.min(length, travelled + 2);
        const cos = Math.cos(projectile.angle);
        const sin = Math.sin(projectile.angle);
        const slot = batches.ribbon.pushYaw(projectile.x - (cos * tail), altitude, projectile.y - (sin * tail), yaw, tail, 1, width, GUN_TRACER_GLOW.r * 1.6, GUN_TRACER_GLOW.g * 1.6, GUN_TRACER_GLOW.b * 1.6);
        batches.ribbon.setExtra(slot, 0, 1);
        batches.glow.push(projectile.x, altitude, projectile.y, 0, width * 2.2, width * 2.2, 0, GUN_TRACER.r * 2, GUN_TRACER.g * 2, GUN_TRACER.b * 2, 0.9);
      } else if (projectile instanceof DroneProjectile) {
        const color = linearColor(projectile.accentColor);
        const cos = Math.cos(projectile.angle);
        const sin = Math.sin(projectile.angle);
        const tail = Math.min(10, travelled + 1);
        const slot = batches.ribbon.pushYaw(projectile.x - (cos * tail), altitude, projectile.y - (sin * tail), yaw, tail, 1, 2.2, color.r * 1.8, color.g * 1.8, color.b * 1.8);
        batches.ribbon.setExtra(slot, 0, 1);
        batches.glow.push(projectile.x, altitude, projectile.y, 0, 5, 5, 0, color.r * 2.2, color.g * 2.2, color.b * 2.2, 0.9);
      }
    }
    for (const [projectile, flight] of this.shots) {
      if (flight.frame !== frame.frame) {
        this.shots.delete(projectile);
      }
    }
  }

  private writeMissiles(missiles: readonly Missile[], batches: RenderBatches, frame: FrameContext): void {
    for (const missile of missiles) {
      if (missile.removed) {
        continue;
      }
      let flight = this.missiles.get(missile);
      if (!flight) {
        flight = { frame: 0, launchX: missile.previousX, launchY: missile.previousY, altitude: MISSILE_RACK_Y + 1, climbAngle: 0 };
        this.missiles.set(missile, flight);
      }
      flight.frame = frame.frame;

      const target = missile.trackedMonster;
      const travelled = Math.hypot(missile.x - flight.launchX, missile.y - flight.launchY);
      const climb = clamp(travelled / MISSILE_CLIMB_DISTANCE, 0, 1);
      let desired = MISSILE_RACK_Y + ((MISSILE_CRUISE_ALTITUDE - MISSILE_RACK_Y) * Math.sin(climb * Math.PI * 0.5));
      if (target && !target.removed) {
        const remaining = Math.hypot(target.x - missile.x, target.y - missile.y);
        const dive = clamp(1 - (remaining / 110), 0, 1);
        const targetHeight = this.monsters.getCenterHeight(target);
        desired += (targetHeight - desired) * dive * dive;
      } else {
        desired = Math.max(8, desired - (travelled * 0.05));
      }
      const previousAltitude = flight.altitude;
      flight.altitude = frame.deltaSeconds > 0 ? smoothTowards(flight.altitude, desired, 14, frame.deltaSeconds) : flight.altitude;
      const horizontalSpeed = Math.max(1, missile.speedPerSecond);
      const verticalSpeed = frame.deltaSeconds > 0 ? (flight.altitude - previousAltitude) / frame.deltaSeconds : 0;
      flight.climbAngle = Math.atan2(verticalSpeed, horizontalSpeed);

      const scale = missile.scale * 0.62;
      batches.pushBlobShadow(missile.x, missile.y, flight.altitude, 8 * scale, 2.6 * scale, -missile.angle, 0.4);
      rotation.setHeadingPitchRoll(missile.angle, flight.climbAngle, frame.time * 6);
      batches.missile.pushQuaternion(missile.x, flight.altitude, missile.y, rotation.x, rotation.y, rotation.z, rotation.w, scale, scale, scale, MISSILE_BODY.r, MISSILE_BODY.g, MISSILE_BODY.b);

      const bloom = missile.launchBloomSeconds / 0.28;
      const back = 7.2 * scale * 1.6;
      const cosPitch = Math.cos(flight.climbAngle);
      const flameX = missile.x - (Math.cos(missile.angle) * back * cosPitch);
      const flameZ = missile.y - (Math.sin(missile.angle) * back * cosPitch);
      const flameY = flight.altitude - (Math.sin(flight.climbAngle) * back);
      const flicker = 0.85 + (Math.sin((frame.time * 47) + missile.x) * 0.15);
      batches.glow.push(flameX, flameY, flameZ, -missile.angle, (9 + (bloom * 8)) * flicker, (4.5 + (bloom * 3)) * flicker, 0, MISSILE_FLAME.r * 2.4, MISSILE_FLAME.g * 2.4, MISSILE_FLAME.b * 2.4, 0.95);
      batches.glow.push(flameX, flameY, flameZ, 0, 4 + (bloom * 3), 4 + (bloom * 3), 0, MISSILE_FLAME_CORE.r * 3, MISSILE_FLAME_CORE.g * 3, MISSILE_FLAME_CORE.b * 3, 1);
    }
    for (const [missile, flight] of this.missiles) {
      if (flight.frame !== frame.frame) {
        this.missiles.delete(missile);
      }
    }
  }

  private writeDrones(drones: readonly Drone[], batches: RenderBatches, frame: FrameContext): void {
    const dt = frame.deltaSeconds;
    for (const drone of drones) {
      if (drone.removed) {
        continue;
      }
      let flight = this.drones.get(drone);
      if (!flight) {
        flight = { frame: 0, altitude: 8, bank: 0, pitch: 0, previousX: drone.x, previousY: drone.y, previousAngle: drone.angle };
        this.drones.set(drone, flight);
      }
      flight.frame = frame.frame;

      if (dt > 0) {
        const targetAltitude = drone.isExiting ? flight.altitude + (DRONE_EXIT_CLIMB_PER_SECOND * dt) : DRONE_CRUISE_ALTITUDE;
        flight.altitude = drone.isExiting ? targetAltitude : smoothTowards(flight.altitude, targetAltitude, 3, dt);
        const velocityX = (drone.x - flight.previousX) / dt;
        const velocityY = (drone.y - flight.previousY) / dt;
        const forwardSpeed = (velocityX * Math.cos(drone.angle)) + (velocityY * Math.sin(drone.angle));
        const turnRate = normalizeAngle(drone.angle - flight.previousAngle) / dt;
        flight.pitch = smoothTowards(flight.pitch, clamp(-forwardSpeed * 0.0022, -0.35, 0.35), 6, dt);
        flight.bank = smoothTowards(flight.bank, clamp(turnRate * 0.09, -0.45, 0.45), 6, dt);
      }
      flight.previousX = drone.x;
      flight.previousY = drone.y;
      flight.previousAngle = drone.angle;

      const accent = linearColor(DRONE_ACCENT_COLORS[Math.min(drone.level, DRONE_ACCENT_COLORS.length - 1)]);
      const scale = 0.752 + (drone.level * 0.025);
      batches.pushBlobShadow(drone.x, drone.y, flight.altitude, 11 * scale, 11 * scale, -drone.angle, 0.45);
      rotation.setHeadingPitchRoll(drone.angle, flight.pitch, flight.bank);
      batches.droneBody.pushQuaternion(drone.x, flight.altitude, drone.y, rotation.x, rotation.y, rotation.z, rotation.w, scale, scale, scale, accent.r, accent.g, accent.b);

      const cos = Math.cos(drone.angle);
      const sin = Math.sin(drone.angle);
      const rotorSize = (2.85 + (drone.level * 0.18)) * 2.6 * scale;
      const shimmer = 0.3 + (Math.sin((frame.time * 52) + drone.id) * 0.1);
      for (const [localX, localZ] of ROTOR_OFFSETS) {
        const worldX = drone.x + (((localX * cos) - (localZ * sin)) * scale);
        const worldZ = drone.y + (((localX * sin) + (localZ * cos)) * scale);
        batches.glow.push(worldX, flight.altitude + 1.6, worldZ, 0, rotorSize, rotorSize, 1, ROTOR_COLOR.r, ROTOR_COLOR.g, ROTOR_COLOR.b, shimmer);
      }
    }
    for (const [drone, flight] of this.drones) {
      if (flight.frame !== frame.frame) {
        this.drones.delete(drone);
      }
    }
  }
}

import { TIMER_EPSILON_SECONDS, TOWER_RADIUS, TOWER_UPGRADE_RING_GROWTH, TOWER_UPGRADE_RING_OFFSET } from "../constants";
import { DRONE_ACCENT_COLORS } from "../entities/drone-visuals";
import { getMissileScale } from "../entities/projectiles/missile-visuals";
import { DroneTower } from "../entities/towers/drone-tower";
import { GUN_PROJECTILE_SOURCE_OFFSET, GunTower, MUZZLE_FLASH_DURATION_SECONDS } from "../entities/towers/gun-tower";
import { LASER_COLORS, LaserTower } from "../entities/towers/laser-tower";
import { LIGHTNING_COLORS, LightningTower } from "../entities/towers/lightning-tower";
import { MISSILE_POWERBANK_COLORS, MissileTower } from "../entities/towers/missile-tower";
import { SlowTower } from "../entities/towers/slow-tower";
import type { Tower } from "../entities/towers/tower";
import { clamp, easeOutCubic } from "../utils";
import type { FxSystem } from "./fx-system";
import type { FrameContext } from "./frame-math";
import {
  DRONE_PAD_TOP,
  GUN_BARREL_Y,
  LASER_CRYSTAL_Y,
  MISSILE_RACK_Y,
  SLOW_CORE_Y,
  TESLA_COIL_SCALE,
  TESLA_TOP_Y,
} from "./models";
import { linearColor, type LinearColor } from "./palette";
import type { RenderBatches } from "./render-batches";

const GUN_ACCENT = linearColor("#ffe27a");
const GUN_BODY = linearColor("#e6fff4");
const GUN_POWERBANK = linearColor("#9dffd7");
const MUZZLE_HOT = linearColor("#fff7d1");
const MUZZLE_WARM = linearColor("#ff9d5c");
const MISSILE_BODY = linearColor("#ff9d5c");
const SLOW_ACCENT = linearColor("#d8ff4f");
const SLOW_CORE = linearColor("#ffdc5c");
const SLOW_NODE_WARM = linearColor("#ffe27a");
const LOCK_COLOR = linearColor("#ffe36f");
const DARK_PIP = { r: 0.012, g: 0.02, b: 0.018 };
const LEVEL_PIP_SLOTS = 6;
const LEVEL_PIP_RADIUS = 12.2;
const LEVEL_PIP_Y = 2.3;
const DRONE_READINESS_PIPS = 12;
const LASER_BEAM_LENGTH = 1000;
const MISSILE_RELOAD_TRAVEL = 22;
const DRONE_DOCK_SCALE = 0.7;
const MAX_LIGHTNING_COOLDOWN_SECONDS = 1.12;
const SELECTED_BOOST = 1.7;
// The original's base strokes: white for most towers, softer for the missile and drone pads.
const PLINTH = linearColor("#3a4a44");
const RIM_WHITE = linearColor("#ffffff");
const MISSILE_RIM = linearColor("#d7e2ea");
const DRONE_RIM = linearColor("#effff7");
const RIM_INTENSITY = 0.9;
// Upgrade halos: the original's outer ring, in each tower's ring color, widening and
// brightening with each level.
const GOLD_HALO = linearColor("#ffe27a");
const SLOW_HALO = linearColor("#d8ff4f");
const DRONE_HALO = linearColor("#9dffd7");
const HALO_Y = 0.6;
const HALO_BASE_INTENSITY = 0.32;
const HALO_INTENSITY_PER_LEVEL = 0.08;

/** Replaces every part color while drawing a placement hologram. */
export interface TowerTint {
  readonly color: LinearColor;
}

interface TowerVisual {
  frame: number;
  previousCooldown: number;
}

/** Tower 3D presentation: reads each tower's public state and composes instanced neon parts. */
export class TowerView {
  private readonly visuals = new Map<Tower, TowerVisual>();

  constructor(private readonly fx: FxSystem) {}

  reset(): void {
    this.visuals.clear();
  }

  write(towers: readonly Tower[], selected: Tower | undefined, batches: RenderBatches, frame: FrameContext): void {
    for (const tower of towers) {
      let visual = this.visuals.get(tower);
      if (!visual) {
        visual = { frame: 0, previousCooldown: tower.cooldownSeconds };
        this.visuals.set(tower, visual);
      }
      visual.frame = frame.frame;
      this.detectFiring(tower, visual);
      this.writeTower(tower, tower === selected, null, batches, frame);
    }
    for (const [tower, visual] of this.visuals) {
      if (visual.frame !== frame.frame) {
        this.visuals.delete(tower);
      }
    }
  }

  /** Draws one tower; with a tint every part uses the hologram color. */
  writeTower(tower: Tower, selected: boolean, tint: TowerTint | null, batches: RenderBatches, frame: FrameContext): void {
    const accent = getAccentColor(tower);
    const boost = selected ? SELECTED_BOOST : 1;
    // The original's near-black base fill; the rim, halo, and weapon carry the color.
    const baseColor = tint?.color ?? PLINTH;
    if (!tint) {
      batches.pushBlobShadow(tower.x, tower.y, getShadowHeight(tower), 14.5, 14.5, 0, 0.6);
    }
    batches.towerBase.pushYaw(tower.x, 0, tower.y, 0, 1, 1, 1, baseColor.r * boost, baseColor.g * boost, baseColor.b * boost);
    const rim = tint?.color ?? getRimColor(tower);
    const rimIntensity = tint ? 1 : RIM_INTENSITY * boost;
    batches.towerRim.pushYaw(tower.x, 0, tower.y, 0, 1, 1, 1, rim.r * rimIntensity, rim.g * rimIntensity, rim.b * rimIntensity);
    if (tower.level > 0) {
      const halo = tint?.color ?? getHaloColor(tower);
      const haloRadius = TOWER_RADIUS + TOWER_UPGRADE_RING_OFFSET + (tower.level * TOWER_UPGRADE_RING_GROWTH);
      const haloIntensity = (HALO_BASE_INTENSITY + (tower.level * HALO_INTENSITY_PER_LEVEL)) * boost;
      batches.upgradeRing.pushYaw(tower.x, HALO_Y, tower.y, 0, haloRadius, haloRadius, haloRadius, halo.r * haloIntensity, halo.g * haloIntensity, halo.b * haloIntensity);
    }
    this.writeLevelPips(tower, accent, tint, batches);

    if (tower instanceof GunTower) {
      this.writeGun(tower, tint, batches);
    } else if (tower instanceof LaserTower) {
      this.writeLaser(tower, tint, batches);
    } else if (tower instanceof MissileTower) {
      this.writeMissileTower(tower, tint, batches);
    } else if (tower instanceof SlowTower) {
      this.writeSlow(tower, tint, batches, frame);
    } else if (tower instanceof DroneTower) {
      this.writeDroneTower(tower, accent, tint, batches, frame);
    } else if (tower instanceof LightningTower) {
      this.writeLightning(tower, tint, batches, frame);
    }
  }

  private detectFiring(tower: Tower, visual: TowerVisual): void {
    const fired = tower.cooldownSeconds > visual.previousCooldown + 0.25;
    visual.previousCooldown = tower.cooldownSeconds;
    if (!fired) {
      return;
    }
    // Slow pulses show only through their links to the slowed monsters.
    if (tower instanceof LightningTower) {
      this.fx.zap(tower.x, TESLA_TOP_Y, tower.y);
    }
  }

  private writeLevelPips(tower: Tower, accent: LinearColor, tint: TowerTint | null, batches: RenderBatches): void {
    for (let slot = 0; slot < LEVEL_PIP_SLOTS; slot += 1) {
      const angle = ((slot / LEVEL_PIP_SLOTS) * Math.PI * 2) + (Math.PI / 2);
      const lit = slot < tower.level;
      const color = tint?.color ?? (lit ? accent : DARK_PIP);
      const intensity = lit ? 1.8 : 1;
      batches.pip.pushYaw(
        tower.x + (Math.cos(angle) * LEVEL_PIP_RADIUS),
        LEVEL_PIP_Y,
        tower.y + (Math.sin(angle) * LEVEL_PIP_RADIUS),
        -angle,
        1,
        1,
        1,
        color.r * intensity,
        color.g * intensity,
        color.b * intensity,
      );
    }
  }

  private writeGun(tower: GunTower, tint: TowerTint | null, batches: RenderBatches): void {
    const yaw = -tower.angle;
    const cos = Math.cos(tower.angle);
    const sin = Math.sin(tower.angle);
    const body = tint?.color ?? GUN_BODY;
    batches.gunHead.pushYaw(tower.x, 0, tower.y, yaw, 1, 1, 1, body.r, body.g, body.b);

    const flash = clamp(tower.muzzleFlashSeconds / MUZZLE_FLASH_DURATION_SECONDS, 0, 1);
    const recoil = flash * 2.4;
    const barrelRadius = 1 + (tower.level * 0.2);
    const front = GUN_PROJECTILE_SOURCE_OFFSET + (tower.level * 0.9) - recoil;
    const back = -2 - (tower.level * 0.55);
    const length = front - back;
    batches.gunBarrel.pushYaw(tower.x + (cos * back), GUN_BARREL_Y, tower.y + (sin * back), yaw, length, barrelRadius, barrelRadius, body.r, body.g, body.b);
    const muzzle = tint?.color ?? GUN_ACCENT;
    batches.gunMuzzle.pushYaw(tower.x + (cos * front), GUN_BARREL_Y, tower.y + (sin * front), yaw, barrelRadius, barrelRadius, barrelRadius, muzzle.r * (1 + flash * 2), muzzle.g * (1 + flash * 2), muzzle.b * (1 + flash * 2));

    if (tower.level > 0) {
      const railColor = tint?.color ?? GUN_POWERBANK;
      const railLength = 7.8 + (tower.level * 0.6);
      const railOffset = barrelRadius + 1.3;
      for (const side of [-1, 1]) {
        batches.rail.pushYaw(
          tower.x + (cos * back) - (sin * railOffset * side),
          GUN_BARREL_Y,
          tower.y + (sin * back) + (cos * railOffset * side),
          yaw,
          railLength,
          0.9 + (tower.level * 0.08),
          0.9,
          railColor.r,
          railColor.g,
          railColor.b,
        );
      }
    }

    if (flash > 0 && !tint) {
      const tipX = tower.x + (cos * (front + 2));
      const tipZ = tower.y + (sin * (front + 2));
      const flashSize = 10 + (tower.level * 0.9);
      batches.glow.push(tipX, GUN_BARREL_Y, tipZ, -tower.angle, flashSize * 1.6, flashSize * 0.9, 0, MUZZLE_WARM.r * 2, MUZZLE_WARM.g * 2, MUZZLE_WARM.b * 2, flash * 0.9);
      batches.glow.push(tipX, GUN_BARREL_Y, tipZ, 0, flashSize * 0.6, flashSize * 0.6, 0, MUZZLE_HOT.r * 3, MUZZLE_HOT.g * 3, MUZZLE_HOT.b * 3, flash);
    }
  }

  private writeLaser(tower: LaserTower, tint: TowerTint | null, batches: RenderBatches): void {
    const colors = LASER_COLORS[Math.min(tower.level, LASER_COLORS.length - 1)];
    const bodyColor = tint?.color ?? linearColor(colors.body);
    const accentColor = tint?.color ?? linearColor(colors.accent);
    const yaw = -tower.angle;
    const cos = Math.cos(tower.angle);
    const sin = Math.sin(tower.angle);
    const visualLevel = tower.level + 1;
    const muzzle = tower.getMuzzleOffset();
    const tail = -8.5 - (visualLevel * 0.36);
    const halfLength = (muzzle - tail) / 2;
    const center = (muzzle + tail) / 2;
    const girth = 3.4 + (visualLevel * 0.26);
    const charge = tower.beamAlpha;

    batches.laserCradle.pushYaw(tower.x, 0, tower.y, yaw, 1, 1, 1, accentColor.r, accentColor.g, accentColor.b);
    const crystalBoost = 1 + (charge * 1.4);
    batches.laserCrystal.pushYaw(
      tower.x + (cos * center),
      LASER_CRYSTAL_Y,
      tower.y + (sin * center),
      yaw,
      halfLength,
      girth * 0.78,
      girth,
      bodyColor.r * crystalBoost,
      bodyColor.g * crystalBoost,
      bodyColor.b * crystalBoost,
    );

    if (tower.level > 0) {
      const railLength = 6.22 + ((tower.level - 1) * 1.14);
      for (const side of [-1, 1]) {
        const offset = girth + 1.6;
        batches.rail.pushYaw(
          tower.x + (cos * -6.9) - (sin * offset * side),
          LASER_CRYSTAL_Y - 1.2,
          tower.y + (sin * -6.9) + (cos * offset * side),
          yaw,
          railLength,
          0.8,
          0.9,
          accentColor.r,
          accentColor.g,
          accentColor.b,
        );
      }
    }

    if (tower.directionLocked) {
      const lock = tint?.color ?? LOCK_COLOR;
      batches.orbNode.pushYaw(tower.x - (cos * 5.2), LASER_CRYSTAL_Y + girth * 0.8, tower.y - (sin * 5.2), 0, 1.7, 1.7, 1.7, lock.r * 1.8, lock.g * 1.8, lock.b * 1.8);
    }

    if (charge > 0 && !tint) {
      const beam = linearColor(`rgb(${colors.beam})`);
      const sourceX = tower.x + (cos * muzzle);
      const sourceZ = tower.y + (sin * muzzle);
      const outer = batches.ribbon.pushYaw(sourceX, LASER_CRYSTAL_Y, sourceZ, yaw, LASER_BEAM_LENGTH, 1, 8 + (tower.level * 0.4), beam.r * 0.5 * charge, beam.g * 0.5 * charge, beam.b * 0.5 * charge);
      batches.ribbon.setExtra(outer, 0, 0);
      const core = batches.ribbon.pushYaw(sourceX, LASER_CRYSTAL_Y + 0.05, sourceZ, yaw, LASER_BEAM_LENGTH, 1, 2 + (tower.level * 0.35), beam.r * 2.2 * charge, beam.g * 2.2 * charge, beam.b * 2.2 * charge);
      batches.ribbon.setExtra(core, 0, 0);
      const flare = 11 + (tower.level * 0.6);
      batches.glow.push(sourceX, LASER_CRYSTAL_Y, sourceZ, 0, flare, flare, 0, beam.r * 2.5, beam.g * 2.5, beam.b * 2.5, charge);
    }
  }

  private writeMissileTower(tower: MissileTower, tint: TowerTint | null, batches: RenderBatches): void {
    const powerbank = tint?.color ?? linearColor(MISSILE_POWERBANK_COLORS[Math.min(tower.level, MISSILE_POWERBANK_COLORS.length - 1)]);
    const yaw = -tower.angle;
    const cos = Math.cos(tower.angle);
    const sin = Math.sin(tower.angle);
    batches.missileLauncher.pushYaw(tower.x, 0, tower.y, yaw, 1, 1, 1, powerbank.r, powerbank.g, powerbank.b);

    const reload = easeOutCubic(tower.getReloadProgress());
    if (reload > 0.12) {
      const offset = -MISSILE_RELOAD_TRAVEL * (1 - reload);
      const scale = getMissileScale(tower.level) * 0.62;
      const body = tint?.color ?? MISSILE_BODY;
      batches.missile.pushYaw(
        tower.x + (cos * (offset + 1)),
        MISSILE_RACK_Y + 1.2,
        tower.y + (sin * (offset + 1)),
        yaw,
        scale,
        scale,
        scale,
        body.r,
        body.g,
        body.b,
      );
    }
  }

  private writeSlow(tower: SlowTower, tint: TowerTint | null, batches: RenderBatches, frame: FrameContext): void {
    const pulse = 0.8 + (Math.sin(tower.pulse) * 0.3);
    const core = tint?.color ?? SLOW_CORE;
    batches.slowCore.pushYaw(tower.x, SLOW_CORE_Y, tower.y, -tower.orbit * 0.5, 1, 1, 1, core.r * pulse, core.g * pulse, core.b * pulse);
    if (tower.level === 0) {
      return;
    }
    const nodeCount = Math.min(7, tower.level + 1);
    const orbitRadius = 7.5 + (tower.level * 0.75);
    const nodeRadius = 1.6 + (tower.level * 0.12);
    for (let index = 0; index < nodeCount; index += 1) {
      const angle = tower.orbit + ((Math.PI * 2 * index) / nodeCount);
      const color = tint?.color ?? (index % 2 === 0 ? SLOW_NODE_WARM : SLOW_ACCENT);
      batches.orbNode.pushYaw(
        tower.x + (Math.cos(angle) * orbitRadius),
        SLOW_CORE_Y + (Math.sin((angle * 2) + (frame.time * 2.5)) * 1.4),
        tower.y + (Math.sin(angle) * orbitRadius),
        0,
        nodeRadius,
        nodeRadius,
        nodeRadius,
        color.r * 1.4,
        color.g * 1.4,
        color.b * 1.4,
      );
    }
  }

  private writeDroneTower(tower: DroneTower, accent: LinearColor, tint: TowerTint | null, batches: RenderBatches, frame: FrameContext): void {
    const color = tint?.color ?? accent;
    const readiness = tower.getLaunchReadiness();
    batches.dronePad.pushYaw(tower.x, 0, tower.y, 0, 1, 1, 1, color.r, color.g, color.b);
    for (let slot = 0; slot < DRONE_READINESS_PIPS; slot += 1) {
      const angle = ((slot / DRONE_READINESS_PIPS) * Math.PI * 2) - (Math.PI / 2);
      const lit = (slot + 0.5) / DRONE_READINESS_PIPS <= readiness;
      const pipColor = tint?.color ?? (lit ? accent : DARK_PIP);
      const intensity = lit ? 1.6 : 1;
      batches.pip.pushYaw(
        tower.x + (Math.cos(angle) * 6.6),
        DRONE_PAD_TOP + 0.1,
        tower.y + (Math.sin(angle) * 6.6),
        -angle,
        0.55,
        0.4,
        0.55,
        pipColor.r * intensity,
        pipColor.g * intensity,
        pipColor.b * intensity,
      );
    }
    if (readiness >= 1 - TIMER_EPSILON_SECONDS) {
      const hover = Math.sin(frame.time * 3) * 0.4;
      batches.droneBody.pushYaw(tower.x, DRONE_PAD_TOP + 2.4 + hover, tower.y, frame.time * 0.4, DRONE_DOCK_SCALE, DRONE_DOCK_SCALE, DRONE_DOCK_SCALE, color.r, color.g, color.b);
    }
  }

  private writeLightning(tower: LightningTower, tint: TowerTint | null, batches: RenderBatches, frame: FrameContext): void {
    const coil = tint?.color ?? linearColor(LIGHTNING_COLORS[Math.min(tower.level, LIGHTNING_COLORS.length - 1)]);
    const charge = tower.cooldownSeconds <= 0 ? 1 : clamp(1 - (tower.cooldownSeconds / MAX_LIGHTNING_COOLDOWN_SECONDS), 0, 1);
    const intensity = 0.65 + (charge * 0.7);
    batches.teslaCoil.pushYaw(tower.x, 0, tower.y, frame.time * 0.6, 1, 1, 1, coil.r * intensity, coil.g * intensity, coil.b * intensity);
    if (!tint) {
      const crackle = 0.5 + (Math.sin((frame.time * 37) + tower.x) * 0.25) + (Math.sin((frame.time * 53) + tower.y) * 0.25);
      const size = (13 + (tower.level * 0.8)) * TESLA_COIL_SCALE;
      batches.glow.push(tower.x, TESLA_TOP_Y, tower.y, 0, size, size, 0, coil.r * 1.8, coil.g * 1.8, coil.b * 1.8, (0.25 + (charge * 0.45)) * crackle);
    }
  }
}

/** Rough height of each tower's mass, which sets how far its blob shadow falls. */
function getShadowHeight(tower: Tower): number {
  if (tower instanceof LightningTower) {
    return 13 * TESLA_COIL_SCALE;
  }
  if (tower instanceof SlowTower) {
    return 10;
  }
  return 7;
}

function getRimColor(tower: Tower): LinearColor {
  if (tower instanceof MissileTower) {
    return MISSILE_RIM;
  }
  if (tower instanceof DroneTower) {
    return DRONE_RIM;
  }
  return RIM_WHITE;
}

function getHaloColor(tower: Tower): LinearColor {
  if (tower instanceof LaserTower) {
    // The original drew the laser halo in its beam color.
    return linearColor(`rgb(${LASER_COLORS[Math.min(tower.level, LASER_COLORS.length - 1)].beam})`);
  }
  if (tower instanceof SlowTower) {
    return SLOW_HALO;
  }
  if (tower instanceof DroneTower) {
    return DRONE_HALO;
  }
  return GOLD_HALO;
}

function getAccentColor(tower: Tower): LinearColor {
  if (tower instanceof GunTower) {
    return GUN_ACCENT;
  }
  if (tower instanceof LaserTower) {
    return linearColor(LASER_COLORS[Math.min(tower.level, LASER_COLORS.length - 1)].body);
  }
  if (tower instanceof MissileTower) {
    return linearColor(MISSILE_POWERBANK_COLORS[Math.min(tower.level, MISSILE_POWERBANK_COLORS.length - 1)]);
  }
  if (tower instanceof SlowTower) {
    return SLOW_ACCENT;
  }
  if (tower instanceof DroneTower) {
    return linearColor(DRONE_ACCENT_COLORS[Math.min(tower.level, DRONE_ACCENT_COLORS.length - 1)]);
  }
  if (tower instanceof LightningTower) {
    return linearColor(LIGHTNING_COLORS[Math.min(tower.level, LIGHTNING_COLORS.length - 1)]);
  }
  return GUN_ACCENT;
}

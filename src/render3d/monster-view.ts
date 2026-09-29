import { BerserkerMonster } from "../entities/monsters/berserker-monster";
import { BulwarkMonster } from "../entities/monsters/bulwark-monster";
import type { Monster } from "../entities/monsters/monster";
import { PackManMonster } from "../entities/monsters/packman-monster";
import { RunnerMonster } from "../entities/monsters/runner-monster";
import { SplitterMonster } from "../entities/monsters/splitter-monster";
import { SquareMonster } from "../entities/monsters/square-monster";
import { TankMonster } from "../entities/monsters/tank-monster";
import { TriangleMonster } from "../entities/monsters/triangle-monster";
import type { LevelRuntime } from "../level-runtime";
import { clamp, normalizeAngle } from "../utils";
import type { FxSystem } from "./fx-system";
import { Quat, hash01, smoothTowards, type FrameContext } from "./frame-math";
import { linearColor, type LinearColor } from "./palette";
import type { RenderBatches } from "./render-batches";

const SPAWN_MATERIALIZE_SECONDS = 0.3;
const HIT_FLASH_DECAY_PER_SECOND = 7;
const BANK_RESPONSE_PER_SECOND = 7;
const MAX_BANK = 0.55;
const HEALTH_BAR_LIFT = 7;
const HEALTH_BAR_GAP = 3;
const HEALTH_BAR_HEIGHT = 3.2;
const ESCAPE_PROGRESS_THRESHOLD = 0.995;
const ICE = linearColor("#9ff4ff");
const BULWARK_ARMOR_GLOW = linearColor("#dff7ff");
const BULWARK_CORE_TINT = 0.6;
const HEALTH_TRACK = { r: 0.004, g: 0.01, b: 0.008 };
const BERSERKER_EMBER_COLOR = "#ffba4f";
const TANK_HULL_TOP = 0.82;

interface MonsterVisual {
  frame: number;
  phase: number;
  age: number;
  previousHeading: number;
  bank: number;
  lastHitPoints: number;
  hitFlash: number;
  centerY: number;
  emberTimer: number;
}

const rotation = new Quat();
const flip = new Quat();

function srgbToLinear(channel: number): number {
  return channel <= 0.04045 ? channel / 12.92 : Math.pow((channel + 0.055) / 1.055, 2.4);
}

/**
 * Draws every live monster through shared neon batches and turns lifecycle changes
 * (spawn, hit, kill, escape) into 3D effects. Monster classes stay renderer-agnostic;
 * this view only reads their public presentation state.
 */
export class MonsterView {
  private readonly visuals = new Map<Monster, MonsterVisual>();
  private runtime?: LevelRuntime;
  private spawnPoint = { x: 0, y: 0 };

  constructor(private readonly fx: FxSystem) {}

  reset(runtime: LevelRuntime): void {
    this.visuals.clear();
    this.runtime = runtime;
    const start = runtime.routePath?.start;
    this.spawnPoint = start ? { x: start.x, y: start.y } : { x: 0, y: 0 };
  }

  /** World height of a monster's body center, for beams and impact effects. */
  getCenterHeight(monster: Monster): number {
    return this.visuals.get(monster)?.centerY ?? (monster.radius + 3);
  }

  write(runtime: LevelRuntime, batches: RenderBatches, frame: FrameContext): void {
    if (runtime !== this.runtime) {
      this.reset(runtime);
    }

    for (const monster of runtime.monsters) {
      if (monster.removed) {
        continue;
      }
      let visual = this.visuals.get(monster);
      if (!visual) {
        visual = this.createVisual(monster);
        this.visuals.set(monster, visual);
      }
      visual.frame = frame.frame;
      this.advance(monster, visual, frame);
      this.writeMonster(monster, visual, batches, frame);
      this.writeShadow(monster, visual, batches);
      this.writeHealthBar(monster, visual, batches);
    }

    for (const [monster, visual] of this.visuals) {
      if (visual.frame === frame.frame) {
        continue;
      }
      this.resolveDeparture(monster, visual);
      this.visuals.delete(monster);
    }
  }

  private createVisual(monster: Monster): MonsterVisual {
    const seed = (monster.x * 0.137) + (monster.y * 0.271) + (this.visuals.size * 1.618) + monster.distanceAlongPath;
    const nearSpawn = Math.hypot(monster.x - this.spawnPoint.x, monster.y - this.spawnPoint.y) < 4;
    if (nearSpawn) {
      this.fx.spawnPulse(monster.x, monster.y, monster.color, monster.radius);
    }
    return {
      frame: 0,
      phase: hash01(seed) * Math.PI * 2,
      age: nearSpawn ? 0 : SPAWN_MATERIALIZE_SECONDS,
      previousHeading: monster.angle,
      bank: 0,
      lastHitPoints: monster.hitPoints,
      hitFlash: 0,
      centerY: monster.radius + 3,
      emberTimer: 0,
    };
  }

  private advance(monster: Monster, visual: MonsterVisual, frame: FrameContext): void {
    const dt = frame.deltaSeconds;
    visual.age += dt;
    if (monster.hitPoints < visual.lastHitPoints - 0.01) {
      const damage = visual.lastHitPoints - monster.hitPoints;
      const discreteHit = damage >= monster.maxHitPoints * 0.012;
      visual.hitFlash = Math.min(1, visual.hitFlash + ((damage / monster.maxHitPoints) * 5) + (discreteHit ? 0.25 : 0));
      if (monster instanceof BulwarkMonster && damage >= 3) {
        this.fx.shieldHit(monster.visualX, visual.centerY, monster.visualY, monster.radius, "#dff7ff");
      }
    }
    visual.lastHitPoints = monster.hitPoints;
    visual.hitFlash = Math.max(0, visual.hitFlash - (HIT_FLASH_DECAY_PER_SECOND * dt));
    if (dt > 0) {
      const turnRate = normalizeAngle(monster.angle - visual.previousHeading) / dt;
      const targetBank = clamp(turnRate * 0.16, -MAX_BANK, MAX_BANK);
      visual.bank = smoothTowards(visual.bank, targetBank, BANK_RESPONSE_PER_SECOND, dt);
    }
    visual.previousHeading = monster.angle;
  }

  private writeMonster(monster: Monster, visual: MonsterVisual, batches: RenderBatches, frame: FrameContext): void {
    const materialize = Math.min(1, visual.age / SPAWN_MATERIALIZE_SECONDS);
    const grow = 1 - ((1 - materialize) * (1 - materialize));
    const r = monster.radius * grow;
    const x = monster.visualX;
    const z = monster.visualY;
    const time = frame.time;
    const bob = Math.sin((time * 5.2) + visual.phase);
    const base = linearColor(monster.color);
    const slow = monster.maxSpeedPerSecond > 0 ? clamp(1 - (monster.speedPerSecond / monster.maxSpeedPerSecond), 0, 1) : 0;
    const flash = visual.hitFlash + (1 - materialize) * 1.5;
    const iceMix = slow * 0.35;
    const red = mixChannel(base.r, ICE.r, iceMix) * (1 + flash) + (flash * 0.5);
    const green = mixChannel(base.g, ICE.g, iceMix) * (1 + flash) + (flash * 0.5);
    const blue = mixChannel(base.b, ICE.b, iceMix) * (1 + flash) + (flash * 0.5);

    if (monster instanceof PackManMonster) {
      const heading = monster.angle + monster.currentBodyRotation;
      const mouth = monster.currentMouthAngle;
      const y = r + 1.2 + (bob * 0.9);
      visual.centerY = y;
      const waddle = Math.sin((time * 9) + visual.phase) * 0.08;
      rotation.setYaw(-(heading - mouth)).roll(waddle);
      batches.packmanJaw.pushQuaternion(x, y, z, rotation.x, rotation.y, rotation.z, rotation.w, r, r, r, red, green, blue);
      rotation.setYaw(-(heading + mouth)).roll(waddle);
      flip.setAxisAngle(1, 0, 0, Math.PI);
      rotation.multiply(flip);
      batches.packmanJaw.pushQuaternion(x, y, z, rotation.x, rotation.y, rotation.z, rotation.w, r, r, r, red, green, blue);
    } else if (monster instanceof SquareMonster) {
      const size = monster.getVisualRadius() * grow;
      const y = size + 4.5 + (bob * 1.3);
      visual.centerY = y;
      rotation.setYaw(-monster.rotation).roll(0.34 + (Math.sin((time * 2.1) + visual.phase) * 0.12)).pitch(0.22);
      batches.squareBody.pushQuaternion(x, y, z, rotation.x, rotation.y, rotation.z, rotation.w, size, size, size, red, green, blue);
    } else if (monster instanceof TriangleMonster) {
      const wobble = monster.currentNoseWobbleAngle;
      const y = (r * 0.4) + 4 + (bob * 1.1);
      visual.centerY = y;
      rotation.setHeadingPitchRoll(monster.angle + wobble, 0.06, visual.bank - (wobble * 1.4));
      batches.triangleBody.pushQuaternion(x, y, z, rotation.x, rotation.y, rotation.z, rotation.w, r, r, r, red, green, blue);
    } else if (monster instanceof TankMonster) {
      const rumble = Math.sin((time * 23) + visual.phase) * 0.18;
      visual.centerY = r * 0.6;
      rotation.setHeadingPitchRoll(monster.angle, rumble * 0.02, visual.bank * 0.25);
      batches.tankHull.pushQuaternion(x, rumble, z, rotation.x, rotation.y, rotation.z, rotation.w, r, r, r, red, green, blue);
      const turretOffset = r * 0.08;
      rotation.setYaw(-(monster.angle + monster.currentTurretRotation));
      batches.tankTurret.pushQuaternion(
        x + (Math.cos(monster.angle) * turretOffset),
        (TANK_HULL_TOP * r) + rumble,
        z + (Math.sin(monster.angle) * turretOffset),
        rotation.x,
        rotation.y,
        rotation.z,
        rotation.w,
        r,
        r,
        r,
        red,
        green,
        blue,
      );
    } else if (monster instanceof RunnerMonster) {
      const dash = monster.getDashPulse();
      const y = 3.4 + (bob * 0.5);
      visual.centerY = y + 1.5;
      rotation.setHeadingPitchRoll(monster.angle, 0, visual.bank * 1.3);
      batches.runnerBody.pushQuaternion(x, y, z, rotation.x, rotation.y, rotation.z, rotation.w, r * (1 + (dash * 0.3)), r, r * (1 - (dash * 0.12)), red, green, blue);
      this.writeRunnerTrail(monster, dash, y + (r * 0.2), base, batches, frame);
    } else if (monster instanceof SplitterMonster) {
      const y = (r * 0.5) + 4 + (bob * 1.2);
      visual.centerY = y;
      rotation.setYaw(-monster.rotation).roll(Math.sin((time * 1.7) + visual.phase) * 0.14);
      batches.splitterBody.pushQuaternion(x, y, z, rotation.x, rotation.y, rotation.z, rotation.w, r, r, r, red, green, blue);
    } else if (monster instanceof BerserkerMonster) {
      const motion = monster.getRageMotion();
      const stage = monster.currentRageStage;
      const tremor = stage === 2 ? Math.sin(time * 61) * 0.06 : 0;
      const y = 1.6 + (bob * 0.35);
      visual.centerY = y + (r * 0.4);
      rotation.setHeadingPitchRoll(monster.angle, stage === 2 ? -0.1 : 0, (visual.bank * 0.9) + tremor);
      const scaleX = r * motion.scaleX;
      const scaleY = r * (1 + (stage * 0.1));
      const scaleZ = r * motion.scaleY;
      batches.berserkerBody.pushQuaternion(x, y, z, rotation.x, rotation.y, rotation.z, rotation.w, scaleX, scaleY, scaleZ, red, green, blue);
      if (stage > 0) {
        const glow = 1.2 + (Math.sin(time * 14) * 0.3);
        batches.berserkerSpikes.pushQuaternion(x, y, z, rotation.x, rotation.y, rotation.z, rotation.w, scaleX, scaleY, scaleZ, red * glow, green * glow, blue * glow);
      }
      this.emitBerserkerEmbers(monster, visual, motion.emberAlpha, frame);
    } else if (monster instanceof BulwarkMonster) {
      const y = 0.6 + (Math.sin((time * 2.3) + visual.phase) * 0.3);
      visual.centerY = y + (r * 0.5);
      rotation.setHeadingPitchRoll(monster.angle, 0, visual.bank * 0.3);
      batches.bulwarkShell.pushQuaternion(x, y, z, rotation.x, rotation.y, rotation.z, rotation.w, r, r, r, red, green, blue);
      const pulse = 0.42 + (Math.sin(monster.currentShieldPulse) * 0.18) + (flash * 0.5);
      // Lean the armor glow toward the bulwark's own blue so it doesn't read as a white core.
      batches.bulwarkCore.pushQuaternion(
        x,
        y,
        z,
        rotation.x,
        rotation.y,
        rotation.z,
        rotation.w,
        r,
        r,
        r,
        mixChannel(BULWARK_ARMOR_GLOW.r, red, BULWARK_CORE_TINT) * pulse,
        mixChannel(BULWARK_ARMOR_GLOW.g, green, BULWARK_CORE_TINT) * pulse,
        mixChannel(BULWARK_ARMOR_GLOW.b, blue, BULWARK_CORE_TINT) * pulse,
      );
    }
  }

  private writeRunnerTrail(monster: RunnerMonster, dash: number, y: number, color: LinearColor, batches: RenderBatches, frame: FrameContext): void {
    const r = monster.radius;
    const cos = Math.cos(monster.angle);
    const sin = Math.sin(monster.angle);
    const length = r * (2.2 + (dash * 3.2));
    const intensity = 0.45 + (dash * 0.9);
    const flicker = 0.85 + (Math.sin((frame.time * 40) + monster.distanceAlongPath) * 0.15);
    for (const side of [-1, 1]) {
      const offset = side * r * 0.42;
      const startX = monster.visualX - (cos * r * 1.1) - (sin * offset);
      const startZ = monster.visualY - (sin * r * 1.1) + (cos * offset);
      const slot = batches.ribbon.pushYaw(startX, y, startZ, -(monster.angle + Math.PI), length, 1, 1.4 + dash, color.r * intensity * flicker, color.g * intensity * flicker, color.b * intensity * flicker);
      batches.ribbon.setExtra(slot, 0, 0);
    }
  }

  private emitBerserkerEmbers(monster: BerserkerMonster, visual: MonsterVisual, emberAlpha: number, frame: FrameContext): void {
    visual.emberTimer -= frame.deltaSeconds;
    if (visual.emberTimer > 0) {
      return;
    }
    visual.emberTimer += 0.12 / (0.3 + emberAlpha);
    const cos = Math.cos(monster.angle);
    const sin = Math.sin(monster.angle);
    const back = monster.radius * 1.1;
    const lateral = (Math.random() - 0.5) * monster.radius;
    this.fx.ember(
      monster.visualX - (cos * back) - (sin * lateral),
      visual.centerY,
      monster.visualY - (sin * back) + (cos * lateral),
      (-cos * 70) + ((Math.random() - 0.5) * 30),
      (-sin * 70) + ((Math.random() - 0.5) * 30),
      BERSERKER_EMBER_COLOR,
    );
  }

  private writeShadow(monster: Monster, visual: MonsterVisual, batches: RenderBatches): void {
    const grow = Math.min(1, visual.age / SPAWN_MATERIALIZE_SECONDS);
    const r = monster.radius * grow;
    const tank = monster instanceof TankMonster || monster instanceof BulwarkMonster;
    batches.pushBlobShadow(monster.visualX, monster.visualY, visual.centerY, r * (tank ? 1.3 : 1.1), r * (tank ? 1 : 1.1), -monster.angle, 0.75);
  }

  private writeHealthBar(monster: Monster, visual: MonsterVisual, batches: RenderBatches): void {
    const width = Math.max(16, monster.radius * 2);
    const ratio = clamp(monster.hitPoints / monster.maxHitPoints, 0, 1);
    const x = monster.visualX;
    const y = visual.centerY + monster.radius + HEALTH_BAR_LIFT;
    const z = monster.visualY - monster.radius - HEALTH_BAR_GAP;
    batches.healthBar.pushYaw(x, y, z, 0, width + 1.2, 1, HEALTH_BAR_HEIGHT + 1.2, HEALTH_TRACK.r, HEALTH_TRACK.g, HEALTH_TRACK.b);
    if (ratio <= 0) {
      return;
    }
    const fillWidth = width * ratio;
    let red: number;
    let green: number;
    let blue: number;
    if (ratio > 0.5) {
      const danger = (1 - ratio) * 2;
      red = (76 + (179 * danger)) / 255;
      green = 1;
      blue = (144 * (1 - danger)) / 255;
    } else {
      const danger = 1 - (ratio * 2);
      red = 1;
      green = (227 * (1 - danger)) / 255;
      blue = 79 / 255;
    }
    batches.healthBar.pushYaw(
      x - (width / 2) + (fillWidth / 2),
      y + 0.1,
      z,
      0,
      fillWidth,
      1,
      HEALTH_BAR_HEIGHT,
      srgbToLinear(red) * 1.3,
      srgbToLinear(green) * 1.3,
      srgbToLinear(blue) * 1.3,
    );
  }

  private resolveDeparture(monster: Monster, visual: MonsterVisual): void {
    if (monster.hitPoints <= 0) {
      const heavy = monster instanceof TankMonster || monster instanceof BulwarkMonster || monster instanceof BerserkerMonster;
      this.fx.monsterDeath(monster.visualX, visual.centerY, monster.visualY, monster.color, monster.radius, heavy);
      return;
    }
    if (monster.getPathProgress() >= ESCAPE_PROGRESS_THRESHOLD) {
      this.fx.escapeBlast(monster.x, monster.y);
    }
  }
}

function mixChannel(from: number, to: number, amount: number): number {
  return from + ((to - from) * amount);
}

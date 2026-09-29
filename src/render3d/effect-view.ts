import { EscapeFragmentParticle } from "../entities/effects/escape-fragment-particle";
import { GlassShardParticle } from "../entities/effects/glass-shard-particle";
import { HitRingEffect } from "../entities/effects/hit-ring-effect";
import { LightningLinkEffect } from "../entities/effects/lightning-link-effect";
import { LinkEffect } from "../entities/effects/link-effect";
import { EmberStreakParticle, SmokeParticle } from "../entities/effects/missile-explosion-effect";
import type { Particle } from "../entities/effects/particle";
import { ShockwaveEffect } from "../entities/effects/shockwave-effect";
import { Monster } from "../entities/monsters/monster";
import { TankTrackPrintParticle, TankTurretParticle } from "../entities/monsters/tank-effects";
import { LightningTower } from "../entities/towers/lightning-tower";
import type { LevelRuntime, RuntimeLinkEffect } from "../level-runtime";
import { randomRange } from "../utils";
import type { FxSystem } from "./fx-system";
import { Quat, type FrameContext } from "./frame-math";
import { vec3, type Vec3 } from "./math";
import { SLOW_CORE_Y, TESLA_TOP_Y } from "./models";
import type { MonsterView } from "./monster-view";
import { linearColor } from "./palette";
import type { ProjectileView } from "./projectile-view";
import type { RenderBatches } from "./render-batches";

const GRAVITY = 520;
const SHARD_RESTITUTION = 0.34;
const SHARD_GROUND_FRICTION = 0.6;
const DEFAULT_EFFECT_HEIGHT = 7.5;
const ESCAPE_SHOCKWAVE_MIN_SCALE = 1.3;
const MISSILE_TRAIL_COLORS = new Set(["#fff0a8", "#ff8f45"]);
const MISSILE_TRAIL_SMOKE = "#7e858c";
const TRACK_PRINT_COLOR = linearColor("#0a1e18");
const SLOW_LINK_COLOR = linearColor("#d8ff4f");
const SLOW_LINK_CORE = linearColor("#f4ff9a");
const FROST_ARC_COLOR = linearColor("#8ff7ff");
const LIGHTNING_CORE = linearColor("#ffffff");
const SMOKE_TINT = linearColor("#5b5a52");
const MISSILE_SHOCKWAVE = linearColor("#f99a5f");
const ESCAPE_SHOCKWAVE = linearColor("#b0ffe1");
const TRAIL_SMOKE_TINT = linearColor("#6b7178");
const LIGHTNING_SEGMENT_LENGTH = 9;

/** Renderer-side 3D state layered onto a flat simulation particle. */
interface ParticleDepth {
  frame: number;
  y: number;
  vy: number;
  grounded: boolean;
  spin: number;
  axisX: number;
  axisY: number;
  axisZ: number;
  size: number;
  shape: number;
}

interface Endpoint {
  x: number;
  y: number;
  z: number;
}

const rotation = new Quat();
const tumble = new Quat();
const segmentStart: Endpoint = { x: 0, y: 0, z: 0 };
const segmentEnd: Endpoint = { x: 0, y: 0, z: 0 };
const ribbonDirection = vec3(0, 0, 0);
const ribbonSide = vec3(0, 0, 0);
const ribbonNormal = vec3(0, 0, 0);

function shardRadius(vertices: readonly { x: number; y: number }[]): number {
  let radius = 0;
  let cx = 0;
  let cy = 0;
  for (const vertex of vertices) {
    cx += vertex.x;
    cy += vertex.y;
  }
  cx /= Math.max(1, vertices.length);
  cy /= Math.max(1, vertices.length);
  for (const vertex of vertices) {
    radius = Math.max(radius, Math.hypot(vertex.x - cx, vertex.y - cy));
  }
  return Math.max(0.8, radius);
}

/**
 * Maps the simulation's flat effect particles and links into 3D: shards fly, tumble,
 * and bounce; smoke rises; sparks arc; links become camera-facing energy ribbons.
 */
export class EffectView {
  private readonly depth = new Map<Particle, ParticleDepth>();
  private runtime?: LevelRuntime;

  constructor(
    private readonly fx: FxSystem,
    private readonly monsters: MonsterView,
    private readonly projectiles: ProjectileView,
  ) {}

  reset(): void {
    this.depth.clear();
    this.runtime = undefined;
  }

  write(runtime: LevelRuntime, batches: RenderBatches, frame: FrameContext): void {
    if (runtime !== this.runtime) {
      this.reset();
      this.runtime = runtime;
    }

    for (const particle of runtime.particles) {
      if (particle.removed) {
        continue;
      }
      let state = this.depth.get(particle);
      if (!state) {
        state = this.createDepth(particle);
        this.depth.set(particle, state);
      }
      state.frame = frame.frame;
      this.writeParticle(particle, state, batches, frame);
    }
    for (const [particle, state] of this.depth) {
      if (state.frame !== frame.frame) {
        this.depth.delete(particle);
      }
    }

    for (const link of runtime.links) {
      if (!link.removed) {
        this.writeLink(link, batches, frame);
      }
    }
  }

  private createDepth(particle: Particle): ParticleDepth {
    const axisTheta = randomRange(0, Math.PI * 2);
    const axisPhi = randomRange(0.3, Math.PI - 0.3);
    const state: ParticleDepth = {
      frame: 0,
      y: DEFAULT_EFFECT_HEIGHT,
      vy: 0,
      grounded: false,
      spin: randomRange(6, 16) * (Math.random() < 0.5 ? -1 : 1),
      axisX: Math.sin(axisPhi) * Math.cos(axisTheta),
      axisY: Math.cos(axisPhi),
      axisZ: Math.sin(axisPhi) * Math.sin(axisTheta),
      size: particle.size,
      shape: 0,
    };

    if (particle instanceof GlassShardParticle) {
      state.y = randomRange(5, 11);
      state.vy = randomRange(70, 190);
      state.size = shardRadius(particle.vertices);
    } else if (particle instanceof EscapeFragmentParticle) {
      state.y = randomRange(3, 9);
      state.vy = randomRange(110, 300);
      state.size = shardRadius(particle.vertices) * 0.9;
    } else if (particle instanceof TankTurretParticle) {
      state.y = particle.radius * 0.9;
      state.vy = randomRange(210, 280);
      state.spin = randomRange(7, 12);
    } else if (particle instanceof TankTrackPrintParticle) {
      state.y = 0.5;
    } else if (particle instanceof SmokeParticle) {
      state.y = randomRange(4, 10);
      state.vy = randomRange(10, 22);
      state.shape = Math.floor(randomRange(0, 4));
    } else if (particle instanceof EmberStreakParticle) {
      state.y = randomRange(4, 9);
      state.vy = randomRange(60, 190);
    } else if (particle instanceof ShockwaveEffect) {
      state.y = 1.2;
      if (particle.scale < ESCAPE_SHOCKWAVE_MIN_SCALE) {
        this.fx.explosion(particle.x, particle.y, particle.scale);
      }
    } else if (particle instanceof HitRingEffect) {
      state.y = DEFAULT_EFFECT_HEIGHT;
    } else if (MISSILE_TRAIL_COLORS.has(particle.color) || particle.color === MISSILE_TRAIL_SMOKE) {
      state.y = this.projectiles.getMissileAltitudeNear(particle.x, particle.y) ?? DEFAULT_EFFECT_HEIGHT;
      state.vy = particle.color === MISSILE_TRAIL_SMOKE ? randomRange(4, 10) : 0;
      state.shape = Math.floor(randomRange(0, 4));
    } else {
      state.y = DEFAULT_EFFECT_HEIGHT + randomRange(-1.5, 2.5);
      state.vy = randomRange(20, 110);
    }
    return state;
  }

  private integrateBallistic(state: ParticleDepth, deltaSeconds: number, floor: number, restitution: number): void {
    if (state.grounded || deltaSeconds <= 0) {
      return;
    }
    state.vy -= GRAVITY * deltaSeconds;
    state.y += state.vy * deltaSeconds;
    if (state.y <= floor) {
      state.y = floor;
      state.vy = -state.vy * restitution;
      state.spin *= SHARD_GROUND_FRICTION;
      if (state.vy < 26) {
        state.vy = 0;
        state.grounded = true;
      }
    }
  }

  private writeParticle(particle: Particle, state: ParticleDepth, batches: RenderBatches, frame: FrameContext): void {
    const dt = frame.deltaSeconds;
    const alpha = Math.max(0, particle.alpha);
    const color = linearColor(particle.color);

    if (particle instanceof GlassShardParticle || particle instanceof EscapeFragmentParticle) {
      const thickness = state.size * 0.55;
      this.integrateBallistic(state, dt, thickness * 0.5, SHARD_RESTITUTION);
      const heat = 0.35 + (alpha * alpha * 2.2);
      const shrink = Math.min(1, alpha * 3);
      tumble.setAxisAngle(state.axisX, state.axisY, state.axisZ, state.spin * (1 - alpha) * (state.grounded ? 0.35 : 1));
      rotation.setYaw(-particle.rotation).multiply(tumble);
      const size = state.size * shrink;
      batches.pushBlobShadow(particle.x, particle.y, state.y, size, size, 0, 0.35 * shrink);
      batches.shard.pushQuaternion(particle.x, state.y, particle.y, rotation.x, rotation.y, rotation.z, rotation.w, size, size, size, color.r * heat, color.g * heat, color.b * heat);
      return;
    }

    if (particle instanceof TankTurretParticle) {
      this.integrateBallistic(state, dt, 0.4 * particle.radius, 0.3);
      tumble.setAxisAngle(state.axisX, state.axisY, state.axisZ, state.spin * (1 - alpha) * 2.2);
      rotation.setYaw(-particle.rotation).multiply(tumble);
      const r = particle.radius * Math.min(1, alpha * 3);
      batches.pushBlobShadow(particle.x, particle.y, state.y, r * 0.8, r * 0.8, 0, 0.5 * Math.min(1, alpha * 3));
      batches.tankTurret.pushQuaternion(particle.x, state.y, particle.y, rotation.x, rotation.y, rotation.z, rotation.w, r, r, r, color.r * (0.6 + alpha), color.g * (0.6 + alpha), color.b * (0.6 + alpha));
      return;
    }

    if (particle instanceof TankTrackPrintParticle) {
      const slot = batches.decal.pushYaw(particle.x, state.y, particle.y, -particle.angle, 2.6, 1, 1.5, TRACK_PRINT_COLOR.r, TRACK_PRINT_COLOR.g, TRACK_PRINT_COLOR.b);
      batches.decal.setExtra(slot, 0, alpha * 1.5);
      batches.decal.setExtra(slot, 1, 1);
      return;
    }

    if (particle instanceof SmokeParticle) {
      state.y += state.vy * dt;
      const size = particle.size * 2.8;
      batches.smoke.push(particle.x, state.y, particle.y, state.spin * 0.05 * (1 - alpha), size, size, state.shape, SMOKE_TINT.r, SMOKE_TINT.g, SMOKE_TINT.b, alpha * 1.1);
      return;
    }

    if (particle instanceof EmberStreakParticle) {
      this.integrateBallistic(state, dt, 0.5, 0.25);
      const length = 4.5 + (particle.size * 2.35);
      const heading = Math.atan2(-particle.velocityYPerSecond, particle.velocityXPerSecond);
      batches.glow.push(particle.x, state.y, particle.y, heading, length * 1.5, particle.size * 1.2, 0, color.r * 2.4, color.g * 2.4, color.b * 2.4, alpha);
      return;
    }

    if (particle instanceof ShockwaveEffect) {
      const progress = 1 - alpha;
      const radius = (5.75 + (progress * 44)) * particle.scale;
      const warm = particle.scale < ESCAPE_SHOCKWAVE_MIN_SCALE ? MISSILE_SHOCKWAVE : ESCAPE_SHOCKWAVE;
      batches.pushGroundGlow(particle.x, state.y, particle.y, radius * 1.2, 1, warm.r * 1.8, warm.g * 1.8, warm.b * 1.8, alpha);
      batches.pushGroundGlow(particle.x, state.y, particle.y, radius * 0.9, 0, warm.r * 0.6, warm.g * 0.6, warm.b * 0.6, alpha * alpha);
      return;
    }

    if (particle instanceof HitRingEffect) {
      const ring = linearColor(particle.ringColor);
      const progress = Math.min(1, 1 - (alpha / 0.85));
      const radius = 2 + (particle.maxRadius * progress);
      batches.glow.push(particle.x, state.y, particle.y, 0, radius * 2.6, radius * 2.6, 1, ring.r * 1.8, ring.g * 1.8, ring.b * 1.8, alpha);
      return;
    }

    if (particle.color === MISSILE_TRAIL_SMOKE) {
      state.y += state.vy * dt;
      const size = 7 + ((1 - alpha) * 12);
      batches.smoke.push(particle.x, state.y, particle.y, state.spin * (1 - alpha) * 0.2, size, size, state.shape, TRAIL_SMOKE_TINT.r, TRAIL_SMOKE_TINT.g, TRAIL_SMOKE_TINT.b, alpha * 0.5);
      return;
    }

    if (MISSILE_TRAIL_COLORS.has(particle.color)) {
      const size = 3 + (particle.size * 1.8);
      batches.glow.push(particle.x, state.y, particle.y, 0, size, size, 0, color.r * 2.2, color.g * 2.2, color.b * 2.2, alpha * 0.8);
      return;
    }

    this.integrateBallistic(state, dt, 0.5, 0.35);
    const size = 2.2 + (particle.size * 2.2);
    batches.glow.push(particle.x, state.y, particle.y, 0, size, size, 0, color.r * 2.4, color.g * 2.4, color.b * 2.4, alpha);
  }

  private resolveEndpoint(source: { x: number; y: number; visualX?: number; visualY?: number }, out: Endpoint): Endpoint {
    if (source instanceof Monster) {
      out.x = source.visualX;
      out.z = source.visualY;
      out.y = this.monsters.getCenterHeight(source);
    } else if (source instanceof LightningTower) {
      out.x = source.x;
      out.z = source.y;
      out.y = TESLA_TOP_Y;
    } else {
      out.x = source.visualX ?? source.x;
      out.z = source.visualY ?? source.y;
      out.y = SLOW_CORE_Y;
    }
    return out;
  }

  private writeLink(link: RuntimeLinkEffect, batches: RenderBatches, frame: FrameContext): void {
    const level = link.source.level ?? 0;
    const alpha = Math.max(0, link.alpha);
    const from = this.resolveEndpoint(link.source, segmentStart);
    const to = this.resolveEndpoint(link.target, segmentEnd);

    if (link instanceof LightningLinkEffect) {
      const color = linearColor(link.color);
      const dx = to.x - from.x;
      const dz = to.z - from.z;
      const dy = to.y - from.y;
      const distance = Math.hypot(dx, dz);
      const segments = Math.max(2, Math.ceil(distance / LIGHTNING_SEGMENT_LENGTH));
      const normalX = distance > 0 ? -dz / distance : 0;
      const normalZ = distance > 0 ? dx / distance : 0;
      let previousX = from.x;
      let previousY = from.y;
      let previousZ = from.z;
      for (let index = 1; index <= segments; index += 1) {
        const t = index / segments;
        const envelope = index === segments ? 0 : Math.sin(Math.PI * t);
        const jitter = Math.sin((link.ageSeconds * 55) + (index * 4.31)) * 5.6 * envelope;
        const lift = Math.sin((link.ageSeconds * 47) + (index * 2.7)) * 3.2 * envelope;
        const x = from.x + (dx * t) + (normalX * jitter);
        const y = from.y + (dy * t) + lift;
        const z = from.z + (dz * t) + (normalZ * jitter);
        this.pushRibbonSegment(batches, frame, previousX, previousY, previousZ, x, y, z, 7 + (level * 0.4), color.r * alpha * 0.7, color.g * alpha * 0.7, color.b * alpha * 0.7);
        this.pushRibbonSegment(batches, frame, previousX, previousY, previousZ, x, y, z, 1.6 + (level * 0.1), LIGHTNING_CORE.r * alpha * 2.2, LIGHTNING_CORE.g * alpha * 2.2, LIGHTNING_CORE.b * alpha * 2.2);
        previousX = x;
        previousY = y;
        previousZ = z;
      }
      const arcCount = Math.min(5, 3 + level);
      const arcRadius = link.target.radius + 4 + (level * 0.32);
      const spin = link.ageSeconds * 18;
      for (let index = 0; index < arcCount; index += 1) {
        const angle = spin + ((Math.PI * 2 * index) / arcCount);
        batches.glow.push(to.x + (Math.cos(angle) * arcRadius), to.y, to.z + (Math.sin(angle) * arcRadius), -angle, 5, 1.6, 0, color.r * 2.2, color.g * 2.2, color.b * 2.2, alpha);
      }
      batches.glow.push(to.x, to.y, to.z, 0, 16 + (level * 1.2), 16 + (level * 1.2), 0, color.r * 1.6, color.g * 1.6, color.b * 1.6, alpha * 0.6);
      return;
    }

    if (link instanceof LinkEffect) {
      this.pushRibbonSegment(batches, frame, from.x, from.y, from.z, to.x, to.y, to.z, 4 + (level * 0.25), SLOW_LINK_COLOR.r * alpha * 0.6, SLOW_LINK_COLOR.g * alpha * 0.6, SLOW_LINK_COLOR.b * alpha * 0.6);
      this.pushRibbonSegment(batches, frame, from.x, from.y, from.z, to.x, to.y, to.z, 1.2 + (level * 0.12), SLOW_LINK_CORE.r * alpha * 1.4, SLOW_LINK_CORE.g * alpha * 1.4, SLOW_LINK_CORE.b * alpha * 1.4);
      const travel = (link.ageSeconds * 2.2) % 1;
      batches.glow.push(from.x + ((to.x - from.x) * travel), from.y + ((to.y - from.y) * travel), from.z + ((to.z - from.z) * travel), 0, 6, 6, 0, SLOW_LINK_CORE.r * 2, SLOW_LINK_CORE.g * 2, SLOW_LINK_CORE.b * 2, alpha);
      const pulse = Math.sin(link.ageSeconds * 18) * 0.9;
      const ringRadius = link.target.radius + 3.4 + pulse + ((1 - alpha) * 1.8);
      batches.glow.push(to.x, to.y, to.z, link.ageSeconds * 5.8, ringRadius * 2.3, ringRadius * 2.3, 1, FROST_ARC_COLOR.r * 0.6, FROST_ARC_COLOR.g * 0.6, FROST_ARC_COLOR.b * 0.6, alpha * 0.8);
    }
  }

  /** Camera-facing ribbon between two world points. */
  private pushRibbonSegment(
    batches: RenderBatches,
    frame: FrameContext,
    ax: number,
    ay: number,
    az: number,
    bx: number,
    by: number,
    bz: number,
    width: number,
    red: number,
    green: number,
    blue: number,
  ): void {
    const length = Math.hypot(bx - ax, by - ay, bz - az);
    if (length < 0.01) {
      return;
    }
    const view = frame.viewDirection;
    ribbonDirection.x = (bx - ax) / length;
    ribbonDirection.y = (by - ay) / length;
    ribbonDirection.z = (bz - az) / length;
    cross(ribbonSide, ribbonDirection, view);
    let sideLength = Math.hypot(ribbonSide.x, ribbonSide.y, ribbonSide.z);
    if (sideLength * sideLength < 1e-6) {
      ribbonSide.x = 0;
      ribbonSide.y = 0;
      ribbonSide.z = 1;
      sideLength = 1;
    }
    ribbonSide.x /= sideLength;
    ribbonSide.y /= sideLength;
    ribbonSide.z /= sideLength;
    cross(ribbonNormal, ribbonSide, ribbonDirection);
    // Keep the (single-sided) front face toward the camera; flipping the width axis flips the face.
    if ((ribbonNormal.x * view.x) + (ribbonNormal.y * view.y) + (ribbonNormal.z * view.z) > 0) {
      ribbonSide.x = -ribbonSide.x;
      ribbonSide.y = -ribbonSide.y;
      ribbonSide.z = -ribbonSide.z;
      ribbonNormal.x = -ribbonNormal.x;
      ribbonNormal.y = -ribbonNormal.y;
      ribbonNormal.z = -ribbonNormal.z;
    }
    const slot = batches.ribbon.pushBasis(
      ax,
      ay,
      az,
      ribbonDirection.x * length,
      ribbonDirection.y * length,
      ribbonDirection.z * length,
      ribbonNormal.x,
      ribbonNormal.y,
      ribbonNormal.z,
      ribbonSide.x * width,
      ribbonSide.y * width,
      ribbonSide.z * width,
      red,
      green,
      blue,
    );
    batches.ribbon.setExtra(slot, 0, 0);
  }
}

function cross(out: Vec3, a: Vec3, b: Vec3): void {
  const x = (a.y * b.z) - (a.z * b.y);
  const y = (a.z * b.x) - (a.x * b.z);
  const z = (a.x * b.y) - (a.y * b.x);
  out.x = x;
  out.y = y;
  out.z = z;
}

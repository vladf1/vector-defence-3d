import { EscapeFragmentParticle } from "../entities/effects/escape-fragment-particle";
import { HitRingEffect } from "../entities/effects/hit-ring-effect";
import { EmberStreakParticle, SmokeParticle } from "../entities/effects/missile-explosion-effect";
import { Particle } from "../entities/effects/particle";
import { ShockwaveEffect } from "../entities/effects/shockwave-effect";
import { randomRange } from "../utils";

export interface EscapeBurstConfig {
  readonly largeFragments: number;
  readonly smallFragments: number;
  readonly smokeCount: number;
  readonly speedScale: number;
}

export const ESCAPE_BURST_CONFIG: EscapeBurstConfig = {
  largeFragments: 58,
  smallFragments: 30,
  smokeCount: 18,
  speedScale: 1,
};

const ESCAPE_BURST_COLORS = ["#b0ffe1", "#6df0c2", "#ffe36f", "#f4fff8", "#7fd7ff"];

export function createHitImpactParticles(x: number, y: number, color: string, sparkAngle: number | undefined, limit: number): Particle[] {
  const particles: Particle[] = [];
  if (limit > 0) particles.push(new HitRingEffect(x, y, color, randomRange(5.5, 9)));
  for (let index = 0; index < 5 && particles.length < limit; index += 1) {
    const angle = sparkAngle === undefined ? randomRange(-Math.PI, Math.PI) : sparkAngle + randomRange(-0.95, 0.95);
    particles.push(new Particle(x, y, randomRange(0.8, 1.7), color, randomRange(4.2, 6), {
      speedPerSecond: randomRange(80, 210),
      offset: randomRange(1, 3),
      angle,
    }));
  }
  return particles;
}

export function createLaserImpactParticles(x: number, y: number, beamAngle: number, color: string, limit: number): Particle[] {
  const particles: Particle[] = [];
  for (let index = 0; index < 4 && particles.length < limit; index += 1) {
    const side = Math.random() < 0.5 ? -1 : 1;
    const angle = beamAngle + (side * (Math.PI / 2)) + randomRange(-0.55, 0.55);
    const sparkColor = index === 0 ? "#f6f0ff" : (index === 1 ? "#ffe36f" : color);
    particles.push(new Particle(x, y, randomRange(0.7, 1.45), sparkColor, randomRange(3.4, 5.2), {
      speedPerSecond: randomRange(45, 145),
      offset: randomRange(1.5, 4),
      angle,
    }));
  }
  return particles;
}

export function createMissileExplosionParticles(x: number, y: number, blastAngle: number, level: number, limit: number): Particle[] {
  const particles: Particle[] = [];

  for (let index = 0; index < 10 && particles.length < limit; index += 1) {
    particles.push(new SmokeParticle(x, y, blastAngle, level));
  }

  if (particles.length < limit) particles.push(new ShockwaveEffect(x, y, 0.8 + (0.06 * level)));

  for (let index = 0; index < 14 && particles.length < limit; index += 1) {
    particles.push(new EmberStreakParticle(x, y, blastAngle, level));
  }
  return particles;
}

export function createEscapeBurstParticles(x: number, y: number, config: EscapeBurstConfig, limit: number): Particle[] {
  const particles: Particle[] = [];
  if (limit > 0) particles.push(new ShockwaveEffect(x, y, 1.45));
  if (limit > 1) particles.push(new HitRingEffect(x, y, "#b0ffe1", 24));
  if (limit > 2) particles.push(new HitRingEffect(x, y, "#ffe36f", 12));

  for (let index = 0; index < config.largeFragments && particles.length < limit; index += 1) {
    const color = getRandomEscapeBurstColor();
    particles.push(new EscapeFragmentParticle(
      x,
      y,
      color,
      randomRange(-Math.PI, Math.PI),
      randomRange(185, 500) * config.speedScale,
      randomRange(5.5, 13),
      randomRange(2.4, 5.2),
      randomRange(3, 9),
    ));
  }

  for (let index = 0; index < config.smallFragments && particles.length < limit; index += 1) {
    const color = getRandomEscapeBurstColor();
    particles.push(new EscapeFragmentParticle(
      x,
      y,
      color,
      randomRange(-Math.PI, Math.PI),
      randomRange(260, 620) * config.speedScale,
      randomRange(2.8, 6.8),
      randomRange(1.1, 2.6),
      randomRange(2, 11),
    ));
  }

  for (let index = 0; index < config.smokeCount && particles.length < limit; index += 1) {
    particles.push(new SmokeParticle(x, y, randomRange(-Math.PI, Math.PI), 2));
  }
  return particles;
}

function getRandomEscapeBurstColor(): string {
  return ESCAPE_BURST_COLORS[Math.floor(randomRange(0, ESCAPE_BURST_COLORS.length))] ?? "#b0ffe1";
}

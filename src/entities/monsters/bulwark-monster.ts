import { GlassShardParticle } from "../effects/glass-shard-particle";
import { AudioCue } from "../../audio-manifest";
import type { UpdateContext, UpdateResult } from "../../game-engine/update-context";
import type { PathEntry } from "../../route-path";
import { randomRange } from "../../utils";
import { createDeathEffectOrigin, createPolygonShardParticles } from "./death-effect-helpers";
import { Monster } from "./monster";
import { createPolygonShardSplitter } from "./polygon-shard-splitter";

const COLOR = "#78d7ff";
const SPEED_PER_SECOND = 49;
const HIT_POINTS = 409;
const BOUNTY = 4;
const RADIUS = 9.5;
const ARMOR_PER_HIT = 3.5;
const MIN_CHIP_DAMAGE = 0.4;
const SHELL_HALF_HEIGHT = RADIUS * 0.8;
const SHELL_OUTLINE = [
  { x: RADIUS * 1.35, y: 0 },
  { x: RADIUS * 0.82, y: -SHELL_HALF_HEIGHT },
  { x: -RADIUS * 0.2, y: -RADIUS * 0.98 },
  { x: -RADIUS * 1.08, y: -SHELL_HALF_HEIGHT },
  { x: -RADIUS * 1.32, y: 0 },
  { x: -RADIUS * 1.08, y: SHELL_HALF_HEIGHT },
  { x: -RADIUS * 0.2, y: RADIUS * 0.98 },
  { x: RADIUS * 0.82, y: SHELL_HALF_HEIGHT },
];
const FRONT_PLATE_OUTLINE = [
  { x: RADIUS * 1.08, y: 0 },
  { x: RADIUS * 0.76, y: -RADIUS * 0.28 },
  { x: RADIUS * 0.16, y: -RADIUS * 0.28 },
  { x: RADIUS * 0.16, y: RADIUS * 0.28 },
  { x: RADIUS * 0.76, y: RADIUS * 0.28 },
];
const SHARD_SPLITTER = createPolygonShardSplitter({
  minShardCount: 5,
  maxShardCount: 11,
});

export class BulwarkMonster extends Monster {
  private shieldPulse = 0;

  get currentShieldPulse(): number {
    return this.shieldPulse;
  }

  constructor(path: PathEntry[], speedScale: number) {
    super(path, COLOR, SPEED_PER_SECOND * speedScale, HIT_POINTS, BOUNTY, RADIUS);
  }

  // Flat armor applies once per discrete impact. Continuous effects use
  // Monster.takeContinuousDamage() so their result cannot depend on tick rate.
  override takeDamage(amount: number): void {
    if (amount <= 0) {
      return;
    }

    const mitigated = Math.max(MIN_CHIP_DAMAGE, amount - ARMOR_PER_HIT);
    super.takeDamage(mitigated);
  }

  protected override updateSpecial(context: UpdateContext): void {
    this.shieldPulse += 2.8 * context.deltaSeconds;
  }

  override addDeathEffect(result: UpdateResult): void {
    createPolygonShardParticles(
      result,
      this,
      SHELL_OUTLINE,
      createDeathEffectOrigin(this.radius, -0.18, 0.18, -0.18, 0.18),
      this.angle,
      120,
      205,
      0,
      SHARD_SPLITTER,
    );
    if (result.remainingParticleCapacity > 0) {
      result.addParticle(new GlassShardParticle(
        this.visualX,
        this.visualY,
        this.color,
        FRONT_PLATE_OUTLINE,
        { x: 0, y: 0 },
        this.angle,
        randomRange(105, 175),
        0,
      ));
    }
    result.playSound(AudioCue.MonsterHeavyDeath, this.x, 1.05);
  }

}

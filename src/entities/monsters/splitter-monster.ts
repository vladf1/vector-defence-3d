import { AudioCue } from "../../audio-manifest";
import type { UpdateContext, UpdateResult } from "../../game-engine/update-context";
import type { PathEntry } from "../../route-path";
import { createDeathEffectOrigin, createPolygonShardParticles } from "./death-effect-helpers";
import { Monster } from "./monster";
import { createPolygonShardSplitter } from "./polygon-shard-splitter";

const COLOR = "#ff8bd5";
const SPEED_PER_SECOND = 73;
const HIT_POINTS = 304;
const BOUNTY = 3;
const RADIUS = 8.5;
const OUTLINE = Array.from({ length: 6 }, (_, index) => {
  const angle = (Math.PI / 3) * index;
  const radius = index % 2 === 0 ? RADIUS * 1.15 : RADIUS * 0.72;
  return {
    x: Math.cos(angle) * radius,
    y: Math.sin(angle) * radius,
  };
});
const SHARD_SPLITTER = createPolygonShardSplitter({
  minShardCount: 5,
  maxShardCount: 11,
});

export class SplitterMonster extends Monster {
  constructor(path: PathEntry[], speedScale: number) {
    super(path, COLOR, SPEED_PER_SECOND * speedScale, HIT_POINTS, BOUNTY, RADIUS);
  }

  protected override updateSpecial(context: UpdateContext): void {
    this.rotation += 2.7 * context.deltaSeconds;
  }

  override addDeathEffect(result: UpdateResult): void {
    createPolygonShardParticles(
      result,
      this,
      OUTLINE,
      createDeathEffectOrigin(this.radius, -0.14, 0.14, -0.14, 0.14),
      this.rotation,
      110,
      185,
      0,
      SHARD_SPLITTER,
    );
    result.playSound(AudioCue.MonsterPop, this.x, 1.1);
  }

}

import { AudioCue } from "../../audio-manifest";
import type { UpdateContext, UpdateResult } from "../../game-engine/update-context";
import type { PathEntry } from "../../route-path";
import { easeInOutSine, randomRange } from "../../utils";
import { createDeathEffectOrigin, createPolygonShardParticles } from "./death-effect-helpers";
import { Monster } from "./monster";
import { createPolygonShardSplitter } from "./polygon-shard-splitter";

const COLOR = "#ffba4f";
const SPEED_PER_SECOND = 95;
const HIT_POINTS = 132;
const BOUNTY = 3;
const RADIUS = 7;
const OUTLINE_RADIUS = 7;
const NOSE_WOBBLE_INTERVAL_MIN_SECONDS = 2;
const NOSE_WOBBLE_INTERVAL_MAX_SECONDS = 5;
const NOSE_WOBBLE_DURATION_SECONDS = 1.575;
const NOSE_WOBBLE_MAX_ANGLE = Math.PI * 0.14;
const NOSE_WOBBLE_OSCILLATIONS = 7;
const NOSE_WOBBLE_DECAY = 0.72;
const OUTLINE = [
  { x: OUTLINE_RADIUS, y: 0 },
  { x: -OUTLINE_RADIUS, y: -OUTLINE_RADIUS },
  { x: -OUTLINE_RADIUS, y: OUTLINE_RADIUS },
];
const SHARD_SPLITTER = createPolygonShardSplitter({
  minShardCount: 5,
  maxShardCount: 11,
});

export class TriangleMonster extends Monster {
  private noseWobbleAngle = 0;
  private noseWobbleElapsedSeconds = 0;
  private noseWobbleDirection = 1;
  private secondsUntilNoseWobble = randomRange(NOSE_WOBBLE_INTERVAL_MIN_SECONDS, NOSE_WOBBLE_INTERVAL_MAX_SECONDS);

  constructor(path: PathEntry[], speedScale: number) {
    super(path, COLOR, SPEED_PER_SECOND * speedScale, HIT_POINTS, BOUNTY, RADIUS);
  }

  get currentNoseWobbleAngle(): number {
    return this.noseWobbleAngle;
  }

  protected override updateSpecial(context: UpdateContext): void {
    if (this.noseWobbleElapsedSeconds > 0) {
      this.advanceNoseWobble(context.deltaSeconds);
      return;
    }

    this.secondsUntilNoseWobble -= context.deltaSeconds;
    if (this.secondsUntilNoseWobble <= 0) {
      this.noseWobbleDirection = randomRange(0, 1) < 0.5 ? -1 : 1;
      this.advanceNoseWobble(context.deltaSeconds);
    }
  }

  override addDeathEffect(result: UpdateResult): void {
    createPolygonShardParticles(
      result,
      this,
      OUTLINE,
      createDeathEffectOrigin(OUTLINE_RADIUS, -0.1, 0.14, -0.12, 0.12),
      this.angle + this.noseWobbleAngle,
      115,
      195,
      1.4,
      SHARD_SPLITTER,
    );
    result.playSound(AudioCue.MonsterShatter, this.x);
  }

  private advanceNoseWobble(deltaSeconds: number): void {
    this.noseWobbleElapsedSeconds += deltaSeconds;
    const progress = Math.min(1, this.noseWobbleElapsedSeconds / NOSE_WOBBLE_DURATION_SECONDS);
    const easedStart = easeInOutSine(Math.min(1, progress * 4));
    const easedAmplitude = easedStart * (1 - (progress * NOSE_WOBBLE_DECAY));
    const oscillation = Math.sin(progress * Math.PI * 2 * NOSE_WOBBLE_OSCILLATIONS);
    this.noseWobbleAngle = this.noseWobbleDirection * NOSE_WOBBLE_MAX_ANGLE * oscillation * easedAmplitude;

    if (progress === 1) {
      this.noseWobbleAngle = 0;
      this.noseWobbleElapsedSeconds = 0;
      this.secondsUntilNoseWobble = randomRange(NOSE_WOBBLE_INTERVAL_MIN_SECONDS, NOSE_WOBBLE_INTERVAL_MAX_SECONDS);
    }
  }

}

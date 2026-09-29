import { GameMode, type GameProfile } from "../game-profile";

/**
 * Startup-fixed rendering budget. Anything that would change shader programs (MSAA,
 * light count) is decided once here; only resolution adapts at runtime.
 */
export interface RenderQuality {
  readonly maxPixelRatio: number;
  readonly minPixelRatio: number;
  /** Upper bound on rendered pixels, so huge high-DPR windows don't render far beyond need. */
  readonly maxRenderPixels: number;
  readonly msaaSamples: number;
  readonly bloomResolution: number;
  readonly flashLights: number;
  readonly fxParticles: number;
  readonly glowSprites: number;
  readonly smokeSprites: number;
  readonly ribbons: number;
}

const DESKTOP_QUALITY: RenderQuality = {
  maxPixelRatio: 2,
  minPixelRatio: 0.85,
  maxRenderPixels: 6_000_000,
  msaaSamples: 4,
  bloomResolution: 0.5,
  flashLights: 4,
  fxParticles: 1400,
  glowSprites: 4096,
  smokeSprites: 1024,
  ribbons: 2048,
};

const MOBILE_QUALITY: RenderQuality = {
  maxPixelRatio: 2,
  minPixelRatio: 1,
  maxRenderPixels: 2_600_000,
  msaaSamples: 0,
  bloomResolution: 0.35,
  flashLights: 2,
  fxParticles: 800,
  glowSprites: 3072,
  smokeSprites: 640,
  ribbons: 1536,
};

export function selectRenderQuality(profile: GameProfile): RenderQuality {
  return profile.mode === GameMode.Mobile ? MOBILE_QUALITY : DESKTOP_QUALITY;
}

const SLOW_FRAME_MS = 19.5;
const FAST_FRAME_MS = 17.6;
const EVALUATION_WINDOW_MS = 800;
const UPGRADE_PATIENCE_MS = 5000;
const CEILING_RECOVERY_MS = 20000;
const STEP_DOWN_RATIO = 0.84;
const STEP_UP_RATIO = 1.1;
const MAX_TRACKED_INTERVAL_MS = 100;

/**
 * Adapts the render pixel ratio to keep frames near 60 fps. It steps down quickly when
 * frames run long and probes back up slowly; a ratio that proved too slow becomes a
 * ceiling that only relaxes after a long stable stretch.
 */
export class ResolutionGovernor {
  private devicePixelRatio: number;
  private pixelBudgetRatio = Infinity;
  private pixelRatio: number;
  private ceiling: number;
  private ceilingSetAt = 0;
  private windowStart = 0;
  private windowFrames = 0;
  private windowTotalMs = 0;
  private fastSince = 0;
  private lastFrameTime = 0;

  constructor(private readonly quality: RenderQuality, devicePixelRatio: number) {
    this.devicePixelRatio = devicePixelRatio;
    this.pixelRatio = this.maxRatio;
    this.ceiling = this.maxRatio;
  }

  get currentPixelRatio(): number {
    return this.pixelRatio;
  }

  private get maxRatio(): number {
    return Math.min(this.quality.maxPixelRatio, this.devicePixelRatio, this.pixelBudgetRatio);
  }

  private get minRatio(): number {
    return Math.min(this.quality.minPixelRatio, this.maxRatio);
  }

  /** Re-derives limits for a new CSS size and device pixel ratio. */
  updateViewport(cssWidth: number, cssHeight: number, devicePixelRatio: number): void {
    this.devicePixelRatio = devicePixelRatio;
    this.pixelBudgetRatio = Math.sqrt(this.quality.maxRenderPixels / Math.max(1, cssWidth * cssHeight));
    // Resume at the best ratio not already proven too slow; a smaller window may allow more.
    this.pixelRatio = Math.max(this.minRatio, Math.min(this.ceiling, this.maxRatio));
  }

  /** Records a rendered frame; returns true when the pixel ratio changed. */
  recordFrame(now: number, animating: boolean): boolean {
    const interval = now - this.lastFrameTime;
    this.lastFrameTime = now;
    if (!animating || interval <= 0 || interval > MAX_TRACKED_INTERVAL_MS) {
      this.resetWindow(now);
      return false;
    }

    this.windowFrames += 1;
    this.windowTotalMs += interval;
    if (now - this.windowStart < EVALUATION_WINDOW_MS) {
      return false;
    }

    const average = this.windowTotalMs / this.windowFrames;
    this.resetWindow(now);

    if (average > SLOW_FRAME_MS) {
      this.fastSince = 0;
      if (this.pixelRatio <= this.minRatio) {
        return false;
      }
      this.ceiling = this.pixelRatio * 0.97;
      this.ceilingSetAt = now;
      this.pixelRatio = Math.max(this.minRatio, this.pixelRatio * STEP_DOWN_RATIO);
      return true;
    }

    if (average >= FAST_FRAME_MS) {
      this.fastSince = 0;
      return false;
    }

    if (this.fastSince === 0) {
      this.fastSince = now;
    }
    if (now - this.ceilingSetAt >= CEILING_RECOVERY_MS) {
      this.ceiling = this.maxRatio;
    }
    const target = Math.min(this.ceiling, this.maxRatio, this.pixelRatio * STEP_UP_RATIO);
    if (now - this.fastSince < UPGRADE_PATIENCE_MS || target <= this.pixelRatio + 0.01) {
      return false;
    }
    this.pixelRatio = target;
    this.fastSince = now;
    return true;
  }

  private resetWindow(now: number): void {
    this.windowStart = now;
    this.windowFrames = 0;
    this.windowTotalMs = 0;
  }
}

import { getTowerClass } from "../entities/towers/tower-registry";
import type { Tower } from "../entities/towers/tower";
import type { Game } from "../game-engine";
import type { TowerKind } from "../types";
import type { FrameContext } from "./frame-math";
import { HOLOGRAM_INVALID, HOLOGRAM_VALID, linearColor } from "./palette";
import type { RenderBatches } from "./render-batches";
import type { TowerView } from "./tower-view";

const RANGE_Y = 0.7;
const SELECTED_RANGE = linearColor("#5cff9e");
// The range layer is additive, so this scale is its opacity.
const SELECTED_RANGE_INTENSITY = 0.385;
const CROSSHAIR_WIDTH = 0.9;
const HOLOGRAM_FLICKER_HZ = 7;

/** Build-mode hologram, range rings, and crosshair guides. */
export class PlacementView {
  private readonly ghosts = new Map<TowerKind, Tower>();

  constructor(private readonly game: Game, private readonly towers: TowerView) {}

  write(batches: RenderBatches, frame: FrameContext): void {
    const runtime = this.game.runtime;
    const selected = runtime.selectedTower;
    if (selected && !runtime.placingTower) {
      this.pushRange(
        batches,
        selected.x,
        selected.y,
        selected.range,
        SELECTED_RANGE.r * SELECTED_RANGE_INTENSITY,
        SELECTED_RANGE.g * SELECTED_RANGE_INTENSITY,
        SELECTED_RANGE.b * SELECTED_RANGE_INTENSITY,
      );
    }

    const pointer = runtime.pointer;
    const kind = runtime.placingTower;
    if (!pointer || !kind) {
      return;
    }

    const towerClass = getTowerClass(kind);
    const bounds = this.game.renderer.getVisibleFieldBounds();
    const valid = this.game.canPlaceTowerInBounds(pointer, bounds) && runtime.money >= towerClass.baseCost;
    const color = valid ? HOLOGRAM_VALID : HOLOGRAM_INVALID;
    const range = towerClass.baseRange * this.game.profile.towerRangeScale;
    this.pushRange(batches, pointer.x, pointer.y, range, color.r * 0.8, color.g * 0.8, color.b * 0.8);

    const guide = 0.16;
    const spanX = bounds.maxX - bounds.minX;
    const spanY = bounds.maxY - bounds.minY;
    const horizontal = batches.ribbon.pushYaw(bounds.minX, RANGE_Y, pointer.y, 0, spanX, 1, CROSSHAIR_WIDTH, color.r * guide, color.g * guide, color.b * guide);
    batches.ribbon.setExtra(horizontal, 0, 0);
    const vertical = batches.ribbon.pushYaw(pointer.x, RANGE_Y, bounds.minY, -Math.PI / 2, spanY, 1, CROSSHAIR_WIDTH, color.r * guide, color.g * guide, color.b * guide);
    batches.ribbon.setExtra(vertical, 0, 0);

    const ghost = this.getGhost(kind);
    ghost.x = pointer.x;
    ghost.y = pointer.y;
    const flicker = 0.75 + (Math.sin(frame.time * Math.PI * 2 * HOLOGRAM_FLICKER_HZ) * 0.12);
    this.towers.writeTower(ghost, false, { color: { r: color.r * flicker, g: color.g * flicker, b: color.b * flicker } }, batches, frame);
  }

  private pushRange(batches: RenderBatches, x: number, y: number, radius: number, red: number, green: number, blue: number): void {
    batches.range.pushYaw(x, RANGE_Y, y, 0, radius, 1, radius, red, green, blue);
  }

  private getGhost(kind: TowerKind): Tower {
    let ghost = this.ghosts.get(kind);
    if (!ghost) {
      const TowerClass = getTowerClass(kind);
      ghost = new TowerClass(0, 0);
      if ("angle" in ghost) {
        (ghost as Tower & { angle: number }).angle = -Math.PI / 4;
      }
      this.ghosts.set(kind, ghost);
    }
    return ghost;
  }
}

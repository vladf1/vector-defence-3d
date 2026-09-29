import type { FieldBounds, Point } from "./types";

/**
 * Presentation boundary between `Game` and a concrete board renderer.
 * Renderers read `Game.runtime` each frame; they never mutate simulation state.
 */
export interface BoardRenderer {
  resize(): void;
  /** Called when static board content (route, escape allowance) changes. */
  renderBackgroundLayer(): void;
  draw(): void;
  getVisibleFieldBounds(): FieldBounds;
  /*
   * Player camera controls. Picking follows them; visible field bounds (gameplay) never do.
   * Each returns whether the view changed.
   */
  /** Tilts toward the horizon (positive) or straight down (negative), keeping the field framed. */
  tiltBy(deltaRadians: number): boolean;
  /** Zooms by `factor` (above 1 zooms in), keeping the ground under the client point in place. */
  zoomAt(factor: number, clientX: number, clientY: number, surfaceRect: DOMRect): boolean;
  /** Pans so the ground under the `from` client point moves under the `to` client point. */
  panBetween(fromClientX: number, fromClientY: number, toClientX: number, toClientY: number, surfaceRect: DOMRect): boolean;
  /** Restores the default tilt, zoom, and pan. */
  resetView(): boolean;
  isPointInUpgradeButton(point: Point): boolean;
  isPointInLaserLockButton(point: Point): boolean;
  /** Maps a client-space pointer position over the input surface to field coordinates. */
  clientToField(clientX: number, clientY: number, surfaceRect: DOMRect): Point | null;
  dispose(): void;
}

/** Stand-in used while no board surface is mounted, or while an async renderer is loading. */
export class DetachedBoardRenderer implements BoardRenderer {
  constructor(private readonly bounds: FieldBounds) {}

  resize(): void {
  }

  renderBackgroundLayer(): void {
  }

  draw(): void {
  }

  getVisibleFieldBounds(): FieldBounds {
    return this.bounds;
  }

  tiltBy(): boolean {
    return false;
  }

  zoomAt(): boolean {
    return false;
  }

  panBetween(): boolean {
    return false;
  }

  resetView(): boolean {
    return false;
  }

  isPointInUpgradeButton(): boolean {
    return false;
  }

  isPointInLaserLockButton(): boolean {
    return false;
  }

  clientToField(): Point | null {
    return null;
  }

  dispose(): void {
  }
}

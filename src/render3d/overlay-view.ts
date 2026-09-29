import { LaserTower } from "../entities/towers/laser-tower";
import type { Game } from "../game-engine";
import type { Point } from "../types";
import type { CameraRig } from "./camera-rig";

interface ScreenRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

const BUTTON_WIDTH = 32;
const BUTTON_HEIGHT = 26;
const COMPACT_BUTTON_WIDTH = 84;
const COMPACT_GROUPED_BUTTON_WIDTH = 54;
const COMPACT_BUTTON_HEIGHT = 54;
const BUTTON_GAP = 6;
const BUTTON_OFFSET = 24;
const EDGE_GUTTER = 42;
const COMPACT_WIDTH_THRESHOLD = 520;
const ESCAPE_LABEL_HEIGHT = 3;

/**
 * Screen-space HUD drawn over the 3D board: selected-tower actions and the escape
 * allowance. Field-space hit tests are projected so they match what the player sees.
 */
export class OverlayView {
  private readonly context: CanvasRenderingContext2D;
  private width = 1;
  private height = 1;
  private readonly scratch: Point = { x: 0, y: 0 };

  constructor(
    private readonly canvas: HTMLCanvasElement,
    private readonly rig: CameraRig,
    private readonly game: Game,
  ) {
    const context = canvas.getContext("2d");
    if (!context) {
      throw new Error("Overlay canvas context unavailable.");
    }
    this.context = context;
  }

  private get compact(): boolean {
    return this.width <= COMPACT_WIDTH_THRESHOLD;
  }

  resize(width: number, height: number, pixelRatio: number): void {
    this.width = Math.max(1, width);
    this.height = Math.max(1, height);
    this.canvas.width = Math.round(this.width * pixelRatio);
    this.canvas.height = Math.round(this.height * pixelRatio);
    this.context.setTransform(pixelRatio, 0, 0, pixelRatio, 0, 0);
  }

  draw(): void {
    const context = this.context;
    context.clearRect(0, 0, this.width, this.height);
    this.drawEscapeAllowance(context);
    this.drawTowerActions(context);
  }

  isPointInUpgradeButton(point: Point): boolean {
    return this.hitTest(point, this.getActionRect(0));
  }

  isPointInLaserLockButton(point: Point): boolean {
    const selected = this.game.runtime.selectedTower;
    return selected instanceof LaserTower && this.hitTest(point, this.getActionRect(1));
  }

  private hitTest(fieldPoint: Point, rect: ScreenRect | undefined): boolean {
    if (!rect) {
      return false;
    }
    const screen = this.rig.projectToViewport(fieldPoint.x, 0, fieldPoint.y, this.scratch);
    return screen.x >= rect.x && screen.x <= rect.x + rect.width && screen.y >= rect.y && screen.y <= rect.y + rect.height;
  }

  private getActionRect(index: number): ScreenRect | undefined {
    if (!this.game.profile.ui.drawCanvasTowerActions) {
      return undefined;
    }
    const selected = this.game.runtime.selectedTower;
    if (!selected) {
      return undefined;
    }
    const actionCount = selected instanceof LaserTower ? 2 : 1;
    if (index >= actionCount) {
      return undefined;
    }

    const fieldScale = this.width / this.game.profile.fieldWidth;
    const compact = this.compact;
    const width = compact ? (actionCount > 1 ? COMPACT_GROUPED_BUTTON_WIDTH : COMPACT_BUTTON_WIDTH) : BUTTON_WIDTH * Math.max(1, fieldScale * 0.85);
    const height = compact ? COMPACT_BUTTON_HEIGHT : BUTTON_HEIGHT * Math.max(1, fieldScale * 0.85);
    const groupWidth = (width * actionCount) + (BUTTON_GAP * (actionCount - 1));
    const anchor = this.rig.projectToViewport(selected.x, 0, selected.y, this.scratch);
    const offset = BUTTON_OFFSET * Math.max(1, fieldScale);
    const centerX = Math.min(Math.max(anchor.x, EDGE_GUTTER + (groupWidth / 2)), this.width - EDGE_GUTTER - (groupWidth / 2));
    const placeAbove = anchor.y + offset + height > this.height - 8;
    const top = placeAbove ? anchor.y - offset - height - (8 * fieldScale) : anchor.y + offset;
    return {
      x: centerX - (groupWidth / 2) + (index * (width + BUTTON_GAP)),
      y: top,
      width,
      height,
    };
  }

  private drawTowerActions(context: CanvasRenderingContext2D): void {
    const upgradeRect = this.getActionRect(0);
    if (!upgradeRect) {
      return;
    }
    const pointer = this.game.runtime.pointer;
    const disabled = !this.game.canUpgradeSelectedTower();
    const hovered = pointer ? this.isPointInUpgradeButton(pointer) : false;
    context.save();
    this.drawButton(context, upgradeRect, disabled, hovered);
    const scale = upgradeRect.height / BUTTON_HEIGHT;
    const centerX = upgradeRect.x + (upgradeRect.width / 2);
    const centerY = upgradeRect.y + (upgradeRect.height / 2);
    const halfWidth = 7 * Math.min(scale, 1.6);
    const halfHeight = 6 * Math.min(scale, 1.6);
    context.beginPath();
    context.moveTo(centerX, centerY - halfHeight);
    context.lineTo(centerX + halfWidth, centerY + halfHeight);
    context.lineTo(centerX - halfWidth, centerY + halfHeight);
    context.closePath();
    context.fill();
    context.restore();

    const selected = this.game.runtime.selectedTower;
    const lockRect = this.getActionRect(1);
    if (!(selected instanceof LaserTower) || !lockRect) {
      return;
    }
    const lockHovered = pointer ? this.isPointInLaserLockButton(pointer) : false;
    context.save();
    this.drawButton(context, lockRect, !this.game.canPerformBattleAction(), lockHovered);
    context.font = `${Math.round(lockRect.height * 0.58)}px "Apple Color Emoji", "Segoe UI Emoji", system-ui, sans-serif`;
    context.textAlign = "center";
    context.textBaseline = "middle";
    context.fillText(selected.directionLocked ? "🔓" : "🔒", lockRect.x + (lockRect.width / 2), lockRect.y + (lockRect.height / 2));
    context.restore();
  }

  private drawButton(context: CanvasRenderingContext2D, rect: ScreenRect, disabled: boolean, hovered: boolean): void {
    context.globalAlpha = disabled ? 0.4 : 1;
    context.fillStyle = hovered && !disabled ? "rgba(33, 57, 50, 0.72)" : "rgba(10, 24, 20, 0.86)";
    context.strokeStyle = hovered && !disabled ? "rgba(176, 255, 225, 0.6)" : "rgba(176, 255, 225, 0.28)";
    context.lineWidth = 1;
    context.shadowColor = disabled ? "transparent" : "rgba(49, 255, 235, 0.25)";
    context.shadowBlur = hovered && !disabled ? 12 : 7;
    context.beginPath();
    context.roundRect(rect.x, rect.y, rect.width, rect.height, 7);
    context.fill();
    context.stroke();
    context.shadowBlur = 0;
    context.fillStyle = "#effff7";
  }

  private drawEscapeAllowance(context: CanvasRenderingContext2D): void {
    const level = this.game.currentLevel;
    const route = this.game.runtime.routePath;
    if (!level || !route || route.entries.length === 0) {
      return;
    }
    const exit = route.entries[route.entries.length - 1];
    const screen = this.rig.projectToViewport(exit.x, ESCAPE_LABEL_HEIGHT, exit.y, this.scratch);
    const fieldScale = this.width / this.game.profile.fieldWidth;
    const size = Math.round(19 * Math.max(0.85, Math.min(1.6, fieldScale)));
    context.save();
    context.font = `700 ${size}px Inter, system-ui, sans-serif`;
    context.textAlign = "center";
    context.textBaseline = "middle";
    context.shadowColor = "rgba(49, 255, 235, 0.4)";
    context.shadowBlur = 5;
    context.fillStyle = "rgba(238, 255, 248, 0.95)";
    context.fillText(String(Math.max(0, this.game.runtime.escapesLeft)), screen.x, screen.y + 1);
    context.restore();
  }
}

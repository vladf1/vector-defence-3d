import type { HudSnapshot } from "./types";

// The HUD shown before the engine loads; afterwards every snapshot comes from the engine
// (`crates/core/src/view.rs`).

export interface RuntimeHudStats {
  fps: number;
  frameTimeMs: number;
  updateTimeMs: number;
  drawTimeMs: number;
}

export const INITIAL_RUNTIME_HUD_STATS: RuntimeHudStats = {
  fps: 0,
  frameTimeMs: 0,
  updateTimeMs: 0,
  drawTimeMs: 0,
};

export const INITIAL_HUD_SNAPSHOT: HudSnapshot = {
  money: 0,
  waveTotal: 0,
  banner: "Awaiting orders",
  selectionName: "",
  selectionSummary: "Select a tower to view upgrades, range, and sell value.",
  upgradeLabel: "Max",
  upgradeValue: "Max",
  sellLabel: "Sell",
  sellValue: "Sell",
  upgradeDisabled: true,
  upgradeUnaffordable: false,
  hasSelectedTower: false,
  hasLaserLockAction: false,
  laserLocked: false,
  laserLockDisabled: true,
  sellDisabled: true,
  cancelBuildDisabled: true,
  canTogglePause: false,
  showStatusHud: false,
  canSkipBreak: false,
  paused: false,
  towerButtonsDisabled: true,
  availableTowers: [],
  affordableTowers: { gun: false, laser: false, missile: false, slow: false, drone: false, lightning: false },
  nerdStats: {
    fps: "0",
    frameTime: "0.0 ms",
    updateTime: "0.0 ms",
    drawTime: "0.0 ms",
    trackedObjects: "0",
    towers: "0",
    hostiles: "0",
    shots: "0",
    effects: "0",
  },
};

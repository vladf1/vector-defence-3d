import { formatMoney } from "./utils";
import { LaserTower } from "./entities/towers/laser-tower";
import { TOWER_CLASSES, getTowerClass } from "./entities/towers/tower-registry";
import type { Game } from "./game-engine";
import {
  GameState,
  ModalAction,
  TowerKind,
  type HudSnapshot,
  type ModalActionView,
  type ModalLevelCardView,
  type ModalStarAwardView,
  type ModalView,
} from "./types";

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
  affordableTowers: createAffordableTowers(0),
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

function formatTimingMs(value: number): string {
  return `${value.toFixed(3)} ms`;
}

function createAffordableTowers(money: number): Record<TowerKind, boolean> {
  const affordableTowers = {} as Record<TowerKind, boolean>;
  for (const towerClass of TOWER_CLASSES) {
    affordableTowers[towerClass.kind] = money >= towerClass.baseCost;
  }
  return affordableTowers;
}

function createBannerText(game: Game): string {
  const activeWave = game.runtime.activeWave;

  if (game.state === GameState.Playing && activeWave && game.runtime.spawnDelay > 0) {
    return `NEXT WAVE IN ${Math.ceil(game.runtime.spawnDelay)}`;
  }

  if (game.state === GameState.Won || game.state === GameState.CampaignWon || game.state === GameState.Lost) {
    return "";
  }

  if (game.bannerTimer > 0) {
    return game.bannerText;
  }

  if (game.state === GameState.Menu) {
    return "Awaiting orders";
  }

  if (game.state === GameState.Paused) {
    return "Paused";
  }

  return game.state === GameState.DefeatPending ? "Base breached" : "";
}

export function createHudSnapshot(game: Game, runtimeStats: RuntimeHudStats = INITIAL_RUNTIME_HUD_STATS): HudSnapshot {
  const currentLevel = game.currentLevel;
  const runtime = game.runtime;
  const selected = runtime.selectedTower;
  const activeWave = runtime.activeWave;
  const battleActionsDisabled = !game.canPerformBattleAction();
  const banner = createBannerText(game);

  const mobile = game.profile.mode === "mobile";
  let selectionName = "";
  let selectionSummary = "Select a tower to view upgrades, range, and sell value.";
  const upgradeValue = selected?.canUpgrade() ? formatMoney(selected.upgradeCost) : "Max";
  const sellValue = selected ? formatMoney(selected.resaleValue) : "Sell";

  if (selected) {
    const details = `Level ${selected.level + 1} · Range ${Math.round(selected.range)}`;
    selectionName = `${getTowerClass(selected.kind).label} Tower${mobile ? "" : ` · ${details}`}`;
    selectionSummary = mobile ? details : "";
  } else if (runtime.placingTower) {
    const towerClass = getTowerClass(runtime.placingTower);
    selectionName = `Placing ${towerClass.label} Tower`;
    selectionSummary = mobile ? `Tap field to build · ${formatMoney(towerClass.baseCost)}` : towerClass.summary;
  }

  const shotsTracked = runtime.projectiles.length + runtime.missiles.length + runtime.drones.length;
  const effectsTracked = runtime.particles.length + runtime.links.length;
  const trackedObjects = runtime.towers.length + runtime.monsters.length + shotsTracked + effectsTracked;
  const upgradeUnaffordable = selected !== undefined
    && selected.canUpgrade()
    && runtime.money < selected.upgradeCost;

  return {
    levelNumber: currentLevel?.levelNumber,
    money: runtime.money,
    waveCurrent: activeWave ? runtime.currentWaveIndex + 1 : undefined,
    waveTotal: runtime.waveTotal,
    waveMonstersSpawned: activeWave ? Math.min(runtime.waveSpawnedMonsters, activeWave.count) : undefined,
    waveMonsterTotal: activeWave?.count,
    banner,
    selectionName,
    selectionSummary,
    upgradeLabel: selected?.canUpgrade() ? `Upgrade - ${upgradeValue}` : "Max",
    upgradeValue,
    sellLabel: selected ? `Sell - ${sellValue}` : "Sell",
    sellValue,
    upgradeDisabled: !selected || !selected.canUpgrade() || runtime.money < selected.upgradeCost || battleActionsDisabled,
    upgradeUnaffordable,
    hasSelectedTower: selected !== undefined,
    hasLaserLockAction: selected instanceof LaserTower,
    laserLocked: selected instanceof LaserTower && selected.directionLocked,
    laserLockDisabled: !(selected instanceof LaserTower) || battleActionsDisabled,
    sellDisabled: !selected || battleActionsDisabled,
    cancelBuildDisabled: !runtime.placingTower || battleActionsDisabled,
    canTogglePause: game.state === GameState.Playing || game.state === GameState.Paused,
    showStatusHud: currentLevel !== undefined && game.state !== GameState.Menu,
    canSkipBreak: game.state === GameState.Playing
      && !!activeWave
      && runtime.spawnDelay > 0
      && !battleActionsDisabled,
    paused: game.state === GameState.Paused,
    placingTower: runtime.placingTower,
    towerButtonsDisabled: battleActionsDisabled,
    availableTowers: currentLevel?.availableTowers ?? [],
    affordableTowers: createAffordableTowers(runtime.money),
    nerdStats: {
      fps: String(Math.max(0, Math.round(runtimeStats.fps))),
      frameTime: `${runtimeStats.frameTimeMs.toFixed(1)} ms`,
      updateTime: formatTimingMs(runtimeStats.updateTimeMs),
      drawTime: formatTimingMs(runtimeStats.drawTimeMs),
      trackedObjects: String(trackedObjects),
      towers: String(runtime.towers.length),
      hostiles: String(runtime.monsters.length),
      shots: String(shotsTracked),
      effects: String(effectsTracked),
    },
  };
}

export function createModalView(game: Game): ModalView | null {
  if (game.state === GameState.Menu) {
    const actions: ModalActionView[] = [
      {
        action: game.menuReturnState && game.currentLevel ? ModalAction.Resume : ModalAction.PlayUnlocked,
        label: game.menuReturnState && game.currentLevel
          ? "Resume Battle"
          : (game.campaignCleared ? "Replay Campaign" : "Play Next"),
      },
    ];

    if (game.highestUnlockedLevelIndex > 0 || game.campaignCleared) {
      actions.push({
        action: ModalAction.RestartCampaign,
        label: "Restart Campaign",
      });
    }

    return {
      title: "Campaign Map",
      description: `${game.campaignLevelCount} campaign battles. Clear one route to unlock the next`,
      actions,
      levelCards: createModalLevelCards(game),
    };
  }

  if (game.state === GameState.Won) {
    return {
      title: "Level Clear",
      description: `Level ${game.currentLevel?.levelNumber ?? "?"} secured. Next route unlocked!`,
      sheet: true,
      starAward: createModalStarAward(game),
      actions: [
        { action: ModalAction.NextLevel, label: `Continue to Level ${(game.currentLevel?.levelNumber ?? 0) + 1}` },
        { action: ModalAction.CampaignMap, label: "Campaign Map" },
        { action: ModalAction.Replay, label: "Replay" },
      ],
    };
  }

  if (game.state === GameState.CampaignWon) {
    return {
      title: "You Won the Campaign",
      description: `All ${game.campaignLevelCount} campaign levels are secure`,
      sheet: true,
      starAward: createModalStarAward(game),
      actions: [
        { action: ModalAction.RestartCampaign, label: "Restart Campaign" },
        { action: ModalAction.CampaignMap, label: "Campaign Map" },
        { action: ModalAction.Replay, label: "Replay" },
      ],
    };
  }

  if (game.state === GameState.Lost) {
    return {
      title: "Defeat",
      description: "Too many enemies reached the exit. Better luck next time!",
      sheet: true,
      actions: [
        { action: ModalAction.Replay, label: "Try Again" },
        { action: ModalAction.CampaignMap, label: "Campaign Map" },
      ],
    };
  }

  return null;
}

export function performModalAction(game: Game, action: ModalAction): void {
  switch (action) {
    case ModalAction.Resume:
      game.resumeBattle();
      break;
    case ModalAction.PlayUnlocked:
      game.startLevelByIndex(game.campaignCleared ? 0 : game.highestUnlockedLevelIndex);
      break;
    case ModalAction.RestartCampaign:
      game.restartCampaign();
      break;
    case ModalAction.NextLevel:
      game.startNextLevel();
      break;
    case ModalAction.Replay:
      game.restart();
      break;
    case ModalAction.CampaignMap:
      game.openMenu();
      break;
    default:
      assertNever(action);
  }
}

function assertNever(value: never): never {
  throw new Error(`Unhandled modal action: ${value}`);
}

function createModalLevelCards(game: Game): ModalLevelCardView[] {
  return game.levels.map((level, index) => {
    const unlocked = game.debugAllLevelsUnlocked || game.campaignCleared || index <= game.highestUnlockedLevelIndex;
    const cleared = game.campaignCleared || index < game.highestUnlockedLevelIndex;
    const current = game.currentLevelIndex === index && !!game.currentLevel;
    const nextIndex = Math.min(game.highestUnlockedLevelIndex, game.campaignLevelCount - 1);
    const status = !unlocked
      ? "Locked"
      : (cleared ? "Cleared" : (index === nextIndex ? "Next" : "Ready"));

    return {
      index,
      unlocked,
      cleared,
      current,
      stars: game.levelStars[index] ?? 0,
      status,
      title: `${level.levelNumber} - ${level.name}`,
      description: (level.subtitle ?? "Hold the route.").replace(/\.$/, ""),
      summary: `${level.waves.length} waves · ${level.monsterCount} enemies`,
      starsLabel: `${formatStarCount(game.levelStars[index] ?? 0)} best clear`,
    };
  });
}

function createModalStarAward(game: Game): ModalStarAwardView {
  const bestStars = game.currentLevelIndex >= 0 ? (game.levelStars[game.currentLevelIndex] ?? 0) : 0;
  return {
    stars: game.lastAwardedStars,
    title: awardTitle(game.lastAwardedStars),
    description: awardCopy(game.lastAwardedStars, bestStars),
    label: `${formatStarCount(game.lastAwardedStars)} awarded`,
    perfect: game.lastAwardedStars === 3,
  };
}

function formatStarCount(stars: number): string {
  return `${stars} star${stars === 1 ? "" : "s"}`;
}

function awardTitle(stars: number): string {
  if (stars === 3) {
    return "Perfect route";
  }
  if (stars === 2) {
    return "Strong clear";
  }
  return "Route secured";
}

function awardCopy(stars: number, bestStars: number): string {
  if (bestStars > stars) {
    return `Best clear remains ${formatStarCount(bestStars)}.`;
  }
  if (stars === 3) {
    return "No leaks. Full control.";
  }
  if (stars === 2) {
    return "Cleared with escape room to spare.";
  }
  return "Replay for a cleaner defense.";
}

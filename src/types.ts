export const GameState = {
  Menu: "menu",
  Playing: "playing",
  Paused: "paused",
  DefeatPending: "defeat-pending",
  Won: "won",
  Lost: "lost",
  CampaignWon: "campaign-won",
} as const;

export type GameState = typeof GameState[keyof typeof GameState];

export const MonsterKind = {
  PackMan: "packman",
  Square: "square",
  Triangle: "triangle",
  Tank: "tank",
  Runner: "runner",
  Splitter: "splitter",
  Berserker: "berserker",
  Bulwark: "bulwark",
} as const;

export type MonsterKind = typeof MonsterKind[keyof typeof MonsterKind];

export const TowerKind = {
  Gun: "gun",
  Laser: "laser",
  Missile: "missile",
  Slow: "slow",
  Drone: "drone",
  Lightning: "lightning",
} as const;

export type TowerKind = typeof TowerKind[keyof typeof TowerKind];

export const ModalAction = {
  Resume: "resume",
  PlayUnlocked: "play-unlocked",
  RestartCampaign: "restart-campaign",
  NextLevel: "next-level",
  Replay: "replay",
  CampaignMap: "campaign-map",
} as const;

export type ModalAction = typeof ModalAction[keyof typeof ModalAction];

export interface Point {
  x: number;
  y: number;
}

export interface FieldBounds {
  minX: number;
  minY: number;
  maxX: number;
  maxY: number;
}

export type LevelJsonPoint = [number, number];

export interface WaveData {
  count: number;
  monsterSequence: MonsterKind[];
  spawnIntervalMin: number;
  spawnIntervalMax: number;
  buildTime: number;
  reward: number;
}

export interface CampaignRouteData {
  name: string;
  allowEscape: number;
  monsterSequence: MonsterKind[];
  points: Point[];
  availableTowers: TowerKind[];
  subtitle?: string;
  initialBuildTime?: number;
  startingMoney: number;
  waveCount?: number;
}

export interface LevelData extends Omit<CampaignRouteData, "monsterSequence"> {
  monsterCount: number;
  id: string;
  levelNumber: number;
  waves: WaveData[];
}

export interface LevelJsonData {
  name: string;
  subtitle?: string;
  initialBuildTime?: number;
  startingMoney: number;
  waveCount?: number;
  allowEscape: number;
  monsterSequence: string[];
  availableTowers: string[];
  points: LevelJsonPoint[];
  mobile?: Partial<Omit<LevelJsonData, "mobile">>;
}

export interface HudSnapshot {
  levelNumber?: number;
  money: number;
  waveCurrent?: number;
  waveTotal: number;
  waveMonstersSpawned?: number;
  waveMonsterTotal?: number;
  banner: string;
  selectionName: string;
  selectionSummary: string;
  upgradeLabel: string;
  upgradeValue: string;
  sellLabel: string;
  sellValue: string;
  upgradeDisabled: boolean;
  upgradeUnaffordable: boolean;
  hasSelectedTower: boolean;
  hasLaserLockAction: boolean;
  laserLocked: boolean;
  laserLockDisabled: boolean;
  sellDisabled: boolean;
  cancelBuildDisabled: boolean;
  canTogglePause: boolean;
  showStatusHud: boolean;
  canSkipBreak: boolean;
  paused: boolean;
  placingTower?: TowerKind;
  towerButtonsDisabled: boolean;
  availableTowers: TowerKind[];
  affordableTowers: Record<TowerKind, boolean>;
  nerdStats: NerdStatsSnapshot;
}

export interface NerdStatsSnapshot {
  fps: string;
  frameTime: string;
  updateTime: string;
  drawTime: string;
  trackedObjects: string;
  towers: string;
  hostiles: string;
  shots: string;
  effects: string;
}

export interface ModalActionView {
  action: ModalAction;
  label: string;
}

export interface ModalStarAwardView {
  stars: number;
  title: string;
  description: string;
  label: string;
  perfect: boolean;
}

export interface ModalLevelCardView {
  index: number;
  unlocked: boolean;
  cleared: boolean;
  current: boolean;
  stars: number;
  status: string;
  title: string;
  description: string;
  summary: string;
  starsLabel: string;
}

export interface ModalView {
  title: string;
  description: string;
  actions: ModalActionView[];
  sheet?: boolean;
  levelCards?: ModalLevelCardView[];
  starAward?: ModalStarAwardView;
}

import {
  FIELD_HEIGHT,
  FIELD_WIDTH,
  MAX_TOWER_LEVEL,
  ROAD_TURN_RADIUS,
  ROAD_WIDTH,
  ROUTE_CURVE_SAMPLE_STEP,
  TOWER_RADIUS,
  TOWER_ROAD_EDGE_OVERLAP_ALLOWANCE,
  TOWER_UPGRADE_RING_GROWTH,
  TOWER_UPGRADE_RING_OFFSET,
} from "./constants";
import type { PlacementGeometry } from "./placement-rules";

export const GameMode = {
  Desktop: "desktop",
  Mobile: "mobile",
} as const;

export type GameMode = typeof GameMode[keyof typeof GameMode];

export interface GameProfile {
  mode: GameMode;
  fieldWidth: number;
  fieldHeight: number;
  fieldAspectRatio: string;
  fieldAspectScale: number;
  towerRadius: number;
  towerSelectionPadding: number;
  towerRangeScale: number;
  monsterSpeedScale: number;
  roadTurnRadius: number;
  roadWidth: number;
  routeCurveSampleStep: number;
  placement: PlacementGeometry;
  ui: {
    drawCanvasTowerActions: boolean;
    showShortcutLabels: boolean;
    showTitle: boolean;
    showFootnote: boolean;
    portraitOnly: boolean;
    /** Board camera controls: drag to pan, wheel/pinch to zoom, Shift+wheel or up/down to tilt. */
    allowViewControls: boolean;
  };
}

function createProfile(options: {
  mode: GameMode;
  fieldWidth: number;
  fieldHeight: number;
  towerRadius: number;
  towerSelectionPadding: number;
  towerRangeScale: number;
  monsterSpeedScale: number;
  minDistanceToOtherTowers: number;
  roadTurnRadius: number;
  roadWidth: number;
  routeCurveSampleStep: number;
  ui: GameProfile["ui"];
}): GameProfile {
  const maxTowerBodyRadius = options.towerRadius
    + TOWER_UPGRADE_RING_OFFSET
    + (MAX_TOWER_LEVEL * TOWER_UPGRADE_RING_GROWTH);

  const placement = {
    bounds: { minX: 0, minY: 0, maxX: options.fieldWidth, maxY: options.fieldHeight },
    towerRadius: options.towerRadius,
    towerSelectionPadding: options.towerSelectionPadding,
    minDistanceToOtherTowers: options.minDistanceToOtherTowers,
    minDistanceToRoad: (options.roadWidth / 2) + maxTowerBodyRadius - TOWER_ROAD_EDGE_OVERLAP_ALLOWANCE,
  };

  return {
    mode: options.mode,
    fieldWidth: options.fieldWidth,
    fieldHeight: options.fieldHeight,
    fieldAspectRatio: `${options.fieldWidth} / ${options.fieldHeight}`,
    fieldAspectScale: options.fieldWidth / options.fieldHeight,
    towerRadius: options.towerRadius,
    towerSelectionPadding: options.towerSelectionPadding,
    towerRangeScale: options.towerRangeScale,
    monsterSpeedScale: options.monsterSpeedScale,
    roadTurnRadius: options.roadTurnRadius,
    roadWidth: options.roadWidth,
    routeCurveSampleStep: options.routeCurveSampleStep,
    placement,
    ui: options.ui,
  };
}

export const DESKTOP_GAME_PROFILE = createProfile({
  mode: GameMode.Desktop,
  fieldWidth: FIELD_WIDTH,
  fieldHeight: FIELD_HEIGHT,
  towerRadius: TOWER_RADIUS,
  towerRangeScale: 1,
  monsterSpeedScale: 1,
  minDistanceToOtherTowers: 32,
  towerSelectionPadding: 6,
  roadTurnRadius: ROAD_TURN_RADIUS,
  roadWidth: ROAD_WIDTH,
  routeCurveSampleStep: ROUTE_CURVE_SAMPLE_STEP,
  ui: {
    drawCanvasTowerActions: true,
    showShortcutLabels: true,
    showTitle: true,
    showFootnote: true,
    portraitOnly: false,
    allowViewControls: true,
  },
});

export const MOBILE_GAME_PROFILE = createProfile({
  mode: GameMode.Mobile,
  fieldWidth: 390,
  fieldHeight: 560,
  towerRadius: TOWER_RADIUS,
  towerRangeScale: 0.9,
  monsterSpeedScale: 0.9,
  minDistanceToOtherTowers: 27,
  towerSelectionPadding: 12,
  roadTurnRadius: 34,
  roadWidth: 25,
  routeCurveSampleStep: 4,
  ui: {
    drawCanvasTowerActions: false,
    showShortcutLabels: false,
    showTitle: false,
    showFootnote: false,
    portraitOnly: true,
    allowViewControls: false,
  },
});

export function selectStartupGameProfile(viewport: Window): GameProfile {
  const coarsePointer = viewport.matchMedia("(hover: none) and (pointer: coarse)").matches;
  const shortestSide = Math.min(viewport.innerWidth, viewport.innerHeight);
  const portrait = viewport.innerHeight >= viewport.innerWidth;
  const phoneWidth = shortestSide <= 480;
  const touchCapable = viewport.navigator.maxTouchPoints > 0;
  const portraitPhoneViewport = portrait && phoneWidth;

  return (coarsePointer && touchCapable && phoneWidth) || portraitPhoneViewport
    ? MOBILE_GAME_PROFILE
    : DESKTOP_GAME_PROFILE;
}

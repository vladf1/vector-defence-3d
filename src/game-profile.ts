// Page-shell side of the desktop/mobile profiles: layout, UI flags, and startup selection.
// Gameplay geometry (tower radius, placement, speed and range scales, road shape) lives in
// the engine (`crates/core/src/profile.rs`), which picks the same profile from `mode`.

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
  ui: {
    showShortcutLabels: boolean;
    showTitle: boolean;
    showFootnote: boolean;
    portraitOnly: boolean;
    /** Board camera controls: drag to pan, wheel/pinch to zoom, Shift+wheel or up/down to tilt. */
    allowViewControls: boolean;
  };
}

function createProfile(mode: GameMode, fieldWidth: number, fieldHeight: number, ui: GameProfile["ui"]): GameProfile {
  return {
    mode,
    fieldWidth,
    fieldHeight,
    fieldAspectRatio: `${fieldWidth} / ${fieldHeight}`,
    fieldAspectScale: fieldWidth / fieldHeight,
    ui,
  };
}

export const DESKTOP_GAME_PROFILE = createProfile(GameMode.Desktop, 800, 450, {
  showShortcutLabels: true,
  showTitle: true,
  showFootnote: true,
  portraitOnly: false,
  allowViewControls: true,
});

export const MOBILE_GAME_PROFILE = createProfile(GameMode.Mobile, 390, 560, {
  showShortcutLabels: false,
  showTitle: false,
  showFootnote: false,
  portraitOnly: true,
  allowViewControls: false,
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

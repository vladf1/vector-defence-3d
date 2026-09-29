// Top-bar control icons as neon-tube vector strokes (256 x 256 view box), traced from the
// former PNG artwork: each stroke is drawn as stacked layers, widest and darkest first.

export type ControlIconKind = "home" | "pause" | "play" | "sound-on" | "sound-muted";

interface StrokeLayer {
  readonly color: string;
  readonly width: number;
  readonly opacity: number;
}

export interface ControlIconStroke {
  readonly d: string;
  readonly layers: readonly StrokeLayer[];
}

const layer = (color: string, width: number, opacity: number): StrokeLayer => ({ color, width, opacity });

/** Green-rimmed cyan tube with a white core (the main outlines). */
const TUBE = [
  layer("#07f85c", 16.5, 1),
  layer("#07f98c", 13.5, 1),
  layer("#04f7c4", 11.5, 1),
  layer("#00f2f4", 9.5, 1),
  layer("#fdfdfd", 6, 1),
];

/** Hairline cyan tube for inner detail lines. */
const THIN = [
  layer("#04f29e", 6, 1),
  layer("#0ff0e1", 4, 1),
  layer("#a0f5f2", 2, 1),
];

/** Lime sound waves; graded translucent layers fake the soft halo falloff. */
const LIME = [
  layer("#b2ff44", 18, 0.1),
  layer("#b2ff44", 15, 0.12),
  layer("#b2ff44", 12, 0.16),
  layer("#a8ff3f", 9, 1),
  layer("#efffdf", 2.4, 1),
];

/** Magenta mute slash; graded translucent layers fake the soft halo falloff. */
const MAGENTA = [
  layer("#fc00f0", 30, 0.12),
  layer("#fc00f0", 26, 0.14),
  layer("#fc00f0", 22, 0.18),
  layer("#ff22f2", 17, 1),
  layer("#ffe8ff", 6.4, 1),
];

const SPEAKER: readonly ControlIconStroke[] = [
  { d: "M19 88H60V163.5H19Z", layers: TUBE },
  { d: "M60 88L138 23.7V227L60 163.5", layers: TUBE },
  { d: "M162.3 79.3A63.6 63.6 0 0 1 162.3 176.7", layers: LIME },
  { d: "M187.9 58.5A100 100 0 0 1 187.9 197.5", layers: LIME },
];

export const CONTROL_ICONS: Record<ControlIconKind, readonly ControlIconStroke[]> = {
  home: [
    { d: "M52 222V135H22L128.75 29.3L235.5 135H205V222H152V160.5H105.5V222Z", layers: TUBE },
    { d: "M87 204H70.5V137.5L58.5 132.5L128.75 62.9L199 132.5L187 137.5V204H171", layers: THIN },
    { d: "M75 137L120 94.5H137.5L182.5 137", layers: THIN },
  ],
  pause: [
    { d: "M68 20.5H96.5A6 6 0 0 1 102.5 26.5V223A6 6 0 0 1 96.5 229H68A6 6 0 0 1 62 223V26.5A6 6 0 0 1 68 20.5Z", layers: TUBE },
    { d: "M157.5 20.5H185.5A6 6 0 0 1 191.5 26.5V223A6 6 0 0 1 185.5 229H157.5A6 6 0 0 1 151.5 223V26.5A6 6 0 0 1 157.5 20.5Z", layers: TUBE },
  ],
  play: [
    { d: "M53 26Q53 18.6 59.6 23.2L198.9 120.6Q205.4 125.2 198.9 129.8L59.6 227.3Q53 231.9 53 224.5Z", layers: TUBE },
  ],
  "sound-on": SPEAKER,
  "sound-muted": [...SPEAKER, { d: "M212 44L47.5 212", layers: MAGENTA }],
};

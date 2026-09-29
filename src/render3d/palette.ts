/** A linear-space RGB triple; entity colors are authored as sRGB CSS strings. */
export interface LinearColor {
  readonly r: number;
  readonly g: number;
  readonly b: number;
}

const cache = new Map<string, LinearColor>();
let normalizer: CanvasRenderingContext2D | null | undefined;

export function srgbToLinear(channel: number): number {
  return channel <= 0.04045 ? channel * 0.0773993808 : Math.pow((channel * 0.9478672986) + 0.0521327014, 2.4);
}

/** sRGB channels (0..255) from `#rgb`, `#rrggbb` (alpha digits ignored) or `rgb()`/`rgba()`. */
function parseSrgb(css: string): [number, number, number] | null {
  const text = css.trim();
  if (text.startsWith("#")) {
    const hex = text.slice(1);
    if (hex.length === 3 || hex.length === 4) {
      return [0, 1, 2].map((index) => parseInt(hex[index] + hex[index], 16)) as [number, number, number];
    }
    if (hex.length === 6 || hex.length === 8) {
      return [0, 2, 4].map((index) => parseInt(hex.slice(index, index + 2), 16)) as [number, number, number];
    }
    return null;
  }
  const functional = /^rgba?\(\s*([\d.]+)[\s,]+([\d.]+)[\s,]+([\d.]+)/i.exec(text);
  return functional ? [Number(functional[1]), Number(functional[2]), Number(functional[3])] : null;
}

/** Any other CSS color (named, hsl, ...) is normalized through a 2D context once. */
function normalizeCss(css: string): string {
  if (normalizer === undefined) {
    normalizer = document.createElement("canvas").getContext("2d");
  }
  if (!normalizer) {
    return "#ffffff";
  }
  normalizer.fillStyle = "#ffffff";
  normalizer.fillStyle = css;
  return String(normalizer.fillStyle);
}

/** Parses (once) any CSS color string into linear RGB. */
export function linearColor(css: string): LinearColor {
  let color = cache.get(css);
  if (!color) {
    const srgb = parseSrgb(css) ?? parseSrgb(normalizeCss(css)) ?? [255, 255, 255];
    color = { r: srgbToLinear(srgb[0] / 255), g: srgbToLinear(srgb[1] / 255), b: srgbToLinear(srgb[2] / 255) };
    cache.set(css, color);
  }
  return color;
}

export const HOLOGRAM_VALID = linearColor("#5cff9e");
export const HOLOGRAM_INVALID = linearColor("#ff6a6a");

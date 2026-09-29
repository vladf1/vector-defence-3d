// Page-shell helpers; gameplay math lives in the engine (`crates/core/src/utils.rs`).

export function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

export function formatMoney(value: number): string {
  return `$${Math.round(value)}`;
}

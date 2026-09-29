export const RendererStatus = {
  Loading: "loading",
  Ready: "ready",
  Failed: "failed",
} as const;

export type RendererStatus = typeof RendererStatus[keyof typeof RendererStatus];

/** `?timings` shows the startup phase breakdown on the board (for profiling devices). */
export function shouldShowStartupTimings(viewport: Window): boolean {
  return new URLSearchParams(viewport.location.search).has("timings");
}

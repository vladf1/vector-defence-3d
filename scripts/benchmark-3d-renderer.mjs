// Measures the 3D board renderer: time until precompiled and ready, pipeline count, and
// per-frame CPU cost under a crowded late-campaign fight driven by the real session loop.
// All pipelines are created at startup, so gameplay can never compile new ones.
// Usage: node scripts/benchmark-3d-renderer.mjs [--mobile] [--seconds=N]
import { WEBGPU_LAUNCH_ARGS, buildReleaseEngine, runBrowserPage } from "./benchmark-browser-harness.mjs";

buildReleaseEngine();

const mobile = process.argv.includes("--mobile");
const secondsArgument = process.argv.find((argument) => argument.startsWith("--seconds="));
const seconds = secondsArgument ? Number(secondsArgument.split("=")[1]) : 8;
const viewport = mobile ? { width: 390, height: 844 } : { width: 1400, height: 900 };

const report = await runBrowserPage({
  path: "/",
  viewport,
  deviceScaleFactor: 2,
  launchArgs: WEBGPU_LAUNCH_ARGS,
}, async (page) => {
  const started = Date.now();
  await page.waitForFunction(() => window.__vectorDefence && !document.querySelector(".board-loading"), undefined, { timeout: 60_000 });
  const readyMs = Date.now() - started;

  return page.evaluate(async (durationSeconds) => {
    const { game, sync } = window.__vectorDefence;
    const towers = game.stageBenchmarkFight(9, 160, 6);
    sync();

    const drawSamples = [];
    const intervals = [];
    const counts = { particles: 0, links: 0, instances: 0, calls: 0, frames: 0 };
    const originalDraw = game.draw.bind(game);
    let lastFrame = 0;
    game.draw = () => {
      const start = performance.now();
      originalDraw();
      const end = performance.now();
      drawSamples.push(end - start);
      if (lastFrame !== 0) {
        intervals.push(start - lastFrame);
      }
      lastFrame = start;
      const stats = JSON.parse(game.debugStats());
      counts.particles += stats.particles;
      counts.links += stats.links;
      counts.instances += stats.instances;
      counts.calls += stats.drawCalls;
      counts.frames += 1;
    };
    await new Promise((resolve) => setTimeout(resolve, durationSeconds * 1000));
    game.draw = originalDraw;
    game.togglePause();
    sync();

    const stats = JSON.parse(game.debugStats());
    const sorted = [...drawSamples].sort((a, b) => a - b);
    const percentile = (p) => sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * p))] ?? 0;
    const average = (values) => values.reduce((sum, value) => sum + value, 0) / Math.max(1, values.length);
    return {
      pipelines: stats.pipelines,
      towers,
      frames: counts.frames,
      averageDrawMs: average(drawSamples),
      p95DrawMs: percentile(0.95),
      averageFrameIntervalMs: average(intervals),
      averageParticles: counts.particles / Math.max(1, counts.frames),
      averageLinks: counts.links / Math.max(1, counts.frames),
      averageInstances: counts.instances / Math.max(1, counts.frames),
      averageDrawCalls: counts.calls / Math.max(1, counts.frames),
      pixelRatio: stats.pixelRatio,
    };
  }, seconds).then((result) => ({ readyMs, ...result }));
});

const format = (value) => (typeof value === "number" ? value.toFixed(value % 1 === 0 ? 0 : 2) : JSON.stringify(value));
console.log(`3D renderer benchmark (${mobile ? "mobile" : "desktop"} profile)`);
for (const [key, value] of Object.entries(report)) {
  console.log(`  ${key}: ${format(value)}`);
}

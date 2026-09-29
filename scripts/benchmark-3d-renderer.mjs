// Measures the 3D board renderer: time until precompiled and ready, pipeline count, and
// per-frame CPU cost under a crowded late-campaign fight driven by the real session loop.
// All pipelines are created at startup, so gameplay can never compile new ones.
// Usage: node scripts/benchmark-3d-renderer.mjs [--mobile] [--seconds=N]
import { WEBGPU_LAUNCH_ARGS, runBrowserPage } from "./benchmark-browser-harness.mjs";

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
    const renderer = game.renderer;
    const pipelines = renderer.startupTimings.pipelines;

    const { createMonster } = await import("/src/game-engine/monster-factory.ts");
    const { createPathEntriesFromDistance } = await import("/src/route-path.ts");
    game.debugAllLevelsUnlocked = true;
    game.startLevelByIndex(9);
    game.runtime.money = 999999;
    game.runtime.escapesLeft = 99999;
    const entries = game.runtime.routePath.entries;
    const pathLength = entries[entries.length - 1].totalDistance;
    const kinds = game.currentLevel.availableTowers;
    const distanceToPath = (point) => entries.reduce((best, entry) => Math.min(best, Math.hypot(entry.x - point.x, entry.y - point.y)), Infinity);
    let towerIndex = 0;
    for (let y = 20; y < game.profile.fieldHeight - 10; y += 13) {
      for (let x = 20; x < game.profile.fieldWidth - 10; x += 13) {
        const point = { x, y };
        const distance = distanceToPath(point);
        if (distance < 26 || distance > 60 || !game.canPlaceTower(point)) {
          continue;
        }
        const kind = kinds[towerIndex % kinds.length];
        if (game.placeTower(kind, point)) {
          const tower = game.runtime.towers[game.runtime.towers.length - 1];
          for (let level = 0; level < 6; level += 1) {
            tower.upgrade();
          }
          towerIndex += 1;
        }
      }
    }
    game.runtime.selectedTower = undefined;
    const monsterKinds = ["packman", "square", "triangle", "tank", "runner", "splitter", "berserker", "bulwark"];
    const refill = () => {
      while (game.runtime.monsters.length < 160) {
        const index = game.runtime.monsters.length;
        const monster = createMonster(
          monsterKinds[index % monsterKinds.length],
          createPathEntriesFromDistance(entries, Math.random() * pathLength * 0.85),
          game.profile.monsterSpeedScale,
          9,
        );
        monster.hitPoints *= 6;
        monster.maxHitPoints = monster.hitPoints;
        game.runtime.monsters.push(monster);
      }
    };
    refill();
    game.runtime.spawnDelay = 0;
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
      counts.particles += game.runtime.particles.length;
      counts.links += game.runtime.links.length;
      counts.instances += renderer.batches.drawnInstances;
      counts.calls += renderer.frameDrawCalls;
      counts.frames += 1;
      refill();
    };
    await new Promise((resolve) => setTimeout(resolve, durationSeconds * 1000));
    game.draw = originalDraw;
    game.togglePause();
    sync();

    const sorted = [...drawSamples].sort((a, b) => a - b);
    const percentile = (p) => sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * p))] ?? 0;
    const average = (values) => values.reduce((sum, value) => sum + value, 0) / Math.max(1, values.length);
    return {
      pipelines,
      towers: game.runtime.towers.length,
      frames: counts.frames,
      averageDrawMs: average(drawSamples),
      p95DrawMs: percentile(0.95),
      averageFrameIntervalMs: average(intervals),
      averageParticles: counts.particles / Math.max(1, counts.frames),
      averageLinks: counts.links / Math.max(1, counts.frames),
      averageInstances: counts.instances / Math.max(1, counts.frames),
      averageDrawCalls: counts.calls / Math.max(1, counts.frames),
      pixelRatio: renderer.pixelRatio,
    };
  }, seconds).then((result) => ({ readyMs, ...result }));
});

const format = (value) => (typeof value === "number" ? value.toFixed(value % 1 === 0 ? 0 : 2) : JSON.stringify(value));
console.log(`3D renderer benchmark (${mobile ? "mobile" : "desktop"} profile)`);
for (const [key, value] of Object.entries(report)) {
  console.log(`  ${key}: ${format(value)}`);
}

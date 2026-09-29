// Renders staged 3D battle frames (overview, close-ups, and an explosion sequence) with
// the real game session and WebGPU renderer, writing PNGs under artifacts/3d-board/.
// The session's animation loop is frozen and Math.random seeded, so runs are repeatable.
// Usage: node scripts/render-3d-board.mjs [--mobile] [--level=N] [--out=DIR]
import path from "node:path";
import { mkdir } from "node:fs/promises";
import { WEBGPU_LAUNCH_ARGS, repoRoot, runBrowserPage } from "./benchmark-browser-harness.mjs";

const mobile = process.argv.includes("--mobile");
const levelArgument = process.argv.find((argument) => argument.startsWith("--level="));
const levelIndex = levelArgument ? Math.max(0, Number(levelArgument.split("=")[1]) - 1) : 6;
const outArgument = process.argv.find((argument) => argument.startsWith("--out="));
const outputDir = outArgument ? path.resolve(outArgument.slice("--out=".length)) : path.join(repoRoot, "artifacts", "3d-board");
const viewport = mobile ? { width: 390, height: 844 } : { width: 1400, height: 900 };

await mkdir(outputDir, { recursive: true });

const report = await runBrowserPage({
  path: "/",
  viewport,
  deviceScaleFactor: 2,
  launchArgs: WEBGPU_LAUNCH_ARGS,
  forwardConsole: true,
}, async (page) => {
  await page.waitForFunction(() => window.__vectorDefence && !document.querySelector(".board-loading"), undefined, { timeout: 60_000 });
  // Freeze the session's frame loop: every simulation step and draw below is scripted.
  await page.evaluate(async () => {
    window.requestAnimationFrame = () => 0;
    await new Promise((resolve) => setTimeout(resolve, 150));
  });

  const stage = await page.evaluate(async (level) => {
    const { game, sync } = window.__vectorDefence;
    const { createMonster } = await import("/src/game-engine/monster-factory.ts");
    const { createPathEntriesFromDistance } = await import("/src/route-path.ts");
    let seed = 7;
    Math.random = () => ((seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0) / 4294967296);

    game.debugAllLevelsUnlocked = true;
    game.startLevelByIndex(level);
    game.runtime.money = 99999;
    game.runtime.spawnDelay = 999;
    game.runtime.escapesLeft = 999;
    const entries = game.runtime.routePath.entries;
    const pathLength = entries[entries.length - 1].totalDistance;
    const kinds = game.currentLevel.availableTowers;
    const distanceToPath = (point) => entries.reduce((best, entry) => Math.min(best, Math.hypot(entry.x - point.x, entry.y - point.y)), Infinity);
    const placed = [];
    let kindIndex = 0;
    for (let y = 26; y < game.profile.fieldHeight - 20 && placed.length < 12; y += 17) {
      for (let x = 26; x < game.profile.fieldWidth - 20 && placed.length < 12; x += 17) {
        const point = { x, y };
        const distance = distanceToPath(point);
        if (distance < 27 || distance > 36 || placed.some((other) => Math.hypot(other.x - x, other.y - y) < 64) || !game.canPlaceTower(point)) {
          continue;
        }
        const kind = kinds[kindIndex % kinds.length];
        if (game.placeTower(kind, point)) {
          const tower = game.runtime.towers[game.runtime.towers.length - 1];
          const level = (kindIndex * 2) % 7;
          for (let upgrade = 0; upgrade < level; upgrade += 1) {
            tower.upgrade();
          }
          placed.push({ x, y, kind, level });
          kindIndex += 1;
        }
      }
    }
    game.runtime.selectedTower = undefined;

    const monsterKinds = ["packman", "square", "triangle", "tank", "runner", "splitter", "berserker", "bulwark"];
    const lineup = [];
    monsterKinds.forEach((kind, index) => {
      const distance = pathLength * (0.08 + (index * 0.1));
      const monster = createMonster(kind, createPathEntriesFromDistance(entries, distance), game.profile.monsterSpeedScale, level);
      monster.hitPoints = monster.maxHitPoints * (1 - (index * 0.09));
      game.runtime.monsters.push(monster);
      lineup.push({ kind, x: monster.x, y: monster.y });
    });

    sync();
    return { placed, lineup };
  }, levelIndex);

  const step = async (seconds) => page.evaluate((duration) => {
    const { game } = window.__vectorDefence;
    const frames = Math.round(duration * 60);
    for (let frame = 0; frame < frames; frame += 1) {
      game.updateSimulation(1 / 60);
    }
    game.state = "paused";
    game.draw();
    return game.runtime.monsters.length;
  }, seconds);

  const board = await page.locator(".board-depth").boundingBox();
  const shots = [];
  const capture = async (name, clip) => {
    const file = path.join(outputDir, `${mobile ? "mobile-" : ""}${name}.png`);
    await page.screenshot({ path: file, clip: clip ?? board });
    shots.push(file);
  };
  const centerClip = (width, height) => ({
    x: board.x + ((board.width - width) / 2),
    y: board.y + ((board.height - height) / 2),
    width,
    height,
  });
  // Moves only the render camera (same pitch, closer) over a field point, draws, and restores.
  const closeUp = async (name, fieldX, fieldY, visibleHeight) => {
    await page.evaluate(async ([x, y, height]) => {
      const { game } = window.__vectorDefence;
      const { BOARD_TILT_RADIANS } = await import("/src/render3d/camera-rig.ts");
      game.renderer.inspect({ x, y, visibleHeight: height, yaw: 0, tilt: BOARD_TILT_RADIANS });
      game.draw();
    }, [fieldX, fieldY, visibleHeight]);
    await capture(name, centerClip(Math.min(board.width, 560), Math.min(board.height, 420)));
    await page.evaluate(() => {
      const { game } = window.__vectorDefence;
      game.renderer.inspect(null);
      game.draw();
    });
  };

  await page.evaluate(() => {
    window.__vectorDefence.game.state = "playing";
  });
  await step(1.4);
  await capture("overview");
  const live = await page.evaluate(() => window.__vectorDefence.game.runtime.monsters
    .filter((monster) => !monster.removed)
    .map((monster) => ({ name: monster.constructor.name, x: monster.visualX, y: monster.visualY })));
  for (const monster of live) {
    await closeUp(`monster-${monster.name}`, monster.x, monster.y, 46);
  }
  for (const tower of stage.placed.slice(0, 8)) {
    await closeUp(`tower-${tower.kind}-${tower.level}`, tower.x, tower.y, 58);
  }

  const blast = await page.evaluate(async () => {
    const { game } = window.__vectorDefence;
    const { Missile } = await import("/src/entities/projectiles/missile.ts");
    const { createMissileVisual } = await import("/src/entities/projectiles/missile-visuals.ts");
    const target = game.runtime.monsters.find((monster) => !monster.removed);
    if (!target) {
      return null;
    }
    for (const monster of game.runtime.monsters) {
      if (Math.hypot(monster.x - target.x, monster.y - target.y) < 70) {
        monster.hitPoints = 1;
      }
    }
    game.runtime.missiles.push(new Missile({ x: target.x - 40, y: target.y - 40 }, target, 4, createMissileVisual(4)));
    return { x: target.x, y: target.y };
  });
  if (blast) {
    for (const [index, seconds] of [0.2, 0.06, 0.1, 0.16, 0.3, 0.5].entries()) {
      await page.evaluate(() => { window.__vectorDefence.game.state = "playing"; });
      await step(seconds);
      await closeUp(`explosion-${index}`, blast.x, blast.y, 190);
    }
  }
  const tank = await page.evaluate(async () => {
    const { game } = window.__vectorDefence;
    const { createMonster } = await import("/src/game-engine/monster-factory.ts");
    const { createPathEntriesFromDistance } = await import("/src/route-path.ts");
    const entries = game.runtime.routePath.entries;
    const monster = createMonster("tank", createPathEntriesFromDistance(entries, entries[entries.length - 1].totalDistance * 0.45), game.profile.monsterSpeedScale, 6);
    game.runtime.monsters.push(monster);
    game.state = "playing";
    for (let frame = 0; frame < 30; frame += 1) {
      game.updateSimulation(1 / 60);
    }
    monster.hitPoints = 0;
    return { x: monster.x, y: monster.y };
  });
  for (const [index, seconds] of [0.05, 0.12, 0.25, 0.45].entries()) {
    await page.evaluate(() => { window.__vectorDefence.game.state = "playing"; });
    await step(seconds);
    await closeUp(`tank-death-${index}`, tank.x, tank.y, 150);
  }

  const exit = await page.evaluate(async () => {
    const { game } = window.__vectorDefence;
    const { createMonster } = await import("/src/game-engine/monster-factory.ts");
    const { createPathEntriesFromDistance } = await import("/src/route-path.ts");
    const entries = game.runtime.routePath.entries;
    const end = entries[entries.length - 1];
    const monster = createMonster("runner", createPathEntriesFromDistance(entries, end.totalDistance - 12), game.profile.monsterSpeedScale, 6);
    monster.hitPoints = 1e9;
    monster.maxHitPoints = 1e9;
    game.runtime.monsters.push(monster);
    game.state = "playing";
    return { x: end.x, y: end.y };
  });
  for (const [index, seconds] of [0.2, 0.1, 0.2, 0.4].entries()) {
    await page.evaluate(() => { window.__vectorDefence.game.state = "playing"; });
    await step(seconds);
    await closeUp(`breach-${index}`, exit.x, exit.y, 230);
  }
  return { shots: shots.length };
});

console.log(`Rendered ${report.shots} images to ${path.relative(repoRoot, outputDir)}`);

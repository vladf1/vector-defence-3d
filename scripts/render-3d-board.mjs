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

  const stage = await page.evaluate((level) => {
    const { game, sync } = window.__vectorDefence;
    game.seedRandom(7);
    game.debugStartLevel(level);
    game.debugSetEconomy(99999, 999, 999);
    const kinds = game.availableTowers();
    const placed = [];
    let kindIndex = 0;
    for (let y = 26; y < game.fieldHeight() - 20 && placed.length < 12; y += 17) {
      for (let x = 26; x < game.fieldWidth() - 20 && placed.length < 12; x += 17) {
        const distance = game.distanceToRoute(x, y);
        if (distance < 27 || distance > 36 || placed.some((other) => Math.hypot(other.x - x, other.y - y) < 64) || !game.canPlaceTower(x, y)) {
          continue;
        }
        const kind = kinds[kindIndex % kinds.length];
        if (game.placeTower(kind, x, y)) {
          const level = (kindIndex * 2) % 7;
          game.debugUpgradeSelected(level);
          placed.push({ x, y, kind, level });
          kindIndex += 1;
        }
      }
    }
    game.debugClearSelection();

    const pathLength = game.routeLength();
    const monsterKinds = ["packman", "square", "triangle", "tank", "runner", "splitter", "berserker", "bulwark"];
    monsterKinds.forEach((kind, index) => {
      const id = game.debugSpawnMonster(kind, pathLength * (0.08 + (index * 0.1)), level);
      game.debugSetMonsterHitPoints(id, game.debugMonsterMaxHitPoints(id) * (1 - (index * 0.09)), Number.NaN);
    });

    sync();
    return { placed };
  }, levelIndex);

  const step = async (seconds) => page.evaluate((duration) => {
    const { game } = window.__vectorDefence;
    const frames = Math.round(duration * 60);
    for (let frame = 0; frame < frames; frame += 1) {
      game.updateSimulation(1 / 60);
    }
    game.debugSetState("paused");
    game.draw();
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
    await page.evaluate(([x, y, height]) => {
      const { game } = window.__vectorDefence;
      game.inspect(x, y, height, 0, game.boardTiltRadians());
      game.draw();
    }, [fieldX, fieldY, visibleHeight]);
    await capture(name, centerClip(Math.min(board.width, 560), Math.min(board.height, 420)));
    await page.evaluate(() => {
      const { game } = window.__vectorDefence;
      game.clearInspect();
      game.draw();
    });
  };

  await page.evaluate(() => {
    window.__vectorDefence.game.debugSetState("playing");
  });
  await step(1.4);
  await capture("overview");
  const live = await page.evaluate(() => JSON.parse(window.__vectorDefence.game.debugMonsters())
    .filter((monster) => !monster.removed));
  for (const monster of live) {
    await closeUp(`monster-${monster.kind}`, monster.x, monster.y, 46);
  }
  for (const tower of stage.placed.slice(0, 8)) {
    await closeUp(`tower-${tower.kind}-${tower.level}`, tower.x, tower.y, 58);
  }

  const blast = await page.evaluate(() => {
    const { game } = window.__vectorDefence;
    const monsters = JSON.parse(game.debugMonsters()).filter((monster) => !monster.removed);
    const target = monsters[0];
    if (!target) {
      return null;
    }
    for (const monster of monsters) {
      if (Math.hypot(monster.x - target.x, monster.y - target.y) < 70) {
        game.debugSetMonsterHitPoints(monster.id, 1, Number.NaN);
      }
    }
    game.debugLaunchMissile(target.x - 40, target.y - 40, target.id, 4);
    return { x: target.x, y: target.y };
  });
  if (blast) {
    for (const [index, seconds] of [0.2, 0.06, 0.1, 0.16, 0.3, 0.5].entries()) {
      await page.evaluate(() => { window.__vectorDefence.game.debugSetState("playing"); });
      await step(seconds);
      await closeUp(`explosion-${index}`, blast.x, blast.y, 190);
    }
  }
  const tank = await page.evaluate(() => {
    const { game } = window.__vectorDefence;
    const id = game.debugSpawnMonster("tank", game.routeLength() * 0.45, 6);
    game.debugSetState("playing");
    for (let frame = 0; frame < 30; frame += 1) {
      game.updateSimulation(1 / 60);
    }
    const monster = JSON.parse(game.debugMonsters()).find((candidate) => candidate.id === id);
    game.debugSetMonsterHitPoints(id, 0, Number.NaN);
    return { x: monster.x, y: monster.y };
  });
  for (const [index, seconds] of [0.05, 0.12, 0.25, 0.45].entries()) {
    await page.evaluate(() => { window.__vectorDefence.game.debugSetState("playing"); });
    await step(seconds);
    await closeUp(`tank-death-${index}`, tank.x, tank.y, 150);
  }

  const exit = await page.evaluate(() => {
    const { game } = window.__vectorDefence;
    const id = game.debugSpawnMonster("runner", game.routeLength() - 12, 6);
    game.debugSetMonsterHitPoints(id, 1e9, 1e9);
    game.debugSetState("playing");
    const [x, y] = game.routeEnd();
    return { x, y };
  });
  for (const [index, seconds] of [0.2, 0.1, 0.2, 0.4].entries()) {
    await page.evaluate(() => { window.__vectorDefence.game.debugSetState("playing"); });
    await step(seconds);
    await closeUp(`breach-${index}`, exit.x, exit.y, 230);
  }
  return { shots: shots.length };
});

console.log(`Rendered ${report.shots} images to ${path.relative(repoRoot, outputDir)}`);

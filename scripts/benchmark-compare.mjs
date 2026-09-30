// Before/after benchmark that works on this checkout and on older TypeScript-engine checkouts
// (`--root=DIR`), so both are measured the same way:
//  - download: every file the production page fetches before the board is ready (raw, gzip -9, brotli)
//  - startup: production build served locally, fresh browser per run, until the board is ready
//  - frame cost: dev server, the crowded late-campaign fight from benchmark:3d, measured two ways:
//    main-thread script time per rendered frame (Chrome's ScriptDuration) and a tight loop of
//    `updateSimulation(1/60)` + `draw()` (CPU only; the GPU is not waited on)
// Usage: node scripts/benchmark-compare.mjs [--root=DIR] [--runs=5] [--cpu=4] [--mobile] [--skip-frames] [--skip-production]
import { createReadStream } from "node:fs";
import { readFile, stat } from "node:fs/promises";
import { createServer as createHttpServer } from "node:http";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { brotliCompressSync, constants as zlibConstants, gzipSync } from "node:zlib";
import { chromium } from "playwright";
import { buildReleaseEngine } from "./benchmark-browser-harness.mjs";

const argument = (name, fallback) => {
  const match = process.argv.find((value) => value.startsWith(`--${name}=`));
  return match ? match.slice(name.length + 3) : fallback;
};
const root = path.resolve(argument("root", path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..")));
const runs = Number(argument("runs", "5"));
const cpuThrottle = Number(argument("cpu", "1"));
const mobile = process.argv.includes("--mobile");
const skipFrames = process.argv.includes("--skip-frames");
const skipProduction = process.argv.includes("--skip-production");
const viewport = mobile ? { width: 390, height: 844 } : { width: 1400, height: 900 };
const launchArgs = ["--enable-unsafe-webgpu", "--enable-gpu", "--ignore-gpu-blocklist"];
const MIME = { ".html": "text/html", ".js": "text/javascript", ".css": "text/css", ".wasm": "application/wasm", ".svg": "image/svg+xml", ".m4a": "audio/mp4", ".json": "application/json" };

function serveDirectory(directory) {
  const server = createHttpServer(async (request, response) => {
    const url = new URL(request.url, "http://localhost");
    let file = path.join(directory, decodeURIComponent(url.pathname));
    try {
      if ((await stat(file)).isDirectory()) {
        file = path.join(file, "index.html");
      }
      await stat(file);
    } catch {
      response.writeHead(404).end();
      return;
    }
    response.writeHead(200, { "Content-Type": MIME[path.extname(file)] ?? "application/octet-stream", "Cache-Control": "no-store" });
    createReadStream(file).pipe(response);
  });
  return new Promise((resolve) => server.listen(0, "127.0.0.1", () => resolve(server)));
}

async function launch() {
  try {
    return await chromium.launch({ channel: "chromium", args: launchArgs });
  } catch (error) {
    if (!String(error).includes("Executable doesn't exist")) {
      throw error;
    }
    return chromium.launch({ channel: "chrome", args: launchArgs });
  }
}

async function newPage(browser) {
  const page = await browser.newPage({ viewport, deviceScaleFactor: 2 });
  if (cpuThrottle > 1) {
    const session = await page.context().newCDPSession(page);
    await session.send("Emulation.setCPUThrottlingRate", { rate: cpuThrottle });
  }
  return page;
}

const boardReady = () => document.querySelector(".board-canvas") && !document.querySelector(".board-loading");

async function measureProduction() {
  const dist = path.join(root, "dist");
  const server = await serveDirectory(dist);
  const origin = `http://127.0.0.1:${server.address().port}`;
  const readyTimes = [];
  const files = new Map();
  try {
    for (let run = 0; run < runs; run += 1) {
      const browser = await launch();
      try {
        const page = await newPage(browser);
        page.on("response", (response) => {
          const url = new URL(response.url());
          if (url.origin === origin && run === 0) {
            files.set(url.pathname, true);
          }
        });
        await page.goto(`${origin}/`, { waitUntil: "domcontentloaded" });
        await page.waitForFunction(boardReady, undefined, { timeout: 120_000 });
        readyTimes.push(await page.evaluate(() => performance.now()));
      } finally {
        await browser.close();
      }
    }
  } finally {
    server.close();
  }

  const download = { raw: 0, gzip: 0, brotli: 0, files: [] };
  for (const pathname of [...files.keys()].sort()) {
    const bytes = await readFile(path.join(dist, pathname === "/" ? "index.html" : pathname));
    const gzip = gzipSync(bytes, { level: 9 }).length;
    const brotli = brotliCompressSync(bytes, { params: { [zlibConstants.BROTLI_PARAM_QUALITY]: 11 } }).length;
    download.raw += bytes.length;
    download.gzip += gzip;
    download.brotli += brotli;
    download.files.push({ file: pathname, raw: bytes.length, gzip, brotli });
  }
  readyTimes.sort((a, b) => a - b);
  return { download, readyMs: { median: readyTimes[Math.floor(readyTimes.length / 2)], min: readyTimes[0], max: readyTimes.at(-1) } };
}

/** Stages the crowded fight through whichever engine the checkout has, then measures frames. */
async function measureFrames() {
  // The dev server's labs engine is built without LTO for the edit loop; measure the release one.
  buildReleaseEngine(root);
  const { createServer } = await import(path.join(root, "node_modules/vite/dist/node/index.js"));
  const server = await createServer({ root, configFile: path.join(root, "vite.config.ts"), logLevel: "error", server: { host: "127.0.0.1" } });
  await server.listen(0);
  const browser = await launch();
  try {
    const page = await newPage(browser);
    const session = await page.context().newCDPSession(page);
    await session.send("Performance.enable");
    await page.goto(server.resolvedUrls.local[0], { waitUntil: "domcontentloaded" });
    await page.waitForFunction(() => window.__vectorDefence && document.querySelector(".board-canvas") && !document.querySelector(".board-loading"), undefined, { timeout: 120_000 });
    const staged = await page.evaluate(stageCrowdedFight);
    await page.waitForTimeout(1000);
    const metric = async () => Object.fromEntries((await session.send("Performance.getMetrics")).metrics.map(({ name, value }) => [name, value]));
    const frameCount = () => page.evaluate(() => window.__benchmarkFrames);
    const before = await metric();
    const framesBefore = await frameCount();
    await page.waitForTimeout(8000);
    const after = await metric();
    const frames = (await frameCount()) - framesBefore;
    const loop = await page.evaluate(timeTightLoop);
    const memory = await page.evaluate(() => ({
      jsHeapMb: performance.memory ? performance.memory.usedJSHeapSize / 1048576 : 0,
      wasmMb: window.__vectorDefence.game.wasmMemoryBytes ? window.__vectorDefence.game.wasmMemoryBytes() / 1048576 : 0,
    }));
    return {
      ...staged,
      frames,
      scriptMsPerFrame: ((after.ScriptDuration - before.ScriptDuration) * 1000) / frames,
      taskMsPerFrame: ((after.TaskDuration - before.TaskDuration) * 1000) / frames,
      ...loop,
      ...memory,
    };
  } finally {
    await browser.close();
    await server.close();
  }
}

// Runs in the page. Mirrors scripts/benchmark-3d-renderer.mjs: level 10, towers every 13 units
// near the road (all upgraded to level 7), 160 monsters of every kind with 6x hit points.
async function stageCrowdedFight() {
  const { game, sync } = window.__vectorDefence;
  window.__benchmarkFrames = 0;
  if (typeof game.stageBenchmarkFight === "function") {
    const towers = game.stageBenchmarkFight(9, 160, 6);
    sync();
    const originalDraw = game.draw.bind(game);
    game.draw = () => {
      originalDraw();
      window.__benchmarkFrames += 1;
    };
    return { engine: "wasm", towers };
  }

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
      if (game.placeTower(kinds[towerIndex % kinds.length], point)) {
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
      const monster = createMonster(
        monsterKinds[game.runtime.monsters.length % monsterKinds.length],
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
  const originalDraw = game.draw.bind(game);
  game.draw = () => {
    originalDraw();
    refill();
    window.__benchmarkFrames += 1;
  };
  return { engine: "typescript", towers: game.runtime.towers.length };
}

// Runs in the page: 600 back-to-back simulation steps and draws, no requestAnimationFrame.
function timeTightLoop() {
  const { game } = window.__vectorDefence;
  // GPU uploads per draw (calls and bytes), counted over a few draws before timing.
  const originalWriteBuffer = GPUQueue.prototype.writeBuffer;
  let uploadCalls = 0;
  let uploadBytes = 0;
  GPUQueue.prototype.writeBuffer = function (buffer, offset, data, dataOffset, size) {
    uploadCalls += 1;
    uploadBytes += size !== undefined ? size * (data.BYTES_PER_ELEMENT ?? 1) : data.byteLength - (dataOffset ?? 0) * (data.BYTES_PER_ELEMENT ?? 1);
    return originalWriteBuffer.call(this, buffer, offset, data, dataOffset, size);
  };
  for (let index = 0; index < 60; index += 1) {
    game.draw();
  }
  GPUQueue.prototype.writeBuffer = originalWriteBuffer;
  const iterations = 600;
  let updateMs = 0;
  let drawMs = 0;
  for (let index = 0; index < iterations; index += 1) {
    const start = performance.now();
    game.updateSimulation(1 / 60);
    const middle = performance.now();
    game.draw();
    drawMs += performance.now() - middle;
    updateMs += middle - start;
  }
  return { loopUpdateMs: updateMs / iterations, loopDrawMs: drawMs / iterations, uploadCallsPerDraw: uploadCalls / 60, uploadKbPerDraw: uploadBytes / 60 / 1024 };
}

const report = { root, profile: mobile ? "mobile" : "desktop", cpuThrottle, runs };
if (!skipProduction) {
  report.production = await measureProduction();
}
if (!skipFrames) {
  report.frames = await measureFrames();
}
console.log(JSON.stringify(report, null, 2));

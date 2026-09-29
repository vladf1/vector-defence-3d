// Browser checks of the real page on the Wasm engine: WebGPU startup, the session loop and
// placement painting, HUD/modal flow, a short seeded fight, and modal focus handling. The
// simulation's own checks (timing, collisions, effects, lifecycle, HUD strings) are native
// Rust tests: `npm run test:rust`.
import { WEBGPU_LAUNCH_ARGS, runBrowserPage } from "./benchmark-browser-harness.mjs";

const checks = [];
const errors = [];

await runBrowserPage({ path: "/", viewport: { width: 1200, height: 900 }, launchArgs: WEBGPU_LAUNCH_ARGS }, async (page) => {
  page.on("console", (message) => {
    if (message.type() === "error" || /WebGPU|GPUValidationError|Invalid/.test(message.text())) {
      errors.push(message.text());
    }
  });
  await page.waitForFunction(() => window.__vectorDefence && !document.querySelector(".board-loading"), undefined, { timeout: 60_000 });
  const board = await page.evaluate(() => ({
    canvas: Boolean(document.querySelector(".board-canvas")),
    failed: Boolean(document.querySelector(".board-unsupported")),
  }));
  if (!board.canvas || board.failed) throw new Error("WebGPU board renderer must initialize and attach");
  checks.push("WebGPU board renderer initializes and attaches");

  const dialog = page.getByRole("dialog");
  if ((await dialog.locator(".level-card, [class*=level]").count()) === 0) throw new Error("Campaign map must list levels");
  await dialog.getByRole("button", { name: "Play Next", exact: true }).click();
  await page.locator(".tower-button").first().waitFor();
  const towerButtons = await page.locator(".tower-button").count();
  if (towerButtons !== 5) throw new Error(`Level 1 must offer 5 towers, saw ${towerButtons}`);
  checks.push("Play Next starts level 1 with its tower toolbar");

  const painted = await page.evaluate(async () => {
    const { game } = window.__vectorDefence;
    const originalDraw = game.draw.bind(game);
    const record = { draws: 0, placement: null };
    game.draw = () => {
      record.draws += 1;
      record.placement = game.placingTower() ?? null;
      originalDraw();
    };
    window.__paint = record;
    return true;
  });
  if (!painted) throw new Error("draw hook");
  await page.keyboard.press("1");
  await page.evaluate(() => new Promise(requestAnimationFrame));
  if ((await page.evaluate(() => window.__paint.placement)) !== "gun") throw new Error("Placement must be painted before pause");
  checks.push("Placement is painted before pause");
  const beforePause = await page.evaluate(() => window.__paint.draws);
  await page.keyboard.press("Space");
  const afterPause = await page.evaluate(() => ({ ...window.__paint }));
  if (afterPause.draws !== beforePause + 1 || afterPause.placement !== null) throw new Error("Pausing must immediately paint the cleared placement");
  checks.push("Pausing immediately paints the cleared placement");
  await page.keyboard.press("Space");

  const fight = await page.evaluate(async () => {
    const { game, sync } = window.__vectorDefence;
    game.seedRandom(42);
    game.debugSetEconomy(200, 0.1, 99);
    let placed = 0;
    for (let y = 30; y < game.fieldHeight() - 20 && placed < 8; y += 17) {
      for (let x = 30; x < game.fieldWidth() - 20 && placed < 8; x += 17) {
        const distance = game.distanceToRoute(x, y);
        if (distance > 26 && distance < 40 && game.placeTower(["gun", "laser", "missile", "slow", "drone"][placed % 5], x, y)) {
          placed += 1;
        }
      }
    }
    sync();
    const moneyBefore = JSON.parse(game.takeHud(true, 0, 0, 0, 0)).money;
    for (let frame = 0; frame < 60 * 20; frame += 1) {
      game.updateSimulation(1 / 60);
      if (frame % 30 === 0) game.draw();
    }
    sync();
    const hud = JSON.parse(game.takeHud(true, 0, 0, 0, 0));
    return { placed, moneyBefore, money: hud.money, wave: hud.waveCurrent, stats: JSON.parse(game.debugStats()) };
  });
  if (fight.placed !== 8 || fight.money <= fight.moneyBefore) throw new Error(`Seeded fight must earn bounties: ${JSON.stringify(fight)}`);
  checks.push(`Seeded 20 s fight with 8 towers earns bounties ($${fight.moneyBefore} -> $${fight.money}, wave ${fight.wave})`);
});

for (const viewport of [{ width: 1200, height: 900 }, { width: 375, height: 812 }, { width: 390, height: 1000 }]) {
  await runBrowserPage({ path: "/", viewport, launchArgs: WEBGPU_LAUNCH_ARGS }, async (page) => {
    const dialog = page.getByRole("dialog");
    await dialog.getByRole("button", { name: "Play Next", exact: true }).click();
    const campaign = page.getByRole("button", { name: "Campaign", exact: true });
    await campaign.click();
    const containsFocus = () => dialog.evaluate((element) => element.contains(document.activeElement));
    if (!await containsFocus()) throw new Error("Opening map must move focus into the dialog");
    const buttons = dialog.locator("button:enabled");
    await buttons.first().focus();
    await page.keyboard.press("Shift+Tab");
    if (!await buttons.last().evaluate((element) => element === document.activeElement)) throw new Error("Shift+Tab must wrap inside the modal");
    await page.keyboard.press("Tab");
    if (!await buttons.first().evaluate((element) => element === document.activeElement)) throw new Error("Tab must wrap inside the modal");
    await dialog.getByRole("button", { name: "Resume Battle", exact: true }).click();
    if (!await campaign.evaluate((element) => element === document.activeElement)) throw new Error("Closing map must restore focus to its opener");
    await page.keyboard.press("j");
    await page.getByRole("dialog", { name: "Level Clear", exact: true }).waitFor();
    await dialog.getByRole("button", { name: "Campaign Map", exact: true }).click();
    if (!await containsFocus()) throw new Error("Switching modal content must retain focus inside it");
  });
  checks.push(`${viewport.width}x${viewport.height}: modal focus, tab containment, restoration, and content transition`);
}

if (errors.length > 0) {
  throw new Error(`Console errors:\n${errors.join("\n")}`);
}
console.log(`${checks.length} runtime checks passed`);
for (const check of checks) console.log(`  ${check}`);

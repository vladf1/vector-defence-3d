// Profiles the board's startup phases (renderer import, device, CPU setup, pipeline
// compiles, warm-up frame, GPU drain) with optional CPU throttling to approximate phones.
// Usage: node scripts/benchmark-3d-startup.mjs [--mobile] [--cpu=4] [--runs=3] [--cold]
import { WEBGPU_LAUNCH_ARGS, runBrowserPage } from "./benchmark-browser-harness.mjs";

const mobile = process.argv.includes("--mobile");
const numberArgument = (name, fallback) => {
  const match = process.argv.find((argument) => argument.startsWith(`--${name}=`));
  return match ? Number(match.split("=")[1]) : fallback;
};
const cpuThrottle = numberArgument("cpu", 1);
// --cold salts every shader per run so GPU driver shader caches miss (first-visit cost).
const cold = process.argv.includes("--cold");
const runs = numberArgument("runs", 3);
const viewport = mobile ? { width: 390, height: 844 } : { width: 1400, height: 900 };

const samples = [];
for (let run = 0; run < runs; run += 1) {
  samples.push(await runBrowserPage({
    path: "/",
    query: cold ? `shaderSalt=${1 + Math.floor(Math.random() * 999_999)}` : "",
    viewport,
    deviceScaleFactor: mobile ? 3 : 2,
    launchArgs: WEBGPU_LAUNCH_ARGS,
    beforeNavigate: async (page) => {
      if (cpuThrottle > 1) {
        const session = await page.context().newCDPSession(page);
        await session.send("Emulation.setCPUThrottlingRate", { rate: cpuThrottle });
      }
    },
  }, async (page) => {
    await page.waitForFunction(() => window.__vectorDefenceStartup, undefined, { timeout: 120_000 });
    return page.evaluate(() => window.__vectorDefenceStartup);
  }));
}

console.log(`Board startup (${mobile ? "mobile" : "desktop"}, cpu x${cpuThrottle}, ${cold ? "cold" : "warm"} shader cache, ${runs} runs, median)`);
for (const key of Object.keys(samples[0])) {
  const values = samples.map((sample) => sample[key]).sort((a, b) => a - b);
  const median = values[Math.floor(values.length / 2)];
  console.log(`  ${key}: ${Number.isInteger(median) ? median : median.toFixed(1)}`);
}

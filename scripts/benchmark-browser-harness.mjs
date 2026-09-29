import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
import { createServer } from "vite";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
export const repoRoot = path.resolve(__dirname, "..");

export async function runBrowserPage(options, runPage) {
  const plugins = [];
  if (options.html !== undefined) {
    plugins.push({
      name: options.pluginName,
      configureServer(viteServer) {
        viteServer.middlewares.use(options.path, (_request, response) => {
          response.setHeader("Content-Type", "text/html; charset=utf-8");
          response.end(options.html);
        });
      },
    });
  }

  const server = await createServer({
    root: repoRoot,
    logLevel: "error",
    server: {
      host: "127.0.0.1",
    },
    plugins,
  });

  let browser;
  try {
    await server.listen(0);
    const url = server.resolvedUrls.local[0];
    browser = await launchChromium(options.launchArgs ?? []);
    const page = await browser.newPage({
      viewport: options.viewport ?? { width: 960, height: 540 },
      deviceScaleFactor: options.deviceScaleFactor ?? 1,
    });
    page.on("console", (message) => {
      if (options.forwardConsole) {
        console.log(message.text());
      }
    });
    const targetUrl = new URL(options.path.replace(/^\//, ""), url);
    targetUrl.search = options.query?.replace(/^\?/, "") ?? "";
    const pageError = new Promise((_resolve, reject) => {
      page.once("pageerror", reject);
    });
    const pageWork = (async () => {
      await options.beforeNavigate?.(page);
      await page.goto(targetUrl.href, { waitUntil: options.waitUntil ?? "domcontentloaded" });
      return runPage(page);
    })();
    return await Promise.race([pageWork, pageError]);
  } finally {
    await browser?.close();
    await server.close();
  }
}

export function runBenchmarkPage(options) {
  return runBrowserPage(options, (page) => waitForPageResult(page, "__benchmarkResults", options.timeoutMs ?? 120_000));
}

export async function waitForPageResult(page, resultName, timeoutMs) {
  const result = await page.waitForFunction((name) => window[name], resultName, { timeout: timeoutMs });
  return result.jsonValue();
}

export async function writeDataUrlPng(outputPath, dataUrl) {
  const pngBase64 = dataUrl.replace(/^data:image\/png;base64,/, "");
  await mkdir(path.dirname(outputPath), { recursive: true });
  await writeFile(outputPath, Buffer.from(pngBase64, "base64"));
}

export async function writeDataUrlPngMap(outputDir, dataUrls) {
  await Promise.all(Object.entries(dataUrls).map(([filename, dataUrl]) =>
    writeDataUrlPng(path.join(outputDir, `${filename}.png`), dataUrl),
  ));
}

/** Chromium flags that expose WebGPU to headless pages. */
export const WEBGPU_LAUNCH_ARGS = ["--enable-unsafe-webgpu", "--enable-gpu", "--ignore-gpu-blocklist"];

async function launchChromium(args) {
  const headlessChannel = args.length > 0 ? { channel: "chromium" } : {};
  try {
    return await chromium.launch({ ...headlessChannel, args });
  } catch (error) {
    if (!String(error).includes("Executable doesn't exist")) {
      throw error;
    }
    return chromium.launch({ channel: "chrome", args });
  }
}

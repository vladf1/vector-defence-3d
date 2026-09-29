import { defineConfig, type Plugin } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { fileURLToPath } from "node:url";
import { resolve } from "node:path";

const rootDir = fileURLToPath(new URL(".", import.meta.url));
const ENGINE_WASM = "src/generated/engine/engine_bg.wasm";

/**
 * The engine binary is fetched by `src/engine.ts` once `main.js` runs, which would hide it
 * from the browser until then. Preloading it from index.html downloads it alongside `main.js`
 * (the `crossorigin` fetch mode matches the glue's `fetch`, so the preload is reused).
 */
function preloadEngine(): Plugin {
  let base = "/";
  return {
    name: "preload-engine",
    apply: "build",
    configResolved(config) {
      base = config.base;
    },
    transformIndexHtml: {
      order: "post",
      handler(html, context) {
        const bundle = context.bundle;
        if (!bundle || context.path !== "/index.html") {
          return html;
        }
        // The labs build (debug pages) has a binary of the same name; match the game's by source.
        const engine = Object.values(bundle).find((output) => output.type === "asset" && output.originalFileNames.includes(ENGINE_WASM))?.fileName;
        if (!engine) {
          throw new Error("preload-engine: no engine_bg.wasm in the bundle");
        }
        return [{
          tag: "link",
          attrs: { rel: "preload", as: "fetch", type: "application/wasm", crossorigin: true, href: `${base}${engine}` },
          injectTo: "head" as const,
        }];
      },
    },
  };
}

export default defineConfig({
  plugins: [svelte(), preloadEngine()],
  server: {
    allowedHosts: [".trycloudflare.com"],
  },
  build: {
    // Keep the engine a separate file even though it is small enough to inline.
    assetsInlineLimit: (file) => (file.endsWith(".wasm") ? false : undefined),
    rollupOptions: {
      input: {
        main: resolve(rootDir, "index.html"),
        debug: resolve(rootDir, "debug/index.html"),
        debugTowers: resolve(rootDir, "debug/towers.html"),
        debugSoundboard: resolve(rootDir, "debug/soundboard.html"),
      },
      output: {
        manualChunks(id) {
          if (id.includes("node_modules")) {
            return "vendor";
          }
        },
      },
    },
  },
});

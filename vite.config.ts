import { defineConfig, type Plugin } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { fileURLToPath } from "node:url";
import { resolve } from "node:path";

const rootDir = fileURLToPath(new URL(".", import.meta.url));
const BOARD_RENDERER_MODULE = "/src/render3d/webgpu-board-renderer.ts";

/**
 * The board renderer is a dynamic import (so Svelte mounts first), which hides it from the
 * page: the browser would only discover it after main.js downloads and runs. Preloading it
 * (and any chunks it imports) from index.html lets everything download in parallel.
 */
function preloadBoardRenderer(): Plugin {
  let base = "/";
  return {
    name: "preload-board-renderer",
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
        // The renderer may share a chunk with debug pages, so match any chunk containing it.
        const renderer = Object.values(bundle).find((output) => output.type === "chunk" && output.moduleIds.some((id) => id.endsWith(BOARD_RENDERER_MODULE)));
        if (!renderer || renderer.type !== "chunk") {
          throw new Error(`preload-board-renderer: no chunk for ${BOARD_RENDERER_MODULE}`);
        }
        const files = new Set<string>();
        const visit = (fileName: string): void => {
          const chunk = bundle[fileName];
          if (files.has(fileName) || chunk?.type !== "chunk") {
            return;
          }
          files.add(fileName);
          chunk.imports.forEach(visit);
        };
        visit(renderer.fileName);
        return [...files]
          .filter((fileName) => !html.includes(fileName))
          .map((fileName) => ({
            tag: "link",
            attrs: { rel: "modulepreload", crossorigin: true, href: `${base}${fileName}` },
            injectTo: "head" as const,
          }));
      },
    },
  };
}

export default defineConfig({
  plugins: [svelte(), preloadBoardRenderer()],
  server: {
    allowedHosts: [".trycloudflare.com"],
  },
  build: {
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

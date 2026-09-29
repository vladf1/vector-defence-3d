// Builds the game engine: release Wasm for `vd-web`, then its wasm-bindgen glue into the
// ignored `src/generated/`. Vite does not compile Rust: rerun this after Rust or WGSL edits
// (`npm run dev` and `npm run build` run it first).
//
// Two builds come out of one run:
//  - `src/generated/engine/`: the game, loaded by the production page.
//  - `src/generated/engine-labs/`: the same engine plus the `labs` feature (the scripted
//    staging API behind `window.__vectorDefence` and the `TowerLab` of debug/towers.html).
//    The dev server's page and the debug pages load this one; production pages never do.
//
// Cargo runs from the repository root so `.cargo/config.toml` (WebGPU bindings, SIMD) applies.
// Setup: rustup's wasm32-unknown-unknown target and `wasm-bindgen-cli` 0.2.129, matching the
// `wasm-bindgen` crate pin. `--game-only` skips the labs build.
import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
// rustup's toolchain (pinned by rust-toolchain.toml) has the wasm32 target; a system cargo
// earlier on PATH (Homebrew's, for example) usually does not, so ask rustup when it exists.
const rustup = spawnSync("rustup", ["which", "cargo"], { cwd: root, encoding: "utf8" });
const cargo = rustup.status === 0 ? rustup.stdout.trim() : "cargo";
// Put that toolchain's rustc first on PATH too, and (macOS) let its rust-lld find libLLVM in
// the toolchain's lib directory, which is not on the default search path outside rustup's proxies.
const toolchainBin = rustup.status === 0 ? path.dirname(cargo) : undefined;
const env = toolchainBin
  ? {
    ...process.env,
    PATH: [toolchainBin, process.env.PATH].join(path.delimiter),
    DYLD_FALLBACK_LIBRARY_PATH: [path.join(toolchainBin, "..", "lib"), process.env.DYLD_FALLBACK_LIBRARY_PATH].filter(Boolean).join(":"),
  }
  : process.env;
const variants = [{ outDir: "src/generated/engine", features: [] }];
if (!process.argv.includes("--game-only")) {
  variants.push({ outDir: "src/generated/engine-labs", features: ["--features", "labs"] });
}

function run(command, args) {
  const result = spawnSync(command, args, { cwd: root, stdio: "inherit", env });
  if (result.error) {
    console.error(
      `${command}: ${result.error.message}. Install Rust with the wasm32-unknown-unknown target and wasm-bindgen-cli 0.2.129.`,
    );
  }
  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

for (const { outDir, features } of variants) {
  run(cargo, ["build", "--locked", "--release", "--target", "wasm32-unknown-unknown", "-p", "vd-web", ...features]);
  run("wasm-bindgen", [
    "target/wasm32-unknown-unknown/release/vd_web.wasm",
    "--target",
    "web",
    "--out-dir",
    outDir,
    "--out-name",
    "engine",
  ]);
}

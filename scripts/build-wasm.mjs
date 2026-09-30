// Builds the game engine: Wasm for `vd-web`, then its wasm-bindgen glue into the ignored
// `src/generated/`. Vite does not compile Rust: rerun this after Rust or WGSL edits
// (`npm run dev` and `npm run build` run it first).
//
// Two engines:
//  - `src/generated/engine/`: the game, loaded by the production page.
//  - `src/generated/engine-labs/`: the same engine plus the `labs` feature (the scripted
//    staging API behind `window.__vectorDefence` and the `TowerLab` of debug/towers.html).
//    The dev server's page and the debug pages load this one; production pages never do.
//
// Modes:
//  - default (`npm run wasm`, `npm run dev`): the edit-reload loop. Only the labs engine, with
//    the `wasm-dev` profile (no whole-program LTO); the game engine is built once if missing.
//  - `--release`: the labs engine with the `release` profile (benchmark scripts use this).
//  - `--all` (`npm run build`): both engines, `release`. `--game-only`: the game, `release`.
// wasm-bindgen runs while the next engine compiles, and is skipped when its input Wasm and CLI
// version match the stamp it left in the output directory.
//
// Cargo runs from the repository root so `.cargo/config.toml` (WebGPU bindings, SIMD) applies.
// Setup: rustup's wasm32-unknown-unknown target and `wasm-bindgen-cli` 0.2.129, matching the
// `wasm-bindgen` crate pin.
import { spawn, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { copyFileSync, existsSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { cargo, repoRoot as root, toolchainEnv as env } from "./rust-toolchain.mjs";
const GAME = { outDir: "src/generated/engine", features: [] };
const LABS = { outDir: "src/generated/engine-labs", features: ["--features", "labs"] };
const all = process.argv.includes("--all");
const gameOnly = process.argv.includes("--game-only");
// Production and benchmarks use the fully optimized `release` profile; the dev loop uses
// `wasm-dev` (no whole-program LTO), which relinks after an edit about four times faster.
const profile = all || gameOnly || process.argv.includes("--release") ? "release" : "wasm-dev";
// Both variants link to the same `vd_web.wasm`, so every switch between them relinks. The dev
// loop therefore rebuilds only the labs engine, which the dev server and debug pages run; the
// game engine is built once when missing (the page imports its glue even in dev) and always
// for production.
const variants = gameOnly
  ? [GAME]
  : all || !existsSync(path.join(root, GAME.outDir, "engine_bg.wasm"))
    ? [GAME, LABS]
    : [LABS];
const targetDir = path.resolve(root, process.env.CARGO_TARGET_DIR ?? "target");
const artifact = path.join(targetDir, "wasm32-unknown-unknown", profile, "vd_web.wasm");

function fail(command, reason) {
  console.error(`${command}: ${reason}. Install Rust with the wasm32-unknown-unknown target and wasm-bindgen-cli 0.2.129.`);
  process.exit(1);
}

function run(command, args) {
  const result = spawnSync(command, args, { cwd: root, stdio: "inherit", env });
  if (result.error) {
    fail(command, result.error.message);
  }
  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

function bindgen(input, outDir) {
  return new Promise((resolve) => {
    const child = spawn("wasm-bindgen", [input, "--target", "web", "--out-dir", outDir, "--out-name", "engine"], {
      cwd: root,
      stdio: "inherit",
      env,
    });
    child.on("error", (error) => fail("wasm-bindgen", error.message));
    child.on("exit", (status) => (status === 0 ? resolve() : process.exit(status ?? 1)));
  });
}

const bindgenVersion = spawnSync("wasm-bindgen", ["--version"], { encoding: "utf8", env });
if (bindgenVersion.error || bindgenVersion.status !== 0) {
  fail("wasm-bindgen", bindgenVersion.error?.message ?? "not runnable");
}

const pending = [];
for (const { outDir, features } of variants) {
  run(cargo, ["build", "--locked", "--profile", profile, "--target", "wasm32-unknown-unknown", "-p", "vd-web", ...features]);
  const bytes = readFileSync(artifact);
  const stamp = path.join(root, outDir, ".source-hash");
  const hash = createHash("sha256").update(bindgenVersion.stdout).update(bytes).digest("hex");
  if (existsSync(stamp) && readFileSync(stamp, "utf8") === hash && existsSync(path.join(root, outDir, "engine_bg.wasm"))) {
    continue;
  }
  // The next variant's build replaces the artifact, so wasm-bindgen reads a private copy.
  const input = path.join(targetDir, `wasm-bindgen-input-${path.basename(outDir)}.wasm`);
  copyFileSync(artifact, input);
  pending.push(bindgen(input, outDir).then(() => writeFileSync(stamp, hash)));
}
await Promise.all(pending);

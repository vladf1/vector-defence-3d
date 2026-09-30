// The pinned Rust toolchain for the npm scripts. rustup's toolchain (rust-toolchain.toml) has
// the wasm32 target; a system cargo earlier on PATH (Homebrew's, for example) usually does not,
// so ask rustup when it exists. Its rustc goes first on PATH too, and (macOS) its rust-lld finds
// libLLVM in the toolchain's lib directory, which is not on the default search path outside
// rustup's proxies.
import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

export const repoRoot = fileURLToPath(new URL("..", import.meta.url));

const rustup = spawnSync("rustup", ["which", "cargo"], { cwd: repoRoot, encoding: "utf8" });
export const cargo = rustup.status === 0 ? rustup.stdout.trim() : "cargo";

const toolchainBin = rustup.status === 0 ? path.dirname(cargo) : undefined;
export const toolchainEnv = toolchainBin
  ? {
    ...process.env,
    PATH: [toolchainBin, process.env.PATH].join(path.delimiter),
    DYLD_FALLBACK_LIBRARY_PATH: [path.join(toolchainBin, "..", "lib"), process.env.DYLD_FALLBACK_LIBRARY_PATH].filter(Boolean).join(":"),
  }
  : process.env;

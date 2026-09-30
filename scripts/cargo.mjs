// `cargo` from the pinned toolchain (see rust-toolchain.mjs), for the npm scripts:
// `node scripts/cargo.mjs <cargo arguments>`.
import { spawnSync } from "node:child_process";
import { cargo, repoRoot, toolchainEnv } from "./rust-toolchain.mjs";

const result = spawnSync(cargo, process.argv.slice(2), { cwd: repoRoot, stdio: "inherit", env: toolchainEnv });
if (result.error) {
  console.error(`cargo: ${result.error.message}. Install Rust with rustup (rust-toolchain.toml pins the version).`);
}
process.exit(result.status ?? 1);

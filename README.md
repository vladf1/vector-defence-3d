# Vector Defence 3D

Vector Defence 3D is the 3D edition of [Vector Defence](https://github.com/vladf1/vector-defence-2026), a browser-based tower defense game: a Rust engine (simulation and WebGPU renderer) compiled to WebAssembly, inside a Svelte 5 + TypeScript + Vite page shell. It keeps the original's gameplay, campaign, and UI, and replaces the 2D canvas board with a 2.5D battlefield rendered with raw WebGPU.

- Play the 3D edition: [https://fridman.me/vector-defence-3d/](https://fridman.me/vector-defence-3d/)
- Original 2D game: [github.com/vladf1/vector-defence-2026](https://github.com/vladf1/vector-defence-2026), playable at [https://vd.fridman.me/](https://vd.fridman.me/)

The game features a fixed 10-level campaign, six tower types, and waves generated from handcrafted routes. This repository's active implementation is the browser app at the repo root: the engine in `crates/`, the page shell in `src/`. Gameplay changes usually start in the original; this edition follows them.

## WebGPU board

The board is a 2.5D scene drawn with raw WebGPU from Rust (through `web-sys`): no rendering library, a fixed set of 8 WGSL pipelines created asynchronously at startup (kept small because Safari compiles them one at a time), instanced procedural geometry, and a small HDR bloom chain. Browsers without WebGPU see a "WebGPU required" notice instead of the board.

- `?timings` shows how long each startup phase took (useful when profiling a phone).
- `?shaderSalt=N` (dev server only) perturbs every shader so GPU shader caches miss, for first-visit compile measurements.

## Requirements

- Node.js 20.19+, 22.12+, or 24+
- npm 10.8.2+
- [rustup](https://rustup.rs/) (the toolchain pinned in `rust-toolchain.toml`, with the `wasm32-unknown-unknown` target, installs on first use)
- `wasm-bindgen-cli` 0.2.129, matching the `wasm-bindgen` crate pin: `cargo install wasm-bindgen-cli --version 0.2.129 --locked`

## Getting Started

Install dependencies:

```bash
npm install
```

Start the local development server (it builds the Wasm engine first; rerun `npm run wasm` after Rust or WGSL edits):

```bash
npm run dev
```

Vite will print a local URL in the terminal, typically `http://localhost:5173/`.

## Build

Create a production build:

```bash
npm run build
```

Create a GitHub Pages build with the repository base path:

```bash
npm run build:pages
```

Preview the production build locally:

```bash
npm run preview
```

Useful validation commands:

```bash
npm run build
npm run build:pages
npm run test:rust
npm run rust:clippy
npm run check:runtime
npm run dev
npm run render:3d
npm run benchmark:3d
npm run benchmark:3d:startup
npm run benchmark:compare
```

## Deploy To GitHub Pages

This repository includes a GitHub Actions workflow that builds the app for the `vector-defence-3d` Pages path (`npm run build:pages`) and deploys the generated `dist/` output whenever changes are pushed to `main`. (The original deploys to a custom domain, so it builds for the site root instead.)

One-time GitHub setup:

1. Open the repository Settings page on GitHub.
2. Open Pages.
3. Set the publishing source to `GitHub Actions`.

Deploy from `main`:

```bash
git push origin main
```

Deployment runs are available in [GitHub Actions](https://github.com/vladf1/vector-defence-3d/actions/workflows/deploy-pages.yml).

The published site is available at [https://fridman.me/vector-defence-3d/](https://fridman.me/vector-defence-3d/).

## Controls

- `1` / `G`: Gun tower
- `2` / `Z`: Laser tower
- `3` / `R`: Missile tower
- `4` / `S`: Slow tower
- `5` / `D`: Drone tower on routes that offer it
- `5` / `E`: Lightning tower on routes that offer it
- `U`: Upgrade selected tower
- `Esc`: Cancel build mode
- `Space`: Pause or resume
- `?` or `H`: How to play (towers and controls)
- `N`: Stats for nerds (frame rate, update and draw times, object counts)
- Drag the board (any mouse button): Pan the view (desktop)
- Mouse wheel or trackpad pinch over the board, or `=` / `-`: Zoom the view (desktop)
- `Shift` + mouse wheel, or `↑` / `↓`: Tilt the view (desktop)
- `0`: Reset the view (desktop)

## Project Notes

- App entry: `src/main.ts`
- Root Svelte component: `src/App.svelte`
- Shared session bridge: `src/game-session.ts` (drives the Wasm engine loaded by `src/engine.ts`)
- Simulation engine: `crates/core/` (`game.rs`, entities, campaign, HUD/modal view models)
- WebGPU board renderer: `crates/render/` (entry `renderer.rs`, shaders in `shaders.rs`)
- Wasm bindings: `crates/web/`
- Browser level data: `game-levels.json` (compiled into the engine by `crates/core/build.rs`)

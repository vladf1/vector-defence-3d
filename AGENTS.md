# Agents

This repository (`vladf1/vector-defence-3d`) is the 3D edition of Vector Defence: the original 2D game lives in `vladf1/vector-defence-2026` (played at https://vd.fridman.me/). Gameplay, campaign, and UI come from the original; this edition replaces its 2D canvas board with a raw-WebGPU 2.5D board. When porting a gameplay change from the original (TypeScript), port it into the Rust simulation in `crates/core/`, keep entities free of drawing code, and add the 3D presentation in `crates/render/`.

Active browser implementation lives in this repo root. The engine is Rust compiled to WebAssembly (like `vladf1/sloppy-tanks`): a simulation crate, a raw-WebGPU renderer crate, and a wasm-bindgen crate. The TypeScript left in `src/` is the page shell: Svelte UI, input gathering, the frame loop, audio, and the GPU device prefetch.

Key paths:

- Rust workspace: `Cargo.toml` (release profile: `opt-level = "z"`, LTO, `panic = "abort"`), `rust-toolchain.toml`, `.cargo/config.toml` (`--cfg=web_sys_unstable_apis` for the WebGPU bindings, SIMD), `rustfmt.toml` (120 columns)
- Simulation crate: `crates/core/` (`vd-core`; native tests with `cargo test`)
- Renderer crate: `crates/render/` (`vd-render`; pure parts build natively for tests, GPU parts are Wasm-only)
- Wasm bindings crate: `crates/web/` (`vd-web`: `WebGame`, `createBoardRenderer`, and the `labs` feature)
- Wasm build script: `scripts/build-wasm.mjs` (`npm run wasm`; writes the ignored `src/generated/engine/` and `src/generated/engine-labs/`)
- Engine loader: `src/engine.ts`
- Browser app entry: `src/main.ts`
- Root Svelte component: `src/App.svelte`
- Svelte components: `src/components/`
- Svelte session context: `src/game-context.ts`
- Svelte/engine bridge: `src/game-session.ts`
- Desktop/mobile UI profiles and startup selection: `src/game-profile.ts` (gameplay geometry lives in `crates/core/src/profile.rs`)
- HUD shown before the engine loads: `src/initial-hud.ts`
- Browser simulation engine: `crates/core/src/game.rs`
- Frame backlog/substep policy: `crates/core/src/simulation_timing.rs` (`Game::advance`)
- HUD/modal view models and banner text: `crates/core/src/view.rs` (JSON parsed by the shell into `src/types.ts` shapes)
- Level runtime state: `crates/core/src/level_runtime.rs`
- Campaign progress persistence: `crates/core/src/progress.rs` (storage adapter `crates/web/src/storage.rs`)
- Route geometry/motion samples: `crates/core/src/route_path.rs`
- Placement geometry/tower hit-testing: `crates/core/src/placement.rs`
- Renderer loading status / `?timings` flag: `src/renderer-status.ts`
- WebGPU board renderer: `crates/render/src/` (entry `renderer.rs`)
- WGSL shaders: `crates/render/src/shaders.rs`
- Toolbar tower icons (static SVG): `src/components/tower-icons.ts`
- Top-bar control icons (vector neon strokes): `src/components/control-icons.ts`, rendered by `src/components/ControlIcon.svelte`
- GPU device prefetch: `src/gpu-device.ts` (adopts the request started by the inline script in `index.html`)
- Build config (engine preload plugin): `vite.config.ts`
- 3D board render sheet script: `scripts/render-3d-board.mjs`
- 3D renderer benchmark: `scripts/benchmark-3d-renderer.mjs`
- 3D startup profiler: `scripts/benchmark-3d-startup.mjs`
- Before/after benchmark (works on older TypeScript-engine checkouts too): `scripts/benchmark-compare.mjs`
- Gameplay entities: `crates/core/src/entities/`
- Engine helpers: `crates/core/src/{update.rs, collision.rs, combat_effects.rs, monster_factory.rs}`
- Browser audio orchestration: `src/game-audio.ts`
- Browser audio cue manifest: `src/audio-manifest.ts` (cue order matches `crates/core/src/audio.rs`)
- Tower metadata/shortcuts/toolbar order: `crates/core/src/entities/towers/registry.rs`
- Campaign builder: `crates/core/src/campaign.rs` (routes compiled from JSON by `crates/core/build.rs`)
- Shared shell types: `src/types.ts`; shared engine types: `crates/core/src/types.rs`
- Shared engine constants: `crates/core/src/constants.rs`
- Shared engine utilities: `crates/core/src/utils.rs` (the shell keeps only `formatMoney`/`clamp` in `src/utils.ts`)
- Browser styles: `src/style.css`
- Browser package/scripts: `package.json`
- Browser level data: `game-levels.json`
- Browser audio assets: `src/assets/audio/`
- Level render sheet script: `scripts/render-levels.mjs`
- Browser render/benchmark harness: `scripts/benchmark-browser-harness.mjs`
- Debug tools hub: `debug/index.html`
- 3D tower/projectile sheet page: `debug/towers.html`
- 3D tower/projectile sheet script: `src/tower-testing.ts` (staging in `crates/web/src/labs.rs`, `TowerLab`)
- Audio soundboard: `debug/soundboard.html`

Repository notes:

- The project root is the active browser app repo.
- The engine in `crates/` is the active implementation of gameplay and rendering; `src/` is the page shell. Vite does not compile Rust: run `npm run wasm` (or `npm run dev`/`npm run build`, which run it first) after Rust or WGSL edits.
- The page shell is a Svelte 5 + Vite app. Avoid reintroducing hand-built DOM/UI glue when a small Svelte component or view model is the cleaner boundary.
- The board renders only through raw WebGPU, driven from Rust through `web-sys` (no wgpu, three.js or other rendering library, and no 2D canvas board). Keeping the download small is a goal: do not add rendering dependencies, bring back a 2D renderer, or add raster UI images (icons are inline SVG data) without being asked. Check the Wasm size (`gzip -9` of `src/generated/engine/engine_bg.wasm`) when adding dependencies.
- Wasm size rules: no runtime serde/JSON parser (levels are compiled in by `build.rs`; view models use the small writer in `crates/core/src/json.rs`), no float `Display`/`{:.N}` formatting or `str::parse::<f64>` in the game build (they pull ~20 KB of tables; use `js_to_fixed` and integer formatting), no `HashMap` (SipHash and hashbrown; use `SmallMap` or dense arrays), no Unicode case/whitespace helpers (use the ASCII ones). `crates/web/src/math.rs` routes libm (`sin`, `cos`, `exp`, `pow`, ...) to the browser's `Math`, so f32 and f64 math share one native implementation.
- The Rust toolchain comes from rustup (`rust-toolchain.toml`); a system `cargo` earlier on PATH (Homebrew) may lack the wasm32 target, so `scripts/build-wasm.mjs` asks `rustup which cargo` and also puts that toolchain's `lib` on `DYLD_FALLBACK_LIBRARY_PATH` for rust-lld on macOS.

Current code structure:

- `src/main.ts` owns Svelte app bootstrapping, startup profile selection, and visual-viewport height synchronization. It also starts the engine download/compile (`loadEngine()`) and the GPU device request before Svelte mounts.
- Startup is tuned so nothing waits in series: an inline script in `index.html` starts `requestAdapter()`/`requestDevice()` during HTML parsing (`src/gpu-device.ts` adopts that promise once, and requests a fresh device for later remounts), and the `preload-engine` plugin in `vite.config.ts` adds a `<link rel="preload" as="fetch">` for the game's `engine_bg.wasm`, so it downloads alongside `main.js` instead of after it runs. Keep both when restructuring startup.
- `src/engine.ts` loads the production engine (`src/generated/engine/`) or, on the dev server, the `labs` build (`src/generated/engine-labs/`), which adds the scripted staging API used by render/benchmark scripts. Production pages never load the labs build; debug pages load it through `loadLabsEngine()`.
- `src/App.svelte` wires the main shell and creates the shared game session context.
- The WebGPU canvas fills the whole screen and the HUD floats over it as glass panels: `src/components/TopBar.svelte` (brand and level, credits/wave/hostiles/breach-limit status, help/sound/pause/map buttons), `src/components/TowerDock.svelte` (the tower dock plus the contextual selected-tower or placement card; on mobile the selection card replaces the dock), `src/components/GameBoard.svelte` (canvases and the wave banner), `src/components/NerdStatsPanel.svelte` (toggled with `N`), `src/components/HelpDialog.svelte` (how to play, towers, controls; `?` or `H`, pauses a running battle while open), and `src/components/GameModal.svelte` (campaign map and results). There is no on-screen tips text; controls live in the help dialog. The dock and help read the engine's tower catalog (`session.towerCatalog`); `src/style.css` holds the whole HUD design (tokens on `:root`, `.panel` glass with neon corner brackets).
- `src/App.svelte` measures the top bar and the dock and reports them as HUD insets (`session.setViewInsets` → `WebGame.setViewInsets` → the camera rig), so the camera frames the field in the uncovered middle with an off-center lens shift while the ground keeps rendering under the HUD; it also exposes the bands as `--hud-top`/`--hud-bottom` for floating pieces (banner, nerd stats, placement hint). Keep HUD panels inside those two bands so they never cover the playfield.
- `src/game-context.ts` owns the Svelte context helpers for the shared `GameSession`.
- `src/game-session.ts` bridges Svelte stores/events to the engine's `WebGame`: it handles keyboard/pointer input, owns the animation-frame loop and bounded simulation backlog (the substeps run inside `WebGame.advance`), publishes HUD/modal snapshots (`takeHud`/`takeModal` JSON, only when dirty), and plays the sounds the engine queued (`takeSounds`: `[cue, panX, intensity]` triples, NaN for defaults). It mounts a `BoardSurface` (the WebGPU canvas plus a 2D overlay canvas), creates the renderer with `createBoardRenderer(...)` once the engine and device are ready, holds the simulation while it loads, remounts a fresh renderer on `device.lost`, and reports `RendererStatus.Failed` (shown as a "WebGPU required" notice) if WebGPU cannot start. In dev builds it exposes `window.__vectorDefence = { game, sync }` (a labs `WebGame`) for render/benchmark scripts.
- `crates/core/src/simulation_timing.rs` owns the bounded substep policy: native high-refresh deltas are preserved, slow frames are split into steps of at most 1/60 second, catch-up work is capped, and drawing still happens once per rendered frame.
- `crates/core/src/profile.rs` owns desktop/mobile logical dimensions, movement/range scales, and placement geometry; `src/game-profile.ts` owns the UI flags and startup selection and passes only the mode to the engine.
- `crates/core/src/game.rs` owns gameplay state, campaign progression, lifecycle resolution, and `update_simulation(...)`. It does not own the renderer: the web layer keeps both, feeds the renderer's visible field bounds into `Game::set_visible_field_bounds`, resolves overlay button hits before `handle_board_click(point, CanvasActionHit)`, and calls `renderer.draw(&game)` once per frame. `Game.runtime_generation` changes whenever the runtime is replaced, so renderers reset per-entity state.
- `crates/core/src/level_runtime.rs` owns per-level mutable runtime collections such as monsters, towers, projectiles, particles, links, placement state, money, wave counters, and route path.
- `crates/core/src/progress.rs` owns campaign unlock/star persistence (same `localStorage` keys and clamping as before) over a `ProgressStorage` trait, with an in-memory fallback when browser storage is unavailable or fails.
- `crates/core/src/update.rs` defines the per-update read model (`UpdateContext`) and the `UpdateResult` accumulator through which monsters, towers, projectiles, and drones report lifecycle outcomes, spawned entities, presentation effects, and sounds. `StandaloneUpdateContext` owns a context for entities updated outside a `Game` (the tower lab, tests).
- `crates/core/src/collision.rs` owns the active-monster swept-collision index (a dense uniform grid over the indexed targets) plus its linear comparison implementation.
- `crates/core/src/monster_factory.rs` owns monster construction, level hit-point scaling, and splitter child creation through explicit speed-scale and level-index inputs; it does not depend on `Game`.
- `crates/core/src/combat_effects.rs` owns shared hit, laser, missile, and escape particle construction. Monster polygon breakup lives in `crates/core/src/entities/monsters/death_effect_helpers.rs` and `polygon_shard_splitter.rs`.
- `crates/core/src/placement.rs` owns tower placement geometry and board hit-testing through explicit route/tower inputs; it does not use `Game`.
- `crates/core/src/route_path.rs` owns route drawing commands plus the sampled motion path entries used by monster movement (`SharedPath = Rc<[PathEntry]>`).
- `crates/core/src/view.rs` owns HUD/modal view-model generation as JSON matching the TS `HudSnapshot`/`ModalView` shapes, and the tower catalog.
- `crates/core/src/rng.rs` is the engine's `Math.random()`: one seedable generator for the simulation and effects (`WebGame.seedRandom` in labs builds; scripts use it for repeatable frames).
- `crates/core/src/entities/` owns gameplay entities split by concern: towers, monsters, projectiles, and effects. Entities referenced by identity are shared handles (`MonsterRef`, `TowerRef`, `DroneRef` = `Rc<RefCell<...>>`), and every entity has a unique `id` the renderer keys its per-entity state by.
- `crates/render/src/` is the raw-WebGPU board renderer. It reads `Game` every frame and never mutates simulation state:
  - `renderer.rs` owns device/canvas setup, the frame uniform buffer (camera matrices, lights, field constants, time, flash lights), startup (with per-phase `StartupTimings`, `startup_timings.rs`), resize, and per-frame orchestration: `draw(&Game)` refills batches through `frame_composer.rs` (governor, runtime switch, frame context, board → entity views → fx), records one command encoder (scene pass, bloom chain, composite), submits once, then draws the 2D overlay.
  - `shaders.rs` builds every WGSL module with `create_shader_sources(...)`, baking the startup-fixed scene constants (hemisphere/key/rim light colors and directions, field size, road half width, smoke-puff blobs) in as WGSL `const`s. The `Frame` uniform (`FrameLayout` gives its float offsets) holds only per-frame data: camera matrices, camera position + time, and a fixed four-slot point-light array. Six modules: `neon`, `ground`, `road`, `effect` (ribbon/decal/health bar/range/ground glow/road chevron, selected per instance by `EffectMode` in the tint's w), `sprite` (glow/smoke, `SpriteMode` in the shape's w), and `post` (downsample/blur/composite, `PostMode` in the pass parameters). The post code tone-maps by hand: the brightest channel goes through the ACES filmic curve and scales the color (so saturated neon keeps its hue instead of bleaching to pastel), handing over to full per-channel ACES (matrices written row by row, so vectors multiply on the left) only for white-hot values such as explosion cores; then sRGB encoding. `crates/render/tests/wgsl_validation.rs` validates every generated module natively with naga and enforces the Safari fragment-resource rule.
  - `gpu_pipelines.rs` creates the 8 pipelines (neon, ground, road, effect, health bar, sprite, bloom, composite) with `createRenderPipelineAsync` in one parallel batch (`Promise.all`), with explicit bind group layouts. Nothing compiles after startup. Effects and sprites use premultiplied-alpha blending, so one pipeline serves additive layers (shader alpha 0) and normally blended ones. `?shaderSalt=N` (dev only, passed by the session) perturbs every fragment shader so driver caches miss. `gpu_flags.rs` holds the WebGPU usage bit constants and zero-copy byte views of f32/u32 slices.
  - `post_processing.rs` owns the HDR targets (rgba16float scene color with optional MSAA resolve, depth24plus, two bloom levels plus scratch) and records the prefilter/downsample, separable blur, and composite passes (vignette, ACES, sRGB) onto the canvas: all seven passes use the one `post` module and a shared bind group layout, with a 1x1 placeholder in unused texture slots.
  - Draw order: opaque neon batches, ground, road, health bars (no depth test), then blended layers in fixed order (road chevrons, ribbon, smoke, glow, decal, range, ground glow). Road chevrons are rigid quads that `BoardScene` places along the route every frame, turned to the local heading and lifted just above the road sample they lie on, so turns never bend them (the road shader itself paints only the channel and edges). Blending is `src-alpha`-based additive or normal alpha.
  - `camera_rig.rs` (plain matrices from `math.rs`) fits the tilted perspective camera to the field inside the HUD safe area (`set_insets`: an off-center projection, so the view axis lands in the middle of the uncovered area), ray-casts pointer picking onto the ground, projects field points for the overlay, and applies screen shake to a separate render camera (picking never shakes). `create_board_camera_rig(...)` is the board framing. The player view controls (`tilt_by`, `zoom_at`, `pan_between`, `reset_view`) apply on top of the fitted framing: tilt re-fits the whole field (straight down to 0.6 rad), zoom (1x to 4x) keeps the ground under the cursor in place, pan grabs the ground and is clamped so the view never drifts past the field, and screen shake is scaled back by the zoom. Picking and projection follow the player view, but `field_bounds` (placement) always comes from the default framing, limited to the field itself (a wide screen shows more ground, not more playable area), so the view is presentation only. `inspect(...)` frames a shake-free orbit close-up (`InspectView`: field point, visible height, yaw, tilt; `BOARD_TILT_RADIANS` matches the board) with a distance-scaled near plane, for render scripts and the tower sheet.
  - `render_batches.rs` is the complete, fixed list of instanced batches (the `Batch` enum indexes them), each naming its pipeline. `instanced_batch.rs` (24 floats per instance: column-major transform, tint, extras) and `sprite_batch.rs` (12 floats: position/rotation, size/shape, color) refill `Vec<f32>`s every frame and upload the used range with one `writeBuffer` (`gpu_batches.rs`).
  - Lighting is Lambert (the neon look comes from emissive trims, rims, an analytic key-light highlight, and bloom). Neon batches pick a `NeonMode` (written to the tint w): `Metal` parts (towers, scenery) get a white highlight, `Creature` parts (every monster batch and the shards) take highlight and rim in their own color, so monsters read in their original colors. Every monster needs a glowing (`glow = 1`) outline trim in its color that matches the original 2D silhouette from above; that trim is what carries its identity. Shadows are soft blob decals pushed through `RenderBatches::push_blob_shadow(...)`, offset along the key light by each object's height; there are no shadow maps.
  - `models.rs` / `geometry_kit.rs` build all procedural low-poly models as non-indexed triangle soups (monsters at unit radius, towers in field units); neon parts become interleaved `position(3) normal(3) glow(1)` vertices, and flat effect quads are `position(3) uv(2)`. `extrude_outline(...)` reproduces three.js `ExtrudeGeometry` bevels, including the sqrt(2) miter cap.
  - `puff_blobs.rs` generates the seeded blob parameters of the former 2x2 smoke-puff canvas atlas; `puffCoverage(...)` in the effect and sprite shaders evaluates them per pixel (smoke sprites, and the noisy scorch-decal mask), without any texture.
  - `monster_view.rs`, `tower_view.rs`, `projectile_view.rs`, `effect_view.rs`, `placement_view.rs`, and `board_scene.rs` compose entities into batches (`entity_views.rs` runs them in order; per-entity state lives in `IdMap`s keyed by entity id, `id_map.rs`); `fx_system.rs` owns renderer-only spectacle (pooled 3D sparks, fireballs, smoke, ground rings, a fixed flash-light pool written into the frame uniforms, scorch decals, camera trauma handed to the rig once per frame; slow-tower pulses deliberately show only their links, with no ring); `overlay.rs` lays out and draws the screen-space tower actions and escape counter on a 2D canvas above the WebGPU canvas (its hit tests use the last drawn frame's selection).
  - `render_quality.rs` holds the desktop/mobile budgets and the dynamic-resolution governor.
- `crates/web/src/game.rs` is the wasm-bindgen surface for the page (`WebGame`, `BoardRenderer`, `createBoardRenderer`); `crates/web/src/labs.rs` (feature `labs`) adds the scripted staging/benchmark API, the `TowerLab` for `debug/towers.html`, and `levelSheet` for `scripts/render-levels.mjs`.
- `crates/core/src/entities/monsters/monster.rs` owns shared monster movement, damage, slow recovery, and lifecycle outcome reporting; per-kind state lives in `MonsterSpecial` variants.
- Concrete monster kinds live in `crates/core/src/entities/monsters/` (one file each) and own monster-specific base stats, outlines, animation state, death effects, and special behavior (`berserker` ramps speed as it loses health; `bulwark` mitigates incoming damage).
- Projectiles live under `crates/core/src/entities/projectiles/` (`projectile.rs` for gun and drone shots, `missile.rs`, `drone.rs`).
- Effects live under `crates/core/src/entities/effects/`: `Particle` with a `ParticleKind` per former particle class, and `Link` (slow and lightning links).
- `crates/core/src/entities/drone_visuals.rs` holds the drone accent colors; the tank's track-print and detached-turret particles live with the tank in `monsters/tank.rs` and `ParticleKind`. Keep such helpers narrow and colocated with the entities that use them.
- Towers live under `crates/core/src/entities/towers/`; `Tower` in `tower.rs` owns shared targeting/upgrade/selection behavior, and per-kind state lives in `TowerSpecial` variants.
- `crates/core/src/entities/towers/registry.rs` is the source of truth for tower metadata, keyboard shortcuts, and toolbar order (`TOWER_INFOS`, `TowerKind::ALL`). Toolbar icons are static SVG in `src/components/tower-icons.ts`.
- Towers, projectiles, drones, monsters, and presentation effects update through `UpdateContext`; gameplay entities report additions and lifecycle outcomes through `UpdateResult`. Do not give them a `Game` dependency when the context/result boundary is sufficient.
- `crates/core/src/campaign.rs` turns the ten authored routes into the campaign.
- `crates/core/src/view.rs` derives banner, HUD, and modal presentation data; keep text/formatting policy out of Svelte components and the renderer where practical.
- `src/types.ts` is the source of truth for the shell's view-model shapes (`HudSnapshot`, `ModalView`, `TowerCatalogEntry`, `TowerKind`, `ModalAction`); `crates/core/src/types.rs` holds the engine's `GameState`, `MonsterKind`, `TowerKind`, `Point`, `LevelData`, and `WaveData`, with the same string ids.
- `crates/core/src/constants.rs` and `crates/core/src/utils.rs` are shared by the engine, so prefer reusing those helpers instead of re-declaring gameplay constants or math utilities.

Data / naming conventions:

- Monster identifiers in `game-levels.json` use the readable string values from `MonsterKind`, not one-letter codes:
  - `packman`
  - `square`
  - `triangle`
  - `tank`
  - `runner`
  - `splitter`
  - `berserker`
  - `bulwark`
- In Rust, `GameState`, `MonsterKind`, `TowerKind`, and `ModalAction` are enums with `as_str()`/`parse()` for their string ids; in the shell they stay `as const` value objects with derived union types in `src/types.ts`, not TypeScript enums.
- Keep `game-levels.json` monster identifiers as plain strings. `crates/core/build.rs` applies the mobile overrides, validates monster/tower names (a bad name fails the build), and generates the route data that `crates/core/src/campaign.rs` expands.
- `game-levels.json` provides the ten campaign routes. The actual playable campaign data is generated at startup by `create_campaign_levels(...)`, which expands those authored routes into per-wave monster sequences and build windows.
- Authored routes do not own `monsterCount`; playable `LevelData.monster_count` is derived from the generated waves in `crates/core/src/campaign.rs`.
- Monster kinds pass private named constants to the shared constructor (`Monster::base(path, COLOR, SPEED, HIT_POINTS, BOUNTY, RADIUS)`).
- Monster stats use `hit_points`, not `hp`. `hit_points` is current monster health; `max_hit_points` is the full-health denominator used by the health bar.
- Monster constructors take the concrete `SharedPath` they should follow, not `LevelData`.
- For unusual spawn positions, build a new path with route-path helpers such as `create_path_entries_from_distance(...)` instead of passing raw level points to monsters.
- Monsters report `killed` and `escaped` lifecycle outcomes through `UpdateResult`; `Game` resolves those outcomes after monster updates.
- Monster kinds should not reach into `Game` or call game orchestration methods directly.
- Use `MonsterKind` for level/campaign data and `Monster::kind()` / `MonsterSpecial` matches for runtime kind-specific behavior.

Gameplay / UI notes:

- The campaign is a fixed 10-level progression with unlocks and stars persisted in browser `localStorage`; `CampaignProgressStore` (`crates/core/src/progress.rs`) falls back to memory if storage access fails.
- Initial build time is campaign-driven, not a fixed global delay: early levels start around 10 seconds and later ones reach 14 seconds.
- Intermission build windows between later waves are shorter and are generated per wave in `crates/core/src/campaign.rs` (roughly 2.5 to 5.5 seconds).
- Level 1 is an introductory route and is not intended to showcase the full monster roster; its generated waves should stay within the monster pool authored for `Outer Line` in `game-levels.json`.
- Later campaign waves introduce heavier and specialist monsters such as `tank`, `splitter`, `bulwark`, and `berserker`. Splitters burst into weakened runner children when killed.
- Monster spawning is orchestrated by `Game::spawn_monster(...)`, while monster construction/scaling and splitter children are centralized in `crates/core/src/monster_factory.rs`; tower creation goes through `Game::create_tower(...)` and `Tower::new(kind, ...)` with registry metadata.
- The main frame loop preserves native high-refresh updates, uses bounded substeps to recover slow-frame time, draws once, and freezes background-tab time by resetting the frame clock on visibility changes.
- Bulwark flat armor applies only to discrete `take_damage(...)` hits. Continuous effects use `take_continuous_damage(...)`; laser beam damage is analytically integrated over its fade so results do not depend on refresh rate.
- Entities hold no drawing code. All rendering lives in `crates/render/`, which reads public fields and small presentation getters on entities (for example `current_mouth_angle()`, `get_dash_pulse()`, `get_reload_progress()`). When adding a monster or tower, add its model in `models.rs`, a batch in `render_batches.rs`, and a kind branch in the matching view; expose read-only presentation state rather than moving rendering into the entity.
- Shader budget rules: all entity parts share the one `neon` pipeline and instance tint carries identity, so new visuals should add geometry/batches, not pipelines; a new effect look is a new `EffectMode`/`SpriteMode` branch, not a new module. Pipeline state (MSAA sample count, formats, light slot count) is fixed at startup; resolution is the only runtime quality knob. A new pipeline must be added to the parallel batch in `gpu_pipelines.rs`, never created lazily mid-game. Ribbons are single-sided and always faced toward the camera.
- Safari/WebKit compile rules (measured on Safari 27 and Playwright WebKit): WebKit compiles pipelines one at a time, pays for every distinct pipeline state (blend state included) again at its first draw, and a fragment stage that reads a uniform buffer (~130 ms) or a texture (~250 ms) compiles far slower than math-only fragment code (~5 ms). So keep fragment code free of buffer/texture reads (pass per-frame values from the vertex stage as flat varyings, as ground and road do with the point lights; bake startup constants into the WGSL), share modules, and keep the pipeline count low. Only `neon` (per-pixel point lights) and `post` read resources in fragment code. WebKit keeps compiled shaders across launches, so the full cost lands on the first visit after shader text changes.
- Startup rules: first-visit cost is dominated by GPU driver shader compiles, so every pipeline is created asynchronously in one parallel batch while the CPU builds geometry; a warm-up frame touching every batch is drained before the board reports ready. `npm run benchmark:3d:startup -- --cold` measures first-visit startup by salting every shader (`?shaderSalt=N`, dev only) so driver caches miss. `?timings` shows the startup phase breakdown on the board in any build.
- 3D effects are driven from simulation state: the effect view maps each simulation particle/link kind to a 3D treatment (shards and turret debris get height, gravity, tumble, and bounce), monster disappearances are classified into kill/escape by hit points and path progress, and `ShockwaveEffect` instances trigger missile blasts. Presentation time comes from `Game.simulation_seconds`, so 3D effects freeze with the game.
- Monster-specific visual animations, such as tank turret spins or packman mouth/body flourishes, should live in the monster kind's state and run through its special update; if an animation changes visible body geometry or orientation, keep that current shape reflected in the monster's `add_death_effect(...)` outline/rotation so shards match the death frame.
- Use shared easing helpers from `crates/core/src/utils.rs` for monster animation progress, and keep mutually exclusive monster flourishes in one local state machine when they should not overlap.
- Tower kinds own their attack behavior and presentation state. Shared targeting/selection concerns belong in `Tower`.
- 3D towers follow the original's design language: a near-black plinth (`tower-base`, tinted `PLINTH`), a glowing rim on its top edge in the original's base-stroke color (`tower-rim`: white, softer for missile and drone), an upgrade halo from level 2 on (`upgrade-ring`, sized from `TOWER_RADIUS`/`TOWER_UPGRADE_RING_*` in the tower's original ring color), and the weapon in its accent colors. Keep the plinth dark so the rim, halo, and weapon carry each tower's identity.
- Svelte components should consume `HudSnapshot` and `ModalView` data rather than reaching into the engine directly.
- The HUD selection card supports upgrade, sell, and cancel-build actions; keep those interactions flowing through `GameSession` and the HUD snapshot rather than binding components directly to `WebGame`.
- Mobile layout support starts at a `375 x 812` CSS-pixel viewport. Do not optimize for older/smaller phone viewports such as `320 x 568` or `360 x 667` unless explicitly asked.
- The campaign modal doubles as the map screen, win/loss screen, and resume flow.
- On desktop (`profile.ui.allowViewControls`), `GameSession` drives the board camera through the `WebGame` view controls (`tiltBy`, `zoomAt`, `panBetween`, `resetView`): dragging pans (middle or right button anywhere; left button outside build mode once it passes a small threshold, after the normal click has run), the wheel or a trackpad pinch zooms toward the cursor, Shift+wheel and the up/down arrows tilt, `=`/`-` zoom around the center, and `0` resets. View keys repeat while held and are ignored while a modal is open; redraws while the game loop is idle are coalesced to one per frame. The view is not persisted and never changes gameplay bounds.

To run the browser version:

- `npm run dev` (builds both Wasm variants, then starts Vite)

Useful validation commands:

- `npm run build` (Wasm build, `svelte-check`, Vite build)
- `npm run test:rust` (native simulation tests: timing, collisions, effects, lifecycle, HUD/modal strings, seeded campaign smoke runs; renderer tests: camera rig, geometry, WGSL validation, a seeded busy scene)
- `npm run rust:clippy` (native and wasm32, `-D warnings`) and `cargo fmt --all --check`
- `npm run check:runtime` (browser checks on the real page: WebGPU startup, placement painting, a seeded fight, modal focus handling)
- `npm run build:pages`
- `npm run wasm` (after Rust or WGSL edits while `vite` is running; `-- --game-only` skips the labs build)
- `npm run benchmark:compare` (download size, production startup, and crowded-fight frame cost; `--root=DIR` measures another checkout, including pre-Rust TypeScript ones; `--mobile`, `--cpu=4`, `--runs=N`)
- `npm run render:levels`
- `npm run render:3d` (staged, repeatable 3D overview, per-monster/tower close-ups, explosion, tank-death, and breach sequences under `artifacts/3d-board/`: the session loop is frozen and `Math.random` seeded; `--mobile`, `--level=N`, `--out=DIR`)
- `npm run benchmark:3d` (time to ready, pipeline count, per-frame CPU draw cost, instances, and draw calls in a crowded fight; `--mobile`)
- `npm run benchmark:3d:startup` (median per-phase startup timings; `--mobile`, `--cpu=4` CPU throttling, `--cold` for first-visit shader compiles, `--runs=N`)

The supported runtime ranges are declared in `package.json`; `.nvmrc` pins the local/CI Node release, and `rust-toolchain.toml` pins Rust (with `wasm-bindgen-cli` 0.2.129 matching the crate pin; CI installs both). There is currently no general `test`, `lint`, or `format:check` npm script, so do not claim those checks ran unless they have been added.

GitHub Pages branch publishing:

- The site deploys to https://fridman.me/vector-defence-3d/ (the account's Pages domain; vladf1.github.io/vector-defence-3d/ redirects there), so the workflow builds with `npm run build:pages` (base `/vector-defence-3d/`); keep that base in sync with the repository name. The workflow installs the pinned Rust toolchain and `wasm-bindgen-cli` before building.
- To publish a non-main branch for testing, use the existing `Deploy GitHub Pages` workflow with `workflow_dispatch` on that branch. If the `github-pages` environment blocks the branch, temporarily add a deployment branch policy for that exact branch, run the workflow, then remove the temporary policy after the deploy succeeds.
- Do not create or push a `gh-pages` branch for branch testing. The Pages publish path for this repo is the Actions artifact workflow, not a deploy branch workaround.

Generated PNGs under `artifacts/` are ignored by Git and should normally stay uncommitted.

Other render and benchmark tooling:

- `npm run render:levels` renders desktop and mobile route/placement sheets (route and placement data from the engine's `levelSheet`).
- `scripts/benchmark-browser-harness.mjs` centralizes temporary Vite pages, Playwright/Chrome launch fallback, page-error handling, cleanup, result waiting, and PNG data-URL writing for browser benchmarks and render scripts.

Debug pages:

- `debug/index.html` is the index for standalone development tools; `debug/soundboard.html` plays every audio cue. The production build emits the pages under `debug/` as separate Rollup entries; keep debug-only code out of the main game imports.
- `debug/towers.html` / `src/tower-testing.ts` render every tower (levels 1-7), gun and drone shot, missile, and missile blast through the real WebGPU board renderer: the engine's `TowerLab` (`crates/web/src/labs.rs`) stages each cell's subject at the field center of a fresh level runtime with the level scenery hidden (`set_scenery_visible(false)`: no road, portal, spawn gate, or motes; the ground grid stays), settles the views (advancing shots and blast particles with the real entity updates), frames it with `inspect(...)`, and the page copies the WebGPU canvas into a 2D cell canvas. The zoom dialog keeps the staged scene and orbits the camera (drag, wheel/pinch, double-click to reset). When changing tower or projectile visuals, check every level there.

Audio assets:

- The committed `.m4a` files in `src/assets/audio/` are the source of truth for game sound effects.
- `src/audio-manifest.ts` is the single source of truth for cue IDs, soundboard labels, imported asset URLs, cooldowns, gain, and rate variation. `src/game-audio.ts` and `debug/soundboard.html` both consume that manifest. The engine names cues by `AudioCue` discriminant (`crates/core/src/audio.rs`), which indexes `AUDIO_CUE_ORDER`; keep both lists in the same order.
- `src/game-audio.ts` owns Web Audio loading, retryable buffer caching, cooldowns, panning, playback, and `AudioBufferSourceNode.onended` cleanup. Keep rejected loads recoverable and do not queue repeated transient playbacks behind one unresolved load.
- Audio sources are documented in `src/assets/audio/README.md` at the source-pack level.
- When replacing audio, overwrite the relevant `.m4a` files directly, keep the source-pack documentation current, and verify the soundboard/build before committing.

Maintenance preferences:

- Prefer explicit imports of engine constants and types (`crates/core/src/constants.rs`, `types.rs`) so call sites show their dependencies clearly.
- Keep Svelte UI declarative and thin; put formatting and modal/HUD derivation in `crates/core/src/view.rs`.
- Keep simulation logic in `crates/core` (`game.rs` or entities), not in Svelte components or the page shell.
- Keep gameplay rates time-based and compatible with variable substep sizes. Reuse `CalibratedExponentialDecay` for calibrated particle damping instead of introducing `1 - k * delta_seconds` velocity damping.
- Avoid default parameter values in new code (and `Default`-style hidden behavior); make call sites pass behavior-affecting values explicitly.
- When changing models, shaders, or views, run `npm run render:3d`, inspect the close-ups (and `debug/towers.html` for towers/projectiles), and run `npm run build` before calling the visuals done.
- When adding monsters, add a `MonsterKind` value (Rust and `build.rs` name table), a kind file with its `MonsterSpecial` state, a `Monster::new` branch, and campaign usage as needed.
- When adding towers, add a `TowerKind` value (Rust, `build.rs` name table, `src/types.ts`), a kind file with its `TowerSpecial` state, its `TOWER_INFOS` entry in `registry.rs`, and its toolbar SVG in `TOWER_ICON_SVG`.

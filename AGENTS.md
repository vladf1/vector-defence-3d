# Agents

This repository (`vladf1/vector-defence-3d`) is the 3D edition of Vector Defence: the original 2D game lives in `vladf1/vector-defence-2026` (played at https://vd.fridman.me/). Gameplay, campaign, and UI come from the original; this edition replaces its 2D canvas board with a raw-WebGPU 2.5D board. When porting a gameplay change from the original, keep entities free of drawing code and add the 3D presentation in `src/render3d/`.

Active browser implementation lives in this repo root.

Key paths:

- Browser app entry: `src/main.ts`
- Root Svelte component: `src/App.svelte`
- Svelte components: `src/components/`
- Svelte session context: `src/game-context.ts`
- Svelte/game bridge: `src/game-session.ts`
- Desktop/mobile runtime profiles: `src/game-profile.ts`
- Frame backlog/substep policy: `src/simulation-timing.ts`
- Browser HUD/modal view-models: `src/game-view.ts`
- Browser banner text derivation: `src/game-view.ts`
- Browser simulation engine: `src/game-engine.ts`
- Browser level runtime state: `src/level-runtime.ts`
- Browser campaign progress persistence: `src/campaign-progress.ts`
- Browser route geometry/motion samples: `src/route-path.ts`
- Browser placement geometry/tower hit-testing: `src/placement-rules.ts`
- Board renderer interface: `src/board-renderer.ts`
- Renderer loading status / `?timings` flag: `src/renderer-status.ts`
- WebGPU board renderer: `src/render3d/` (entry `src/render3d/webgpu-board-renderer.ts`)
- WGSL shaders: `src/render3d/shaders.ts`
- Toolbar tower icons (static SVG): `src/components/tower-icons.ts`
- Top-bar control icons (vector neon strokes): `src/components/control-icons.ts`, rendered by `src/components/ControlIcon.svelte`
- GPU device prefetch: `src/gpu-device.ts` (adopts the request started by the inline script in `index.html`)
- Build config (renderer-chunk preload plugin): `vite.config.ts`
- 3D board render sheet script: `scripts/render-3d-board.mjs`
- 3D renderer benchmark: `scripts/benchmark-3d-renderer.mjs`
- 3D startup profiler: `scripts/benchmark-3d-startup.mjs`
- Browser gameplay entities: `src/entities/`
- Browser gameplay engine helpers: `src/game-engine/`
- Browser audio orchestration: `src/game-audio.ts`
- Browser audio cue manifest: `src/audio-manifest.ts`
- Tower metadata/shortcuts/toolbar order: `src/entities/towers/tower-registry.ts`
- Browser campaign builder: `src/campaign.ts`
- Shared browser types: `src/types.ts`
- Shared browser constants: `src/constants.ts`
- Shared browser utilities: `src/utils.ts`
- Browser styles: `src/style.css`
- Browser package/scripts: `package.json`
- Browser level data: `game-levels.json`
- Browser audio assets: `src/assets/audio/`
- Level render sheet script: `scripts/render-levels.mjs`
- Browser render/benchmark harness: `scripts/benchmark-browser-harness.mjs`
- Debug tools hub: `debug/index.html`
- 3D tower/projectile sheet page: `debug/towers.html`
- 3D tower/projectile sheet script: `src/tower-testing.ts`
- Audio soundboard: `debug/soundboard.html`

Repository notes:

- The project root is the active browser app repo.
- The browser app in `src/` is the active implementation.
- The browser app is a Svelte 5 + Vite app. Avoid reintroducing hand-built DOM/UI glue when a small Svelte component or view model is the cleaner boundary.
- The board renders only through raw WebGPU (no three.js or other rendering library, and no 2D canvas board). Keeping the download small is a goal: do not add rendering dependencies, bring back a 2D renderer, or add raster UI images (icons are inline SVG data) without being asked.

Current code structure:

- `src/main.ts` owns Svelte app bootstrapping, startup profile selection, and visual-viewport height synchronization. It also starts the renderer chunk import and the GPU device request before Svelte mounts.
- Startup is tuned so nothing waits in series: an inline script in `index.html` starts `requestAdapter()`/`requestDevice()` during HTML parsing (`src/gpu-device.ts` adopts that promise once, and requests a fresh device for later remounts), and the `preload-board-renderer` plugin in `vite.config.ts` adds `<link rel="modulepreload">` tags for the dynamically imported renderer chunk and its imports, so it downloads alongside `main.js` instead of after it runs. Keep both when restructuring startup.
- `src/App.svelte` wires the main shell and creates the shared game session context.
- `src/components/ChromeBar.svelte`, `src/components/GameBoard.svelte`, `src/components/GameModal.svelte`, `src/components/TowerPanel.svelte`, and `src/components/NerdStatsPanel.svelte` own the declarative UI around the canvas.
- `src/game-context.ts` owns the Svelte context helpers for the shared `GameSession`.
- `src/game-session.ts` bridges Svelte stores/events to the imperative game runtime, handles keyboard/pointer input, owns the animation-frame loop and bounded simulation backlog, and publishes HUD/modal snapshots. It mounts a `BoardSurface` (the WebGPU canvas plus a 2D overlay canvas), lazily imports the renderer, holds the simulation while it loads, remounts a fresh renderer if the GPU device is lost, and reports `RendererStatus.Failed` (shown as a "WebGPU required" notice) if WebGPU cannot start. In dev builds it exposes `window.__vectorDefence = { game, sync }` for render/benchmark scripts.
- `src/simulation-timing.ts` owns the bounded substep policy: native high-refresh deltas are preserved, slow frames are split into steps of at most 1/60 second, catch-up work is capped, and drawing still happens once per rendered frame.
- `src/game-profile.ts` owns desktop/mobile logical dimensions, movement/range scales, placement geometry, UI flags, and startup profile selection.
- `src/game-engine.ts` owns gameplay state, campaign progression, lifecycle resolution, and `updateSimulation(...)`; rendering is a separate once-per-frame `draw()` call.
- `src/level-runtime.ts` owns per-level mutable runtime collections such as monsters, towers, projectiles, particles, links, placement state, money, wave counters, and route path.
- `src/campaign-progress.ts` owns campaign unlock/star persistence through `localStorage`, with an in-memory fallback when browser storage is unavailable.
- `src/game-engine/update-context.ts` defines the per-update read model and the `UpdateResult` accumulator through which monsters, towers, projectiles, and drones report lifecycle outcomes, spawned entities, presentation effects, and sounds.
- `src/game-engine/collision-detection.ts` owns the active-monster swept-collision index plus its linear comparison implementation.
- `src/game-engine/monster-factory.ts` owns monster class lookup, level hit-point scaling, and splitter child creation through explicit speed-scale and level-index inputs; it does not depend on `Game`.
- `src/game-engine/combat-effects.ts` owns shared hit, laser, missile, and escape particle construction. Monster polygon breakup lives in `src/entities/monsters/death-effect-helpers.ts` and `src/entities/monsters/polygon-shard-splitter.ts`.
- `src/placement-rules.ts` owns tower placement geometry and board hit-testing through explicit route/tower inputs; it should not import `Game`.
- `src/route-path.ts` owns route drawing commands plus the sampled motion path entries used by monster movement.
- `src/board-renderer.ts` defines the `BoardRenderer` boundary `Game` talks to (resize, draw, visible bounds, player camera controls, canvas action hit tests, client-to-field picking, dispose) plus the `DetachedBoardRenderer` used while no board is mounted. `Game` owns no canvases; the session attaches a renderer with `game.setRenderer(...)`.
- `src/renderer-status.ts` owns the renderer loading status values and the `?timings` flag.
- `src/game-view.ts` owns HUD/modal view-model generation for Svelte.
- `src/entities/` owns active gameplay entities split by concern: towers, monsters, projectiles, and effects.
- `src/render3d/` is the raw-WebGPU board renderer. It is only reached through a dynamic import (started early from `main.ts`), reads `Game.runtime` every frame, and never mutates simulation state:
  - `webgpu-board-renderer.ts` owns adapter/device/canvas setup, the frame uniform buffer (camera matrices, lights, field constants, time, flash lights), startup (with per-phase `StartupTimings`), resize, and per-frame orchestration: refill batches, then record one command encoder (scene pass, bloom chain, composite) and submit once.
  - `shaders.ts` builds every WGSL module with `createShaderSources(...)`, baking the startup-fixed scene constants (hemisphere/key/rim light colors and directions, field size, road half width, smoke-puff blobs) in as WGSL `const`s. The `Frame` uniform (`FrameLayout` gives its float offsets) holds only per-frame data: camera matrices, camera position + time, and a fixed four-slot point-light array. Six modules: `neon`, `ground`, `road`, `effect` (ribbon/decal/health bar/range/ground glow/road chevron, selected per instance by `EffectMode` in the tint's w), `sprite` (glow/smoke, `SpriteMode` in the shape's w), and `post` (downsample/blur/composite, `PostMode` in the pass parameters). The post code tone-maps by hand: the brightest channel goes through the ACES filmic curve and scales the color (so saturated neon keeps its hue instead of bleaching to pastel), handing over to full per-channel ACES (matrices written row by row, so vectors multiply on the left) only for white-hot values such as explosion cores; then sRGB encoding.
  - `gpu-pipelines.ts` creates the 8 pipelines (neon, ground, road, effect, health bar, sprite, bloom, composite) with `createRenderPipelineAsync` in one parallel batch, with explicit bind group layouts. Nothing compiles after startup. Effects and sprites use premultiplied-alpha blending, so one pipeline serves additive layers (shader alpha 0) and normally blended ones. `?shaderSalt=N` (dev only) perturbs every fragment shader so driver caches miss. `gpu-flags.ts` holds the WebGPU usage bit constants (the TypeScript DOM lib lacks them).
  - `post-processing.ts` owns the HDR targets (rgba16float scene color with optional MSAA resolve, depth24plus, two bloom levels plus scratch) and records the prefilter/downsample, separable blur, and composite passes (vignette, ACES, sRGB) onto the canvas: all seven passes use the one `post` module and a shared bind group layout, with a 1x1 placeholder in unused texture slots.
  - Draw order matches the previous three.js renderer: opaque neon batches, ground, road, health bars (no depth test), then blended layers in fixed order (road chevrons, ribbon, smoke, glow, decal, range, ground glow). Road chevrons are rigid quads that `BoardScene` places along the route every frame, turned to the local heading and lifted just above the road sample they lie on, so turns never bend them (the road shader itself paints only the channel and edges). Blending is `src-alpha`-based additive or normal alpha.
  - `camera-rig.ts` (plain matrices from `math.ts`) fits the tilted perspective camera to the field, ray-casts pointer picking onto the ground, projects field points for the overlay, and applies screen shake to a separate render camera (picking never shakes). `createBoardCameraRig(...)` is the board framing (also used by `check:runtime` for real visible bounds). The player view controls (`tiltBy`, `zoomAt`, `panBetween`, `resetView`) apply on top of the fitted framing: tilt re-fits the whole field (straight down to 0.6 rad), zoom (1x to 4x) keeps the ground under the cursor in place, pan grabs the ground and is clamped so the view never drifts past the field, and screen shake is scaled back by the zoom. Picking and projection follow the player view, but `fieldBounds` (placement and culling) always comes from the default framing, so the view is presentation only. `inspect(...)` (exposed as `InspectableBoardRenderer.inspect`) frames a shake-free orbit close-up (`InspectView`: field point, visible height, yaw, tilt; `BOARD_TILT_RADIANS` matches the board) with a distance-scaled near plane, for render scripts and the tower sheet.
  - `render-batches.ts` is the complete, fixed list of instanced batches, each naming its pipeline by key. `instanced-batch.ts` (24 floats per instance: column-major transform, tint, extras) and `sprite-batch.ts` (12 floats: position/rotation, size/shape, color) refill typed arrays every frame and upload the used range with one `writeBuffer`.
  - Lighting is Lambert (the neon look comes from emissive trims, rims, an analytic key-light highlight, and bloom). Neon batches pick a `NeonMode` (written to the tint w): `Metal` parts (towers, scenery) get a white highlight, `Creature` parts (every monster batch and the shards) take highlight and rim in their own color, so monsters read in their original colors. Every monster needs a glowing (`glow = 1`) outline trim in its color that matches the original 2D silhouette from above; that trim is what carries its identity. Shadows are soft blob decals pushed through `RenderBatches.pushBlobShadow(...)`, offset along the key light by each object's height; there are no shadow maps.
  - `models.ts` / `geometry-kit.ts` build all procedural low-poly models as non-indexed triangle soups (monsters at unit radius, towers in field units); neon parts become interleaved `position(3) normal(3) glow(1)` vertices, and flat effect quads are `position(3) uv(2)`. `extrudeOutline(...)` reproduces three.js `ExtrudeGeometry` bevels, including the sqrt(2) miter cap.
  - `puff-blobs.ts` generates the seeded blob parameters of the former 2x2 smoke-puff canvas atlas; `puffCoverage(...)` in the effect and sprite shaders evaluates them per pixel (smoke sprites, and the noisy scorch-decal mask), matching the old atlas within ~2/255 without any texture.
  - `monster-view.ts`, `tower-view.ts`, `projectile-view.ts`, `effect-view.ts`, `placement-view.ts`, and `board-scene.ts` compose entities into batches; `fx-system.ts` owns renderer-only spectacle (pooled 3D sparks, fireballs, smoke, ground rings, a fixed flash-light pool written into the frame uniforms, scorch decals, camera trauma; slow-tower pulses deliberately show only their links, with no ring); `overlay-view.ts` draws the screen-space tower actions and escape counter on a 2D canvas above the WebGPU canvas.
  - `render-quality.ts` holds the desktop/mobile budgets and the dynamic-resolution governor.
- `src/entities/monsters/monster.ts` owns shared monster movement, damage, slow recovery, and lifecycle outcome reporting.
- Concrete monster classes live under `src/entities/monsters/` and own monster-specific base stats, outlines, animation state, death effects, and special behavior (`berserker` ramps speed as it loses health; `bulwark` mitigates incoming damage).
- Projectile classes live under `src/entities/projectiles/`; import the exact base or concrete file (`projectile.ts`, `gun-projectile.ts`, `drone-projectile.ts`, `missile.ts`, or `drone.ts`) rather than adding a barrel.
- General effect classes live under `src/entities/effects/`; import the concrete effect file directly.
- `src/entities/drone-visuals.ts` holds the drone accent colors; `src/entities/monsters/tank-effects.ts` owns the tank's track-print and detached-turret particles and turret offset. Keep such helpers narrow and colocated with the entities that use them.
- Tower classes live under `src/entities/towers/`; `Tower` in `tower.ts` owns shared targeting/upgrade/selection behavior.
- `src/entities/towers/tower-registry.ts` is the source of truth for tower class lookup, keyboard shortcuts, and toolbar order (`TOWER_CLASSES`). Toolbar icons are static SVG in `src/components/tower-icons.ts`.
- Towers, projectiles, drones, monsters, and presentation effects update through `UpdateContext`; gameplay entities report additions and lifecycle outcomes through `UpdateResult`. Do not give them a `Game` dependency when the context/result boundary is sufficient.
- `src/campaign.ts` turns the ten authored routes into the campaign.
- `src/game-view.ts` derives banner, HUD, and modal presentation data; keep text/formatting policy out of Svelte components and the renderer where practical.
- `src/types.ts` is the source of truth for shared browser types such as `TowerKind`, `MonsterKind`, `GameState`, `Point`, `LevelData`, `WaveData`, and `HudSnapshot`.
- `src/constants.ts` and `src/utils.ts` are shared by the active runtime, so prefer reusing those helpers instead of re-declaring gameplay constants or math utilities.
- `src/entities.ts`, `src/entities/monsters.ts`, `src/entities/projectiles.ts`, and `src/entities/effects.ts` have been removed; do not recreate monolithic entity files or barrel files unless there is a clear payoff.

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
- `GameState`, `MonsterKind`, and `TowerKind` are `as const` value objects with derived union types in `src/types.ts`, not TypeScript enums.
- Keep `game-levels.json` monster identifiers as plain strings. `src/game-engine.ts` applies the selected mobile overrides and normalizes JSON route data before `src/campaign.ts` expands it.
- `game-levels.json` provides the ten campaign routes. The actual playable campaign data is generated at runtime by `createCampaignLevels(...)`, which expands those authored routes into per-wave monster sequences and build windows.
- Authored routes do not own `monsterCount`; playable `LevelData.monsterCount` is derived from the generated waves in `src/campaign.ts`.
- Monster constructors pass private named constants to `Monster` with `super(path, COLOR, SPEED, HIT_POINTS, BOUNTY, RADIUS)`.
- Monster constructor stats use `hitPoints`, not `hp`.
- `hitPoints` is current monster health. `maxHitPoints` is the full-health denominator used by the health bar.
- Monster constructors take the concrete `PathEntry[]` path they should follow, not `LevelData`.
- For unusual spawn positions, build a new `PathEntry[]` with route-path helpers such as `createPathEntriesFromDistance(...)` instead of passing raw level points to monsters.
- Monsters report `killed` and `escaped` lifecycle outcomes through `UpdateResult`; `Game` resolves those outcomes after monster updates.
- Concrete monster classes should not import `Game` or call game orchestration methods directly.
- Monster instances do not carry `MonsterKind`; use `MonsterKind` for level/campaign data and `instanceof` for runtime class-specific behavior.

Gameplay / UI notes:

- The campaign is a fixed 10-level progression with unlocks and stars persisted in browser `localStorage`; `CampaignProgressStore` falls back to memory if storage access fails.
- Initial build time is campaign-driven, not a fixed global delay: early levels start around 10 seconds and later ones reach 14 seconds.
- Intermission build windows between later waves are shorter and are generated per wave in `src/campaign.ts` (roughly 2.5 to 5.5 seconds).
- Level 1 is an introductory route and is not intended to showcase the full monster roster; its generated waves should stay within the monster pool authored for `Outer Line` in `game-levels.json`.
- Later campaign waves introduce heavier and specialist monsters such as `tank`, `splitter`, `bulwark`, and `berserker`. Splitters burst into weakened runner children when killed.
- Monster spawning is orchestrated by `Game.spawnMonster(...)`, while monster construction/scaling and splitter children are centralized in `src/game-engine/monster-factory.ts`; tower creation is registry-driven through `Game.createTower(...)` and `getTowerClass(...)`.
- The main frame loop preserves native high-refresh updates, uses bounded substeps to recover slow-frame time, draws once, and freezes background-tab time by resetting the frame clock on visibility changes.
- Bulwark flat armor applies only to discrete `takeDamage(...)` hits. Continuous effects use `takeContinuousDamage(...)`; laser beam damage is analytically integrated over its fade so results do not depend on refresh rate.
- Entities hold no drawing code. All rendering lives in `src/render3d/`, which reads small public presentation getters on entities (for example `currentMouthAngle`, `getDashPulse()`, `getReloadProgress()`). When adding a monster or tower, add its model in `models.ts`, a batch in `render-batches.ts`, and an `instanceof` branch in the matching view; expose read-only presentation state rather than moving rendering into the entity.
- Shader budget rules: all entity parts share the one `neon` pipeline and instance tint carries identity, so new visuals should add geometry/batches, not pipelines; a new effect look is a new `EffectMode`/`SpriteMode` branch, not a new module. Pipeline state (MSAA sample count, formats, light slot count) is fixed at startup; resolution is the only runtime quality knob. A new pipeline must be added to the parallel batch in `createPipelines(...)`, never created lazily mid-game. Ribbons are single-sided and always faced toward the camera.
- Safari/WebKit compile rules (measured on Safari 27 and Playwright WebKit): WebKit compiles pipelines one at a time, pays for every distinct pipeline state (blend state included) again at its first draw, and a fragment stage that reads a uniform buffer (~130 ms) or a texture (~250 ms) compiles far slower than math-only fragment code (~5 ms). So keep fragment code free of buffer/texture reads (pass per-frame values from the vertex stage as flat varyings, as ground and road do with the point lights; bake startup constants into the WGSL), share modules, and keep the pipeline count low. Only `neon` (per-pixel point lights) and `post` read resources in fragment code. WebKit keeps compiled shaders across launches, so the full cost lands on the first visit after shader text changes.
- Startup rules: first-visit cost is dominated by GPU driver shader compiles, so every pipeline is created asynchronously in one parallel batch while the CPU builds geometry; a warm-up frame touching every batch is drained before the board reports ready. `npm run benchmark:3d:startup -- --cold` measures first-visit startup by salting every shader (`?shaderSalt=N`, dev only) so driver caches miss. `?timings` shows the startup phase breakdown on the board in any build.
- 3D effects are driven from simulation state: the effect view maps each simulation particle/link class to a 3D treatment (shards and turret debris get height, gravity, tumble, and bounce), monster disappearances are classified into kill/escape by hit points and path progress, and `ShockwaveEffect` instances trigger missile blasts. Presentation time comes from `Game.simulationSeconds`, so 3D effects freeze with the game.
- Monster-specific visual animations, such as tank turret spins or packman mouth/body flourishes, should live on the concrete monster class and run through `updateSpecial(...)`; if an animation changes visible body geometry or orientation, keep that current shape reflected in the monster's `addDeathEffect(...)` outline/rotation so shards match the death frame.
- Use shared easing helpers from `src/utils.ts` for monster animation progress, and keep mutually exclusive monster flourishes in one local state machine when they should not overlap.
- Tower classes own their attack behavior and presentation state. Shared targeting/selection concerns belong in `Tower`.
- 3D towers follow the original's design language: a near-black plinth (`tower-base`, tinted `PLINTH`), a glowing rim on its top edge in the original's base-stroke color (`tower-rim`: white, softer for missile and drone), an upgrade halo from level 2 on (`upgrade-ring`, sized from `TOWER_RADIUS`/`TOWER_UPGRADE_RING_*` in the tower's original ring color), and the weapon in its accent colors. Keep the plinth dark so the rim, halo, and weapon carry each tower's identity.
- Svelte components should consume `HudSnapshot` and `ModalView` data rather than reaching into the `Game` object directly.
- The HUD selection card supports upgrade, sell, and cancel-build actions; keep those interactions flowing through `GameSession` and the HUD snapshot rather than binding components directly to `Game`.
- Mobile layout support starts at a `375 x 812` CSS-pixel viewport. Do not optimize for older/smaller phone viewports such as `320 x 568` or `360 x 667` unless explicitly asked.
- The campaign modal doubles as the map screen, win/loss screen, and resume flow.
- On desktop (`profile.ui.allowViewControls`), `GameSession` drives the board camera through the `BoardRenderer` view controls: dragging pans (middle or right button anywhere; left button outside build mode once it passes a small threshold, after the normal click has run), the wheel or a trackpad pinch zooms toward the cursor, Shift+wheel and the up/down arrows tilt, `=`/`-` zoom around the center, and `0` resets. View keys repeat while held and are ignored while a modal is open; redraws while the game loop is idle are coalesced to one per frame. The view is not persisted and never changes gameplay bounds.

To run the browser version:

- `npm run dev`

Useful validation commands:

- `npm run build`
- `npm run check:runtime` (browser regression checks for timing, collisions, effects, and UI state)
- `npm run build:pages`
- `npm run dev`
- `npm run benchmark:update`
- `npm run benchmark:death-effects`
- `npm run render:levels`
- `npm run render:3d` (staged, repeatable 3D overview, per-monster/tower close-ups, explosion, tank-death, and breach sequences under `artifacts/3d-board/`: the session loop is frozen and `Math.random` seeded; `--mobile`, `--level=N`, `--out=DIR`)
- `npm run benchmark:3d` (time to ready, pipeline count, per-frame CPU draw cost, instances, and draw calls in a crowded fight; `--mobile`)
- `npm run benchmark:3d:startup` (median per-phase startup timings; `--mobile`, `--cpu=4` CPU throttling, `--cold` for first-visit shader compiles, `--runs=N`)

The supported runtime ranges are declared in `package.json`; `.nvmrc` pins the local/CI Node release. There is currently no general `test`, `lint`, or `format:check` script, so do not claim those checks ran unless they have been added.

GitHub Pages branch publishing:

- The site deploys to https://fridman.me/vector-defence-3d/ (the account's Pages domain; vladf1.github.io/vector-defence-3d/ redirects there), so the workflow builds with `npm run build:pages` (base `/vector-defence-3d/`); keep that base in sync with the repository name.
- To publish a non-main branch for testing, use the existing `Deploy GitHub Pages` workflow with `workflow_dispatch` on that branch. If the `github-pages` environment blocks the branch, temporarily add a deployment branch policy for that exact branch, run the workflow, then remove the temporary policy after the deploy succeeds.
- Do not create or push a `gh-pages` branch for branch testing. The Pages publish path for this repo is the Actions artifact workflow, not a deploy branch workaround.

Generated PNGs under `artifacts/` are ignored by Git and should normally stay uncommitted.

Other render and benchmark tooling:

- `npm run render:levels` renders desktop and mobile route/placement sheets.
- `node scripts/render-polygon-shards.mjs` renders seeded polygon-breakup variants under `artifacts/polygon-shards/`.
- `npm run benchmark:death-effects` measures synchronous monster death-effect construction with seeded, resettable randomness.
- `npm run benchmark:update` reports both the reconstructed busy-runtime method timings and separate seeded `Game.updateSimulation(...)` timings with drones and lifecycle effects. Whole-engine samples exclude drawing and fixture maintenance; they are not full browser-frame timings.
- `scripts/benchmark-browser-harness.mjs` centralizes temporary Vite pages, Playwright/Chrome launch fallback, page-error handling, cleanup, result waiting, and PNG data-URL writing for browser benchmarks and render scripts.

Debug pages:

- `debug/index.html` is the index for standalone development tools; `debug/soundboard.html` plays every audio cue. The production build emits the pages under `debug/` as separate Rollup entries; keep debug-only code out of the main game imports.
- `debug/towers.html` / `src/tower-testing.ts` render every tower (levels 1-7), gun and drone shot, missile, and missile blast through the real WebGPU board renderer: each cell stages one subject at the field center of a fresh level runtime with the level scenery hidden (`setSceneryVisible(false)`: no road, portal, spawn gate, or motes; the ground grid stays), settles the views (advancing shots and blast particles with the real entity updates), frames it with `inspect(...)`, and copies the WebGPU canvas into a 2D cell canvas. The zoom dialog keeps the staged scene and orbits the camera (drag, wheel/pinch, double-click to reset). When changing tower or projectile visuals, check every level there.

Audio assets:

- The committed `.m4a` files in `src/assets/audio/` are the source of truth for game sound effects.
- `src/audio-manifest.ts` is the single source of truth for cue IDs, soundboard labels, imported asset URLs, cooldowns, gain, and rate variation. `src/game-audio.ts` and `debug/soundboard.html` both consume that manifest.
- `src/game-audio.ts` owns Web Audio loading, retryable buffer caching, cooldowns, panning, playback, and `AudioBufferSourceNode.onended` cleanup. Keep rejected loads recoverable and do not queue repeated transient playbacks behind one unresolved load.
- Audio sources are documented in `src/assets/audio/README.md` at the source-pack level.
- When replacing audio, overwrite the relevant `.m4a` files directly, keep the source-pack documentation current, and verify the soundboard/build before committing.

Maintenance preferences:

- Prefer named imports from `src/constants.ts` and `src/types.ts` so call sites show their dependencies clearly.
- Keep Svelte UI declarative and thin; put formatting and modal/HUD derivation in `src/game-view.ts`.
- Keep imperative simulation logic in `src/game-engine.ts` or entity classes, not in Svelte components.
- Keep gameplay rates time-based and compatible with variable substep sizes. Reuse `CalibratedExponentialDecay` for calibrated particle damping instead of introducing `1 - k * deltaSeconds` velocity damping.
- Avoid default parameter values in new code; make call sites pass behavior-affecting values explicitly.
- When changing models, shaders, or views, run `npm run render:3d`, inspect the close-ups (and `debug/towers.html` for towers/projectiles), and run `npm run build` before calling the visuals done.
- When adding monsters, add a `MonsterKind` value, a concrete monster class, a `createBaseMonster(...)` branch in `src/game-engine/monster-factory.ts`, and campaign usage as needed.
- When adding towers, add a `TowerKind` value and concrete tower class, add it to `TOWER_CLASSES` in `tower-registry.ts`, and add its toolbar SVG to `TOWER_ICON_SVG`. `Game.createTower(...)` already resolves classes through the registry; do not add a kind switch there.

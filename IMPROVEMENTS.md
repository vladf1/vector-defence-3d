# Vector Defence Improvement Backlog

Last reviewed: 2026-09-22

This document captures the repository-wide correctness, performance, tooling,
accessibility, and maintainability audit. Keep item numbers stable so work can
be referenced across Codex tasks, commits, and pull requests.

## Status legend

- `[ ]` Not started
- `[~]` Partially addressed or awaiting a decision
- `[x]` Completed and verified
- **Confirmed defect**: directly reproducible incorrect behavior
- **Measured hotspot**: supported by the repository's current benchmarks
- **Investigation**: measure or decide before changing behavior

## Highest priority

### 1. [x] Make Bulwark damage independent of refresh rate

**Type:** Confirmed defect — completed 2026-07-17

Bulwark's flat armor previously ran once per animation-frame laser callback.
A base laser therefore dealt about 6.9 damage at 15 Hz and 64.4 at 144 Hz.

Implemented:

- Added an explicit continuous-damage path in
  `src/entities/monsters/monster.ts`.
- Kept Bulwark flat armor for discrete projectile/lightning impacts in
  `src/entities/monsters/bulwark-monster.ts`.
- Analytically integrated the fading laser beam in
  `src/entities/towers/laser-tower.ts`.
- Updated the tower render fixture for the new monster damage method.

Verification:

- Base laser damage is exactly `33.333333` at 15, 30, 60, 90, 120, and
  144 Hz.
- A level-6 laser deals `83.333333`, so upgrades continue to scale.
- A discrete 10-point hit still deals 6.5 damage through Bulwark armor.
- `npm run build`, the update benchmark, and a fresh tower render sheet pass.

### 2. [x] Replace gameplay `shadowBlur` effects

**Type:** Measured hotspot — completed 2026-07-17

Forced-GPU-flush measurements found upgraded Lightning tower draws around
1.62–1.72 ms, compared with roughly 8.75 us at the base level without its
blurred glow. Active gun and missile muzzle flashes measured approximately
0.82–0.87 ms per draw.

Implemented:

- Replaced Lightning tower, gun/missile muzzle flash, and laser beam blur with
  layered translucent fills and strokes.
- Extended the tower benchmark and render sheet so active laser and muzzle
  effects are actually exercised.
- Preserved the bright core, colored glow hierarchy, and per-level color and
  geometry progression in a fresh `artifacts/tower-render-layered-final.png`.

Forced-GPU-flush medians across the affected levels changed from:

- Active laser: 2,761–2,891 us to 101–119 us.
- Upgraded Lightning: 1,471–1,543 us to 21–29 us.
- Active gun muzzle flash: 779–801 us to 14–20 us.
- Active missile muzzle flash: 738–773 us to 14–28 us.

A live Chrome battle smoke through two waves exercised gun, laser, and missile
effects with no console warnings or errors. The available Chrome control channel
did not expose DevTools trace capture; the repaired forced-GPU-flush benchmark,
fresh render sheet, and live smoke were accepted as sufficient verification.

### 3. [~] Apply particle and link budgets before construction

**Type:** Confirmed inefficiency

Particle recipes and monster death effects now check the remaining particle
capacity before construction. Link effects also check remaining capacity
before construction, so saturated frames avoid discarded link objects.
Prioritizing effects and reporting dropped counts remain open.

Actions:

- Prioritize gameplay-readable effects when capacity is scarce.
- Expose dropped-effect counts in nerd stats.
- Consider pooling only after the admission fix is benchmarked.

### 4. [x] Recover elapsed time with bounded simulation substeps

**Type:** Correctness and consistency — completed 2026-07-17

The old frame loop discarded elapsed time beyond 66.7 ms. The replacement
preserves native high-refresh updates while splitting slow frames into steps no
larger than 1/60 second, then draws only once.

Implemented:

- Added bounded timing policy in `src/simulation-timing.ts`.
- A 120 Hz frame receives one 1/120-second update; it does not duplicate a
  60 Hz simulation frame.
- A 100 ms frame receives six updates of at most 1/60 second.
- At most eight substeps run in one rendered frame; remaining time is retained
  for subsequent frames.
- Backlog is capped at 500 ms to prevent unbounded catch-up work.
- The frame clock and backlog reset when animation stops or tab visibility
  changes, so background-tab time is intentionally frozen.
- Split `Game.updateSimulation()` from the once-per-frame `Game.draw()`.
- A one-off timing validator passed before being removed.

### 5. [x] Replace linear velocity damping with exponential damping

**Type:** Correctness — completed 2026-07-17

Replaced the four `1 - k * deltaSeconds` particle damping paths with shared,
analytically integrated exponential decay.

Implemented:

- Covered base particles plus escape fragments, glass shards, and tank turrets.
- Calibrated each decay to preserve the old 60 Hz velocity and displacement.
- Integrated displacement analytically so both position and velocity are
  invariant across refresh rates, not only final velocity.
- Cached decay factors per timestep so a particle class performs at most one
  `Math.exp` calculation per distinct substep rather than one per particle.
- A one-off 15–144 Hz damping validator passed before being removed.

### 6. [~] Establish an automated test suite

**Type:** Correctness infrastructure

`npm run check:runtime` now covers timing, combat, effects, level validation,
UI state, and browser modal behavior. A general unit/integration command and
CI gate are still open. Extend deterministic coverage around:

- Swept projectile collision and spatial-index cell boundaries.
- Route sampling, distances, curves, and headings.
- Placement boundaries, route clearance, and tower overlap.
- Wave spawning, intermissions, completion, and defeat transitions.
- Splitter children and monster lifecycle ordering.
- Campaign generation and progress migration.
- Tower upgrade costs, ranges, and damage scaling.
- Invalid level-data rejection.
- Playwright smoke flows for start, place, upgrade, sell, pause, and mobile.

CI should run the tests before the Pages build.

## Correctness and reliability

### 7. [~] Reject invalid monster identifiers

Level normalization rejects unknown monster strings and empty sequences with
the level name and bad value. Add the field and array index to its error,
then validate the remaining authored level fields.

### 8. [ ] Define laser range semantics

Target acquisition uses the advertised range, but the active 1,000-unit beam
can damage aligned monsters beyond it. Decide between:

- Clipping the damage segment to tower range, or
- Documenting and testing unlimited piercing after in-range acquisition.

Update the HUD summary to match the chosen behavior.

### 9. [ ] Confirm the tower range-upgrade formula

`Tower.upgrade()` adds `level * TOWER_RANGE_UPGRADE_STEP`, producing larger
increments at every level. Confirm this is intended, encode it in tests, and
rename the constant if it represents progressive rather than fixed growth.

### 10. [ ] Define simultaneous escape behavior

After defeat becomes pending, additional monsters escaping in the same frame
can still generate redundant escape bursts and sounds. Decide whether a
simultaneous breach should produce one presentation or one per monster, then
batch or suppress effects accordingly.

### 11. [ ] Store level results by stable level ID

Campaign stars are keyed by array index. Reordering or inserting routes can
attach saved results to the wrong level. Store by stable campaign ID and add a
one-time migration from the current index keys.

### 12. [x] Remove or restore ownership of authored `monsterCount`

**Type:** Correctness and data ownership — completed 2026-07-17

Removed the dead authored `monsterCount` values from `game-levels.json` and
from the JSON input type. `CampaignRouteData` now represents normalized route
input without a count, while playable `LevelData` requires the count derived
from its generated waves. A one-off validator confirmed every desktop and
mobile level reports exactly the sum of its generated wave counts, then was
removed.

### 13. [x] Validate tower shortcut uniqueness

Drone and Lightning both accept `5`, but never appear together on an authored
route. Level normalization now rejects per-level shortcut collisions and the
runtime check covers that rule.

### 14. [x] Make audio loading recoverable

**Type:** Reliability — completed 2026-07-17

Failed fetches and decodes now remove their cached promise so the next preload
or playback request retries the asset. Each cue reports its first load failure
once, and preload/playback callers consume the rejected promise without an
unhandled rejection.

The existing unlock path still preloads every cue. While a cue is loading, at
most one playback waits for it; later rapid-fire requests are deliberately
dropped instead of building a delayed burst.

### 15. [x] Use `AudioBufferSourceNode.onended` for cleanup

**Type:** Reliability — completed 2026-07-17

Source, gain, and optional stereo-panner nodes now disconnect through an
idempotent `AudioBufferSourceNode.onended` handler. The old duration timer was
removed, so playback-rate variation no longer makes cleanup early or late. A
synchronous `start()` failure also runs the same cleanup path.

A focused browser validator confirmed retry after fetch and decode failures,
one-time failure reporting, single queued playback per loading cue, and
exactly-once node cleanup before it was removed. The production build passes
with zero Svelte errors or warnings.

### 16. [ ] Model non-empty geometry explicitly

`noUncheckedIndexedAccess` currently reports 193 errors, concentrated in
route paths, polygon splitting, render geometry, and fixed-size visual arrays.
Introduce focused `NonEmptyArray<T>` types and validation at data boundaries
instead of broad non-null assertions.

### 17. [~] Adopt stricter TypeScript checks incrementally

Audit results:

- `noUnusedLocals` / `noUnusedParameters`: enabled 2026-07-20 with zero
  remaining violations; removed the unused `GlassShardParticle` import from
  `src/explosion-testing.ts`.
- `noUncheckedIndexedAccess`: 193 errors.
- `exactOptionalPropertyTypes`: 31 errors.
- `noImplicitOverride`: 11 errors.

Next, enable override checks and clean up optional-property handling. Stage
unchecked indexing after non-empty geometry types exist.

### 18. [ ] Make runtime states explicit

`LevelRuntime` represents both empty and active levels through optional
properties, while `Game` mutates several public runtime collections. Consider
explicit `LevelRuntime.empty()` / `LevelRuntime.forLevel(...)` constructors
and narrow transition methods for start, pause, win, loss, and restart.

## Performance

### 19. [x] Measure and reduce polygon-shard construction cost

**Completed:** 2026-07-17

`npm run benchmark:death-effects` now measures synchronous
`addDeathEffect(...)` construction in Chrome. It uses resettable seeded
randomness, a warmup of at least 2,000 deaths, 500 frame-spaced samples, and 15
trials of 1,000 consecutive deaths so splitter revisions see identical topology
work and the throughput comparison is not dominated by frame scheduling.

The geometry hot-path optimizations preserve the splitter configuration and
random choices, but now:

- carries each shard's area instead of recomputing it during selection,
  validation, and scoring;
- computes bounds, centroid, diagonal, and edge clearance once per split round;
- uses squared distances for boundary clearance, point merging, and collinear
  cleanup, avoiding repeated square roots;
- avoids candidate/filter arrays, redundant final simplification, and internal
  point clones while still returning independently owned vertices.

| Balanced-roster construction | Before | After | Change |
| --- | ---: | ---: | ---: |
| Mean per death, sustained throughput | 74.107 us | 48.173 us | -35.0% |
| 1,000 deaths, p50 | 73.9 ms | 48.0 ms | -35.0% |
| 1,000 deaths, p95 | 77.6 ms | 52.1 ms | -32.9% |
| 48 deaths, p50 | 3.7 ms | 3.0 ms | -18.9% |
| 48 deaths, p95 | 4.4 ms | 3.7 ms | -15.9% |
| 48 deaths, p99 | 4.7 ms | 4.1 ms | -12.8% |

A follow-up experiment capped each split round at 16 candidates instead of 42.
Two alternating 50-trial A/B pairs measured:

| 1,000-death throughput | 42 candidates | 16 candidates | Change |
| --- | ---: | ---: | ---: |
| Mean | 50.230 ms | 47.711 ms | -5.0% |
| p50 | 49.9 ms | 47.75 ms | -4.3% |
| p95 | 52.55 ms | 50.2 ms | -4.5% |

The 16-candidate cap is now the default. Aggregate shard and particle counts
were unchanged in the A/B runs. Eight of nine seeded polygon-shard sheets
remained byte-identical; the square sheet selected alternate valid layouts with
the same shard counts, and both versions passed visual inspection. The full
explosion sheets were regenerated with the cap, with representative square and
tank sequences inspected for breakup quality.

The splitter still accounts for about 97% of this isolated construction slice;
the result excludes simulation, drawing, and GPU work. Mean shard and particle
counts are unchanged. Before applying the candidate cap, all nine seeded sheets
were byte-identical and the full explosion sequences were visually checked
across simple, animated, and multi-part monster bodies.

If substantially more headroom is needed later, the next paths are a small
seeded topology bank per static or quantized body state, or an effect-budget
admission check before splitting when particles would be discarded anyway.

### 20. [ ] Reduce link-effect rendering cost

`LightningLinkEffect` measured about 34 us per forced-flush draw and creates
temporary point objects each draw. At the 120-link cap this can become
material.

Actions:

- Reuse instance scratch storage.
- Simplify segment count as links fade.
- Batch links by composite mode, stroke width, and color where practical.
- Benchmark before and after each change.

### 21. [ ] Cache gradients only after isolated measurements

Smoke, shockwaves, and missile exhaust create gradients while drawing.
Benchmark cached gradients or pre-rendered sprites independently. Prior work
showed that bitmap conversion is not automatically faster than vector draws.

### 22. [x] Cache canvas geometry for pointer movement

Avoid repeated layout reads during high-frequency pointer movement. Cache the
canvas rectangle and logical viewport, updating them on resize, scroll, and
pointer-down so coordinate conversion remains correct.

### 23. [x] Preserve the spatial collision index

The extreme update fixture measured approximately:

- Spatial index: 1.367 ms/frame.
- Linear search: 23.868 ms/frame.

This is roughly a 17.5x improvement. Keep the index and add tests around cell
boundaries, hash collisions, target ordering, and fast swept projectiles.

### 24. [x] Continue using cached backgrounds and full animated redraws

Canvas backing-store scaling and background caching are correct. Dirty-rect
rendering would add complexity while most of the scene is animated. Revisit
only if fresh live profiling shows full redraw is the limiting factor after
items 2, 3, 19, and 20.

### 25. [x] Pause auxiliary animation pages while hidden

The explosion and escape labs continuously request frames. Suspend their
loops on `visibilitychange` and resume without a large accumulated delta.

## Benchmarks, build, and tooling

### 26. [x] Repair draw-benchmark reporting

**Type:** Benchmark correctness — completed 2026-07-17

The full draw suite uses one benchmark order, then reports min/max across
orders. Those values are identical and hide the five samples collected inside
each benchmark.

Implemented:

- Reports sample count plus min, median, mean, and max directly.
- Runs normal, reversed, and deterministically randomized orders.
- Keeps forced-GPU-flush results separate from bounded CPU submission timing.
- Drains the canvas command queue outside CPU-submission timing so samples do
  not accumulate GPU backpressure.
- Uses larger full-suite batches so even `Particle.draw()` has non-zero medians.
- Supports targeted `--filter=...` runs and exposes `benchmark:draw` and
  `benchmark:draw:towers` package scripts.

Verification:

- Every summary contains 15 samples across three orders.
- The cheapest measured draw reported 0.167 us CPU submission and 0.333 us
  forced-GPU-flush medians instead of rounding to zero.
- The full draw suite and the active-effect tower suite pass.

### 27. [ ] Add an end-to-end frame benchmark

The update benchmark reconstructs the runtime loops and omits lifecycle
construction, spawn work, HUD synchronization, audio, and some drone paths.
Keep it as a component benchmark, then add real `Game` scenarios for:

- A steady wave.
- Projectile and missile saturation.
- Mass deaths and shard construction.
- Mass escapes.
- Effect-budget saturation.
- Drone-heavy play.

### 28. [ ] Fix seeded explosion rendering

The explosion render script imports monster modules before replacing
`Math.random`. Singleton shard splitters therefore capture the original random
function, and their topology is not actually seeded. Inject a random source
or seed before module initialization; avoid global `Math.random` mutation.

### 29. [x] Consolidate browser render harnesses

Completed 2026-07-20. The tower, level, berserker, monster-explosion, and
polygon-shard renderers now use `scripts/benchmark-browser-harness.mjs` for
temporary Vite pages, ephemeral ports, Chromium/installed-Chrome fallback,
console and page-error handling, cleanup, result waiting, and PNG data-URL
writing. All five renderers generated non-empty artifacts, representative
outputs passed visual inspection, the focused draw benchmark passed through
the same harness, and local and Pages builds pass.

### 30. [ ] Expose diagnostics through package scripts

Add consistent scripts such as:

- `test`
- `check`
- `benchmark:draw`
- `render:explosions`
- `lint`
- `format:check`

Keep permanent scripts limited to reusable project checks rather than one-off
implementation validators.

### 31. [x] Declare the supported Node and npm versions

`package.json` now declares the Node and npm ranges required by Vite and the
Svelte plugin. `.nvmrc` pins Node 20.20.2 for local development, and CI reads
that same file instead of resolving an unspecified Node 20 release.

### 32. [x] Upgrade dependencies with current advisories

Updated Svelte, Vite, the Svelte Vite plugin, `svelte-check`, Playwright, and
their transitive dependencies within their current major versions. TypeScript
7 remains deferred as an unrelated major upgrade. The full `npm audit` now
reports zero vulnerabilities; local, Pages, Node 20, and browser verification
all pass.

## UI, accessibility, and maintainability

### 33. [ ] Give the battlefield an accessible interface

The foreground canvas has pointer handlers but no accessible label, role,
instructions, or keyboard placement mechanism. Add an accessible name and
instructions, then define keyboard placement if full non-pointer play is in
scope.

### 34. [~] Add dialog semantics and focus management

The campaign/game modal has dialog semantics, initial focus, Tab containment,
and focus restoration; browser checks cover desktop and mobile viewports.
Escape behavior for menu, pause, victory, and defeat still needs a decision.

### 35. [ ] Restore browser zoom

Remove `maximum-scale=1` and `user-scalable=no` from the viewport metadata.
Use CSS `touch-action` to control game gestures without blocking pinch zoom.

### 36. [ ] Support reduced motion

Honor `prefers-reduced-motion` for modal, star, banner, and decorative
animations without changing simulation timing.

### 37. [ ] Define tall-screen logical-field behavior

Tall layouts can expose logical space below the authored 960x560 route and
allow placement there. The current 375x812 layout is visually sound, so this
is a design decision: explicitly allow the extra build area or clip placement
to the canonical field, then test the chosen policy.

### 38. [ ] Split large files only at behavioral boundaries

Useful future extractions are wave/spawn orchestration, lifecycle resolution,
and input gesture state. Avoid splitting `game-engine.ts`, `game-session.ts`,
or `style.css` solely to reduce line count.

### 39. [x] Generate duplicated audio metadata

Completed 2026-07-20. `src/audio-manifest.ts` now owns each cue's ID,
soundboard label, imported asset URL, cooldown, gain, and rate variation. The
game audio loader reads URLs directly from the cue definitions, and the
soundboard generates its 31 audio elements and buttons from the same manifest.
Local and Pages builds pass; a live browser smoke confirmed 31 unique cues,
successful asset requests, and working click playback state.

### 40. [x] Update stale documentation

`README.md` now lists all six towers, route-specific `5` shortcuts, generated
waves, and the runtime check. `GAMEPLAY_NOTES.md` no longer states an outdated
tower count.

## Historical validation baseline

As of 2026-07-17:

- `npm run build`: passes with zero Svelte errors and warnings.
- `npm run build:pages`: passes.
- A one-off campaign validator confirmed derived desktop/mobile totals.
- A one-off damage validator passed across 15–144 Hz.
- A one-off damping validator preserved position and velocity across 15–144 Hz.
- A one-off timing validator confirmed native 120 Hz updates and bounded catch-up.
- All `scripts/*.mjs` pass Node syntax checking.
- Ten desktop/mobile routes pass identifier, coordinate, and numeric checks.
- Desktop campaign selection, placement, and pause smoke checks pass.
- Mobile 375x812 smoke check has no body overflow or console errors.
- Canvas backing stores correctly include display scale and device pixel ratio.
- The collision index remains the largest confirmed update-performance win.

## Recommended implementation order

1. Item 3: remove saturated-effect construction waste.
2. Item 6, then items 7–18: build coverage and harden correctness boundaries.
3. Items 19–22: measure and optimize remaining runtime hotspots.
4. Items 27–32: make benchmarks and CI enforce the intended behavior.
5. Items 33–40: accessibility, documentation, and structural cleanup.

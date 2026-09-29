import initGameEngine, * as gameEngine from "./generated/engine/engine.js";
import gameEngineUrl from "./generated/engine/engine_bg.wasm?url";

export type { BoardRenderer, WebGame } from "./generated/engine/engine.js";

type EngineModule = typeof gameEngine;

let loading: Promise<EngineModule> | undefined;

/**
 * Starts downloading and compiling the Rust engine (simulation plus WebGPU renderer) once;
 * index.html preloads the production binary, so this usually adopts the finished download.
 * The dev server loads the `labs` build instead, which adds the scripted staging API used by
 * the render and benchmark scripts (`window.__vectorDefence`).
 */
export function loadEngine(): Promise<EngineModule> {
  loading ??= import.meta.env.DEV
    ? (loadLabsEngine() as unknown as Promise<EngineModule>)
    : initGameEngine({ module_or_path: gameEngineUrl }).then(() => gameEngine);
  return loading;
}

type LabsEngineModule = typeof import("./generated/engine-labs/engine.js");

let loadingLabs: Promise<LabsEngineModule> | undefined;

/** The engine with the `labs` feature (dev server page, debug pages, scripts). */
export function loadLabsEngine(): Promise<LabsEngineModule> {
  loadingLabs ??= Promise.all([
    import("./generated/engine-labs/engine.js"),
    import("./generated/engine-labs/engine_bg.wasm?url"),
  ]).then(async ([engine, { default: url }]) => {
    await engine.default({ module_or_path: url });
    return engine;
  });
  return loadingLabs;
}

//! Browser entry point: the game, its WebGPU board renderer, and the camera controls, exposed
//! to the Svelte page shell (`src/game-session.ts`) through wasm-bindgen.
#[cfg(target_arch = "wasm32")]
mod game;
#[cfg(all(target_arch = "wasm32", feature = "labs"))]
mod labs;
#[cfg(target_arch = "wasm32")]
mod math;
#[cfg(target_arch = "wasm32")]
mod storage;

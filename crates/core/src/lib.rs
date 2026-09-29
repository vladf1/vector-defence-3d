//! Vector Defence simulation, shared by the Wasm build and native tests.
pub mod audio;
pub mod campaign;
pub mod collision;
pub mod combat_effects;
pub mod constants;
pub mod entities;
pub mod game;
pub mod json;
pub mod level_runtime;
pub mod monster_factory;
pub mod placement;
pub mod profile;
pub mod progress;
pub mod rng;
pub mod route_path;
pub mod simulation_timing;
pub mod small_map;
pub mod types;
pub mod update;
pub mod utils;
pub mod view;

#[cfg(test)]
mod tests;

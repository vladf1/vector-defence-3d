//! Raw-WebGPU board renderer (a port of the TypeScript `src/render3d/`), driven through
//! `web-sys` with no rendering library.
//!
//! Everything that does not touch WebGPU or the 2D canvas (math, geometry, models, WGSL
//! generation, batches, the camera rig, fx, the board scene, the entity views, the overlay
//! layout, and the frame composer) builds natively for tests; the GPU objects
//! (`gpu_batches`, `post_processing`, `renderer`, and the GPU halves of
//! `gpu_pipelines`/`board_scene`/`overlay`) are Wasm-only.
pub mod board_scene;
pub mod camera_rig;
pub mod effect_view;
pub mod entity_views;
pub mod frame_composer;
pub mod frame_math;
pub mod fx_system;
pub mod geometry_kit;
pub mod gpu_flags;
pub mod gpu_pipelines;
pub mod id_map;
pub mod instanced_batch;
pub mod math;
pub mod models;
pub mod monster_view;
pub mod overlay;
pub mod palette;
pub mod placement_view;
pub mod projectile_view;
pub mod puff_blobs;
pub mod render_batches;
pub mod render_quality;
pub mod scene_constants;
pub mod shaders;
pub mod sprite_batch;
pub mod startup_timings;
pub mod tower_view;

#[cfg(target_arch = "wasm32")]
pub mod gpu_batches;
#[cfg(target_arch = "wasm32")]
pub mod post_processing;
#[cfg(target_arch = "wasm32")]
pub mod renderer;

pub use camera_rig::{BOARD_TILT_RADIANS, CameraRig, InspectView, SurfaceRect, create_board_camera_rig};
pub use entity_views::{EntityViews, GameScene};
pub use frame_composer::{FrameComposer, FrameInput, SceneFrame, SceneViews};
#[cfg(target_arch = "wasm32")]
pub use renderer::BoardRenderer;
pub use startup_timings::StartupTimings;

#[cfg(test)]
mod scene_tests;

//! The CPU half of a board frame, independent of WebGPU so it runs in native tests: the
//! resolution governor, level-runtime switches, the frame context, refilling every batch
//! (board, entity views, fx), camera shake, and the frame uniform data.
use vd_core::profile::GameProfile;
use vd_core::route_path::SharedPath;

use crate::board_scene::BoardScene;
use crate::camera_rig::{CameraRig, create_board_camera_rig};
use crate::frame_math::FrameContext;
use crate::fx_system::{FxBudget, FxSystem, LightSink};
use crate::math::{Vec3, vec3};
use crate::render_batches::{BatchCapacities, RenderBatches};
use crate::render_quality::{RenderQuality, ResolutionGovernor, select_render_quality};
use crate::shaders::{MAX_POINT_LIGHTS, frame_layout};

const MAX_FRAME_DELTA_SECONDS: f64 = 0.1;

/// What the renderer reads from the simulation each frame (the web layer fills it from
/// `Game`; the renderer never mutates simulation state).
#[derive(Clone, Copy, Debug)]
pub struct FrameInput<'a> {
    /// `Game.needsAnimationFrame()`: only animating frames count toward the resolution governor.
    pub animating: bool,
    /// Identity of the game's current level runtime: any value that changes whenever `Game`
    /// replaces its `LevelRuntime` (a generation counter). A change resets the views and fx
    /// and applies `route` to the board, like the TypeScript renderer's runtime identity check.
    pub runtime_id: u64,
    /// The runtime's route path entries (`runtime.routePath.entries`).
    pub route: Option<&'a SharedPath>,
    /// `runtime.escapesLeft`; a drop flashes the exit portal.
    pub escapes_left: i32,
    /// `Game.simulationSeconds`: the presentation clock, so 3D effects freeze with the game.
    pub simulation_seconds: f64,
}

/// What entity views write into each frame.
pub struct SceneFrame<'a> {
    pub batches: &'a mut RenderBatches,
    pub fx: &'a mut FxSystem,
    pub frame: &'a FrameContext,
}

/// The entity views (towers, monsters, projectiles, effects, placement) plug in here. The
/// renderer calls `reset` when the level runtime changes and `write` once per frame, after
/// the board scene and before the fx system, inside the batch refill.
pub trait SceneViews {
    /// A new level runtime began: drop per-entity visual state.
    fn reset(&mut self) {}

    /// Writes towers, monsters, projectiles, effects, then placement, in that order.
    fn write(&mut self, _scene: &mut SceneFrame<'_>) {}
}

/// No entity views: just the board and fx (startup warm-up, render tests, phase 1).
impl SceneViews for () {}

/// Everything a frame needs on the CPU side.
pub struct FrameComposer {
    pub rig: CameraRig,
    pub quality: RenderQuality,
    pub governor: ResolutionGovernor,
    pub batches: RenderBatches,
    pub board: BoardScene,
    pub fx: FxSystem,
    view_direction: Vec3,
    frame_data: [f32; frame_layout::FLOATS],
    active_runtime: Option<u64>,
    last_simulation_seconds: f64,
    frame_index: u32,
}

impl FrameComposer {
    pub fn new(profile: &GameProfile, device_pixel_ratio: f64) -> Self {
        let quality = select_render_quality(profile);
        FrameComposer {
            rig: create_board_camera_rig(profile.field_width as f32, profile.field_height as f32),
            quality,
            governor: ResolutionGovernor::new(quality, device_pixel_ratio),
            batches: RenderBatches::new(BatchCapacities {
                glow_sprites: quality.glow_sprites,
                smoke_sprites: quality.smoke_sprites,
                ribbons: quality.ribbons,
            }),
            board: BoardScene::new(profile.field_width, profile.field_height, profile.road_width),
            fx: FxSystem::new(FxBudget {
                particles: quality.fx_particles,
                lights: MAX_POINT_LIGHTS.min(quality.flash_lights),
            }),
            view_direction: vec3(0.0, -1.0, 0.0),
            frame_data: [0.0; frame_layout::FLOATS],
            active_runtime: None,
            last_simulation_seconds: 0.0,
            frame_index: 0,
        }
    }

    /// Copies the logical camera's forward vector for camera-facing ribbons.
    pub fn sync_view_direction(&mut self) {
        self.view_direction = self.rig.logical_camera.forward;
    }

    /// Refills every batch for one frame. Returns true when the resolution governor changed
    /// the pixel ratio (the caller resizes the canvas and targets).
    pub fn compose(&mut self, now_ms: f64, input: &FrameInput<'_>, views: &mut dyn SceneViews) -> bool {
        let pixel_ratio_changed = self.governor.record_frame(now_ms, input.animating);

        if self.active_runtime != Some(input.runtime_id) {
            self.switch_runtime(input, views);
        }

        let time = input.simulation_seconds;
        let delta_seconds = (time - self.last_simulation_seconds).clamp(0.0, MAX_FRAME_DELTA_SECONDS) as f32;
        self.last_simulation_seconds = time;
        self.frame_index = self.frame_index.wrapping_add(1);
        let frame = FrameContext { delta_seconds, time, frame: self.frame_index, view_direction: self.view_direction };

        self.board.notify_escapes(input.escapes_left);
        self.batches.begin();
        self.board.write(&mut self.batches, &frame);
        views.write(&mut SceneFrame { batches: &mut self.batches, fx: &mut self.fx, frame: &frame });
        self.fx.update(delta_seconds);
        self.fx.write(&mut self.batches);

        self.rig.add_trauma(self.fx.take_trauma());
        self.rig.update(delta_seconds);
        pixel_ratio_changed
    }

    fn switch_runtime(&mut self, input: &FrameInput<'_>, views: &mut dyn SceneViews) {
        self.active_runtime = Some(input.runtime_id);
        self.last_simulation_seconds = input.simulation_seconds;
        views.reset();
        self.fx.clear();
        self.board.set_route(input.route);
    }

    /// The Frame uniform block for the render camera (camera matrices, camera position and
    /// presentation time, and the flash lights).
    pub fn frame_uniforms(&mut self) -> &[f32] {
        let data = &mut self.frame_data;
        let camera = &self.rig.render_camera;
        data[frame_layout::VIEW_PROJECTION..frame_layout::VIEW_PROJECTION + 16]
            .copy_from_slice(&camera.view_projection);
        data[frame_layout::VIEW..frame_layout::VIEW + 16].copy_from_slice(&camera.view);
        data[frame_layout::PROJECTION..frame_layout::PROJECTION + 16].copy_from_slice(&camera.projection);
        let position = camera.position;
        let time = self.last_simulation_seconds as f32;
        data[frame_layout::CAMERA..frame_layout::CAMERA + 4]
            .copy_from_slice(&[position.x, position.y, position.z, time]);
        self.fx.write_lights(LightSink {
            data,
            position_offset: frame_layout::POINT_POSITION,
            color_offset: frame_layout::POINT_COLOR,
            slots: MAX_POINT_LIGHTS,
        });
        &self.frame_data
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render_batches::Batch;
    use vd_core::profile::GameMode;
    use vd_core::route_path::create_route_motion_path;
    use vd_core::types::Point;

    struct CountingViews {
        resets: u32,
        writes: u32,
    }

    impl SceneViews for CountingViews {
        fn reset(&mut self) {
            self.resets += 1;
        }

        fn write(&mut self, scene: &mut SceneFrame<'_>) {
            self.writes += 1;
            scene.batches[Batch::TowerBase].push_yaw(10.0, 0.0, 10.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0);
            if scene.frame.frame == 2 {
                scene.fx.explosion(100.0, 100.0, 1.0);
            }
        }
    }

    #[test]
    fn composes_board_views_and_fx_each_frame() {
        let profile = GameProfile::for_mode(GameMode::Desktop);
        let mut composer = FrameComposer::new(&profile, 2.0);
        composer.rig.resize(1280.0, 720.0);
        composer.sync_view_direction();
        let route = create_route_motion_path(&[Point::new(0.0, 200.0), Point::new(800.0, 200.0)], 24.0, 7.0);
        let mut views = CountingViews { resets: 0, writes: 0 };
        let mut input = FrameInput {
            animating: true,
            runtime_id: 1,
            route: Some(&route.entries),
            escapes_left: 5,
            simulation_seconds: 3.0,
        };
        for frame in 0..3 {
            input.simulation_seconds = 3.0 + frame as f64 / 60.0;
            composer.compose(1000.0 + frame as f64 * 16.0, &input, &mut views);
        }
        assert_eq!((views.resets, views.writes), (1, 3));
        assert_eq!(composer.batches[Batch::TowerBase].size(), 1);
        assert!(composer.batches[Batch::RoadChevron].size() > 0);
        assert!(composer.fx.active_particles() > 0);
        assert_ne!(composer.rig.render_camera.position, composer.rig.logical_camera.position, "explosion shakes");

        let uniforms = composer.frame_uniforms();
        assert_eq!(uniforms.len(), frame_layout::FLOATS);
        assert!((uniforms[frame_layout::CAMERA + 3] - (3.0 + 2.0 / 60.0)).abs() < 1e-5);
        assert!(uniforms.iter().all(|value| value.is_finite()));

        input.runtime_id = 2;
        input.route = None;
        composer.compose(1100.0, &input, &mut views);
        assert_eq!(views.resets, 2);
        assert_eq!(composer.fx.active_particles(), 0);
        assert_eq!(composer.batches[Batch::RoadChevron].size(), 0);
    }
}

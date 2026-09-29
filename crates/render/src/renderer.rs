//! Raw WebGPU board renderer. It never mutates the simulation: every frame it reads a
//! `FrameInput` plus the entity views, refills instanced batches, and records one command
//! buffer (scene pass, bloom chain, composite). All pipelines are created up front,
//! asynchronously and in parallel.
use std::cell::Cell;
use std::rc::Rc;

use vd_core::game::Game;
use vd_core::profile::GameProfile;
use vd_core::route_path::create_route_motion_path;
use vd_core::types::{FieldBounds, Point};
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{
    GpuBindGroup, GpuBindGroupDescriptor, GpuBindGroupEntry, GpuBuffer, GpuBufferDescriptor, GpuCanvasAlphaMode,
    GpuCanvasConfiguration, GpuCanvasContext, GpuColorDict, GpuDevice, GpuQueue, HtmlCanvasElement,
};

use crate::board_scene::GpuRoad;
use crate::camera_rig::{InspectView, SurfaceRect};
use crate::entity_views::{EntityViews, GameScene};
use crate::frame_composer::{FrameComposer, FrameInput, SceneViews};
use crate::gpu_batches::GpuBatches;
use crate::gpu_flags::{buffer_usage, f32_bytes};
use crate::gpu_pipelines::{GpuPipelines, PIPELINE_COUNT, start_pipelines};
use crate::overlay::{OverlayLayout, OverlaySurface};
use crate::palette::linear_color;
use crate::post_processing::PostProcessing;
use crate::render_batches::build_batch_meshes;
use crate::scene_constants::{BACKGROUND, create_scene_constants};
use crate::shaders::{create_shader_sources, frame_layout};
pub use crate::startup_timings::StartupTimings;

fn now() -> f64 {
    web_sys::window().and_then(|window| window.performance()).map_or(0.0, |performance| performance.now())
}

fn device_pixel_ratio() -> f64 {
    let ratio = web_sys::window().map_or(1.0, |window| window.device_pixel_ratio());
    if ratio > 0.0 { ratio } else { 1.0 }
}

/// Flags shared with the device-lost watcher.
#[derive(Default)]
struct Lifecycle {
    ready: Cell<bool>,
    disposed: Cell<bool>,
}

/// The WebGPU board renderer (the TypeScript `WebGpuBoardRenderer`).
pub struct BoardRenderer {
    composer: FrameComposer,
    device: GpuDevice,
    queue: GpuQueue,
    context: GpuCanvasContext,
    canvas: HtmlCanvasElement,
    overlay: OverlaySurface,
    overlay_layout: OverlayLayout,
    views: EntityViews,
    pipelines: GpuPipelines,
    frame_buffer: GpuBuffer,
    frame_bind_group: GpuBindGroup,
    batches: GpuBatches,
    road: GpuRoad,
    post: PostProcessing,
    clear_color: GpuColorDict,
    lifecycle: Rc<Lifecycle>,
    startup_timings: StartupTimings,
    frame_draw_calls: u32,
    width: f64,
    height: f64,
}

impl BoardRenderer {
    /// Builds the renderer on a device the page already requested (`requestAdapter()` /
    /// `requestDevice()` start during HTML parsing). Pipelines compile in the GPU process while
    /// the CPU builds geometry; a warm-up frame touching every batch is drained before this
    /// resolves. `on_device_lost` runs (once, unless disposed first) when the device is lost,
    /// so the web layer can remount a fresh renderer. On failure the device is destroyed, as
    /// the TypeScript renderer disposed itself.
    pub async fn create(
        device: GpuDevice,
        canvas: HtmlCanvasElement,
        overlay: HtmlCanvasElement,
        profile: &GameProfile,
        shader_salt: u32,
        on_device_lost: Option<Box<dyn FnOnce()>>,
    ) -> Result<BoardRenderer, JsValue> {
        let lifecycle = Rc::new(Lifecycle::default());
        let result =
            Self::initialize(device.clone(), canvas, overlay, profile, shader_salt, on_device_lost, lifecycle.clone())
                .await;
        if result.is_err() {
            lifecycle.disposed.set(true);
            device.destroy();
        }
        result
    }

    async fn initialize(
        device: GpuDevice,
        canvas: HtmlCanvasElement,
        overlay: HtmlCanvasElement,
        profile: &GameProfile,
        shader_salt: u32,
        on_device_lost: Option<Box<dyn FnOnce()>>,
        lifecycle: Rc<Lifecycle>,
    ) -> Result<BoardRenderer, JsValue> {
        let started_at = now();
        let lost = device.lost();
        let watcher = lifecycle.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let _ = lost.await;
            if !watcher.disposed.get() {
                watcher.ready.set(false);
                if let Some(callback) = on_device_lost {
                    callback();
                }
            }
        });

        let context: GpuCanvasContext = canvas
            .get_context("webgpu")?
            .ok_or_else(|| JsValue::from_str("Unable to create a WebGPU canvas context."))?
            .unchecked_into();
        let canvas_format = web_sys::window()
            .ok_or_else(|| JsValue::from_str("no window"))?
            .navigator()
            .gpu()
            .get_preferred_canvas_format();
        let configuration = GpuCanvasConfiguration::new(&device, canvas_format);
        configuration.set_alpha_mode(GpuCanvasAlphaMode::Opaque);
        context.configure(&configuration)?;
        let device_ready_at = now();

        // Pipelines compile in the GPU process while the CPU builds geometry below.
        let mut composer = FrameComposer::new(profile, device_pixel_ratio());
        let quality = composer.quality;
        let sources = create_shader_sources(&create_scene_constants(
            profile.field_width,
            profile.field_height,
            profile.road_width,
        ));
        let pending = start_pipelines(&device, canvas_format, quality.msaa_samples, shader_salt, &sources)?;

        let geometry_start = now();
        let meshes = build_batch_meshes(profile.road_width as f32);
        let geometry_ms = now() - geometry_start;
        let queue = device.queue();
        let frame_bytes = (frame_layout::FLOATS * 4) as u32;
        let descriptor = GpuBufferDescriptor::new(frame_bytes, buffer_usage::UNIFORM | buffer_usage::COPY_DST);
        descriptor.set_label("frame");
        let frame_buffer = device.create_buffer(&descriptor)?;
        let batches = GpuBatches::new(&device, &meshes, &composer.batches)?;
        let road = GpuRoad::new(&device)?;
        let overlay = OverlaySurface::new(overlay)?;
        let setup_done_at = now();

        let pipelines = pending.finish().await?;
        let pipelines_ready_at = now();
        let entries = [GpuBindGroupEntry::new_with_gpu_buffer(0, &frame_buffer)];
        let descriptor = GpuBindGroupDescriptor::new(&entries, &pipelines.frame_layout);
        descriptor.set_label("frame");
        let frame_bind_group = device.create_bind_group(&descriptor);
        let post = PostProcessing::new(&device, &pipelines, &context, quality.msaa_samples, quality.bloom_resolution)?;
        let background = linear_color(BACKGROUND);
        let clear_color = GpuColorDict::new(1.0, background.b as f64, background.g as f64, background.r as f64);

        // Warm-up: one frame with every batch, the road, and the post chain, then a GPU drain.
        composer.batches.prepare_warmup();
        let warmup_route =
            create_route_motion_path(&[Point::new(-400.0, -400.0), Point::new(-300.0, -400.0)], 24.0, 7.0);
        composer.board.set_route(Some(&warmup_route.entries));
        let mut renderer = BoardRenderer {
            composer,
            device,
            queue,
            context,
            canvas,
            overlay,
            overlay_layout: OverlayLayout::new(profile.field_width, profile.draw_canvas_tower_actions),
            views: EntityViews::default(),
            pipelines,
            frame_buffer,
            frame_bind_group,
            batches,
            road,
            post,
            clear_color,
            lifecycle,
            startup_timings: StartupTimings::default(),
            frame_draw_calls: 0,
            width: 1.0,
            height: 1.0,
        };
        renderer.resize();
        renderer.batches.upload(&renderer.queue, &renderer.composer.batches);
        renderer.render_frame()?;
        let warmup_rendered_at = now();
        renderer.queue.on_submitted_work_done().await?;
        let drained_at = now();
        renderer.composer.board.set_route(None);
        renderer.composer.batches.begin();
        renderer.batches.upload(&renderer.queue, &renderer.composer.batches);

        renderer.startup_timings = StartupTimings {
            device_ms: device_ready_at - started_at,
            geometry_ms,
            setup_ms: setup_done_at - device_ready_at,
            pipeline_ms: pipelines_ready_at - setup_done_at,
            warmup_frame_ms: warmup_rendered_at - pipelines_ready_at,
            gpu_drain_ms: drained_at - warmup_rendered_at,
            total_ms: drained_at - started_at,
            pipelines: PIPELINE_COUNT,
        };
        renderer.lifecycle.ready.set(!renderer.lifecycle.disposed.get());
        Ok(renderer)
    }

    /// Matches the canvas, targets, camera, and overlay to the canvas's CSS box.
    pub fn resize(&mut self) {
        let rect = self.canvas.get_bounding_client_rect();
        self.width = rect.width().round().max(1.0);
        self.height = rect.height().round().max(1.0);
        let pixel_ratio = device_pixel_ratio();
        self.composer.governor.update_viewport(self.width, self.height, pixel_ratio);
        self.apply_pixel_ratio();
        self.composer.rig.resize(self.width as f32, self.height as f32);
        self.composer.sync_view_direction();
        self.overlay.resize(&mut self.overlay_layout, self.width, self.height, pixel_ratio);
    }

    /// Draws one frame of `game`: governor, runtime switch, batch refill (board, entity views,
    /// fx), camera shake, the GPU frame, then the 2D overlay.
    pub fn draw(&mut self, game: &Game) {
        if !self.lifecycle.ready.get() {
            return;
        }
        let input = FrameInput {
            animating: game.needs_animation_frame(),
            runtime_id: game.runtime_generation,
            route: game.runtime.route_path.as_ref().map(|route| &route.entries),
            escapes_left: game.runtime.escapes_left,
            simulation_seconds: game.simulation_seconds,
        };
        // The views are taken out for the frame so the scene can borrow them beside the renderer.
        let mut views = std::mem::take(&mut self.views);
        let bounds = self.composer.rig.field_bounds();
        self.draw_frame(&input, &mut GameScene { game, views: &mut views, bounds });
        self.views = views;
        self.overlay.draw(game, &mut self.overlay_layout, &self.composer.rig);
    }

    /// The lower-level frame: `views` write the entities (no overlay). `draw` uses this.
    pub fn draw_frame(&mut self, input: &FrameInput<'_>, views: &mut dyn SceneViews) {
        if !self.lifecycle.ready.get() {
            return;
        }
        if self.composer.compose(now(), input, views) {
            self.apply_pixel_ratio();
        }
        self.batches.upload(&self.queue, &self.composer.batches);
        let _ = self.render_frame();
    }

    /// Whether a field point (as picked by `client_to_field`) is on the selected tower's
    /// upgrade button, as drawn in the last frame.
    pub fn is_point_in_upgrade_button(&self, point: Point) -> bool {
        self.overlay_layout.is_point_in_upgrade_button(&self.composer.rig, point)
    }

    /// Whether a field point is on the selected laser tower's direction-lock button.
    pub fn is_point_in_laser_lock_button(&self, point: Point) -> bool {
        self.overlay_layout.is_point_in_laser_lock_button(&self.composer.rig, point)
    }

    /*
     * Player camera controls. Picking follows them; visible field bounds (gameplay) never do.
     * Each returns whether the view changed.
     */

    /// Tilts toward the horizon (positive) or straight down (negative), keeping the field framed.
    pub fn tilt_by(&mut self, delta_radians: f64) -> bool {
        if !self.composer.rig.tilt_by(delta_radians as f32) {
            return false;
        }
        self.composer.sync_view_direction();
        true
    }

    /// Zooms by `factor` (above 1 zooms in), keeping the ground under the client point in place.
    pub fn zoom_at(&mut self, factor: f64, client_x: f64, client_y: f64, rect: &SurfaceRect) -> bool {
        self.composer.rig.zoom_at(factor as f32, client_x, client_y, rect)
    }

    /// Pans so the ground under the `from` client point moves under the `to` client point.
    pub fn pan_between(&mut self, from_x: f64, from_y: f64, to_x: f64, to_y: f64, rect: &SurfaceRect) -> bool {
        self.composer.rig.pan_between(from_x, from_y, to_x, to_y, rect)
    }

    /// Restores the default tilt, zoom, and pan.
    pub fn reset_view(&mut self) -> bool {
        if !self.composer.rig.reset_view() {
            return false;
        }
        self.composer.sync_view_direction();
        true
    }

    /// Frames the render camera over a field point (no shake); `None` restores the board view.
    pub fn inspect(&mut self, view: Option<InspectView>) {
        self.composer.rig.inspect(view);
    }

    /// Shows or hides the level's road, portal, spawn gate, and motes (the ground stays).
    pub fn set_scenery_visible(&mut self, visible: bool) {
        self.composer.board.set_scenery_visible(visible);
    }

    pub fn visible_field_bounds(&self) -> FieldBounds {
        self.composer.rig.field_bounds()
    }

    /// Maps a client-space pointer position over the input surface to field coordinates.
    pub fn client_to_field(&self, client_x: f64, client_y: f64, rect: &SurfaceRect) -> Option<Point> {
        self.composer.rig.client_to_field(client_x, client_y, rect)
    }

    /// Projects a world point (field x, height, field y) into CSS pixels relative to the canvas,
    /// through the logical (unshaken) camera, as the overlay needs.
    pub fn project_to_viewport(&self, x: f64, height: f64, y: f64) -> Point {
        self.composer.rig.project_to_viewport(x as f32, height as f32, y as f32)
    }

    pub fn startup_timings(&self) -> StartupTimings {
        self.startup_timings
    }

    pub fn pixel_ratio(&self) -> f64 {
        self.composer.governor.current_pixel_ratio()
    }

    /// Draw calls recorded by the most recent frame (scene, bloom, and composite).
    pub fn frame_draw_calls(&self) -> u32 {
        self.frame_draw_calls
    }

    pub fn drawn_instances(&self) -> usize {
        self.composer.batches.drawn_instances()
    }

    /// The CPU frame state (camera rig, batches, board, fx) for the entity views and overlay.
    pub fn composer(&self) -> &FrameComposer {
        &self.composer
    }

    pub fn is_ready(&self) -> bool {
        self.lifecycle.ready.get()
    }

    pub fn dispose(&mut self) {
        if self.lifecycle.disposed.get() {
            return;
        }
        self.lifecycle.disposed.set(true);
        self.lifecycle.ready.set(false);
        self.batches.dispose();
        self.road.dispose();
        self.post.dispose();
        self.frame_buffer.destroy();
        self.context.unconfigure();
        self.device.destroy();
    }

    fn apply_pixel_ratio(&mut self) {
        let pixel_ratio = self.composer.governor.current_pixel_ratio();
        let width = ((self.width * pixel_ratio).floor() as u32).max(1);
        let height = ((self.height * pixel_ratio).floor() as u32).max(1);
        self.canvas.set_width(width);
        self.canvas.set_height(height);
        let _ = self.post.resize(width, height, &self.clear_color);
    }

    fn render_frame(&mut self) -> Result<(), JsValue> {
        let uniforms = self.composer.frame_uniforms();
        self.queue.write_buffer_with_u32_and_u8_slice(&self.frame_buffer, 0, f32_bytes(uniforms))?;
        self.road.sync(&self.queue, &mut self.composer.board);

        let encoder = self.device.create_command_encoder();
        let (batches, pipelines, board, road) = (&self.batches, &self.pipelines, &self.composer.board, &self.road);
        let cpu_batches = &self.composer.batches;
        let frame_bind_group = &self.frame_bind_group;
        self.post.render(&encoder, pipelines, |pass| {
            pass.set_bind_group(0, Some(frame_bind_group));
            batches.draw_opaque(pass, pipelines, cpu_batches);
            road.draw(pass, pipelines, board);
            batches.draw_overlays(pass, pipelines, cpu_batches);
        })?;
        self.queue.submit(&[encoder.finish()]);
        self.frame_draw_calls =
            (self.composer.batches.draw_calls() as u32) + self.composer.board.draw_calls() + PostProcessing::DRAW_CALLS;
        Ok(())
    }
}

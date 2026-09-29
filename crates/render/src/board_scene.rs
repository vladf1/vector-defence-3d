//! Static battlefield: ground, the route's road, road chevrons, the exit portal, the spawn
//! gate, and ambient motes.
use vd_core::route_path::{PathEntry, SharedPath, get_path_heading_angle};

use crate::frame_math::{FrameContext, hash01};
use crate::palette::{LinearColor, linear_color};
use crate::render_batches::{Batch, RenderBatches};
use crate::shaders::{CHEVRON_ARM_SLOPE, CHEVRON_BAND, CHEVRON_SPAN};

const ROAD_BORDER: f64 = 1.5;
const ROAD_BASE_Y: f64 = 0.22;
// Later road samples sit microscopically higher so self-crossing routes never z-fight.
const ROAD_LAYER_STEP: f64 = 0.00035;
const ROAD_LEAD_IN: f64 = 90.0;
const EXIT_PORTAL_RADIUS: f32 = 19.0;
const PORTAL_COLOR: u32 = 0xb0ffe1;
const PORTAL_ALERT: u32 = 0xff6f62;
// Kept well below the monsters and towers so the exit reads as a landmark, not a light.
const PORTAL_INTENSITY: f32 = 0.55;
const PORTAL_RISING_RINGS: u32 = 2;
const GATE_COLOR: u32 = 0xff8f6a;
const MOTE_COLOR: u32 = 0x7dffd4;
const MOTE_COUNT: u32 = 70;
// Road chevrons drift toward the exit; each sits just above the road sample it lies on.
const CHEVRON_SPACING: f64 = 30.0;
const CHEVRON_SPEED: f64 = 22.0;
const CHEVRON_LIFT: f64 = 0.03;
const CHEVRON_END_MARGIN: f64 = 6.0;
const CHEVRON_COLOR: LinearColor = LinearColor::new(0.03 * 0.22, 0.2 * 0.22, 0.15 * 0.22);

// Longest authored route samples to ~1k entries; headroom keeps one buffer for all levels.
pub const ROAD_CAPACITY_ENTRIES: usize = 4096;

pub const ROAD_VERTEX_FLOATS: usize = 5;
pub const GROUND_VERTICES: u32 = 6;

/// Samples the road centerline at ascending distances without rescanning: the lead-in before
/// the first entry extends the first segment backward, as the road ribbon does.
struct RoadCursor<'a> {
    x: f64,
    y: f64,
    /// The road ribbon slot under the sample, which sets its layer height.
    slot: usize,
    /// The entry ending the segment under the sample (a search hint for path lookups).
    index: usize,
    entries: &'a [PathEntry],
    count: usize,
}

impl<'a> RoadCursor<'a> {
    fn new(entries: &'a [PathEntry], count: usize) -> Self {
        RoadCursor { x: 0.0, y: 0.0, slot: 0, index: 1, entries, count }
    }

    fn seek(&mut self, distance: f64) {
        let entries = self.entries;
        let first = &entries[0];
        if distance < first.total_distance {
            self.interpolate(first, &entries[1], distance);
            self.slot = 0;
            return;
        }
        while self.index < self.count - 1 && entries[self.index].total_distance < distance {
            self.index += 1;
        }
        let end = &entries[self.index];
        self.interpolate(&entries[self.index - 1], end, distance.min(end.total_distance));
        self.slot = self.index + 1;
    }

    fn interpolate(&mut self, start: &PathEntry, stop: &PathEntry, distance: f64) {
        let span = stop.total_distance - start.total_distance;
        let ratio = if span > 0.0 { (distance - start.total_distance) / span } else { 0.0 };
        self.x = start.x + (stop.x - start.x) * ratio;
        self.y = start.y + (stop.y - start.y) * ratio;
    }
}

/// One persistent road ribbon (uv.x = distance, uv.y = -1..1 across) rewritten in place per
/// level: interleaved `position(3) uv(2)` pairs joined by a fixed index buffer
/// (`road_indices`). The GPU side uploads `pending_upload` when the route changes.
struct RoadRibbon {
    vertices: Vec<f32>,
    used_floats: usize,
    index_count: u32,
    dirty: bool,
}

/// The fixed road index buffer: two triangles per ribbon slot.
pub fn road_indices() -> Vec<u32> {
    let mut indices = Vec::with_capacity(ROAD_CAPACITY_ENTRIES * 6);
    for slot in 0..ROAD_CAPACITY_ENTRIES as u32 {
        let a = slot * 2;
        indices.extend([a, a + 2, a + 1, a + 1, a + 2, a + 3]);
    }
    indices
}

impl RoadRibbon {
    fn new() -> Self {
        RoadRibbon {
            vertices: vec![0.0; (ROAD_CAPACITY_ENTRIES + 1) * 2 * ROAD_VERTEX_FLOATS],
            used_floats: 0,
            index_count: 0,
            dirty: false,
        }
    }

    fn write(&mut self, entries: &[PathEntry], width: f64) {
        let count = entries.len().min(ROAD_CAPACITY_ENTRIES);
        let half_width = width / 2.0;
        let tangent_at = |index: usize| {
            let previous = &entries[index.saturating_sub(1)];
            let next = &entries[(index + 1).min(count - 1)];
            let dx = next.x - previous.x;
            let dy = next.y - previous.y;
            let length = dx.hypot(dy);
            let length = if length > 0.0 { length } else { 1.0 };
            (dx / length, dy / length)
        };
        let vertices = &mut self.vertices;
        let mut write_pair = |slot: usize, x: f64, y: f64, (tangent_x, tangent_y): (f64, f64), distance: f64| {
            let height = (ROAD_BASE_Y + slot as f64 * ROAD_LAYER_STEP) as f32;
            let offset = slot * ROAD_VERTEX_FLOATS * 2;
            vertices[offset..offset + 10].copy_from_slice(&[
                (x - tangent_y * half_width) as f32,
                height,
                (y + tangent_x * half_width) as f32,
                distance as f32,
                1.0,
                (x + tangent_y * half_width) as f32,
                height,
                (y - tangent_x * half_width) as f32,
                distance as f32,
                -1.0,
            ]);
        };

        let start_tangent = tangent_at(0);
        let first = &entries[0];
        let lead_x = first.x - start_tangent.0 * ROAD_LEAD_IN;
        let lead_y = first.y - start_tangent.1 * ROAD_LEAD_IN;
        write_pair(0, lead_x, lead_y, start_tangent, -ROAD_LEAD_IN);
        for (index, entry) in entries.iter().take(count).enumerate() {
            write_pair(index + 1, entry.x, entry.y, tangent_at(index), entry.total_distance);
        }
        self.used_floats = (count + 1) * 2 * ROAD_VERTEX_FLOATS;
        self.index_count = count as u32 * 6;
        self.dirty = true;
    }

    fn clear(&mut self) {
        self.index_count = 0;
    }
}

/// Static battlefield: ground, the route's road, the exit portal, and the spawn gate.
pub struct BoardScene {
    field_width: f64,
    field_height: f64,
    road_width: f64,
    road: RoadRibbon,
    route: Option<SharedPath>,
    exit_alert: f32,
    last_escapes_left: i32,
    scenery_visible: bool,
    portal_color: LinearColor,
    portal_alert: LinearColor,
    gate_color: LinearColor,
    mote_color: LinearColor,
}

impl BoardScene {
    pub fn new(field_width: f64, field_height: f64, road_width: f64) -> Self {
        BoardScene {
            field_width,
            field_height,
            road_width,
            road: RoadRibbon::new(),
            route: None,
            exit_alert: 0.0,
            last_escapes_left: -1,
            scenery_visible: true,
            portal_color: linear_color(PORTAL_COLOR),
            portal_alert: linear_color(PORTAL_ALERT),
            gate_color: linear_color(GATE_COLOR),
            mote_color: linear_color(MOTE_COLOR),
        }
    }

    /// The route's path entries (identity decides whether the road is rebuilt).
    pub fn route_path(&self) -> Option<&SharedPath> {
        self.route.as_ref()
    }

    pub fn set_route(&mut self, route: Option<&SharedPath>) {
        let same = match (&self.route, route) {
            (Some(current), Some(next)) => SharedPath::ptr_eq(current, next),
            (None, None) => true,
            _ => false,
        };
        if same {
            return;
        }
        self.route = route.cloned();
        self.last_escapes_left = -1;
        match route {
            Some(entries) if entries.len() >= 2 => self.road.write(entries, self.road_width + ROAD_BORDER * 2.0),
            _ => self.road.clear(),
        }
    }

    pub fn notify_escapes(&mut self, escapes_left: i32) {
        if self.last_escapes_left >= 0 && escapes_left < self.last_escapes_left {
            self.exit_alert = 1.0;
        }
        self.last_escapes_left = escapes_left;
    }

    /// Road vertices written since the last call (for the GPU road buffer), if any.
    pub fn take_road_upload(&mut self) -> Option<&[f32]> {
        if !self.road.dirty {
            return None;
        }
        self.road.dirty = false;
        Some(&self.road.vertices[..self.road.used_floats])
    }

    /// Indices the road draw uses (0 when there is no road).
    pub fn road_index_count(&self) -> u32 {
        self.road.index_count
    }

    /// Road chevrons as rigid quads, so turns cannot bend them. Each is centered on the road and
    /// takes the route's analytic heading at its center, the same smooth heading monsters steer
    /// by, so it turns through a curve instead of snapping between path segments.
    fn write_chevrons(&self, entries: &[PathEntry], batches: &mut RenderBatches, frame: &FrameContext) {
        let count = entries.len().min(ROAD_CAPACITY_ENTRIES);
        let half_width = self.road_width / 2.0 + ROAD_BORDER;
        let back = half_width * CHEVRON_SPAN as f64 * CHEVRON_ARM_SLOPE as f64;
        let length = CHEVRON_BAND as f64 + back;
        let width = (half_width * CHEVRON_SPAN as f64 * 2.0) as f32;
        // The quad's center sits this far ahead of the chevron's apex (see the effect shader).
        let center_ahead = length / 2.0 - back;
        let end = entries[count - 1].total_distance - CHEVRON_END_MARGIN;
        let offset = (frame.time * CHEVRON_SPEED) % CHEVRON_SPACING;
        let first_distance = entries[0].total_distance;
        let lead_heading = get_path_heading_angle(entries, first_distance, 1);
        let mut cursor = RoadCursor::new(entries, count);
        let chevrons = &mut batches[Batch::RoadChevron];
        let mut apex = offset - ((offset + ROAD_LEAD_IN) / CHEVRON_SPACING).floor() * CHEVRON_SPACING;
        while apex <= end {
            let center = apex + center_ahead;
            cursor.seek(center);
            let heading = if center < first_distance {
                lead_heading
            } else {
                get_path_heading_angle(entries, center, cursor.index)
            };
            chevrons.push_yaw(
                cursor.x as f32,
                (ROAD_BASE_Y + cursor.slot as f64 * ROAD_LAYER_STEP + CHEVRON_LIFT) as f32,
                cursor.y as f32,
                -heading as f32,
                length as f32,
                1.0,
                width,
                CHEVRON_COLOR.r,
                CHEVRON_COLOR.g,
                CHEVRON_COLOR.b,
            );
            apex += CHEVRON_SPACING;
        }
    }

    /// Stateless drifting energy motes: positions are pure functions of time and index.
    fn write_motes(&self, batches: &mut RenderBatches, frame: &FrameContext) {
        let (field_width, field_height) = (self.field_width, self.field_height);
        let time = frame.time;
        let color = self.mote_color;
        for index in 0..MOTE_COUNT {
            let i = index as f64;
            let seed_x = hash01(i * 3.1) as f64;
            let seed_y = hash01(i * 7.7 + 1.0) as f64;
            let speed = 3.0 + hash01(i * 5.3) as f64 * 6.0;
            let x = (seed_x * field_width + time * speed) % (field_width + 80.0) - 40.0;
            let y = seed_y * field_height + (time * 0.4 + i * 1.7).sin() * 14.0;
            let height = 12.0 + hash01(i * 2.3) as f64 * 40.0 + (time * 0.7 + i).sin() * 5.0;
            let twinkle = 0.35 + (time * (1.3 + seed_x) + i * 2.1).sin() * 0.35;
            let size = (1.6 + hash01(i * 9.1) as f64 * 2.2) as f32;
            let alpha = (twinkle.max(0.0) * 0.55) as f32;
            batches.glow.push(
                x as f32,
                height as f32,
                y as f32,
                0.0,
                size,
                size,
                0.0,
                color.r,
                color.g,
                color.b,
                alpha,
            );
        }
    }

    /// Hides the road, exit portal, spawn gate, and ambient motes (the ground stays).
    pub fn set_scenery_visible(&mut self, visible: bool) {
        self.scenery_visible = visible;
    }

    pub fn scenery_visible(&self) -> bool {
        self.scenery_visible
    }

    pub fn write(&mut self, batches: &mut RenderBatches, frame: &FrameContext) {
        if !self.scenery_visible {
            return;
        }
        self.write_motes(batches, frame);
        let Some(route) = self.route.clone() else {
            return;
        };
        let entries: &[PathEntry] = &route;
        if entries.len() < 2 {
            return;
        }
        self.write_chevrons(entries, batches, frame);
        self.exit_alert = (self.exit_alert - frame.delta_seconds * 1.6).max(0.0);
        let time = frame.time;
        let exit = &entries[entries.len() - 1];
        let (exit_x, exit_z) = (exit.x as f32, exit.y as f32);
        let pulse = (0.92 + ((time * 3.1).sin() * 0.08) as f32) * PORTAL_INTENSITY;
        let alert = self.exit_alert;
        let (portal, warning) = (self.portal_color, self.portal_alert);
        let red = (portal.r + (warning.r - portal.r) * alert) * pulse * (1.0 + alert);
        let green = (portal.g + (warning.g - portal.g) * alert) * pulse * (1.0 + alert);
        let blue = (portal.b + (warning.b - portal.b) * alert) * pulse * (1.0 + alert);
        let spin = (time * 0.35) as f32;
        let radius = EXIT_PORTAL_RADIUS;
        batches[Batch::Portal].push_yaw(exit_x, 0.3, exit_z, spin, radius, radius, radius, red, green, blue);
        let (glow_r, glow_g, glow_b) = (red * 0.18, green * 0.18, blue * 0.18);
        batches.push_ground_glow(exit_x, 0.9, exit_z, radius * 1.5, 0.0, glow_r, glow_g, glow_b, 1.0);
        for index in 0..PORTAL_RISING_RINGS {
            let phase = ((time * 0.4 + index as f64 / PORTAL_RISING_RINGS as f64) % 1.0) as f32;
            let size = radius * (1.0 - phase) * 1.4;
            let alpha = phase * (1.0 - phase) * 1.6;
            let y = 1.0 + phase * 10.0;
            batches.glow.push(exit_x, y, exit_z, 0.0, size, size, 1.0, red * 0.5, green * 0.5, blue * 0.5, alpha);
        }

        let start = &entries[0];
        let next = &entries[(entries.len() - 1).min(3)];
        let angle = (next.y - start.y).atan2(next.x - start.x) as f32;
        let gate_glow = 0.8 + ((time * 4.2).sin() * 0.2) as f32;
        let gate = self.gate_color.scaled(gate_glow);
        let (start_x, start_z) = (start.x as f32, start.y as f32);
        batches[Batch::SpawnGate].push_yaw(start_x, 0.0, start_z, -angle, 1.0, 1.0, 1.0, gate.r, gate.g, gate.b);
    }

    pub fn draw_calls(&self) -> u32 {
        if self.scenery_visible && self.route.as_ref().is_some_and(|route| route.len() >= 2) { 2 } else { 1 }
    }
}

#[cfg(target_arch = "wasm32")]
pub use gpu::GpuRoad;

#[cfg(target_arch = "wasm32")]
mod gpu {
    use wasm_bindgen::JsValue;
    use web_sys::{GpuBuffer, GpuBufferDescriptor, GpuDevice, GpuIndexFormat, GpuQueue, GpuRenderPassEncoder};

    use super::{BoardScene, GROUND_VERTICES, ROAD_CAPACITY_ENTRIES, ROAD_VERTEX_FLOATS, road_indices};
    use crate::gpu_flags::{buffer_usage, f32_bytes, u32_bytes};
    use crate::gpu_pipelines::{GpuPipelines, ScenePipeline};

    /// The road's GPU buffers: one vertex buffer rewritten per level and a fixed index buffer.
    pub struct GpuRoad {
        vertex_buffer: GpuBuffer,
        index_buffer: GpuBuffer,
    }

    impl GpuRoad {
        pub fn new(device: &GpuDevice) -> Result<Self, JsValue> {
            let vertex_bytes = ((ROAD_CAPACITY_ENTRIES + 1) * 2 * ROAD_VERTEX_FLOATS * 4) as u32;
            let descriptor = GpuBufferDescriptor::new(vertex_bytes, buffer_usage::VERTEX | buffer_usage::COPY_DST);
            descriptor.set_label("road");
            let vertex_buffer = device.create_buffer(&descriptor)?;
            let indices = road_indices();
            let index_bytes = (indices.len() * 4) as u32;
            let descriptor = GpuBufferDescriptor::new(index_bytes, buffer_usage::INDEX | buffer_usage::COPY_DST);
            descriptor.set_label("road-indices");
            let index_buffer = device.create_buffer(&descriptor)?;
            device.queue().write_buffer_with_u32_and_u8_slice(&index_buffer, 0, u32_bytes(&indices))?;
            Ok(GpuRoad { vertex_buffer, index_buffer })
        }

        /// Uploads the road vertices when the board wrote a new route.
        pub fn sync(&self, queue: &GpuQueue, board: &mut BoardScene) {
            if let Some(vertices) = board.take_road_upload() {
                let _ = queue.write_buffer_with_u32_and_u8_slice(&self.vertex_buffer, 0, f32_bytes(vertices));
            }
        }

        /// The ground is a shader-generated quad around the field (no vertex buffers).
        pub fn draw(&self, pass: &GpuRenderPassEncoder, pipelines: &GpuPipelines, board: &BoardScene) {
            pass.set_pipeline(pipelines.scene(ScenePipeline::Ground));
            pass.draw(GROUND_VERTICES);
            if !board.scenery_visible() {
                return;
            }
            pass.set_pipeline(pipelines.scene(ScenePipeline::Road));
            let index_count = board.road_index_count();
            if index_count == 0 {
                return;
            }
            pass.set_vertex_buffer(0, Some(&self.vertex_buffer));
            pass.set_index_buffer(&self.index_buffer, GpuIndexFormat::Uint32);
            pass.draw_indexed(index_count);
        }

        pub fn dispose(&self) {
            self.vertex_buffer.destroy();
            self.index_buffer.destroy();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::vec3;
    use crate::render_batches::BatchCapacities;
    use vd_core::route_path::create_route_motion_path;
    use vd_core::types::Point;

    fn frame(time: f64) -> FrameContext {
        FrameContext { delta_seconds: 1.0 / 60.0, time, frame: 1, view_direction: vec3(0.0, -1.0, 0.0) }
    }

    #[test]
    fn writes_road_chevrons_portal_gate_and_motes() {
        let points = [Point::new(-20.0, 100.0), Point::new(300.0, 100.0), Point::new(300.0, 400.0)];
        let route = create_route_motion_path(&points, 24.0, 7.0);
        let mut board = BoardScene::new(800.0, 450.0, 21.0);
        board.set_route(Some(&route.entries));
        let upload = board.take_road_upload().expect("road written").len();
        assert_eq!(upload, (route.entries.len() + 1) * 2 * ROAD_VERTEX_FLOATS);
        assert!(board.take_road_upload().is_none());
        board.set_route(Some(&route.entries));
        assert!(board.take_road_upload().is_none(), "same route is not rewritten");
        assert_eq!(board.road_index_count(), route.entries.len() as u32 * 6);

        let mut batches = RenderBatches::new(BatchCapacities { glow_sprites: 512, smoke_sprites: 8, ribbons: 8 });
        board.notify_escapes(5);
        board.notify_escapes(4);
        board.write(&mut batches, &frame(12.5));
        let length = route.entries.last().unwrap().total_distance;
        let chevrons = batches[Batch::RoadChevron].size();
        assert!(chevrons as f64 >= (length + ROAD_LEAD_IN) / CHEVRON_SPACING - 2.0, "{chevrons}");
        assert!(batches[Batch::RoadChevron].used_data().iter().all(|value| value.is_finite()));
        assert_eq!(batches[Batch::Portal].size(), 1);
        assert_eq!(batches[Batch::SpawnGate].size(), 1);
        assert_eq!(batches.glow.size(), MOTE_COUNT as usize + PORTAL_RISING_RINGS as usize);
        assert_eq!(board.draw_calls(), 2);

        board.set_scenery_visible(false);
        batches.begin();
        board.write(&mut batches, &frame(13.0));
        assert_eq!(batches.drawn_instances(), 0);
        assert_eq!(board.draw_calls(), 1);
        board.set_route(None);
        assert_eq!(board.road_index_count(), 0);
    }
}

//! The tilted perspective board camera: field fitting, pointer picking, projection for the
//! overlay, screen shake, player view controls, and the debug close-up framing.
use vd_core::types::{FieldBounds, Point};

use crate::math::{
    Mat4, Vec3, mat4_identity, mat4_invert, mat4_look_at_world, mat4_multiply, mat4_perspective,
    transform_point_projective, vec3,
};

/// A client-space rectangle (the web layer converts a `DOMRect`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SurfaceRect {
    pub left: f64,
    pub top: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct CameraRigOptions {
    pub field_width: f32,
    pub field_height: f32,
    pub vertical_fov_degrees: f32,
    /// Tilt from straight down, toward the bottom of the field.
    pub pitch_radians: f32,
    /// Fraction of the view kept clear around the field.
    pub margin: f32,
}

const NEAR: f32 = 60.0;
const FAR: f32 = 4000.0;
// Close-ups scale the near plane with distance, so zooming in never clips the subject.
const INSPECT_NEAR_RATIO: f32 = 0.1;
const INSPECT_MIN_NEAR: f32 = 0.5;
const FIT_ITERATIONS: usize = 24;
const SHAKE_DECAY_PER_SECOND: f32 = 1.9;
const MAX_SHAKE_OFFSET: f32 = 9.0;
const SHAKE_FREQUENCY: f32 = 31.0;
// Screen-up is world -Z (field y grows down the screen).
const CAMERA_UP: Vec3 = vec3(0.0, 0.0, -1.0);

/// A perspective camera as plain matrices (column-major, WebGPU clip depth 0..1).
#[derive(Clone, Debug)]
pub struct CameraState {
    pub position: Vec3,
    pub world: Mat4,
    pub view: Mat4,
    pub projection: Mat4,
    pub view_projection: Mat4,
    pub inverse_view_projection: Mat4,
    /// Normalized world-space forward vector.
    pub forward: Vec3,
}

impl Default for CameraState {
    fn default() -> Self {
        CameraState {
            position: Vec3::default(),
            world: mat4_identity(),
            view: mat4_identity(),
            projection: mat4_identity(),
            view_projection: mat4_identity(),
            inverse_view_projection: mat4_identity(),
            forward: vec3(0.0, -1.0, 0.0),
        }
    }
}

impl CameraState {
    /// Takes effect with the next `look_at` or `copy_orientation`. `lens_shift` moves the
    /// principal point in NDC (an off-center frustum), so the view axis lands off the canvas
    /// center without changing the field of view.
    pub fn set_perspective(
        &mut self,
        vertical_fov_radians: f32,
        aspect: f32,
        near: f32,
        far: f32,
        lens_shift: (f32, f32),
    ) {
        mat4_perspective(&mut self.projection, vertical_fov_radians, aspect, near, far);
        // clip.xy += shift * clip.w, where clip.w = -view.z.
        self.projection[8] -= lens_shift.0;
        self.projection[9] -= lens_shift.1;
    }

    pub fn look_at(&mut self, eye: Vec3, target: Vec3, up: Vec3) {
        self.position = eye;
        mat4_look_at_world(&mut self.world, eye, target, up);
        self.update();
    }

    /// Keeps the orientation of `source` from another position.
    pub fn copy_orientation(&mut self, source: &CameraState, eye: Vec3) {
        self.world = source.world;
        self.position = eye;
        self.world[12] = eye.x;
        self.world[13] = eye.y;
        self.world[14] = eye.z;
        self.update();
    }

    fn update(&mut self) {
        mat4_invert(&mut self.view, &self.world);
        mat4_multiply(&mut self.view_projection, &self.projection, &self.view);
        mat4_invert(&mut self.inverse_view_projection, &self.view_projection);
        self.forward = vec3(-self.world[8], -self.world[9], -self.world[10]);
    }
}

/// A close-up orbit framing: the field point to center, how many field units fit vertically,
/// the heading around it (0 looks from the bottom of the field, as the board does), and the
/// tilt from straight down (the board uses `BOARD_TILT_RADIANS`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InspectView {
    pub x: f32,
    pub y: f32,
    pub visible_height: f32,
    pub yaw: f32,
    pub tilt: f32,
}

const BOARD_FOV_DEGREES: f32 = 24.0;
pub const BOARD_TILT_RADIANS: f32 = 0.25;
// The range players can tilt the board camera through (desktop), from straight down; the
// field always stays framed.
const MIN_BOARD_TILT_RADIANS: f32 = 0.0;
const MAX_BOARD_TILT_RADIANS: f32 = 0.6;
// Player zoom over the fitted framing (1 = the whole field); panning is limited so the view
// never drifts past the field's edges.
const MIN_BOARD_ZOOM: f32 = 1.0;
const MAX_BOARD_ZOOM: f32 = 4.0;
const BOARD_MARGIN: f32 = 0.006;

/// The board's camera framing; also used by headless checks for real visible bounds.
pub fn create_board_camera_rig(field_width: f32, field_height: f32) -> CameraRig {
    CameraRig::new(CameraRigOptions {
        field_width,
        field_height,
        vertical_fov_degrees: BOARD_FOV_DEGREES,
        pitch_radians: BOARD_TILT_RADIANS,
        margin: BOARD_MARGIN,
    })
}

/// Owns the logical camera used for picking and projection, plus a render camera that
/// adds screen shake. Field (x, y) maps to world (x, 0, y); screen-up is world -Z.
#[derive(Clone, Debug)]
pub struct CameraRig {
    pub logical_camera: CameraState,
    pub render_camera: CameraState,
    options: CameraRigOptions,
    vertical_fov: f32,
    target: Vec3,
    offset_direction: Vec3,
    pitch: f32,
    zoom: f32,
    pan_x: f32,
    pan_z: f32,
    fit_target: Vec3,
    fit_distance: f32,
    aspect: f32,
    distance: f32,
    trauma: f32,
    shake_time: f32,
    viewport_width: f32,
    viewport_height: f32,
    /// CSS pixels covered by the page's HUD (top, right, bottom, left); the field is framed in
    /// the rest (the safe area) while the ground keeps rendering under the HUD.
    insets: [f32; 4],
    visible_bounds: FieldBounds,
    inspection: Option<InspectView>,
}

/// The safe area in NDC: its center (the lens shift) and half extents.
#[derive(Clone, Copy, Debug)]
struct SafeArea {
    center_x: f32,
    center_y: f32,
    half_x: f32,
    half_y: f32,
}

impl CameraRig {
    pub fn new(options: CameraRigOptions) -> CameraRig {
        CameraRig {
            logical_camera: CameraState::default(),
            render_camera: CameraState::default(),
            options,
            vertical_fov: options.vertical_fov_degrees.to_radians(),
            target: Vec3::default(),
            offset_direction: vec3(0.0, 1.0, 0.0),
            pitch: options.pitch_radians,
            zoom: MIN_BOARD_ZOOM,
            pan_x: 0.0,
            pan_z: 0.0,
            fit_target: Vec3::default(),
            fit_distance: 1000.0,
            aspect: 1.0,
            distance: 1000.0,
            trauma: 0.0,
            shake_time: 0.0,
            viewport_width: 1.0,
            viewport_height: 1.0,
            insets: [0.0; 4],
            visible_bounds: FieldBounds {
                min_x: 0.0,
                min_y: 0.0,
                max_x: options.field_width as f64,
                max_y: options.field_height as f64,
            },
            inspection: None,
        }
    }

    /// Gameplay bounds (placement, culling) from the default framing.
    pub fn field_bounds(&self) -> FieldBounds {
        self.visible_bounds
    }

    pub fn tilt(&self) -> f32 {
        self.pitch
    }

    pub fn zoom_factor(&self) -> f32 {
        self.zoom
    }

    /*
     * Player view controls (desktop). Picking and projection follow them; `field_bounds` never
     * does, so they change only what the camera shows, not the rules. Each returns whether
     * the view changed.
     */

    /// Tilts the camera by `delta_radians` within the player range, re-framing the field.
    pub fn tilt_by(&mut self, delta_radians: f32) -> bool {
        let pitch = MAX_BOARD_TILT_RADIANS.min(MIN_BOARD_TILT_RADIANS.max(self.pitch + delta_radians));
        if pitch == self.pitch {
            return false;
        }
        self.pitch = pitch;
        self.fit_field(pitch);
        self.apply_player_view();
        true
    }

    /// Zooms by `factor`, keeping the ground point under the client point in place.
    pub fn zoom_at(&mut self, factor: f32, client_x: f64, client_y: f64, rect: &SurfaceRect) -> bool {
        let zoom = MAX_BOARD_ZOOM.min(MIN_BOARD_ZOOM.max(self.zoom * factor));
        if zoom == self.zoom {
            return false;
        }
        let before = self.client_to_field(client_x, client_y, rect);
        self.zoom = zoom;
        self.apply_player_view();
        let after = self.client_to_field(client_x, client_y, rect);
        if let (Some(before), Some(after)) = (before, after) {
            self.pan_by((before.x - after.x) as f32, (before.y - after.y) as f32);
        }
        true
    }

    /// Pans so the ground point under `from` moves under `to` (grab-the-ground dragging).
    pub fn pan_between(&mut self, from_x: f64, from_y: f64, to_x: f64, to_y: f64, rect: &SurfaceRect) -> bool {
        let from = self.client_to_field(from_x, from_y, rect);
        let to = self.client_to_field(to_x, to_y, rect);
        match (from, to) {
            (Some(from), Some(to)) => self.pan_by((from.x - to.x) as f32, (from.y - to.y) as f32),
            _ => false,
        }
    }

    /// Restores the default tilt, zoom, and pan.
    pub fn reset_view(&mut self) -> bool {
        if self.pitch == self.options.pitch_radians
            && self.zoom == MIN_BOARD_ZOOM
            && self.pan_x == 0.0
            && self.pan_z == 0.0
        {
            return false;
        }
        self.pitch = self.options.pitch_radians;
        self.zoom = MIN_BOARD_ZOOM;
        self.pan_x = 0.0;
        self.pan_z = 0.0;
        self.fit_field(self.pitch);
        self.apply_player_view();
        true
    }

    fn pan_by(&mut self, delta_x: f32, delta_z: f32) -> bool {
        let (pan_x, pan_z) = (self.pan_x, self.pan_z);
        self.pan_x += delta_x;
        self.pan_z += delta_z;
        self.apply_player_view();
        self.pan_x != pan_x || self.pan_z != pan_z
    }

    /// Places the logical camera: the fitted framing, zoomed and panned (pan clamped).
    fn apply_player_view(&mut self) {
        let slack = 1.0 - 1.0 / self.zoom;
        let max_pan_x = self.options.field_width / 2.0 * slack;
        let max_pan_z = self.options.field_height / 2.0 * slack;
        self.pan_x = max_pan_x.min((-max_pan_x).max(self.pan_x));
        self.pan_z = max_pan_z.min((-max_pan_z).max(self.pan_z));
        self.target = vec3(self.fit_target.x + self.pan_x, self.fit_target.y, self.fit_target.z + self.pan_z);
        self.distance = self.fit_distance / self.zoom;
        self.place_camera();
        self.sync_render_camera(0.0, 0.0);
    }

    /// Sets the HUD insets (CSS pixels: top, right, bottom, left) and re-frames the field inside
    /// the remaining safe area. Returns whether they changed.
    pub fn set_insets(&mut self, top: f32, right: f32, bottom: f32, left: f32) -> bool {
        let insets = [top.max(0.0), right.max(0.0), bottom.max(0.0), left.max(0.0)];
        if insets == self.insets {
            return false;
        }
        self.insets = insets;
        self.resize(self.viewport_width, self.viewport_height);
        true
    }

    /// The safe area in NDC; insets that would leave less than a fifth of the view are scaled back.
    fn safe_area(&self) -> SafeArea {
        let [top, right, bottom, left] = self.insets;
        let horizontal = (left + right) / self.viewport_width;
        let vertical = (top + bottom) / self.viewport_height;
        let scale_x = if horizontal > 0.8 { 0.8 / horizontal } else { 1.0 };
        let scale_y = if vertical > 0.8 { 0.8 / vertical } else { 1.0 };
        let min_x = -1.0 + 2.0 * left * scale_x / self.viewport_width;
        let max_x = 1.0 - 2.0 * right * scale_x / self.viewport_width;
        let min_y = -1.0 + 2.0 * bottom * scale_y / self.viewport_height;
        let max_y = 1.0 - 2.0 * top * scale_y / self.viewport_height;
        SafeArea {
            center_x: (min_x + max_x) / 2.0,
            center_y: (min_y + max_y) / 2.0,
            half_x: (max_x - min_x) / 2.0,
            half_y: (max_y - min_y) / 2.0,
        }
    }

    fn lens_shift(&self) -> (f32, f32) {
        let safe = self.safe_area();
        (safe.center_x, safe.center_y)
    }

    pub fn resize(&mut self, width: f32, height: f32) {
        self.viewport_width = width.max(1.0);
        self.viewport_height = height.max(1.0);
        self.aspect = self.viewport_width / self.viewport_height;
        let shift = self.lens_shift();
        self.logical_camera.set_perspective(self.vertical_fov, self.aspect, NEAR, FAR, shift);
        self.render_camera.set_perspective(self.vertical_fov, self.aspect, NEAR, FAR, shift);
        // Gameplay bounds (placement, culling) always come from the default framing, so the
        // player's view changes only what the camera shows, never the rules.
        self.fit_field(self.options.pitch_radians);
        self.update_visible_bounds();
        if self.pitch != self.options.pitch_radians {
            self.fit_field(self.pitch);
        }
        self.apply_player_view();
    }

    /// Debug/render-script aid: frames the render camera tightly over a field point at the
    /// same pitch, without screen shake. Picking keeps using the logical camera. Pass `None`
    /// to restore.
    pub fn inspect(&mut self, view: Option<InspectView>) {
        self.inspection = view;
        self.sync_render_camera(0.0, 0.0);
    }

    /// Adds screen-shake trauma in 0..1; shake strength follows trauma squared.
    pub fn add_trauma(&mut self, amount: f32) {
        self.trauma = (self.trauma + amount).min(1.0);
    }

    pub fn update(&mut self, delta_seconds: f32) {
        if self.trauma <= 0.0 {
            return;
        }

        self.shake_time += delta_seconds;
        self.trauma = (self.trauma - SHAKE_DECAY_PER_SECOND * delta_seconds).max(0.0);
        // Shake is in world units, so zoomed-in views scale it back to the same on-screen jolt.
        let strength = self.trauma * self.trauma * MAX_SHAKE_OFFSET / self.zoom;
        let t = self.shake_time * SHAKE_FREQUENCY;
        let offset_x = ((t * 1.13).sin() + (t * 2.71).sin() * 0.5) * strength * 0.66;
        let offset_z = ((t * 0.97).cos() + (t * 3.17).sin() * 0.5) * strength * 0.66;
        self.sync_render_camera(offset_x, offset_z);
    }

    /// Ray-casts a client-space point onto the ground plane.
    pub fn client_to_field(&self, client_x: f64, client_y: f64, rect: &SurfaceRect) -> Option<Point> {
        if rect.width <= 0.0 || rect.height <= 0.0 {
            return None;
        }
        let ndc_x = (client_x - rect.left) / rect.width * 2.0 - 1.0;
        let ndc_y = 1.0 - (client_y - rect.top) / rect.height * 2.0;
        self.ndc_to_ground(ndc_x as f32, ndc_y as f32)
    }

    /// Projects a world point into CSS pixels relative to the canvas.
    pub fn project_to_viewport(&self, x: f32, y: f32, z: f32) -> Point {
        let projected = transform_point_projective(&self.logical_camera.view_projection, x, y, z);
        Point::new(
            ((projected.x + 1.0) * 0.5 * self.viewport_width) as f64,
            ((1.0 - projected.y) * 0.5 * self.viewport_height) as f64,
        )
    }

    fn ndc_to_ground(&self, ndc_x: f32, ndc_y: f32) -> Option<Point> {
        let camera = &self.logical_camera;
        let origin = camera.position;
        let point = transform_point_projective(&camera.inverse_view_projection, ndc_x, ndc_y, 0.5);
        let dx = (point.x - origin.x) as f64;
        let dy = (point.y - origin.y) as f64;
        let dz = (point.z - origin.z) as f64;
        let length = (dx * dx + dy * dy + dz * dz).sqrt();
        let length = if length > 0.0 { length } else { 1.0 };
        let direction_y = dy / length;
        if direction_y >= -1e-6 {
            return None;
        }
        let distance = -origin.y as f64 / direction_y;
        Some(Point::new(origin.x as f64 + dx / length * distance, origin.z as f64 + dz / length * distance))
    }

    fn place_camera(&mut self) {
        let (target, direction, distance) = (self.target, self.offset_direction, self.distance);
        let eye = vec3(
            target.x + direction.x * distance,
            target.y + direction.y * distance,
            target.z + direction.z * distance,
        );
        self.logical_camera.look_at(eye, target, CAMERA_UP);
    }

    /// Fits the logical camera at `pitch`: finds the distance and look-at target that frame the
    /// whole field with the requested margin, re-centering the (trapezoidal) projection
    /// vertically, and records them as the base the player's zoom and pan apply to.
    fn fit_field(&mut self, pitch: f32) {
        let CameraRigOptions { field_width, field_height, margin, .. } = self.options;
        self.offset_direction.y = pitch.cos();
        self.offset_direction.z = pitch.sin();
        let limit = 1.0 - margin;
        let safe = self.safe_area();
        self.target = vec3(field_width / 2.0, 0.0, field_height / 2.0);
        let half_fov = self.vertical_fov / 2.0;
        self.distance =
            (field_height / safe.half_y).max(field_width / self.aspect / safe.half_x) / 2.0 / half_fov.tan();

        let corners = [(0.0, 0.0), (field_width, 0.0), (0.0, field_height), (field_width, field_height)];
        for _ in 0..FIT_ITERATIONS {
            self.place_camera();
            let (mut min_x, mut max_x) = (f32::INFINITY, f32::NEG_INFINITY);
            let (mut min_y, mut max_y) = (f32::INFINITY, f32::NEG_INFINITY);
            for (x, z) in corners {
                let projected = transform_point_projective(&self.logical_camera.view_projection, x, 0.0, z);
                min_x = min_x.min(projected.x);
                max_x = max_x.max(projected.x);
                min_y = min_y.min(projected.y);
                max_y = max_y.max(projected.y);
            }
            // Extents relative to the safe area, which the lens shift already centers on.
            let extent = ((max_x - min_x) / 2.0 / safe.half_x).max((max_y - min_y) / 2.0 / safe.half_y);
            let center_y = (max_y + min_y) / 2.0 - safe.center_y;
            // Screen-up is -Z; shift the target so the projected field sits centered.
            let world_per_ndc = half_fov.tan() * self.distance;
            self.target.z -= center_y * world_per_ndc * 0.9;
            self.distance *= 1.0 + (extent / limit - 1.0) * 0.9;
        }
        self.place_camera();
        self.fit_target = self.target;
        self.fit_distance = self.distance;
    }

    /// Gameplay bounds: the largest axis-aligned rectangle inside the ground visible in the safe
    /// area (not under the HUD), limited to the field itself. A wide screen shows more ground
    /// around the field, but the playable area stays the authored one.
    fn update_visible_bounds(&mut self) {
        let safe = self.safe_area();
        let (left, right) = (safe.center_x - safe.half_x, safe.center_x + safe.half_x);
        let (bottom, top) = (safe.center_y - safe.half_y, safe.center_y + safe.half_y);
        let (Some(top_left), Some(top_right), Some(bottom_left), Some(bottom_right)) = (
            self.ndc_to_ground(left, top),
            self.ndc_to_ground(right, top),
            self.ndc_to_ground(left, bottom),
            self.ndc_to_ground(right, bottom),
        ) else {
            return;
        };
        let (field_width, field_height) = (self.options.field_width as f64, self.options.field_height as f64);
        self.visible_bounds = FieldBounds {
            min_x: top_left.x.max(bottom_left.x).max(0.0),
            max_x: top_right.x.min(bottom_right.x).min(field_width),
            min_y: top_left.y.max(top_right.y).max(0.0),
            max_y: bottom_left.y.min(bottom_right.y).min(field_height),
        };
    }

    fn sync_render_camera(&mut self, offset_x: f32, offset_z: f32) {
        if let Some(inspection) = self.inspection {
            let distance = inspection.visible_height / 2.0 / (self.vertical_fov / 2.0).tan();
            let (sin_tilt, cos_tilt) = inspection.tilt.sin_cos();
            let (sin_yaw, cos_yaw) = inspection.yaw.sin_cos();
            let near = NEAR.min(INSPECT_MIN_NEAR.max(distance * INSPECT_NEAR_RATIO));
            // Close-ups center on the canvas (no HUD lens shift).
            self.render_camera.set_perspective(self.vertical_fov, self.aspect, near, FAR, (0.0, 0.0));
            // The up vector is the orbit direction's tilt derivative, so it never degenerates.
            self.render_camera.look_at(
                vec3(
                    inspection.x + sin_yaw * sin_tilt * distance,
                    cos_tilt * distance,
                    inspection.y + cos_yaw * sin_tilt * distance,
                ),
                vec3(inspection.x, 0.0, inspection.y),
                vec3(-sin_yaw * cos_tilt, sin_tilt, -cos_yaw * cos_tilt),
            );
        } else {
            let shift = self.lens_shift();
            self.render_camera.set_perspective(self.vertical_fov, self.aspect, NEAR, FAR, shift);
            let logical = self.logical_camera.position;
            let eye = vec3(logical.x + offset_x, logical.y, logical.z + offset_z);
            self.render_camera.copy_orientation(&self.logical_camera, eye);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rig(width: f32, height: f32, viewport: (f32, f32)) -> CameraRig {
        let mut rig = create_board_camera_rig(width, height);
        rig.resize(viewport.0, viewport.1);
        rig
    }

    fn rect(viewport: (f32, f32)) -> SurfaceRect {
        SurfaceRect { left: 10.0, top: 20.0, width: viewport.0 as f64, height: viewport.1 as f64 }
    }

    #[test]
    fn picking_round_trips_projected_field_points() {
        for (field, viewport) in [((800.0, 450.0), (1280.0, 720.0)), ((390.0, 560.0), (375.0, 600.0))] {
            let rig = rig(field.0, field.1, viewport);
            let surface = rect(viewport);
            for (x, y) in [(0.0, 0.0), (field.0 / 2.0, field.1 / 2.0), (field.0, field.1), (120.0, 333.0)] {
                let screen = rig.project_to_viewport(x, 0.0, y);
                let picked =
                    rig.client_to_field(screen.x + surface.left, screen.y + surface.top, &surface).expect("ground hit");
                assert!((picked.x - x as f64).abs() < 0.05 && (picked.y - y as f64).abs() < 0.05, "{picked:?}");
            }
        }
    }

    #[test]
    fn fitted_field_fills_the_view_with_a_margin() {
        let viewport = (1280.0, 720.0);
        let rig = rig(800.0, 450.0, viewport);
        let corners = [(0.0, 0.0), (800.0, 0.0), (0.0, 450.0), (800.0, 450.0)];
        let mut max_extent: f64 = 0.0;
        for (x, y) in corners {
            let screen = rig.project_to_viewport(x, 0.0, y);
            assert!(screen.x >= -1.0 && screen.x <= 1281.0 && screen.y >= -1.0 && screen.y <= 721.0, "{screen:?}");
            max_extent = max_extent.max((screen.x - 640.0).abs() / 640.0).max((screen.y - 360.0).abs() / 360.0);
        }
        assert!((max_extent - (1.0 - BOARD_MARGIN as f64)).abs() < 0.01, "{max_extent}");
        let bounds = rig.field_bounds();
        assert_eq!(bounds, FieldBounds { min_x: 0.0, min_y: 0.0, max_x: 800.0, max_y: 450.0 });
    }

    #[test]
    fn insets_frame_the_field_in_the_safe_area() {
        let viewport = (1440.0, 900.0);
        let mut rig = rig(800.0, 450.0, viewport);
        assert!(rig.set_insets(80.0, 20.0, 140.0, 20.0));
        assert!(!rig.set_insets(80.0, 20.0, 140.0, 20.0));
        let corners = [(0.0, 0.0), (800.0, 0.0), (0.0, 450.0), (800.0, 450.0)];
        let (mut top, mut bottom): (f64, f64) = (f64::INFINITY, f64::NEG_INFINITY);
        for (x, y) in corners {
            let screen = rig.project_to_viewport(x, 0.0, y);
            assert!(screen.x >= 19.0 && screen.x <= 1421.0, "{screen:?}");
            top = top.min(screen.y);
            bottom = bottom.max(screen.y);
        }
        assert!(top >= 79.0 && bottom <= 761.0, "{top} {bottom}");
        // The field touches the safe area on its limiting axis.
        assert!((top - 80.0).abs() < 8.0 || (bottom - 760.0).abs() < 8.0, "{top} {bottom}");
        // Picking still round-trips through the shifted projection.
        let surface = rect(viewport);
        let screen = rig.project_to_viewport(123.0, 0.0, 321.0);
        let picked = rig.client_to_field(screen.x + surface.left, screen.y + surface.top, &surface).unwrap();
        assert!((picked.x - 123.0).abs() < 0.05 && (picked.y - 321.0).abs() < 0.05, "{picked:?}");
        assert_eq!(rig.field_bounds(), FieldBounds { min_x: 0.0, min_y: 0.0, max_x: 800.0, max_y: 450.0 });
    }

    #[test]
    fn player_view_controls_keep_gameplay_bounds() {
        let viewport = (1280.0, 720.0);
        let surface = rect(viewport);
        let mut rig = rig(800.0, 450.0, viewport);
        let bounds = rig.field_bounds();
        let center = (surface.left + 640.0, surface.top + 360.0);
        let anchor = rig.client_to_field(center.0 + 100.0, center.1, &surface).unwrap();
        assert!(rig.zoom_at(2.0, center.0 + 100.0, center.1, &surface));
        let after = rig.client_to_field(center.0 + 100.0, center.1, &surface).unwrap();
        assert!((anchor.x - after.x).abs() < 0.05 && (anchor.y - after.y).abs() < 0.05);
        assert!(rig.pan_between(center.0, center.1, center.0 + 50.0, center.1, &surface));
        assert!(rig.tilt_by(0.2));
        assert!(!rig.tilt_by(10.0) || rig.tilt() == MAX_BOARD_TILT_RADIANS);
        assert_eq!(rig.field_bounds(), bounds);
        assert!(rig.reset_view());
        assert!(!rig.reset_view());
    }

    #[test]
    fn shake_moves_only_the_render_camera() {
        let mut rig = rig(800.0, 450.0, (1280.0, 720.0));
        let logical = rig.logical_camera.position;
        rig.add_trauma(0.8);
        rig.update(0.05);
        assert_eq!(rig.logical_camera.position, logical);
        assert_ne!(rig.render_camera.position, logical);
        rig.inspect(Some(InspectView { x: 400.0, y: 225.0, visible_height: 60.0, yaw: 0.0, tilt: BOARD_TILT_RADIANS }));
        assert!(rig.render_camera.position.y < 400.0);
    }
}

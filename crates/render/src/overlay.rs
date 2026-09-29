//! Screen-space HUD drawn over the 3D board on a 2D canvas: selected-tower actions and the
//! escape allowance. Field-space hit tests are projected so they match what the player sees.
//!
//! The layout and hit tests are native (and tested); the canvas drawing is Wasm-only.
use vd_core::types::Point;

use crate::camera_rig::CameraRig;

const BUTTON_WIDTH: f64 = 32.0;
const BUTTON_HEIGHT: f64 = 26.0;
const COMPACT_BUTTON_WIDTH: f64 = 84.0;
const COMPACT_GROUPED_BUTTON_WIDTH: f64 = 54.0;
const COMPACT_BUTTON_HEIGHT: f64 = 54.0;
const BUTTON_GAP: f64 = 6.0;
const BUTTON_OFFSET: f64 = 24.0;
const EDGE_GUTTER: f64 = 42.0;
const COMPACT_WIDTH_THRESHOLD: f64 = 520.0;
#[cfg(target_arch = "wasm32")]
const ESCAPE_LABEL_HEIGHT: f32 = 3.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// The selected tower as the overlay last saw it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OverlaySelection {
    pub x: f64,
    pub y: f64,
    pub laser: bool,
}

/// Overlay size and the selection its buttons belong to. The TypeScript overlay read the
/// selection from `Game` on every hit test; here it is the selection of the last drawn frame
/// (the one the player is clicking on).
pub struct OverlayLayout {
    width: f64,
    height: f64,
    field_width: f64,
    draw_actions: bool,
    selection: Option<OverlaySelection>,
}

impl OverlayLayout {
    pub fn new(field_width: f64, draw_actions: bool) -> Self {
        OverlayLayout { width: 1.0, height: 1.0, field_width, draw_actions, selection: None }
    }

    pub fn resize(&mut self, width: f64, height: f64) {
        self.width = width.max(1.0);
        self.height = height.max(1.0);
    }

    pub fn size(&self) -> (f64, f64) {
        (self.width, self.height)
    }

    pub fn set_selection(&mut self, selection: Option<OverlaySelection>) {
        self.selection = selection;
    }

    fn compact(&self) -> bool {
        self.width <= COMPACT_WIDTH_THRESHOLD
    }

    /// Width of the field in CSS pixels per field unit.
    pub fn field_scale(&self) -> f64 {
        self.width / self.field_width
    }

    pub fn action_rect(&self, rig: &CameraRig, index: usize) -> Option<ScreenRect> {
        if !self.draw_actions {
            return None;
        }
        let selected = self.selection?;
        let action_count = if selected.laser { 2 } else { 1 };
        if index >= action_count {
            return None;
        }
        let field_scale = self.field_scale();
        let compact = self.compact();
        let width = if compact {
            if action_count > 1 { COMPACT_GROUPED_BUTTON_WIDTH } else { COMPACT_BUTTON_WIDTH }
        } else {
            BUTTON_WIDTH * 1f64.max(field_scale * 0.85)
        };
        let height = if compact { COMPACT_BUTTON_HEIGHT } else { BUTTON_HEIGHT * 1f64.max(field_scale * 0.85) };
        let count = action_count as f64;
        let group_width = width * count + BUTTON_GAP * (count - 1.0);
        let anchor = rig.project_to_viewport(selected.x as f32, 0.0, selected.y as f32);
        let offset = BUTTON_OFFSET * 1f64.max(field_scale);
        let center_x = anchor.x.max(EDGE_GUTTER + group_width / 2.0).min(self.width - EDGE_GUTTER - group_width / 2.0);
        let place_above = anchor.y + offset + height > self.height - 8.0;
        let top = if place_above { anchor.y - offset - height - 8.0 * field_scale } else { anchor.y + offset };
        Some(ScreenRect {
            x: center_x - group_width / 2.0 + index as f64 * (width + BUTTON_GAP),
            y: top,
            width,
            height,
        })
    }

    fn hit_test(rig: &CameraRig, field_point: Point, rect: Option<ScreenRect>) -> bool {
        let Some(rect) = rect else {
            return false;
        };
        let screen = rig.project_to_viewport(field_point.x as f32, 0.0, field_point.y as f32);
        screen.x >= rect.x && screen.x <= rect.x + rect.width && screen.y >= rect.y && screen.y <= rect.y + rect.height
    }

    pub fn is_point_in_upgrade_button(&self, rig: &CameraRig, point: Point) -> bool {
        Self::hit_test(rig, point, self.action_rect(rig, 0))
    }

    pub fn is_point_in_laser_lock_button(&self, rig: &CameraRig, point: Point) -> bool {
        self.selection.is_some_and(|selection| selection.laser) && Self::hit_test(rig, point, self.action_rect(rig, 1))
    }
}

/// JavaScript `Math.round`.
#[cfg(target_arch = "wasm32")]
fn js_round(value: f64) -> f64 {
    (value + 0.5).floor()
}

#[cfg(target_arch = "wasm32")]
pub use surface::OverlaySurface;

#[cfg(target_arch = "wasm32")]
mod surface {
    use vd_core::game::Game;
    use wasm_bindgen::{JsCast, JsValue};
    use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

    use super::{BUTTON_HEIGHT, ESCAPE_LABEL_HEIGHT, OverlayLayout, OverlaySelection, ScreenRect, js_round};
    use crate::camera_rig::CameraRig;
    use crate::shaders::push_integer;

    /// The 2D canvas stacked above the WebGPU canvas.
    pub struct OverlaySurface {
        canvas: HtmlCanvasElement,
        context: CanvasRenderingContext2d,
        text: String,
    }

    impl OverlaySurface {
        pub fn new(canvas: HtmlCanvasElement) -> Result<Self, JsValue> {
            let context = canvas
                .get_context("2d")?
                .ok_or_else(|| JsValue::from_str("Overlay canvas context unavailable."))?
                .unchecked_into::<CanvasRenderingContext2d>();
            Ok(OverlaySurface { canvas, context, text: String::with_capacity(96) })
        }

        pub fn resize(&mut self, layout: &mut OverlayLayout, width: f64, height: f64, pixel_ratio: f64) {
            layout.resize(width, height);
            let (width, height) = layout.size();
            self.canvas.set_width(js_round(width * pixel_ratio) as u32);
            self.canvas.set_height(js_round(height * pixel_ratio) as u32);
            let _ = self.context.set_transform(pixel_ratio, 0.0, 0.0, pixel_ratio, 0.0, 0.0);
        }

        /// Redraws the overlay for this frame (as the TypeScript overlay did every frame).
        pub fn draw(&mut self, game: &Game, layout: &mut OverlayLayout, rig: &CameraRig) {
            let selection = game.runtime.selected_tower.as_ref().map(|tower| {
                let tower = tower.borrow();
                OverlaySelection { x: tower.x, y: tower.y, laser: tower.is_laser() }
            });
            layout.set_selection(selection);
            let (width, height) = layout.size();
            self.context.clear_rect(0.0, 0.0, width, height);
            self.draw_escape_allowance(game, layout, rig);
            self.draw_tower_actions(game, layout, rig);
        }

        fn draw_tower_actions(&mut self, game: &Game, layout: &OverlayLayout, rig: &CameraRig) {
            let Some(upgrade) = layout.action_rect(rig, 0) else {
                return;
            };
            let context = &self.context;
            let pointer = game.runtime.pointer;
            let disabled = !game.can_upgrade_selected_tower();
            let hovered = pointer.is_some_and(|pointer| layout.is_point_in_upgrade_button(rig, pointer));
            context.save();
            draw_button(context, upgrade, disabled, hovered);
            let scale = upgrade.height / BUTTON_HEIGHT;
            let center_x = upgrade.x + upgrade.width / 2.0;
            let center_y = upgrade.y + upgrade.height / 2.0;
            let half_width = 7.0 * scale.min(1.6);
            let half_height = 6.0 * scale.min(1.6);
            context.begin_path();
            context.move_to(center_x, center_y - half_height);
            context.line_to(center_x + half_width, center_y + half_height);
            context.line_to(center_x - half_width, center_y + half_height);
            context.close_path();
            context.fill();
            context.restore();

            let Some(selected) = game.runtime.selected_tower.as_ref() else {
                return;
            };
            let locked = {
                let tower = selected.borrow();
                if !tower.is_laser() {
                    return;
                }
                tower.direction_locked()
            };
            let Some(lock) = layout.action_rect(rig, 1) else {
                return;
            };
            let lock_hovered = pointer.is_some_and(|pointer| layout.is_point_in_laser_lock_button(rig, pointer));
            context.save();
            draw_button(context, lock, !game.can_perform_battle_action(), lock_hovered);
            self.text.clear();
            push_integer(&mut self.text, js_round(lock.height * 0.58));
            self.text.push_str("px \"Apple Color Emoji\", \"Segoe UI Emoji\", system-ui, sans-serif");
            context.set_font(&self.text);
            context.set_text_align("center");
            context.set_text_baseline("middle");
            let glyph = if locked { "🔓" } else { "🔒" };
            let _ = context.fill_text(glyph, lock.x + lock.width / 2.0, lock.y + lock.height / 2.0);
            context.restore();
        }

        fn draw_escape_allowance(&mut self, game: &Game, layout: &OverlayLayout, rig: &CameraRig) {
            if game.current_level().is_none() {
                return;
            }
            let Some(exit) = game.runtime.route_path.as_ref().and_then(|route| route.entries.last()) else {
                return;
            };
            let screen = rig.project_to_viewport(exit.x as f32, ESCAPE_LABEL_HEIGHT, exit.y as f32);
            let size = js_round(19.0 * 0.85f64.max(1.6f64.min(layout.field_scale())));
            let context = &self.context;
            context.save();
            self.text.clear();
            self.text.push_str("700 ");
            push_integer(&mut self.text, size);
            self.text.push_str("px Inter, system-ui, sans-serif");
            context.set_font(&self.text);
            context.set_text_align("center");
            context.set_text_baseline("middle");
            context.set_shadow_color("rgba(49, 255, 235, 0.4)");
            context.set_shadow_blur(5.0);
            context.set_fill_style_str("rgba(238, 255, 248, 0.95)");
            self.text.clear();
            push_integer(&mut self.text, game.runtime.escapes_left.max(0) as f64);
            let _ = context.fill_text(&self.text, screen.x, screen.y + 1.0);
            context.restore();
        }
    }

    fn draw_button(context: &CanvasRenderingContext2d, rect: ScreenRect, disabled: bool, hovered: bool) {
        let active = hovered && !disabled;
        context.set_global_alpha(if disabled { 0.4 } else { 1.0 });
        context.set_fill_style_str(if active { "rgba(33, 57, 50, 0.72)" } else { "rgba(10, 24, 20, 0.86)" });
        context.set_stroke_style_str(if active { "rgba(176, 255, 225, 0.6)" } else { "rgba(176, 255, 225, 0.28)" });
        context.set_line_width(1.0);
        context.set_shadow_color(if disabled { "transparent" } else { "rgba(49, 255, 235, 0.25)" });
        context.set_shadow_blur(if active { 12.0 } else { 7.0 });
        context.begin_path();
        let _ = context.round_rect_with_f64(rect.x, rect.y, rect.width, rect.height, 7.0);
        context.fill();
        context.stroke();
        context.set_shadow_blur(0.0);
        context.set_fill_style_str("#effff7");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera_rig::create_board_camera_rig;

    #[test]
    fn buttons_follow_the_selection_and_hit_tests_match() {
        let mut rig = create_board_camera_rig(800.0, 450.0);
        rig.resize(1280.0, 720.0);
        let mut layout = OverlayLayout::new(800.0, true);
        layout.resize(1280.0, 720.0);
        assert!(layout.action_rect(&rig, 0).is_none());
        layout.set_selection(Some(OverlaySelection { x: 400.0, y: 200.0, laser: true }));
        let upgrade = layout.action_rect(&rig, 0).expect("upgrade button");
        let lock = layout.action_rect(&rig, 1).expect("lock button");
        assert!(lock.x > upgrade.x + upgrade.width);
        let anchor = rig.project_to_viewport(400.0, 0.0, 200.0);
        assert!(upgrade.y > anchor.y, "buttons sit below the tower");
        // A field point that projects into the middle of the upgrade button hits it.
        let rect = crate::camera_rig::SurfaceRect { left: 0.0, top: 0.0, width: 1280.0, height: 720.0 };
        let inside =
            rig.client_to_field(upgrade.x + upgrade.width / 2.0, upgrade.y + upgrade.height / 2.0, &rect).unwrap();
        assert!(layout.is_point_in_upgrade_button(&rig, inside));
        assert!(!layout.is_point_in_laser_lock_button(&rig, inside));
        layout.set_selection(Some(OverlaySelection { x: 400.0, y: 200.0, laser: false }));
        assert!(layout.action_rect(&rig, 1).is_none());
        assert!(!layout.is_point_in_laser_lock_button(&rig, inside));
        let mobile = OverlayLayout::new(390.0, false);
        assert!(mobile.action_rect(&rig, 0).is_none());
    }
}

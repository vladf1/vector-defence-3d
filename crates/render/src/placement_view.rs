//! Build-mode hologram, range rings, and crosshair guides.
use std::f32::consts::PI;

use vd_core::entities::Tower;
use vd_core::entities::towers::registry::tower_info;
use vd_core::game::Game;
use vd_core::types::{Color, FieldBounds, TowerKind};

use crate::frame_math::FrameContext;
use crate::palette::{HOLOGRAM_INVALID, HOLOGRAM_VALID, LinearColor, linear_color};
use crate::render_batches::{Batch, RenderBatches};
use crate::tower_view::write_tower;

const RANGE_Y: f32 = 0.7;
const SELECTED_RANGE: Color = 0x5cff9e;
// The range layer is additive, so this scale is its opacity.
const SELECTED_RANGE_INTENSITY: f32 = 0.385;
const CROSSHAIR_WIDTH: f32 = 0.9;
const HOLOGRAM_FLICKER_HZ: f64 = 7.0;

#[derive(Default)]
pub struct PlacementView {
    /// One hologram tower per kind, created on first use (the TS `ghosts` map).
    ghosts: Vec<Tower>,
}

impl PlacementView {
    /// `bounds` are the renderer's visible field bounds (the TS `game.renderer.getVisibleFieldBounds()`).
    pub fn write(&mut self, game: &Game, bounds: FieldBounds, batches: &mut RenderBatches, frame: &FrameContext) {
        let runtime = &game.runtime;
        if let Some(selected) = &runtime.selected_tower
            && runtime.placing_tower.is_none()
        {
            let selected = selected.borrow();
            let color = linear_color(SELECTED_RANGE).scaled(SELECTED_RANGE_INTENSITY);
            push_range(batches, selected.x as f32, selected.y as f32, selected.range as f32, color);
        }

        let (Some(pointer), Some(kind)) = (runtime.pointer, runtime.placing_tower) else {
            return;
        };
        let info = tower_info(kind);
        let valid = game.can_place_tower_in_bounds(pointer, bounds) && runtime.money >= info.base_cost;
        let color = linear_color(if valid { HOLOGRAM_VALID } else { HOLOGRAM_INVALID });
        let range = (info.base_range * game.profile.tower_range_scale) as f32;
        let (x, z) = (pointer.x as f32, pointer.y as f32);
        push_range(batches, x, z, range, color.scaled(0.8));

        let guide = color.scaled(0.16);
        let span_x = (bounds.max_x - bounds.min_x) as f32;
        let span_y = (bounds.max_y - bounds.min_y) as f32;
        let (min_x, min_y) = (bounds.min_x as f32, bounds.min_y as f32);
        let ribbon = &mut batches[Batch::Ribbon];
        let horizontal =
            ribbon.push_yaw(min_x, RANGE_Y, z, 0.0, span_x, 1.0, CROSSHAIR_WIDTH, guide.r, guide.g, guide.b);
        ribbon.set_extra(horizontal, 0, 0.0);
        let vertical =
            ribbon.push_yaw(x, RANGE_Y, min_y, -PI / 2.0, span_y, 1.0, CROSSHAIR_WIDTH, guide.r, guide.g, guide.b);
        ribbon.set_extra(vertical, 0, 0.0);

        let flicker = 0.75 + ((frame.time * std::f64::consts::PI * 2.0 * HOLOGRAM_FLICKER_HZ).sin() * 0.12) as f32;
        let ghost = self.ghost(kind);
        ghost.x = pointer.x;
        ghost.y = pointer.y;
        write_tower(ghost, false, Some(color.scaled(flicker)), batches, frame);
    }

    fn ghost(&mut self, kind: TowerKind) -> &mut Tower {
        let index = match self.ghosts.iter().position(|ghost| ghost.kind == kind) {
            Some(index) => index,
            None => {
                let mut ghost = Tower::new(kind, 0.0, 0.0);
                ghost.set_angle(-std::f64::consts::PI / 4.0);
                self.ghosts.push(ghost);
                self.ghosts.len() - 1
            }
        };
        &mut self.ghosts[index]
    }
}

fn push_range(batches: &mut RenderBatches, x: f32, z: f32, radius: f32, color: LinearColor) {
    batches[Batch::Range].push_yaw(x, RANGE_Y, z, 0.0, radius, 1.0, radius, color.r, color.g, color.b);
}

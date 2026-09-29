//! The startup-fixed scene constants the shaders bake in: lights, field size, road width,
//! and smoke-puff blobs.
use vd_core::types::Color;

use crate::palette::linear_channels;
use crate::puff_blobs::create_puff_blobs;
use crate::shaders::{SceneConstants, Vec3Tuple};

pub const ROAD_BORDER_TOTAL: f64 = 3.0;
pub const BACKGROUND: Color = 0x010403;
const HEMISPHERE_INTENSITY: f64 = 0.55;
const SUN_INTENSITY: f64 = 2.7;
const RIM_INTENSITY: f64 = 0.7;
// Toward the lights from the field center (the three.js scene's sun and rim placements).
const SUN_OFFSET: Vec3Tuple = [-320.0, 820.0, -430.0];
const RIM_OFFSET: Vec3Tuple = [400.0, 260.0, 520.0];

fn scaled_color(color: Color, intensity: f64) -> Vec3Tuple {
    linear_channels(color).map(|channel| channel * intensity)
}

fn direction([x, y, z]: Vec3Tuple) -> Vec3Tuple {
    let length = (x * x + y * y + z * z).sqrt();
    let length = if length > 0.0 { length } else { 1.0 };
    [x / length, y / length, z / length]
}

/// Lights, field size, and smoke-puff blobs never change after startup; shaders bake them in.
pub fn create_scene_constants(field_width: f64, field_height: f64, road_width: f64) -> SceneConstants {
    SceneConstants {
        field_width,
        field_height,
        road_half_width: (road_width + ROAD_BORDER_TOTAL) / 2.0,
        hemi_sky: scaled_color(0x4fb39a, HEMISPHERE_INTENSITY),
        hemi_ground: scaled_color(0x020504, HEMISPHERE_INTENSITY),
        key_color: scaled_color(0xe4fff4, SUN_INTENSITY),
        rim_color: scaled_color(0x39d8ff, RIM_INTENSITY),
        key_direction: direction(SUN_OFFSET),
        rim_direction: direction(RIM_OFFSET),
        puff_blobs: create_puff_blobs(),
    }
}

//! Linear-space colors. Entity colors are authored as sRGB `0xRRGGBB` values (`Color`).
use std::sync::LazyLock;

use vd_core::types::Color;

/// A linear-space RGB triple.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LinearColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl LinearColor {
    pub const fn new(r: f32, g: f32, b: f32) -> LinearColor {
        LinearColor { r, g, b }
    }

    pub fn scaled(self, factor: f32) -> LinearColor {
        LinearColor::new(self.r * factor, self.g * factor, self.b * factor)
    }
}

/// sRGB transfer (0..1) to linear, in `f64` for the baked shader constants.
pub fn srgb_to_linear(channel: f64) -> f64 {
    if channel <= 0.04045 { channel * 0.0773993808 } else { (channel * 0.9478672986 + 0.0521327014).powf(2.4) }
}

/// `color` as linear `f64` channels (the precision the shader constants are baked with).
pub fn linear_channels(color: Color) -> [f64; 3] {
    [(color >> 16) & 0xff, (color >> 8) & 0xff, color & 0xff].map(|channel| srgb_to_linear(channel as f64 / 255.0))
}

/// Linear values of the 256 sRGB channel levels, so per-frame color conversion is three
/// table reads (entity colors are 8-bit sRGB).
static CHANNEL_TABLE: LazyLock<[f32; 256]> =
    LazyLock::new(|| std::array::from_fn(|level| srgb_to_linear(level as f64 / 255.0) as f32));

/// Converts an sRGB `0xRRGGBB` color into linear RGB.
pub fn linear_color(color: Color) -> LinearColor {
    let table = &*CHANNEL_TABLE;
    LinearColor::new(
        table[((color >> 16) & 0xff) as usize],
        table[((color >> 8) & 0xff) as usize],
        table[(color & 0xff) as usize],
    )
}

pub const HOLOGRAM_VALID: Color = 0x5cff9e;
pub const HOLOGRAM_INVALID: Color = 0xff6a6a;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_endpoints_and_midtones() {
        assert_eq!(linear_color(0x000000), LinearColor::new(0.0, 0.0, 0.0));
        let white = linear_color(0xffffff);
        assert!((white.r - 1.0).abs() < 1e-6 && (white.g - 1.0).abs() < 1e-6);
        let mid = linear_color(0x808080);
        assert!((mid.r - 0.2158605).abs() < 1e-5);
        let red = linear_color(0xff0000);
        assert!(red.g == 0.0 && red.b == 0.0);
    }
}

use crate::types::Color;

/// Drone accent colors by level (clamped to the last entry).
pub const DRONE_ACCENT_COLORS: [Color; 7] = [0x9dffd7, 0xd8ff4f, 0xffe27a, 0xffad4f, 0xff8edb, 0xb58cff, 0x7fd7ff];

pub fn drone_accent_color(level: u32) -> Color {
    DRONE_ACCENT_COLORS[(level as usize).min(DRONE_ACCENT_COLORS.len() - 1)]
}

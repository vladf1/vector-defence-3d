//! Per-frame values shared by every 3D view, plus small allocation-free helpers.
use crate::math::Vec3;

/// Per-frame values shared by every 3D view.
#[derive(Clone, Copy, Debug)]
pub struct FrameContext {
    /// Presentation seconds since the previous frame; 0 while the simulation is frozen.
    pub delta_seconds: f32,
    /// Presentation clock (simulated seconds). Kept in `f64` so long sessions stay smooth.
    pub time: f64,
    pub frame: u32,
    /// Normalized world-space camera forward vector, for camera-facing ribbons.
    pub view_direction: Vec3,
}

/// Minimal mutable quaternion used to compose instance rotations without allocations.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quat {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Default for Quat {
    fn default() -> Self {
        Quat::IDENTITY
    }
}

impl Quat {
    pub const IDENTITY: Quat = Quat { x: 0.0, y: 0.0, z: 0.0, w: 1.0 };

    pub fn set(&mut self, x: f32, y: f32, z: f32, w: f32) -> &mut Self {
        *self = Quat { x, y, z, w };
        self
    }

    /// Rotation about world up; pass `-field_angle` to face a 2D heading.
    pub fn set_yaw(&mut self, angle: f32) -> &mut Self {
        let (sin, cos) = (angle / 2.0).sin_cos();
        self.set(0.0, sin, 0.0, cos)
    }

    pub fn set_axis_angle(&mut self, axis_x: f32, axis_y: f32, axis_z: f32, angle: f32) -> &mut Self {
        let (s, cos) = (angle / 2.0).sin_cos();
        self.set(axis_x * s, axis_y * s, axis_z * s, cos)
    }

    /// self = self * other
    pub fn multiply(&mut self, other: &Quat) -> &mut Self {
        self.multiply_components(other.x, other.y, other.z, other.w)
    }

    pub fn multiply_components(&mut self, bx: f32, by: f32, bz: f32, bw: f32) -> &mut Self {
        let Quat { x: ax, y: ay, z: az, w: aw } = *self;
        self.set(
            aw * bx + ax * bw + ay * bz - az * by,
            aw * by - ax * bz + ay * bw + az * bx,
            aw * bz + ax * by - ay * bx + az * bw,
            aw * bw - ax * bx - ay * by - az * bz,
        )
    }

    /// self = self * rotationAboutLocalZ(angle); pitches a +X-forward model nose-up.
    pub fn pitch(&mut self, angle: f32) -> &mut Self {
        let (sin, cos) = (angle / 2.0).sin_cos();
        self.multiply_components(0.0, 0.0, sin, cos)
    }

    /// self = self * rotationAboutLocalX(angle); banks a +X-forward model.
    pub fn roll(&mut self, angle: f32) -> &mut Self {
        let (sin, cos) = (angle / 2.0).sin_cos();
        self.multiply_components(sin, 0.0, 0.0, cos)
    }

    /// Heading from a 2D field angle, then pitch and roll in the model's local frame.
    pub fn set_heading_pitch_roll(&mut self, field_angle: f32, pitch_angle: f32, roll_angle: f32) -> &mut Self {
        self.set_yaw(-field_angle).pitch(pitch_angle).roll(roll_angle)
    }
}

pub fn smooth_towards(current: f32, target: f32, rate_per_second: f32, delta_seconds: f32) -> f32 {
    target + (current - target) * (-rate_per_second * delta_seconds).exp()
}

/// Cheap deterministic 0..1 hash for per-entity animation phase offsets (evaluated in `f64`,
/// since the large sine multiplier needs the precision).
pub fn hash01(seed: f64) -> f32 {
    let value = (seed * 12.9898).sin() * 43758.5453;
    (value - value.floor()) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heading_pitch_roll_composes_like_the_original() {
        let mut quat = Quat::IDENTITY;
        quat.set_heading_pitch_roll(0.5, 0.2, -0.3);
        let length = (quat.x * quat.x + quat.y * quat.y + quat.z * quat.z + quat.w * quat.w).sqrt();
        assert!((length - 1.0).abs() < 1e-5);
        let mut yaw_only = Quat::IDENTITY;
        yaw_only.set_heading_pitch_roll(0.5, 0.0, 0.0);
        assert!((yaw_only.y - (-0.25f32).sin()).abs() < 1e-6);
    }

    #[test]
    fn hash_stays_in_unit_range() {
        for index in 0..500 {
            let value = hash01(index as f64 * 3.1);
            assert!((0.0..1.0).contains(&value));
        }
    }
}

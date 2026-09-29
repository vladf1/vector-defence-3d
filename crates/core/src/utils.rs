use std::f64::consts::PI;

use crate::types::{FieldBounds, Point};

pub use crate::rng::{random, random_range};

/// Predicts the earliest reachable intercept; aim at the current position if none exists.
pub fn calculate_intercept(
    target: Point,
    velocity_x_per_second: f64,
    velocity_y_per_second: f64,
    projectile_speed_per_second: f64,
    from: Point,
) -> Point {
    let dx = target.x - from.x;
    let dy = target.y - from.y;
    let a = projectile_speed_per_second.powi(2) - velocity_x_per_second.powi(2) - velocity_y_per_second.powi(2);
    let b = dx * velocity_x_per_second + dy * velocity_y_per_second;
    let c = dx * dx + dy * dy;
    let discriminant = b * b + a * c;
    // Rationalizing the smaller positive root also handles equal projectile/target speeds.
    let denominator = if discriminant >= 0.0 { discriminant.sqrt() - b } else { 0.0 };
    let time_seconds = if denominator > 0.0 { c / denominator } else { 0.0 };
    Point::new(target.x + velocity_x_per_second * time_seconds, target.y + velocity_y_per_second * time_seconds)
}

/// A point with a velocity, advanced by `CalibratedExponentialDecay`.
pub struct MovingPoint<'a> {
    pub x: &'a mut f64,
    pub y: &'a mut f64,
    pub velocity_x_per_second: &'a mut f64,
    pub velocity_y_per_second: &'a mut f64,
}

/// Converts legacy per-update linear slowdown into continuous decay while preserving its
/// velocity and displacement at the chosen reference rate.
#[derive(Clone, Debug)]
pub struct CalibratedExponentialDecay {
    decay_rate_per_second: f64,
    displacement_scale: f64,
}

impl CalibratedExponentialDecay {
    pub fn new(linear_slowdown_per_second: f64, reference_updates_per_second: f64) -> Self {
        assert!(
            linear_slowdown_per_second.is_finite()
                && reference_updates_per_second.is_finite()
                && linear_slowdown_per_second >= 0.0
                && reference_updates_per_second > 0.0
                && linear_slowdown_per_second < reference_updates_per_second,
            "Velocity decay requires 0 <= slowdown < reference update rate."
        );
        if linear_slowdown_per_second == 0.0 {
            return CalibratedExponentialDecay { decay_rate_per_second: 0.0, displacement_scale: 1.0 };
        }
        let reference_delta_seconds = 1.0 / reference_updates_per_second;
        let reference_velocity_factor = 1.0 - linear_slowdown_per_second * reference_delta_seconds;
        let decay_rate_per_second = -reference_velocity_factor.ln() * reference_updates_per_second;
        let displacement_scale = (reference_velocity_factor * reference_delta_seconds * decay_rate_per_second)
            / (1.0 - reference_velocity_factor);
        CalibratedExponentialDecay { decay_rate_per_second, displacement_scale }
    }

    /// `(displacement seconds, velocity factor)` for one step.
    pub fn step_factors(&self, delta_seconds: f64) -> (f64, f64) {
        if self.decay_rate_per_second == 0.0 {
            return (delta_seconds, 1.0);
        }
        let velocity_factor = (-self.decay_rate_per_second * delta_seconds).exp();
        let displacement_seconds = self.displacement_scale * (1.0 - velocity_factor) / self.decay_rate_per_second;
        (displacement_seconds, velocity_factor)
    }

    pub fn apply(&self, target: MovingPoint<'_>, delta_seconds: f64) {
        if delta_seconds <= 0.0 {
            return;
        }
        let (displacement_seconds, velocity_factor) = self.step_factors(delta_seconds);
        let velocity_x = *target.velocity_x_per_second;
        let velocity_y = *target.velocity_y_per_second;
        *target.x += velocity_x * displacement_seconds;
        *target.y += velocity_y * displacement_seconds;
        *target.velocity_x_per_second = velocity_x * velocity_factor;
        *target.velocity_y_per_second = velocity_y * velocity_factor;
    }
}

/// Like `Math.min(max, Math.max(min, value))` (never panics on inverted bounds).
pub fn clamp(value: f64, min: f64, max: f64) -> f64 {
    max.min(min.max(value))
}

pub fn ease_in_out_cubic(progress: f64) -> f64 {
    let clamped = clamp(progress, 0.0, 1.0);
    if clamped < 0.5 { 4.0 * clamped * clamped * clamped } else { 1.0 - (-2.0 * clamped + 2.0).powi(3) / 2.0 }
}

pub fn ease_in_out_sine(progress: f64) -> f64 {
    let clamped = clamp(progress, 0.0, 1.0);
    -((PI * clamped).cos() - 1.0) / 2.0
}

pub fn ease_out_cubic(progress: f64) -> f64 {
    let clamped = clamp(progress, 0.0, 1.0);
    1.0 - (1.0 - clamped).powi(3)
}

pub fn calculate_distance(source: Point, target: Point) -> f64 {
    (source.x - target.x).hypot(source.y - target.y)
}

pub fn within_distance(source: Point, target: Point, max_distance: f64) -> bool {
    let dx = target.x - source.x;
    if dx.abs() > max_distance {
        return false;
    }
    let dy = target.y - source.y;
    if dy.abs() > max_distance {
        return false;
    }
    dx * dx + dy * dy <= max_distance * max_distance
}

pub fn is_outside_bounds(x: f64, y: f64, bounds: &FieldBounds, margin: f64) -> bool {
    x < bounds.min_x - margin || y < bounds.min_y - margin || x > bounds.max_x + margin || y > bounds.max_y + margin
}

pub fn angle_between(source: Point, target: Point) -> f64 {
    (target.y - source.y).atan2(target.x - source.x)
}

pub fn normalize_angle(angle: f64) -> f64 {
    angle.sin().atan2(angle.cos())
}

pub fn turn_angle_towards(current: f64, target: f64, max_step: f64) -> f64 {
    let delta = normalize_angle(target - current);
    let step = clamp(delta, -max_step, max_step);
    normalize_angle(current + step)
}

pub fn closest_point_on_segment(point: Point, start: Point, end: Point) -> Point {
    let segment_x = end.x - start.x;
    let segment_y = end.y - start.y;
    let length_squared = segment_x * segment_x + segment_y * segment_y;
    if length_squared == 0.0 {
        return start;
    }
    let projection = project_point_onto_segment(point, start, segment_x, segment_y, length_squared);
    Point::new(start.x + projection * segment_x, start.y + projection * segment_y)
}

pub fn is_within_distance_to_segment(point: Point, start: Point, end: Point, max_distance: f64) -> bool {
    if point.x < start.x.min(end.x) - max_distance
        || point.x > start.x.max(end.x) + max_distance
        || point.y < start.y.min(end.y) - max_distance
        || point.y > start.y.max(end.y) + max_distance
    {
        return false;
    }
    let segment_x = end.x - start.x;
    let segment_y = end.y - start.y;
    let length_squared = segment_x * segment_x + segment_y * segment_y;
    if length_squared == 0.0 {
        return within_distance(point, start, max_distance);
    }
    let projection = project_point_onto_segment(point, start, segment_x, segment_y, length_squared);
    let distance_x = start.x + projection * segment_x - point.x;
    let distance_y = start.y + projection * segment_y - point.y;
    distance_x * distance_x + distance_y * distance_y <= max_distance * max_distance
}

fn project_point_onto_segment(point: Point, start: Point, segment_x: f64, segment_y: f64, length_squared: f64) -> f64 {
    let dot = (point.x - start.x) * segment_x + (point.y - start.y) * segment_y;
    clamp(dot / length_squared, 0.0, 1.0)
}

/// `$N` with JavaScript `Math.round` (half rounds up).
pub fn format_money(value: f64) -> String {
    format!("${}", js_round(value) as i64)
}

/// JavaScript `Math.round`: halves round toward positive infinity.
pub fn js_round(value: f64) -> f64 {
    (value + 0.5).floor()
}

/// Removes entries whose `removed` flag is set, keeping order (like `compactInPlace`).
pub fn compact_in_place<T>(items: &mut Vec<T>, removed: impl Fn(&T) -> bool) {
    items.retain(|item| !removed(item));
}

//! A piercing beam that melts lines of enemies. The beam fades after each shot, and its damage
//! is integrated analytically over the fade so it does not depend on the step size.
use std::f64::consts::PI;

use crate::audio::AudioCue;
use crate::combat_effects::create_laser_impact_particles;
use crate::entities::towers::tower::{DEFAULT_FIRING_ANGLE_TOLERANCE, Tower};
use crate::types::{Color, Point};
use crate::update::{UpdateContext, UpdateResult};
use crate::utils::{
    angle_between, clamp, is_within_distance_to_segment, random_range, turn_angle_towards, within_distance,
};

/// Colors by level: body, accent, and the beam (the TS `"r, g, b"` strings as `0xRRGGBB`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LaserColors {
    pub body: Color,
    pub accent: Color,
    pub beam: Color,
}

pub const LASER_COLORS: [LaserColors; 7] = [
    LaserColors { body: 0x5bf4ff, accent: 0x9dffd7, beam: 0x6eff98 },
    LaserColors { body: 0x6dff9c, accent: 0xd8ff4f, beam: 0xb9ff69 },
    LaserColors { body: 0xffe36f, accent: 0xff9d5c, beam: 0xffe36f },
    LaserColors { body: 0xffad4f, accent: 0xff6d8c, beam: 0xff9d5c },
    LaserColors { body: 0xff8edb, accent: 0xb58cff, beam: 0xff8edb },
    LaserColors { body: 0xb58cff, accent: 0x78a7ff, beam: 0xb58cff },
    LaserColors { body: 0x4f8cff, accent: 0xf6f0ff, beam: 0x4f8cff },
];
const BEAM_FADE_PER_SECOND: f64 = 0.9;
/// The beam endpoint is this far along the tower heading.
pub const LASER_BEAM_REACH: f64 = 1000.0;
const FIRE_INTERVAL_SECONDS: f64 = 1.5;
const SPARK_INTERVAL_SECONDS: f64 = 0.055;

pub fn laser_colors(level: u32) -> &'static LaserColors {
    &LASER_COLORS[(level as usize).min(LASER_COLORS.len() - 1)]
}

pub fn get_muzzle_offset(level: u32) -> f64 {
    8.5 + level as f64 * 0.78
}

#[derive(Clone, Debug)]
pub struct LaserState {
    pub angle: f64,
    /// 1 right after firing, fading to 0; the beam deals damage while it is lit.
    pub beam_alpha: f64,
    pub beam_target: Point,
    pub damage_per_second: f64,
    pub direction_locked: bool,
    pub laser_spark_cooldown_seconds: f64,
    pub turn_speed_per_second: f64,
}

impl LaserState {
    pub(crate) fn new() -> LaserState {
        LaserState {
            angle: random_range(-PI, PI),
            beam_alpha: 0.0,
            beam_target: Point::new(0.0, 0.0),
            damage_per_second: 60.0,
            direction_locked: false,
            laser_spark_cooldown_seconds: 0.0,
            turn_speed_per_second: 6.72,
        }
    }

    pub(crate) fn on_upgrade(&mut self, level: u32) {
        self.damage_per_second = 60.0 + 15.0 * level as f64;
    }

    pub fn toggle_direction_lock(&mut self) {
        self.direction_locked = !self.direction_locked;
    }

    pub(crate) fn update(&mut self, tower: &mut Tower, context: &UpdateContext, result: &mut UpdateResult) {
        self.laser_spark_cooldown_seconds = (self.laser_spark_cooldown_seconds - context.delta_seconds).max(0.0);

        let mut aligned_to_target = false;
        if !self.direction_locked
            && let Some(tracked) = tower.find_tracked_monster(context)
        {
            let tracked_point = {
                let tracked = tracked.borrow();
                Point::new(tracked.x, tracked.y)
            };
            let target_angle = angle_between(tower.position(), tracked_point);
            self.angle =
                turn_angle_towards(self.angle, target_angle, self.turn_speed_per_second * context.delta_seconds);
            aligned_to_target = Tower::is_aimed_at_target(self.angle, target_angle, DEFAULT_FIRING_ANGLE_TOLERANCE);
        }

        let direction_x = self.angle.cos();
        let direction_y = self.angle.sin();
        let muzzle_offset = get_muzzle_offset(tower.level);
        let source = Point::new(tower.x + direction_x * muzzle_offset, tower.y + direction_y * muzzle_offset);
        self.beam_target =
            Point::new(tower.x + direction_x * LASER_BEAM_REACH, tower.y + direction_y * LASER_BEAM_REACH);
        let should_fire =
            if self.direction_locked { self.has_monster_in_beam(tower, context, source) } else { aligned_to_target };
        if tower.ready() && should_fire {
            self.beam_alpha = 1.0;
            tower.reset_cooldown(FIRE_INTERVAL_SECONDS);
            result.play_sound(AudioCue::LaserFire, Some(tower.x), Some(0.9 + tower.level as f64 * 0.1));
        }

        let integrated_beam_strength_seconds = self.advance_beam(context.delta_seconds);
        if integrated_beam_strength_seconds <= 0.0 {
            return;
        }

        let should_create_sparks = self.laser_spark_cooldown_seconds <= 0.0 && result.remaining_particle_capacity() > 0;
        let mut spark_bursts_created = 0;
        let colors = laser_colors(tower.level);
        // The beam is fixed during this update; reuse its geometry and each hit's spark position.
        let min_x = source.x.min(self.beam_target.x);
        let max_x = source.x.max(self.beam_target.x);
        let min_y = source.y.min(self.beam_target.y);
        let max_y = source.y.max(self.beam_target.y);
        let beam_x = self.beam_target.x - source.x;
        let beam_y = self.beam_target.y - source.y;
        let inverse_length_squared = 1.0 / (beam_x * beam_x + beam_y * beam_y);

        for monster in context.active_monsters {
            let mut monster = monster.borrow_mut();
            if !monster.is_active() {
                continue;
            }
            if monster.x < min_x - monster.radius
                || monster.x > max_x + monster.radius
                || monster.y < min_y - monster.radius
                || monster.y > max_y + monster.radius
            {
                continue;
            }
            let dot = (monster.x - source.x) * beam_x + (monster.y - source.y) * beam_y;
            let projection = clamp(dot * inverse_length_squared, 0.0, 1.0);
            let impact_x = source.x + projection * beam_x;
            let impact_y = source.y + projection * beam_y;
            let dx = monster.x - impact_x;
            let dy = monster.y - impact_y;
            if dx * dx + dy * dy <= monster.radius * monster.radius {
                monster.take_continuous_damage(self.damage_per_second * integrated_beam_strength_seconds);
                if should_create_sparks && spark_bursts_created < 2 {
                    let particles = create_laser_impact_particles(
                        impact_x,
                        impact_y,
                        self.angle,
                        colors.accent,
                        result.remaining_particle_capacity(),
                    );
                    result.add_particles(particles);
                    spark_bursts_created += 1;
                }
            }
        }

        if spark_bursts_created > 0 {
            self.laser_spark_cooldown_seconds = SPARK_INTERVAL_SECONDS;
        }
    }

    /// Fades the beam and returns its alpha integrated over the step (in beam-seconds).
    fn advance_beam(&mut self, delta_seconds: f64) -> f64 {
        if self.beam_alpha <= 0.0 || delta_seconds <= 0.0 {
            return 0.0;
        }
        let starting_alpha = self.beam_alpha;
        let active_seconds = delta_seconds.min(starting_alpha / BEAM_FADE_PER_SECOND);
        let ending_alpha = (starting_alpha - BEAM_FADE_PER_SECOND * active_seconds).max(0.0);
        self.beam_alpha = ending_alpha;
        active_seconds * ((starting_alpha + ending_alpha) / 2.0)
    }

    fn has_monster_in_beam(&self, tower: &Tower, context: &UpdateContext, source: Point) -> bool {
        for monster in context.active_monsters {
            let monster = monster.borrow();
            if !monster.is_active() {
                continue;
            }
            let position = Point::new(monster.x, monster.y);
            if !within_distance(tower.position(), position, tower.range) {
                continue;
            }
            if is_within_distance_to_segment(position, source, self.beam_target, monster.radius) {
                return true;
            }
        }
        false
    }
}

use std::f64::consts::PI;

use crate::audio::AudioCue;
use crate::entities::monsters::death_effect_helpers::{create_death_effect_origin, create_polygon_shard_particles};
use crate::entities::monsters::monster::{Monster, MonsterSpecial};
use crate::entities::monsters::polygon_shard_splitter::{PolygonShardSplitter, PolygonShardSplitterConfig};
use crate::route_path::SharedPath;
use crate::types::{Color, Point};
use crate::update::UpdateResult;
use crate::utils::{ease_in_out_sine, random_range};

pub const COLOR: Color = 0xffba4f;
const SPEED_PER_SECOND: f64 = 95.0;
const HIT_POINTS: f64 = 132.0;
const BOUNTY: i32 = 3;
const RADIUS: f64 = 7.0;
pub const OUTLINE_RADIUS: f64 = 7.0;
const NOSE_WOBBLE_INTERVAL_MIN_SECONDS: f64 = 2.0;
const NOSE_WOBBLE_INTERVAL_MAX_SECONDS: f64 = 5.0;
const NOSE_WOBBLE_DURATION_SECONDS: f64 = 1.575;
const NOSE_WOBBLE_MAX_ANGLE: f64 = PI * 0.14;
const NOSE_WOBBLE_OSCILLATIONS: f64 = 7.0;
const NOSE_WOBBLE_DECAY: f64 = 0.72;
pub const OUTLINE: [Point; 3] = [
    Point::new(OUTLINE_RADIUS, 0.0),
    Point::new(-OUTLINE_RADIUS, -OUTLINE_RADIUS),
    Point::new(-OUTLINE_RADIUS, OUTLINE_RADIUS),
];
const SHARD_SPLITTER: PolygonShardSplitter =
    PolygonShardSplitter::new(PolygonShardSplitterConfig::with_shard_counts(5, 11));

#[derive(Clone, Debug)]
pub struct TriangleState {
    pub nose_wobble_angle: f64,
    nose_wobble_elapsed_seconds: f64,
    nose_wobble_direction: f64,
    seconds_until_nose_wobble: f64,
}

pub(crate) fn create(path: SharedPath, speed_scale: f64) -> Monster {
    let mut monster = Monster::base(path, COLOR, SPEED_PER_SECOND * speed_scale, HIT_POINTS, BOUNTY, RADIUS);
    monster.special = MonsterSpecial::Triangle(TriangleState {
        nose_wobble_angle: 0.0,
        nose_wobble_elapsed_seconds: 0.0,
        nose_wobble_direction: 1.0,
        seconds_until_nose_wobble: random_range(NOSE_WOBBLE_INTERVAL_MIN_SECONDS, NOSE_WOBBLE_INTERVAL_MAX_SECONDS),
    });
    monster
}

impl TriangleState {
    pub(crate) fn update(&mut self, delta_seconds: f64) {
        if self.nose_wobble_elapsed_seconds > 0.0 {
            self.advance_nose_wobble(delta_seconds);
            return;
        }
        self.seconds_until_nose_wobble -= delta_seconds;
        if self.seconds_until_nose_wobble <= 0.0 {
            self.nose_wobble_direction = if random_range(0.0, 1.0) < 0.5 { -1.0 } else { 1.0 };
            self.advance_nose_wobble(delta_seconds);
        }
    }

    pub(crate) fn add_death_effect(&self, monster: &Monster, result: &mut UpdateResult) {
        let origin = create_death_effect_origin(OUTLINE_RADIUS, -0.1, 0.14, -0.12, 0.12);
        create_polygon_shard_particles(
            result,
            monster.shard_source(),
            &OUTLINE,
            origin,
            monster.angle + self.nose_wobble_angle,
            115.0,
            195.0,
            1.4,
            &SHARD_SPLITTER,
        );
        result.play_sound(AudioCue::MonsterShatter, Some(monster.x), None);
    }

    fn advance_nose_wobble(&mut self, delta_seconds: f64) {
        self.nose_wobble_elapsed_seconds += delta_seconds;
        let progress = (self.nose_wobble_elapsed_seconds / NOSE_WOBBLE_DURATION_SECONDS).min(1.0);
        let eased_start = ease_in_out_sine((progress * 4.0).min(1.0));
        let eased_amplitude = eased_start * (1.0 - progress * NOSE_WOBBLE_DECAY);
        let oscillation = (progress * PI * 2.0 * NOSE_WOBBLE_OSCILLATIONS).sin();
        self.nose_wobble_angle = self.nose_wobble_direction * NOSE_WOBBLE_MAX_ANGLE * oscillation * eased_amplitude;
        if progress == 1.0 {
            self.nose_wobble_angle = 0.0;
            self.nose_wobble_elapsed_seconds = 0.0;
            self.seconds_until_nose_wobble =
                random_range(NOSE_WOBBLE_INTERVAL_MIN_SECONDS, NOSE_WOBBLE_INTERVAL_MAX_SECONDS);
        }
    }
}

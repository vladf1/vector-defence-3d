use crate::audio::AudioCue;
use crate::entities::monsters::death_effect_helpers::{create_death_effect_origin, create_polygon_shard_particles};
use crate::entities::monsters::monster::{Monster, MonsterSpecial};
use crate::entities::monsters::polygon_shard_splitter::{PolygonShardSplitter, PolygonShardSplitterConfig};
use crate::route_path::SharedPath;
use crate::types::{Color, Point};
use crate::update::UpdateResult;
use crate::utils::{ease_in_out_sine, random_range};

pub const COLOR: Color = 0xff6f62;
const SPEED_PER_SECOND: f64 = 68.0;
const HIT_POINTS: f64 = 198.0;
const BOUNTY: i32 = 3;
const RADIUS: f64 = 6.5;
const SIZE_PULSE_DURATION_SECONDS: f64 = 1.25;
const SIZE_PULSE_MIN_SCALE: f64 = 0.95;
const SIZE_PULSE_MAX_SCALE: f64 = 1.1;
const SHARD_SPLITTER: PolygonShardSplitter =
    PolygonShardSplitter::new(PolygonShardSplitterConfig::with_shard_counts(5, 11));

#[derive(Clone, Debug)]
pub struct SquareState {
    size_pulse_elapsed_seconds: f64,
}

pub(crate) fn create(path: SharedPath, speed_scale: f64) -> Monster {
    let mut monster = Monster::base(path, COLOR, SPEED_PER_SECOND * speed_scale, HIT_POINTS, BOUNTY, RADIUS);
    monster.special = MonsterSpecial::Square(SquareState {
        size_pulse_elapsed_seconds: random_range(0.0, SIZE_PULSE_DURATION_SECONDS),
    });
    monster
}

impl SquareState {
    pub(crate) fn update(&mut self, monster: &mut Monster, delta_seconds: f64) {
        monster.rotation += 4.2 * delta_seconds;
        self.size_pulse_elapsed_seconds =
            (self.size_pulse_elapsed_seconds + delta_seconds) % SIZE_PULSE_DURATION_SECONDS;
    }

    pub(crate) fn add_death_effect(&self, monster: &Monster, result: &mut UpdateResult) {
        let visual_radius = self.visual_radius(monster.radius);
        let outline = create_outline(visual_radius);
        let origin = create_death_effect_origin(visual_radius, -0.24, 0.24, -0.24, 0.24);
        create_polygon_shard_particles(
            result,
            monster.shard_source(),
            &outline,
            origin,
            monster.rotation,
            128.0,
            233.0,
            1.2,
            &SHARD_SPLITTER,
        );
        result.play_sound(AudioCue::MonsterShatter, Some(monster.x), None);
    }

    pub fn visual_radius(&self, radius: f64) -> f64 {
        let progress = self.size_pulse_elapsed_seconds / SIZE_PULSE_DURATION_SECONDS;
        let mirrored_progress = if progress <= 0.5 { progress * 2.0 } else { (1.0 - progress) * 2.0 };
        let eased_progress = ease_in_out_sine(mirrored_progress);
        let scale = SIZE_PULSE_MIN_SCALE + (SIZE_PULSE_MAX_SCALE - SIZE_PULSE_MIN_SCALE) * eased_progress;
        radius * scale
    }
}

fn create_outline(visual_radius: f64) -> [Point; 4] {
    [
        Point::new(-visual_radius, -visual_radius),
        Point::new(visual_radius, -visual_radius),
        Point::new(visual_radius, visual_radius),
        Point::new(-visual_radius, visual_radius),
    ]
}

use crate::audio::AudioCue;
use crate::entities::monsters::death_effect_helpers::{create_death_effect_origin, create_polygon_shard_particles};
use crate::entities::monsters::monster::{Monster, MonsterSpecial};
use crate::entities::monsters::polygon_shard_splitter::{PolygonShardSplitter, PolygonShardSplitterConfig};
use crate::route_path::SharedPath;
use crate::types::{Color, Point};
use crate::update::UpdateResult;
use crate::utils::{ease_in_out_sine, random_range};

pub const COLOR: Color = 0x91ff63;
const SPEED_PER_SECOND: f64 = 132.0;
const HIT_POINTS: f64 = 100.0;
const BOUNTY: i32 = 2;
pub const RADIUS: f64 = 5.5;
const DASH_PULSE_INTERVAL_MIN_SECONDS: f64 = 1.5;
const DASH_PULSE_INTERVAL_MAX_SECONDS: f64 = 5.0;
const DASH_PULSE_DURATION_SECONDS: f64 = 1.0;
const DASH_SPEED_MULTIPLIER: f64 = 1.15;
const DASH_PULSE_FRONT_STRETCH: f64 = RADIUS * 1.15;
const DASH_PULSE_TAIL_TUCK: f64 = RADIUS * 0.72;
pub const OUTLINE: [Point; 6] = [
    Point::new(RADIUS * 1.8, 0.0),
    Point::new(RADIUS * 0.28, -RADIUS * 0.86),
    Point::new(-RADIUS * 1.35, -RADIUS * 0.58),
    Point::new(-RADIUS * 0.92, 0.0),
    Point::new(-RADIUS * 1.35, RADIUS * 0.58),
    Point::new(RADIUS * 0.28, RADIUS * 0.86),
];
const SHARD_SPLITTER: PolygonShardSplitter =
    PolygonShardSplitter::new(PolygonShardSplitterConfig::with_shard_counts(3, 6));

#[derive(Clone, Debug)]
pub struct RunnerState {
    base_speed_per_second: f64,
    dash_pulse_elapsed_seconds: f64,
    dash_speed_active: bool,
    seconds_until_dash_pulse: f64,
}

pub(crate) fn create(path: SharedPath, speed_scale: f64) -> Monster {
    let base_speed_per_second = SPEED_PER_SECOND * speed_scale;
    let mut monster = Monster::base(path, COLOR, base_speed_per_second, HIT_POINTS, BOUNTY, RADIUS);
    monster.special = MonsterSpecial::Runner(RunnerState {
        base_speed_per_second,
        dash_pulse_elapsed_seconds: 0.0,
        dash_speed_active: false,
        seconds_until_dash_pulse: random_range(DASH_PULSE_INTERVAL_MIN_SECONDS, DASH_PULSE_INTERVAL_MAX_SECONDS),
    });
    monster
}

impl RunnerState {
    pub(crate) fn update(&mut self, monster: &mut Monster, delta_seconds: f64) {
        if self.dash_pulse_elapsed_seconds > 0.0 {
            self.advance_dash_pulse(monster, delta_seconds);
            return;
        }
        self.seconds_until_dash_pulse -= delta_seconds;
        if self.seconds_until_dash_pulse <= 0.0 {
            self.advance_dash_pulse(monster, delta_seconds);
        }
    }

    pub(crate) fn add_death_effect(&self, monster: &Monster, result: &mut UpdateResult) {
        let outline = create_runner_outline(self.dash_pulse());
        let origin = create_death_effect_origin(monster.radius, -0.35, 0.3, -0.16, 0.16);
        create_polygon_shard_particles(
            result,
            monster.shard_source(),
            &outline,
            origin,
            monster.angle,
            155.0,
            255.0,
            0.0,
            &SHARD_SPLITTER,
        );
        result.play_sound(AudioCue::MonsterPop, Some(monster.x), Some(0.85));
    }

    fn advance_dash_pulse(&mut self, monster: &mut Monster, delta_seconds: f64) {
        if self.dash_pulse_elapsed_seconds == 0.0 {
            self.start_dash_speed_boost(monster);
        }
        self.dash_pulse_elapsed_seconds += delta_seconds;
        let progress = (self.dash_pulse_elapsed_seconds / DASH_PULSE_DURATION_SECONDS).min(1.0);
        if progress == 1.0 {
            self.dash_pulse_elapsed_seconds = 0.0;
            self.stop_dash_speed_boost(monster);
            self.seconds_until_dash_pulse =
                random_range(DASH_PULSE_INTERVAL_MIN_SECONDS, DASH_PULSE_INTERVAL_MAX_SECONDS);
        }
    }

    /// The dash stretch, easing 0 to 1 and back over the pulse.
    pub fn dash_pulse(&self) -> f64 {
        if self.dash_pulse_elapsed_seconds <= 0.0 {
            return 0.0;
        }
        let progress = (self.dash_pulse_elapsed_seconds / DASH_PULSE_DURATION_SECONDS).min(1.0);
        let mirrored_progress = if progress <= 0.5 { progress * 2.0 } else { (1.0 - progress) * 2.0 };
        ease_in_out_sine(mirrored_progress)
    }

    fn start_dash_speed_boost(&mut self, monster: &mut Monster) {
        if self.dash_speed_active {
            return;
        }
        self.dash_speed_active = true;
        monster.max_speed_per_second = self.base_speed_per_second * DASH_SPEED_MULTIPLIER;
        monster.speed_per_second *= DASH_SPEED_MULTIPLIER;
        monster.set_velocity_from_angle();
    }

    fn stop_dash_speed_boost(&mut self, monster: &mut Monster) {
        if !self.dash_speed_active {
            return;
        }
        self.dash_speed_active = false;
        monster.max_speed_per_second = self.base_speed_per_second;
        monster.speed_per_second = self.base_speed_per_second.min(monster.speed_per_second / DASH_SPEED_MULTIPLIER);
        monster.set_velocity_from_angle();
    }
}

pub fn create_runner_outline(dash_pulse: f64) -> [Point; 6] {
    [
        Point::new(OUTLINE[0].x + DASH_PULSE_FRONT_STRETCH * dash_pulse, OUTLINE[0].y),
        OUTLINE[1],
        Point::new(OUTLINE[2].x + DASH_PULSE_TAIL_TUCK * dash_pulse, OUTLINE[2].y * (1.0 - dash_pulse * 0.28)),
        Point::new(OUTLINE[3].x + DASH_PULSE_TAIL_TUCK * dash_pulse, OUTLINE[3].y),
        Point::new(OUTLINE[4].x + DASH_PULSE_TAIL_TUCK * dash_pulse, OUTLINE[4].y * (1.0 - dash_pulse * 0.28)),
        OUTLINE[5],
    ]
}

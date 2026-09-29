//! Ramps speed (and shifts color) as it loses health.
use std::f64::consts::PI;

use crate::audio::AudioCue;
use crate::entities::monsters::death_effect_helpers::{create_death_effect_origin, create_polygon_shard_particles};
use crate::entities::monsters::monster::{Monster, MonsterSpecial};
use crate::entities::monsters::polygon_shard_splitter::{PolygonShardSplitter, PolygonShardSplitterConfig};
use crate::route_path::SharedPath;
use crate::types::{Color, Point};
use crate::update::UpdateResult;
use crate::utils::random_range;

pub const BASE_COLOR: Color = 0xff7a4f;
pub const ENRAGED_COLOR: Color = 0xff5a36;
pub const FRENZIED_COLOR: Color = 0xff3158;
const BASE_SPEED_PER_SECOND: f64 = 62.0;
const ENRAGED_SPEED_PER_SECOND: f64 = 100.0;
const FRENZIED_SPEED_PER_SECOND: f64 = 138.0;
const HIT_POINTS: f64 = 343.0;
const BOUNTY: i32 = 4;
pub const RADIUS: f64 = 8.0;
const RAGE_ANIMATION_DURATION_SECONDS: f64 = 1.05;
const BODY_SURGE_MIN_SCALE: f64 = 0.015;
const BODY_SURGE_MAX_SCALE: f64 = 0.085;
pub const OUTLINE: [Point; 8] = [
    Point::new(RADIUS * 1.55, 0.0),
    Point::new(RADIUS * 0.4, -RADIUS * 0.8),
    Point::new(-RADIUS * 0.1, -RADIUS * 1.08),
    Point::new(-RADIUS * 1.28, -RADIUS * 0.44),
    Point::new(-RADIUS * 0.72, 0.0),
    Point::new(-RADIUS * 1.28, RADIUS * 0.44),
    Point::new(-RADIUS * 0.1, RADIUS * 1.08),
    Point::new(RADIUS * 0.4, RADIUS * 0.8),
];
const SHARD_SPLITTER: PolygonShardSplitter =
    PolygonShardSplitter::new(PolygonShardSplitterConfig::with_shard_counts(5, 11));

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RageMotion {
    pub scale_x: f64,
    pub scale_y: f64,
    pub ember_alpha: f64,
}

#[derive(Clone, Debug)]
pub struct BerserkerState {
    speed_scale: f64,
    /// 0 calm, 1 enraged (at half health), 2 frenzied (at a fifth).
    pub rage_stage: u32,
    rage_animation_elapsed_seconds: f64,
}

pub(crate) fn create(path: SharedPath, speed_scale: f64) -> Monster {
    let mut monster = Monster::base(path, BASE_COLOR, BASE_SPEED_PER_SECOND * speed_scale, HIT_POINTS, BOUNTY, RADIUS);
    monster.special = MonsterSpecial::Berserker(BerserkerState {
        speed_scale,
        rage_stage: 0,
        rage_animation_elapsed_seconds: random_range(0.0, RAGE_ANIMATION_DURATION_SECONDS),
    });
    monster
}

impl BerserkerState {
    pub(crate) fn update(&mut self, monster: &mut Monster, delta_seconds: f64) {
        let next_stage = if monster.hit_points <= monster.max_hit_points * 0.2 {
            2
        } else if monster.hit_points <= monster.max_hit_points * 0.5 {
            1
        } else {
            0
        };
        if next_stage != self.rage_stage {
            self.rage_stage = next_stage;
            let burst_floor = self.stage_speed_per_second() * (0.72 + self.rage_stage as f64 * 0.08);
            monster.speed_per_second = monster.speed_per_second.max(burst_floor);
        }

        self.rage_animation_elapsed_seconds += delta_seconds;
        monster.max_speed_per_second = self.stage_speed_per_second();
        monster.color = self.stage_color();

        if monster.speed_per_second < monster.max_speed_per_second {
            monster.speed_per_second = monster.max_speed_per_second.min(
                monster.speed_per_second + (50.4 + self.rage_stage as f64 * 43.2) * self.speed_scale * delta_seconds,
            );
        } else if monster.speed_per_second > monster.max_speed_per_second {
            monster.speed_per_second = monster.max_speed_per_second;
        }
        monster.set_velocity_from_angle();
    }

    pub(crate) fn add_death_effect(&self, monster: &Monster, result: &mut UpdateResult) {
        let motion = self.rage_motion();
        let origin = create_death_effect_origin(monster.radius, -0.15, 0.22, -0.15, 0.15);
        let outline = OUTLINE.map(|point| transform_point(point, &motion));
        create_polygon_shard_particles(
            result,
            monster.shard_source(),
            &outline,
            transform_point(origin, &motion),
            monster.angle,
            140.0,
            230.0,
            0.0,
            &SHARD_SPLITTER,
        );
        result.play_sound(AudioCue::MonsterHeavyDeath, Some(monster.x), Some(1.05));
    }

    fn stage_color(&self) -> Color {
        match self.rage_stage {
            2 => FRENZIED_COLOR,
            1 => ENRAGED_COLOR,
            _ => BASE_COLOR,
        }
    }

    fn stage_speed_per_second(&self) -> f64 {
        match self.rage_stage {
            2 => FRENZIED_SPEED_PER_SECOND * self.speed_scale,
            1 => ENRAGED_SPEED_PER_SECOND * self.speed_scale,
            _ => BASE_SPEED_PER_SECOND * self.speed_scale,
        }
    }

    /// The breathing body surge, stronger at higher rage stages.
    pub fn rage_motion(&self) -> RageMotion {
        let stage_intensity = match self.rage_stage {
            2 => 1.0,
            1 => 0.68,
            _ => 0.36,
        };
        let cycle = self.rage_animation_elapsed_seconds / RAGE_ANIMATION_DURATION_SECONDS;
        let surge = (1.0 - (cycle * PI * 2.0).cos()) / 2.0;
        let scale_amount = BODY_SURGE_MIN_SCALE + (BODY_SURGE_MAX_SCALE - BODY_SURGE_MIN_SCALE) * stage_intensity;
        RageMotion {
            scale_x: 1.0 + surge * scale_amount,
            scale_y: 1.0 - surge * scale_amount * 0.45,
            ember_alpha: 0.24 + stage_intensity * 0.42,
        }
    }
}

fn transform_point(point: Point, motion: &RageMotion) -> Point {
    Point::new(point.x * motion.scale_x, point.y * motion.scale_y)
}

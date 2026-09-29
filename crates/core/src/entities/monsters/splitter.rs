//! Bursts into weakened runner children when killed (see `monster_factory`).
use std::f64::consts::PI;

use crate::audio::AudioCue;
use crate::entities::monsters::death_effect_helpers::{create_death_effect_origin, create_polygon_shard_particles};
use crate::entities::monsters::monster::{Monster, MonsterSpecial};
use crate::entities::monsters::polygon_shard_splitter::{PolygonShardSplitter, PolygonShardSplitterConfig};
use crate::route_path::SharedPath;
use crate::types::{Color, Point};
use crate::update::UpdateResult;

pub const COLOR: Color = 0xff8bd5;
const SPEED_PER_SECOND: f64 = 73.0;
const HIT_POINTS: f64 = 304.0;
const BOUNTY: i32 = 3;
pub const RADIUS: f64 = 8.5;
const SHARD_SPLITTER: PolygonShardSplitter =
    PolygonShardSplitter::new(PolygonShardSplitterConfig::with_shard_counts(5, 11));

/// A six-point star alternating long and short spokes.
pub fn outline() -> [Point; 6] {
    std::array::from_fn(|index| {
        let angle = (PI / 3.0) * index as f64;
        let radius = if index % 2 == 0 { RADIUS * 1.15 } else { RADIUS * 0.72 };
        Point::new(angle.cos() * radius, angle.sin() * radius)
    })
}

pub(crate) fn create(path: SharedPath, speed_scale: f64) -> Monster {
    let mut monster = Monster::base(path, COLOR, SPEED_PER_SECOND * speed_scale, HIT_POINTS, BOUNTY, RADIUS);
    monster.special = MonsterSpecial::Splitter;
    monster
}

pub(crate) fn update(monster: &mut Monster, delta_seconds: f64) {
    monster.rotation += 2.7 * delta_seconds;
}

pub(crate) fn add_death_effect(monster: &Monster, result: &mut UpdateResult) {
    let origin = create_death_effect_origin(monster.radius, -0.14, 0.14, -0.14, 0.14);
    create_polygon_shard_particles(
        result,
        monster.shard_source(),
        &outline(),
        origin,
        monster.rotation,
        110.0,
        185.0,
        0.0,
        &SHARD_SPLITTER,
    );
    result.play_sound(AudioCue::MonsterPop, Some(monster.x), Some(1.1));
}

//! Flat armor on discrete hits; continuous damage bypasses it (see `Monster::take_damage`).
use crate::audio::AudioCue;
use crate::entities::Particle;
use crate::entities::monsters::death_effect_helpers::{create_death_effect_origin, create_polygon_shard_particles};
use crate::entities::monsters::monster::{Monster, MonsterSpecial};
use crate::entities::monsters::polygon_shard_splitter::{PolygonShardSplitter, PolygonShardSplitterConfig};
use crate::route_path::SharedPath;
use crate::types::{Color, Point};
use crate::update::UpdateResult;
use crate::utils::random_range;

pub const COLOR: Color = 0x78d7ff;
const SPEED_PER_SECOND: f64 = 49.0;
const HIT_POINTS: f64 = 409.0;
const BOUNTY: i32 = 4;
pub const RADIUS: f64 = 9.5;
pub const ARMOR_PER_HIT: f64 = 3.5;
pub const MIN_CHIP_DAMAGE: f64 = 0.4;
const SHELL_HALF_HEIGHT: f64 = RADIUS * 0.8;
pub const SHELL_OUTLINE: [Point; 8] = [
    Point::new(RADIUS * 1.35, 0.0),
    Point::new(RADIUS * 0.82, -SHELL_HALF_HEIGHT),
    Point::new(-RADIUS * 0.2, -RADIUS * 0.98),
    Point::new(-RADIUS * 1.08, -SHELL_HALF_HEIGHT),
    Point::new(-RADIUS * 1.32, 0.0),
    Point::new(-RADIUS * 1.08, SHELL_HALF_HEIGHT),
    Point::new(-RADIUS * 0.2, RADIUS * 0.98),
    Point::new(RADIUS * 0.82, SHELL_HALF_HEIGHT),
];
pub const FRONT_PLATE_OUTLINE: [Point; 5] = [
    Point::new(RADIUS * 1.08, 0.0),
    Point::new(RADIUS * 0.76, -RADIUS * 0.28),
    Point::new(RADIUS * 0.16, -RADIUS * 0.28),
    Point::new(RADIUS * 0.16, RADIUS * 0.28),
    Point::new(RADIUS * 0.76, RADIUS * 0.28),
];
const SHARD_SPLITTER: PolygonShardSplitter =
    PolygonShardSplitter::new(PolygonShardSplitterConfig::with_shard_counts(5, 11));

#[derive(Clone, Debug)]
pub struct BulwarkState {
    pub shield_pulse: f64,
}

pub(crate) fn create(path: SharedPath, speed_scale: f64) -> Monster {
    let mut monster = Monster::base(path, COLOR, SPEED_PER_SECOND * speed_scale, HIT_POINTS, BOUNTY, RADIUS);
    monster.special = MonsterSpecial::Bulwark(BulwarkState { shield_pulse: 0.0 });
    monster
}

/// Flat armor applies once per discrete impact; continuous effects use
/// `Monster::take_continuous_damage` so their result cannot depend on tick rate.
pub(crate) fn mitigate(amount: f64) -> f64 {
    MIN_CHIP_DAMAGE.max(amount - ARMOR_PER_HIT)
}

impl BulwarkState {
    pub(crate) fn update(&mut self, delta_seconds: f64) {
        self.shield_pulse += 2.8 * delta_seconds;
    }
}

pub(crate) fn add_death_effect(monster: &Monster, result: &mut UpdateResult) {
    let origin = create_death_effect_origin(monster.radius, -0.18, 0.18, -0.18, 0.18);
    create_polygon_shard_particles(
        result,
        monster.shard_source(),
        &SHELL_OUTLINE,
        origin,
        monster.angle,
        120.0,
        205.0,
        0.0,
        &SHARD_SPLITTER,
    );
    if result.remaining_particle_capacity() > 0 {
        let speed = random_range(105.0, 175.0);
        result.add_particle(Particle::glass_shard(
            monster.visual_x(),
            monster.visual_y(),
            monster.color,
            FRONT_PLATE_OUTLINE.to_vec(),
            Point::new(0.0, 0.0),
            monster.angle,
            speed,
            0.0,
        ));
    }
    result.play_sound(AudioCue::MonsterHeavyDeath, Some(monster.x), Some(1.05));
}

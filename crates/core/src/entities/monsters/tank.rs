use std::f64::consts::PI;

use crate::audio::AudioCue;
use crate::entities::Particle;
use crate::entities::monsters::death_effect_helpers::{
    create_death_effect_origin, create_polygon_shard_particles, rotate_point,
};
use crate::entities::monsters::monster::{Monster, MonsterSpecial};
use crate::entities::monsters::polygon_shard_splitter::{PolygonShardSplitter, PolygonShardSplitterConfig};
use crate::route_path::SharedPath;
use crate::types::{Color, Point};
use crate::update::UpdateResult;
use crate::utils::{ease_in_out_cubic, random_range};

pub const COLOR: Color = 0x9fb6ff;
const SPEED_PER_SECOND: f64 = 41.0;
const HIT_POINTS: f64 = 554.0;
const BOUNTY: i32 = 6;
pub const RADIUS: f64 = 10.5;
const HULL_X: f64 = -RADIUS;
const HULL_Y: f64 = -RADIUS * 0.72;
const HULL_WIDTH: f64 = RADIUS * 2.1;
const HULL_HEIGHT: f64 = RADIUS * 1.44;
pub const HULL_OUTLINE: [Point; 4] = [
    Point::new(HULL_X, HULL_Y),
    Point::new(HULL_X + HULL_WIDTH, HULL_Y),
    Point::new(HULL_X + HULL_WIDTH, HULL_Y + HULL_HEIGHT),
    Point::new(HULL_X, HULL_Y + HULL_HEIGHT),
];
const SHARD_SPLITTER: PolygonShardSplitter =
    PolygonShardSplitter::new(PolygonShardSplitterConfig::with_shard_counts(6, 13));
const TURRET_SPIN_INTERVAL_MIN_SECONDS: f64 = 3.0;
const TURRET_SPIN_INTERVAL_MAX_SECONDS: f64 = 10.0;
const TURRET_SPIN_DURATION_SECONDS: f64 = 2.2;
const FULL_ROTATION: f64 = PI * 2.0;
const TRACK_PRINT_INTERVAL: f64 = RADIUS * 0.44;
const TRACK_PRINT_SIDE_OFFSET: f64 = RADIUS * 0.62;

#[derive(Clone, Debug)]
pub struct TankState {
    pub turret_rotation: f64,
    turret_spin_elapsed_seconds: f64,
    turret_spin_direction: f64,
    seconds_until_turret_spin: f64,
    last_left_track_print: Option<Point>,
    last_right_track_print: Option<Point>,
}

/// The turret sits slightly forward of the hull center.
pub fn get_tank_turret_center_offset_x(tank_radius: f64) -> f64 {
    tank_radius * 0.08
}

pub(crate) fn create(path: SharedPath, speed_scale: f64) -> Monster {
    let mut monster = Monster::base(path, COLOR, SPEED_PER_SECOND * speed_scale, HIT_POINTS, BOUNTY, RADIUS);
    monster.special = MonsterSpecial::Tank(TankState {
        turret_rotation: 0.0,
        turret_spin_elapsed_seconds: 0.0,
        turret_spin_direction: 1.0,
        seconds_until_turret_spin: random_range(TURRET_SPIN_INTERVAL_MIN_SECONDS, TURRET_SPIN_INTERVAL_MAX_SECONDS),
        last_left_track_print: None,
        last_right_track_print: None,
    });
    monster
}

impl TankState {
    pub(crate) fn update(&mut self, delta_seconds: f64) {
        if self.turret_spin_elapsed_seconds > 0.0 {
            self.advance_turret_spin(delta_seconds);
            return;
        }
        self.seconds_until_turret_spin -= delta_seconds;
        if self.seconds_until_turret_spin <= 0.0 {
            self.turret_spin_direction = if random_range(0.0, 1.0) < 0.5 { -1.0 } else { 1.0 };
            self.advance_turret_spin(delta_seconds);
        }
    }

    pub(crate) fn add_death_effect(&self, monster: &Monster, result: &mut UpdateResult) {
        let turret_center_offset =
            rotate_point(Point::new(get_tank_turret_center_offset_x(monster.radius), 0.0), monster.angle);
        let origin = create_death_effect_origin(monster.radius, -0.15, 0.35, -0.22, 0.22);
        create_polygon_shard_particles(
            result,
            monster.shard_source(),
            &HULL_OUTLINE,
            origin,
            monster.angle,
            125.0,
            220.0,
            0.0,
            &SHARD_SPLITTER,
        );
        if result.remaining_particle_capacity() > 0 {
            result.add_particle(Particle::tank_turret(
                monster.visual_x() + turret_center_offset.x,
                monster.visual_y() + turret_center_offset.y,
                monster.radius,
                monster.color,
                monster.angle,
                self.turret_rotation,
            ));
        }
        result.play_sound(AudioCue::MonsterHeavyDeath, Some(monster.x), Some(1.25));
    }

    fn advance_turret_spin(&mut self, delta_seconds: f64) {
        self.turret_spin_elapsed_seconds += delta_seconds;
        let progress = (self.turret_spin_elapsed_seconds / TURRET_SPIN_DURATION_SECONDS).min(1.0);
        self.turret_rotation = self.turret_spin_direction * FULL_ROTATION * ease_in_out_cubic(progress);
        if progress == 1.0 {
            self.turret_rotation = 0.0;
            self.turret_spin_elapsed_seconds = 0.0;
            self.seconds_until_turret_spin =
                random_range(TURRET_SPIN_INTERVAL_MIN_SECONDS, TURRET_SPIN_INTERVAL_MAX_SECONDS);
        }
    }
}

/// Leaves a print under each track every `TRACK_PRINT_INTERVAL` of travel.
pub(crate) fn add_track_prints(monster: &mut Monster, result: &mut UpdateResult) {
    let forward_x = monster.velocity_x_per_second / monster.speed_per_second;
    let forward_y = monster.velocity_y_per_second / monster.speed_per_second;
    let side_x = -forward_y;
    let side_y = forward_x;
    let left = Point::new(monster.x - side_x * TRACK_PRINT_SIDE_OFFSET, monster.y - side_y * TRACK_PRINT_SIDE_OFFSET);
    let right = Point::new(monster.x + side_x * TRACK_PRINT_SIDE_OFFSET, monster.y + side_y * TRACK_PRINT_SIDE_OFFSET);
    let angle = monster.angle;
    let MonsterSpecial::Tank(state) = &mut monster.special else {
        return;
    };
    state.last_left_track_print = Some(add_track_print_if_ready(result, left, state.last_left_track_print, angle));
    state.last_right_track_print = Some(add_track_print_if_ready(result, right, state.last_right_track_print, angle));
}

fn add_track_print_if_ready(result: &mut UpdateResult, point: Point, last_point: Option<Point>, angle: f64) -> Point {
    match last_point {
        // Written as a negated `>=` so a NaN distance (a stopped tank) keeps the last print, like the TS.
        #[allow(clippy::neg_cmp_op_on_partial_ord)]
        Some(last) if !((point.x - last.x).hypot(point.y - last.y) >= TRACK_PRINT_INTERVAL) => last,
        _ => {
            if result.remaining_particle_capacity() > 0 {
                result.add_particle(Particle::tank_track_print(point.x, point.y, angle));
            }
            point
        }
    }
}

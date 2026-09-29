use std::f64::consts::PI;

use crate::audio::AudioCue;
use crate::entities::monsters::death_effect_helpers::{
    create_death_effect_origin, create_polygon_shard_particles, point_on_radius,
};
use crate::entities::monsters::monster::{Monster, MonsterSpecial};
use crate::entities::monsters::polygon_shard_splitter::{PolygonShardSplitter, PolygonShardSplitterConfig};
use crate::route_path::SharedPath;
use crate::types::{Color, Point};
use crate::update::UpdateResult;
use crate::utils::{ease_in_out_cubic, ease_in_out_sine, random_range};

pub const COLOR: Color = 0x5df2ef;
const SPEED_PER_SECOND: f64 = 81.0;
const HIT_POINTS: f64 = 264.0;
const BOUNTY: i32 = 2;
const RADIUS: f64 = 7.5;
pub const MOUTH_OPEN_ANGLE: f64 = PI * 0.18;
pub const MOUTH_CLOSED_ANGLE: f64 = PI * 0.035;
const IDLE_ANIMATION_INTERVAL_MIN_SECONDS: f64 = 2.0;
const IDLE_ANIMATION_INTERVAL_MAX_SECONDS: f64 = 5.0;
const MOUTH_ANIMATION_DURATION_SECONDS: f64 = 0.5;
const ROTATION_ANIMATION_DURATION_SECONDS: f64 = 0.9;
const FULL_ROTATION: f64 = PI * 2.0;
const SHARD_SPLITTER: PolygonShardSplitter = PolygonShardSplitter::new(PolygonShardSplitterConfig {
    max_shard_vertices: 26,
    ..PolygonShardSplitterConfig::with_shard_counts(5, 11)
});

/// Idle flourishes (a mouth snap or a full spin) never overlap.
#[derive(Clone, Debug)]
pub struct PackManState {
    pub mouth_angle: f64,
    mouth_animation_elapsed_seconds: f64,
    pub body_rotation: f64,
    body_rotation_direction: f64,
    rotation_animation_elapsed_seconds: f64,
    seconds_until_idle_animation: f64,
}

pub(crate) fn create(path: SharedPath, speed_scale: f64) -> Monster {
    let mut monster = Monster::base(path, COLOR, SPEED_PER_SECOND * speed_scale, HIT_POINTS, BOUNTY, RADIUS);
    monster.special = MonsterSpecial::PackMan(PackManState {
        mouth_angle: MOUTH_OPEN_ANGLE,
        mouth_animation_elapsed_seconds: 0.0,
        body_rotation: 0.0,
        body_rotation_direction: 1.0,
        rotation_animation_elapsed_seconds: 0.0,
        seconds_until_idle_animation: random_range(
            IDLE_ANIMATION_INTERVAL_MIN_SECONDS,
            IDLE_ANIMATION_INTERVAL_MAX_SECONDS,
        ),
    });
    monster
}

impl PackManState {
    pub(crate) fn update(&mut self, delta_seconds: f64) {
        if self.mouth_animation_elapsed_seconds > 0.0 {
            self.advance_mouth_animation(delta_seconds);
            return;
        }
        if self.rotation_animation_elapsed_seconds > 0.0 {
            self.advance_rotation_animation(delta_seconds);
            return;
        }
        self.seconds_until_idle_animation -= delta_seconds;
        if self.seconds_until_idle_animation <= 0.0 {
            self.start_idle_animation(delta_seconds);
        }
    }

    pub fn create_outline(&self, radius: f64, arc_vertex_count: usize) -> Vec<Point> {
        create_packman_outline(radius, self.mouth_angle, arc_vertex_count)
    }

    pub(crate) fn add_death_effect(&self, monster: &Monster, result: &mut UpdateResult) {
        let outline = self.create_outline(monster.radius, 18);
        let origin = create_death_effect_origin(monster.radius, -0.12, 0.12, -0.12, 0.12);
        create_polygon_shard_particles(
            result,
            monster.shard_source(),
            &outline,
            origin,
            monster.angle + self.body_rotation,
            125.0,
            205.0,
            0.0,
            &SHARD_SPLITTER,
        );
        result.play_sound(AudioCue::MonsterShatter, Some(monster.x), None);
    }

    fn advance_mouth_animation(&mut self, delta_seconds: f64) {
        self.mouth_animation_elapsed_seconds += delta_seconds;
        let progress = (self.mouth_animation_elapsed_seconds / MOUTH_ANIMATION_DURATION_SECONDS).min(1.0);
        self.mouth_angle = get_packman_mouth_angle(progress);
        if progress == 1.0 {
            self.mouth_angle = MOUTH_OPEN_ANGLE;
            self.mouth_animation_elapsed_seconds = 0.0;
            self.reset_idle_animation_interval();
        }
    }

    fn advance_rotation_animation(&mut self, delta_seconds: f64) {
        self.rotation_animation_elapsed_seconds += delta_seconds;
        let progress = (self.rotation_animation_elapsed_seconds / ROTATION_ANIMATION_DURATION_SECONDS).min(1.0);
        self.body_rotation = self.body_rotation_direction * FULL_ROTATION * ease_in_out_cubic(progress);
        if progress == 1.0 {
            self.body_rotation = 0.0;
            self.rotation_animation_elapsed_seconds = 0.0;
            self.reset_idle_animation_interval();
        }
    }

    fn reset_idle_animation_interval(&mut self) {
        self.seconds_until_idle_animation =
            random_range(IDLE_ANIMATION_INTERVAL_MIN_SECONDS, IDLE_ANIMATION_INTERVAL_MAX_SECONDS);
    }

    fn start_idle_animation(&mut self, delta_seconds: f64) {
        if random_range(0.0, 1.0) < 0.5 {
            self.advance_mouth_animation(delta_seconds);
            return;
        }
        self.body_rotation_direction = if random_range(0.0, 1.0) < 0.5 { -1.0 } else { 1.0 };
        self.advance_rotation_animation(delta_seconds);
    }
}

fn get_packman_mouth_angle(animation_progress: f64) -> f64 {
    let clamped_progress = animation_progress.clamp(0.0, 1.0);
    let snap_progress = if clamped_progress < 1.0 { (clamped_progress * 2.0) % 1.0 } else { 1.0 };
    let mirrored_progress = if snap_progress <= 0.5 { snap_progress * 2.0 } else { (1.0 - snap_progress) * 2.0 };
    let close_then_open = ease_in_out_sine(mirrored_progress);
    MOUTH_OPEN_ANGLE - (MOUTH_OPEN_ANGLE - MOUTH_CLOSED_ANGLE) * close_then_open
}

pub fn create_packman_outline(radius: f64, mouth_angle: f64, arc_vertex_count: usize) -> Vec<Point> {
    let body_sweep_angle = PI * 2.0 - mouth_angle * 2.0;
    let vertex_count = arc_vertex_count.max(2);
    let mut outline = vec![Point::new(0.0, 0.0)];
    for index in 0..vertex_count {
        let ratio = index as f64 / (vertex_count - 1) as f64;
        let angle = mouth_angle + body_sweep_angle * ratio;
        outline.push(point_on_radius(angle, radius));
    }
    outline
}

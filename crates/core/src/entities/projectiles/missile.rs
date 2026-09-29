//! Homing missiles with splash damage, plus the silhouette extents that set their collision radius.
use std::f64::consts::PI;
use std::rc::Rc;

use crate::audio::AudioCue;
use crate::collision::CircleSweep;
use crate::combat_effects::create_missile_explosion_particles;
use crate::entities::effects::particle::ParticleMotion;
use crate::entities::{MonsterRef, Particle, next_entity_id};
use crate::types::{Color, Point};
use crate::update::{UpdateContext, UpdateResult};
use crate::utils::{angle_between, calculate_distance, clamp, is_outside_bounds, random_range, turn_angle_towards};

const MISSILE_DAMAGE_BASE: f64 = 50.0;
const MISSILE_DAMAGE_PER_LEVEL: f64 = 4.0;
const MISSILE_EFFECT_RADIUS_BASE: f64 = 60.0;
const MISSILE_EFFECT_RADIUS_PER_LEVEL: f64 = 5.0;
const MISSILE_SPEED_BASE_PER_SECOND: f64 = 108.0;
const MISSILE_SPEED_PER_LEVEL_PER_SECOND: f64 = 30.0;
const MISSILE_HIT_SHAKE_MIN_DURATION_SECONDS: f64 = 0.08;
const MISSILE_HIT_SHAKE_DURATION_RANGE_SECONDS: f64 = 0.035;
const MISSILE_HIT_SHAKE_MIN_DISTANCE: f64 = 0.45;
const MISSILE_HIT_SHAKE_DISTANCE_RANGE: f64 = 0.8;
const MISSILE_TURN_SPEED_PER_SECOND: f64 = 7.2;
pub const MISSILE_LAUNCH_BLOOM_SECONDS: f64 = 0.28;
/// Missile trail spark colors; the renderer tells trail particles apart by these.
pub const MISSILE_TRAIL_HOT_COLOR: Color = 0xfff0a8;
pub const MISSILE_TRAIL_WARM_COLOR: Color = 0xff8f45;
pub const MISSILE_TRAIL_SMOKE_COLOR: Color = 0x7e858c;

const BASE_TAIL_X: f64 = -7.8;
const BASE_NOSE_TIP_X: f64 = 8.9;
const ROCKET_OFFSET_X: f64 = 1.8;
const TAIL_CAP_LENGTH: f64 = 2.4;
const MISSILE_LENGTH_SCALE_PER_LEVEL: f64 = 0.03;

/// Missile silhouette extents in unscaled missile space; they set the missile's collision radius.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MissileVisual {
    pub nose_tip_x: f64,
    pub tail_cap_left_x: f64,
}

pub fn get_missile_scale(level: u32) -> f64 {
    1.0 + 0.05 * level as f64
}

pub fn create_missile_visual(level: u32) -> MissileVisual {
    let level_scale = get_missile_scale(level);
    let coordinate_scale_x = (1.0 + MISSILE_LENGTH_SCALE_PER_LEVEL * level as f64) / level_scale;
    let scale_x = |x: f64| (x + ROCKET_OFFSET_X) * coordinate_scale_x;
    MissileVisual {
        nose_tip_x: scale_x(BASE_NOSE_TIP_X + 3.2),
        tail_cap_left_x: scale_x(BASE_TAIL_X - TAIL_CAP_LENGTH),
    }
}

pub fn get_missile_half_length(visual: &MissileVisual) -> f64 {
    visual.tail_cap_left_x.abs().max(visual.nose_tip_x.abs())
}

fn get_shake_strength_from_splash_ratio(ratio: f64) -> f64 {
    clamp(ratio, 0.15, 1.0)
}

#[derive(Clone, Debug)]
pub struct Missile {
    pub id: u32,
    pub x: f64,
    pub y: f64,
    pub previous_x: f64,
    pub previous_y: f64,
    pub radius: f64,
    pub angle: f64,
    pub speed_per_second: f64,
    pub damage: f64,
    pub effect_radius: f64,
    pub scale: f64,
    pub level: u32,
    pub tracked_monster: Option<MonsterRef>,
    pub removed: bool,
    pub trail_timer: f64,
    pub launch_bloom_seconds: f64,
}

impl Missile {
    /// `initial_angle: None` aims straight at the tracked monster.
    pub fn new(
        source: Point,
        tracked_monster: MonsterRef,
        level: u32,
        visual: &MissileVisual,
        initial_angle: Option<f64>,
    ) -> Missile {
        let scale = get_missile_scale(level);
        let angle = initial_angle.unwrap_or_else(|| {
            let monster = tracked_monster.borrow();
            angle_between(source, Point::new(monster.x, monster.y))
        });
        Missile {
            id: next_entity_id(),
            x: source.x,
            y: source.y,
            previous_x: source.x,
            previous_y: source.y,
            radius: get_missile_half_length(visual) * scale,
            angle,
            speed_per_second: MISSILE_SPEED_BASE_PER_SECOND + MISSILE_SPEED_PER_LEVEL_PER_SECOND * level as f64,
            damage: MISSILE_DAMAGE_BASE + MISSILE_DAMAGE_PER_LEVEL * level as f64,
            effect_radius: MISSILE_EFFECT_RADIUS_BASE + MISSILE_EFFECT_RADIUS_PER_LEVEL * level as f64,
            scale,
            level,
            tracked_monster: Some(tracked_monster),
            removed: false,
            trail_timer: 0.0,
            launch_bloom_seconds: MISSILE_LAUNCH_BLOOM_SECONDS,
        }
    }

    pub fn sweep(&self) -> CircleSweep {
        CircleSweep {
            previous_x: self.previous_x,
            previous_y: self.previous_y,
            x: self.x,
            y: self.y,
            radius: self.radius,
        }
    }

    pub fn update(&mut self, context: &UpdateContext, result: &mut UpdateResult) {
        let delta_seconds = context.delta_seconds;
        self.launch_bloom_seconds = (self.launch_bloom_seconds - delta_seconds).max(0.0);
        self.speed_per_second += 180.0 * delta_seconds;
        if self.tracked_monster.as_ref().is_some_and(|monster| monster.borrow().removed) {
            self.tracked_monster = None;
        }
        if let Some(monster) = &self.tracked_monster {
            let target = {
                let monster = monster.borrow();
                Point::new(monster.x, monster.y)
            };
            let target_angle = angle_between(Point::new(self.x, self.y), target);
            self.angle = turn_angle_towards(self.angle, target_angle, MISSILE_TURN_SPEED_PER_SECOND * delta_seconds);
        }

        self.previous_x = self.x;
        self.previous_y = self.y;
        self.x += self.angle.cos() * self.speed_per_second * delta_seconds;
        self.y += self.angle.sin() * self.speed_per_second * delta_seconds;

        if let Some((_, collision)) = context.find_earliest_collision(&self.sweep()) {
            self.x = collision.x;
            self.y = collision.y;
            self.explode(context, result);
            return;
        }

        self.add_trail(delta_seconds, result);
        if is_outside_bounds(self.x, self.y, &context.field_bounds, 20.0) {
            self.removed = true;
        }
    }

    pub fn is_tracking(&self, monster: &MonsterRef) -> bool {
        self.tracked_monster.as_ref().is_some_and(|tracked| Rc::ptr_eq(tracked, monster))
    }

    fn add_trail(&mut self, delta_seconds: f64, result: &mut UpdateResult) {
        self.trail_timer += delta_seconds;
        if self.trail_timer < 0.02 {
            return;
        }
        self.trail_timer -= 0.02;
        if result.remaining_particle_capacity() == 0 {
            return;
        }
        let trail_x = self.x + random_range(-3.0, 3.0) - self.angle.cos() * 9.0;
        let trail_y = self.y + random_range(-3.0, 3.0) - self.angle.sin() * 9.0;
        let exhaust_angle = self.angle + PI + random_range(-0.35, 0.35);

        let size = random_range(0.6, 1.2);
        let speed_per_second = random_range(36.0, 82.0);
        result.add_particle(Particle::spark(
            trail_x,
            trail_y,
            size,
            MISSILE_TRAIL_HOT_COLOR,
            5.5,
            ParticleMotion { speed_per_second, offset: 0.0, angle: Some(exhaust_angle) },
        ));
        let size = random_range(0.8, 1.5);
        let speed_per_second = random_range(28.0, 68.0);
        result.add_particle(Particle::spark(
            trail_x,
            trail_y,
            size,
            MISSILE_TRAIL_WARM_COLOR,
            3.8,
            ParticleMotion { speed_per_second, offset: 1.0, angle: Some(exhaust_angle) },
        ));
        let speed_per_second = random_range(22.0, 50.0);
        result.add_particle(Particle::spark(
            trail_x,
            trail_y,
            1.0,
            MISSILE_TRAIL_SMOKE_COLOR,
            1.4,
            ParticleMotion { speed_per_second, offset: 2.0, angle: Some(exhaust_angle) },
        ));
    }

    fn explode(&mut self, context: &UpdateContext, result: &mut UpdateResult) {
        self.removed = true;
        let particles = create_missile_explosion_particles(
            self.x,
            self.y,
            self.angle,
            self.level,
            result.remaining_particle_capacity(),
        );
        result.add_particles(particles);
        let center = Point::new(self.x, self.y);
        for nearby in context.active_monsters {
            let mut nearby = nearby.borrow_mut();
            if nearby.removed || nearby.hit_points <= 0.0 {
                continue;
            }
            let distance = calculate_distance(center, Point::new(nearby.x, nearby.y));
            if distance > self.effect_radius {
                continue;
            }
            let ratio = (self.effect_radius - distance) / self.effect_radius;
            let shake_strength = get_shake_strength_from_splash_ratio(ratio);
            nearby.shake(
                MISSILE_HIT_SHAKE_MIN_DURATION_SECONDS + MISSILE_HIT_SHAKE_DURATION_RANGE_SECONDS * shake_strength,
                MISSILE_HIT_SHAKE_MIN_DISTANCE + MISSILE_HIT_SHAKE_DISTANCE_RANGE * shake_strength,
            );
            nearby.take_damage(self.damage * ratio);
        }
        result.play_sound(AudioCue::MissileExplosion, Some(self.x), Some(1.1));
    }
}

//! Autonomous hunter drones: they pick targets (spreading out across monsters), orbit them at a
//! stand-off distance, keep apart from each other, fire lead shots, and fly off when they expire.
use std::f64::consts::PI;
use std::rc::Rc;

use crate::audio::AudioCue;
use crate::constants::TIMER_EPSILON_SECONDS;
use crate::entities::projectiles::projectile::{DRONE_PROJECTILE_SPEED_PER_SECOND, Projectile};
use crate::entities::{MonsterRef, next_entity_id};
use crate::types::Point;
use crate::update::{UpdateContext, UpdateResult};
use crate::utils::{
    calculate_distance, calculate_intercept, clamp, is_outside_bounds, random_range, turn_angle_towards,
    within_distance,
};

const DRONE_SPEED_BASE: f64 = 158.6;
const DRONE_SPEED_PER_LEVEL: f64 = 10.4;
const DRONE_LOITER_RADIUS: f64 = 25.0;
const DRONE_LOITER_SPEED_PER_SECOND: f64 = 1.65;
const DRONE_ARRIVE_DISTANCE: f64 = 6.0;
const DRONE_TARGET_STANDOFF_MIN: f64 = 34.0;
const DRONE_TARGET_ORBIT_RADIUS_MAX: f64 = 42.0;
const DRONE_TARGET_ORBIT_SPEED_MIN: f64 = 0.95;
const DRONE_TARGET_ORBIT_SPEED_MAX: f64 = 1.45;
const DRONE_TARGET_ORBIT_PHASE_JITTER: f64 = 0.7;
const DRONE_SEPARATION_DISTANCE: f64 = 24.0;
const DRONE_SEPARATION_SPEED_PER_SECOND: f64 = 96.0;
const DRONE_EXIT_SPEED_PER_SECOND: f64 = 330.0;
const DRONE_EXIT_MARGIN: f64 = 42.0;
const DRONE_TARGET_DISTANCE_WEIGHT: f64 = 1.0;
const DRONE_TARGET_ASSIGNED_PENALTY: f64 = 150.0;
const DRONE_TARGET_PROGRESS_BONUS: f64 = 70.0;
const DRONE_TARGET_STICKINESS_BONUS: f64 = 42.0;
const DRONE_RETARGET_INTERVAL_SECONDS: f64 = 0.55;
const DRONE_RETARGET_JITTER_SECONDS: f64 = 0.18;

#[derive(Clone, Copy, Debug)]
struct DroneTargetOrbit {
    angle: f64,
    angular_speed_per_second: f64,
    radius: f64,
}

#[derive(Clone, Debug)]
pub struct Drone {
    pub id: u32,
    pub x: f64,
    pub y: f64,
    pub angle: f64,
    pub removed: bool,
    pub level: u32,
    home: Point,
    lifetime_seconds: f64,
    movement_speed_per_second: f64,
    fire_interval_seconds: f64,
    attack_range: f64,
    age_seconds: f64,
    fire_cooldown_seconds: f64,
    retarget_cooldown_seconds: f64,
    target: Option<MonsterRef>,
    target_orbit: DroneTargetOrbit,
    exiting: bool,
    exit_velocity_x_per_second: f64,
    exit_velocity_y_per_second: f64,
}

impl Drone {
    pub fn new(home: Point, level: u32) -> Drone {
        let level_value = level as f64;
        Drone {
            id: next_entity_id(),
            x: home.x,
            y: home.y,
            angle: -PI / 2.0,
            removed: false,
            level,
            home,
            lifetime_seconds: 20.0 + level_value * 5.0,
            movement_speed_per_second: DRONE_SPEED_BASE + level_value * DRONE_SPEED_PER_LEVEL,
            fire_interval_seconds: clamp(0.58 - level_value * 0.035, 0.35, 0.58),
            attack_range: 44.0 + level_value * 4.0,
            age_seconds: 0.0,
            fire_cooldown_seconds: 0.18,
            retarget_cooldown_seconds: 0.0,
            target: None,
            target_orbit: create_target_orbit(None),
            exiting: false,
            exit_velocity_x_per_second: 0.0,
            exit_velocity_y_per_second: 0.0,
        }
    }

    pub fn update(&mut self, context: &UpdateContext, result: &mut UpdateResult) {
        self.age_seconds += context.delta_seconds;
        if self.exiting {
            self.update_exit(context);
            return;
        }
        if self.age_seconds >= self.lifetime_seconds {
            self.start_exit(context);
            self.update_exit(context);
            return;
        }

        if self.fire_cooldown_seconds > 0.0 {
            self.fire_cooldown_seconds -= context.delta_seconds;
        }
        self.retarget_cooldown_seconds = (self.retarget_cooldown_seconds - context.delta_seconds).max(0.0);
        self.update_target(context);

        if let Some(target) = self.target_position() {
            self.advance_target_orbit(context.delta_seconds);
            self.move_toward_position(
                target.x + self.target_orbit.angle.cos() * self.target_orbit.radius,
                target.y + self.target_orbit.angle.sin() * self.target_orbit.radius,
                context.delta_seconds,
            );
        } else {
            let loiter_angle = self.age_seconds * DRONE_LOITER_SPEED_PER_SECOND;
            self.move_toward_position(
                self.home.x + loiter_angle.cos() * DRONE_LOITER_RADIUS,
                self.home.y + loiter_angle.sin() * DRONE_LOITER_RADIUS,
                context.delta_seconds,
            );
        }
        if let Some(target) = self.target_position() {
            self.enforce_target_stand_off(target);
        }
        let separated = self.apply_drone_separation(context);
        if separated && let Some(target) = self.target_position() {
            self.enforce_target_stand_off(target);
        }
        self.try_fire(result);
        self.fire_cooldown_seconds = self.fire_cooldown_seconds.max(0.0);
    }

    pub fn is_exiting(&self) -> bool {
        self.exiting
    }

    /// The current target while it is still alive.
    pub fn get_assigned_target(&self) -> Option<MonsterRef> {
        self.target.as_ref().filter(|target| target.borrow().is_active()).cloned()
    }

    fn target_position(&self) -> Option<Point> {
        self.target.as_ref().map(|target| {
            let target = target.borrow();
            Point::new(target.x, target.y)
        })
    }

    fn update_target(&mut self, context: &UpdateContext) {
        if self.target.as_ref().is_some_and(|target| target.borrow().is_active())
            && self.retarget_cooldown_seconds > 0.0
        {
            return;
        }
        let next_target = self.get_tracked_target(context);
        let same = match (&next_target, &self.target) {
            (Some(next), Some(current)) => Rc::ptr_eq(next, current),
            (None, None) => true,
            _ => false,
        };
        if !same {
            let target_point = next_target.as_ref().map(|target| {
                let target = target.borrow();
                Point::new(target.x, target.y)
            });
            self.target_orbit = create_target_orbit(target_point.map(|target| (Point::new(self.x, self.y), target)));
        }
        self.target = next_target;
        self.retarget_cooldown_seconds = get_retarget_cooldown_seconds();
    }

    fn get_tracked_target(&self, context: &UpdateContext) -> Option<MonsterRef> {
        let mut best_target = None;
        let mut best_score = f64::INFINITY;
        for monster in context.active_monsters {
            if !monster.borrow().is_active() {
                continue;
            }
            let score = self.score_target(monster, context);
            if score < best_score {
                best_score = score;
                best_target = Some(monster);
            }
        }
        best_target.cloned()
    }

    fn score_target(&self, monster: &MonsterRef, context: &UpdateContext) -> f64 {
        let is_current = self.target.as_ref().is_some_and(|target| Rc::ptr_eq(target, monster));
        let monster = monster.borrow();
        let dx = monster.x - self.x;
        let dy = monster.y - self.y;
        let distance_score = dx.hypot(dy) * DRONE_TARGET_DISTANCE_WEIGHT;
        let assigned_drone_count = context.drone_assignments.get(&monster.id).copied().unwrap_or(0);
        let other_drone_count = if is_current { (assigned_drone_count - 1).max(0) } else { assigned_drone_count };
        let assigned_drone_penalty = other_drone_count as f64 * DRONE_TARGET_ASSIGNED_PENALTY;
        let progress_bonus = monster.get_path_progress() * DRONE_TARGET_PROGRESS_BONUS;
        let stickiness_bonus = if is_current { DRONE_TARGET_STICKINESS_BONUS } else { 0.0 };
        distance_score + assigned_drone_penalty - progress_bonus - stickiness_bonus
    }

    fn move_toward_position(&mut self, destination_x: f64, destination_y: f64, delta_seconds: f64) {
        let dx = destination_x - self.x;
        let dy = destination_y - self.y;
        let distance = dx.hypot(dy);
        if distance <= DRONE_ARRIVE_DISTANCE {
            return;
        }
        let target_angle = dy.atan2(dx);
        self.angle = turn_angle_towards(self.angle, target_angle, 10.0 * delta_seconds);
        let travel = (distance - DRONE_ARRIVE_DISTANCE).min(self.movement_speed_per_second * delta_seconds);
        self.x += target_angle.cos() * travel;
        self.y += target_angle.sin() * travel;
    }

    fn enforce_target_stand_off(&mut self, target: Point) {
        let dx = self.x - target.x;
        let dy = self.y - target.y;
        let distance = dx.hypot(dy);
        if distance >= DRONE_TARGET_STANDOFF_MIN {
            return;
        }
        let angle = if distance > 0.001 { dy.atan2(dx) } else { self.target_orbit.angle };
        self.x = target.x + angle.cos() * DRONE_TARGET_STANDOFF_MIN;
        self.y = target.y + angle.sin() * DRONE_TARGET_STANDOFF_MIN;
    }

    fn advance_target_orbit(&mut self, delta_seconds: f64) {
        self.target_orbit.angle += self.target_orbit.angular_speed_per_second * delta_seconds;
    }

    fn apply_drone_separation(&mut self, context: &UpdateContext) -> bool {
        let mut push_x = 0.0;
        let mut push_y = 0.0;
        let position = Point::new(self.x, self.y);
        for other in context.active_drones {
            // The updating drone is mutably borrowed; recognize it by address before borrowing.
            if std::ptr::eq(other.as_ptr().cast_const(), self as *const Drone) {
                continue;
            }
            let other = other.borrow();
            if other.removed {
                continue;
            }
            let other_position = Point::new(other.x, other.y);
            if !within_distance(position, other_position, DRONE_SEPARATION_DISTANCE) {
                continue;
            }
            let mut dx = self.x - other.x;
            let mut dy = self.y - other.y;
            let mut distance = calculate_distance(position, other_position);
            if distance < 0.001 {
                let angle = (self.id as f64 * 2.399963) % (PI * 2.0);
                dx = angle.cos();
                dy = angle.sin();
                distance = 1.0;
            }
            let overlap_ratio = (DRONE_SEPARATION_DISTANCE - distance) / DRONE_SEPARATION_DISTANCE;
            push_x += (dx / distance) * overlap_ratio;
            push_y += (dy / distance) * overlap_ratio;
        }

        let push_distance = push_x.hypot(push_y);
        if push_distance <= 0.0 {
            return false;
        }
        let max_step = DRONE_SEPARATION_SPEED_PER_SECOND * context.delta_seconds;
        let step = max_step.min(push_distance * DRONE_SEPARATION_DISTANCE * 0.45);
        self.x += (push_x / push_distance) * step;
        self.y += (push_y / push_distance) * step;
        true
    }

    fn start_exit(&mut self, context: &UpdateContext) {
        self.exiting = true;
        self.target = None;
        let field_center_x = context.field_width / 2.0;
        let field_center_y = context.field_height / 2.0;
        let away_from_center = (self.y - field_center_y).atan2(self.x - field_center_x);
        let exit_angle = away_from_center + random_range(-0.35, 0.35);
        self.angle = exit_angle;
        self.exit_velocity_x_per_second = exit_angle.cos() * DRONE_EXIT_SPEED_PER_SECOND;
        self.exit_velocity_y_per_second = exit_angle.sin() * DRONE_EXIT_SPEED_PER_SECOND;
    }

    fn update_exit(&mut self, context: &UpdateContext) {
        self.x += self.exit_velocity_x_per_second * context.delta_seconds;
        self.y += self.exit_velocity_y_per_second * context.delta_seconds;
        if is_outside_bounds(self.x, self.y, &context.field_bounds, DRONE_EXIT_MARGIN) {
            self.removed = true;
        }
    }

    fn try_fire(&mut self, result: &mut UpdateResult) {
        if self.fire_cooldown_seconds > TIMER_EPSILON_SECONDS {
            return;
        }
        let Some(target) = &self.target else {
            return;
        };
        let (target_point, velocity_x, velocity_y) = {
            let target = target.borrow();
            (Point::new(target.x, target.y), target.velocity_x_per_second, target.velocity_y_per_second)
        };
        let position = Point::new(self.x, self.y);
        if !within_distance(position, target_point, self.attack_range) {
            return;
        }
        self.fire_cooldown_seconds += self.fire_interval_seconds;
        let intercept =
            calculate_intercept(target_point, velocity_x, velocity_y, DRONE_PROJECTILE_SPEED_PER_SECOND, position);
        result.add_projectile(Projectile::drone(position, intercept, self.level));
        result.play_sound(AudioCue::GunFire, Some(self.x), Some(0.14 + self.level as f64 * 0.018));
    }
}

/// `source_and_target: None` picks a random approach angle.
fn create_target_orbit(source_and_target: Option<(Point, Point)>) -> DroneTargetOrbit {
    let fallback_angle = random_range(0.0, PI * 2.0);
    let approach_angle =
        source_and_target.map_or(fallback_angle, |(source, target)| (source.y - target.y).atan2(source.x - target.x));
    let direction = if random_range(0.0, 1.0) < 0.5 { -1.0 } else { 1.0 };
    let angle = approach_angle + random_range(-DRONE_TARGET_ORBIT_PHASE_JITTER, DRONE_TARGET_ORBIT_PHASE_JITTER);
    let angular_speed_per_second = random_range(DRONE_TARGET_ORBIT_SPEED_MIN, DRONE_TARGET_ORBIT_SPEED_MAX) * direction;
    let radius = random_range(DRONE_TARGET_STANDOFF_MIN, DRONE_TARGET_ORBIT_RADIUS_MAX);
    DroneTargetOrbit { angle, angular_speed_per_second, radius }
}

fn get_retarget_cooldown_seconds() -> f64 {
    DRONE_RETARGET_INTERVAL_SECONDS + random_range(-DRONE_RETARGET_JITTER_SECONDS, DRONE_RETARGET_JITTER_SECONDS)
}

//! Shared monster movement, damage, slow recovery, hit shake, and lifecycle reporting.
//! Concrete monsters carry their state in `MonsterSpecial`.
use std::f64::consts::PI;

use crate::collision::{ActiveCircleSweep, CircleSweep};
use crate::entities::monsters::berserker::BerserkerState;
use crate::entities::monsters::bulwark::BulwarkState;
use crate::entities::monsters::packman::PackManState;
use crate::entities::monsters::runner::RunnerState;
use crate::entities::monsters::square::SquareState;
use crate::entities::monsters::tank::TankState;
use crate::entities::monsters::triangle::TriangleState;
use crate::entities::monsters::{berserker, bulwark, packman, runner, splitter, square, tank, triangle};
use crate::entities::{MonsterRef, next_entity_id};
use crate::route_path::{SharedPath, get_path_heading_angle};
use crate::types::{Color, MonsterKind, Point};
use crate::update::{UpdateContext, UpdateResult};
use crate::utils::{angle_between, clamp, random_range};

const HIT_SHAKE_DURATION_SECONDS: f64 = 0.16;
const HIT_SHAKE_DISTANCE: f64 = 2.0;
const HIT_SHAKE_HORIZONTAL_FREQUENCY_PER_SECOND: f64 = 92.0;
const HIT_SHAKE_VERTICAL_FREQUENCY_PER_SECOND: f64 = 117.0;
const HIT_SHAKE_VERTICAL_PHASE_SCALE: f64 = 0.7;

/// Per-kind state (the TS monster subclasses).
#[derive(Clone, Debug)]
pub enum MonsterSpecial {
    PackMan(PackManState),
    Square(SquareState),
    Triangle(TriangleState),
    Tank(TankState),
    Runner(RunnerState),
    Splitter,
    Berserker(BerserkerState),
    Bulwark(BulwarkState),
}

#[derive(Clone, Debug)]
pub struct Monster {
    pub id: u32,
    pub x: f64,
    pub y: f64,
    pub previous_x: f64,
    pub previous_y: f64,
    pub velocity_x_per_second: f64,
    pub velocity_y_per_second: f64,
    pub speed_per_second: f64,
    pub max_speed_per_second: f64,
    /// Current health.
    pub hit_points: f64,
    /// The full-health denominator used by the health bar.
    pub max_hit_points: f64,
    pub bounty: i32,
    pub radius: f64,
    pub color: Color,
    pub path: SharedPath,
    path_length: f64,
    pub distance_along_path: f64,
    pub target_index: usize,
    pub rotation: f64,
    pub angle: f64,
    pub removed: bool,
    slow_recovery_speed_per_second: f64,
    hit_shake_seconds: f64,
    hit_shake_duration_seconds: f64,
    hit_shake_distance: f64,
    hit_shake_phase: f64,
    hit_shake_offset_x: f64,
    hit_shake_offset_y: f64,
    pub special: MonsterSpecial,
}

enum MonsterOutcome {
    Killed,
    Escaped,
}

impl ActiveCircleSweep for MonsterRef {
    fn sweep(&self) -> CircleSweep {
        let monster = self.borrow();
        CircleSweep {
            previous_x: monster.previous_x,
            previous_y: monster.previous_y,
            x: monster.x,
            y: monster.y,
            radius: monster.radius,
        }
    }

    fn is_active(&self) -> bool {
        self.borrow().is_active()
    }
}

impl Monster {
    /// A monster of `kind` at base stats (level hit-point scaling lives in `monster_factory`).
    pub fn new(kind: MonsterKind, path: SharedPath, speed_scale: f64) -> Monster {
        match kind {
            MonsterKind::PackMan => packman::create(path, speed_scale),
            MonsterKind::Square => square::create(path, speed_scale),
            MonsterKind::Triangle => triangle::create(path, speed_scale),
            MonsterKind::Tank => tank::create(path, speed_scale),
            MonsterKind::Runner => runner::create(path, speed_scale),
            MonsterKind::Splitter => splitter::create(path, speed_scale),
            MonsterKind::Berserker => berserker::create(path, speed_scale),
            MonsterKind::Bulwark => bulwark::create(path, speed_scale),
        }
    }

    /// The shared constructor; `special` starts as a placeholder the concrete kind replaces
    /// (after the base fields, matching the TS field-initializer order).
    pub(crate) fn base(
        path: SharedPath,
        color: Color,
        speed_per_second: f64,
        hit_points: f64,
        bounty: i32,
        radius: f64,
    ) -> Monster {
        let rotation = random_range(0.0, PI * 2.0);
        let start = path.first().map_or(Point::default(), |entry| entry.point());
        let next = path.get(1).map_or(start, |entry| entry.point());
        let angle = angle_between(start, next);
        let path_length = path.last().map_or(0.0, |entry| entry.total_distance);
        Monster {
            id: next_entity_id(),
            x: start.x,
            y: start.y,
            previous_x: start.x,
            previous_y: start.y,
            velocity_x_per_second: angle.cos() * speed_per_second,
            velocity_y_per_second: angle.sin() * speed_per_second,
            speed_per_second,
            max_speed_per_second: speed_per_second,
            hit_points,
            max_hit_points: hit_points,
            bounty,
            radius,
            color,
            path,
            path_length,
            distance_along_path: 0.0,
            target_index: 1,
            rotation,
            angle,
            removed: false,
            slow_recovery_speed_per_second: 0.0,
            hit_shake_seconds: 0.0,
            hit_shake_duration_seconds: HIT_SHAKE_DURATION_SECONDS,
            hit_shake_distance: HIT_SHAKE_DISTANCE,
            hit_shake_phase: 0.0,
            hit_shake_offset_x: 0.0,
            hit_shake_offset_y: 0.0,
            special: MonsterSpecial::Splitter,
        }
    }

    pub fn kind(&self) -> MonsterKind {
        match self.special {
            MonsterSpecial::PackMan(_) => MonsterKind::PackMan,
            MonsterSpecial::Square(_) => MonsterKind::Square,
            MonsterSpecial::Triangle(_) => MonsterKind::Triangle,
            MonsterSpecial::Tank(_) => MonsterKind::Tank,
            MonsterSpecial::Runner(_) => MonsterKind::Runner,
            MonsterSpecial::Splitter => MonsterKind::Splitter,
            MonsterSpecial::Berserker(_) => MonsterKind::Berserker,
            MonsterSpecial::Bulwark(_) => MonsterKind::Bulwark,
        }
    }

    /// Not removed and still alive: the only monsters towers, shots, and drones consider.
    pub fn is_active(&self) -> bool {
        !self.removed && self.hit_points > 0.0
    }

    pub fn path_length(&self) -> f64 {
        self.path_length
    }

    /// A discrete hit. Bulwarks apply flat armor once per hit.
    pub fn take_damage(&mut self, amount: f64) {
        if matches!(self.special, MonsterSpecial::Bulwark(_)) {
            if amount <= 0.0 {
                return;
            }
            self.apply_damage(bulwark::mitigate(amount));
            return;
        }
        self.apply_damage(amount);
    }

    /// Continuous damage (laser beams): never armored, so results do not depend on tick rate.
    pub fn take_continuous_damage(&mut self, amount: f64) {
        self.apply_damage(amount);
    }

    fn apply_damage(&mut self, amount: f64) {
        self.hit_points = (self.hit_points - amount).max(0.0);
    }

    pub fn shake_from_hit(&mut self) {
        self.shake(HIT_SHAKE_DURATION_SECONDS, HIT_SHAKE_DISTANCE);
    }

    pub fn get_path_progress(&self) -> f64 {
        if self.path_length <= 0.0 {
            return 0.0;
        }
        clamp(self.distance_along_path / self.path_length, 0.0, 1.0)
    }

    pub fn shake(&mut self, duration_seconds: f64, distance: f64) {
        self.hit_shake_duration_seconds = duration_seconds.max(0.001);
        self.hit_shake_seconds = self.hit_shake_duration_seconds;
        self.hit_shake_distance = distance.max(0.0);
        self.hit_shake_phase = random_range(0.0, PI * 2.0);
        self.update_hit_shake_offset();
    }

    pub fn slow_down(&mut self, factor: f64, recovery_speed_per_second: f64) {
        let slowed_speed_per_second = self.max_speed_per_second * factor;
        if slowed_speed_per_second < self.speed_per_second {
            self.speed_per_second = slowed_speed_per_second;
            self.slow_recovery_speed_per_second = recovery_speed_per_second;
        }
    }

    /// Advances `monster`, reporting a kill or escape to `result`.
    pub fn update(monster: &MonsterRef, context: &UpdateContext, result: &mut UpdateResult) {
        let outcome = monster.borrow_mut().advance(context, result);
        match outcome {
            Some(MonsterOutcome::Killed) => result.add_killed_monster(monster.clone()),
            Some(MonsterOutcome::Escaped) => result.add_escaped_monster(monster.clone()),
            None => {}
        }
    }

    fn advance(&mut self, context: &UpdateContext, result: &mut UpdateResult) -> Option<MonsterOutcome> {
        if self.removed {
            return None;
        }
        if self.hit_points <= 0.0 {
            self.removed = true;
            return Some(MonsterOutcome::Killed);
        }

        self.previous_x = self.x;
        self.previous_y = self.y;

        if self.speed_per_second < self.max_speed_per_second {
            self.speed_per_second = self
                .max_speed_per_second
                .min(self.speed_per_second + self.slow_recovery_speed_per_second * context.delta_seconds);
        }

        if self.move_along_path(context.delta_seconds) {
            return Some(MonsterOutcome::Escaped);
        }

        self.update_special(context.delta_seconds);
        self.hit_shake_seconds = (self.hit_shake_seconds - context.delta_seconds).max(0.0);
        self.update_hit_shake_offset();

        if matches!(self.special, MonsterSpecial::Tank(_)) {
            tank::add_track_prints(self, result);
        }
        None
    }

    fn update_special(&mut self, delta_seconds: f64) {
        // The kind's state is taken out so its update can also change shared fields.
        let mut special = std::mem::replace(&mut self.special, MonsterSpecial::Splitter);
        match &mut special {
            MonsterSpecial::PackMan(state) => state.update(delta_seconds),
            MonsterSpecial::Square(state) => state.update(self, delta_seconds),
            MonsterSpecial::Triangle(state) => state.update(delta_seconds),
            MonsterSpecial::Tank(state) => state.update(delta_seconds),
            MonsterSpecial::Runner(state) => state.update(self, delta_seconds),
            MonsterSpecial::Splitter => splitter::update(self, delta_seconds),
            MonsterSpecial::Berserker(state) => state.update(self, delta_seconds),
            MonsterSpecial::Bulwark(state) => state.update(delta_seconds),
        }
        self.special = special;
    }

    /// Adds the kind's breakup particles and death sound.
    pub fn add_death_effect(&self, result: &mut UpdateResult) {
        match &self.special {
            MonsterSpecial::PackMan(state) => state.add_death_effect(self, result),
            MonsterSpecial::Square(state) => state.add_death_effect(self, result),
            MonsterSpecial::Triangle(state) => state.add_death_effect(self, result),
            MonsterSpecial::Tank(state) => state.add_death_effect(self, result),
            MonsterSpecial::Runner(state) => state.add_death_effect(self, result),
            MonsterSpecial::Splitter => splitter::add_death_effect(self, result),
            MonsterSpecial::Berserker(state) => state.add_death_effect(self, result),
            MonsterSpecial::Bulwark(_) => bulwark::add_death_effect(self, result),
        }
    }

    /// Returns true when the monster reached the end of its path (escaped).
    fn move_along_path(&mut self, delta_seconds: f64) -> bool {
        self.distance_along_path += self.speed_per_second * delta_seconds;
        if self.distance_along_path >= self.path_length {
            if let Some(end) = self.path.last() {
                self.x = end.x;
                self.y = end.y;
            }
            self.target_index = self.path.len().saturating_sub(1);
            self.angle = get_path_heading_angle(&self.path, self.path_length, self.target_index);
            self.set_velocity_from_angle();
            self.removed = true;
            return true;
        }
        self.update_position_at_distance(self.distance_along_path);
        self.set_velocity_from_angle();
        false
    }

    pub(crate) fn set_velocity_from_angle(&mut self) {
        self.velocity_x_per_second = self.angle.cos() * self.speed_per_second;
        self.velocity_y_per_second = self.angle.sin() * self.speed_per_second;
    }

    fn update_position_at_distance(&mut self, distance: f64) {
        let path = &self.path;
        while self.target_index < path.len().saturating_sub(1) && path[self.target_index].total_distance < distance {
            self.target_index += 1;
        }
        let end = path.get(self.target_index).or(path.last());
        let start_index = self.target_index.saturating_sub(1);
        let start = path.get(start_index).or(end);
        let (Some(start), Some(end)) = (start, end) else {
            return;
        };
        let span = end.total_distance - start.total_distance;
        let ratio = if span > 0.0 { (distance - start.total_distance) / span } else { 1.0 };
        self.x = start.x + (end.x - start.x) * ratio;
        self.y = start.y + (end.y - start.y) * ratio;
        self.angle = get_path_heading_angle(path, distance, self.target_index);
    }

    /// Presentation x (with hit shake); also the origin of death particles. `x`/`y` stay on the route.
    pub fn visual_x(&self) -> f64 {
        self.x + self.hit_shake_offset_x
    }

    pub fn visual_y(&self) -> f64 {
        self.y + self.hit_shake_offset_y
    }

    fn update_hit_shake_offset(&mut self) {
        if self.hit_shake_seconds <= 0.0 {
            self.hit_shake_offset_x = 0.0;
            self.hit_shake_offset_y = 0.0;
            return;
        }
        let elapsed_seconds = self.hit_shake_duration_seconds - self.hit_shake_seconds;
        let fade = self.hit_shake_seconds / self.hit_shake_duration_seconds;
        let distance = self.hit_shake_distance * fade;
        self.hit_shake_offset_x =
            (self.hit_shake_phase + elapsed_seconds * HIT_SHAKE_HORIZONTAL_FREQUENCY_PER_SECOND).sin() * distance;
        self.hit_shake_offset_y = (self.hit_shake_phase * HIT_SHAKE_VERTICAL_PHASE_SCALE
            + elapsed_seconds * HIT_SHAKE_VERTICAL_FREQUENCY_PER_SECOND)
            .cos()
            * distance;
    }

    pub(crate) fn shard_source(&self) -> crate::entities::monsters::death_effect_helpers::ShardSource {
        crate::entities::monsters::death_effect_helpers::ShardSource {
            visual_x: self.visual_x(),
            visual_y: self.visual_y(),
            color: self.color,
        }
    }

    // Presentation getters for the renderer; 0 (or neutral) for other kinds.

    /// Pack-man mouth half-angle in radians.
    pub fn current_mouth_angle(&self) -> f64 {
        match &self.special {
            MonsterSpecial::PackMan(state) => state.mouth_angle,
            _ => 0.0,
        }
    }

    /// Pack-man idle spin, added to the heading.
    pub fn current_body_rotation(&self) -> f64 {
        match &self.special {
            MonsterSpecial::PackMan(state) => state.body_rotation,
            _ => 0.0,
        }
    }

    /// Square's pulsing half-size; the radius for other kinds.
    pub fn get_visual_radius(&self) -> f64 {
        match &self.special {
            MonsterSpecial::Square(state) => state.visual_radius(self.radius),
            _ => self.radius,
        }
    }

    pub fn current_nose_wobble_angle(&self) -> f64 {
        match &self.special {
            MonsterSpecial::Triangle(state) => state.nose_wobble_angle,
            _ => 0.0,
        }
    }

    pub fn current_turret_rotation(&self) -> f64 {
        match &self.special {
            MonsterSpecial::Tank(state) => state.turret_rotation,
            _ => 0.0,
        }
    }

    /// Runner dash stretch, 0 to 1.
    pub fn get_dash_pulse(&self) -> f64 {
        match &self.special {
            MonsterSpecial::Runner(state) => state.dash_pulse(),
            _ => 0.0,
        }
    }

    pub fn current_rage_stage(&self) -> u32 {
        match &self.special {
            MonsterSpecial::Berserker(state) => state.rage_stage,
            _ => 0,
        }
    }

    pub fn get_rage_motion(&self) -> berserker::RageMotion {
        match &self.special {
            MonsterSpecial::Berserker(state) => state.rage_motion(),
            _ => berserker::RageMotion { scale_x: 1.0, scale_y: 1.0, ember_alpha: 0.0 },
        }
    }

    pub fn current_shield_pulse(&self) -> f64 {
        match &self.special {
            MonsterSpecial::Bulwark(state) => state.shield_pulse,
            _ => 0.0,
        }
    }
}

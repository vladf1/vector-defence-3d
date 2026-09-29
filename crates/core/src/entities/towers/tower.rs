//! Shared tower targeting, cooldown, upgrade, and selection behavior. Concrete towers carry
//! their state and attack behavior in `TowerSpecial`.
use std::rc::Rc;

use crate::constants::{MAX_TOWER_LEVEL, TIMER_EPSILON_SECONDS, TOWER_RANGE_UPGRADE_STEP, UPGRADE_COST};
use crate::entities::towers::drone_tower;
use crate::entities::towers::gun::GunState;
use crate::entities::towers::laser::{self, LaserState};
use crate::entities::towers::lightning;
use crate::entities::towers::missile_tower::{self, MissileTowerState};
use crate::entities::towers::registry::tower_info;
use crate::entities::towers::slow::SlowState;
use crate::entities::{MonsterRef, TowerRef, next_entity_id};
use crate::types::{Point, TowerKind};
use crate::update::{UpdateContext, UpdateResult};
use crate::utils::{js_round, normalize_angle};

pub const DEFAULT_FIRING_ANGLE_TOLERANCE: f64 = 0.08;

#[derive(Clone, Debug)]
pub enum TowerSpecial {
    Gun(GunState),
    Laser(LaserState),
    Missile(MissileTowerState),
    Slow(SlowState),
    Drone,
    Lightning,
}

#[derive(Clone, Debug)]
pub struct Tower {
    pub id: u32,
    pub kind: TowerKind,
    pub x: f64,
    pub y: f64,
    pub range: f64,
    pub cost: i32,
    pub current_target: Option<MonsterRef>,
    /// 0 when built; each upgrade adds one (up to `MAX_TOWER_LEVEL`).
    pub level: u32,
    pub cooldown_seconds: f64,
    pub removed: bool,
    pub special: TowerSpecial,
}

impl Tower {
    /// A fresh level-0 tower at its registry range and cost (the profile range scale is
    /// applied by `Game::create_tower`).
    pub fn new(kind: TowerKind, x: f64, y: f64) -> Tower {
        let info = tower_info(kind);
        let special = match kind {
            TowerKind::Gun => TowerSpecial::Gun(GunState::new()),
            TowerKind::Laser => TowerSpecial::Laser(LaserState::new()),
            TowerKind::Missile => TowerSpecial::Missile(MissileTowerState::new(0)),
            TowerKind::Slow => TowerSpecial::Slow(SlowState::new()),
            TowerKind::Drone => TowerSpecial::Drone,
            TowerKind::Lightning => TowerSpecial::Lightning,
        };
        Tower {
            id: next_entity_id(),
            kind,
            x,
            y,
            range: info.base_range,
            cost: info.base_cost,
            current_target: None,
            level: 0,
            cooldown_seconds: 0.0,
            removed: false,
            special,
        }
    }

    pub fn position(&self) -> Point {
        Point::new(self.x, self.y)
    }

    pub fn upgrade_cost(&self) -> i32 {
        UPGRADE_COST
    }

    pub fn resale_value(&self) -> i32 {
        js_round(self.cost as f64 * 0.75) as i32
    }

    pub fn can_upgrade(&self) -> bool {
        self.level < MAX_TOWER_LEVEL
    }

    pub fn update(tower: &TowerRef, context: &UpdateContext, result: &mut UpdateResult) {
        let mut this = tower.borrow_mut();
        if this.cooldown_seconds > 0.0 {
            this.cooldown_seconds -= context.delta_seconds;
        }
        // The kind's state is taken out so its update can also use the shared fields.
        let mut special = std::mem::replace(&mut this.special, TowerSpecial::Drone);
        match &mut special {
            TowerSpecial::Gun(state) => state.update(&mut this, context, result),
            TowerSpecial::Laser(state) => state.update(&mut this, context, result),
            TowerSpecial::Missile(state) => state.update(&mut this, context, result),
            TowerSpecial::Slow(state) => state.update(&mut this, tower, context, result),
            TowerSpecial::Drone => drone_tower::update(&mut this, context, result),
            TowerSpecial::Lightning => lightning::update(&mut this, tower, context, result),
        }
        this.special = special;
        // Keep overshoot for a shot fired this step, but never bank idle shots.
        this.cooldown_seconds = this.cooldown_seconds.max(0.0);
    }

    pub fn upgrade(&mut self) {
        if !self.can_upgrade() {
            return;
        }
        self.level += 1;
        self.cost += UPGRADE_COST;
        self.range += self.level as f64 * TOWER_RANGE_UPGRADE_STEP;
        let level = self.level;
        match &mut self.special {
            TowerSpecial::Laser(state) => state.on_upgrade(level),
            TowerSpecial::Missile(state) => state.on_upgrade(level),
            _ => {}
        }
    }

    pub(crate) fn find_tracked_monster(&mut self, context: &UpdateContext) -> Option<MonsterRef> {
        if let Some(current) = &self.current_target
            && self.can_track_monster(current)
        {
            return Some(current.clone());
        }
        self.current_target = self.find_closest_monster(context);
        self.current_target.clone()
    }

    pub(crate) fn find_closest_monster(&self, context: &UpdateContext) -> Option<MonsterRef> {
        let mut closest = None;
        let mut smallest_distance_squared = f64::INFINITY;
        for monster in context.active_monsters {
            let monster_ref = monster.borrow();
            if !monster_ref.is_active() {
                continue;
            }
            let Some(distance_squared) = self.distance_squared_in_range(monster_ref.x, monster_ref.y) else {
                continue;
            };
            if distance_squared < smallest_distance_squared {
                smallest_distance_squared = distance_squared;
                closest = Some(monster);
            }
        }
        closest.cloned()
    }

    fn can_track_monster(&self, monster: &MonsterRef) -> bool {
        let monster = monster.borrow();
        monster.is_active() && self.distance_squared_in_range(monster.x, monster.y).is_some()
    }

    fn distance_squared_in_range(&self, x: f64, y: f64) -> Option<f64> {
        let dx = x - self.x;
        if dx.abs() > self.range {
            return None;
        }
        let dy = y - self.y;
        if dy.abs() > self.range {
            return None;
        }
        let distance_squared = dx * dx + dy * dy;
        if distance_squared > self.range * self.range {
            return None;
        }
        Some(distance_squared)
    }

    pub(crate) fn reset_cooldown(&mut self, seconds: f64) {
        self.cooldown_seconds += seconds;
    }

    pub fn ready(&self) -> bool {
        self.cooldown_seconds <= TIMER_EPSILON_SECONDS
    }

    pub(crate) fn is_aimed_at_target(current_angle: f64, target_angle: f64, tolerance: f64) -> bool {
        normalize_angle(target_angle - current_angle).abs() <= tolerance
    }

    pub fn is_targeting(&self, monster: &MonsterRef) -> bool {
        self.current_target.as_ref().is_some_and(|target| Rc::ptr_eq(target, monster))
    }

    // Presentation getters for the renderer.

    /// Turret heading of gun, laser, and missile towers (0 for the others).
    pub fn angle(&self) -> f64 {
        match &self.special {
            TowerSpecial::Gun(state) => state.angle,
            TowerSpecial::Laser(state) => state.angle,
            TowerSpecial::Missile(state) => state.angle,
            _ => 0.0,
        }
    }

    pub fn set_angle(&mut self, angle: f64) {
        match &mut self.special {
            TowerSpecial::Gun(state) => state.angle = angle,
            TowerSpecial::Laser(state) => state.angle = angle,
            TowerSpecial::Missile(state) => state.angle = angle,
            _ => {}
        }
    }

    pub fn gun(&self) -> Option<&GunState> {
        match &self.special {
            TowerSpecial::Gun(state) => Some(state),
            _ => None,
        }
    }

    pub fn gun_mut(&mut self) -> Option<&mut GunState> {
        match &mut self.special {
            TowerSpecial::Gun(state) => Some(state),
            _ => None,
        }
    }

    pub fn laser(&self) -> Option<&LaserState> {
        match &self.special {
            TowerSpecial::Laser(state) => Some(state),
            _ => None,
        }
    }

    pub fn laser_mut(&mut self) -> Option<&mut LaserState> {
        match &mut self.special {
            TowerSpecial::Laser(state) => Some(state),
            _ => None,
        }
    }

    pub fn missile(&self) -> Option<&MissileTowerState> {
        match &self.special {
            TowerSpecial::Missile(state) => Some(state),
            _ => None,
        }
    }

    pub fn slow(&self) -> Option<&SlowState> {
        match &self.special {
            TowerSpecial::Slow(state) => Some(state),
            _ => None,
        }
    }

    pub fn slow_mut(&mut self) -> Option<&mut SlowState> {
        match &mut self.special {
            TowerSpecial::Slow(state) => Some(state),
            _ => None,
        }
    }

    pub fn is_laser(&self) -> bool {
        matches!(self.special, TowerSpecial::Laser(_))
    }

    /// Laser direction lock (false for other towers).
    pub fn direction_locked(&self) -> bool {
        self.laser().is_some_and(|state| state.direction_locked)
    }

    /// Laser beam muzzle distance from the tower center.
    pub fn get_muzzle_offset(&self) -> f64 {
        laser::get_muzzle_offset(self.level)
    }

    /// Missile tower: 0 right after launch, 1 once the next missile is seated.
    pub fn get_reload_progress(&self) -> f64 {
        missile_tower::get_reload_progress(self)
    }

    /// Drone tower: 0 right after launch, 1 when a drone is docked and ready.
    pub fn get_launch_readiness(&self) -> f64 {
        drone_tower::get_launch_readiness(self)
    }

    pub fn laser_colors(&self) -> &'static laser::LaserColors {
        laser::laser_colors(self.level)
    }

    pub fn lightning_color(&self) -> crate::types::Color {
        lightning::lightning_color(self.level)
    }

    pub fn missile_powerbank_color(&self) -> crate::types::Color {
        missile_tower::powerbank_color(self.level)
    }

    pub fn muzzle_flash_seconds(&self) -> f64 {
        self.gun().map_or(0.0, |state| state.muzzle_flash_seconds)
    }
}

//! Fast, cheap, accurate lead shots.
use std::f64::consts::PI;

use crate::audio::AudioCue;
use crate::entities::Projectile;
use crate::entities::projectiles::projectile::GUN_PROJECTILE_SPEED_PER_SECOND;
use crate::entities::towers::tower::{DEFAULT_FIRING_ANGLE_TOLERANCE, Tower};
use crate::types::Point;
use crate::update::{UpdateContext, UpdateResult};
use crate::utils::{angle_between, calculate_intercept, random_range, turn_angle_towards};

pub const MUZZLE_FLASH_DURATION_SECONDS: f64 = 0.055;
pub const GUN_PROJECTILE_SOURCE_OFFSET: f64 = 16.0;
const FIRE_INTERVAL_SECONDS: f64 = 0.2;

#[derive(Clone, Debug)]
pub struct GunState {
    pub angle: f64,
    pub turn_speed_per_second: f64,
    pub muzzle_flash_seconds: f64,
}

impl GunState {
    pub(crate) fn new() -> GunState {
        GunState { angle: random_range(-PI, PI), turn_speed_per_second: 9.6, muzzle_flash_seconds: 0.0 }
    }

    pub(crate) fn update(&mut self, tower: &mut Tower, context: &UpdateContext, result: &mut UpdateResult) {
        self.muzzle_flash_seconds = (self.muzzle_flash_seconds - context.delta_seconds).max(0.0);
        let Some(tracked) = tower.find_tracked_monster(context) else {
            return;
        };

        let source = self.projectile_source(tower);
        let target = {
            let tracked = tracked.borrow();
            calculate_intercept(
                Point::new(tracked.x, tracked.y),
                tracked.velocity_x_per_second,
                tracked.velocity_y_per_second,
                GUN_PROJECTILE_SPEED_PER_SECOND,
                source,
            )
        };
        let target_angle = angle_between(tower.position(), target);
        self.angle = turn_angle_towards(self.angle, target_angle, self.turn_speed_per_second * context.delta_seconds);

        let aligned_to_target = Tower::is_aimed_at_target(self.angle, target_angle, DEFAULT_FIRING_ANGLE_TOLERANCE);
        if aligned_to_target && tower.ready() {
            let actual_source = self.projectile_source(tower);
            self.muzzle_flash_seconds = MUZZLE_FLASH_DURATION_SECONDS;
            tower.reset_cooldown(FIRE_INTERVAL_SECONDS);
            result.add_projectile(Projectile::gun(actual_source, target, tower.level));
            result.play_sound(AudioCue::GunFire, Some(actual_source.x), Some(0.92 + tower.level as f64 * 0.08));
        }
    }

    fn projectile_source(&self, tower: &Tower) -> Point {
        Point::new(
            tower.x + self.angle.cos() * GUN_PROJECTILE_SOURCE_OFFSET,
            tower.y + self.angle.sin() * GUN_PROJECTILE_SOURCE_OFFSET,
        )
    }
}

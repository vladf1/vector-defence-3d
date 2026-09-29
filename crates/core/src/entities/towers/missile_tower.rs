//! Slow launcher with splash damage.
use std::f64::consts::PI;

use crate::audio::AudioCue;
use crate::entities::projectiles::missile::{Missile, MissileVisual, create_missile_visual};
use crate::entities::towers::tower::{Tower, TowerSpecial};
use crate::types::{Color, Point};
use crate::update::{UpdateContext, UpdateResult};
use crate::utils::{angle_between, clamp, random_range, turn_angle_towards};

const MISSILE_FIRING_ANGLE_TOLERANCE: f64 = PI / 12.0;
const MISSILE_RACK_CENTER_X: f64 = 0.0;
pub const MISSILE_POWERBANK_COLORS: [Color; 7] = [0x9dffd7, 0xd8ff4f, 0xff9d5c, 0xff6d8c, 0xb58cff, 0x78a7ff, 0xf6f0ff];

pub fn powerbank_color(level: u32) -> Color {
    MISSILE_POWERBANK_COLORS[(level as usize).min(MISSILE_POWERBANK_COLORS.len() - 1)]
}

#[derive(Clone, Debug)]
pub struct MissileTowerState {
    pub angle: f64,
    pub turn_speed_per_second: f64,
    pub visual: MissileVisual,
}

impl MissileTowerState {
    pub(crate) fn new(level: u32) -> MissileTowerState {
        MissileTowerState {
            angle: random_range(-PI, PI),
            turn_speed_per_second: 3.6,
            visual: create_missile_visual(level),
        }
    }

    pub(crate) fn on_upgrade(&mut self, level: u32) {
        self.visual = create_missile_visual(level);
    }

    pub(crate) fn update(&mut self, tower: &mut Tower, context: &UpdateContext, result: &mut UpdateResult) {
        let tracked = tower.find_tracked_monster(context);
        let mut aligned_to_target = false;
        if let Some(tracked) = &tracked {
            let tracked_point = {
                let tracked = tracked.borrow();
                Point::new(tracked.x, tracked.y)
            };
            let target_angle = angle_between(tower.position(), tracked_point);
            self.angle =
                turn_angle_towards(self.angle, target_angle, self.turn_speed_per_second * context.delta_seconds);
            aligned_to_target = Tower::is_aimed_at_target(self.angle, target_angle, MISSILE_FIRING_ANGLE_TOLERANCE);
        }

        if let Some(tracked) = tracked
            && aligned_to_target
            && tower.ready()
        {
            let source = Point::new(
                tower.x + self.angle.cos() * MISSILE_RACK_CENTER_X,
                tower.y + self.angle.sin() * MISSILE_RACK_CENTER_X,
            );
            tower.reset_cooldown(get_cooldown_duration_seconds(tower.level));
            result.add_missile(Missile::new(source, tracked, tower.level, &self.visual, Some(self.angle)));
            result.play_sound(AudioCue::MissileLaunch, Some(source.x), Some(1.0 + tower.level as f64 * 0.09));
        }
    }
}

fn get_cooldown_duration_seconds(level: u32) -> f64 {
    2.0 - 0.2 * level as f64
}

/// 0 right after launch, 1 once the next missile is seated (1 for non-missile towers).
pub(crate) fn get_reload_progress(tower: &Tower) -> f64 {
    if !matches!(tower.special, TowerSpecial::Missile(_)) || tower.ready() {
        return 1.0;
    }
    1.0 - clamp(tower.cooldown_seconds / get_cooldown_duration_seconds(tower.level), 0.0, 1.0)
}

//! Launches one autonomous hunter drone on a long cooldown.
use crate::audio::AudioCue;
use crate::entities::towers::tower::{Tower, TowerSpecial};
use crate::entities::{Drone, share};
use crate::update::{UpdateContext, UpdateResult};

pub const DRONE_COOLDOWN_SECONDS: f64 = 30.0;

pub(crate) fn update(tower: &mut Tower, context: &UpdateContext, result: &mut UpdateResult) {
    if !tower.ready() {
        return;
    }
    if tower.find_closest_monster(context).is_none() {
        return;
    }
    result.add_drone(share(Drone::new(tower.position(), tower.level)));
    tower.reset_cooldown(DRONE_COOLDOWN_SECONDS);
    result.play_sound(AudioCue::GunFire, Some(tower.x), Some(0.22 + tower.level as f64 * 0.025));
}

/// 0 right after launch, 1 when a drone is docked and ready (1 for other towers).
pub(crate) fn get_launch_readiness(tower: &Tower) -> f64 {
    if !matches!(tower.special, TowerSpecial::Drone) || tower.ready() {
        return 1.0;
    }
    1.0 - (tower.cooldown_seconds / DRONE_COOLDOWN_SECONDS).min(1.0)
}

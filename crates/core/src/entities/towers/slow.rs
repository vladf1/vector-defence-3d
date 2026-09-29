//! Freezes clusters so the rest can clean up.
use crate::audio::AudioCue;
use crate::entities::TowerRef;
use crate::entities::effects::link::{Link, LinkSource, SLOW_LINK_COLOR};
use crate::entities::towers::tower::Tower;
use crate::types::Point;
use crate::update::{UpdateContext, UpdateResult};
use crate::utils::within_distance;

pub const SLOW_FACTOR: f64 = 0.5;
const RECOVERY_SPEED_PER_SECOND: f64 = 36.0;
const PULSE_INTERVAL_SECONDS: f64 = 1.0;

#[derive(Clone, Debug)]
pub struct SlowState {
    pub pulse: f64,
    pub orbit: f64,
}

impl SlowState {
    pub(crate) fn new() -> SlowState {
        SlowState { pulse: 0.0, orbit: 0.0 }
    }

    pub(crate) fn update(
        &mut self,
        tower: &mut Tower,
        tower_ref: &TowerRef,
        context: &UpdateContext,
        result: &mut UpdateResult,
    ) {
        self.pulse += 4.8 * context.delta_seconds;
        self.orbit += get_orbit_speed_per_second(tower.level) * context.delta_seconds;
        if !tower.ready() {
            return;
        }

        let mut affected = 0;
        let max_targets = tower.level + 2;
        for monster in context.active_monsters {
            {
                let mut monster = monster.borrow_mut();
                if !monster.is_active() {
                    continue;
                }
                if !within_distance(tower.position(), Point::new(monster.x, monster.y), tower.range) {
                    continue;
                }
                monster.slow_down(SLOW_FACTOR, RECOVERY_SPEED_PER_SECOND);
            }
            if result.remaining_link_capacity() > 0 {
                result.add_link(Link::slow(
                    monster.clone(),
                    SLOW_LINK_COLOR,
                    1.0,
                    LinkSource::Tower(tower_ref.clone()),
                ));
            }
            affected += 1;
            if affected == max_targets {
                break;
            }
        }

        if affected == 0 {
            return;
        }
        tower.reset_cooldown(PULSE_INTERVAL_SECONDS);
        result.play_sound(AudioCue::SlowPulse, Some(tower.x), Some(0.85 + affected as f64 * 0.1));
    }
}

fn get_orbit_speed_per_second(level: u32) -> f64 {
    4.8 / (1.0 + level as f64 * 0.7)
}

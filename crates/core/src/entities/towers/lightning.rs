//! Chains shocks that damage and heavily slow monsters.
use std::rc::Rc;

use crate::audio::AudioCue;
use crate::entities::effects::link::{Link, LinkSource};
use crate::entities::towers::tower::Tower;
use crate::entities::{MonsterRef, TowerRef};
use crate::types::{Color, Point};
use crate::update::{UpdateContext, UpdateResult};
use crate::utils::within_distance;

pub const LIGHTNING_COLORS: [Color; 7] = [0x8ff7ff, 0x7fe5ff, 0x71d0ff, 0x9fb8ff, 0xc9a7ff, 0xf09cff, 0xf5fbff];
const SLOW_FACTOR: f64 = 0.15;
const RECOVERY_SPEED_PER_SECOND: f64 = 100.0;

pub fn lightning_color(level: u32) -> Color {
    LIGHTNING_COLORS[(level as usize).min(LIGHTNING_COLORS.len() - 1)]
}

pub fn get_damage(level: u32) -> f64 {
    12.0 + level as f64 * 4.8
}

pub fn get_cooldown_seconds(level: u32) -> f64 {
    (1.12 - level as f64 * 0.055).max(0.72)
}

pub(crate) fn update(tower: &mut Tower, tower_ref: &TowerRef, context: &UpdateContext, result: &mut UpdateResult) {
    if !tower.ready() {
        return;
    }
    let Some(first_target) = tower.find_closest_monster(context) else {
        return;
    };

    let targets = collect_chain_targets(tower.level, context, first_target);
    let damage = get_damage(tower.level);
    let color = lightning_color(tower.level);
    let mut source = LinkSource::Tower(tower_ref.clone());
    for target in targets {
        {
            let mut monster = target.borrow_mut();
            monster.take_damage(damage);
            monster.shake_from_hit();
            monster.slow_down(SLOW_FACTOR, RECOVERY_SPEED_PER_SECOND);
        }
        if result.remaining_link_capacity() > 0 {
            result.add_link(Link::lightning(source.clone(), target.clone(), color));
        }
        source = LinkSource::Monster(target);
    }

    tower.reset_cooldown(get_cooldown_seconds(tower.level));
    result.play_sound(AudioCue::LightningShock, Some(tower.x), Some(0.95 + tower.level as f64 * 0.09));
}

fn collect_chain_targets(level: u32, context: &UpdateContext, first_target: MonsterRef) -> Vec<MonsterRef> {
    let max_targets = 6.min(2 + (level / 2) as usize);
    let chain_range = 58.0 + level as f64 * 5.0;
    let mut targets = vec![first_target.clone()];
    let mut source = first_target;

    while targets.len() < max_targets {
        let source_point = {
            let source = source.borrow();
            Point::new(source.x, source.y)
        };
        let mut next_target = None;
        let mut closest_distance_squared = f64::INFINITY;
        for monster in context.active_monsters {
            if targets.iter().any(|target| Rc::ptr_eq(target, monster)) {
                continue;
            }
            let candidate = monster.borrow();
            if !candidate.is_active() {
                continue;
            }
            let position = Point::new(candidate.x, candidate.y);
            if !within_distance(source_point, position, chain_range) {
                continue;
            }
            let dx = candidate.x - source_point.x;
            let dy = candidate.y - source_point.y;
            let distance_squared = dx * dx + dy * dy;
            if distance_squared < closest_distance_squared {
                closest_distance_squared = distance_squared;
                next_target = Some(monster);
            }
        }
        let Some(next_target) = next_target.cloned() else {
            break;
        };
        targets.push(next_target.clone());
        source = next_target;
    }
    targets
}

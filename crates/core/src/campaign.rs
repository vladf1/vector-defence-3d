//! Turns the ten authored routes into the campaign: per-wave monster sequences and build windows.
use crate::profile::GameMode;
use crate::types::{LevelData, MonsterKind, Point, RouteData, TowerKind, WaveData};
use crate::utils::{clamp, js_round};

include!(concat!(env!("OUT_DIR"), "/levels.rs"));

const MOBILE_WAVE_COUNT_RATIO: f64 = 0.65;
const MOBILE_SPAWN_INTERVAL_RATIO: f64 = 1.19;

pub fn routes_for_mode(mode: GameMode) -> &'static [RouteData] {
    match mode {
        GameMode::Desktop => DESKTOP_ROUTES,
        GameMode::Mobile => MOBILE_ROUTES,
    }
}

pub fn create_levels(mode: GameMode) -> Vec<LevelData> {
    create_campaign_levels(routes_for_mode(mode), mode == GameMode::Mobile)
}

fn unique_in_order(sequence: &[MonsterKind]) -> Vec<MonsterKind> {
    let mut source = Vec::new();
    for kind in sequence {
        if !source.contains(kind) {
            source.push(*kind);
        }
    }
    source
}

fn build_wave_sequence(base_sequence: &[MonsterKind], level_index: usize, wave_index: usize) -> Vec<MonsterKind> {
    let source = unique_in_order(base_sequence);
    let length = clamp((4 + wave_index + level_index / 2) as f64, 4.0, 12.0) as usize;
    let mut sequence = Vec::with_capacity(length + 2);
    let opening = if source.contains(&MonsterKind::Runner) && wave_index % 3 == 1 {
        MonsterKind::Runner
    } else if source.contains(&MonsterKind::PackMan) {
        MonsterKind::PackMan
    } else {
        source.first().copied().unwrap_or(MonsterKind::PackMan)
    };
    sequence.push(opening);
    for index in 0..length {
        sequence.push(source[(index + wave_index + level_index) % source.len()]);
    }

    let finisher_options: Vec<MonsterKind> = [
        MonsterKind::Tank,
        MonsterKind::Splitter,
        MonsterKind::Bulwark,
        MonsterKind::Berserker,
        MonsterKind::Triangle,
        MonsterKind::Square,
    ]
    .into_iter()
    .filter(|kind| source.contains(kind))
    .collect();
    if wave_index >= 2 && !finisher_options.is_empty() {
        sequence.push(finisher_options[(wave_index + level_index) % finisher_options.len()]);
    }
    sequence
}

fn build_wave(
    level_index: usize,
    wave_index: usize,
    wave_total: usize,
    base_sequence: &[MonsterKind],
    initial_build_time: f64,
    mobile: bool,
) -> WaveData {
    let level = level_index as f64;
    let wave = wave_index as f64;
    let count_base = 12.0 + level * 1.5;
    let count_step = 3.8 + (level_index / 4) as f64;
    let last_wave_bonus = if wave_index == wave_total - 1 { 4.0 + js_round(level * 0.8) } else { 0.0 };
    let count_scale = if mobile { MOBILE_WAVE_COUNT_RATIO } else { 1.0 };
    let count = js_round((count_base + wave * count_step + last_wave_bonus) * count_scale) as u32;
    let pressure = level * 0.65 + wave * 0.55;
    let spawn_interval_scale = if mobile { MOBILE_SPAWN_INTERVAL_RATIO } else { 1.0 };
    let base_min = clamp(0.78 - pressure * 0.067, 0.23, 0.78);
    let spawn_interval_min = base_min * spawn_interval_scale;
    let spawn_interval_max =
        clamp(base_min + 0.32 - (level_index.min(6) as f64) * 0.01, base_min + 0.11, 1.03) * spawn_interval_scale;
    let intermission = clamp(5.75 - level * 0.18 - wave * 0.32, 2.5, 5.5);
    WaveData {
        count,
        monster_sequence: build_wave_sequence(base_sequence, level_index, wave_index),
        spawn_interval_min,
        spawn_interval_max,
        build_time: if wave_index == 0 { initial_build_time } else { intermission },
        reward: js_round((60.0 + level * 9.0 + wave * 13.0) / 10.0) as i32,
    }
}

pub fn create_campaign_levels(routes: &[RouteData], mobile: bool) -> Vec<LevelData> {
    routes
        .iter()
        .enumerate()
        .map(|(level_index, route)| {
            let wave_total = route.wave_count.unwrap_or(6);
            let build_time = route.initial_build_time.unwrap_or(12.0);
            let waves: Vec<WaveData> = (0..wave_total)
                .map(|wave_index| {
                    build_wave(level_index, wave_index, wave_total, route.monster_sequence, build_time, mobile)
                })
                .collect();
            let monster_count = waves.iter().map(|wave| wave.count).sum();
            LevelData {
                id: format!("campaign-{}", level_index + 1),
                level_number: level_index as u32 + 1,
                name: route.name,
                subtitle: route.subtitle.unwrap_or("Hold the route and keep scaling your defense."),
                allow_escape: route.allow_escape,
                starting_money: route.starting_money,
                available_towers: route.available_towers.to_vec(),
                points: route.points.to_vec(),
                waves,
                monster_count,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn campaign_has_ten_levels_with_waves() {
        for mode in [GameMode::Desktop, GameMode::Mobile] {
            let levels = create_levels(mode);
            assert_eq!(levels.len(), 10);
            for level in &levels {
                assert!(!level.waves.is_empty());
                assert_eq!(level.monster_count, level.waves.iter().map(|wave| wave.count).sum::<u32>());
            }
        }
    }

    #[test]
    fn first_level_stays_within_its_monster_pool() {
        let levels = create_levels(GameMode::Desktop);
        let pool = DESKTOP_ROUTES[0].monster_sequence;
        for wave in &levels[0].waves {
            assert!(wave.monster_sequence.iter().all(|kind| pool.contains(kind)));
        }
    }
}

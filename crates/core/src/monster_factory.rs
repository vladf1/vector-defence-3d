//! Monster construction with level hit-point scaling, and splitter children.
use crate::entities::Monster;
use crate::route_path::{SharedPath, create_path_entries_from_distance};
use crate::types::MonsterKind;
use crate::utils::{js_round, random_range};

const MIN_SPLITTER_CHILD_OFFSET_DISTANCE: f64 = 10.0;
const MAX_SPLITTER_CHILD_OFFSET_DISTANCE: f64 = 18.0;
const HIT_POINT_BONUS_PER_LEVEL: f64 = 0.06;
const SPLITTER_CHILD_COUNT: usize = 2;

/// `level_index` is the campaign level (negative before any level started).
pub fn create_monster(kind: MonsterKind, path: SharedPath, speed_scale: f64, level_index: i32) -> Monster {
    let mut monster = Monster::new(kind, path, speed_scale);
    let level_hit_point_multiplier = get_level_hit_point_multiplier(level_index);
    monster.hit_points *= level_hit_point_multiplier;
    monster.max_hit_points *= level_hit_point_multiplier;
    monster
}

/// Two weakened runners placed a little behind and ahead of the splitter on its route.
pub fn create_splitter_children(monster: &Monster, speed_scale: f64, level_index: i32) -> Vec<Monster> {
    let mut children = Vec::with_capacity(SPLITTER_CHILD_COUNT);
    let split_angle = monster.angle;
    let path_length = monster.path.last().map_or(0.0, |entry| entry.total_distance);
    let min_speed_multiplier = 0.89;
    let max_speed_multiplier = 0.97;

    for index in 0..SPLITTER_CHILD_COUNT {
        let spawn_distance = monster.distance_along_path
            + create_splitter_child_path_offset(monster.distance_along_path, path_length, index);
        let child_path: SharedPath = create_path_entries_from_distance(&monster.path, spawn_distance).into();
        let mut child = create_monster(MonsterKind::Runner, child_path, speed_scale, level_index);
        let speed_multiplier = random_range(min_speed_multiplier, max_speed_multiplier);
        child.angle = split_angle + random_range(-0.12, 0.12);
        child.max_speed_per_second *= speed_multiplier;
        child.speed_per_second = child.max_speed_per_second;
        child.velocity_x_per_second = child.angle.cos() * child.speed_per_second;
        child.velocity_y_per_second = child.angle.sin() * child.speed_per_second;
        child.hit_points = js_round(child.max_hit_points * 0.72);
        child.max_hit_points = child.hit_points;
        child.bounty = 1.max(js_round(child.bounty as f64 * 0.55) as i32);
        child.radius *= 0.86;
        children.push(child);
    }
    children
}

fn create_splitter_child_path_offset(distance_along_path: f64, path_length: f64, child_index: usize) -> f64 {
    let preferred_direction = if child_index.is_multiple_of(2) { -1.0 } else { 1.0 };
    create_random_offset_in_direction(distance_along_path, path_length, preferred_direction)
        .or_else(|| create_random_offset_in_direction(distance_along_path, path_length, -preferred_direction))
        .unwrap_or(0.0)
}

fn create_random_offset_in_direction(distance_along_path: f64, path_length: f64, direction: f64) -> Option<f64> {
    let available_distance =
        if direction < 0.0 { distance_along_path } else { (path_length - distance_along_path).max(0.0) };
    if available_distance < MIN_SPLITTER_CHILD_OFFSET_DISTANCE {
        return None;
    }
    let offset_distance =
        random_range(MIN_SPLITTER_CHILD_OFFSET_DISTANCE, MAX_SPLITTER_CHILD_OFFSET_DISTANCE.min(available_distance));
    Some(offset_distance * direction)
}

fn get_level_hit_point_multiplier(level_index: i32) -> f64 {
    let level_offset = level_index.max(0) as f64;
    1.0 + level_offset * HIT_POINT_BONUS_PER_LEVEL
}

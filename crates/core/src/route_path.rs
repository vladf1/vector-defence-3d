//! Route drawing commands plus the sampled motion path entries used by monster movement.
use std::rc::Rc;

use crate::types::Point;
use crate::utils::{angle_between, calculate_distance};

#[derive(Clone, Copy, Debug)]
pub enum RoutePathCommand {
    Line { point: Point },
    Quadratic { control: Point, point: Point },
}

#[derive(Clone, Copy, Debug)]
pub enum PathEntryHeading {
    Line { angle: f64 },
    Quadratic { start: Point, control: Point, end: Point, t_start: f64, t_end: f64 },
}

#[derive(Clone, Copy, Debug)]
pub struct PathEntry {
    pub x: f64,
    pub y: f64,
    pub total_distance: f64,
    pub heading: Option<PathEntryHeading>,
}

impl PathEntry {
    pub fn point(&self) -> Point {
        Point::new(self.x, self.y)
    }
}

/// Monsters share their route's entries (splitter children get a fresh tail).
pub type SharedPath = Rc<[PathEntry]>;

#[derive(Clone, Debug)]
pub struct RouteMotionPath {
    pub start: Point,
    pub commands: Vec<RoutePathCommand>,
    pub entries: SharedPath,
}

struct RoundedTurn {
    index: usize,
    entry: Point,
    control: Point,
    exit: Point,
}

pub fn create_route_motion_path(
    points: &[Point],
    road_turn_radius: f64,
    route_curve_sample_step: f64,
) -> RouteMotionPath {
    let start = points.first().copied().unwrap_or_default();
    let mut entries = vec![PathEntry { x: start.x, y: start.y, total_distance: 0.0, heading: None }];
    let mut commands = Vec::new();
    if points.len() < 2 {
        return RouteMotionPath { start, commands, entries: entries.into() };
    }

    let turns = create_rounded_turns(points, road_turn_radius);
    let mut current = start;
    let mut turn_index = 0;
    for segment_index in 0..points.len() - 1 {
        let next_turn = turns.get(turn_index).filter(|turn| turn.index == segment_index + 1);
        let segment_end = next_turn.map_or(points[segment_index + 1], |turn| turn.entry);
        append_line(&mut commands, &mut entries, current, segment_end);
        current = segment_end;
        if let Some(turn) = next_turn {
            append_quadratic(&mut commands, &mut entries, turn.entry, turn.control, turn.exit, route_curve_sample_step);
            current = turn.exit;
            turn_index += 1;
        }
    }
    RouteMotionPath { start, commands, entries: entries.into() }
}

pub fn create_path_entries(points: &[Point]) -> Vec<PathEntry> {
    let mut total_distance = 0.0;
    points
        .iter()
        .enumerate()
        .map(|(index, point)| {
            if index > 0 {
                total_distance += calculate_distance(points[index - 1], *point);
            }
            PathEntry { x: point.x, y: point.y, total_distance, heading: None }
        })
        .collect()
}

pub fn create_path_entries_from_distance(source: &[PathEntry], start_distance: f64) -> Vec<PathEntry> {
    let path_length = source.last().map_or(0.0, |entry| entry.total_distance);
    let clamped_distance = start_distance.max(0.0).min(path_length);
    let spawn_point = get_point_at_distance(source, clamped_distance);
    let mut entries = vec![PathEntry { x: spawn_point.x, y: spawn_point.y, total_distance: 0.0, heading: None }];

    let first_tail_index = find_path_index_at_distance(source, clamped_distance);
    if first_tail_index >= source.len() {
        return entries;
    }
    for (index, entry) in source.iter().enumerate().skip(first_tail_index) {
        let total_distance = entry.total_distance - clamped_distance;
        if total_distance <= 0.0 && calculate_distance(spawn_point, entry.point()) <= 0.0 {
            continue;
        }
        entries.push(PathEntry {
            x: entry.x,
            y: entry.y,
            total_distance: total_distance.max(0.0),
            heading: if index == first_tail_index {
                create_tail_heading(source, entry, clamped_distance, first_tail_index)
            } else {
                entry.heading
            },
        });
    }
    entries
}

pub fn get_point_at_distance(source: &[PathEntry], distance: f64) -> Point {
    let Some(first) = source.first() else {
        return Point::default();
    };
    if distance <= first.total_distance {
        return first.point();
    }
    for index in 1..source.len() {
        let end = &source[index];
        if distance > end.total_distance {
            continue;
        }
        let start = &source[index - 1];
        let span = end.total_distance - start.total_distance;
        let ratio = if span > 0.0 { (distance - start.total_distance) / span } else { 1.0 };
        return Point::new(start.x + (end.x - start.x) * ratio, start.y + (end.y - start.y) * ratio);
    }
    source[source.len() - 1].point()
}

fn find_path_index_at_distance(source: &[PathEntry], distance: f64) -> usize {
    source.iter().position(|entry| entry.total_distance >= distance).unwrap_or(source.len())
}

pub fn get_path_heading_angle(path: &[PathEntry], distance: f64, target_index: usize) -> f64 {
    if path.len() < 2 {
        return 0.0;
    }
    let end_index = find_local_path_index_at_distance(path, distance, target_index);
    let start = &path[end_index - 1];
    let end = &path[end_index];
    match end.heading {
        None => angle_between(start.point(), end.point()),
        Some(PathEntryHeading::Line { angle }) => angle,
        Some(PathEntryHeading::Quadratic { start: curve_start, control, end: curve_end, t_start, t_end }) => {
            let span = end.total_distance - start.total_distance;
            let ratio = if span > 0.0 { (distance - start.total_distance) / span } else { 1.0 };
            let t = t_start + (t_end - t_start) * ratio;
            get_quadratic_angle(curve_start, control, curve_end, t)
        }
    }
}

fn create_tail_heading(
    source: &[PathEntry],
    entry: &PathEntry,
    clamped_distance: f64,
    first_tail_index: usize,
) -> Option<PathEntryHeading> {
    match entry.heading {
        Some(PathEntryHeading::Quadratic { start, control, end, t_start, t_end }) => {
            let start_distance = first_tail_index.checked_sub(1).map(|index| source[index].total_distance);
            let span = entry.total_distance - start_distance.unwrap_or(entry.total_distance);
            let ratio =
                if span > 0.0 { (clamped_distance - start_distance.unwrap_or(clamped_distance)) / span } else { 1.0 };
            Some(PathEntryHeading::Quadratic {
                start,
                control,
                end,
                t_start: t_start + (t_end - t_start) * ratio,
                t_end,
            })
        }
        heading => heading,
    }
}

fn find_local_path_index_at_distance(path: &[PathEntry], distance: f64, target_index: usize) -> usize {
    let mut index = target_index.max(1).min(path.len() - 1);
    while index > 1 && path[index - 1].total_distance >= distance {
        index -= 1;
    }
    while index < path.len() - 1 && path[index].total_distance < distance {
        index += 1;
    }
    index
}

fn create_rounded_turns(points: &[Point], road_turn_radius: f64) -> Vec<RoundedTurn> {
    let mut turns = Vec::new();
    for index in 1..points.len().saturating_sub(1) {
        let previous = points[index - 1];
        let control = points[index];
        let next = points[index + 1];
        let previous_length = calculate_distance(previous, control);
        let next_length = calculate_distance(control, next);
        let turn_distance = road_turn_radius.min(previous_length * 0.45).min(next_length * 0.45);
        if turn_distance <= 1.0 {
            continue;
        }
        turns.push(RoundedTurn {
            index,
            entry: point_toward(control, previous, turn_distance),
            control,
            exit: point_toward(control, next, turn_distance),
        });
    }
    turns
}

fn point_toward(source: Point, target: Point, distance: f64) -> Point {
    let total = calculate_distance(source, target);
    if total == 0.0 {
        return source;
    }
    let ratio = distance / total;
    Point::new(source.x + (target.x - source.x) * ratio, source.y + (target.y - source.y) * ratio)
}

fn append_line(commands: &mut Vec<RoutePathCommand>, entries: &mut Vec<PathEntry>, start: Point, end: Point) {
    if calculate_distance(start, end) <= 0.0 {
        return;
    }
    commands.push(RoutePathCommand::Line { point: end });
    append_point(entries, end, PathEntryHeading::Line { angle: angle_between(start, end) });
}

fn append_quadratic(
    commands: &mut Vec<RoutePathCommand>,
    entries: &mut Vec<PathEntry>,
    start: Point,
    control: Point,
    end: Point,
    route_curve_sample_step: f64,
) {
    commands.push(RoutePathCommand::Quadratic { control, point: end });
    let estimated_length = calculate_distance(start, control) + calculate_distance(control, end);
    let steps = 4.0_f64.max((estimated_length / route_curve_sample_step).ceil()) as u32;
    for step in 1..=steps {
        let t = step as f64 / steps as f64;
        let t_start = (step - 1) as f64 / steps as f64;
        let inverse = 1.0 - t;
        append_point(
            entries,
            Point::new(
                inverse * inverse * start.x + 2.0 * inverse * t * control.x + t * t * end.x,
                inverse * inverse * start.y + 2.0 * inverse * t * control.y + t * t * end.y,
            ),
            PathEntryHeading::Quadratic { start, control, end, t_start, t_end: t },
        );
    }
}

fn append_point(entries: &mut Vec<PathEntry>, point: Point, heading: PathEntryHeading) {
    let previous = entries[entries.len() - 1];
    let segment_length = calculate_distance(previous.point(), point);
    if segment_length <= 0.0 {
        return;
    }
    entries.push(PathEntry {
        x: point.x,
        y: point.y,
        total_distance: previous.total_distance + segment_length,
        heading: Some(heading),
    });
}

fn get_quadratic_angle(start: Point, control: Point, end: Point, t: f64) -> f64 {
    let inverse = 1.0 - t;
    let dx = 2.0 * inverse * (control.x - start.x) + 2.0 * t * (end.x - control.x);
    let dy = 2.0 * inverse * (control.y - start.y) + 2.0 * t * (end.y - control.y);
    dy.atan2(dx)
}

//! Tower placement geometry and board hit-testing through explicit route/tower inputs.
use crate::route_path::RouteMotionPath;
use crate::types::{FieldBounds, Point};
use crate::utils::{is_within_distance_to_segment, within_distance};

#[derive(Clone, Copy, Debug)]
pub struct PlacementGeometry {
    pub bounds: FieldBounds,
    pub tower_radius: f64,
    pub tower_selection_padding: f64,
    pub min_distance_to_other_towers: f64,
    pub min_distance_to_road: f64,
}

/// `towers` yields tower centers.
pub fn can_place_tower(
    point: Point,
    route_path: Option<&RouteMotionPath>,
    towers: impl IntoIterator<Item = Point>,
    geometry: &PlacementGeometry,
) -> bool {
    let Some(route_path) = route_path else {
        return false;
    };
    let bounds = &geometry.bounds;
    if point.x < bounds.min_x + geometry.tower_radius
        || point.y < bounds.min_y + geometry.tower_radius
        || point.x > bounds.max_x - geometry.tower_radius
        || point.y > bounds.max_y - geometry.tower_radius
    {
        return false;
    }
    if towers.into_iter().any(|tower| within_distance(point, tower, geometry.min_distance_to_other_towers)) {
        return false;
    }
    let entries = &route_path.entries;
    for pair in entries.windows(2) {
        if is_within_distance_to_segment(point, pair[0].point(), pair[1].point(), geometry.min_distance_to_road) {
            return false;
        }
    }
    true
}

/// Index of the topmost (last placed) tower under `point`.
pub fn find_tower_at_point(
    point: Point,
    towers: &[Point],
    tower_radius: f64,
    tower_selection_padding: f64,
) -> Option<usize> {
    (0..towers.len()).rev().find(|&index| within_distance(point, towers[index], tower_radius + tower_selection_padding))
}

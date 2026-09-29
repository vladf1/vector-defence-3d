//! Splits a monster outline into readable glass shards with kinked random cracks.
use crate::rng;
use crate::types::Point;

#[derive(Clone, Copy, Debug)]
pub struct PolygonShardSplitterConfig {
    pub min_shard_count: usize,
    pub max_shard_count: usize,
    pub max_consecutive_split_failures: usize,
    pub crack_attempts_per_shard: usize,
    pub min_boundary_separation_ratio: f64,
    pub boundary_endpoint_inset_ratio: f64,
    pub min_shard_area_ratio: f64,
    pub max_shard_area_ratio: f64,
    pub max_split_child_area_ratio: f64,
    pub max_shard_vertices: usize,
    pub kink_offset_ratio: f64,
    pub point_merge_distance: f64,
    pub area_tolerance_ratio: f64,
    /// The random source (the simulation RNG unless a render script swaps it).
    pub random: fn() -> f64,
}

impl PolygonShardSplitterConfig {
    /// The shard counts plus the tuned defaults of `createPolygonShardSplitterConfig(...)`.
    pub const fn with_shard_counts(min_shard_count: usize, max_shard_count: usize) -> Self {
        PolygonShardSplitterConfig {
            min_shard_count,
            max_shard_count,
            max_consecutive_split_failures: 120,
            crack_attempts_per_shard: 12,
            min_boundary_separation_ratio: 0.2,
            boundary_endpoint_inset_ratio: 0.055,
            min_shard_area_ratio: 0.034,
            max_shard_area_ratio: 0.56,
            max_split_child_area_ratio: 0.72,
            max_shard_vertices: 11,
            kink_offset_ratio: 0.18,
            point_merge_distance: 0.025,
            area_tolerance_ratio: 0.035,
            random: rng::random,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Shard {
    pub vertices: Vec<Point>,
}

struct WorkingShard {
    vertices: Vec<Point>,
    area: f64,
}

#[derive(Clone, Copy)]
struct BoundarySample {
    point: Point,
    edge_index: usize,
    distance: f64,
    ratio: f64,
}

#[derive(Clone, Copy)]
struct EdgeMetric {
    length: f64,
    distance_at_start: f64,
}

struct EdgeMetrics {
    edges: Vec<EdgeMetric>,
    perimeter: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct PolygonShardSplitter {
    config: PolygonShardSplitterConfig,
}

impl PolygonShardSplitter {
    pub const fn new(config: PolygonShardSplitterConfig) -> Self {
        PolygonShardSplitter { config }
    }

    pub fn config(&self) -> &PolygonShardSplitterConfig {
        &self.config
    }

    fn random(&self) -> f64 {
        (self.config.random)()
    }

    pub fn split_into_shards(&self, outline: &[Point]) -> Vec<Shard> {
        let source_polygon = outline.to_vec();
        if source_polygon.len() < 3 {
            return Vec::new();
        }
        let source_area = polygon_area(&source_polygon);
        if source_area == 0.0 {
            return Vec::new();
        }

        let target_shard_count = self.config.min_shard_count
            + (self.random() * (self.config.max_shard_count - self.config.min_shard_count + 1) as f64).floor() as usize;
        let mut shards = vec![WorkingShard { vertices: source_polygon, area: source_area }];
        let mut consecutive_failures = 0;

        while (shards.len() < target_shard_count
            || max_shard_area_ratio(&shards, source_area) > self.config.max_shard_area_ratio)
            && shards.len() < self.config.max_shard_count
            && consecutive_failures < self.config.max_consecutive_split_failures
        {
            let Some(shard_index) = self.choose_shard_index(&shards, source_area) else {
                break;
            };
            let Some((first, second)) = self.try_split_shard(&shards[shard_index], source_area) else {
                consecutive_failures += 1;
                continue;
            };
            shards.splice(shard_index..=shard_index, [first, second]);
            consecutive_failures = 0;
        }

        shards.into_iter().map(|shard| Shard { vertices: shard.vertices }).collect()
    }

    fn choose_shard_index(&self, shards: &[WorkingShard], source_area: f64) -> Option<usize> {
        let min_candidate_area = source_area * self.config.min_shard_area_ratio * 2.15;
        let oversized_area = source_area * self.config.max_shard_area_ratio;
        let mut has_oversized_candidate = false;
        let mut total_weight = 0.0;
        let mut last_candidate_index = None;

        for (index, shard) in shards.iter().enumerate() {
            let area = shard.area;
            if area < min_candidate_area {
                continue;
            }
            let is_oversized = area > oversized_area;
            if is_oversized && !has_oversized_candidate {
                has_oversized_candidate = true;
                total_weight = 0.0;
                last_candidate_index = None;
            }
            if has_oversized_candidate && !is_oversized {
                continue;
            }
            total_weight += area * area;
            last_candidate_index = Some(index);
        }

        let last_candidate_index = last_candidate_index?;
        let mut threshold = self.random() * total_weight;
        for (index, shard) in shards.iter().enumerate() {
            let area = shard.area;
            if area < min_candidate_area || (has_oversized_candidate && area <= oversized_area) {
                continue;
            }
            threshold -= area * area;
            if threshold <= 0.0 {
                return Some(index);
            }
        }
        Some(last_candidate_index)
    }

    fn try_split_shard(&self, shard: &WorkingShard, source_area: f64) -> Option<(WorkingShard, WorkingShard)> {
        let polygon = &shard.vertices;
        let edge_metrics = create_edge_metrics(polygon);
        if edge_metrics.perimeter == 0.0 {
            return None;
        }

        for _ in 0..self.config.crack_attempts_per_shard {
            let Some((start, end)) = self.sample_boundary_pair(polygon, &edge_metrics) else {
                continue;
            };
            let kink_point = self.create_kink_point(start.point, end.point);
            if let Some(split) = self.try_crack_path(shard, source_area, &start, &end, Some(kink_point)) {
                return Some(split);
            }
            if let Some(split) = self.try_crack_path(shard, source_area, &start, &end, None) {
                return Some(split);
            }
        }
        None
    }

    fn try_crack_path(
        &self,
        shard: &WorkingShard,
        source_area: f64,
        start: &BoundarySample,
        end: &BoundarySample,
        kink_point: Option<Point>,
    ) -> Option<(WorkingShard, WorkingShard)> {
        let polygon = &shard.vertices;
        if !is_valid_crack_path(polygon, start, end, kink_point, self.config.point_merge_distance) {
            return None;
        }
        let (first, second) = split_polygon_with_crack(polygon, start, end, kink_point)?;
        let first_area = polygon_area(&first);
        let second_area = polygon_area(&second);
        let area_delta_ratio = (first_area + second_area - shard.area).abs() / shard.area;
        if area_delta_ratio > self.config.area_tolerance_ratio {
            return None;
        }
        let largest_child_area_ratio = first_area.max(second_area) / shard.area;
        if largest_child_area_ratio > self.config.max_split_child_area_ratio {
            return None;
        }
        if !is_readable_shard(&first, first_area, source_area, &self.config)
            || !is_readable_shard(&second, second_area, source_area, &self.config)
        {
            return None;
        }
        Some((WorkingShard { vertices: first, area: first_area }, WorkingShard { vertices: second, area: second_area }))
    }

    fn sample_boundary_pair(
        &self,
        polygon: &[Point],
        edge_metrics: &EdgeMetrics,
    ) -> Option<(BoundarySample, BoundarySample)> {
        let max_attempts = self.config.crack_attempts_per_shard * 3;
        let inset = self.config.boundary_endpoint_inset_ratio;
        for _ in 0..max_attempts {
            let start =
                sample_boundary_at_distance(polygon, &edge_metrics.edges, self.random() * edge_metrics.perimeter);
            let end = sample_boundary_at_distance(polygon, &edge_metrics.edges, self.random() * edge_metrics.perimeter);
            if start.edge_index == end.edge_index {
                continue;
            }
            if start.ratio < inset || start.ratio > 1.0 - inset || end.ratio < inset || end.ratio > 1.0 - inset {
                continue;
            }
            let direct_distance = (start.distance - end.distance).abs();
            let separation = direct_distance.min(edge_metrics.perimeter - direct_distance);
            if separation < edge_metrics.perimeter * self.config.min_boundary_separation_ratio {
                continue;
            }
            return Some((start, end));
        }
        None
    }

    fn create_kink_point(&self, start: Point, end: Point) -> Point {
        let dx = end.x - start.x;
        let dy = end.y - start.y;
        let length = dx.hypot(dy);
        let normal_x = if length == 0.0 { 0.0 } else { -dy / length };
        let normal_y = if length == 0.0 { 0.0 } else { dx / length };
        let ratio = 0.5 + (self.random() - 0.5) * 0.18;
        let offset = (self.random() - 0.5) * length * self.config.kink_offset_ratio;
        Point::new(start.x + dx * ratio + normal_x * offset, start.y + dy * ratio + normal_y * offset)
    }
}

fn is_valid_crack_path(
    polygon: &[Point],
    start: &BoundarySample,
    end: &BoundarySample,
    kink_point: Option<Point>,
    epsilon: f64,
) -> bool {
    let Some(kink_point) = kink_point else {
        return is_valid_crack_segment(
            start.point,
            end.point,
            polygon,
            Some(start.edge_index),
            Some(end.edge_index),
            epsilon,
        );
    };
    point_inside_polygon(kink_point.x, kink_point.y, polygon, epsilon)
        && is_valid_crack_segment(start.point, kink_point, polygon, Some(start.edge_index), None, epsilon)
        && is_valid_crack_segment(kink_point, end.point, polygon, None, Some(end.edge_index), epsilon)
}

fn is_valid_crack_segment(
    start: Point,
    end: Point,
    polygon: &[Point],
    skipped_edge_index_a: Option<usize>,
    skipped_edge_index_b: Option<usize>,
    epsilon: f64,
) -> bool {
    if !point_inside_polygon((start.x + end.x) / 2.0, (start.y + end.y) / 2.0, polygon, epsilon) {
        return false;
    }
    for edge_index in 0..polygon.len() {
        if Some(edge_index) == skipped_edge_index_a || Some(edge_index) == skipped_edge_index_b {
            continue;
        }
        if segments_intersect(start, end, polygon[edge_index], polygon[(edge_index + 1) % polygon.len()], epsilon) {
            return false;
        }
    }
    true
}

fn split_polygon_with_crack(
    polygon: &[Point],
    start: &BoundarySample,
    end: &BoundarySample,
    kink_point: Option<Point>,
) -> Option<(Vec<Point>, Vec<Point>)> {
    let mut first_shard = boundary_chain(polygon, start, end);
    let end_to_start_boundary = boundary_chain(polygon, end, start);
    if let Some(kink_point) = kink_point {
        first_shard.push(kink_point);
    }

    let mut second_shard = vec![start.point];
    if let Some(kink_point) = kink_point {
        second_shard.push(kink_point);
    }
    second_shard.push(end.point);
    if end_to_start_boundary.len() > 2 {
        second_shard.extend_from_slice(&end_to_start_boundary[1..end_to_start_boundary.len() - 1]);
    }

    if first_shard.len() < 3 || second_shard.len() < 3 {
        return None;
    }
    Some((first_shard, second_shard))
}

fn boundary_chain(polygon: &[Point], start: &BoundarySample, end: &BoundarySample) -> Vec<Point> {
    let mut chain = vec![start.point];
    let mut index = (start.edge_index + 1) % polygon.len();
    let stop_index = (end.edge_index + 1) % polygon.len();
    let mut guard = 0;
    while index != stop_index && guard <= polygon.len() {
        chain.push(polygon[index]);
        index = (index + 1) % polygon.len();
        guard += 1;
    }
    chain.push(end.point);
    chain
}

fn create_edge_metrics(polygon: &[Point]) -> EdgeMetrics {
    let mut edges = Vec::with_capacity(polygon.len());
    let mut perimeter = 0.0;
    for index in 0..polygon.len() {
        let start = polygon[index];
        let end = polygon[(index + 1) % polygon.len()];
        let length = (end.x - start.x).hypot(end.y - start.y);
        edges.push(EdgeMetric { length, distance_at_start: perimeter });
        perimeter += length;
    }
    EdgeMetrics { edges, perimeter }
}

fn sample_boundary_at_distance(polygon: &[Point], edge_metrics: &[EdgeMetric], distance: f64) -> BoundarySample {
    for (index, edge) in edge_metrics.iter().enumerate() {
        if distance <= edge.distance_at_start + edge.length {
            let start = polygon[index];
            let end = polygon[(index + 1) % polygon.len()];
            let ratio = if edge.length == 0.0 { 0.0 } else { (distance - edge.distance_at_start) / edge.length };
            return BoundarySample {
                point: Point::new(start.x + (end.x - start.x) * ratio, start.y + (end.y - start.y) * ratio),
                edge_index: index,
                distance,
                ratio,
            };
        }
    }
    BoundarySample { point: polygon[0], edge_index: 0, distance: 0.0, ratio: 0.0 }
}

fn is_readable_shard(shard: &[Point], area: f64, source_area: f64, config: &PolygonShardSplitterConfig) -> bool {
    shard.len() >= 3 && shard.len() <= config.max_shard_vertices && area >= source_area * config.min_shard_area_ratio
}

fn max_shard_area_ratio(shards: &[WorkingShard], source_area: f64) -> f64 {
    let mut max_area = 0.0_f64;
    for shard in shards {
        max_area = max_area.max(shard.area);
    }
    max_area / source_area
}

pub fn polygon_area(polygon: &[Point]) -> f64 {
    let mut total = 0.0;
    for index in 0..polygon.len() {
        let current = polygon[index];
        let next = polygon[(index + 1) % polygon.len()];
        total += current.x * next.y - next.x * current.y;
    }
    total.abs() / 2.0
}

fn point_inside_polygon(point_x: f64, point_y: f64, polygon: &[Point], epsilon: f64) -> bool {
    let mut inside = false;
    let mut previous_index = polygon.len() - 1;
    for index in 0..polygon.len() {
        let current = polygon[index];
        let previous = polygon[previous_index];
        if point_on_segment_coordinates(point_x, point_y, previous, current, epsilon) {
            return true;
        }
        if (current.y > point_y) != (previous.y > point_y)
            && point_x < ((previous.x - current.x) * (point_y - current.y)) / (previous.y - current.y) + current.x
        {
            inside = !inside;
        }
        previous_index = index;
    }
    inside
}

fn segments_intersect(a: Point, b: Point, c: Point, d: Point, epsilon: f64) -> bool {
    let direction_a = orientation(a, b, c);
    let direction_b = orientation(a, b, d);
    let direction_c = orientation(c, d, a);
    let direction_d = orientation(c, d, b);

    if direction_a.abs() <= epsilon && point_on_segment(c, a, b, epsilon) {
        return true;
    }
    if direction_b.abs() <= epsilon && point_on_segment(d, a, b, epsilon) {
        return true;
    }
    if direction_c.abs() <= epsilon && point_on_segment(a, c, d, epsilon) {
        return true;
    }
    if direction_d.abs() <= epsilon && point_on_segment(b, c, d, epsilon) {
        return true;
    }
    (direction_a > 0.0) != (direction_b > 0.0) && (direction_c > 0.0) != (direction_d > 0.0)
}

fn orientation(a: Point, b: Point, c: Point) -> f64 {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}

fn point_on_segment(point: Point, start: Point, end: Point, epsilon: f64) -> bool {
    orientation(start, end, point).abs() <= epsilon
        && point.x >= start.x.min(end.x) - epsilon
        && point.x <= start.x.max(end.x) + epsilon
        && point.y >= start.y.min(end.y) - epsilon
        && point.y <= start.y.max(end.y) + epsilon
}

fn point_on_segment_coordinates(point_x: f64, point_y: f64, start: Point, end: Point, epsilon: f64) -> bool {
    ((end.x - start.x) * (point_y - start.y) - (end.y - start.y) * (point_x - start.x)).abs() <= epsilon
        && point_x >= start.x.min(end.x) - epsilon
        && point_x <= start.x.max(end.x) + epsilon
        && point_y >= start.y.min(end.y) - epsilon
        && point_y <= start.y.max(end.y) + epsilon
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shards_cover_the_outline_area() {
        rng::seed(11);
        let splitter = PolygonShardSplitter::new(PolygonShardSplitterConfig::with_shard_counts(5, 11));
        let outline = [Point::new(-6.5, -6.5), Point::new(6.5, -6.5), Point::new(6.5, 6.5), Point::new(-6.5, 6.5)];
        for _ in 0..50 {
            let shards = splitter.split_into_shards(&outline);
            assert!(shards.len() >= 2 && shards.len() <= 11);
            let total: f64 = shards.iter().map(|shard| polygon_area(&shard.vertices)).sum();
            assert!((total - polygon_area(&outline)).abs() / polygon_area(&outline) < 0.2);
            assert!(shards.iter().all(|shard| shard.vertices.len() >= 3 && shard.vertices.len() <= 11));
        }
    }
}

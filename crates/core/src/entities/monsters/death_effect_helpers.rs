use crate::entities::Particle;
use crate::entities::monsters::polygon_shard_splitter::PolygonShardSplitter;
use crate::types::{Color, Point};
use crate::update::UpdateResult;
use crate::utils::random_range;

pub fn rotate_point(point: Point, angle: f64) -> Point {
    Point::new(point.x * angle.cos() - point.y * angle.sin(), point.x * angle.sin() + point.y * angle.cos())
}

pub fn point_on_radius(angle: f64, radius: f64) -> Point {
    Point::new(angle.cos() * radius, angle.sin() * radius)
}

pub fn create_death_effect_origin(
    radius: f64,
    x_min_ratio: f64,
    x_max_ratio: f64,
    y_min_ratio: f64,
    y_max_ratio: f64,
) -> Point {
    let x = random_range(radius * x_min_ratio, radius * x_max_ratio);
    let y = random_range(radius * y_min_ratio, radius * y_max_ratio);
    Point::new(x, y)
}

/// Where death shards start: the monster's visual (shaken) position and its color.
#[derive(Clone, Copy, Debug)]
pub struct ShardSource {
    pub visual_x: f64,
    pub visual_y: f64,
    pub color: Color,
}

/// Breaks `outline` into glass shards; a full particle budget skips the polygon splitting.
#[allow(clippy::too_many_arguments)]
pub fn create_polygon_shard_particles(
    result: &mut UpdateResult,
    source: ShardSource,
    outline: &[Point],
    origin: Point,
    rotation: f64,
    speed_min_per_second: f64,
    speed_max_per_second: f64,
    initial_separation: f64,
    splitter: &PolygonShardSplitter,
) {
    if result.remaining_particle_capacity() == 0 {
        return;
    }
    for shard in splitter.split_into_shards(outline) {
        if result.remaining_particle_capacity() == 0 {
            break;
        }
        let speed = random_range(speed_min_per_second, speed_max_per_second);
        result.add_particle(Particle::glass_shard(
            source.visual_x,
            source.visual_y,
            source.color,
            shard.vertices,
            origin,
            rotation,
            speed,
            initial_separation,
        ));
    }
}

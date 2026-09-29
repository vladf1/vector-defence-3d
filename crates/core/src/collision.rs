//! Active-monster swept-collision index (uniform grid) plus its linear comparison implementation.
use std::cell::{Cell, RefCell};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CircleSweep {
    pub previous_x: f64,
    pub previous_y: f64,
    pub x: f64,
    pub y: f64,
    pub radius: f64,
}

/// A target that can be swept against; inactive targets (removed or dead) never collide.
pub trait ActiveCircleSweep {
    fn sweep(&self) -> CircleSweep;
    fn is_active(&self) -> bool;
}

/// `target` indexes the slice the query ran against.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CircleSweepCollision {
    pub target: usize,
    pub time: f64,
    pub x: f64,
    pub y: f64,
}

pub trait ActiveCircleSweepCollisionQuery<T: ActiveCircleSweep> {
    fn find_earliest_collision(&self, source: &CircleSweep, targets: &[T]) -> Option<CircleSweepCollision>;
}

/// Checks every target; used by debug pages and as the reference implementation.
pub struct LinearActiveCircleSweepCollisionIndex;

impl<T: ActiveCircleSweep> ActiveCircleSweepCollisionQuery<T> for LinearActiveCircleSweepCollisionIndex {
    fn find_earliest_collision(&self, source: &CircleSweep, targets: &[T]) -> Option<CircleSweepCollision> {
        find_earliest_active_circle_sweep_collision(source, targets)
    }
}

/// A uniform grid over the indexed targets' swept bounds. Cells are a dense array covering
/// just those bounds (rebuilt every step), so no hashing is needed; queries outside it find nothing.
pub struct ActiveCircleSweepCollisionIndex {
    cell_size: f64,
    /// Cell coordinates of `cells[0]`, and the grid size in cells.
    origin_x: i64,
    origin_y: i64,
    columns: i64,
    rows: i64,
    cells: Vec<Vec<usize>>,
    query_marks: RefCell<Vec<u32>>,
    query_id: Cell<u32>,
}

impl ActiveCircleSweepCollisionIndex {
    pub fn new(cell_size: f64) -> Self {
        ActiveCircleSweepCollisionIndex {
            cell_size,
            origin_x: 0,
            origin_y: 0,
            columns: 0,
            rows: 0,
            cells: Vec::new(),
            query_marks: RefCell::new(Vec::new()),
            query_id: Cell::new(0),
        }
    }

    /// Indexes `targets`; later queries must pass the same slice (targets may change state, not order).
    pub fn rebuild<T: ActiveCircleSweep>(&mut self, targets: &[T]) {
        for cell in &mut self.cells {
            cell.clear();
        }
        {
            let mut marks = self.query_marks.borrow_mut();
            marks.clear();
            marks.resize(targets.len(), 0);
        }
        self.query_id.set(0);

        let (mut min_x, mut min_y, mut max_x, mut max_y) = (i64::MAX, i64::MAX, i64::MIN, i64::MIN);
        for target in targets.iter().filter(|target| target.is_active()) {
            let (left, top, right, bottom) = self.cell_bounds(&target.sweep());
            min_x = min_x.min(left);
            min_y = min_y.min(top);
            max_x = max_x.max(right);
            max_y = max_y.max(bottom);
        }
        if min_x > max_x {
            self.columns = 0;
            self.rows = 0;
            return;
        }
        self.origin_x = min_x;
        self.origin_y = min_y;
        self.columns = max_x - min_x + 1;
        self.rows = max_y - min_y + 1;
        let cell_count = (self.columns * self.rows) as usize;
        if self.cells.len() < cell_count {
            self.cells.resize_with(cell_count, Vec::new);
        }

        for (target_index, target) in targets.iter().enumerate() {
            if !target.is_active() {
                continue;
            }
            let (left, top, right, bottom) = self.cell_bounds(&target.sweep());
            for cell_y in top..=bottom {
                for cell_x in left..=right {
                    let index = self.cell_index(cell_x, cell_y);
                    self.cells[index].push(target_index);
                }
            }
        }
    }

    fn to_cell(&self, value: f64) -> i64 {
        (value / self.cell_size).floor() as i64
    }

    fn cell_bounds(&self, sweep: &CircleSweep) -> (i64, i64, i64, i64) {
        (
            self.to_cell(sweep.previous_x.min(sweep.x) - sweep.radius),
            self.to_cell(sweep.previous_y.min(sweep.y) - sweep.radius),
            self.to_cell(sweep.previous_x.max(sweep.x) + sweep.radius),
            self.to_cell(sweep.previous_y.max(sweep.y) + sweep.radius),
        )
    }

    fn cell_index(&self, cell_x: i64, cell_y: i64) -> usize {
        ((cell_y - self.origin_y) * self.columns + (cell_x - self.origin_x)) as usize
    }
}

impl<T: ActiveCircleSweep> ActiveCircleSweepCollisionQuery<T> for ActiveCircleSweepCollisionIndex {
    fn find_earliest_collision(&self, source: &CircleSweep, targets: &[T]) -> Option<CircleSweepCollision> {
        if self.columns == 0 {
            return None;
        }
        let query_id = self.query_id.get().wrapping_add(1).max(1);
        self.query_id.set(query_id);
        let mut marks = self.query_marks.borrow_mut();
        if marks.len() < targets.len() {
            marks.resize(targets.len(), 0);
        }
        let mut hit: Option<(usize, f64)> = None;
        let (left, top, right, bottom) = self.cell_bounds(source);
        let left = left.max(self.origin_x);
        let top = top.max(self.origin_y);
        let right = right.min(self.origin_x + self.columns - 1);
        let bottom = bottom.min(self.origin_y + self.rows - 1);

        for cell_y in top..=bottom {
            for cell_x in left..=right {
                for &target_index in &self.cells[self.cell_index(cell_x, cell_y)] {
                    if marks[target_index] == query_id {
                        continue;
                    }
                    marks[target_index] = query_id;
                    let target = &targets[target_index];
                    if !target.is_active() {
                        continue;
                    }
                    if let Some(time) = get_swept_circle_collision_time(source, &target.sweep()) {
                        // Ties go to the earlier target, like the linear scan.
                        let better = match hit {
                            None => true,
                            Some((hit_index, hit_time)) => {
                                time < hit_time || (time == hit_time && target_index < hit_index)
                            }
                        };
                        if better {
                            hit = Some((target_index, time));
                        }
                    }
                }
            }
        }
        hit.map(|(target, time)| create_collision(source, target, time))
    }
}

pub fn find_earliest_active_circle_sweep_collision<T: ActiveCircleSweep>(
    source: &CircleSweep,
    targets: &[T],
) -> Option<CircleSweepCollision> {
    let mut hit: Option<(usize, f64)> = None;
    for (index, target) in targets.iter().enumerate() {
        if !target.is_active() {
            continue;
        }
        if let Some(time) = get_swept_circle_collision_time(source, &target.sweep())
            && hit.is_none_or(|(_, hit_time)| time < hit_time)
        {
            hit = Some((index, time));
        }
    }
    hit.map(|(target, time)| create_collision(source, target, time))
}

fn create_collision(source: &CircleSweep, target: usize, time: f64) -> CircleSweepCollision {
    CircleSweepCollision {
        target,
        time,
        x: source.previous_x + (source.x - source.previous_x) * time,
        y: source.previous_y + (source.y - source.previous_y) * time,
    }
}

/// Returns the first contact time from 0 to 1, or `None` when the sweeps do not collide.
pub fn get_swept_circle_collision_time(source: &CircleSweep, target: &CircleSweep) -> Option<f64> {
    // Make the target stationary by subtracting its frame movement from the source movement.
    let start_x = source.previous_x - target.previous_x;
    let start_y = source.previous_y - target.previous_y;
    let end_x = source.x - target.x;
    let end_y = source.y - target.y;
    let combined_radius = source.radius + target.radius;
    if !relative_sweep_bounds_overlap(start_x, start_y, end_x, end_y, combined_radius) {
        return None;
    }
    let movement_x = end_x - start_x;
    let movement_y = end_y - start_y;
    let c = start_x * start_x + start_y * start_y - combined_radius * combined_radius;
    if c <= 0.0 {
        return Some(0.0);
    }
    let a = movement_x * movement_x + movement_y * movement_y;
    if a == 0.0 {
        return None;
    }
    let b = start_x * movement_x + start_y * movement_y;
    let discriminant = b * b - a * c;
    if discriminant < 0.0 {
        return None;
    }
    let time = (-b - discriminant.sqrt()) / a;
    (0.0..=1.0).contains(&time).then_some(time)
}

fn relative_sweep_bounds_overlap(start_x: f64, start_y: f64, end_x: f64, end_y: f64, radius: f64) -> bool {
    !((start_x < -radius && end_x < -radius)
        || (start_x > radius && end_x > radius)
        || (start_y < -radius && end_y < -radius)
        || (start_y > radius && end_y > radius))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng;

    struct Target(CircleSweep, bool);

    impl ActiveCircleSweep for Target {
        fn sweep(&self) -> CircleSweep {
            self.0
        }
        fn is_active(&self) -> bool {
            self.1
        }
    }

    fn random_sweep(radius: f64) -> CircleSweep {
        let x = rng::random_range(0.0, 400.0);
        let y = rng::random_range(0.0, 300.0);
        CircleSweep {
            previous_x: x + rng::random_range(-20.0, 20.0),
            previous_y: y + rng::random_range(-20.0, 20.0),
            x,
            y,
            radius,
        }
    }

    #[test]
    fn grid_matches_linear_scan() {
        rng::seed(7);
        for _ in 0..50 {
            let targets: Vec<Target> =
                (0..60).map(|index| Target(random_sweep(rng::random_range(4.0, 10.0)), index % 7 != 0)).collect();
            let mut grid = ActiveCircleSweepCollisionIndex::new(64.0);
            grid.rebuild(&targets);
            for _ in 0..40 {
                let source = random_sweep(2.0);
                let linear = LinearActiveCircleSweepCollisionIndex.find_earliest_collision(&source, &targets);
                let indexed = grid.find_earliest_collision(&source, &targets);
                assert_eq!(linear, indexed);
            }
        }
    }
}

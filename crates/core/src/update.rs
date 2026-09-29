//! The per-update read model and the `UpdateResult` accumulator through which monsters, towers,
//! projectiles, and drones report lifecycle outcomes, spawned entities, effects, and sounds.
use crate::audio::{AudioCue, SoundEvent};
use crate::collision::{
    ActiveCircleSweepCollisionQuery, CircleSweep, CircleSweepCollision, LinearActiveCircleSweepCollisionIndex,
};
use crate::entities::{DroneRef, Link, Missile, MonsterRef, Particle, Projectile};
use crate::small_map::SmallMap;
use crate::types::FieldBounds;

/// Drone counts per assigned monster, keyed by monster id.
pub type DroneAssignments = SmallMap<u32, i32>;

pub struct UpdateContext<'a> {
    pub delta_seconds: f64,
    pub field_width: f64,
    pub field_height: f64,
    pub field_bounds: FieldBounds,
    pub active_monsters: &'a [MonsterRef],
    /// Indexed over `active_monsters`.
    pub monster_collision_index: &'a dyn ActiveCircleSweepCollisionQuery<MonsterRef>,
    pub active_drones: &'a [DroneRef],
    pub drone_assignments: &'a DroneAssignments,
}

impl UpdateContext<'_> {
    pub fn find_earliest_collision(&self, source: &CircleSweep) -> Option<(&MonsterRef, CircleSweepCollision)> {
        self.monster_collision_index
            .find_earliest_collision(source, self.active_monsters)
            .map(|collision| (&self.active_monsters[collision.target], collision))
    }
}

/// Owns everything an `UpdateContext` borrows, for entities updated outside a `Game`
/// (the tower sheet and tests). Collisions use the linear reference query.
pub struct StandaloneUpdateContext {
    pub field_width: f64,
    pub field_height: f64,
    pub field_bounds: FieldBounds,
    pub active_monsters: Vec<MonsterRef>,
    pub active_drones: Vec<DroneRef>,
    pub drone_assignments: DroneAssignments,
}

impl StandaloneUpdateContext {
    pub fn new(
        field_width: f64,
        field_height: f64,
        field_bounds: FieldBounds,
        active_monsters: Vec<MonsterRef>,
    ) -> Self {
        StandaloneUpdateContext {
            field_width,
            field_height,
            field_bounds,
            active_monsters,
            active_drones: Vec::new(),
            drone_assignments: DroneAssignments::new(),
        }
    }

    pub fn context(&self, delta_seconds: f64) -> UpdateContext<'_> {
        UpdateContext {
            delta_seconds,
            field_width: self.field_width,
            field_height: self.field_height,
            field_bounds: self.field_bounds,
            active_monsters: &self.active_monsters,
            monster_collision_index: &LinearActiveCircleSweepCollisionIndex,
            active_drones: &self.active_drones,
            drone_assignments: &self.drone_assignments,
        }
    }
}

pub struct UpdateResult {
    /// `usize::MAX` means unlimited.
    pub particle_limit: usize,
    pub link_limit: usize,
    pub killed_monsters: Vec<MonsterRef>,
    pub escaped_monsters: Vec<MonsterRef>,
    pub particles: Vec<Particle>,
    pub links: Vec<Link>,
    pub drones: Vec<DroneRef>,
    pub projectiles: Vec<Projectile>,
    pub missiles: Vec<Missile>,
    pub sounds: Vec<SoundEvent>,
}

impl Default for UpdateResult {
    fn default() -> Self {
        UpdateResult {
            particle_limit: usize::MAX,
            link_limit: usize::MAX,
            killed_monsters: Vec::new(),
            escaped_monsters: Vec::new(),
            particles: Vec::new(),
            links: Vec::new(),
            drones: Vec::new(),
            projectiles: Vec::new(),
            missiles: Vec::new(),
            sounds: Vec::new(),
        }
    }
}

impl UpdateResult {
    pub fn new() -> Self {
        UpdateResult::default()
    }

    pub fn remaining_particle_capacity(&self) -> usize {
        self.particle_limit.saturating_sub(self.particles.len())
    }

    pub fn remaining_link_capacity(&self) -> usize {
        self.link_limit.saturating_sub(self.links.len())
    }

    pub fn add_killed_monster(&mut self, monster: MonsterRef) {
        self.killed_monsters.push(monster);
    }

    pub fn add_escaped_monster(&mut self, monster: MonsterRef) {
        self.escaped_monsters.push(monster);
    }

    pub fn add_particle(&mut self, particle: Particle) {
        if self.particles.len() < self.particle_limit {
            self.particles.push(particle);
        }
    }

    pub fn add_particles(&mut self, particles: impl IntoIterator<Item = Particle>) {
        for particle in particles {
            self.add_particle(particle);
        }
    }

    pub fn add_link(&mut self, link: Link) {
        if self.remaining_link_capacity() > 0 {
            self.links.push(link);
        }
    }

    pub fn add_drone(&mut self, drone: DroneRef) {
        self.drones.push(drone);
    }

    pub fn add_projectile(&mut self, projectile: Projectile) {
        self.projectiles.push(projectile);
    }

    pub fn add_missile(&mut self, missile: Missile) {
        self.missiles.push(missile);
    }

    pub fn play_sound(&mut self, cue: AudioCue, pan_x: Option<f64>, intensity: Option<f64>) {
        self.sounds.push(SoundEvent { cue, pan_x, intensity });
    }

    pub fn clear(&mut self) {
        self.particle_limit = usize::MAX;
        self.link_limit = usize::MAX;
        self.killed_monsters.clear();
        self.escaped_monsters.clear();
        self.particles.clear();
        self.links.clear();
        self.drones.clear();
        self.projectiles.clear();
        self.missiles.clear();
        self.sounds.clear();
    }
}

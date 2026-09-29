//! Per-level mutable runtime collections: monsters, towers, shots, effects, placement state,
//! money, wave counters, and the route path.
use crate::entities::{DroneRef, Link, Missile, MonsterRef, Particle, Projectile, TowerRef};
use crate::route_path::{RouteMotionPath, create_route_motion_path};
use crate::types::{LevelData, Point, TowerKind, WaveData};
use crate::utils::compact_in_place;

pub struct LevelRuntime {
    /// The level being played (a copy of the campaign entry), or `None` before the first start.
    pub level: Option<LevelData>,
    pub route_path: Option<RouteMotionPath>,
    pub money: i32,
    pub escapes_left: i32,
    pub spawn_delay: f64,
    pub spawn_cooldown: f64,
    pub spawn_index: usize,
    pub spawned_monsters: u32,
    pub current_wave_index: usize,
    pub wave_spawned_monsters: u32,
    pub towers: Vec<TowerRef>,
    pub monsters: Vec<MonsterRef>,
    pub drones: Vec<DroneRef>,
    pub projectiles: Vec<Projectile>,
    pub missiles: Vec<Missile>,
    pub particles: Vec<Particle>,
    pub links: Vec<Link>,
    pub selected_tower: Option<TowerRef>,
    pub placing_tower: Option<TowerKind>,
    pub pointer: Option<Point>,
    pub win_delay: f64,
}

impl Default for LevelRuntime {
    fn default() -> Self {
        LevelRuntime::empty()
    }
}

impl LevelRuntime {
    /// The runtime before any level has started.
    pub fn empty() -> LevelRuntime {
        LevelRuntime {
            level: None,
            route_path: None,
            money: 0,
            escapes_left: 0,
            spawn_delay: 0.0,
            spawn_cooldown: 0.0,
            spawn_index: 0,
            spawned_monsters: 0,
            current_wave_index: 0,
            wave_spawned_monsters: 0,
            towers: Vec::new(),
            monsters: Vec::new(),
            drones: Vec::new(),
            projectiles: Vec::new(),
            missiles: Vec::new(),
            particles: Vec::new(),
            links: Vec::new(),
            selected_tower: None,
            placing_tower: None,
            pointer: None,
            win_delay: 0.0,
        }
    }

    pub fn new(level: LevelData, road_turn_radius: f64, route_curve_sample_step: f64) -> LevelRuntime {
        let route_path = create_route_motion_path(&level.points, road_turn_radius, route_curve_sample_step);
        LevelRuntime {
            money: level.starting_money,
            escapes_left: level.allow_escape,
            spawn_delay: level.waves.first().map_or(0.0, |wave| wave.build_time),
            spawn_cooldown: 0.2,
            route_path: Some(route_path),
            level: Some(level),
            ..LevelRuntime::empty()
        }
    }

    pub fn active_wave(&self) -> Option<&WaveData> {
        self.level.as_ref()?.waves.get(self.current_wave_index)
    }

    pub fn active_wave_mut(&mut self) -> Option<&mut WaveData> {
        self.level.as_mut()?.waves.get_mut(self.current_wave_index)
    }

    pub fn wave_total(&self) -> usize {
        self.level.as_ref().map_or(0, |level| level.waves.len())
    }

    pub fn active_monsters(&self) -> impl Iterator<Item = &MonsterRef> {
        self.monsters.iter().filter(|monster| !monster.borrow().removed)
    }

    pub fn compact_removed(&mut self) {
        compact_in_place(&mut self.monsters, |monster| monster.borrow().removed);
        compact_in_place(&mut self.drones, |drone| drone.borrow().removed);
        compact_in_place(&mut self.projectiles, |projectile| projectile.removed);
        compact_in_place(&mut self.missiles, |missile| missile.removed);
        compact_in_place(&mut self.particles, |particle| particle.removed);
        compact_in_place(&mut self.links, |link| link.removed);
    }
}

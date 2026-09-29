//! Development-only API (the `labs` feature): scripted staging behind `window.__vectorDefence`
//! for the render/benchmark scripts, and the `TowerLab` of `debug/towers.html`.
use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, PI};
use std::fmt::Write as _;

use vd_core::campaign::routes_for_mode;
use vd_core::constants::MAX_TOWER_LEVEL;
use vd_core::entities::projectiles::missile::create_missile_visual;
use vd_core::entities::towers::registry::tower_info;
use vd_core::entities::{Drone, Missile, Monster, MonsterRef, Projectile, Tower, share};
use vd_core::game::Game;
use vd_core::monster_factory::create_monster;
use vd_core::placement::can_place_tower;
use vd_core::profile::{GameMode, GameProfile};
use vd_core::progress::MemoryProgressStorage;
use vd_core::rng;
use vd_core::route_path::{
    PathEntry, RoutePathCommand, SharedPath, create_path_entries_from_distance, create_route_motion_path,
};
use vd_core::types::{FieldBounds, GameState, MonsterKind, Point, TowerKind};
use vd_core::update::{StandaloneUpdateContext, UpdateResult};
use vd_core::utils::is_within_distance_to_segment;
use wasm_bindgen::prelude::*;

use crate::game::WebGame;

/// Keeps the benchmark fight at a fixed crowd after every simulation step.
pub struct BenchmarkRefill {
    monsters: usize,
    hit_point_scale: f64,
    level_index: i32,
}

const MONSTER_KINDS: [MonsterKind; 8] = MonsterKind::ALL;

pub fn refill_benchmark(web: &mut WebGame) {
    let Some(refill) = &web.benchmark else {
        return;
    };
    let (target, scale, level_index) = (refill.monsters, refill.hit_point_scale, refill.level_index);
    let Some(route) = &web.game.runtime.route_path else {
        return;
    };
    let entries = route.entries.clone();
    let path_length = entries.last().map_or(0.0, |entry| entry.total_distance);
    while web.game.runtime.monsters.len() < target {
        let kind = MONSTER_KINDS[web.game.runtime.monsters.len() % MONSTER_KINDS.len()];
        let path: SharedPath = create_path_entries_from_distance(&entries, rng::random() * path_length * 0.85).into();
        let mut monster = create_monster(kind, path, web.game.profile.monster_speed_scale, level_index);
        monster.hit_points *= scale;
        monster.max_hit_points = monster.hit_points;
        web.game.runtime.monsters.push(share(monster));
    }
}

impl WebGame {
    fn find_monster(&self, id: u32) -> Option<MonsterRef> {
        self.game.runtime.monsters.iter().find(|monster| monster.borrow().id == id).cloned()
    }
}

#[wasm_bindgen]
impl WebGame {
    /// Reseeds the engine's random generator (`Math.random` for the simulation and effects).
    #[wasm_bindgen(js_name = seedRandom)]
    pub fn seed_random(&mut self, seed: f64) {
        rng::seed(seed as u64);
    }

    #[wasm_bindgen(js_name = debugStartLevel)]
    pub fn debug_start_level(&mut self, index: usize) {
        self.game.debug_all_levels_unlocked = true;
        self.game.start_level_by_index(index);
    }

    #[wasm_bindgen(js_name = debugSetEconomy)]
    pub fn debug_set_economy(&mut self, money: i32, spawn_delay: f64, escapes_left: i32) {
        self.game.runtime.money = money;
        self.game.runtime.spawn_delay = spawn_delay;
        self.game.runtime.escapes_left = escapes_left;
        self.game.request_hud_sync();
    }

    /// Sets the state directly (no sounds or menus), like the scripts' `game.state = ...`.
    #[wasm_bindgen(js_name = debugSetState)]
    pub fn debug_set_state(&mut self, state: &str) {
        let next = match state {
            "playing" => GameState::Playing,
            "paused" => GameState::Paused,
            "menu" => GameState::Menu,
            _ => return,
        };
        self.game.set_state(next);
    }

    #[wasm_bindgen(js_name = debugUpgradeSelected)]
    pub fn debug_upgrade_selected(&mut self, times: u32) {
        if let Some(tower) = &self.game.runtime.selected_tower {
            for _ in 0..times {
                tower.borrow_mut().upgrade();
            }
        }
    }

    #[wasm_bindgen(js_name = debugClearSelection)]
    pub fn debug_clear_selection(&mut self) {
        self.game.runtime.selected_tower = None;
        self.game.request_hud_sync();
    }

    #[wasm_bindgen(js_name = availableTowers)]
    pub fn available_towers(&self) -> Vec<String> {
        self.game
            .current_level()
            .map(|level| level.available_towers.iter().map(|kind| kind.as_str().to_string()).collect())
            .unwrap_or_default()
    }

    #[wasm_bindgen(js_name = fieldWidth)]
    pub fn field_width(&self) -> f64 {
        self.game.profile.field_width
    }

    #[wasm_bindgen(js_name = fieldHeight)]
    pub fn field_height(&self) -> f64 {
        self.game.profile.field_height
    }

    #[wasm_bindgen(js_name = routeLength)]
    pub fn route_length(&self) -> f64 {
        self.game
            .runtime
            .route_path
            .as_ref()
            .and_then(|route| route.entries.last())
            .map_or(0.0, |entry| entry.total_distance)
    }

    #[wasm_bindgen(js_name = routeEnd)]
    pub fn route_end(&self) -> Vec<f64> {
        self.game
            .runtime
            .route_path
            .as_ref()
            .and_then(|route| route.entries.last())
            .map_or(vec![0.0, 0.0], |entry| vec![entry.x, entry.y])
    }

    /// Distance from a point to the nearest sampled route point (like the scripts' old helper).
    #[wasm_bindgen(js_name = distanceToRoute)]
    pub fn distance_to_route(&self, x: f64, y: f64) -> f64 {
        let Some(route) = &self.game.runtime.route_path else {
            return f64::INFINITY;
        };
        route.entries.iter().map(|entry| (entry.x - x).hypot(entry.y - y)).fold(f64::INFINITY, f64::min)
    }

    /// Whether a point lies within `distance` of the road centerline.
    #[wasm_bindgen(js_name = isNearRoute)]
    pub fn is_near_route(&self, x: f64, y: f64, distance: f64) -> bool {
        let Some(route) = &self.game.runtime.route_path else {
            return false;
        };
        route
            .entries
            .windows(2)
            .any(|pair| is_within_distance_to_segment(Point::new(x, y), pair[0].point(), pair[1].point(), distance))
    }

    /// Spawns a monster `distance` along the route; returns its id.
    #[wasm_bindgen(js_name = debugSpawnMonster)]
    pub fn debug_spawn_monster(&mut self, kind: &str, distance: f64, level_index: i32) -> u32 {
        let (Some(kind), Some(route)) = (MonsterKind::parse(kind), &self.game.runtime.route_path) else {
            return 0;
        };
        let path: SharedPath = create_path_entries_from_distance(&route.entries, distance).into();
        let monster = create_monster(kind, path, self.game.profile.monster_speed_scale, level_index);
        let id = monster.id;
        self.game.runtime.monsters.push(share(monster));
        id
    }

    #[wasm_bindgen(js_name = debugMonsterMaxHitPoints)]
    pub fn debug_monster_max_hit_points(&self, id: u32) -> f64 {
        self.find_monster(id).map_or(0.0, |monster| monster.borrow().max_hit_points)
    }

    /// Sets hit points (and the full-health denominator unless it is NaN).
    #[wasm_bindgen(js_name = debugSetMonsterHitPoints)]
    pub fn debug_set_monster_hit_points(&mut self, id: u32, hit_points: f64, max_hit_points: f64) {
        if let Some(monster) = self.find_monster(id) {
            let mut monster = monster.borrow_mut();
            monster.hit_points = hit_points;
            if !max_hit_points.is_nan() {
                monster.max_hit_points = max_hit_points;
            }
        }
    }

    /// `[{id, kind, x, y, removed, hitPoints}]` with presentation (shaken) positions, as JSON.
    #[wasm_bindgen(js_name = debugMonsters)]
    pub fn debug_monsters(&self) -> String {
        let mut out = String::from("[");
        for (index, monster) in self.game.runtime.monsters.iter().enumerate() {
            let monster = monster.borrow();
            if index > 0 {
                out.push(',');
            }
            let _ = write!(
                out,
                "{{\"id\":{},\"kind\":\"{}\",\"x\":{},\"y\":{},\"removed\":{},\"hitPoints\":{}}}",
                monster.id,
                monster.kind().as_str(),
                monster.visual_x(),
                monster.visual_y(),
                monster.removed,
                monster.hit_points
            );
        }
        out.push(']');
        out
    }

    /// Launches a missile of `level` from a point at a monster.
    #[wasm_bindgen(js_name = debugLaunchMissile)]
    pub fn debug_launch_missile(&mut self, from_x: f64, from_y: f64, target_id: u32, level: u32) {
        if let Some(target) = self.find_monster(target_id) {
            let visual = create_missile_visual(level);
            self.game.runtime.missiles.push(Missile::new(Point::new(from_x, from_y), target, level, &visual, None));
        }
    }

    /// Stages the crowded benchmark fight (as `benchmark:3d` always has): level `level_index`,
    /// towers every 13 units near the road at level 7, and `monsters` monsters with
    /// `hit_point_scale` times the hit points, refilled after every step. Returns the tower count.
    #[wasm_bindgen(js_name = stageBenchmarkFight)]
    pub fn stage_benchmark_fight(&mut self, level_index: usize, monsters: usize, hit_point_scale: f64) -> usize {
        self.debug_start_level(level_index);
        self.game.runtime.money = 999_999;
        self.game.runtime.escapes_left = 99_999;
        let kinds = self.game.current_level().map(|level| level.available_towers.clone()).unwrap_or_default();
        let (width, height) = (self.game.profile.field_width, self.game.profile.field_height);
        let mut tower_index = 0;
        let mut y = 20.0;
        while y < height - 10.0 {
            let mut x = 20.0;
            while x < width - 10.0 {
                let distance = self.distance_to_route(x, y);
                if (26.0..=60.0).contains(&distance)
                    && self.game.can_place_tower(Point::new(x, y))
                    && self.game.place_tower(kinds[tower_index % kinds.len()], Point::new(x, y))
                {
                    self.debug_upgrade_selected(6);
                    tower_index += 1;
                }
                x += 13.0;
            }
            y += 13.0;
        }
        self.game.runtime.selected_tower = None;
        self.benchmark = Some(BenchmarkRefill { monsters, hit_point_scale, level_index: level_index as i32 });
        crate::labs::refill_benchmark(self);
        self.game.runtime.spawn_delay = 0.0;
        self.game.request_hud_sync();
        self.game.runtime.towers.len()
    }

    /// Counters for the benchmark scripts, as JSON.
    #[wasm_bindgen(js_name = debugStats)]
    pub fn debug_stats(&self) -> String {
        let runtime = &self.game.runtime;
        let (instances, draw_calls, pixel_ratio, pipelines) = match &self.renderer {
            Some(renderer) => (
                renderer.drawn_instances(),
                renderer.frame_draw_calls(),
                renderer.pixel_ratio(),
                renderer.startup_timings().pipelines,
            ),
            None => (0, 0, 0.0, 0),
        };
        format!(
            "{{\"particles\":{},\"links\":{},\"towers\":{},\"monsters\":{},\"instances\":{},\"drawCalls\":{},\"pixelRatio\":{},\"pipelines\":{}}}",
            runtime.particles.len(),
            runtime.links.len(),
            runtime.towers.len(),
            runtime.monsters.len(),
            instances,
            draw_calls,
            pixel_ratio,
            pipelines
        )
    }

    /// Bytes of Wasm linear memory (the engine's heap).
    #[wasm_bindgen(js_name = wasmMemoryBytes)]
    pub fn wasm_memory_bytes(&self) -> f64 {
        let memory: js_sys::WebAssembly::Memory = wasm_bindgen::memory().unchecked_into();
        memory.buffer().unchecked_into::<js_sys::ArrayBuffer>().byte_length() as f64
    }

    /// Frames the render camera over a field point (no shake), for close-ups.
    pub fn inspect(&mut self, x: f64, y: f64, visible_height: f64, yaw: f64, tilt: f64) {
        if let Some(renderer) = &mut self.renderer {
            renderer.inspect(Some(vd_render::InspectView {
                x: x as f32,
                y: y as f32,
                visible_height: visible_height as f32,
                yaw: yaw as f32,
                tilt: tilt as f32,
            }));
        }
    }

    #[wasm_bindgen(js_name = clearInspect)]
    pub fn clear_inspect(&mut self) {
        if let Some(renderer) = &mut self.renderer {
            renderer.inspect(None);
        }
    }

    #[wasm_bindgen(js_name = boardTiltRadians)]
    pub fn board_tilt_radians(&self) -> f64 {
        vd_render::BOARD_TILT_RADIANS as f64
    }
}

const LAB_STEP_SECONDS: f64 = 1.0 / 60.0;
const TOWER_VISIBLE_HEIGHT: f64 = 58.0;
const PROJECTILE_VISIBLE_HEIGHT: f64 = 48.0;
const EXPLOSION_VISIBLE_HEIGHT: f64 = 150.0;
const TOWER_SETTLE_SECONDS: f64 = 0.1;
// Long enough for a tracer to stretch and a drone shot to drop from cruise altitude.
const SHOT_SETTLE_SECONDS: f64 = 0.06;
const EXPLOSION_SETTLE_SECONDS: f64 = 0.12;
const LAB_BOUNDS: FieldBounds = FieldBounds { min_x: -2000.0, min_y: -2000.0, max_x: 2000.0, max_y: 2000.0 };

#[derive(Clone, Copy)]
enum LabRow {
    Tower(TowerKind),
    Projectile,
    DroneProjectile,
    Missile,
    MissileExplosion,
}

impl LabRow {
    fn label(self) -> &'static str {
        match self {
            LabRow::Tower(kind) => tower_info(kind).label,
            LabRow::Projectile => "Projectile",
            LabRow::DroneProjectile => "Drone Projectile",
            LabRow::Missile => "Missile",
            LabRow::MissileExplosion => "Missile Explosion",
        }
    }

    fn visible_height(self) -> f64 {
        match self {
            LabRow::Tower(_) => TOWER_VISIBLE_HEIGHT,
            LabRow::Projectile | LabRow::DroneProjectile | LabRow::Missile => PROJECTILE_VISIBLE_HEIGHT,
            LabRow::MissileExplosion => EXPLOSION_VISIBLE_HEIGHT,
        }
    }

    fn settle_seconds(self) -> f64 {
        match self {
            LabRow::Tower(_) | LabRow::Missile => TOWER_SETTLE_SECONDS,
            LabRow::Projectile | LabRow::DroneProjectile => SHOT_SETTLE_SECONDS,
            LabRow::MissileExplosion => EXPLOSION_SETTLE_SECONDS,
        }
    }
}

fn lab_rows() -> Vec<LabRow> {
    let mut rows: Vec<LabRow> = TowerKind::ALL.into_iter().map(LabRow::Tower).collect();
    rows.extend([LabRow::Projectile, LabRow::DroneProjectile, LabRow::Missile, LabRow::MissileExplosion]);
    rows
}

/// A still target on a two-point path at `point`.
fn create_target(point: Point) -> MonsterRef {
    let path: SharedPath = vec![
        PathEntry { x: point.x, y: point.y, total_distance: 0.0, heading: None },
        PathEntry { x: point.x + 1.0, y: point.y, total_distance: 1.0, heading: None },
    ]
    .into();
    let mut target = Monster::new(MonsterKind::Square, path, 1.0);
    target.x = point.x;
    target.y = point.y;
    target.previous_x = point.x;
    target.previous_y = point.y;
    share(target)
}

/// The previous 2D sheet's poses: turrets at -45 degrees, a firing gun, a live laser beam.
fn pose_tower(tower: &mut Tower) {
    match tower.kind {
        TowerKind::Gun => {
            tower.set_angle(-FRAC_PI_4);
            if let Some(gun) = tower.gun_mut() {
                gun.muzzle_flash_seconds = 0.04;
            }
        }
        TowerKind::Laser => {
            tower.set_angle(-FRAC_PI_4);
            if let Some(laser) = tower.laser_mut() {
                laser.beam_alpha = 0.72;
            }
        }
        TowerKind::Missile => tower.set_angle(-FRAC_PI_4),
        TowerKind::Slow => {
            if let Some(slow) = tower.slow_mut() {
                slow.pulse = FRAC_PI_2;
            }
        }
        TowerKind::Drone | TowerKind::Lightning => {}
    }
}

/// 3D tower and projectile sheet for `debug/towers.html`: each cell stages one subject alone at
/// the field center of a fresh level runtime (scenery hidden), advances it with the real entity
/// updates while the views settle, and frames it with the renderer's close-up `inspect`.
#[wasm_bindgen]
pub struct TowerLab {
    game: Game,
    renderer: vd_render::BoardRenderer,
    rows: Vec<LabRow>,
}

#[wasm_bindgen]
impl TowerLab {
    pub async fn create(
        device: web_sys::GpuDevice,
        canvas: web_sys::HtmlCanvasElement,
        overlay: web_sys::HtmlCanvasElement,
    ) -> Result<TowerLab, JsValue> {
        let game = Game::new(GameMode::Desktop, Box::new(MemoryProgressStorage::default()));
        let mut renderer = vd_render::BoardRenderer::create(device, canvas, overlay, &game.profile, 0, None).await?;
        // Cells stage subjects in a real level runtime; only the subject and the ground should show.
        renderer.set_scenery_visible(false);
        renderer.resize();
        Ok(TowerLab { game, renderer, rows: lab_rows() })
    }

    #[wasm_bindgen(js_name = rowLabels)]
    pub fn row_labels(&self) -> Vec<String> {
        self.rows.iter().map(|row| row.label().to_string()).collect()
    }

    #[wasm_bindgen(js_name = levelCount)]
    pub fn level_count(&self) -> u32 {
        MAX_TOWER_LEVEL + 1
    }

    #[wasm_bindgen(js_name = boardTiltRadians)]
    pub fn board_tilt_radians(&self) -> f64 {
        vd_render::BOARD_TILT_RADIANS as f64
    }

    pub fn resize(&mut self) {
        self.renderer.resize();
    }

    /// Stages one cell in a fresh runtime, lets it settle, and returns its default close-up as
    /// `[x, y, visibleHeight, yaw, tilt]`.
    #[wasm_bindgen(js_name = stageCell)]
    pub fn stage_cell(&mut self, row: usize, level: u32) -> Vec<f64> {
        let Some(&row) = self.rows.get(row) else {
            return Vec::new();
        };
        let stage = Point::new(self.game.profile.field_width / 2.0, self.game.profile.field_height / 2.0);
        // Shots fly along -22.5 degrees toward a far target, so they are mid-flight when captured.
        let shot_target = Point::new(stage.x + 400.0, stage.y - 400.0 * (PI / 8.0).tan());
        // A fresh runtime per cell: the renderer resets its views and effects on the switch.
        self.game.start_level_by_index(0);
        self.game.set_state(GameState::Paused);
        self.game.runtime.monsters.clear();
        let _ = self.game.take_sounds();
        self.stage(row, level, stage, shot_target);
        // First frame at the spawn state, so views record where shots and blasts start.
        self.renderer.draw(&self.game);
        let steps = (row.settle_seconds() / LAB_STEP_SECONDS).round() as usize;
        let standalone = StandaloneUpdateContext::new(
            self.game.profile.field_width,
            self.game.profile.field_height,
            LAB_BOUNDS,
            Vec::new(),
        );
        for _ in 0..steps {
            let context = standalone.context(LAB_STEP_SECONDS);
            let mut result = UpdateResult::new();
            match row {
                LabRow::Projectile | LabRow::DroneProjectile => {
                    for projectile in &mut self.game.runtime.projectiles {
                        projectile.update(&context, &mut result);
                    }
                }
                LabRow::MissileExplosion => {
                    for particle in &mut self.game.runtime.particles {
                        particle.update(&context);
                    }
                }
                LabRow::Tower(_) | LabRow::Missile => {}
            }
            self.game.simulation_seconds += LAB_STEP_SECONDS;
            self.renderer.draw(&self.game);
        }
        let focus = match row {
            LabRow::Projectile | LabRow::DroneProjectile => {
                self.game.runtime.projectiles.first().map_or(stage, |projectile| Point::new(projectile.x, projectile.y))
            }
            _ => stage,
        };
        vec![focus.x, focus.y, row.visible_height(), 0.0, vd_render::BOARD_TILT_RADIANS as f64]
    }

    /// Draws the staged scene from a close-up view (the simulation clock is not advanced).
    #[wasm_bindgen(js_name = renderView)]
    pub fn render_view(&mut self, x: f64, y: f64, visible_height: f64, yaw: f64, tilt: f64) {
        self.renderer.inspect(Some(vd_render::InspectView {
            x: x as f32,
            y: y as f32,
            visible_height: visible_height as f32,
            yaw: yaw as f32,
            tilt: tilt as f32,
        }));
        self.renderer.draw(&self.game);
    }
}

impl TowerLab {
    fn stage(&mut self, row: LabRow, level: u32, stage: Point, shot_target: Point) {
        let runtime = &mut self.game.runtime;
        match row {
            LabRow::Tower(kind) => {
                let mut tower = Tower::new(kind, stage.x, stage.y);
                for _ in 0..level {
                    tower.upgrade();
                }
                pose_tower(&mut tower);
                runtime.towers.push(share(tower));
                if kind == TowerKind::Drone {
                    let mut drone = Drone::new(stage, level);
                    drone.x = stage.x + 15.0;
                    drone.y = stage.y - 12.0;
                    runtime.drones.push(share(drone));
                }
            }
            LabRow::Projectile => runtime.projectiles.push(Projectile::gun(stage, shot_target, level)),
            LabRow::DroneProjectile => runtime.projectiles.push(Projectile::drone(stage, shot_target, level)),
            LabRow::Missile => {
                let target = create_target(Point::new(stage.x + 120.0, stage.y));
                let visual = create_missile_visual(level);
                runtime.missiles.push(Missile::new(stage, target, level, &visual, Some(-PI / 8.0)));
            }
            LabRow::MissileExplosion => {
                // Detonate a real missile on a target at the stage; its particles drive the 3D blast.
                let target = create_target(stage);
                let visual = create_missile_visual(level);
                let mut missile = Missile::new(stage, target.clone(), level, &visual, Some(0.0));
                missile.speed_per_second = 0.0;
                let standalone = StandaloneUpdateContext::new(
                    self.game.profile.field_width,
                    self.game.profile.field_height,
                    LAB_BOUNDS,
                    vec![target],
                );
                let mut result = UpdateResult::new();
                missile.update(&standalone.context(0.0), &mut result);
                runtime.particles.append(&mut result.particles);
            }
        }
    }
}

/// Route and placement data for `scripts/render-levels.mjs`, as JSON: per level the authored
/// points, the route's drawing commands, and a 3-unit placement grid (1 = blocked).
#[wasm_bindgen(js_name = levelSheet)]
pub fn level_sheet(mobile: bool) -> String {
    const SAMPLE: f64 = 3.0;
    let mode = if mobile { GameMode::Mobile } else { GameMode::Desktop };
    let profile = GameProfile::for_mode(mode);
    let mut out = String::new();
    let _ = write!(
        out,
        "{{\"fieldWidth\":{},\"fieldHeight\":{},\"roadWidth\":{},\"sample\":{SAMPLE},\"levels\":[",
        profile.field_width, profile.field_height, profile.road_width
    );
    for (index, route) in routes_for_mode(mode).iter().enumerate() {
        let path = create_route_motion_path(route.points, profile.road_turn_radius, profile.route_curve_sample_step);
        let path_length = path.entries.last().map_or(0.0, |entry| entry.total_distance);
        if index > 0 {
            out.push(',');
        }
        let _ = write!(
            out,
            "{{\"name\":\"{}\",\"allowEscape\":{},\"startingMoney\":{},\"pathLength\":{},\"points\":[",
            route.name, route.allow_escape, route.starting_money, path_length
        );
        for (point_index, point) in route.points.iter().enumerate() {
            let _ = write!(out, "{}[{},{}]", if point_index > 0 { "," } else { "" }, point.x, point.y);
        }
        let _ = write!(out, "],\"start\":[{},{}],\"commands\":[", path.start.x, path.start.y);
        for (command_index, command) in path.commands.iter().enumerate() {
            let separator = if command_index > 0 { "," } else { "" };
            let _ = match command {
                RoutePathCommand::Line { point } => write!(out, "{separator}[{},{}]", point.x, point.y),
                RoutePathCommand::Quadratic { control, point } => {
                    write!(out, "{separator}[{},{},{},{}]", control.x, control.y, point.x, point.y)
                }
            };
        }
        out.push_str("],\"blocked\":\"");
        let mut y = 0.0;
        while y < profile.field_height {
            let mut x = 0.0;
            while x < profile.field_width {
                let point = Point::new(x + SAMPLE / 2.0, y + SAMPLE / 2.0);
                out.push(if can_place_tower(point, Some(&path), [], &profile.placement) { '0' } else { '1' });
                x += SAMPLE;
            }
            y += SAMPLE;
        }
        out.push_str("\"}");
    }
    out.push_str("]}");
    out
}

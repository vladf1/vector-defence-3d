//! Gameplay state, campaign progression, lifecycle resolution, and `update_simulation(...)`.
//! The renderer is not owned here: it reads `Game` every frame, reports the visible field bounds
//! through `set_visible_field_bounds`, and resolves canvas action buttons before a board click.
use std::rc::Rc;

use crate::audio::{AudioCue, SoundEvent};
use crate::campaign::create_levels;
use crate::collision::ActiveCircleSweepCollisionIndex;
use crate::combat_effects::{ESCAPE_BURST_CONFIG, create_escape_burst_particles};
use crate::constants::{MAX_LINKS, MAX_PARTICLES, TIMER_EPSILON_SECONDS};
use crate::entities::towers::registry::tower_info;
use crate::entities::{DroneRef, Monster, MonsterRef, MonsterSpecial, Tower, TowerRef, share};
use crate::level_runtime::LevelRuntime;
use crate::monster_factory::{create_monster, create_splitter_children};
use crate::placement::{self, find_tower_at_point};
use crate::profile::{GameMode, GameProfile};
use crate::progress::{CampaignProgressStore, ProgressStorage};
use crate::simulation_timing::run_bounded_simulation_substeps;
use crate::types::{FieldBounds, GameState, LevelData, ModalAction, MonsterKind, Point, TowerKind, WaveData};
use crate::update::{DroneAssignments, UpdateContext, UpdateResult};
use crate::utils::{format_money, js_round, random_range};

const BREACH_DEFEAT_DELAY_SECONDS: f64 = 1.0;
const MONSTER_COLLISION_CELL_SIZE: f64 = 64.0;

/// Which canvas tower action (drawn by the renderer overlay) a board click landed on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CanvasActionHit {
    pub upgrade_button: bool,
    pub laser_lock_button: bool,
}

fn is_battle_state(state: GameState) -> bool {
    matches!(state, GameState::Playing | GameState::Paused)
}

fn is_modal_state(state: GameState) -> bool {
    matches!(state, GameState::Menu | GameState::Won | GameState::Lost | GameState::CampaignWon)
}

fn refresh_drone_assignments(drones: &[DroneRef], assignments: &mut DroneAssignments) {
    assignments.clear();
    for drone in drones {
        if let Some(target) = drone.borrow().get_assigned_target() {
            *assignments.or_insert(target.borrow().id, 0) += 1;
        }
    }
}

fn same_monster(a: &Option<MonsterRef>, b: &Option<MonsterRef>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => Rc::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    }
}

#[allow(clippy::too_many_arguments)]
fn make_context<'a>(
    delta_seconds: f64,
    profile: &GameProfile,
    field_bounds: FieldBounds,
    active_monsters: &'a [MonsterRef],
    monster_collision_index: &'a ActiveCircleSweepCollisionIndex,
    active_drones: &'a [DroneRef],
    drone_assignments: &'a DroneAssignments,
) -> UpdateContext<'a> {
    UpdateContext {
        delta_seconds,
        field_width: profile.field_width,
        field_height: profile.field_height,
        field_bounds,
        active_monsters,
        monster_collision_index,
        active_drones,
        drone_assignments,
    }
}

pub struct Game {
    pub levels: Vec<LevelData>,
    pub profile: GameProfile,
    /// -1 before any level started.
    pub current_level_index: i32,
    pub highest_unlocked_level_index: usize,
    pub level_stars: Vec<u32>,
    pub last_awarded_stars: u32,
    pub debug_all_levels_unlocked: bool,
    pub campaign_cleared: bool,
    /// The battle state (playing or paused) the campaign map returns to.
    pub menu_return_state: Option<GameState>,
    pub state: GameState,
    pub runtime: LevelRuntime,
    pub banner_text: String,
    pub banner_timer: f64,
    pub hud_dirty: bool,
    pub modal_dirty: bool,
    /// Accumulated simulated seconds; renderers use it as a presentation clock that freezes with the game.
    pub simulation_seconds: f64,
    /// Bumped wherever the TS called `renderBackgroundLayer()` (a level start or an escape), so a
    /// renderer can refresh static layers such as the escape counter.
    pub background_revision: u32,
    /// Changes whenever `runtime` is replaced (every level start), so renderers can reset the
    /// per-entity state they keep for the previous runtime (the TS compared runtime identity).
    pub runtime_generation: u64,
    visible_field_bounds: FieldBounds,
    breach_resolution_delay_seconds: f64,
    active_monsters: Vec<MonsterRef>,
    monster_collision_index: ActiveCircleSweepCollisionIndex,
    drone_assignments: DroneAssignments,
    update_result: UpdateResult,
    sounds: Vec<SoundEvent>,
    progress_store: CampaignProgressStore,
}

impl Game {
    pub fn new(mode: GameMode, storage: Box<dyn ProgressStorage>) -> Game {
        Game::with_progress_store(mode, CampaignProgressStore::new(Some(storage)))
    }

    pub fn with_progress_store(mode: GameMode, progress_store: CampaignProgressStore) -> Game {
        let profile = GameProfile::for_mode(mode);
        let mut game = Game {
            levels: create_levels(mode),
            visible_field_bounds: profile.placement.bounds,
            profile,
            current_level_index: -1,
            highest_unlocked_level_index: 0,
            level_stars: Vec::new(),
            last_awarded_stars: 0,
            debug_all_levels_unlocked: false,
            campaign_cleared: false,
            menu_return_state: None,
            state: GameState::Menu,
            runtime: LevelRuntime::empty(),
            banner_text: "Awaiting orders".to_string(),
            banner_timer: 0.0,
            hud_dirty: true,
            modal_dirty: true,
            simulation_seconds: 0.0,
            background_revision: 0,
            runtime_generation: 0,
            breach_resolution_delay_seconds: 0.0,
            active_monsters: Vec::new(),
            monster_collision_index: ActiveCircleSweepCollisionIndex::new(MONSTER_COLLISION_CELL_SIZE),
            drone_assignments: DroneAssignments::new(),
            update_result: UpdateResult::new(),
            sounds: Vec::new(),
            progress_store,
        };
        let level_count = game.campaign_level_count();
        let progress = game.progress_store.load_campaign_progress(level_count);
        game.campaign_cleared = progress.campaign_cleared;
        game.highest_unlocked_level_index = progress.highest_unlocked_level_index;
        game.level_stars = game.progress_store.load_level_stars(level_count);
        game
    }

    pub fn active_wave(&self) -> Option<&WaveData> {
        self.runtime.active_wave()
    }

    pub fn wave_total(&self) -> usize {
        self.runtime.wave_total()
    }

    pub fn current_level(&self) -> Option<&LevelData> {
        self.runtime.level.as_ref()
    }

    pub fn campaign_level_count(&self) -> usize {
        self.levels.len()
    }

    /// The visible board area (the renderer's camera framing); placement and shot culling use it.
    pub fn visible_field_bounds(&self) -> FieldBounds {
        self.visible_field_bounds
    }

    pub fn set_visible_field_bounds(&mut self, bounds: FieldBounds) {
        self.visible_field_bounds = bounds;
    }

    /// Sounds requested since the last call, in order.
    pub fn take_sounds(&mut self) -> Vec<SoundEvent> {
        std::mem::take(&mut self.sounds)
    }

    /// Moves queued sounds into `buffer` (keeping its allocation).
    pub fn drain_sounds_into(&mut self, buffer: &mut Vec<SoundEvent>) {
        buffer.append(&mut self.sounds);
    }

    pub fn play_sound(&mut self, cue: AudioCue, pan_x: Option<f64>, intensity: Option<f64>) {
        self.sounds.push(SoundEvent { cue, pan_x, intensity });
    }

    pub fn request_hud_sync(&mut self) {
        self.hud_dirty = true;
    }

    pub fn request_modal_sync(&mut self) {
        self.modal_dirty = true;
    }

    pub fn can_perform_battle_action(&self) -> bool {
        self.state == GameState::Playing
    }

    pub fn needs_animation_frame(&self) -> bool {
        self.state == GameState::Playing || self.breach_resolution_delay_seconds > 0.0
    }

    pub fn set_banner(&mut self, text: &str, duration_seconds: f64) {
        self.banner_text = text.to_string();
        self.banner_timer = duration_seconds;
        self.request_hud_sync();
    }

    pub fn set_state(&mut self, next: GameState) {
        self.state = next;
        self.request_hud_sync();
    }

    pub fn start_level(&mut self, level: LevelData) {
        self.current_level_index =
            self.levels.iter().position(|candidate| candidate.id == level.id).map_or(-1, |index| index as i32);
        self.last_awarded_stars = 0;
        let banner = format!("Level {}: {}", level.level_number, level.name);
        self.runtime = LevelRuntime::new(level, self.profile.road_turn_radius, self.profile.route_curve_sample_step);
        self.runtime_generation += 1;
        self.breach_resolution_delay_seconds = 0.0;
        self.menu_return_state = None;
        self.set_banner(&banner, 2.4);
        self.set_state(GameState::Playing);
        self.play_sound(AudioCue::LevelStart, None, None);
        self.background_revision = self.background_revision.wrapping_add(1);
        self.request_modal_sync();
        self.request_hud_sync();
    }

    pub fn start_level_by_index(&mut self, index: usize) {
        let Some(level) = self.levels.get(index).cloned() else {
            return;
        };
        if !self.debug_all_levels_unlocked && !self.campaign_cleared && index > self.highest_unlocked_level_index {
            self.play_sound(AudioCue::InvalidAction, None, None);
            return;
        }
        self.start_level(level);
    }

    pub fn unlock_all_levels_for_debug(&mut self) {
        self.debug_all_levels_unlocked = true;
        self.set_banner("All levels unlocked", 1.8);
        self.request_modal_sync();
        self.request_hud_sync();
    }

    pub fn restart(&mut self) {
        if let Some(level) = self.runtime.level.clone() {
            self.start_level(level);
        } else {
            self.request_modal_sync();
        }
    }

    pub fn restart_campaign(&mut self) {
        self.last_awarded_stars = 0;
        self.start_level_by_index(0);
    }

    pub fn start_next_level(&mut self) {
        if self.current_level_index < 0 {
            self.start_level_by_index(0);
            return;
        }
        let next_index = (self.current_level_index as usize + 1).min(self.campaign_level_count().saturating_sub(1));
        self.start_level_by_index(next_index);
    }

    pub fn open_menu(&mut self) {
        if self.state == GameState::Menu {
            return;
        }
        self.menu_return_state = if is_battle_state(self.state) { Some(self.state) } else { None };
        self.play_sound(AudioCue::MenuOpen, None, None);
        self.set_state(GameState::Menu);
        self.request_modal_sync();
    }

    pub fn resume_battle(&mut self) {
        let Some(return_state) = self.menu_return_state else {
            return;
        };
        if self.current_level().is_none() {
            return;
        }
        self.set_state(return_state);
        self.menu_return_state = None;
        self.play_sound(AudioCue::Resume, None, None);
        self.request_modal_sync();
    }

    pub fn toggle_pause(&mut self) {
        if self.state == GameState::Playing {
            self.clear_tower_placement();
            self.set_state(GameState::Paused);
            self.play_sound(AudioCue::Pause, None, None);
        } else if self.state == GameState::Paused {
            self.set_state(GameState::Playing);
            self.play_sound(AudioCue::Resume, None, None);
        }
        self.request_modal_sync();
    }

    pub fn spawn_monster(&mut self) {
        let (Some(route_path), Some(wave)) = (&self.runtime.route_path, self.runtime.active_wave()) else {
            return;
        };
        let sequence = &wave.monster_sequence;
        let kind = sequence.get(self.runtime.spawn_index).copied().unwrap_or(MonsterKind::PackMan);
        let sequence_length = sequence.len().max(1);
        let entries = route_path.entries.clone();
        self.runtime.spawn_index = (self.runtime.spawn_index + 1) % sequence_length;
        self.runtime.spawned_monsters += 1;
        self.runtime.wave_spawned_monsters += 1;
        let monster = create_monster(kind, entries, self.profile.monster_speed_scale, self.current_level_index);
        self.runtime.monsters.push(share(monster));
    }

    pub fn on_monster_killed(&mut self, monster: &MonsterRef, result: &mut UpdateResult) {
        let monster = monster.borrow();
        self.runtime.money += monster.bounty;
        monster.add_death_effect(result);
        self.request_hud_sync();
    }

    pub fn spawn_splitters(&mut self, monster: &MonsterRef) {
        if self.current_level().is_none() {
            return;
        }
        let (children, x) = {
            let monster = monster.borrow();
            (create_splitter_children(&monster, self.profile.monster_speed_scale, self.current_level_index), monster.x)
        };
        for child in children {
            self.runtime.monsters.push(share(child));
        }
        self.play_sound(AudioCue::SplitterBurst, Some(x), None);
    }

    pub fn on_monster_escaped(&mut self, monster: &MonsterRef, result: &mut UpdateResult) {
        let (x, y) = {
            let monster = monster.borrow();
            (monster.x, monster.y)
        };
        let particles = create_escape_burst_particles(x, y, &ESCAPE_BURST_CONFIG, result.remaining_particle_capacity());
        result.add_particles(particles);
        result.play_sound(AudioCue::EscapeBurst, Some(x), None);
        let escapes_left_before = self.runtime.escapes_left;
        self.runtime.escapes_left = (self.runtime.escapes_left - 1).max(0);
        if self.runtime.escapes_left != escapes_left_before {
            self.background_revision = self.background_revision.wrapping_add(1);
        }
        if escapes_left_before > 0 && self.runtime.escapes_left == 0 {
            self.breach_resolution_delay_seconds = BREACH_DEFEAT_DELAY_SECONDS;
            self.clear_tower_placement();
            self.set_state(GameState::DefeatPending);
            self.set_banner("Base breached", BREACH_DEFEAT_DELAY_SECONDS);
        }
        self.request_hud_sync();
    }

    pub fn lose_level(&mut self) {
        if self.current_level().is_none() {
            return;
        }
        self.breach_resolution_delay_seconds = 0.0;
        for monster in &self.runtime.monsters {
            monster.borrow_mut().removed = true;
        }
        self.set_state(GameState::Lost);
        self.menu_return_state = None;
        self.set_banner("Defeat", 5.0);
        self.play_sound(AudioCue::LevelLoss, None, None);
        self.request_modal_sync();
        self.request_hud_sync();
    }

    pub fn set_pointer(&mut self, point: Option<Point>) {
        self.runtime.pointer = point;
    }

    fn clear_tower_placement(&mut self) {
        if self.runtime.placing_tower.is_none() {
            return;
        }
        self.runtime.placing_tower = None;
        self.request_hud_sync();
    }

    pub fn cancel_tower_placement(&mut self) {
        if !self.can_perform_battle_action() {
            return;
        }
        self.clear_tower_placement();
    }

    pub fn skip_build_break(&mut self) {
        if self.state != GameState::Playing {
            return;
        }
        if self.active_wave().is_none() || self.runtime.spawn_delay <= 0.0 {
            return;
        }
        self.runtime.spawn_delay = 0.0;
        self.banner_timer = 0.0;
        self.play_sound(AudioCue::WaveStart, None, None);
        self.request_hud_sync();
    }

    pub fn start_tower_placement(&mut self, kind: TowerKind) {
        if self.current_level().is_none() || !self.can_perform_battle_action() {
            return;
        }
        if !self.is_tower_available(kind) {
            self.play_sound(AudioCue::InvalidAction, None, None);
            return;
        }
        self.runtime.selected_tower = None;
        self.runtime.placing_tower = Some(kind);
        self.request_hud_sync();
    }

    pub fn toggle_tower_placement(&mut self, kind: TowerKind) {
        if self.current_level().is_none() || !self.can_perform_battle_action() {
            return;
        }
        if !self.is_tower_available(kind) {
            self.play_sound(AudioCue::InvalidAction, None, None);
            return;
        }
        let is_canceling = self.runtime.placing_tower == Some(kind);
        self.runtime.selected_tower = None;
        self.runtime.placing_tower = if is_canceling { None } else { Some(kind) };
        self.request_hud_sync();
    }

    /// A click on the board at field `point`; `hit` says whether it landed on a canvas tower
    /// action (checked by the renderer overlay, which draws them).
    pub fn handle_board_click(&mut self, point: Point, hit: CanvasActionHit) {
        if is_modal_state(self.state) {
            return;
        }
        if !self.can_perform_battle_action() {
            return;
        }
        if hit.upgrade_button {
            if self.can_upgrade_selected_tower() {
                self.upgrade_selected_tower();
            } else {
                self.play_sound(AudioCue::InvalidAction, None, None);
            }
            return;
        }
        if hit.laser_lock_button {
            self.toggle_selected_laser_lock();
            return;
        }
        if let Some(placing_tower) = self.runtime.placing_tower {
            if self.find_tower_at(point).is_some() {
                self.runtime.placing_tower = None;
                self.select_tower_at(point);
                return;
            }
            if !self.place_tower(placing_tower, point) {
                self.play_sound(AudioCue::InvalidAction, Some(point.x), None);
            }
            return;
        }
        self.select_tower_at(point);
    }

    /// The topmost (last placed) tower under `point`.
    pub fn find_tower_at(&self, point: Point) -> Option<TowerRef> {
        let positions: Vec<Point> = self.runtime.towers.iter().map(|tower| tower.borrow().position()).collect();
        find_tower_at_point(point, &positions, self.profile.tower_radius, self.profile.tower_selection_padding)
            .map(|index| self.runtime.towers[index].clone())
    }

    pub fn can_place_tower(&self, point: Point) -> bool {
        self.can_place_tower_in_bounds(point, self.visible_field_bounds)
    }

    pub fn can_place_tower_in_bounds(&self, point: Point, field_bounds: FieldBounds) -> bool {
        let mut geometry = self.profile.placement;
        geometry.bounds = field_bounds;
        placement::can_place_tower(
            point,
            self.runtime.route_path.as_ref(),
            self.runtime.towers.iter().map(|tower| tower.borrow().position()),
            &geometry,
        )
    }

    pub fn place_tower(&mut self, kind: TowerKind, point: Point) -> bool {
        if !self.can_perform_battle_action()
            || !self.is_tower_available(kind)
            || !self.can_afford_tower(kind)
            || !self.can_place_tower(point)
        {
            return false;
        }
        let tower = self.create_tower(kind, point);
        self.runtime.money -= tower.cost;
        let tower = share(tower);
        self.runtime.towers.push(tower.clone());
        self.runtime.selected_tower = Some(tower);
        self.runtime.placing_tower = None;
        self.play_sound(AudioCue::TowerPlace, Some(point.x), None);
        self.request_hud_sync();
        true
    }

    /// A new tower with the profile's range scale applied (not yet on the board).
    pub fn create_tower(&self, kind: TowerKind, point: Point) -> Tower {
        let mut tower = Tower::new(kind, point.x, point.y);
        tower.range = js_round(tower.range * self.profile.tower_range_scale);
        tower
    }

    pub fn select_tower_at(&mut self, point: Point) {
        if !self.can_perform_battle_action() {
            return;
        }
        let selected_tower = self.find_tower_at(point);
        if let (Some(selected), Some(current)) = (&selected_tower, &self.runtime.selected_tower)
            && Rc::ptr_eq(selected, current)
        {
            self.runtime.selected_tower = None;
            self.request_hud_sync();
            return;
        }
        let x = selected_tower.as_ref().map(|tower| tower.borrow().x);
        self.runtime.selected_tower = selected_tower;
        if let Some(x) = x {
            self.play_sound(AudioCue::TowerSelect, Some(x), None);
        }
        self.request_hud_sync();
    }

    pub fn sell_selected_tower(&mut self) {
        if !self.can_perform_battle_action() {
            return;
        }
        let Some(selected_tower) = self.runtime.selected_tower.take() else {
            return;
        };
        let (resale_value, x) = {
            let tower = selected_tower.borrow();
            (tower.resale_value(), tower.x)
        };
        self.runtime.money += resale_value;
        self.play_sound(AudioCue::TowerSell, Some(x), None);
        selected_tower.borrow_mut().removed = true;
        if let Some(index) = self.runtime.towers.iter().position(|tower| Rc::ptr_eq(tower, &selected_tower)) {
            self.runtime.towers.remove(index);
        }
        self.request_hud_sync();
    }

    pub fn upgrade_selected_tower(&mut self) {
        if !self.can_perform_battle_action() || !self.can_upgrade_selected_tower() {
            return;
        }
        let Some(selected_tower) = self.runtime.selected_tower.clone() else {
            return;
        };
        let x = {
            let mut tower = selected_tower.borrow_mut();
            self.runtime.money -= tower.upgrade_cost();
            tower.upgrade();
            tower.x
        };
        self.play_sound(AudioCue::TowerUpgrade, Some(x), None);
        self.request_hud_sync();
    }

    pub fn can_upgrade_selected_tower(&self) -> bool {
        self.runtime.selected_tower.as_ref().is_some_and(|tower| {
            let tower = tower.borrow();
            tower.can_upgrade() && self.runtime.money >= tower.upgrade_cost()
        }) && self.can_perform_battle_action()
    }

    pub fn can_afford_tower(&self, kind: TowerKind) -> bool {
        self.runtime.money >= tower_info(kind).base_cost
    }

    pub fn is_tower_available(&self, kind: TowerKind) -> bool {
        self.current_level().is_some_and(|level| level.available_towers.contains(&kind))
    }

    pub fn toggle_selected_laser_lock(&mut self) {
        if !self.can_perform_battle_action() {
            return;
        }
        let Some(selected_tower) = self.runtime.selected_tower.clone() else {
            self.play_sound(AudioCue::InvalidAction, None, None);
            return;
        };
        let toggled = {
            let mut tower = selected_tower.borrow_mut();
            let x = tower.x;
            tower.laser_mut().map(|laser| {
                laser.toggle_direction_lock();
                (laser.direction_locked, x)
            })
        };
        let Some((locked, x)) = toggled else {
            self.play_sound(AudioCue::InvalidAction, None, None);
            return;
        };
        self.play_sound(if locked { AudioCue::LaserLockOn } else { AudioCue::LaserLockOff }, Some(x), None);
        self.request_hud_sync();
    }

    pub fn complete_current_wave(&mut self) {
        let Some(reward) = self.active_wave().map(|wave| wave.reward) else {
            return;
        };
        self.runtime.money += reward;
        self.runtime.current_wave_index += 1;
        self.runtime.wave_spawned_monsters = 0;
        self.runtime.spawn_index = 0;
        self.runtime.spawn_cooldown = 0.2;
        self.request_hud_sync();

        if let Some(build_time) = self.active_wave().map(|wave| wave.build_time) {
            self.runtime.spawn_delay = build_time;
            let banner = format!("Wave {} cleared · +{}", self.runtime.current_wave_index, format_money(reward as f64));
            self.set_banner(&banner, 2.3);
        } else {
            self.runtime.spawn_delay = 0.0;
            self.set_banner(&format!("Final wave cleared · +{}", format_money(reward as f64)), 2.6);
        }
        self.play_sound(AudioCue::WaveClear, None, None);
    }

    pub fn finish_level(&mut self) {
        if self.current_level().is_none() {
            return;
        }
        let final_campaign_level_index = self.campaign_level_count() as i32 - 1;
        let is_final_campaign_level = self.current_level_index >= final_campaign_level_index;
        self.last_awarded_stars = self.calculate_level_stars();
        self.record_level_stars(self.last_awarded_stars);
        self.menu_return_state = None;

        if is_final_campaign_level {
            self.campaign_cleared = true;
            self.highest_unlocked_level_index = final_campaign_level_index.max(0) as usize;
            self.set_state(GameState::CampaignWon);
            self.play_sound(AudioCue::CampaignComplete, None, None);
        } else {
            self.highest_unlocked_level_index =
                self.highest_unlocked_level_index.max((self.current_level_index + 1).max(0) as usize);
            self.set_state(GameState::Won);
            self.play_sound(AudioCue::LevelWin, None, None);
        }
        self.save_campaign_progress();
        self.request_modal_sync();
    }

    fn save_campaign_progress(&mut self) {
        if self.debug_all_levels_unlocked {
            return;
        }
        self.progress_store.save_campaign_progress(self.highest_unlocked_level_index, self.campaign_cleared);
    }

    fn calculate_level_stars(&self) -> u32 {
        let Some(level) = self.current_level() else {
            return 0;
        };
        if self.runtime.escapes_left >= level.allow_escape {
            return 3;
        }
        if self.runtime.escapes_left as f64 >= (level.allow_escape as f64 / 2.0).ceil() { 2 } else { 1 }
    }

    fn record_level_stars(&mut self, stars: u32) {
        if self.current_level_index < 0 {
            return;
        }
        let index = self.current_level_index as usize;
        let best_stars = self.level_stars.get(index).copied().unwrap_or(0).max(stars);
        if self.level_stars.len() <= index {
            self.level_stars.resize(index + 1, 0);
        }
        self.level_stars[index] = best_stars;
        if !self.debug_all_levels_unlocked {
            self.progress_store.save_level_stars(index, best_stars);
        }
    }

    /// Runs the bounded substeps for `available_seconds` of frame time, exactly like the page's
    /// frame loop, and returns the backlog to carry into the next frame (0 once the game stops
    /// needing frames). The caller still owns the first frame of a run, which calls
    /// `update_simulation(0.0)` instead, and skips frames entirely while
    /// `needs_animation_frame()` is false.
    pub fn advance(&mut self, available_seconds: f64) -> f64 {
        let substeps = run_bounded_simulation_substeps(available_seconds, |delta_seconds| {
            self.update_simulation(delta_seconds);
            self.needs_animation_frame()
        });
        if self.needs_animation_frame() { substeps.remaining_seconds } else { 0.0 }
    }

    pub fn perform_modal_action(&mut self, action: ModalAction) {
        match action {
            ModalAction::Resume => self.resume_battle(),
            ModalAction::PlayUnlocked => {
                let index = if self.campaign_cleared { 0 } else { self.highest_unlocked_level_index };
                self.start_level_by_index(index);
            }
            ModalAction::RestartCampaign => self.restart_campaign(),
            ModalAction::NextLevel => self.start_next_level(),
            ModalAction::Replay => self.restart(),
            ModalAction::CampaignMap => self.open_menu(),
        }
    }

    fn apply_update_result(&mut self, result: &mut UpdateResult) {
        for particle in result.particles.drain(..) {
            if self.runtime.particles.len() < MAX_PARTICLES {
                self.runtime.particles.push(particle);
            }
        }
        self.runtime.links.append(&mut result.links);
        self.runtime.projectiles.append(&mut result.projectiles);
        self.runtime.missiles.append(&mut result.missiles);
        self.runtime.drones.append(&mut result.drones);
        self.sounds.append(&mut result.sounds);
        result.clear();
    }

    fn apply_monster_lifecycle_results(&mut self, result: &mut UpdateResult) {
        let killed = std::mem::take(&mut result.killed_monsters);
        for monster in &killed {
            self.on_monster_killed(monster, result);
            if matches!(monster.borrow().special, MonsterSpecial::Splitter) {
                self.spawn_splitters(monster);
            }
        }

        let escaped = std::mem::take(&mut result.escaped_monsters);
        for monster in &escaped {
            let can_resolve_escape = matches!(self.state, GameState::Playing | GameState::DefeatPending);
            if !can_resolve_escape || (self.runtime.escapes_left == 0 && self.breach_resolution_delay_seconds == 0.0) {
                break;
            }
            self.on_monster_escaped(monster, result);
        }
        result.killed_monsters = killed;
        result.escaped_monsters = escaped;
    }

    fn refresh_active_monsters(&mut self) {
        self.active_monsters.clear();
        for monster in &self.runtime.monsters {
            if monster.borrow().is_active() {
                self.active_monsters.push(monster.clone());
            }
        }
    }

    fn update_presentation_effects(&mut self, delta_seconds: f64) {
        let context = make_context(
            delta_seconds,
            &self.profile,
            self.visible_field_bounds,
            &self.active_monsters,
            &self.monster_collision_index,
            &self.runtime.drones,
            &self.drone_assignments,
        );
        for particle in &mut self.runtime.particles {
            particle.update(&context);
        }
        for link in &mut self.runtime.links {
            link.update(&context);
        }
    }

    pub fn update_simulation(&mut self, delta_seconds: f64) {
        self.simulation_seconds += delta_seconds;
        let previous_pre_wave_second =
            if self.state == GameState::Playing && self.active_wave().is_some() && self.runtime.spawn_delay > 0.0 {
                self.runtime.spawn_delay.ceil()
            } else {
                -1.0
            };

        if self.banner_timer > 0.0 {
            self.banner_timer = (self.banner_timer - delta_seconds).max(0.0);
            if self.banner_timer == 0.0 {
                self.request_hud_sync();
            }
        }

        // Reaching zero escapes commits the defeat. This delay is presentation-only,
        // so it intentionally continues while paused or while the campaign map is open.
        if self.breach_resolution_delay_seconds > 0.0 {
            self.breach_resolution_delay_seconds = (self.breach_resolution_delay_seconds - delta_seconds).max(0.0);
            if self.breach_resolution_delay_seconds == 0.0 {
                self.lose_level();
            }
        }

        if self.state == GameState::Playing {
            self.update_battle(delta_seconds, previous_pre_wave_second);
        } else if self.state == GameState::DefeatPending {
            self.update_presentation_effects(delta_seconds);
            self.runtime.compact_removed();
        }
    }

    fn update_battle(&mut self, delta_seconds: f64, previous_pre_wave_second: f64) {
        let wave = self.active_wave().map(|wave| (wave.count, wave.spawn_interval_min, wave.spawn_interval_max));
        if wave.is_some() && self.runtime.spawn_delay > 0.0 {
            self.runtime.spawn_delay = (self.runtime.spawn_delay - delta_seconds).max(0.0);
            if self.runtime.spawn_delay == 0.0 {
                self.play_sound(AudioCue::WaveStart, None, None);
            }
            let next_pre_wave_second = if self.active_wave().is_some() && self.runtime.spawn_delay > 0.0 {
                self.runtime.spawn_delay.ceil()
            } else {
                -1.0
            };
            if previous_pre_wave_second != next_pre_wave_second {
                self.request_hud_sync();
            }
        } else if let Some((count, interval_min, interval_max)) = wave
            && self.runtime.wave_spawned_monsters < count
        {
            self.runtime.spawn_cooldown -= delta_seconds;
            if self.runtime.spawn_cooldown <= TIMER_EPSILON_SECONDS {
                self.spawn_monster();
                self.runtime.spawn_cooldown += random_range(interval_min, interval_max);
                self.request_hud_sync();
            }
        }

        let mut result = std::mem::take(&mut self.update_result);
        result.clear();
        result.particle_limit = MAX_PARTICLES.saturating_sub(self.runtime.particles.len());
        result.link_limit = MAX_LINKS.saturating_sub(self.runtime.links.len());

        {
            let context = make_context(
                delta_seconds,
                &self.profile,
                self.visible_field_bounds,
                &self.active_monsters,
                &self.monster_collision_index,
                &self.runtime.drones,
                &self.drone_assignments,
            );
            for monster in &self.runtime.monsters {
                Monster::update(monster, &context, &mut result);
            }
        }
        self.apply_monster_lifecycle_results(&mut result);

        if !self.can_perform_battle_action() {
            self.update_presentation_effects(delta_seconds);
            self.runtime.compact_removed();
            self.apply_update_result(&mut result);
            self.update_result = result;
            return;
        }

        self.refresh_active_monsters();
        if !self.runtime.projectiles.is_empty() || !self.runtime.missiles.is_empty() {
            self.monster_collision_index.rebuild(&self.active_monsters);
        }

        {
            let context = make_context(
                delta_seconds,
                &self.profile,
                self.visible_field_bounds,
                &self.active_monsters,
                &self.monster_collision_index,
                &self.runtime.drones,
                &self.drone_assignments,
            );
            for projectile in &mut self.runtime.projectiles {
                projectile.update(&context, &mut result);
            }
            for missile in &mut self.runtime.missiles {
                missile.update(&context, &mut result);
            }
        }

        refresh_drone_assignments(&self.runtime.drones, &mut self.drone_assignments);
        for index in 0..self.runtime.drones.len() {
            let drone = self.runtime.drones[index].clone();
            let previous_target = drone.borrow().get_assigned_target();
            {
                let context = make_context(
                    delta_seconds,
                    &self.profile,
                    self.visible_field_bounds,
                    &self.active_monsters,
                    &self.monster_collision_index,
                    &self.runtime.drones,
                    &self.drone_assignments,
                );
                drone.borrow_mut().update(&context, &mut result);
            }
            let next_target = drone.borrow().get_assigned_target();
            if !same_monster(&previous_target, &next_target) {
                if let Some(previous_target) = previous_target {
                    let count = self.drone_assignments.or_insert(previous_target.borrow().id, 1);
                    *count -= 1;
                }
                if let Some(next_target) = next_target {
                    *self.drone_assignments.or_insert(next_target.borrow().id, 0) += 1;
                }
            }
        }

        self.update_presentation_effects(delta_seconds);

        {
            let context = make_context(
                delta_seconds,
                &self.profile,
                self.visible_field_bounds,
                &self.active_monsters,
                &self.monster_collision_index,
                &self.runtime.drones,
                &self.drone_assignments,
            );
            for tower in &self.runtime.towers {
                Tower::update(tower, &context, &mut result);
            }
        }

        self.runtime.compact_removed();
        self.apply_update_result(&mut result);
        self.update_result = result;

        if let Some((count, _, _)) = wave
            && self.runtime.wave_spawned_monsters >= count
            && self.runtime.monsters.is_empty()
        {
            self.complete_current_wave();
        }

        let all_spawned =
            self.current_level().is_some_and(|level| self.runtime.spawned_monsters >= level.monster_count);
        if self.active_wave().is_none() && all_spawned && self.runtime.monsters.is_empty() {
            self.runtime.win_delay += delta_seconds;
            if self.runtime.win_delay >= 0.6 && self.state == GameState::Playing {
                self.finish_level();
            }
        } else {
            self.runtime.win_delay = 0.0;
        }
    }
}

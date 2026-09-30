use js_sys::Float64Array;
use vd_core::audio::SoundEvent;
use vd_core::entities::towers::registry::find_tower_shortcut;
use vd_core::game::{CanvasActionHit, Game};
use vd_core::profile::GameMode;
use vd_core::rng;
use vd_core::types::{ModalAction, Point, TowerKind};
use vd_core::view::{RuntimeHudStats, create_hud_json, create_modal_json_or_null, tower_catalog_json};
use vd_render::{BoardRenderer as Renderer, SurfaceRect};
use wasm_bindgen::prelude::*;

use crate::storage::BrowserStorage;

#[wasm_bindgen(start)]
fn start() {
    console_error_panic_hook::set_once();
}

fn mode(mobile: bool) -> GameMode {
    if mobile { GameMode::Mobile } else { GameMode::Desktop }
}

/// The board renderer before it is handed to a game (see `createBoardRenderer`).
#[wasm_bindgen]
pub struct BoardRenderer {
    pub(crate) inner: Renderer,
}

#[wasm_bindgen]
impl BoardRenderer {
    /// Per-phase startup milliseconds as a JSON object.
    #[wasm_bindgen(js_name = startupTimings)]
    pub fn startup_timings(&self) -> String {
        self.inner.startup_timings().to_json()
    }
}

/// Creates the renderer on a device the page already requested: pipelines compile in parallel
/// while the CPU builds geometry, then a warm-up frame is drained before this resolves.
#[wasm_bindgen(js_name = createBoardRenderer)]
pub async fn create_board_renderer(
    device: web_sys::GpuDevice,
    canvas: web_sys::HtmlCanvasElement,
    overlay: web_sys::HtmlCanvasElement,
    mobile: bool,
    shader_salt: u32,
) -> Result<BoardRenderer, JsValue> {
    let profile = vd_core::profile::GameProfile::for_mode(mode(mobile));
    // The page watches `device.lost` itself and remounts with a fresh device.
    let inner = Renderer::create(device, canvas, overlay, &profile, shader_salt, None).await?;
    Ok(BoardRenderer { inner })
}

/// The game plus (once attached) its board renderer. Every method mirrors what the TypeScript
/// `Game` offered the session; HUD and modal snapshots come back as JSON, and sounds as a flat
/// `[cue, panX, intensity, ...]` array (NaN where the manifest default applies).
#[wasm_bindgen]
pub struct WebGame {
    pub(crate) game: Game,
    pub(crate) renderer: Option<Renderer>,
    sounds: Vec<SoundEvent>,
    #[cfg(feature = "labs")]
    pub(crate) benchmark: Option<crate::labs::BenchmarkRefill>,
}

impl WebGame {
    fn rect(left: f64, top: f64, width: f64, height: f64) -> SurfaceRect {
        SurfaceRect { left, top, width, height }
    }

    /// Keeps the gameplay bounds in step with the renderer's framing.
    fn sync_bounds(&mut self) {
        if let Some(renderer) = &self.renderer {
            self.game.set_visible_field_bounds(renderer.visible_field_bounds());
        } else {
            self.game.set_visible_field_bounds(self.game.profile.placement.bounds);
        }
    }

    fn after_step(&mut self) {
        #[cfg(feature = "labs")]
        crate::labs::refill_benchmark(self);
    }
}

#[wasm_bindgen]
impl WebGame {
    #[wasm_bindgen(constructor)]
    pub fn new(mobile: bool, seed: f64) -> WebGame {
        rng::seed(seed as u64);
        WebGame {
            game: Game::new(mode(mobile), Box::new(BrowserStorage::new())),
            renderer: None,
            sounds: Vec::new(),
            #[cfg(feature = "labs")]
            benchmark: None,
        }
    }

    /// Takes ownership of the renderer (the JS handle becomes unusable), disposing any previous one.
    #[wasm_bindgen(js_name = attachRenderer)]
    pub fn attach_renderer(&mut self, renderer: BoardRenderer) {
        if let Some(mut previous) = self.renderer.replace(renderer.inner) {
            previous.dispose();
        }
        self.resize();
        self.game.request_hud_sync();
    }

    #[wasm_bindgen(js_name = detachRenderer)]
    pub fn detach_renderer(&mut self) {
        if let Some(mut renderer) = self.renderer.take() {
            renderer.dispose();
        }
        self.sync_bounds();
        self.game.request_hud_sync();
    }

    pub fn resize(&mut self) {
        if let Some(renderer) = &mut self.renderer {
            renderer.resize();
        }
        self.sync_bounds();
    }

    pub fn draw(&mut self) {
        if let Some(renderer) = &mut self.renderer {
            renderer.draw(&self.game);
        }
    }

    #[wasm_bindgen(js_name = needsAnimationFrame)]
    pub fn needs_animation_frame(&self) -> bool {
        self.game.needs_animation_frame()
    }

    #[wasm_bindgen(js_name = canPerformBattleAction)]
    pub fn can_perform_battle_action(&self) -> bool {
        self.game.can_perform_battle_action()
    }

    #[wasm_bindgen(js_name = updateSimulation)]
    pub fn update_simulation(&mut self, delta_seconds: f64) {
        self.game.update_simulation(delta_seconds);
        self.after_step();
    }

    /// Runs bounded substeps for the frame's elapsed time; returns the backlog to carry over.
    pub fn advance(&mut self, available_seconds: f64) -> f64 {
        let remaining = self.game.advance(available_seconds);
        self.after_step();
        remaining
    }

    #[wasm_bindgen(js_name = requestHudSync)]
    pub fn request_hud_sync(&mut self) {
        self.game.request_hud_sync();
    }

    /// The HUD snapshot as JSON when it changed (or `force`), otherwise undefined.
    #[wasm_bindgen(js_name = takeHud)]
    pub fn take_hud(
        &mut self,
        force: bool,
        fps: f64,
        frame_time_ms: f64,
        update_time_ms: f64,
        draw_time_ms: f64,
    ) -> Option<String> {
        if !force && !self.game.hud_dirty {
            return None;
        }
        self.game.hud_dirty = false;
        let stats = RuntimeHudStats { fps, frame_time_ms, update_time_ms, draw_time_ms };
        Some(create_hud_json(&self.game, &stats))
    }

    /// The modal view as JSON (`"null"` when none) when it changed (or `force`), otherwise undefined.
    #[wasm_bindgen(js_name = takeModal)]
    pub fn take_modal(&mut self, force: bool) -> Option<String> {
        if !force && !self.game.modal_dirty {
            return None;
        }
        self.game.modal_dirty = false;
        Some(create_modal_json_or_null(&self.game))
    }

    #[wasm_bindgen(js_name = takeSounds)]
    pub fn take_sounds(&mut self) -> Vec<f64> {
        self.game.drain_sounds_into(&mut self.sounds);
        let mut flat = Vec::with_capacity(self.sounds.len() * 3);
        for sound in self.sounds.drain(..) {
            flat.push(sound.cue as u8 as f64);
            flat.push(sound.pan_x.unwrap_or(f64::NAN));
            flat.push(sound.intensity.unwrap_or(f64::NAN));
        }
        flat
    }

    /// Toolbar metadata (kind, label, summary, baseCost, shortcuts) in toolbar order, as JSON.
    #[wasm_bindgen(js_name = towerCatalog)]
    pub fn tower_catalog(&self) -> String {
        tower_catalog_json()
    }

    /// The field point under a client position, or undefined off the ground.
    #[wasm_bindgen(js_name = clientToField)]
    pub fn client_to_field(
        &self,
        client_x: f64,
        client_y: f64,
        left: f64,
        top: f64,
        width: f64,
        height: f64,
    ) -> Option<Float64Array> {
        let renderer = self.renderer.as_ref()?;
        let point = renderer.client_to_field(client_x, client_y, &Self::rect(left, top, width, height))?;
        Some(Float64Array::from(&[point.x, point.y][..]))
    }

    #[wasm_bindgen(js_name = setPointer)]
    pub fn set_pointer(&mut self, x: f64, y: f64) {
        self.game.set_pointer(Some(Point::new(x, y)));
    }

    #[wasm_bindgen(js_name = clearPointer)]
    pub fn clear_pointer(&mut self) {
        self.game.set_pointer(None);
    }

    pub fn pointer(&self) -> Option<Float64Array> {
        self.game.runtime.pointer.map(|point| Float64Array::from(&[point.x, point.y][..]))
    }

    /// A board click at a field point; the renderer's overlay decides whether it hit a tower action.
    #[wasm_bindgen(js_name = handleBoardClick)]
    pub fn handle_board_click(&mut self, x: f64, y: f64) {
        let point = Point::new(x, y);
        let hit = match &self.renderer {
            Some(renderer) => CanvasActionHit {
                upgrade_button: renderer.is_point_in_upgrade_button(point),
                laser_lock_button: renderer.is_point_in_laser_lock_button(point),
            },
            None => CanvasActionHit { upgrade_button: false, laser_lock_button: false },
        };
        self.game.handle_board_click(point, hit);
    }

    #[wasm_bindgen(js_name = placingTower)]
    pub fn placing_tower(&self) -> Option<String> {
        self.game.runtime.placing_tower.map(|kind| kind.as_str().to_string())
    }

    #[wasm_bindgen(js_name = isTowerAvailable)]
    pub fn is_tower_available(&self, kind: &str) -> bool {
        TowerKind::parse(kind).is_some_and(|kind| self.game.is_tower_available(kind))
    }

    #[wasm_bindgen(js_name = startTowerPlacement)]
    pub fn start_tower_placement(&mut self, kind: &str) {
        if let Some(kind) = TowerKind::parse(kind) {
            self.game.start_tower_placement(kind);
        }
    }

    #[wasm_bindgen(js_name = toggleTowerPlacement)]
    pub fn toggle_tower_placement(&mut self, kind: &str) {
        if let Some(kind) = TowerKind::parse(kind) {
            self.game.toggle_tower_placement(kind);
        }
    }

    #[wasm_bindgen(js_name = cancelTowerPlacement)]
    pub fn cancel_tower_placement(&mut self) {
        self.game.cancel_tower_placement();
    }

    #[wasm_bindgen(js_name = placeTower)]
    pub fn place_tower(&mut self, kind: &str, x: f64, y: f64) -> bool {
        TowerKind::parse(kind).is_some_and(|kind| self.game.place_tower(kind, Point::new(x, y)))
    }

    #[wasm_bindgen(js_name = canPlaceTower)]
    pub fn can_place_tower(&self, x: f64, y: f64) -> bool {
        self.game.can_place_tower(Point::new(x, y))
    }

    /// The available tower a keyboard shortcut selects in the current level.
    #[wasm_bindgen(js_name = towerForShortcut)]
    pub fn tower_for_shortcut(&self, key: &str) -> Option<String> {
        let available = self.game.current_level().map(|level| level.available_towers.as_slice()).unwrap_or(&[]);
        find_tower_shortcut(key, available).map(|kind| kind.as_str().to_string())
    }

    #[wasm_bindgen(js_name = togglePause)]
    pub fn toggle_pause(&mut self) {
        self.game.toggle_pause();
    }

    #[wasm_bindgen(js_name = skipBuildBreak)]
    pub fn skip_build_break(&mut self) {
        self.game.skip_build_break();
    }

    #[wasm_bindgen(js_name = openMenu)]
    pub fn open_menu(&mut self) {
        self.game.open_menu();
    }

    pub fn restart(&mut self) {
        self.game.restart();
    }

    #[wasm_bindgen(js_name = upgradeSelectedTower)]
    pub fn upgrade_selected_tower(&mut self) {
        self.game.upgrade_selected_tower();
    }

    #[wasm_bindgen(js_name = toggleSelectedLaserLock)]
    pub fn toggle_selected_laser_lock(&mut self) {
        self.game.toggle_selected_laser_lock();
    }

    #[wasm_bindgen(js_name = deselectTower)]
    pub fn deselect_tower(&mut self) {
        self.game.clear_selection();
    }

    #[wasm_bindgen(js_name = sellSelectedTower)]
    pub fn sell_selected_tower(&mut self) {
        self.game.sell_selected_tower();
    }

    #[wasm_bindgen(js_name = performModalAction)]
    pub fn perform_modal_action(&mut self, action: &str) {
        if let Some(action) = ModalAction::parse(action) {
            self.game.perform_modal_action(action);
        }
    }

    #[wasm_bindgen(js_name = startLevelByIndex)]
    pub fn start_level_by_index(&mut self, index: usize) {
        self.game.start_level_by_index(index);
    }

    #[wasm_bindgen(js_name = finishLevel)]
    pub fn finish_level(&mut self) {
        self.game.finish_level();
    }

    #[wasm_bindgen(js_name = loseLevel)]
    pub fn lose_level(&mut self) {
        self.game.lose_level();
    }

    #[wasm_bindgen(js_name = unlockAllLevelsForDebug)]
    pub fn unlock_all_levels_for_debug(&mut self) {
        self.game.unlock_all_levels_for_debug();
    }

    #[wasm_bindgen(js_name = tiltBy)]
    pub fn tilt_by(&mut self, delta_radians: f64) -> bool {
        self.renderer.as_mut().is_some_and(|renderer| renderer.tilt_by(delta_radians))
    }

    #[wasm_bindgen(js_name = zoomAt)]
    #[allow(clippy::too_many_arguments)]
    pub fn zoom_at(
        &mut self,
        factor: f64,
        client_x: f64,
        client_y: f64,
        left: f64,
        top: f64,
        width: f64,
        height: f64,
    ) -> bool {
        let rect = Self::rect(left, top, width, height);
        self.renderer.as_mut().is_some_and(|renderer| renderer.zoom_at(factor, client_x, client_y, &rect))
    }

    #[wasm_bindgen(js_name = panBetween)]
    #[allow(clippy::too_many_arguments)]
    pub fn pan_between(
        &mut self,
        from_x: f64,
        from_y: f64,
        to_x: f64,
        to_y: f64,
        left: f64,
        top: f64,
        width: f64,
        height: f64,
    ) -> bool {
        let rect = Self::rect(left, top, width, height);
        self.renderer.as_mut().is_some_and(|renderer| renderer.pan_between(from_x, from_y, to_x, to_y, &rect))
    }

    /// Frames the field inside the canvas area the HUD leaves uncovered (CSS pixels). Returns
    /// whether the framing changed.
    #[wasm_bindgen(js_name = setViewInsets)]
    pub fn set_view_insets(&mut self, top: f32, right: f32, bottom: f32, left: f32) -> bool {
        let changed = self.renderer.as_mut().is_some_and(|renderer| renderer.set_view_insets(top, right, bottom, left));
        if changed {
            self.sync_bounds();
        }
        changed
    }

    #[wasm_bindgen(js_name = resetView)]
    pub fn reset_view(&mut self) -> bool {
        self.renderer.as_mut().is_some_and(|renderer| renderer.reset_view())
    }
}

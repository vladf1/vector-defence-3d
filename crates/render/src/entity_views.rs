//! The entity views (towers, monsters, projectiles, effects, placement) and their adapter
//! onto `SceneViews` for one `Game` frame.
use vd_core::game::Game;
use vd_core::types::FieldBounds;

use crate::effect_view::EffectView;
use crate::frame_composer::{SceneFrame, SceneViews};
use crate::monster_view::MonsterView;
use crate::placement_view::PlacementView;
use crate::projectile_view::ProjectileView;
use crate::tower_view::TowerView;

/// Per-entity view state, kept across frames and reset with each level runtime.
#[derive(Default)]
pub struct EntityViews {
    pub towers: TowerView,
    pub monsters: MonsterView,
    pub projectiles: ProjectileView,
    pub effects: EffectView,
    pub placement: PlacementView,
}

/// One frame's views over a `Game` (the renderer builds it in `draw`).
pub struct GameScene<'a> {
    pub game: &'a Game,
    pub views: &'a mut EntityViews,
    /// The renderer's visible field bounds, for placement validity and crosshair guides.
    pub bounds: FieldBounds,
}

impl SceneViews for GameScene<'_> {
    fn reset(&mut self) {
        let views = &mut *self.views;
        views.towers.reset();
        views.monsters.reset(&self.game.runtime);
        views.projectiles.reset();
        views.effects.reset();
    }

    fn write(&mut self, scene: &mut SceneFrame<'_>) {
        let runtime = &self.game.runtime;
        let views = &mut *self.views;
        let SceneFrame { batches, fx, frame } = scene;
        views.towers.write(&runtime.towers, runtime.selected_tower.as_ref(), batches, fx, frame);
        views.monsters.write(runtime, batches, fx, frame);
        views.projectiles.write(runtime, &views.monsters, batches, frame);
        views.effects.write(runtime, &views.monsters, &views.projectiles, batches, fx, frame);
        views.placement.write(self.game, self.bounds, batches, frame);
    }
}

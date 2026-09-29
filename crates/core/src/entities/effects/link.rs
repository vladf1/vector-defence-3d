//! Tower-to-monster links: the slow tower's frost links (`LinkEffect`) and the lightning
//! tower's chain arcs (`LightningLinkEffect`), whose later segments start at a monster.
use std::rc::Rc;

use crate::entities::{MonsterRef, TowerRef, next_entity_id};
use crate::types::{Color, Point};
use crate::update::UpdateContext;

/// The slow link color gets a brighter start (`#d8ff4f`).
pub const SLOW_LINK_COLOR: Color = 0xd8ff4f;
const LIGHTNING_LINK_FADE_PER_SECOND: f64 = 3.8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkKind {
    /// `LinkEffect`.
    Slow,
    /// `LightningLinkEffect`.
    Lightning,
}

#[derive(Clone, Debug)]
pub enum LinkSource {
    Tower(TowerRef),
    Monster(MonsterRef),
}

impl LinkSource {
    pub fn position(&self) -> Point {
        match self {
            LinkSource::Tower(tower) => {
                let tower = tower.borrow();
                Point::new(tower.x, tower.y)
            }
            LinkSource::Monster(monster) => {
                let monster = monster.borrow();
                Point::new(monster.x, monster.y)
            }
        }
    }

    /// Monster sources include their hit shake; towers never shake.
    pub fn visual_position(&self) -> Point {
        match self {
            LinkSource::Tower(tower) => {
                let tower = tower.borrow();
                Point::new(tower.x, tower.y)
            }
            LinkSource::Monster(monster) => {
                let monster = monster.borrow();
                Point::new(monster.visual_x(), monster.visual_y())
            }
        }
    }

    /// The source tower's level (`undefined` for monster sources in the TS).
    pub fn level(&self) -> Option<u32> {
        match self {
            LinkSource::Tower(tower) => Some(tower.borrow().level),
            LinkSource::Monster(_) => None,
        }
    }

    pub fn is_removed(&self) -> bool {
        match self {
            LinkSource::Tower(tower) => tower.borrow().removed,
            LinkSource::Monster(monster) => monster.borrow().removed,
        }
    }

    pub fn is_same(&self, other: &LinkSource) -> bool {
        match (self, other) {
            (LinkSource::Tower(a), LinkSource::Tower(b)) => Rc::ptr_eq(a, b),
            (LinkSource::Monster(a), LinkSource::Monster(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Link {
    pub id: u32,
    pub kind: LinkKind,
    pub source: LinkSource,
    pub target: MonsterRef,
    pub color: Color,
    pub alpha: f64,
    pub alpha_fade_per_second: f64,
    pub age_seconds: f64,
    pub removed: bool,
}

impl Link {
    /// `new LinkEffect(target, color, alphaFadePerSecond, source)`.
    pub fn slow(target: MonsterRef, color: Color, alpha_fade_per_second: f64, source: LinkSource) -> Link {
        Link {
            id: next_entity_id(),
            kind: LinkKind::Slow,
            source,
            target,
            color,
            alpha: if color == SLOW_LINK_COLOR { 0.8 } else { 0.7 },
            alpha_fade_per_second,
            age_seconds: 0.0,
            removed: false,
        }
    }

    /// `new LightningLinkEffect(source, target, color)`.
    pub fn lightning(source: LinkSource, target: MonsterRef, color: Color) -> Link {
        Link {
            id: next_entity_id(),
            kind: LinkKind::Lightning,
            source,
            target,
            color,
            alpha: 0.92,
            alpha_fade_per_second: LIGHTNING_LINK_FADE_PER_SECOND,
            age_seconds: 0.0,
            removed: false,
        }
    }

    pub fn update(&mut self, context: &UpdateContext) {
        if self.target.borrow().removed || self.source.is_removed() {
            self.alpha = 0.0;
        } else {
            self.age_seconds += context.delta_seconds;
            self.alpha -= self.alpha_fade_per_second * context.delta_seconds;
        }
        if self.alpha <= 0.0 {
            self.removed = true;
        }
    }
}

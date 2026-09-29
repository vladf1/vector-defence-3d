//! Gameplay entities: towers, monsters, projectiles, and presentation effects. Entities hold no
//! drawing code; the renderer reads their public fields and presentation getters every frame.
use std::cell::{Cell, RefCell};
use std::rc::Rc;

pub mod drone_visuals;
pub mod effects;
pub mod monsters;
pub mod projectiles;
pub mod towers;

pub use effects::link::{Link, LinkKind, LinkSource};
pub use effects::particle::{Particle, ParticleKind};
pub use monsters::monster::{Monster, MonsterSpecial};
pub use projectiles::drone::Drone;
pub use projectiles::missile::Missile;
pub use projectiles::projectile::{Projectile, ProjectileKind};
pub use towers::tower::{Tower, TowerSpecial};

/// Monsters are referenced by towers, drones, missiles, and links (TS object identity).
pub type MonsterRef = Rc<RefCell<Monster>>;
/// Towers are referenced by the selection and by the links they emit.
pub type TowerRef = Rc<RefCell<Tower>>;
/// Drones are read by every other drone while one of them updates.
pub type DroneRef = Rc<RefCell<Drone>>;

thread_local! {
    static NEXT_ENTITY_ID: Cell<u32> = const { Cell::new(1) };
}

/// A unique, monotonically increasing id so the renderer can track entities across frames.
pub fn next_entity_id() -> u32 {
    NEXT_ENTITY_ID.with(|next| {
        let id = next.get();
        next.set(id.wrapping_add(1).max(1));
        id
    })
}

pub fn share<T>(value: T) -> Rc<RefCell<T>> {
    Rc::new(RefCell::new(value))
}

//! Straight-flying gun and drone shots that hit the first monster their sweep touches.
use crate::audio::AudioCue;
use crate::collision::CircleSweep;
use crate::combat_effects::create_hit_impact_particles;
use crate::entities::drone_visuals::drone_accent_color;
use crate::entities::next_entity_id;
use crate::types::{Color, Point};
use crate::update::{UpdateContext, UpdateResult};
use crate::utils::{angle_between, is_outside_bounds};

const PROJECTILE_DAMAGE_BASE: f64 = 10.0;
const PROJECTILE_SIZE_BASE: f64 = 3.0;
const PROJECTILE_SIZE_PER_LEVEL: f64 = 0.5;
pub const GUN_PROJECTILE_SPEED_PER_SECOND: f64 = 420.0;
pub const GUN_PROJECTILE_IMPACT_COLOR: Color = 0x9fffe4;

pub const DRONE_PROJECTILE_SPEED_PER_SECOND: f64 = 560.0;
const DRONE_PROJECTILE_DAMAGE_BASE: f64 = 4.752;
const DRONE_PROJECTILE_DAMAGE_PER_LEVEL: f64 = 1.056;
const DRONE_PROJECTILE_RADIUS_BASE: f64 = 1.25;
const DRONE_PROJECTILE_RADIUS_PER_LEVEL: f64 = 0.04;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ProjectileKind {
    /// `GunProjectile`; the level sets its tracer size.
    Gun { visual_level: u32 },
    /// `DroneProjectile`.
    Drone { accent_color: Color },
}

#[derive(Clone, Debug)]
pub struct Projectile {
    pub id: u32,
    pub x: f64,
    pub y: f64,
    pub previous_x: f64,
    pub previous_y: f64,
    pub velocity_x_per_second: f64,
    pub velocity_y_per_second: f64,
    pub damage: f64,
    pub radius: f64,
    pub angle: f64,
    pub removed: bool,
    pub impact_color: Color,
    pub impact_sound_intensity: Option<f64>,
    pub kind: ProjectileKind,
}

impl Projectile {
    #[allow(clippy::too_many_arguments)]
    fn new(
        source: Point,
        target: Point,
        speed_per_second: f64,
        damage: f64,
        radius: f64,
        impact_color: Color,
        impact_sound_intensity: Option<f64>,
        kind: ProjectileKind,
    ) -> Projectile {
        let angle = angle_between(source, target);
        Projectile {
            id: next_entity_id(),
            x: source.x,
            y: source.y,
            previous_x: source.x,
            previous_y: source.y,
            velocity_x_per_second: angle.cos() * speed_per_second,
            velocity_y_per_second: angle.sin() * speed_per_second,
            damage,
            radius,
            angle,
            removed: false,
            impact_color,
            impact_sound_intensity,
            kind,
        }
    }

    /// `new GunProjectile(source, target, level)`.
    pub fn gun(source: Point, target: Point, level: u32) -> Projectile {
        Projectile::new(
            source,
            target,
            GUN_PROJECTILE_SPEED_PER_SECOND,
            PROJECTILE_DAMAGE_BASE + level as f64,
            (PROJECTILE_SIZE_BASE + level as f64 * PROJECTILE_SIZE_PER_LEVEL) / 2.0,
            GUN_PROJECTILE_IMPACT_COLOR,
            None,
            ProjectileKind::Gun { visual_level: level },
        )
    }

    /// `new DroneProjectile(source, target, level)`.
    pub fn drone(source: Point, target: Point, level: u32) -> Projectile {
        let accent_color = drone_accent_color(level);
        Projectile::new(
            source,
            target,
            DRONE_PROJECTILE_SPEED_PER_SECOND,
            DRONE_PROJECTILE_DAMAGE_BASE + level as f64 * DRONE_PROJECTILE_DAMAGE_PER_LEVEL,
            DRONE_PROJECTILE_RADIUS_BASE + level as f64 * DRONE_PROJECTILE_RADIUS_PER_LEVEL,
            accent_color,
            Some(0.08),
            ProjectileKind::Drone { accent_color },
        )
    }

    pub fn sweep(&self) -> CircleSweep {
        CircleSweep {
            previous_x: self.previous_x,
            previous_y: self.previous_y,
            x: self.x,
            y: self.y,
            radius: self.radius,
        }
    }

    pub fn update(&mut self, context: &UpdateContext, result: &mut UpdateResult) {
        self.previous_x = self.x;
        self.previous_y = self.y;
        self.x += self.velocity_x_per_second * context.delta_seconds;
        self.y += self.velocity_y_per_second * context.delta_seconds;

        if let Some((target, collision)) = context.find_earliest_collision(&self.sweep()) {
            self.x = collision.x;
            self.y = collision.y;
            target.borrow_mut().take_damage(self.damage);
            self.removed = true;
            let particles = create_hit_impact_particles(
                self.x,
                self.y,
                self.impact_color,
                Some(self.angle),
                result.remaining_particle_capacity(),
            );
            result.add_particles(particles);
            result.play_sound(AudioCue::ProjectileImpact, Some(self.x), self.impact_sound_intensity);
            return;
        }

        if is_outside_bounds(self.x, self.y, &context.field_bounds, 20.0) {
            self.removed = true;
        }
    }
}

//! Simulation particles. Every TS particle subclass is a `ParticleKind` sharing one struct:
//! plain sparks (hit, laser, and missile-trail particles), glass shards, escape fragments,
//! hit rings, missile smoke and ember streaks, shockwaves, and the tank's track prints and turret.
use std::f64::consts::PI;
use std::sync::LazyLock;

use crate::entities::next_entity_id;
use crate::types::{Color, Point};
use crate::update::UpdateContext;
use crate::utils::{CalibratedExponentialDecay, MovingPoint, is_outside_bounds, random_range};

static VELOCITY_DECAY: LazyLock<CalibratedExponentialDecay> =
    LazyLock::new(|| CalibratedExponentialDecay::new(2.4, 60.0));
static GLASS_SHARD_DRIFT_DECAY: LazyLock<CalibratedExponentialDecay> =
    LazyLock::new(|| CalibratedExponentialDecay::new(0.42, 60.0));
static ESCAPE_FRAGMENT_DRIFT_DECAY: LazyLock<CalibratedExponentialDecay> =
    LazyLock::new(|| CalibratedExponentialDecay::new(0.58, 60.0));
static TANK_TURRET_DRIFT_DECAY: LazyLock<CalibratedExponentialDecay> =
    LazyLock::new(|| CalibratedExponentialDecay::new(0.34, 60.0));

const GLASS_SHARD_ANGULAR_VELOCITY_MAX_PER_SECOND: f64 = 9.9;
const MISSILE_EXPLOSION_SCALE_BASE: f64 = 0.8;
const MISSILE_EXPLOSION_SCALE_PER_LEVEL: f64 = 0.06;
pub const SMOKE_COLOR: Color = 0x7d7b72;
pub const SHOCKWAVE_COLOR: Color = 0xfff0a8;
pub const EMBER_HOT_COLOR: Color = 0xfff0a8;
pub const EMBER_WARM_COLOR: Color = 0xff8f45;
pub const TANK_TRACK_PRINT_COLOR: Color = 0x86ad99;
const TANK_TRACK_PRINT_ALPHA: f64 = 0.46;
const TANK_TRACK_PRINT_FADE_PER_SECOND: f64 = 0.234;

/// Initial motion: velocity along `angle` and a start `offset` along it. `angle: None` picks a
/// random direction (the TS default).
#[derive(Clone, Copy, Debug)]
pub struct ParticleMotion {
    pub speed_per_second: f64,
    pub offset: f64,
    pub angle: Option<f64>,
}

#[derive(Clone, Debug)]
pub enum ParticleKind {
    /// A plain `Particle`: hit/laser sparks and missile trail sparks and smoke.
    Spark,
    GlassShard {
        rotation: f64,
        angular_velocity_per_second: f64,
        vertices: Vec<Point>,
    },
    EscapeFragment {
        rotation: f64,
        angular_velocity_per_second: f64,
        vertices: Vec<Point>,
    },
    HitRing {
        ring_color: Color,
        max_radius: f64,
    },
    Smoke {
        max_size: f64,
        growth_per_second: f64,
    },
    EmberStreak,
    Shockwave {
        age_seconds: f64,
        scale: f64,
    },
    TankTrackPrint {
        angle: f64,
    },
    TankTurret {
        radius: f64,
        rotation: f64,
        barrel_rotation: f64,
        angular_velocity_per_second: f64,
    },
}

#[derive(Clone, Debug)]
pub struct Particle {
    pub id: u32,
    pub x: f64,
    pub y: f64,
    pub velocity_x_per_second: f64,
    pub velocity_y_per_second: f64,
    pub size: f64,
    pub color: Color,
    pub alpha: f64,
    pub alpha_fade_per_second: f64,
    pub removed: bool,
    pub kind: ParticleKind,
}

pub fn get_missile_explosion_scale(level: u32) -> f64 {
    MISSILE_EXPLOSION_SCALE_BASE + MISSILE_EXPLOSION_SCALE_PER_LEVEL * level as f64
}

impl Particle {
    fn base(x: f64, y: f64, size: f64, color: Color, alpha_fade_per_second: f64, motion: ParticleMotion) -> Particle {
        let angle = motion.angle.unwrap_or_else(|| random_range(-PI, PI));
        Particle {
            id: next_entity_id(),
            velocity_x_per_second: angle.cos() * motion.speed_per_second,
            velocity_y_per_second: angle.sin() * motion.speed_per_second,
            x: x + angle.cos() * motion.offset,
            y: y + angle.sin() * motion.offset,
            size,
            color,
            alpha: 1.0,
            alpha_fade_per_second,
            removed: false,
            kind: ParticleKind::Spark,
        }
    }

    /// A plain particle (`new Particle(...)` in the TS).
    pub fn spark(
        x: f64,
        y: f64,
        size: f64,
        color: Color,
        alpha_fade_per_second: f64,
        motion: ParticleMotion,
    ) -> Particle {
        Particle::base(x, y, size, color, alpha_fade_per_second, motion)
    }

    pub fn hit_ring(x: f64, y: f64, ring_color: Color, max_radius: f64) -> Particle {
        let mut particle = Particle::base(x, y, 0.0, ring_color, 1.0, still_motion());
        particle.alpha = 0.85;
        particle.kind = ParticleKind::HitRing { ring_color, max_radius };
        particle
    }

    pub fn shockwave(x: f64, y: f64, scale: f64) -> Particle {
        let mut particle = Particle::base(x, y, 0.0, SHOCKWAVE_COLOR, 1.0, still_motion());
        particle.alpha = 1.0;
        particle.kind = ParticleKind::Shockwave { age_seconds: 0.0, scale };
        particle
    }

    pub fn smoke(x: f64, y: f64, blast_angle: f64, level: u32) -> Particle {
        let scale = get_missile_explosion_scale(level);
        let angle = blast_angle + PI + random_range(-1.1, 1.1);
        let size = random_range(3.2, 6.8) * scale;
        let alpha_fade_per_second = random_range(0.7, 1.15);
        let speed_per_second = random_range(22.0, 78.0) * scale;
        let offset = random_range(1.0, 7.0) * scale;
        let mut particle = Particle::base(
            x,
            y,
            size,
            SMOKE_COLOR,
            alpha_fade_per_second,
            ParticleMotion { speed_per_second, offset, angle: Some(angle) },
        );
        particle.alpha = random_range(0.32, 0.58);
        let max_size = size + random_range(4.0, 8.0) * scale;
        particle.kind = ParticleKind::Smoke { max_size, growth_per_second: 12.0 * scale };
        particle
    }

    pub fn ember_streak(x: f64, y: f64, blast_angle: f64, level: u32) -> Particle {
        let scale = get_missile_explosion_scale(level);
        let angle = blast_angle + PI + random_range(-1.8, 1.8);
        let size = random_range(2.1, 3.6) * scale;
        let color = if random_range(0.0, 1.0) > 0.45 { EMBER_WARM_COLOR } else { EMBER_HOT_COLOR };
        let alpha_fade_per_second = random_range(3.5, 5.4);
        let speed_per_second = random_range(175.0, 385.0) * scale;
        let offset = random_range(2.0, 6.0) * scale;
        let mut particle = Particle::base(
            x,
            y,
            size,
            color,
            alpha_fade_per_second,
            ParticleMotion { speed_per_second, offset, angle: Some(angle) },
        );
        particle.kind = ParticleKind::EmberStreak;
        particle
    }

    /// A monster breakup shard flying away from `origin` (in the unrotated outline space).
    #[allow(clippy::too_many_arguments)]
    pub fn glass_shard(
        x: f64,
        y: f64,
        color: Color,
        vertices: Vec<Point>,
        origin: Point,
        rotation: f64,
        speed_per_second: f64,
        initial_separation: f64,
    ) -> Particle {
        let mut particle =
            Particle::base(x, y, 1.0, color, 0.0, ParticleMotion { speed_per_second, offset: 0.0, angle: None });
        let centroid = get_centroid(&vertices);
        let travel_angle = (centroid.y - origin.y).atan2(centroid.x - origin.x) + rotation + random_range(-0.22, 0.22);
        particle.velocity_x_per_second = travel_angle.cos() * speed_per_second;
        particle.velocity_y_per_second = travel_angle.sin() * speed_per_second;
        particle.x += travel_angle.cos() * initial_separation;
        particle.y += travel_angle.sin() * initial_separation;
        let angular_velocity_per_second =
            random_range(-GLASS_SHARD_ANGULAR_VELOCITY_MAX_PER_SECOND, GLASS_SHARD_ANGULAR_VELOCITY_MAX_PER_SECOND);
        particle.alpha_fade_per_second = random_range(0.8, 1.8);
        particle.kind = ParticleKind::GlassShard { rotation, angular_velocity_per_second, vertices };
        particle
    }

    #[allow(clippy::too_many_arguments)]
    pub fn escape_fragment(
        x: f64,
        y: f64,
        color: Color,
        angle: f64,
        speed_per_second: f64,
        length: f64,
        width: f64,
        initial_separation: f64,
    ) -> Particle {
        let mut particle = Particle::base(
            x,
            y,
            length.max(width),
            color,
            0.0,
            ParticleMotion { speed_per_second, offset: initial_separation, angle: Some(angle) },
        );
        let rotation = angle + random_range(-0.6, 0.6);
        let angular_velocity_per_second = random_range(-13.5, 13.5);
        particle.alpha_fade_per_second = random_range(0.95, 1.55);
        let vertices = create_fragment_vertices(length, width);
        particle.kind = ParticleKind::EscapeFragment { rotation, angular_velocity_per_second, vertices };
        particle
    }

    pub fn tank_track_print(x: f64, y: f64, angle: f64) -> Particle {
        let mut particle =
            Particle::base(x, y, 1.0, TANK_TRACK_PRINT_COLOR, TANK_TRACK_PRINT_FADE_PER_SECOND, still_motion());
        particle.alpha = TANK_TRACK_PRINT_ALPHA;
        particle.kind = ParticleKind::TankTrackPrint { angle };
        particle
    }

    /// The tank's detached turret, tumbling away from the wreck.
    pub fn tank_turret(x: f64, y: f64, radius: f64, color: Color, rotation: f64, barrel_rotation: f64) -> Particle {
        let mut particle = Particle::base(
            x,
            y,
            radius * 2.0,
            color,
            0.0,
            ParticleMotion { speed_per_second: 0.0, offset: 0.0, angle: None },
        );
        let travel_angle = random_range(-PI, PI);
        let speed_per_second = random_range(115.0, 185.0);
        particle.velocity_x_per_second = travel_angle.cos() * speed_per_second;
        particle.velocity_y_per_second = travel_angle.sin() * speed_per_second;
        let angular_velocity_per_second = random_range(-12.5, 12.5);
        particle.alpha_fade_per_second = random_range(0.45, 0.78);
        particle.kind = ParticleKind::TankTurret { radius, rotation, barrel_rotation, angular_velocity_per_second };
        particle
    }

    /// The rotation of shards, fragments, and the tank turret (0 for other kinds).
    pub fn rotation(&self) -> f64 {
        match self.kind {
            ParticleKind::GlassShard { rotation, .. }
            | ParticleKind::EscapeFragment { rotation, .. }
            | ParticleKind::TankTurret { rotation, .. } => rotation,
            _ => 0.0,
        }
    }

    /// Outline vertices of glass shards and escape fragments.
    pub fn vertices(&self) -> Option<&[Point]> {
        match &self.kind {
            ParticleKind::GlassShard { vertices, .. } | ParticleKind::EscapeFragment { vertices, .. } => Some(vertices),
            _ => None,
        }
    }

    fn apply_decay(&mut self, decay: &CalibratedExponentialDecay, delta_seconds: f64) {
        decay.apply(
            MovingPoint {
                x: &mut self.x,
                y: &mut self.y,
                velocity_x_per_second: &mut self.velocity_x_per_second,
                velocity_y_per_second: &mut self.velocity_y_per_second,
            },
            delta_seconds,
        );
    }

    fn update_base(&mut self, context: &UpdateContext) {
        self.apply_decay(&VELOCITY_DECAY, context.delta_seconds);
        self.alpha -= self.alpha_fade_per_second * context.delta_seconds;
        if self.alpha <= 0.0 || is_outside_bounds(self.x, self.y, &context.field_bounds, 20.0) {
            self.removed = true;
        }
    }

    fn update_tumbling(&mut self, context: &UpdateContext, decay: &CalibratedExponentialDecay, margin: f64) {
        let delta_seconds = context.delta_seconds;
        self.apply_decay(decay, delta_seconds);
        if let ParticleKind::GlassShard { rotation, angular_velocity_per_second, .. }
        | ParticleKind::EscapeFragment { rotation, angular_velocity_per_second, .. }
        | ParticleKind::TankTurret { rotation, angular_velocity_per_second, .. } = &mut self.kind
        {
            *rotation += *angular_velocity_per_second * delta_seconds;
        }
        self.alpha = (self.alpha - self.alpha_fade_per_second * delta_seconds).max(0.0);
        if self.alpha <= 0.0 || is_outside_bounds(self.x, self.y, &context.field_bounds, margin) {
            self.removed = true;
        }
    }

    pub fn update(&mut self, context: &UpdateContext) {
        let delta_seconds = context.delta_seconds;
        match &mut self.kind {
            ParticleKind::Spark | ParticleKind::EmberStreak => self.update_base(context),
            ParticleKind::Smoke { .. } => {
                self.update_base(context);
                if let ParticleKind::Smoke { max_size, growth_per_second } = self.kind {
                    self.size = max_size.min(self.size + growth_per_second * delta_seconds);
                }
            }
            ParticleKind::GlassShard { .. } => self.update_tumbling(context, &GLASS_SHARD_DRIFT_DECAY, 28.0),
            ParticleKind::EscapeFragment { .. } => self.update_tumbling(context, &ESCAPE_FRAGMENT_DRIFT_DECAY, 34.0),
            ParticleKind::TankTurret { .. } => self.update_tumbling(context, &TANK_TURRET_DRIFT_DECAY, 34.0),
            ParticleKind::HitRing { .. } => {
                self.alpha = (self.alpha - 5.2 * delta_seconds).max(0.0);
                if self.alpha <= 0.0 {
                    self.removed = true;
                }
            }
            ParticleKind::Shockwave { age_seconds, .. } => {
                *age_seconds += delta_seconds;
                self.alpha = (1.0 - *age_seconds * 4.15).max(0.0);
                if self.alpha <= 0.0 {
                    self.removed = true;
                }
            }
            ParticleKind::TankTrackPrint { .. } => {
                self.alpha -= self.alpha_fade_per_second * delta_seconds;
                if self.alpha <= 0.0 {
                    self.removed = true;
                }
            }
        }
    }
}

fn still_motion() -> ParticleMotion {
    ParticleMotion { speed_per_second: 0.0, offset: 0.0, angle: Some(0.0) }
}

fn get_centroid(vertices: &[Point]) -> Point {
    let mut sum_x = 0.0;
    let mut sum_y = 0.0;
    for vertex in vertices {
        sum_x += vertex.x;
        sum_y += vertex.y;
    }
    let count = vertices.len() as f64;
    Point::new(sum_x / count, sum_y / count)
}

fn create_fragment_vertices(length: f64, width: f64) -> Vec<Point> {
    let point_count = 4 + random_range(0.0, 4.0).floor() as usize;
    let mut vertices = Vec::with_capacity(point_count);
    let angle_offset = random_range(-0.26, 0.26);
    for index in 0..point_count {
        let angle = angle_offset + (PI * 2.0 * index as f64) / point_count as f64 + random_range(-0.2, 0.2);
        let length_radius =
            (length / 2.0) * if angle.cos() > 0.0 { random_range(0.72, 1.22) } else { random_range(0.44, 0.95) };
        let width_radius = (width / 2.0) * random_range(0.58, 1.28);
        let x = angle.cos() * length_radius + random_range(length * -0.08, length * 0.08);
        let y = angle.sin() * width_radius + random_range(width * -0.18, width * 0.18);
        vertices.push(Point::new(x, y));
    }
    vertices
}

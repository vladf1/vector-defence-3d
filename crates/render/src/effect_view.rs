//! Maps the simulation's flat effect particles and links into 3D: shards fly, tumble, and
//! bounce; smoke rises; sparks arc; links become camera-facing energy ribbons.
use std::f64::consts::PI;

use vd_core::entities::projectiles::missile::{
    MISSILE_TRAIL_HOT_COLOR, MISSILE_TRAIL_SMOKE_COLOR, MISSILE_TRAIL_WARM_COLOR,
};
use vd_core::entities::{Link, LinkKind, LinkSource, Particle, ParticleKind};
use vd_core::level_runtime::LevelRuntime;
use vd_core::rng::{random, random_range};
use vd_core::types::{Color, Point, TowerKind};

use crate::frame_math::{FrameContext, Quat};
use crate::fx_system::FxSystem;
use crate::id_map::IdMap;
use crate::math::{Vec3, vec3};
use crate::models::{SLOW_CORE_Y, TESLA_TOP_Y};
use crate::monster_view::MonsterView;
use crate::palette::{LinearColor, linear_color};
use crate::projectile_view::ProjectileView;
use crate::render_batches::{Batch, RenderBatches};

const GRAVITY: f32 = 520.0;
const SHARD_RESTITUTION: f32 = 0.34;
const SHARD_GROUND_FRICTION: f32 = 0.6;
const DEFAULT_EFFECT_HEIGHT: f32 = 7.5;
const ESCAPE_SHOCKWAVE_MIN_SCALE: f64 = 1.3;
const TRACK_PRINT_COLOR: Color = 0x0a1e18;
const SLOW_LINK_COLOR: Color = 0xd8ff4f;
const SLOW_LINK_CORE: Color = 0xf4ff9a;
const FROST_ARC_COLOR: Color = 0x8ff7ff;
const LIGHTNING_CORE: Color = 0xffffff;
const SMOKE_TINT: Color = 0x5b5a52;
const MISSILE_SHOCKWAVE: Color = 0xf99a5f;
const ESCAPE_SHOCKWAVE: Color = 0xb0ffe1;
const TRAIL_SMOKE_TINT: Color = 0x6b7178;
const LIGHTNING_SEGMENT_LENGTH: f32 = 9.0;

fn range(min: f64, max: f64) -> f32 {
    random_range(min, max) as f32
}

fn is_missile_trail_spark(color: Color) -> bool {
    color == MISSILE_TRAIL_HOT_COLOR || color == MISSILE_TRAIL_WARM_COLOR
}

/// Renderer-side 3D state layered onto a flat simulation particle.
struct ParticleDepth {
    frame: u32,
    y: f32,
    vy: f32,
    grounded: bool,
    spin: f32,
    axis: Vec3,
    size: f32,
    shape: f32,
}

fn shard_radius(vertices: &[Point]) -> f32 {
    let count = vertices.len().max(1) as f64;
    let cx = vertices.iter().map(|vertex| vertex.x).sum::<f64>() / count;
    let cy = vertices.iter().map(|vertex| vertex.y).sum::<f64>() / count;
    let radius = vertices.iter().fold(0.0f64, |radius, vertex| radius.max((vertex.x - cx).hypot(vertex.y - cy)));
    radius.max(0.8) as f32
}

/// A link endpoint in world space.
#[derive(Clone, Copy)]
struct Endpoint {
    x: f32,
    y: f32,
    z: f32,
}

#[derive(Default)]
pub struct EffectView {
    depth: IdMap<ParticleDepth>,
    rotation: Quat,
    tumble: Quat,
}

impl EffectView {
    pub fn reset(&mut self) {
        self.depth.clear();
    }

    pub fn write(
        &mut self,
        runtime: &LevelRuntime,
        monsters: &MonsterView,
        projectiles: &ProjectileView,
        batches: &mut RenderBatches,
        fx: &mut FxSystem,
        frame: &FrameContext,
    ) {
        for particle in &runtime.particles {
            if particle.removed {
                continue;
            }
            let state = self.depth.get_or_insert_with(particle.id, |_| create_depth(particle, projectiles, fx));
            state.frame = frame.frame;
            write_particle(particle, state, &mut self.rotation, &mut self.tumble, batches, frame);
        }
        let current = frame.frame;
        self.depth.retain(|state| state.frame == current);

        for link in &runtime.links {
            if !link.removed {
                write_link(link, monsters, batches, frame);
            }
        }
    }
}

fn create_depth(particle: &Particle, projectiles: &ProjectileView, fx: &mut FxSystem) -> ParticleDepth {
    let axis_theta = random_range(0.0, PI * 2.0);
    let axis_phi = random_range(0.3, PI - 0.3);
    let spin = range(6.0, 16.0) * if random() < 0.5 { -1.0 } else { 1.0 };
    let mut state = ParticleDepth {
        frame: 0,
        y: DEFAULT_EFFECT_HEIGHT,
        vy: 0.0,
        grounded: false,
        spin,
        axis: vec3(
            (axis_phi.sin() * axis_theta.cos()) as f32,
            axis_phi.cos() as f32,
            (axis_phi.sin() * axis_theta.sin()) as f32,
        ),
        size: particle.size as f32,
        shape: 0.0,
    };

    match &particle.kind {
        ParticleKind::GlassShard { vertices, .. } => {
            state.y = range(5.0, 11.0);
            state.vy = range(70.0, 190.0);
            state.size = shard_radius(vertices);
        }
        ParticleKind::EscapeFragment { vertices, .. } => {
            state.y = range(3.0, 9.0);
            state.vy = range(110.0, 300.0);
            state.size = shard_radius(vertices) * 0.9;
        }
        ParticleKind::TankTurret { radius, .. } => {
            state.y = *radius as f32 * 0.9;
            state.vy = range(210.0, 280.0);
            state.spin = range(7.0, 12.0);
        }
        ParticleKind::TankTrackPrint { .. } => {
            state.y = 0.5;
        }
        ParticleKind::Smoke { .. } => {
            state.y = range(4.0, 10.0);
            state.vy = range(10.0, 22.0);
            state.shape = range(0.0, 4.0).floor();
        }
        ParticleKind::EmberStreak => {
            state.y = range(4.0, 9.0);
            state.vy = range(60.0, 190.0);
        }
        ParticleKind::Shockwave { scale, .. } => {
            state.y = 1.2;
            if *scale < ESCAPE_SHOCKWAVE_MIN_SCALE {
                fx.explosion(particle.x as f32, particle.y as f32, *scale as f32);
            }
        }
        ParticleKind::HitRing { .. } => {
            state.y = DEFAULT_EFFECT_HEIGHT;
        }
        ParticleKind::Spark
            if is_missile_trail_spark(particle.color) || particle.color == MISSILE_TRAIL_SMOKE_COLOR =>
        {
            state.y = projectiles.missile_altitude_near(particle.x, particle.y).unwrap_or(DEFAULT_EFFECT_HEIGHT);
            state.vy = if particle.color == MISSILE_TRAIL_SMOKE_COLOR { range(4.0, 10.0) } else { 0.0 };
            state.shape = range(0.0, 4.0).floor();
        }
        ParticleKind::Spark => {
            state.y = DEFAULT_EFFECT_HEIGHT + range(-1.5, 2.5);
            state.vy = range(20.0, 110.0);
        }
    }
    state
}

fn integrate_ballistic(state: &mut ParticleDepth, delta_seconds: f32, floor: f32, restitution: f32) {
    if state.grounded || delta_seconds <= 0.0 {
        return;
    }
    state.vy -= GRAVITY * delta_seconds;
    state.y += state.vy * delta_seconds;
    if state.y <= floor {
        state.y = floor;
        state.vy = -state.vy * restitution;
        state.spin *= SHARD_GROUND_FRICTION;
        if state.vy < 26.0 {
            state.vy = 0.0;
            state.grounded = true;
        }
    }
}

fn write_particle(
    particle: &Particle,
    state: &mut ParticleDepth,
    rotation: &mut Quat,
    tumble: &mut Quat,
    batches: &mut RenderBatches,
    frame: &FrameContext,
) {
    let dt = frame.delta_seconds;
    let alpha = particle.alpha.max(0.0) as f32;
    let color = linear_color(particle.color);
    let (x, z) = (particle.x as f32, particle.y as f32);
    let Vec3 { x: axis_x, y: axis_y, z: axis_z } = state.axis;

    match &particle.kind {
        ParticleKind::GlassShard { rotation: spin_angle, .. }
        | ParticleKind::EscapeFragment { rotation: spin_angle, .. } => {
            let thickness = state.size * 0.55;
            integrate_ballistic(state, dt, thickness * 0.5, SHARD_RESTITUTION);
            let heat = 0.35 + alpha * alpha * 2.2;
            let shrink = (alpha * 3.0).min(1.0);
            let tumble_angle = state.spin * (1.0 - alpha) * if state.grounded { 0.35 } else { 1.0 };
            tumble.set_axis_angle(axis_x, axis_y, axis_z, tumble_angle);
            rotation.set_yaw(-(*spin_angle as f32)).multiply(tumble);
            let size = state.size * shrink;
            batches.push_blob_shadow(x, z, state.y, size, size, 0.0, 0.35 * shrink);
            let q = *rotation;
            let tint = color.scaled(heat);
            batches[Batch::Shard]
                .push_quaternion(x, state.y, z, q.x, q.y, q.z, q.w, size, size, size, tint.r, tint.g, tint.b);
        }
        ParticleKind::TankTurret { radius, rotation: spin_angle, .. } => {
            let radius = *radius as f32;
            integrate_ballistic(state, dt, 0.4 * radius, 0.3);
            tumble.set_axis_angle(axis_x, axis_y, axis_z, state.spin * (1.0 - alpha) * 2.2);
            rotation.set_yaw(-(*spin_angle as f32)).multiply(tumble);
            let fade = (alpha * 3.0).min(1.0);
            let r = radius * fade;
            batches.push_blob_shadow(x, z, state.y, r * 0.8, r * 0.8, 0.0, 0.5 * fade);
            let q = *rotation;
            let tint = color.scaled(0.6 + alpha);
            batches[Batch::TankTurret]
                .push_quaternion(x, state.y, z, q.x, q.y, q.z, q.w, r, r, r, tint.r, tint.g, tint.b);
        }
        ParticleKind::TankTrackPrint { angle } => {
            let print = linear_color(TRACK_PRINT_COLOR);
            let decal = &mut batches[Batch::Decal];
            let slot = decal.push_yaw(x, state.y, z, -(*angle as f32), 2.6, 1.0, 1.5, print.r, print.g, print.b);
            decal.set_extra(slot, 0, alpha * 1.5);
            decal.set_extra(slot, 1, 1.0);
        }
        ParticleKind::Smoke { .. } => {
            state.y += state.vy * dt;
            let size = particle.size as f32 * 2.8;
            let tint = linear_color(SMOKE_TINT);
            let spin = state.spin * 0.05 * (1.0 - alpha);
            batches.smoke.push(x, state.y, z, spin, size, size, state.shape, tint.r, tint.g, tint.b, alpha * 1.1);
        }
        ParticleKind::EmberStreak => {
            integrate_ballistic(state, dt, 0.5, 0.25);
            let size = particle.size as f32;
            let length = 4.5 + size * 2.35;
            let heading = (-particle.velocity_y_per_second).atan2(particle.velocity_x_per_second) as f32;
            let glow = color.scaled(2.4);
            batches.glow.push(x, state.y, z, heading, length * 1.5, size * 1.2, 0.0, glow.r, glow.g, glow.b, alpha);
        }
        ParticleKind::Shockwave { scale, .. } => {
            let progress = 1.0 - alpha;
            let radius = (5.75 + progress * 44.0) * *scale as f32;
            let warm =
                linear_color(if *scale < ESCAPE_SHOCKWAVE_MIN_SCALE { MISSILE_SHOCKWAVE } else { ESCAPE_SHOCKWAVE });
            let (ring, disc) = (warm.scaled(1.8), warm.scaled(0.6));
            batches.push_ground_glow(x, state.y, z, radius * 1.2, 1.0, ring.r, ring.g, ring.b, alpha);
            batches.push_ground_glow(x, state.y, z, radius * 0.9, 0.0, disc.r, disc.g, disc.b, alpha * alpha);
        }
        ParticleKind::HitRing { ring_color, max_radius } => {
            let ring = linear_color(*ring_color).scaled(1.8);
            let progress = (1.0 - alpha / 0.85).min(1.0);
            let radius = 2.0 + *max_radius as f32 * progress;
            let size = radius * 2.6;
            batches.glow.push(x, state.y, z, 0.0, size, size, 1.0, ring.r, ring.g, ring.b, alpha);
        }
        ParticleKind::Spark if particle.color == MISSILE_TRAIL_SMOKE_COLOR => {
            state.y += state.vy * dt;
            let size = 7.0 + (1.0 - alpha) * 12.0;
            let tint = linear_color(TRAIL_SMOKE_TINT);
            let spin = state.spin * (1.0 - alpha) * 0.2;
            batches.smoke.push(x, state.y, z, spin, size, size, state.shape, tint.r, tint.g, tint.b, alpha * 0.5);
        }
        ParticleKind::Spark if is_missile_trail_spark(particle.color) => {
            let size = 3.0 + particle.size as f32 * 1.8;
            let glow = color.scaled(2.2);
            batches.glow.push(x, state.y, z, 0.0, size, size, 0.0, glow.r, glow.g, glow.b, alpha * 0.8);
        }
        ParticleKind::Spark => {
            integrate_ballistic(state, dt, 0.5, 0.35);
            let size = 2.2 + particle.size as f32 * 2.2;
            let glow = color.scaled(2.4);
            batches.glow.push(x, state.y, z, 0.0, size, size, 0.0, glow.r, glow.g, glow.b, alpha);
        }
    }
}

fn resolve_source(source: &LinkSource, monsters: &MonsterView) -> Endpoint {
    match source {
        LinkSource::Monster(monster) => {
            let monster = monster.borrow();
            Endpoint {
                x: monster.visual_x() as f32,
                y: monsters.center_height(monster.id, monster.radius),
                z: monster.visual_y() as f32,
            }
        }
        LinkSource::Tower(tower) => {
            let tower = tower.borrow();
            let y = if tower.kind == TowerKind::Lightning { TESLA_TOP_Y } else { SLOW_CORE_Y };
            Endpoint { x: tower.x as f32, y, z: tower.y as f32 }
        }
    }
}

fn write_link(link: &Link, monsters: &MonsterView, batches: &mut RenderBatches, frame: &FrameContext) {
    let level = link.source.level().unwrap_or(0) as f32;
    let alpha = link.alpha.max(0.0) as f32;
    let from = resolve_source(&link.source, monsters);
    let (to, target_radius) = {
        let target = link.target.borrow();
        let to = Endpoint {
            x: target.visual_x() as f32,
            y: monsters.center_height(target.id, target.radius),
            z: target.visual_y() as f32,
        };
        (to, target.radius as f32)
    };
    let age = link.age_seconds;

    match link.kind {
        LinkKind::Lightning => {
            let color = linear_color(link.color);
            let dx = to.x - from.x;
            let dz = to.z - from.z;
            let dy = to.y - from.y;
            let distance = dx.hypot(dz);
            let segments = 2.max((distance / LIGHTNING_SEGMENT_LENGTH).ceil() as u32);
            let (normal_x, normal_z) = if distance > 0.0 { (-dz / distance, dx / distance) } else { (0.0, 0.0) };
            let mut previous = from;
            let glow_color = color.scaled(alpha * 0.7);
            let core_color = linear_color(LIGHTNING_CORE).scaled(alpha * 2.2);
            for index in 1..=segments {
                let t = index as f32 / segments as f32;
                let envelope = if index == segments { 0.0 } else { (std::f32::consts::PI * t).sin() };
                let jitter = ((age * 55.0 + index as f64 * 4.31).sin() * 5.6) as f32 * envelope;
                let lift = ((age * 47.0 + index as f64 * 2.7).sin() * 3.2) as f32 * envelope;
                let next = Endpoint {
                    x: from.x + dx * t + normal_x * jitter,
                    y: from.y + dy * t + lift,
                    z: from.z + dz * t + normal_z * jitter,
                };
                push_ribbon_segment(batches, frame, previous, next, 7.0 + level * 0.4, glow_color);
                push_ribbon_segment(batches, frame, previous, next, 1.6 + level * 0.1, core_color);
                previous = next;
            }
            let arc_count = 5.min(3 + level as u32);
            let arc_radius = target_radius + 4.0 + level * 0.32;
            let spin = age * 18.0;
            let arc = color.scaled(2.2);
            for index in 0..arc_count {
                let angle = (spin + PI * 2.0 * index as f64 / arc_count as f64) as f32;
                let (ax, az) = (to.x + angle.cos() * arc_radius, to.z + angle.sin() * arc_radius);
                batches.glow.push(ax, to.y, az, -angle, 5.0, 1.6, 0.0, arc.r, arc.g, arc.b, alpha);
            }
            let halo = color.scaled(1.6);
            let size = 16.0 + level * 1.2;
            batches.glow.push(to.x, to.y, to.z, 0.0, size, size, 0.0, halo.r, halo.g, halo.b, alpha * 0.6);
        }
        LinkKind::Slow => {
            let link_color = linear_color(SLOW_LINK_COLOR);
            let core = linear_color(SLOW_LINK_CORE);
            push_ribbon_segment(batches, frame, from, to, 4.0 + level * 0.25, link_color.scaled(alpha * 0.6));
            push_ribbon_segment(batches, frame, from, to, 1.2 + level * 0.12, core.scaled(alpha * 1.4));
            let travel = ((age * 2.2) % 1.0) as f32;
            let bead = core.scaled(2.0);
            batches.glow.push(
                from.x + (to.x - from.x) * travel,
                from.y + (to.y - from.y) * travel,
                from.z + (to.z - from.z) * travel,
                0.0,
                6.0,
                6.0,
                0.0,
                bead.r,
                bead.g,
                bead.b,
                alpha,
            );
            let pulse = ((age * 18.0).sin() * 0.9) as f32;
            let ring_radius = target_radius + 3.4 + pulse + (1.0 - alpha) * 1.8;
            let frost = linear_color(FROST_ARC_COLOR).scaled(0.6);
            let size = ring_radius * 2.3;
            let turn = (age * 5.8) as f32;
            batches.glow.push(to.x, to.y, to.z, turn, size, size, 1.0, frost.r, frost.g, frost.b, alpha * 0.8);
        }
    }
}

fn cross(a: Vec3, b: Vec3) -> Vec3 {
    vec3(a.y * b.z - a.z * b.y, a.z * b.x - a.x * b.z, a.x * b.y - a.y * b.x)
}

/// Camera-facing ribbon between two world points.
fn push_ribbon_segment(
    batches: &mut RenderBatches,
    frame: &FrameContext,
    a: Endpoint,
    b: Endpoint,
    width: f32,
    color: LinearColor,
) {
    let (dx, dy, dz) = (b.x - a.x, b.y - a.y, b.z - a.z);
    let length = (dx * dx + dy * dy + dz * dz).sqrt();
    if length < 0.01 {
        return;
    }
    let view = frame.view_direction;
    let direction = vec3(dx / length, dy / length, dz / length);
    let mut side = cross(direction, view);
    let mut side_length = (side.x * side.x + side.y * side.y + side.z * side.z).sqrt();
    if side_length * side_length < 1e-6 {
        side = vec3(0.0, 0.0, 1.0);
        side_length = 1.0;
    }
    side = vec3(side.x / side_length, side.y / side_length, side.z / side_length);
    let mut normal = cross(side, direction);
    // Keep the (single-sided) front face toward the camera; flipping the width axis flips the face.
    if normal.x * view.x + normal.y * view.y + normal.z * view.z > 0.0 {
        side = vec3(-side.x, -side.y, -side.z);
        normal = vec3(-normal.x, -normal.y, -normal.z);
    }
    let ribbon = &mut batches[Batch::Ribbon];
    let slot = ribbon.push_basis(
        a.x,
        a.y,
        a.z,
        [direction.x * length, direction.y * length, direction.z * length],
        [normal.x, normal.y, normal.z],
        [side.x * width, side.y * width, side.z * width],
        color.r,
        color.g,
        color.b,
    );
    ribbon.set_extra(slot, 0, 0.0);
}

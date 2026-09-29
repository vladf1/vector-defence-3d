//! Shared hit, laser, missile, and escape particle recipes. Each respects a particle budget.
use std::f64::consts::PI;

use crate::entities::Particle;
use crate::entities::effects::particle::ParticleMotion;
use crate::types::Color;
use crate::utils::{random, random_range};

#[derive(Clone, Copy, Debug)]
pub struct EscapeBurstConfig {
    pub large_fragments: usize,
    pub small_fragments: usize,
    pub smoke_count: usize,
    pub speed_scale: f64,
}

pub const ESCAPE_BURST_CONFIG: EscapeBurstConfig =
    EscapeBurstConfig { large_fragments: 58, small_fragments: 30, smoke_count: 18, speed_scale: 1.0 };

const ESCAPE_BURST_COLORS: [Color; 5] = [0xb0ffe1, 0x6df0c2, 0xffe36f, 0xf4fff8, 0x7fd7ff];
pub const LASER_SPARK_WHITE: Color = 0xf6f0ff;
pub const LASER_SPARK_YELLOW: Color = 0xffe36f;

/// `spark_angle: None` scatters sparks in every direction.
pub fn create_hit_impact_particles(
    x: f64,
    y: f64,
    color: Color,
    spark_angle: Option<f64>,
    limit: usize,
) -> Vec<Particle> {
    let mut particles = Vec::new();
    if limit > 0 {
        particles.push(Particle::hit_ring(x, y, color, random_range(5.5, 9.0)));
    }
    let mut index = 0;
    while index < 5 && particles.len() < limit {
        let angle = match spark_angle {
            None => random_range(-PI, PI),
            Some(spark_angle) => spark_angle + random_range(-0.95, 0.95),
        };
        let size = random_range(0.8, 1.7);
        let alpha_fade_per_second = random_range(4.2, 6.0);
        let speed_per_second = random_range(80.0, 210.0);
        let offset = random_range(1.0, 3.0);
        particles.push(Particle::spark(
            x,
            y,
            size,
            color,
            alpha_fade_per_second,
            ParticleMotion { speed_per_second, offset, angle: Some(angle) },
        ));
        index += 1;
    }
    particles
}

pub fn create_laser_impact_particles(x: f64, y: f64, beam_angle: f64, color: Color, limit: usize) -> Vec<Particle> {
    let mut particles = Vec::new();
    let mut index = 0;
    while index < 4 && particles.len() < limit {
        let side = if random() < 0.5 { -1.0 } else { 1.0 };
        let angle = beam_angle + side * (PI / 2.0) + random_range(-0.55, 0.55);
        let spark_color = match index {
            0 => LASER_SPARK_WHITE,
            1 => LASER_SPARK_YELLOW,
            _ => color,
        };
        let size = random_range(0.7, 1.45);
        let alpha_fade_per_second = random_range(3.4, 5.2);
        let speed_per_second = random_range(45.0, 145.0);
        let offset = random_range(1.5, 4.0);
        particles.push(Particle::spark(
            x,
            y,
            size,
            spark_color,
            alpha_fade_per_second,
            ParticleMotion { speed_per_second, offset, angle: Some(angle) },
        ));
        index += 1;
    }
    particles
}

pub fn create_missile_explosion_particles(x: f64, y: f64, blast_angle: f64, level: u32, limit: usize) -> Vec<Particle> {
    let mut particles = Vec::new();
    let mut index = 0;
    while index < 10 && particles.len() < limit {
        particles.push(Particle::smoke(x, y, blast_angle, level));
        index += 1;
    }
    if particles.len() < limit {
        particles.push(Particle::shockwave(x, y, 0.8 + 0.06 * level as f64));
    }
    index = 0;
    while index < 14 && particles.len() < limit {
        particles.push(Particle::ember_streak(x, y, blast_angle, level));
        index += 1;
    }
    particles
}

pub fn create_escape_burst_particles(x: f64, y: f64, config: &EscapeBurstConfig, limit: usize) -> Vec<Particle> {
    let mut particles = Vec::new();
    if limit > 0 {
        particles.push(Particle::shockwave(x, y, 1.45));
    }
    if limit > 1 {
        particles.push(Particle::hit_ring(x, y, 0xb0ffe1, 24.0));
    }
    if limit > 2 {
        particles.push(Particle::hit_ring(x, y, 0xffe36f, 12.0));
    }

    let mut index = 0;
    while index < config.large_fragments && particles.len() < limit {
        let color = get_random_escape_burst_color();
        let angle = random_range(-PI, PI);
        let speed = random_range(185.0, 500.0) * config.speed_scale;
        let length = random_range(5.5, 13.0);
        let width = random_range(2.4, 5.2);
        let separation = random_range(3.0, 9.0);
        particles.push(Particle::escape_fragment(x, y, color, angle, speed, length, width, separation));
        index += 1;
    }

    index = 0;
    while index < config.small_fragments && particles.len() < limit {
        let color = get_random_escape_burst_color();
        let angle = random_range(-PI, PI);
        let speed = random_range(260.0, 620.0) * config.speed_scale;
        let length = random_range(2.8, 6.8);
        let width = random_range(1.1, 2.6);
        let separation = random_range(2.0, 11.0);
        particles.push(Particle::escape_fragment(x, y, color, angle, speed, length, width, separation));
        index += 1;
    }

    index = 0;
    while index < config.smoke_count && particles.len() < limit {
        particles.push(Particle::smoke(x, y, random_range(-PI, PI), 2));
        index += 1;
    }
    particles
}

fn get_random_escape_burst_color() -> Color {
    let index = random_range(0.0, ESCAPE_BURST_COLORS.len() as f64).floor() as usize;
    ESCAPE_BURST_COLORS.get(index).copied().unwrap_or(0xb0ffe1)
}

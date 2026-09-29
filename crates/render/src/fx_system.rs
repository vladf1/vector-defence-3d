//! Renderer-only spectacle: pooled 3D sparks, fireballs, smoke, ground rings, a fixed
//! flash-light pool written into the frame uniforms, scorch decals, and camera trauma.
use std::f32::consts::PI;

use vd_core::rng::random_range;
use vd_core::types::Color;

use crate::palette::{LinearColor, linear_color};
use crate::render_batches::{Batch, RenderBatches};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
enum FxKind {
    Spark,
    Ember,
    Fireball,
    Smoke,
    Ring,
    Flash,
    Pillar,
    Halo,
}

const GROUND_BOUNCE_RESTITUTION: f32 = 0.38;
const GROUND_FRICTION: f32 = 0.55;

/// `randomRange` from the shared simulation RNG, narrowed for renderer math.
fn range(min: f32, max: f32) -> f32 {
    random_range(min as f64, max as f64) as f32
}

/// The fixed effect colors, converted to linear once.
#[derive(Clone, Copy, Debug)]
struct FxPalette {
    fireball_hot: LinearColor,
    fireball_warm: LinearColor,
    fireball_cool: LinearColor,
    smoke: LinearColor,
    explosion_light: LinearColor,
    escape: LinearColor,
    escape_accent: LinearColor,
    zap: LinearColor,
}

impl FxPalette {
    fn new() -> Self {
        FxPalette {
            fireball_hot: linear_color(0xfff4d0),
            fireball_warm: linear_color(0xffb04a),
            fireball_cool: linear_color(0xd8452a),
            smoke: linear_color(0x2a2f2c),
            explosion_light: linear_color(0xffae5c),
            escape: linear_color(0xb0ffe1),
            escape_accent: linear_color(0xffe36f),
            zap: linear_color(0x9fe8ff),
        }
    }
}

/// Struct-of-arrays particle pool; dead particles are swapped out so updates stay dense.
struct FxParticlePool {
    capacity: usize,
    count: usize,
    kind: Vec<FxKind>,
    x: Vec<f32>,
    y: Vec<f32>,
    z: Vec<f32>,
    vx: Vec<f32>,
    vy: Vec<f32>,
    vz: Vec<f32>,
    age: Vec<f32>,
    life: Vec<f32>,
    start_size: Vec<f32>,
    end_size: Vec<f32>,
    gravity: Vec<f32>,
    drag: Vec<f32>,
    red: Vec<f32>,
    green: Vec<f32>,
    blue: Vec<f32>,
    spin: Vec<f32>,
}

impl FxParticlePool {
    fn new(capacity: usize) -> Self {
        let floats = || vec![0.0; capacity];
        FxParticlePool {
            capacity,
            count: 0,
            kind: vec![FxKind::Spark; capacity],
            x: floats(),
            y: floats(),
            z: floats(),
            vx: floats(),
            vy: floats(),
            vz: floats(),
            age: floats(),
            life: floats(),
            start_size: floats(),
            end_size: floats(),
            gravity: floats(),
            drag: floats(),
            red: floats(),
            green: floats(),
            blue: floats(),
            spin: floats(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn spawn(
        &mut self,
        kind: FxKind,
        position: [f32; 3],
        velocity: [f32; 3],
        life: f32,
        start_size: f32,
        end_size: f32,
        color: LinearColor,
        gravity: f32,
        drag: f32,
    ) {
        if self.count >= self.capacity {
            return;
        }
        let i = self.count;
        self.kind[i] = kind;
        [self.x[i], self.y[i], self.z[i]] = position;
        [self.vx[i], self.vy[i], self.vz[i]] = velocity;
        self.age[i] = 0.0;
        self.life[i] = life;
        self.start_size[i] = start_size;
        self.end_size[i] = end_size;
        self.gravity[i] = gravity;
        self.drag[i] = drag;
        self.red[i] = color.r;
        self.green[i] = color.g;
        self.blue[i] = color.b;
        self.spin[i] = range(-2.0, 2.0);
        self.count = i + 1;
    }

    fn clear(&mut self) {
        self.count = 0;
    }

    fn update(&mut self, delta_seconds: f32) {
        if delta_seconds <= 0.0 {
            return;
        }
        let mut index = 0;
        while index < self.count {
            self.age[index] += delta_seconds;
            if self.age[index] >= self.life[index] {
                self.swap_remove(index);
                continue;
            }
            let damping = (-self.drag[index] * delta_seconds).exp();
            self.vx[index] *= damping;
            self.vz[index] *= damping;
            self.vy[index] = self.vy[index] * damping - self.gravity[index] * delta_seconds;
            self.x[index] += self.vx[index] * delta_seconds;
            self.y[index] += self.vy[index] * delta_seconds;
            self.z[index] += self.vz[index] * delta_seconds;
            if self.gravity[index] > 0.0 && self.y[index] < 0.4 {
                self.y[index] = 0.4;
                if self.vy[index] < 0.0 {
                    self.vy[index] = -self.vy[index] * GROUND_BOUNCE_RESTITUTION;
                    self.vx[index] *= GROUND_FRICTION;
                    self.vz[index] *= GROUND_FRICTION;
                }
            }
            index += 1;
        }
    }

    fn write(&self, batches: &mut RenderBatches, palette: &FxPalette) {
        for i in 0..self.count {
            let t = self.age[i] / self.life[i];
            let kind = self.kind[i];
            let (r, g, b) = (self.red[i], self.green[i], self.blue[i]);
            let (x, y, z) = (self.x[i], self.y[i], self.z[i]);
            let (start, end) = (self.start_size[i], self.end_size[i]);
            match kind {
                FxKind::Smoke => {
                    let fade = (t * 5.0).min(1.0) * (1.0 - t);
                    let size = start + (end - start) * t.sqrt();
                    let shape = (i % 4) as f32;
                    batches.smoke.push(x, y, z, self.spin[i] * self.age[i], size, size, shape, r, g, b, fade * 0.62);
                }
                FxKind::Fireball => {
                    let grow = 1.0 - (1.0 - t) * (1.0 - t) * (1.0 - t);
                    let size = start + (end - start) * grow;
                    let heat = 1.0 - t;
                    let warm_mix = (t * 2.2).min(1.0);
                    let cool_mix = ((t - 0.45) / 0.55).max(0.0);
                    let (hot, warm, cool) = (palette.fireball_hot, palette.fireball_warm, palette.fireball_cool);
                    let fr = mix3(hot.r, warm.r, cool.r, warm_mix, cool_mix);
                    let fg = mix3(hot.g, warm.g, cool.g, warm_mix, cool_mix);
                    let fb = mix3(hot.b, warm.b, cool.b, warm_mix, cool_mix);
                    let intensity = 4.0 * heat * heat.sqrt();
                    let (fr, fg, fb) = (fr * intensity, fg * intensity, fb * intensity);
                    batches.glow.push(x, y, z, self.spin[i], size, size, 0.0, fr, fg, fb, heat);
                }
                FxKind::Ring | FxKind::Halo => {
                    let grow = 1.0 - (1.0 - t) * (1.0 - t);
                    let size = start + (end - start) * grow;
                    let fade = (1.0 - t) * (1.0 - t);
                    if kind == FxKind::Ring {
                        batches.push_ground_glow(x, y, z, size / 2.0, 1.0, r * 1.5, g * 1.5, b * 1.5, fade);
                    } else {
                        batches.glow.push(x, y, z, 0.0, size, size, 1.0, r * 1.3, g * 1.3, b * 1.3, fade);
                    }
                }
                FxKind::Ember => {
                    let speed = self.vx[i].hypot(self.vz[i]);
                    let length = start + (speed * 0.05).min(18.0);
                    let rotation = (-self.vz[i]).atan2(self.vx[i]);
                    let fade = 1.0 - t;
                    batches.glow.push(x, y, z, rotation, length, start * 0.45, 0.0, r * 2.0, g * 2.0, b * 2.0, fade);
                }
                FxKind::Pillar => {
                    let fade = (1.0 - t) * (1.0 - t);
                    let width = start * (1.0 + t * 0.8);
                    batches.glow.push(x, y, z, 0.0, width, end, 0.0, r * 1.8, g * 1.8, b * 1.8, fade);
                }
                FxKind::Spark | FxKind::Flash => {
                    let size = start + (end - start) * t;
                    let flash = kind == FxKind::Flash;
                    let fade = if flash { (1.0 - t) * (1.0 - t) * (1.0 - t) } else { (1.0 - t).powf(1.4) };
                    let intensity = if flash { 2.2 } else { 1.8 };
                    let (lr, lg, lb) = (r * intensity, g * intensity, b * intensity);
                    batches.glow.push(x, y, z, 0.0, size, size, 0.0, lr, lg, lb, fade);
                }
            }
        }
    }

    fn swap_remove(&mut self, index: usize) {
        let last = self.count - 1;
        if index != last {
            self.kind[index] = self.kind[last];
            for column in [
                &mut self.x,
                &mut self.y,
                &mut self.z,
                &mut self.vx,
                &mut self.vy,
                &mut self.vz,
                &mut self.age,
                &mut self.life,
                &mut self.start_size,
                &mut self.end_size,
                &mut self.gravity,
                &mut self.drag,
                &mut self.red,
                &mut self.green,
                &mut self.blue,
                &mut self.spin,
            ] {
                column[index] = column[last];
            }
        }
        self.count = last;
    }
}

fn mix3(hot: f32, warm: f32, cool: f32, warm_mix: f32, cool_mix: f32) -> f32 {
    let first = hot + (warm - hot) * warm_mix;
    first + (cool - first) * cool_mix
}

const FLASH_LIGHT_DISTANCE: f32 = 190.0;
const FLASH_LIGHT_DECAY: f32 = 1.6;

#[derive(Clone, Copy, Debug)]
struct PooledLight {
    x: f32,
    y: f32,
    z: f32,
    color: LinearColor,
    intensity: f32,
    peak: f32,
    age: f32,
    duration: f32,
}

/// A fixed set of point lights reused for explosion flashes. The shaders always loop over
/// the same slot count; idle lights just sit at zero intensity.
struct FlashLightPool {
    lights: Vec<PooledLight>,
}

impl FlashLightPool {
    fn new(count: usize) -> Self {
        let idle = PooledLight {
            x: 0.0,
            y: -500.0,
            z: 0.0,
            color: LinearColor::new(1.0, 1.0, 1.0),
            intensity: 0.0,
            peak: 0.0,
            age: 0.0,
            duration: 1.0,
        };
        FlashLightPool { lights: vec![idle; count] }
    }

    fn flash(&mut self, x: f32, y: f32, z: f32, color: LinearColor, peak: f32, duration: f32) {
        let mut chosen: Option<&mut PooledLight> = None;
        let mut weakest = f32::INFINITY;
        for pooled in &mut self.lights {
            if pooled.intensity < weakest {
                weakest = pooled.intensity;
                chosen = Some(pooled);
            }
        }
        let Some(chosen) = chosen else {
            return;
        };
        if weakest > peak {
            return;
        }
        *chosen = PooledLight { x, y, z, color, intensity: peak, peak, age: 0.0, duration };
    }

    fn update(&mut self, delta_seconds: f32) {
        for pooled in &mut self.lights {
            if pooled.intensity <= 0.0 {
                continue;
            }
            pooled.age += delta_seconds;
            let t = pooled.age / pooled.duration;
            pooled.intensity = if t >= 1.0 { 0.0 } else { pooled.peak * (1.0 - t) * (1.0 - t) };
        }
    }

    /// Point-light slot layout the shaders read: position + cutoff, color x intensity + decay.
    fn write(&self, sink: LightSink<'_>) {
        for slot in 0..sink.slots {
            let p = sink.position_offset + slot * 4;
            let c = sink.color_offset + slot * 4;
            let (position, color) = match self.lights.get(slot) {
                Some(light) => {
                    let i = light.intensity;
                    ([light.x, light.y, light.z], [light.color.r * i, light.color.g * i, light.color.b * i])
                }
                None => ([0.0, -500.0, 0.0], [0.0; 3]),
            };
            sink.data[p..p + 4].copy_from_slice(&[position[0], position[1], position[2], FLASH_LIGHT_DISTANCE]);
            sink.data[c..c + 4].copy_from_slice(&[color[0], color[1], color[2], FLASH_LIGHT_DECAY]);
        }
    }

    fn clear(&mut self) {
        for pooled in &mut self.lights {
            pooled.intensity = 0.0;
        }
    }
}

/// Where the flash lights go inside the frame uniform data.
pub struct LightSink<'a> {
    pub data: &'a mut [f32],
    pub position_offset: usize,
    pub color_offset: usize,
    pub slots: usize,
}

const SCORCH_CAPACITY: usize = 48;
const SCORCH_LIFETIME_SECONDS: f32 = 9.0;

struct ScorchMarks {
    x: [f32; SCORCH_CAPACITY],
    z: [f32; SCORCH_CAPACITY],
    size: [f32; SCORCH_CAPACITY],
    rotation: [f32; SCORCH_CAPACITY],
    age: [f32; SCORCH_CAPACITY],
    next: usize,
}

impl ScorchMarks {
    fn new() -> Self {
        ScorchMarks {
            x: [0.0; SCORCH_CAPACITY],
            z: [0.0; SCORCH_CAPACITY],
            size: [0.0; SCORCH_CAPACITY],
            rotation: [0.0; SCORCH_CAPACITY],
            age: [SCORCH_LIFETIME_SECONDS; SCORCH_CAPACITY],
            next: 0,
        }
    }

    fn add(&mut self, x: f32, z: f32, size: f32) {
        let index = self.next;
        self.next = (self.next + 1) % SCORCH_CAPACITY;
        self.x[index] = x;
        self.z[index] = z;
        self.size[index] = size;
        self.rotation[index] = range(0.0, PI * 2.0);
        self.age[index] = 0.0;
    }

    fn update(&mut self, delta_seconds: f32) {
        for age in &mut self.age {
            *age += delta_seconds;
        }
    }

    fn write(&self, batches: &mut RenderBatches) {
        let decal = &mut batches[Batch::Decal];
        for index in 0..SCORCH_CAPACITY {
            let t = self.age[index] / SCORCH_LIFETIME_SECONDS;
            if t >= 1.0 {
                continue;
            }
            let fade_in = (self.age[index] * 6.0).min(1.0);
            let size = self.size[index];
            let slot = decal.push_yaw(
                self.x[index],
                0.55,
                self.z[index],
                self.rotation[index],
                size,
                1.0,
                size,
                0.004,
                0.006,
                0.005,
            );
            decal.set_extra(slot, 0, 0.72 * fade_in * (1.0 - t * t));
            decal.set_extra(slot, 1, 0.0);
        }
    }

    fn clear(&mut self) {
        self.age = [SCORCH_LIFETIME_SECONDS; SCORCH_CAPACITY];
    }
}

#[derive(Clone, Copy, Debug)]
pub struct FxBudget {
    pub particles: usize,
    pub lights: usize,
}

/// Renderer-only spectacle layered on top of the simulation's own effect particles.
///
/// Camera trauma is collected here and handed to the camera rig once per frame
/// (`take_trauma`), so the effect methods need no rig reference.
pub struct FxSystem {
    particles: FxParticlePool,
    lights: FlashLightPool,
    scorch: ScorchMarks,
    palette: FxPalette,
    trauma: f32,
}

impl FxSystem {
    pub fn new(budget: FxBudget) -> Self {
        FxSystem {
            particles: FxParticlePool::new(budget.particles),
            lights: FlashLightPool::new(budget.lights),
            scorch: ScorchMarks::new(),
            palette: FxPalette::new(),
            trauma: 0.0,
        }
    }

    pub fn active_particles(&self) -> usize {
        self.particles.count
    }

    pub fn clear(&mut self) {
        self.particles.clear();
        self.lights.clear();
        self.scorch.clear();
        self.trauma = 0.0;
    }

    pub fn update(&mut self, delta_seconds: f32) {
        self.particles.update(delta_seconds);
        self.lights.update(delta_seconds);
        self.scorch.update(delta_seconds);
    }

    pub fn write(&self, batches: &mut RenderBatches) {
        self.scorch.write(batches);
        self.particles.write(batches, &self.palette);
    }

    pub fn write_lights(&self, sink: LightSink<'_>) {
        self.lights.write(sink);
    }

    /// Screen-shake trauma added since the last call (the renderer feeds it to the rig).
    pub fn take_trauma(&mut self) -> f32 {
        std::mem::take(&mut self.trauma)
    }

    fn add_trauma(&mut self, amount: f32) {
        self.trauma += amount;
    }

    /// Missile blast: fireball, smoke column, sparks, scorch, light flash, and shake.
    pub fn explosion(&mut self, x: f32, z: f32, scale: f32) {
        let palette = self.palette;
        let p = &mut self.particles;
        self.lights.flash(x, 22.0, z, palette.explosion_light, 5200.0 * scale, 0.45);
        let still = [0.0; 3];
        p.spawn(FxKind::Flash, [x, 16.0, z], still, 0.18, 30.0 * scale, 64.0 * scale, palette.fireball_hot, 0.0, 0.0);
        for _ in 0..7 {
            let angle = range(0.0, PI * 2.0);
            let spread = range(0.0, 7.0) * scale;
            let position = [x + angle.cos() * spread, range(12.0, 22.0), z + angle.sin() * spread];
            let velocity = [angle.cos() * range(20.0, 50.0), range(30.0, 70.0), angle.sin() * range(20.0, 50.0)];
            let life = range(0.42, 0.7);
            let start = range(8.0, 12.0) * scale;
            let end = range(26.0, 40.0) * scale;
            p.spawn(FxKind::Fireball, position, velocity, life, start, end, palette.fireball_warm, 0.0, 3.0);
        }
        p.spawn(FxKind::Pillar, [x, 18.0, z], still, 0.28, 10.0 * scale, 46.0 * scale, palette.fireball_hot, 0.0, 0.0);
        for _ in 0..7 {
            let position = [x + range(-9.0, 9.0) * scale, range(6.0, 14.0), z + range(-9.0, 9.0) * scale];
            let velocity = [range(-14.0, 14.0), range(16.0, 34.0), range(-14.0, 14.0)];
            let life = range(1.1, 1.9);
            let start = range(10.0, 16.0) * scale;
            let end = range(30.0, 46.0) * scale;
            p.spawn(FxKind::Smoke, position, velocity, life, start, end, palette.smoke, 0.0, 1.4);
        }
        for index in 0..12 {
            let angle = range(0.0, PI * 2.0);
            let speed = range(90.0, 230.0) * scale;
            let position = [x, range(4.0, 10.0), z];
            let velocity = [angle.cos() * speed, range(70.0, 190.0), angle.sin() * speed];
            let life = range(0.5, 0.95);
            let start = range(3.0, 5.0);
            let color = if index % 3 == 0 { palette.fireball_hot } else { palette.fireball_warm };
            p.spawn(FxKind::Ember, position, velocity, life, start, 0.0, color, 520.0, 1.2);
        }
        p.spawn(FxKind::Ring, [x, 1.2, z], still, 0.42, 12.0 * scale, 110.0 * scale, palette.fireball_warm, 0.0, 0.0);
        self.scorch.add(x, z, range(30.0, 40.0) * scale);
        self.add_trauma(0.2 * scale);
    }

    /// Death pop in the monster's color; heavy monsters add smoke, a scorch, and shake.
    pub fn monster_death(&mut self, x: f32, y: f32, z: f32, color: Color, radius: f32, heavy: bool) {
        let color = linear_color(color);
        let smoke = self.palette.smoke;
        let p = &mut self.particles;
        let (peak, duration) = if heavy { (2600.0, 0.4) } else { (1300.0, 0.26) };
        self.lights.flash(x, y + 10.0, z, color, peak, duration);
        let still = [0.0; 3];
        p.spawn(FxKind::Flash, [x, y, z], still, 0.14, radius * 2.4, radius * 6.5, color, 0.0, 0.0);
        p.spawn(FxKind::Ring, [x, 1.0, z], still, 0.36, radius * 1.6, radius * 7.0, color, 0.0, 0.0);
        let spark_count = if heavy { 16 } else { 9 };
        for _ in 0..spark_count {
            let angle = range(0.0, PI * 2.0);
            let speed = range(60.0, 170.0);
            let velocity = [angle.cos() * speed, range(60.0, 200.0), angle.sin() * speed];
            let life = range(0.4, 0.85);
            let start = range(2.6, 4.2);
            p.spawn(FxKind::Spark, [x, y, z], velocity, life, start, 0.6, color, 480.0, 0.9);
        }
        if heavy {
            for _ in 0..4 {
                let position = [x + range(-6.0, 6.0), y, z + range(-6.0, 6.0)];
                let velocity = [range(-10.0, 10.0), range(14.0, 26.0), range(-10.0, 10.0)];
                let life = range(1.0, 1.6);
                p.spawn(FxKind::Smoke, position, velocity, life, radius * 1.2, radius * 3.6, smoke, 0.0, 1.2);
            }
            self.scorch.add(x, z, radius * 3.0);
            self.add_trauma(0.12);
        }
    }

    /// Base breach: towering light pillar, double shockwave, and heavy shake.
    pub fn escape_blast(&mut self, x: f32, z: f32) {
        let (escape, accent) = (self.palette.escape, self.palette.escape_accent);
        let p = &mut self.particles;
        self.lights.flash(x, 30.0, z, escape, 9000.0, 0.8);
        let still = [0.0; 3];
        p.spawn(FxKind::Pillar, [x, 60.0, z], still, 0.9, 20.0, 150.0, escape, 0.0, 0.0);
        p.spawn(FxKind::Pillar, [x, 50.0, z], still, 0.6, 8.0, 130.0, accent, 0.0, 0.0);
        p.spawn(FxKind::Flash, [x, 12.0, z], still, 0.3, 40.0, 120.0, escape, 0.0, 0.0);
        p.spawn(FxKind::Ring, [x, 1.5, z], still, 0.7, 20.0, 190.0, escape, 0.0, 0.0);
        p.spawn(FxKind::Ring, [x, 1.5, z], still, 0.5, 10.0, 110.0, accent, 0.0, 0.0);
        self.add_trauma(0.55);
    }

    pub fn spawn_pulse(&mut self, x: f32, z: f32, color: Color, radius: f32) {
        let color = linear_color(color);
        self.particles.spawn(FxKind::Ring, [x, 1.0, z], [0.0; 3], 0.45, radius * 1.5, radius * 5.0, color, 0.0, 0.0);
    }

    pub fn shield_hit(&mut self, x: f32, y: f32, z: f32, radius: f32, color: Color) {
        let color = linear_color(color);
        self.particles.spawn(FxKind::Halo, [x, y, z], [0.0; 3], 0.22, radius * 2.4, radius * 3.4, color, 0.0, 0.0);
    }

    pub fn ember(&mut self, x: f32, y: f32, z: f32, vx: f32, vz: f32, color: Color) {
        let velocity = [vx, range(10.0, 40.0), vz];
        let life = range(0.25, 0.45);
        let start = range(2.4, 3.6);
        let color = linear_color(color);
        self.particles.spawn(FxKind::Ember, [x, y, z], velocity, life, start, 0.0, color, 60.0, 2.0);
    }

    pub fn zap(&mut self, x: f32, y: f32, z: f32) {
        self.lights.flash(x, y + 6.0, z, self.palette.zap, 1100.0, 0.14);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render_batches::BatchCapacities;
    use crate::shaders::{MAX_POINT_LIGHTS, frame_layout};

    #[test]
    fn explosion_spawns_particles_lights_and_trauma_then_fades() {
        let mut fx = FxSystem::new(FxBudget { particles: 400, lights: 2 });
        fx.explosion(100.0, 120.0, 1.0);
        fx.monster_death(50.0, 8.0, 60.0, 0xff00ff, 10.0, true);
        assert_eq!(fx.active_particles(), 1 + 7 + 1 + 7 + 12 + 1 + 2 + 16 + 4);
        assert!((fx.take_trauma() - 0.32).abs() < 1e-6);
        assert_eq!(fx.take_trauma(), 0.0);

        let mut batches = RenderBatches::new(BatchCapacities { glow_sprites: 512, smoke_sprites: 64, ribbons: 8 });
        fx.update(0.016);
        fx.write(&mut batches);
        assert_eq!(batches[Batch::Decal].size(), 2);
        assert!(batches.glow.size() > 0 && batches.smoke.size() == 11);

        let mut frame = [0.0; frame_layout::FLOATS];
        fx.write_lights(LightSink {
            data: &mut frame,
            position_offset: frame_layout::POINT_POSITION,
            color_offset: frame_layout::POINT_COLOR,
            slots: MAX_POINT_LIGHTS,
        });
        assert_eq!(frame[frame_layout::POINT_POSITION + 3], FLASH_LIGHT_DISTANCE);
        assert!(frame[frame_layout::POINT_COLOR] > 0.0);
        // Slots past the pool's two lights stay parked below the ground.
        assert_eq!(frame[frame_layout::POINT_POSITION + 8 + 1], -500.0);

        for _ in 0..200 {
            fx.update(0.05);
        }
        assert_eq!(fx.active_particles(), 0);
        batches.begin();
        fx.write(&mut batches);
        assert_eq!(batches.drawn_instances(), 0);
    }
}

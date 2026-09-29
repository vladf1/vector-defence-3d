//! Gives flat simulation trajectories a third dimension: missiles climb and dive, drones
//! cruise, bank, and climb away, and drone rounds drop from altitude.
use vd_core::entities::drone_visuals::drone_accent_color;
use vd_core::entities::{DroneRef, Missile, Projectile, ProjectileKind};
use vd_core::level_runtime::LevelRuntime;
use vd_core::types::Color;
use vd_core::utils::normalize_angle;

use crate::frame_math::{FrameContext, Quat, smooth_towards};
use crate::id_map::IdMap;
use crate::models::{GUN_BARREL_Y, MISSILE_RACK_Y};
use crate::monster_view::MonsterView;
use crate::palette::linear_color;
use crate::render_batches::{Batch, RenderBatches};

const GUN_TRACER: Color = 0xd9fff3;
const GUN_TRACER_GLOW: Color = 0x9fffe4;
const MISSILE_BODY: Color = 0xff9d5c;
const MISSILE_FLAME: Color = 0xffb04a;
const MISSILE_FLAME_CORE: Color = 0xfff0a8;
const ROTOR_COLOR: Color = 0xe8fff6;
const DRONE_CRUISE_ALTITUDE: f32 = 36.0;
const DRONE_EXIT_CLIMB_PER_SECOND: f32 = 60.0;
const MISSILE_CRUISE_ALTITUDE: f32 = 46.0;
const MISSILE_CLIMB_DISTANCE: f32 = 70.0;
const DRONE_SHOT_DROP_DISTANCE: f32 = 52.0;
const ROTOR_OFFSETS: [(f32, f32); 4] = [(-6.9, -6.9), (6.9, -6.9), (-6.9, 6.9), (6.9, 6.9)];

struct MissileFlight {
    frame: u32,
    x: f64,
    y: f64,
    launch_x: f64,
    launch_y: f64,
    altitude: f32,
    climb_angle: f32,
}

struct DroneFlight {
    frame: u32,
    x: f64,
    y: f64,
    altitude: f32,
    bank: f32,
    pitch: f32,
    previous_x: f64,
    previous_y: f64,
    previous_angle: f64,
}

struct ShotFlight {
    frame: u32,
    start_x: f64,
    start_y: f64,
    start_altitude: f32,
}

fn clamp(value: f32, min: f32, max: f32) -> f32 {
    max.min(min.max(value))
}

#[derive(Default)]
pub struct ProjectileView {
    missiles: IdMap<MissileFlight>,
    drones: IdMap<DroneFlight>,
    shots: IdMap<ShotFlight>,
    rotation: Quat,
}

/// The altitude of the flight closest to (x, y) within `max_distance`.
fn altitude_near<V>(
    flights: &IdMap<V>,
    x: f64,
    y: f64,
    max_distance: f64,
    read: impl Fn(&V) -> (f64, f64, f32),
) -> Option<f32> {
    let mut best = None;
    let mut best_distance = max_distance * max_distance;
    for (_, flight) in flights.iter() {
        let (fx, fy, altitude) = read(flight);
        let distance = (fx - x) * (fx - x) + (fy - y) * (fy - y);
        if distance < best_distance {
            best_distance = distance;
            best = Some(altitude);
        }
    }
    best
}

impl ProjectileView {
    pub fn reset(&mut self) {
        self.missiles.clear();
        self.drones.clear();
        self.shots.clear();
    }

    /// Altitude of the missile closest to a field point, used to lift its trail particles.
    pub fn missile_altitude_near(&self, x: f64, y: f64) -> Option<f32> {
        altitude_near(&self.missiles, x, y, 26.0, |flight| (flight.x, flight.y, flight.altitude))
    }

    pub fn drone_altitude_near(&self, x: f64, y: f64) -> Option<f32> {
        altitude_near(&self.drones, x, y, 20.0, |flight| (flight.x, flight.y, flight.altitude))
    }

    pub fn write(
        &mut self,
        runtime: &LevelRuntime,
        monsters: &MonsterView,
        batches: &mut RenderBatches,
        frame: &FrameContext,
    ) {
        self.write_drones(&runtime.drones, batches, frame);
        self.write_shots(&runtime.projectiles, batches, frame);
        self.write_missiles(&runtime.missiles, monsters, batches, frame);
    }

    fn write_shots(&mut self, projectiles: &[Projectile], batches: &mut RenderBatches, frame: &FrameContext) {
        for projectile in projectiles {
            if projectile.removed {
                continue;
            }
            let from_drone = matches!(projectile.kind, ProjectileKind::Drone { .. });
            let drone_altitude = if from_drone && self.shots.get(projectile.id).is_none() {
                self.drone_altitude_near(projectile.previous_x, projectile.previous_y)
            } else {
                None
            };
            let flight = self.shots.get_or_insert_with(projectile.id, |_| ShotFlight {
                frame: 0,
                start_x: projectile.previous_x,
                start_y: projectile.previous_y,
                start_altitude: if from_drone { drone_altitude.unwrap_or(DRONE_CRUISE_ALTITUDE) } else { GUN_BARREL_Y },
            });
            flight.frame = frame.frame;

            let travelled = (projectile.x - flight.start_x).hypot(projectile.y - flight.start_y) as f32;
            let target_altitude = 7.0;
            let drop = if from_drone {
                clamp(travelled / DRONE_SHOT_DROP_DISTANCE, 0.0, 1.0)
            } else {
                clamp(travelled / 90.0, 0.0, 1.0)
            };
            let altitude = flight.start_altitude + (target_altitude - flight.start_altitude) * drop;
            let angle = projectile.angle as f32;
            let yaw = -angle;
            let (sin, cos) = angle.sin_cos();
            let (x, z) = (projectile.x as f32, projectile.y as f32);

            match projectile.kind {
                ProjectileKind::Gun { visual_level } => {
                    let level = visual_level as f32;
                    let length = (9.8 + level * 1.45) * 1.25;
                    let width = 2.6 + level * 0.35;
                    let tail = length.min(travelled + 2.0);
                    let glow = linear_color(GUN_TRACER_GLOW).scaled(1.6);
                    let ribbon = &mut batches[Batch::Ribbon];
                    let slot = ribbon.push_yaw(
                        x - cos * tail,
                        altitude,
                        z - sin * tail,
                        yaw,
                        tail,
                        1.0,
                        width,
                        glow.r,
                        glow.g,
                        glow.b,
                    );
                    ribbon.set_extra(slot, 0, 1.0);
                    let tracer = linear_color(GUN_TRACER).scaled(2.0);
                    let size = width * 2.2;
                    batches.glow.push(x, altitude, z, 0.0, size, size, 0.0, tracer.r, tracer.g, tracer.b, 0.9);
                }
                ProjectileKind::Drone { accent_color } => {
                    let color = linear_color(accent_color);
                    let tail = 10f32.min(travelled + 1.0);
                    let trail = color.scaled(1.8);
                    let ribbon = &mut batches[Batch::Ribbon];
                    let slot = ribbon.push_yaw(
                        x - cos * tail,
                        altitude,
                        z - sin * tail,
                        yaw,
                        tail,
                        1.0,
                        2.2,
                        trail.r,
                        trail.g,
                        trail.b,
                    );
                    ribbon.set_extra(slot, 0, 1.0);
                    let glow = color.scaled(2.2);
                    batches.glow.push(x, altitude, z, 0.0, 5.0, 5.0, 0.0, glow.r, glow.g, glow.b, 0.9);
                }
            }
        }
        let current = frame.frame;
        self.shots.retain(|flight| flight.frame == current);
    }

    fn write_missiles(
        &mut self,
        missiles: &[Missile],
        monsters: &MonsterView,
        batches: &mut RenderBatches,
        frame: &FrameContext,
    ) {
        let dt = frame.delta_seconds;
        for missile in missiles {
            if missile.removed {
                continue;
            }
            let flight = self.missiles.get_or_insert_with(missile.id, |_| MissileFlight {
                frame: 0,
                x: missile.x,
                y: missile.y,
                launch_x: missile.previous_x,
                launch_y: missile.previous_y,
                altitude: MISSILE_RACK_Y + 1.0,
                climb_angle: 0.0,
            });
            flight.frame = frame.frame;
            flight.x = missile.x;
            flight.y = missile.y;

            let travelled = (missile.x - flight.launch_x).hypot(missile.y - flight.launch_y) as f32;
            let climb = clamp(travelled / MISSILE_CLIMB_DISTANCE, 0.0, 1.0);
            let mut desired = MISSILE_RACK_Y
                + (MISSILE_CRUISE_ALTITUDE - MISSILE_RACK_Y) * (climb * std::f32::consts::PI * 0.5).sin();
            let target =
                missile.tracked_monster.as_ref().map(|target| target.borrow()).filter(|target| !target.removed);
            if let Some(target) = target {
                let remaining = (target.x - missile.x).hypot(target.y - missile.y) as f32;
                let dive = clamp(1.0 - remaining / 110.0, 0.0, 1.0);
                let target_height = monsters.center_height(target.id, target.radius);
                desired += (target_height - desired) * dive * dive;
            } else {
                desired = 8f32.max(desired - travelled * 0.05);
            }
            let previous_altitude = flight.altitude;
            if dt > 0.0 {
                flight.altitude = smooth_towards(flight.altitude, desired, 14.0, dt);
            }
            let horizontal_speed = 1f32.max(missile.speed_per_second as f32);
            let vertical_speed = if dt > 0.0 { (flight.altitude - previous_altitude) / dt } else { 0.0 };
            flight.climb_angle = vertical_speed.atan2(horizontal_speed);

            let angle = missile.angle as f32;
            let (x, z) = (missile.x as f32, missile.y as f32);
            let scale = missile.scale as f32 * 0.62;
            batches.push_blob_shadow(x, z, flight.altitude, 8.0 * scale, 2.6 * scale, -angle, 0.4);
            self.rotation.set_heading_pitch_roll(angle, flight.climb_angle, (frame.time * 6.0) as f32);
            let q = self.rotation;
            let body = linear_color(MISSILE_BODY);
            batches[Batch::Missile].push_quaternion(
                x,
                flight.altitude,
                z,
                q.x,
                q.y,
                q.z,
                q.w,
                scale,
                scale,
                scale,
                body.r,
                body.g,
                body.b,
            );

            let bloom = (missile.launch_bloom_seconds / 0.28) as f32;
            let back = 7.2 * scale * 1.6;
            let cos_pitch = flight.climb_angle.cos();
            let flame_x = x - angle.cos() * back * cos_pitch;
            let flame_z = z - angle.sin() * back * cos_pitch;
            let flame_y = flight.altitude - flight.climb_angle.sin() * back;
            let flicker = 0.85 + ((frame.time * 47.0 + missile.x).sin() * 0.15) as f32;
            let flame = linear_color(MISSILE_FLAME).scaled(2.4);
            let (long, short) = ((9.0 + bloom * 8.0) * flicker, (4.5 + bloom * 3.0) * flicker);
            batches.glow.push(flame_x, flame_y, flame_z, -angle, long, short, 0.0, flame.r, flame.g, flame.b, 0.95);
            let core = linear_color(MISSILE_FLAME_CORE).scaled(3.0);
            let size = 4.0 + bloom * 3.0;
            batches.glow.push(flame_x, flame_y, flame_z, 0.0, size, size, 0.0, core.r, core.g, core.b, 1.0);
        }
        let current = frame.frame;
        self.missiles.retain(|flight| flight.frame == current);
    }

    fn write_drones(&mut self, drones: &[DroneRef], batches: &mut RenderBatches, frame: &FrameContext) {
        let dt = frame.delta_seconds;
        for drone_ref in drones {
            let drone = drone_ref.borrow();
            if drone.removed {
                continue;
            }
            let flight = self.drones.get_or_insert_with(drone.id, |_| DroneFlight {
                frame: 0,
                x: drone.x,
                y: drone.y,
                altitude: 8.0,
                bank: 0.0,
                pitch: 0.0,
                previous_x: drone.x,
                previous_y: drone.y,
                previous_angle: drone.angle,
            });
            flight.frame = frame.frame;
            flight.x = drone.x;
            flight.y = drone.y;

            if dt > 0.0 {
                let exiting = drone.is_exiting();
                let target_altitude =
                    if exiting { flight.altitude + DRONE_EXIT_CLIMB_PER_SECOND * dt } else { DRONE_CRUISE_ALTITUDE };
                flight.altitude =
                    if exiting { target_altitude } else { smooth_towards(flight.altitude, target_altitude, 3.0, dt) };
                let velocity_x = (drone.x - flight.previous_x) as f32 / dt;
                let velocity_y = (drone.y - flight.previous_y) as f32 / dt;
                let heading = drone.angle as f32;
                let forward_speed = velocity_x * heading.cos() + velocity_y * heading.sin();
                let turn_rate = normalize_angle(drone.angle - flight.previous_angle) as f32 / dt;
                flight.pitch = smooth_towards(flight.pitch, clamp(-forward_speed * 0.0022, -0.35, 0.35), 6.0, dt);
                flight.bank = smooth_towards(flight.bank, clamp(turn_rate * 0.09, -0.45, 0.45), 6.0, dt);
            }
            flight.previous_x = drone.x;
            flight.previous_y = drone.y;
            flight.previous_angle = drone.angle;

            let accent = linear_color(drone_accent_color(drone.level));
            let level = drone.level as f32;
            let scale = 0.752 + level * 0.025;
            let angle = drone.angle as f32;
            let (x, z) = (drone.x as f32, drone.y as f32);
            batches.push_blob_shadow(x, z, flight.altitude, 11.0 * scale, 11.0 * scale, -angle, 0.45);
            self.rotation.set_heading_pitch_roll(angle, flight.pitch, flight.bank);
            let q = self.rotation;
            batches[Batch::DroneBody].push_quaternion(
                x,
                flight.altitude,
                z,
                q.x,
                q.y,
                q.z,
                q.w,
                scale,
                scale,
                scale,
                accent.r,
                accent.g,
                accent.b,
            );

            let (sin, cos) = angle.sin_cos();
            let rotor_size = (2.85 + level * 0.18) * 2.6 * scale;
            let shimmer = 0.3 + ((frame.time * 52.0 + drone.id as f64).sin() * 0.1) as f32;
            let rotor = linear_color(ROTOR_COLOR);
            for (local_x, local_z) in ROTOR_OFFSETS {
                let world_x = x + (local_x * cos - local_z * sin) * scale;
                let world_z = z + (local_x * sin + local_z * cos) * scale;
                let y = flight.altitude + 1.6;
                batches
                    .glow
                    .push(world_x, y, world_z, 0.0, rotor_size, rotor_size, 1.0, rotor.r, rotor.g, rotor.b, shimmer);
            }
        }
        let current = frame.frame;
        self.drones.retain(|flight| flight.frame == current);
    }
}

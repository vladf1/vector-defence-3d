//! Draws every live monster through shared neon batches and turns lifecycle changes
//! (spawn, hit, kill, escape) into 3D effects. Monsters stay renderer-agnostic; this view
//! only reads their public presentation state.
use std::f32::consts::PI;

use vd_core::entities::{MonsterRef, MonsterSpecial};
use vd_core::level_runtime::LevelRuntime;
use vd_core::rng::random;
use vd_core::types::{Color, MonsterKind};
use vd_core::utils::normalize_angle;

use crate::frame_math::{FrameContext, Quat, hash01, smooth_towards};
use crate::fx_system::FxSystem;
use crate::id_map::IdMap;
use crate::palette::{LinearColor, linear_color, srgb_to_linear};
use crate::render_batches::{Batch, RenderBatches};

const SPAWN_MATERIALIZE_SECONDS: f32 = 0.3;
const HIT_FLASH_DECAY_PER_SECOND: f32 = 7.0;
const BANK_RESPONSE_PER_SECOND: f32 = 7.0;
const MAX_BANK: f32 = 0.55;
const HEALTH_BAR_LIFT: f32 = 7.0;
const HEALTH_BAR_GAP: f32 = 3.0;
const HEALTH_BAR_HEIGHT: f32 = 3.2;
const ESCAPE_PROGRESS_THRESHOLD: f64 = 0.995;
const ICE: Color = 0x9ff4ff;
const BULWARK_ARMOR_GLOW: Color = 0xdff7ff;
const BULWARK_CORE_TINT: f32 = 0.6;
const HEALTH_TRACK: LinearColor = LinearColor::new(0.004, 0.01, 0.008);
const BERSERKER_EMBER_COLOR: Color = 0xffba4f;
const TANK_HULL_TOP: f32 = 0.82;

struct MonsterVisual {
    /// Kept so a departed monster's final state (health, position) can be read, as the
    /// TypeScript `Map<Monster, ...>` kept the object alive.
    monster: MonsterRef,
    frame: u32,
    phase: f32,
    age: f32,
    previous_heading: f64,
    bank: f32,
    last_hit_points: f64,
    hit_flash: f32,
    center_y: f32,
    ember_timer: f32,
}

fn mix_channel(from: f32, to: f32, amount: f32) -> f32 {
    from + (to - from) * amount
}

fn clamp(value: f32, min: f32, max: f32) -> f32 {
    max.min(min.max(value))
}

#[derive(Default)]
pub struct MonsterView {
    visuals: IdMap<MonsterVisual>,
    spawn_x: f64,
    spawn_y: f64,
    rotation: Quat,
}

impl MonsterView {
    pub fn reset(&mut self, runtime: &LevelRuntime) {
        self.visuals.clear();
        let start = runtime.route_path.as_ref().map(|route| route.start);
        (self.spawn_x, self.spawn_y) = start.map_or((0.0, 0.0), |start| (start.x, start.y));
    }

    /// World height of a monster's body center, for beams and impact effects.
    pub fn center_height(&self, id: u32, radius: f64) -> f32 {
        self.visuals.get(id).map_or(radius as f32 + 3.0, |visual| visual.center_y)
    }

    pub fn write(
        &mut self,
        runtime: &LevelRuntime,
        batches: &mut RenderBatches,
        fx: &mut FxSystem,
        frame: &FrameContext,
    ) {
        for monster_ref in &runtime.monsters {
            let monster = monster_ref.borrow();
            if monster.removed {
                continue;
            }
            let (spawn_x, spawn_y) = (self.spawn_x, self.spawn_y);
            let visual = self.visuals.get_or_insert_with(monster.id, |count| {
                let seed = monster.x * 0.137 + monster.y * 0.271 + count as f64 * 1.618 + monster.distance_along_path;
                let near_spawn = (monster.x - spawn_x).hypot(monster.y - spawn_y) < 4.0;
                if near_spawn {
                    fx.spawn_pulse(monster.x as f32, monster.y as f32, monster.color, monster.radius as f32);
                }
                MonsterVisual {
                    monster: monster_ref.clone(),
                    frame: 0,
                    phase: hash01(seed) * PI * 2.0,
                    age: if near_spawn { 0.0 } else { SPAWN_MATERIALIZE_SECONDS },
                    previous_heading: monster.angle,
                    bank: 0.0,
                    last_hit_points: monster.hit_points,
                    hit_flash: 0.0,
                    center_y: monster.radius as f32 + 3.0,
                    ember_timer: 0.0,
                }
            });
            visual.frame = frame.frame;
            advance(&monster, visual, fx, frame);
            write_monster(&monster, visual, &mut self.rotation, batches, fx, frame);
            write_shadow(&monster, visual, batches);
            write_health_bar(&monster, visual, batches);
        }

        let current = frame.frame;
        self.visuals.retain(|visual| {
            if visual.frame == current {
                return true;
            }
            resolve_departure(visual, fx);
            false
        });
    }
}

fn advance(monster: &vd_core::entities::Monster, visual: &mut MonsterVisual, fx: &mut FxSystem, frame: &FrameContext) {
    let dt = frame.delta_seconds;
    visual.age += dt;
    if monster.hit_points < visual.last_hit_points - 0.01 {
        let damage = visual.last_hit_points - monster.hit_points;
        let discrete_hit = damage >= monster.max_hit_points * 0.012;
        let added = (damage / monster.max_hit_points * 5.0) as f32 + if discrete_hit { 0.25 } else { 0.0 };
        visual.hit_flash = (visual.hit_flash + added).min(1.0);
        if matches!(monster.special, MonsterSpecial::Bulwark(_)) && damage >= 3.0 {
            let (x, z) = (monster.visual_x() as f32, monster.visual_y() as f32);
            fx.shield_hit(x, visual.center_y, z, monster.radius as f32, BULWARK_ARMOR_GLOW);
        }
    }
    visual.last_hit_points = monster.hit_points;
    visual.hit_flash = (visual.hit_flash - HIT_FLASH_DECAY_PER_SECOND * dt).max(0.0);
    if dt > 0.0 {
        let turn_rate = normalize_angle(monster.angle - visual.previous_heading) as f32 / dt;
        let target_bank = clamp(turn_rate * 0.16, -MAX_BANK, MAX_BANK);
        visual.bank = smooth_towards(visual.bank, target_bank, BANK_RESPONSE_PER_SECOND, dt);
    }
    visual.previous_heading = monster.angle;
}

fn write_monster(
    monster: &vd_core::entities::Monster,
    visual: &mut MonsterVisual,
    rotation: &mut Quat,
    batches: &mut RenderBatches,
    fx: &mut FxSystem,
    frame: &FrameContext,
) {
    let materialize = (visual.age / SPAWN_MATERIALIZE_SECONDS).min(1.0);
    let grow = 1.0 - (1.0 - materialize) * (1.0 - materialize);
    let radius = monster.radius as f32;
    let r = radius * grow;
    let x = monster.visual_x() as f32;
    let z = monster.visual_y() as f32;
    let time = frame.time;
    let phase = visual.phase as f64;
    let bob = (time * 5.2 + phase).sin() as f32;
    let base = linear_color(monster.color);
    let ice = linear_color(ICE);
    let slow = if monster.max_speed_per_second > 0.0 {
        clamp(1.0 - (monster.speed_per_second / monster.max_speed_per_second) as f32, 0.0, 1.0)
    } else {
        0.0
    };
    let flash = visual.hit_flash + (1.0 - materialize) * 1.5;
    let ice_mix = slow * 0.35;
    let red = mix_channel(base.r, ice.r, ice_mix) * (1.0 + flash) + flash * 0.5;
    let green = mix_channel(base.g, ice.g, ice_mix) * (1.0 + flash) + flash * 0.5;
    let blue = mix_channel(base.b, ice.b, ice_mix) * (1.0 + flash) + flash * 0.5;
    let angle = monster.angle as f32;

    match &monster.special {
        MonsterSpecial::PackMan(_) => {
            let heading = angle + monster.current_body_rotation() as f32;
            let mouth = monster.current_mouth_angle() as f32;
            let y = r + 1.2 + bob * 0.9;
            visual.center_y = y;
            let waddle = ((time * 9.0 + phase).sin() * 0.08) as f32;
            let jaws = &mut batches[Batch::PackmanJaw];
            rotation.set_yaw(-(heading - mouth)).roll(waddle);
            let q = *rotation;
            jaws.push_quaternion(x, y, z, q.x, q.y, q.z, q.w, r, r, r, red, green, blue);
            let mut flip = Quat::IDENTITY;
            flip.set_axis_angle(1.0, 0.0, 0.0, PI);
            rotation.set_yaw(-(heading + mouth)).roll(waddle).multiply(&flip);
            let q = *rotation;
            jaws.push_quaternion(x, y, z, q.x, q.y, q.z, q.w, r, r, r, red, green, blue);
        }
        MonsterSpecial::Square(_) => {
            let size = monster.get_visual_radius() as f32 * grow;
            let y = size + 4.5 + bob * 1.3;
            visual.center_y = y;
            let roll = 0.34 + ((time * 2.1 + phase).sin() * 0.12) as f32;
            rotation.set_yaw(-(monster.rotation as f32)).roll(roll).pitch(0.22);
            let q = *rotation;
            let body = &mut batches[Batch::SquareBody];
            body.push_quaternion(x, y, z, q.x, q.y, q.z, q.w, size, size, size, red, green, blue);
        }
        MonsterSpecial::Triangle(_) => {
            let wobble = monster.current_nose_wobble_angle() as f32;
            let y = r * 0.4 + 4.0 + bob * 1.1;
            visual.center_y = y;
            rotation.set_heading_pitch_roll(angle + wobble, 0.06, visual.bank - wobble * 1.4);
            let q = *rotation;
            let body = &mut batches[Batch::TriangleBody];
            body.push_quaternion(x, y, z, q.x, q.y, q.z, q.w, r, r, r, red, green, blue);
        }
        MonsterSpecial::Tank(_) => {
            let rumble = ((time * 23.0 + phase).sin() * 0.18) as f32;
            visual.center_y = r * 0.6;
            rotation.set_heading_pitch_roll(angle, rumble * 0.02, visual.bank * 0.25);
            let q = *rotation;
            batches[Batch::TankHull].push_quaternion(x, rumble, z, q.x, q.y, q.z, q.w, r, r, r, red, green, blue);
            let turret_offset = r * 0.08;
            rotation.set_yaw(-(angle + monster.current_turret_rotation() as f32));
            let q = *rotation;
            batches[Batch::TankTurret].push_quaternion(
                x + angle.cos() * turret_offset,
                TANK_HULL_TOP * r + rumble,
                z + angle.sin() * turret_offset,
                q.x,
                q.y,
                q.z,
                q.w,
                r,
                r,
                r,
                red,
                green,
                blue,
            );
        }
        MonsterSpecial::Runner(_) => {
            let dash = monster.get_dash_pulse() as f32;
            let y = 3.4 + bob * 0.5;
            visual.center_y = y + 1.5;
            rotation.set_heading_pitch_roll(angle, 0.0, visual.bank * 1.3);
            let q = *rotation;
            let (scale_x, scale_z) = (r * (1.0 + dash * 0.3), r * (1.0 - dash * 0.12));
            let body = &mut batches[Batch::RunnerBody];
            body.push_quaternion(x, y, z, q.x, q.y, q.z, q.w, scale_x, r, scale_z, red, green, blue);
            write_runner_trail(monster, dash, y + r * 0.2, base, batches, frame);
        }
        MonsterSpecial::Splitter => {
            let y = r * 0.5 + 4.0 + bob * 1.2;
            visual.center_y = y;
            let roll = ((time * 1.7 + phase).sin() * 0.14) as f32;
            rotation.set_yaw(-(monster.rotation as f32)).roll(roll);
            let q = *rotation;
            let body = &mut batches[Batch::SplitterBody];
            body.push_quaternion(x, y, z, q.x, q.y, q.z, q.w, r, r, r, red, green, blue);
        }
        MonsterSpecial::Berserker(_) => {
            let motion = monster.get_rage_motion();
            let stage = monster.current_rage_stage();
            let tremor = if stage == 2 { ((time * 61.0).sin() * 0.06) as f32 } else { 0.0 };
            let y = 1.6 + bob * 0.35;
            visual.center_y = y + r * 0.4;
            let pitch = if stage == 2 { -0.1 } else { 0.0 };
            rotation.set_heading_pitch_roll(angle, pitch, visual.bank * 0.9 + tremor);
            let q = *rotation;
            let scale_x = r * motion.scale_x as f32;
            let scale_y = r * (1.0 + stage as f32 * 0.1);
            let scale_z = r * motion.scale_y as f32;
            batches[Batch::BerserkerBody]
                .push_quaternion(x, y, z, q.x, q.y, q.z, q.w, scale_x, scale_y, scale_z, red, green, blue);
            if stage > 0 {
                let glow = 1.2 + ((time * 14.0).sin() * 0.3) as f32;
                let (gr, gg, gb) = (red * glow, green * glow, blue * glow);
                batches[Batch::BerserkerSpikes]
                    .push_quaternion(x, y, z, q.x, q.y, q.z, q.w, scale_x, scale_y, scale_z, gr, gg, gb);
            }
            emit_berserker_embers(monster, visual, motion.ember_alpha as f32, fx, frame);
        }
        MonsterSpecial::Bulwark(_) => {
            let y = 0.6 + ((time * 2.3 + phase).sin() * 0.3) as f32;
            visual.center_y = y + r * 0.5;
            rotation.set_heading_pitch_roll(angle, 0.0, visual.bank * 0.3);
            let q = *rotation;
            batches[Batch::BulwarkShell].push_quaternion(x, y, z, q.x, q.y, q.z, q.w, r, r, r, red, green, blue);
            let pulse = 0.42 + (monster.current_shield_pulse().sin() * 0.18) as f32 + flash * 0.5;
            // Lean the armor glow toward the bulwark's own blue so it doesn't read as a white core.
            let armor = linear_color(BULWARK_ARMOR_GLOW);
            batches[Batch::BulwarkCore].push_quaternion(
                x,
                y,
                z,
                q.x,
                q.y,
                q.z,
                q.w,
                r,
                r,
                r,
                mix_channel(armor.r, red, BULWARK_CORE_TINT) * pulse,
                mix_channel(armor.g, green, BULWARK_CORE_TINT) * pulse,
                mix_channel(armor.b, blue, BULWARK_CORE_TINT) * pulse,
            );
        }
    }
}

fn write_runner_trail(
    monster: &vd_core::entities::Monster,
    dash: f32,
    y: f32,
    color: LinearColor,
    batches: &mut RenderBatches,
    frame: &FrameContext,
) {
    let r = monster.radius as f32;
    let angle = monster.angle as f32;
    let (sin, cos) = angle.sin_cos();
    let length = r * (2.2 + dash * 3.2);
    let intensity = 0.45 + dash * 0.9;
    let flicker = 0.85 + ((frame.time * 40.0 + monster.distance_along_path).sin() * 0.15) as f32;
    let (x, z) = (monster.visual_x() as f32, monster.visual_y() as f32);
    let ribbon = &mut batches[Batch::Ribbon];
    let shade = intensity * flicker;
    for side in [-1.0, 1.0] {
        let offset = side * r * 0.42;
        let start_x = x - cos * r * 1.1 - sin * offset;
        let start_z = z - sin * r * 1.1 + cos * offset;
        let slot = ribbon.push_yaw(
            start_x,
            y,
            start_z,
            -(angle + PI),
            length,
            1.0,
            1.4 + dash,
            color.r * shade,
            color.g * shade,
            color.b * shade,
        );
        ribbon.set_extra(slot, 0, 0.0);
    }
}

fn emit_berserker_embers(
    monster: &vd_core::entities::Monster,
    visual: &mut MonsterVisual,
    ember_alpha: f32,
    fx: &mut FxSystem,
    frame: &FrameContext,
) {
    visual.ember_timer -= frame.delta_seconds;
    if visual.ember_timer > 0.0 {
        return;
    }
    visual.ember_timer += 0.12 / (0.3 + ember_alpha);
    let angle = monster.angle as f32;
    let (sin, cos) = angle.sin_cos();
    let radius = monster.radius as f32;
    let back = radius * 1.1;
    let lateral = (random() as f32 - 0.5) * radius;
    let (x, z) = (monster.visual_x() as f32, monster.visual_y() as f32);
    let vx = -cos * 70.0 + (random() as f32 - 0.5) * 30.0;
    let vz = -sin * 70.0 + (random() as f32 - 0.5) * 30.0;
    fx.ember(
        x - cos * back - sin * lateral,
        visual.center_y,
        z - sin * back + cos * lateral,
        vx,
        vz,
        BERSERKER_EMBER_COLOR,
    );
}

fn is_heavy_shadow(kind: MonsterKind) -> bool {
    matches!(kind, MonsterKind::Tank | MonsterKind::Bulwark)
}

fn write_shadow(monster: &vd_core::entities::Monster, visual: &MonsterVisual, batches: &mut RenderBatches) {
    let grow = (visual.age / SPAWN_MATERIALIZE_SECONDS).min(1.0);
    let r = monster.radius as f32 * grow;
    let tank = is_heavy_shadow(monster.kind());
    let (radius_x, radius_z) = if tank { (r * 1.3, r) } else { (r * 1.1, r * 1.1) };
    let (x, z) = (monster.visual_x() as f32, monster.visual_y() as f32);
    batches.push_blob_shadow(x, z, visual.center_y, radius_x, radius_z, -(monster.angle as f32), 0.75);
}

fn write_health_bar(monster: &vd_core::entities::Monster, visual: &MonsterVisual, batches: &mut RenderBatches) {
    let radius = monster.radius as f32;
    let width = 16f32.max(radius * 2.0);
    let ratio = clamp((monster.hit_points / monster.max_hit_points) as f32, 0.0, 1.0);
    let x = monster.visual_x() as f32;
    let y = visual.center_y + radius + HEALTH_BAR_LIFT;
    let z = monster.visual_y() as f32 - radius - HEALTH_BAR_GAP;
    let bars = &mut batches[Batch::HealthBar];
    let track = HEALTH_TRACK;
    bars.push_yaw(x, y, z, 0.0, width + 1.2, 1.0, HEALTH_BAR_HEIGHT + 1.2, track.r, track.g, track.b);
    if ratio <= 0.0 {
        return;
    }
    let fill_width = width * ratio;
    let (red, green, blue) = if ratio > 0.5 {
        let danger = (1.0 - ratio) * 2.0;
        ((76.0 + 179.0 * danger) / 255.0, 1.0, 144.0 * (1.0 - danger) / 255.0)
    } else {
        let danger = 1.0 - ratio * 2.0;
        (1.0, 227.0 * (1.0 - danger) / 255.0, 79.0 / 255.0)
    };
    let linear = |channel: f32| srgb_to_linear(channel as f64) as f32 * 1.3;
    let fill_x = x - width / 2.0 + fill_width / 2.0;
    bars.push_yaw(
        fill_x,
        y + 0.1,
        z,
        0.0,
        fill_width,
        1.0,
        HEALTH_BAR_HEIGHT,
        linear(red),
        linear(green),
        linear(blue),
    );
}

fn resolve_departure(visual: &MonsterVisual, fx: &mut FxSystem) {
    let monster = visual.monster.borrow();
    if monster.hit_points <= 0.0 {
        let heavy = matches!(monster.kind(), MonsterKind::Tank | MonsterKind::Bulwark | MonsterKind::Berserker);
        let (x, z) = (monster.visual_x() as f32, monster.visual_y() as f32);
        fx.monster_death(x, visual.center_y, z, monster.color, monster.radius as f32, heavy);
        return;
    }
    if monster.get_path_progress() >= ESCAPE_PROGRESS_THRESHOLD {
        fx.escape_blast(monster.x as f32, monster.y as f32);
    }
}

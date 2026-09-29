//! Tower 3D presentation: reads each tower's public state and composes instanced neon parts.
use std::f32::consts::PI;

use vd_core::constants::{TIMER_EPSILON_SECONDS, TOWER_RADIUS, TOWER_UPGRADE_RING_GROWTH, TOWER_UPGRADE_RING_OFFSET};
use vd_core::entities::drone_visuals::drone_accent_color;
use vd_core::entities::projectiles::missile::get_missile_scale;
use vd_core::entities::towers::gun::{GUN_PROJECTILE_SOURCE_OFFSET, MUZZLE_FLASH_DURATION_SECONDS};
use vd_core::entities::{Tower, TowerRef};
use vd_core::types::{Color, TowerKind};
use vd_core::utils::ease_out_cubic;

use crate::frame_math::FrameContext;
use crate::fx_system::FxSystem;
use crate::id_map::IdMap;
use crate::models::{
    DRONE_PAD_TOP, GUN_BARREL_Y, LASER_CRYSTAL_Y, MISSILE_RACK_Y, SLOW_CORE_Y, TESLA_COIL_SCALE, TESLA_TOP_Y,
};
use crate::palette::{LinearColor, linear_color};
use crate::render_batches::{Batch, RenderBatches};

const GUN_ACCENT: Color = 0xffe27a;
const GUN_BODY: Color = 0xe6fff4;
const GUN_POWERBANK: Color = 0x9dffd7;
const MUZZLE_HOT: Color = 0xfff7d1;
const MUZZLE_WARM: Color = 0xff9d5c;
const MISSILE_BODY: Color = 0xff9d5c;
const SLOW_ACCENT: Color = 0xd8ff4f;
const SLOW_CORE: Color = 0xffdc5c;
const SLOW_NODE_WARM: Color = 0xffe27a;
const LOCK_COLOR: Color = 0xffe36f;
const DARK_PIP: LinearColor = LinearColor::new(0.012, 0.02, 0.018);
const LEVEL_PIP_SLOTS: u32 = 6;
const LEVEL_PIP_RADIUS: f32 = 12.2;
const LEVEL_PIP_Y: f32 = 2.3;
const DRONE_READINESS_PIPS: u32 = 12;
const LASER_BEAM_LENGTH: f32 = 1000.0;
const MISSILE_RELOAD_TRAVEL: f32 = 22.0;
const DRONE_DOCK_SCALE: f32 = 0.7;
const MAX_LIGHTNING_COOLDOWN_SECONDS: f32 = 1.12;
const SELECTED_BOOST: f32 = 1.7;
// The original's base strokes: white for most towers, softer for the missile and drone pads.
const PLINTH: Color = 0x3a4a44;
const RIM_WHITE: Color = 0xffffff;
const MISSILE_RIM: Color = 0xd7e2ea;
const DRONE_RIM: Color = 0xeffff7;
const RIM_INTENSITY: f32 = 0.9;
// Upgrade halos: the original's outer ring, in each tower's ring color, widening and
// brightening with each level.
const GOLD_HALO: Color = 0xffe27a;
const SLOW_HALO: Color = 0xd8ff4f;
const DRONE_HALO: Color = 0x9dffd7;
const HALO_Y: f32 = 0.6;
const HALO_BASE_INTENSITY: f32 = 0.32;
const HALO_INTENSITY_PER_LEVEL: f32 = 0.08;

struct TowerVisual {
    frame: u32,
    previous_cooldown: f64,
}

#[derive(Default)]
pub struct TowerView {
    visuals: IdMap<TowerVisual>,
}

/// A part color: the hologram tint when set, otherwise `color`.
fn tinted(tint: Option<LinearColor>, color: Color) -> LinearColor {
    tint.unwrap_or_else(|| linear_color(color))
}

impl TowerView {
    pub fn reset(&mut self) {
        self.visuals.clear();
    }

    pub fn write(
        &mut self,
        towers: &[TowerRef],
        selected: Option<&TowerRef>,
        batches: &mut RenderBatches,
        fx: &mut FxSystem,
        frame: &FrameContext,
    ) {
        let selected_id = selected.map(|tower| tower.borrow().id);
        for tower_ref in towers {
            let tower = tower_ref.borrow();
            let visual = self
                .visuals
                .get_or_insert_with(tower.id, |_| TowerVisual { frame: 0, previous_cooldown: tower.cooldown_seconds });
            visual.frame = frame.frame;
            detect_firing(&tower, visual, fx);
            write_tower(&tower, selected_id == Some(tower.id), None, batches, frame);
        }
        let current = frame.frame;
        self.visuals.retain(|visual| visual.frame == current);
    }
}

fn detect_firing(tower: &Tower, visual: &mut TowerVisual, fx: &mut FxSystem) {
    let fired = tower.cooldown_seconds > visual.previous_cooldown + 0.25;
    visual.previous_cooldown = tower.cooldown_seconds;
    // Slow pulses show only through their links to the slowed monsters.
    if fired && tower.kind == TowerKind::Lightning {
        fx.zap(tower.x as f32, TESLA_TOP_Y, tower.y as f32);
    }
}

/// Draws one tower; with a tint every part uses the hologram color.
pub fn write_tower(
    tower: &Tower,
    selected: bool,
    tint: Option<LinearColor>,
    batches: &mut RenderBatches,
    frame: &FrameContext,
) {
    let accent = accent_color(tower);
    let boost = if selected { SELECTED_BOOST } else { 1.0 };
    let (x, z) = (tower.x as f32, tower.y as f32);
    let level = tower.level as f32;
    // The original's near-black base fill; the rim, halo, and weapon carry the color.
    let base = tinted(tint, PLINTH).scaled(boost);
    if tint.is_none() {
        batches.push_blob_shadow(x, z, shadow_height(tower), 14.5, 14.5, 0.0, 0.6);
    }
    batches[Batch::TowerBase].push_yaw(x, 0.0, z, 0.0, 1.0, 1.0, 1.0, base.r, base.g, base.b);
    let rim_intensity = if tint.is_some() { 1.0 } else { RIM_INTENSITY * boost };
    let rim = tinted(tint, rim_color(tower)).scaled(rim_intensity);
    batches[Batch::TowerRim].push_yaw(x, 0.0, z, 0.0, 1.0, 1.0, 1.0, rim.r, rim.g, rim.b);
    if tower.level > 0 {
        let halo_radius =
            (TOWER_RADIUS + TOWER_UPGRADE_RING_OFFSET + tower.level as f64 * TOWER_UPGRADE_RING_GROWTH) as f32;
        let halo_intensity = (HALO_BASE_INTENSITY + level * HALO_INTENSITY_PER_LEVEL) * boost;
        let halo = tinted(tint, halo_color(tower)).scaled(halo_intensity);
        batches[Batch::UpgradeRing].push_yaw(
            x,
            HALO_Y,
            z,
            0.0,
            halo_radius,
            halo_radius,
            halo_radius,
            halo.r,
            halo.g,
            halo.b,
        );
    }
    write_level_pips(tower, accent, tint, batches);

    match tower.kind {
        TowerKind::Gun => write_gun(tower, tint, batches),
        TowerKind::Laser => write_laser(tower, tint, batches),
        TowerKind::Missile => write_missile_tower(tower, tint, batches),
        TowerKind::Slow => write_slow(tower, tint, batches, frame),
        TowerKind::Drone => write_drone_tower(tower, accent, tint, batches, frame),
        TowerKind::Lightning => write_lightning(tower, tint, batches, frame),
    }
}

fn write_level_pips(tower: &Tower, accent: LinearColor, tint: Option<LinearColor>, batches: &mut RenderBatches) {
    let pips = &mut batches[Batch::Pip];
    for slot in 0..LEVEL_PIP_SLOTS {
        let angle = slot as f32 / LEVEL_PIP_SLOTS as f32 * PI * 2.0 + PI / 2.0;
        let lit = slot < tower.level;
        let color = tint.unwrap_or(if lit { accent } else { DARK_PIP });
        let intensity = if lit { 1.8 } else { 1.0 };
        pips.push_yaw(
            tower.x as f32 + angle.cos() * LEVEL_PIP_RADIUS,
            LEVEL_PIP_Y,
            tower.y as f32 + angle.sin() * LEVEL_PIP_RADIUS,
            -angle,
            1.0,
            1.0,
            1.0,
            color.r * intensity,
            color.g * intensity,
            color.b * intensity,
        );
    }
}

fn write_gun(tower: &Tower, tint: Option<LinearColor>, batches: &mut RenderBatches) {
    let angle = tower.angle() as f32;
    let yaw = -angle;
    let (sin, cos) = angle.sin_cos();
    let (x, z) = (tower.x as f32, tower.y as f32);
    let level = tower.level as f32;
    let body = tinted(tint, GUN_BODY);
    batches[Batch::GunHead].push_yaw(x, 0.0, z, yaw, 1.0, 1.0, 1.0, body.r, body.g, body.b);

    let flash = (tower.muzzle_flash_seconds() / MUZZLE_FLASH_DURATION_SECONDS).clamp(0.0, 1.0) as f32;
    let recoil = flash * 2.4;
    let barrel_radius = 1.0 + level * 0.2;
    let front = GUN_PROJECTILE_SOURCE_OFFSET as f32 + level * 0.9 - recoil;
    let back = -2.0 - level * 0.55;
    let length = front - back;
    batches[Batch::GunBarrel].push_yaw(
        x + cos * back,
        GUN_BARREL_Y,
        z + sin * back,
        yaw,
        length,
        barrel_radius,
        barrel_radius,
        body.r,
        body.g,
        body.b,
    );
    let muzzle = tinted(tint, GUN_ACCENT).scaled(1.0 + flash * 2.0);
    batches[Batch::GunMuzzle].push_yaw(
        x + cos * front,
        GUN_BARREL_Y,
        z + sin * front,
        yaw,
        barrel_radius,
        barrel_radius,
        barrel_radius,
        muzzle.r,
        muzzle.g,
        muzzle.b,
    );

    if tower.level > 0 {
        let rail = tinted(tint, GUN_POWERBANK);
        let rail_length = 7.8 + level * 0.6;
        let rail_offset = barrel_radius + 1.3;
        for side in [-1.0, 1.0] {
            batches[Batch::Rail].push_yaw(
                x + cos * back - sin * rail_offset * side,
                GUN_BARREL_Y,
                z + sin * back + cos * rail_offset * side,
                yaw,
                rail_length,
                0.9 + level * 0.08,
                0.9,
                rail.r,
                rail.g,
                rail.b,
            );
        }
    }

    if flash > 0.0 && tint.is_none() {
        let tip_x = x + cos * (front + 2.0);
        let tip_z = z + sin * (front + 2.0);
        let flash_size = 10.0 + level * 0.9;
        let warm = linear_color(MUZZLE_WARM).scaled(2.0);
        let hot = linear_color(MUZZLE_HOT).scaled(3.0);
        let glow = &mut batches.glow;
        let (long, short) = (flash_size * 1.6, flash_size * 0.9);
        glow.push(tip_x, GUN_BARREL_Y, tip_z, -angle, long, short, 0.0, warm.r, warm.g, warm.b, flash * 0.9);
        let core = flash_size * 0.6;
        glow.push(tip_x, GUN_BARREL_Y, tip_z, 0.0, core, core, 0.0, hot.r, hot.g, hot.b, flash);
    }
}

fn write_laser(tower: &Tower, tint: Option<LinearColor>, batches: &mut RenderBatches) {
    let colors = tower.laser_colors();
    let body = tinted(tint, colors.body);
    let accent = tinted(tint, colors.accent);
    let angle = tower.angle() as f32;
    let yaw = -angle;
    let (sin, cos) = angle.sin_cos();
    let (x, z) = (tower.x as f32, tower.y as f32);
    let level = tower.level as f32;
    let visual_level = level + 1.0;
    let muzzle = tower.get_muzzle_offset() as f32;
    let tail = -8.5 - visual_level * 0.36;
    let half_length = (muzzle - tail) / 2.0;
    let center = (muzzle + tail) / 2.0;
    let girth = 3.4 + visual_level * 0.26;
    let (charge, locked) =
        tower.laser().map_or((0.0, false), |laser| (laser.beam_alpha as f32, laser.direction_locked));

    batches[Batch::LaserCradle].push_yaw(x, 0.0, z, yaw, 1.0, 1.0, 1.0, accent.r, accent.g, accent.b);
    let crystal = body.scaled(1.0 + charge * 1.4);
    batches[Batch::LaserCrystal].push_yaw(
        x + cos * center,
        LASER_CRYSTAL_Y,
        z + sin * center,
        yaw,
        half_length,
        girth * 0.78,
        girth,
        crystal.r,
        crystal.g,
        crystal.b,
    );

    if tower.level > 0 {
        let rail_length = 6.22 + (level - 1.0) * 1.14;
        for side in [-1.0, 1.0] {
            let offset = girth + 1.6;
            batches[Batch::Rail].push_yaw(
                x + cos * -6.9 - sin * offset * side,
                LASER_CRYSTAL_Y - 1.2,
                z + sin * -6.9 + cos * offset * side,
                yaw,
                rail_length,
                0.8,
                0.9,
                accent.r,
                accent.g,
                accent.b,
            );
        }
    }

    if locked {
        let lock = tinted(tint, LOCK_COLOR).scaled(1.8);
        batches[Batch::OrbNode].push_yaw(
            x - cos * 5.2,
            LASER_CRYSTAL_Y + girth * 0.8,
            z - sin * 5.2,
            0.0,
            1.7,
            1.7,
            1.7,
            lock.r,
            lock.g,
            lock.b,
        );
    }

    if charge > 0.0 && tint.is_none() {
        let beam = linear_color(colors.beam);
        let source_x = x + cos * muzzle;
        let source_z = z + sin * muzzle;
        let ribbon = &mut batches[Batch::Ribbon];
        let outer_color = beam.scaled(0.5 * charge);
        let outer = ribbon.push_yaw(
            source_x,
            LASER_CRYSTAL_Y,
            source_z,
            yaw,
            LASER_BEAM_LENGTH,
            1.0,
            8.0 + level * 0.4,
            outer_color.r,
            outer_color.g,
            outer_color.b,
        );
        ribbon.set_extra(outer, 0, 0.0);
        let core_color = beam.scaled(2.2 * charge);
        let core = ribbon.push_yaw(
            source_x,
            LASER_CRYSTAL_Y + 0.05,
            source_z,
            yaw,
            LASER_BEAM_LENGTH,
            1.0,
            2.0 + level * 0.35,
            core_color.r,
            core_color.g,
            core_color.b,
        );
        ribbon.set_extra(core, 0, 0.0);
        let flare = 11.0 + level * 0.6;
        let glow = beam.scaled(2.5);
        batches.glow.push(source_x, LASER_CRYSTAL_Y, source_z, 0.0, flare, flare, 0.0, glow.r, glow.g, glow.b, charge);
    }
}

fn write_missile_tower(tower: &Tower, tint: Option<LinearColor>, batches: &mut RenderBatches) {
    let powerbank = tinted(tint, tower.missile_powerbank_color());
    let angle = tower.angle() as f32;
    let yaw = -angle;
    let (sin, cos) = angle.sin_cos();
    let (x, z) = (tower.x as f32, tower.y as f32);
    batches[Batch::MissileLauncher].push_yaw(x, 0.0, z, yaw, 1.0, 1.0, 1.0, powerbank.r, powerbank.g, powerbank.b);

    let reload = ease_out_cubic(tower.get_reload_progress()) as f32;
    if reload > 0.12 {
        let offset = -MISSILE_RELOAD_TRAVEL * (1.0 - reload);
        let scale = get_missile_scale(tower.level) as f32 * 0.62;
        let body = tinted(tint, MISSILE_BODY);
        batches[Batch::Missile].push_yaw(
            x + cos * (offset + 1.0),
            MISSILE_RACK_Y + 1.2,
            z + sin * (offset + 1.0),
            yaw,
            scale,
            scale,
            scale,
            body.r,
            body.g,
            body.b,
        );
    }
}

fn write_slow(tower: &Tower, tint: Option<LinearColor>, batches: &mut RenderBatches, frame: &FrameContext) {
    let (pulse_phase, orbit) = tower.slow().map_or((0.0, 0.0), |slow| (slow.pulse, slow.orbit));
    let (x, z) = (tower.x as f32, tower.y as f32);
    let pulse = 0.8 + (pulse_phase.sin() * 0.3) as f32;
    let core = tinted(tint, SLOW_CORE).scaled(pulse);
    batches[Batch::SlowCore].push_yaw(x, SLOW_CORE_Y, z, -(orbit as f32) * 0.5, 1.0, 1.0, 1.0, core.r, core.g, core.b);
    if tower.level == 0 {
        return;
    }
    let level = tower.level as f32;
    let node_count = 7.min(tower.level + 1);
    let orbit_radius = 7.5 + level * 0.75;
    let node_radius = 1.6 + level * 0.12;
    for index in 0..node_count {
        let angle = orbit + std::f64::consts::PI * 2.0 * index as f64 / node_count as f64;
        let color = tinted(tint, if index % 2 == 0 { SLOW_NODE_WARM } else { SLOW_ACCENT }).scaled(1.4);
        let bob = ((angle * 2.0 + frame.time * 2.5).sin() * 1.4) as f32;
        let angle = angle as f32;
        batches[Batch::OrbNode].push_yaw(
            x + angle.cos() * orbit_radius,
            SLOW_CORE_Y + bob,
            z + angle.sin() * orbit_radius,
            0.0,
            node_radius,
            node_radius,
            node_radius,
            color.r,
            color.g,
            color.b,
        );
    }
}

fn write_drone_tower(
    tower: &Tower,
    accent: LinearColor,
    tint: Option<LinearColor>,
    batches: &mut RenderBatches,
    frame: &FrameContext,
) {
    let color = tint.unwrap_or(accent);
    let readiness = tower.get_launch_readiness();
    let (x, z) = (tower.x as f32, tower.y as f32);
    batches[Batch::DronePad].push_yaw(x, 0.0, z, 0.0, 1.0, 1.0, 1.0, color.r, color.g, color.b);
    let pips = &mut batches[Batch::Pip];
    for slot in 0..DRONE_READINESS_PIPS {
        let angle = slot as f32 / DRONE_READINESS_PIPS as f32 * PI * 2.0 - PI / 2.0;
        let lit = (slot as f64 + 0.5) / DRONE_READINESS_PIPS as f64 <= readiness;
        let pip = tint.unwrap_or(if lit { accent } else { DARK_PIP }).scaled(if lit { 1.6 } else { 1.0 });
        pips.push_yaw(
            x + angle.cos() * 6.6,
            DRONE_PAD_TOP + 0.1,
            z + angle.sin() * 6.6,
            -angle,
            0.55,
            0.4,
            0.55,
            pip.r,
            pip.g,
            pip.b,
        );
    }
    if readiness >= 1.0 - TIMER_EPSILON_SECONDS {
        let hover = ((frame.time * 3.0).sin() * 0.4) as f32;
        let spin = (frame.time * 0.4) as f32;
        let scale = DRONE_DOCK_SCALE;
        batches[Batch::DroneBody].push_yaw(
            x,
            DRONE_PAD_TOP + 2.4 + hover,
            z,
            spin,
            scale,
            scale,
            scale,
            color.r,
            color.g,
            color.b,
        );
    }
}

fn write_lightning(tower: &Tower, tint: Option<LinearColor>, batches: &mut RenderBatches, frame: &FrameContext) {
    let coil = tinted(tint, tower.lightning_color());
    let cooldown = tower.cooldown_seconds as f32;
    let charge = if cooldown <= 0.0 { 1.0 } else { (1.0 - cooldown / MAX_LIGHTNING_COOLDOWN_SECONDS).clamp(0.0, 1.0) };
    let (x, z) = (tower.x as f32, tower.y as f32);
    let body = coil.scaled(0.65 + charge * 0.7);
    let spin = (frame.time * 0.6) as f32;
    batches[Batch::TeslaCoil].push_yaw(x, 0.0, z, spin, 1.0, 1.0, 1.0, body.r, body.g, body.b);
    if tint.is_none() {
        let crackle = 0.5
            + ((frame.time * 37.0 + tower.x).sin() * 0.25) as f32
            + ((frame.time * 53.0 + tower.y).sin() * 0.25) as f32;
        let size = (13.0 + tower.level as f32 * 0.8) * TESLA_COIL_SCALE;
        let glow = coil.scaled(1.8);
        let alpha = (0.25 + charge * 0.45) * crackle;
        batches.glow.push(x, TESLA_TOP_Y, z, 0.0, size, size, 0.0, glow.r, glow.g, glow.b, alpha);
    }
}

/// Rough height of each tower's mass, which sets how far its blob shadow falls.
fn shadow_height(tower: &Tower) -> f32 {
    match tower.kind {
        TowerKind::Lightning => 13.0 * TESLA_COIL_SCALE,
        TowerKind::Slow => 10.0,
        _ => 7.0,
    }
}

fn rim_color(tower: &Tower) -> Color {
    match tower.kind {
        TowerKind::Missile => MISSILE_RIM,
        TowerKind::Drone => DRONE_RIM,
        _ => RIM_WHITE,
    }
}

fn halo_color(tower: &Tower) -> Color {
    match tower.kind {
        // The original drew the laser halo in its beam color.
        TowerKind::Laser => tower.laser_colors().beam,
        TowerKind::Slow => SLOW_HALO,
        TowerKind::Drone => DRONE_HALO,
        _ => GOLD_HALO,
    }
}

fn accent_color(tower: &Tower) -> LinearColor {
    linear_color(match tower.kind {
        TowerKind::Gun => GUN_ACCENT,
        TowerKind::Laser => tower.laser_colors().body,
        TowerKind::Missile => tower.missile_powerbank_color(),
        TowerKind::Slow => SLOW_ACCENT,
        TowerKind::Drone => drone_accent_color(tower.level),
        TowerKind::Lightning => tower.lightning_color(),
    })
}

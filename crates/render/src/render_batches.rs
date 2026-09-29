//! Every drawable in the 3D board: the complete, fixed list of instanced batches (each
//! naming its pipeline) plus the two sprite batches, and the CPU geometry they draw.
use std::ops::{Index, IndexMut};

use crate::geometry_kit::{Mesh, to_neon_mesh};
use crate::gpu_pipelines::ScenePipeline;
use crate::instanced_batch::InstancedBatch;
use crate::models::{
    create_berserker_body, create_berserker_spikes, create_bulwark_core, create_bulwark_shell, create_drone_body,
    create_drone_pad, create_flat_quad, create_gun_barrel, create_gun_head, create_gun_muzzle, create_laser_cradle,
    create_laser_crystal, create_level_pip, create_missile, create_missile_launcher, create_orb_node,
    create_pack_man_jaw, create_portal, create_rail, create_range_quad, create_ribbon_quad, create_runner_body,
    create_shard, create_slow_core, create_spawn_gate, create_splitter_body, create_square_body, create_tank_hull,
    create_tank_turret, create_tesla_coil, create_tower_base, create_tower_rim, create_triangle_body,
    create_upgrade_ring,
};
use crate::shaders::{effect_mode, neon_mode, sprite_mode};
use crate::sprite_batch::SpriteBatch;

#[derive(Clone, Copy, Debug)]
pub struct BatchCapacities {
    pub glow_sprites: usize,
    pub smoke_sprites: usize,
    pub ribbons: usize,
}

const ENTITY_CAPACITY: usize = 256;
const PIP_CAPACITY: usize = 1024;
const SHARD_CAPACITY: usize = 1536;
const DECAL_CAPACITY: usize = 3072;
// One chevron per 30 units of road; the longest route needs about a hundred.
const ROAD_CHEVRON_CAPACITY: usize = 256;
// Blob shadows fall away from the key light (see the renderer's sun placement): per unit height.
const SHADOW_OFFSET_X: f32 = 0.39;
const SHADOW_OFFSET_Z: f32 = 0.52;
const BLOB_SHADOW_Y: f32 = 0.6;
const SHADOW_HEIGHT_SPREAD: f32 = 70.0;

/// Every instanced batch, in creation order: the neon parts first (their draw order), then
/// the flat effect layers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Batch {
    TowerBase,
    TowerRim,
    UpgradeRing,
    Pip,
    GunHead,
    GunBarrel,
    GunMuzzle,
    Rail,
    LaserCrystal,
    LaserCradle,
    MissileLauncher,
    Missile,
    SlowCore,
    OrbNode,
    DronePad,
    TeslaCoil,
    DroneBody,
    PackmanJaw,
    SquareBody,
    TriangleBody,
    TankHull,
    TankTurret,
    RunnerBody,
    SplitterBody,
    BerserkerBody,
    BerserkerSpikes,
    BulwarkShell,
    BulwarkCore,
    Shard,
    Portal,
    SpawnGate,
    Decal,
    Range,
    GroundGlow,
    RoadChevron,
    Ribbon,
    HealthBar,
}

pub const INSTANCED_BATCHES: usize = 37;
/// The neon batches come first; they are the opaque layer.
const NEON_BATCHES: usize = 31;

/// Mesh slots after the 31 neon meshes (whose slot is their `Batch` index).
pub const FLAT_QUAD_MESH: usize = 31;
pub const RANGE_QUAD_MESH: usize = 32;
pub const RIBBON_QUAD_MESH: usize = 33;
pub const MESH_COUNT: usize = 34;

enum Capacity {
    Fixed(usize),
    Ribbons,
}

struct BatchSpec {
    name: &'static str,
    mesh: usize,
    pipeline: ScenePipeline,
    mode: f32,
    capacity: Capacity,
}

const fn neon(name: &'static str, batch: Batch, capacity: usize, mode: f32) -> BatchSpec {
    BatchSpec { name, mesh: batch as usize, pipeline: ScenePipeline::Neon, mode, capacity: Capacity::Fixed(capacity) }
}

const fn flat(name: &'static str, mesh: usize, pipeline: ScenePipeline, mode: f32, capacity: Capacity) -> BatchSpec {
    BatchSpec { name, mesh, pipeline, mode, capacity }
}

const METAL: f32 = neon_mode::METAL;
const CREATURE: f32 = neon_mode::CREATURE;

const BATCH_SPECS: [BatchSpec; INSTANCED_BATCHES] = [
    neon("tower-base", Batch::TowerBase, ENTITY_CAPACITY, METAL),
    neon("tower-rim", Batch::TowerRim, ENTITY_CAPACITY, METAL),
    neon("upgrade-ring", Batch::UpgradeRing, ENTITY_CAPACITY, METAL),
    neon("level-pip", Batch::Pip, PIP_CAPACITY, METAL),
    neon("gun-head", Batch::GunHead, ENTITY_CAPACITY, METAL),
    neon("gun-barrel", Batch::GunBarrel, ENTITY_CAPACITY, METAL),
    neon("gun-muzzle", Batch::GunMuzzle, ENTITY_CAPACITY, METAL),
    neon("rail", Batch::Rail, ENTITY_CAPACITY, METAL),
    neon("laser-crystal", Batch::LaserCrystal, ENTITY_CAPACITY, METAL),
    neon("laser-cradle", Batch::LaserCradle, ENTITY_CAPACITY, METAL),
    neon("missile-launcher", Batch::MissileLauncher, ENTITY_CAPACITY, METAL),
    neon("missile", Batch::Missile, ENTITY_CAPACITY, METAL),
    neon("slow-core", Batch::SlowCore, ENTITY_CAPACITY, METAL),
    neon("orb-node", Batch::OrbNode, ENTITY_CAPACITY, METAL),
    neon("drone-pad", Batch::DronePad, ENTITY_CAPACITY, METAL),
    neon("tesla-coil", Batch::TeslaCoil, ENTITY_CAPACITY, METAL),
    neon("drone-body", Batch::DroneBody, ENTITY_CAPACITY, METAL),
    neon("packman-jaw", Batch::PackmanJaw, ENTITY_CAPACITY, CREATURE),
    neon("square-body", Batch::SquareBody, ENTITY_CAPACITY, CREATURE),
    neon("triangle-body", Batch::TriangleBody, ENTITY_CAPACITY, CREATURE),
    neon("tank-hull", Batch::TankHull, ENTITY_CAPACITY, CREATURE),
    neon("tank-turret", Batch::TankTurret, ENTITY_CAPACITY, CREATURE),
    neon("runner-body", Batch::RunnerBody, ENTITY_CAPACITY, CREATURE),
    neon("splitter-body", Batch::SplitterBody, ENTITY_CAPACITY, CREATURE),
    neon("berserker-body", Batch::BerserkerBody, ENTITY_CAPACITY, CREATURE),
    neon("berserker-spikes", Batch::BerserkerSpikes, ENTITY_CAPACITY, CREATURE),
    neon("bulwark-shell", Batch::BulwarkShell, ENTITY_CAPACITY, CREATURE),
    neon("bulwark-core", Batch::BulwarkCore, ENTITY_CAPACITY, CREATURE),
    neon("shard", Batch::Shard, SHARD_CAPACITY, CREATURE),
    neon("portal", Batch::Portal, ENTITY_CAPACITY, METAL),
    neon("spawn-gate", Batch::SpawnGate, ENTITY_CAPACITY, METAL),
    flat("decal", FLAT_QUAD_MESH, ScenePipeline::Effect, effect_mode::DECAL, Capacity::Fixed(DECAL_CAPACITY)),
    flat("range", RANGE_QUAD_MESH, ScenePipeline::Effect, effect_mode::RANGE, Capacity::Fixed(4)),
    flat(
        "ground-glow",
        RANGE_QUAD_MESH,
        ScenePipeline::Effect,
        effect_mode::GROUND_GLOW,
        Capacity::Fixed(ENTITY_CAPACITY),
    ),
    flat(
        "road-chevron",
        FLAT_QUAD_MESH,
        ScenePipeline::Effect,
        effect_mode::CHEVRON,
        Capacity::Fixed(ROAD_CHEVRON_CAPACITY),
    ),
    flat("ribbon", RIBBON_QUAD_MESH, ScenePipeline::Effect, effect_mode::RIBBON, Capacity::Ribbons),
    flat(
        "health-bar",
        FLAT_QUAD_MESH,
        ScenePipeline::HealthBar,
        effect_mode::HEALTH_BAR,
        Capacity::Fixed(ENTITY_CAPACITY * 2),
    ),
];

/// The mesh slot a batch draws (several flat batches share one quad).
pub fn batch_mesh(index: usize) -> usize {
    BATCH_SPECS[index].mesh
}

/// Builds all procedural geometry on the CPU (no GPU objects), indexed by mesh slot, so it
/// can run while the GPU compiles pipelines.
pub fn build_batch_meshes(road_width: f32) -> Vec<Mesh> {
    let parts = [
        create_tower_base(),
        create_tower_rim(),
        create_upgrade_ring(),
        create_level_pip(),
        create_gun_head(),
        create_gun_barrel(),
        create_gun_muzzle(),
        create_rail(),
        create_laser_crystal(),
        create_laser_cradle(),
        create_missile_launcher(),
        create_missile(),
        create_slow_core(),
        create_orb_node(),
        create_drone_pad(),
        create_tesla_coil(),
        create_drone_body(),
        create_pack_man_jaw(),
        create_square_body(),
        create_triangle_body(),
        create_tank_hull(),
        create_tank_turret(),
        create_runner_body(),
        create_splitter_body(),
        create_berserker_body(),
        create_berserker_spikes(),
        create_bulwark_shell(),
        create_bulwark_core(),
        create_shard(),
        create_portal(),
        create_spawn_gate(road_width),
    ];
    let mut meshes: Vec<Mesh> = parts.iter().map(to_neon_mesh).collect();
    meshes.extend([create_flat_quad(), create_range_quad(), create_ribbon_quad()]);
    meshes
}

/// A layer in the blended draw order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrawItem {
    Instanced(Batch),
    Smoke,
    Glow,
}

/// Chevrons first: they are road paint, under every other blended layer.
pub const TRANSPARENT_ORDER: [DrawItem; 7] = [
    DrawItem::Instanced(Batch::RoadChevron),
    DrawItem::Instanced(Batch::Ribbon),
    DrawItem::Smoke,
    DrawItem::Glow,
    DrawItem::Instanced(Batch::Decal),
    DrawItem::Instanced(Batch::Range),
    DrawItem::Instanced(Batch::GroundGlow),
];

/// The opaque neon batches, in draw order (all share the neon pipeline).
pub fn opaque_batches() -> std::ops::Range<usize> {
    0..NEON_BATCHES
}

/// Every drawable in the 3D board. Neon batches all share one pipeline, so this list is
/// a draw-call budget rather than a shader budget. Draw order follows the previous
/// three.js renderer: opaque parts, health bars (no depth test), then blended effects.
pub struct RenderBatches {
    instanced: Vec<InstancedBatch>,
    pub glow: SpriteBatch,
    pub smoke: SpriteBatch,
}

impl Index<Batch> for RenderBatches {
    type Output = InstancedBatch;

    fn index(&self, batch: Batch) -> &InstancedBatch {
        &self.instanced[batch as usize]
    }
}

impl IndexMut<Batch> for RenderBatches {
    fn index_mut(&mut self, batch: Batch) -> &mut InstancedBatch {
        &mut self.instanced[batch as usize]
    }
}

impl RenderBatches {
    pub fn new(capacities: BatchCapacities) -> Self {
        let instanced = BATCH_SPECS
            .iter()
            .map(|spec| {
                let capacity = match spec.capacity {
                    Capacity::Fixed(capacity) => capacity,
                    Capacity::Ribbons => capacities.ribbons,
                };
                InstancedBatch::new(spec.name, spec.pipeline, spec.mode, capacity)
            })
            .collect();
        RenderBatches {
            instanced,
            smoke: SpriteBatch::new("smoke", capacities.smoke_sprites, sprite_mode::SMOKE),
            glow: SpriteBatch::new("glow", capacities.glow_sprites, sprite_mode::GLOW),
        }
    }

    /// All instanced batches, indexed by `Batch as usize`.
    pub fn instanced(&self) -> &[InstancedBatch] {
        &self.instanced
    }

    pub fn begin(&mut self) {
        for batch in &mut self.instanced {
            batch.begin();
        }
        self.glow.begin();
        self.smoke.begin();
    }

    /// Gives every batch one invisible instance so a warm-up frame touches every pipeline.
    pub fn prepare_warmup(&mut self) {
        for batch in &mut self.instanced {
            batch.begin();
            batch.push_yaw(0.0, -1000.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        }
        for batch in [&mut self.glow, &mut self.smoke] {
            batch.begin();
            batch.push(0.0, -1000.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        }
    }

    /// Soft blob shadow for an object `height` above the ground: offset along the key light,
    /// wider and fainter the higher it flies. Replaces shadow maps, which cost a depth pass
    /// every frame and several slow-compiling shader variants at startup.
    #[allow(clippy::too_many_arguments)]
    pub fn push_blob_shadow(
        &mut self,
        x: f32,
        z: f32,
        height: f32,
        radius_x: f32,
        radius_z: f32,
        yaw: f32,
        strength: f32,
    ) {
        let spread = 1.0 + height.max(0.0) / SHADOW_HEIGHT_SPREAD;
        let decal = &mut self[Batch::Decal];
        let slot = decal.push_yaw(
            x + height * SHADOW_OFFSET_X,
            BLOB_SHADOW_Y,
            z + height * SHADOW_OFFSET_Z,
            yaw,
            radius_x * 2.0 * spread,
            1.0,
            radius_z * 2.0 * spread,
            0.0,
            0.0,
            0.0,
        );
        decal.set_extra(slot, 0, strength / spread);
        decal.set_extra(slot, 2, 1.0);
    }

    /// Flat glow on the ground: `shape` 0 is a soft disc, 1 a ring.
    #[allow(clippy::too_many_arguments)]
    pub fn push_ground_glow(
        &mut self,
        x: f32,
        y: f32,
        z: f32,
        radius: f32,
        shape: f32,
        red: f32,
        green: f32,
        blue: f32,
        alpha: f32,
    ) {
        let glow = &mut self[Batch::GroundGlow];
        let slot = glow.push_yaw(x, y, z, 0.0, radius, 1.0, radius, red, green, blue);
        glow.set_extra(slot, 0, alpha);
        glow.set_extra(slot, 1, shape);
    }

    /// Instanced draw calls the next scene pass will issue.
    pub fn draw_calls(&self) -> usize {
        self.instanced.iter().filter(|batch| batch.size() > 0).count()
            + usize::from(self.glow.size() > 0)
            + usize::from(self.smoke.size() > 0)
    }

    pub fn drawn_instances(&self) -> usize {
        self.instanced.iter().map(InstancedBatch::size).sum::<usize>() + self.glow.size() + self.smoke.size()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry_kit::NEON_VERTEX_FLOATS;

    #[test]
    fn batch_specs_follow_the_enum_order() {
        assert_eq!(BATCH_SPECS.len(), Batch::HealthBar as usize + 1);
        let batches = RenderBatches::new(BatchCapacities { glow_sprites: 8, smoke_sprites: 8, ribbons: 8 });
        assert_eq!(batches[Batch::TowerBase].name, "tower-base");
        assert_eq!(batches[Batch::SpawnGate].name, "spawn-gate");
        assert_eq!(batches[Batch::Decal].name, "decal");
        assert_eq!(batches[Batch::HealthBar].name, "health-bar");
        assert_eq!(batches[Batch::Ribbon].capacity, 8);
        assert!(opaque_batches().all(|index| batches.instanced()[index].pipeline == ScenePipeline::Neon));
        assert!(batches.instanced()[NEON_BATCHES..].iter().all(|batch| batch.pipeline != ScenePipeline::Neon));
    }

    #[test]
    fn meshes_are_finite_and_complete() {
        let meshes = build_batch_meshes(21.0);
        assert_eq!(meshes.len(), MESH_COUNT);
        for (slot, mesh) in meshes.iter().enumerate() {
            let floats = if slot < NEON_BATCHES { NEON_VERTEX_FLOATS } else { 5 };
            assert!(mesh.vertex_count > 0 && mesh.vertex_count % 3 == 0, "mesh {slot}");
            assert_eq!(mesh.vertices.len(), mesh.vertex_count as usize * floats, "mesh {slot}");
            assert!(mesh.vertices.iter().all(|value| value.is_finite()), "mesh {slot}");
            if slot < NEON_BATCHES {
                for vertex in mesh.vertices.as_chunks::<NEON_VERTEX_FLOATS>().0 {
                    let length = (vertex[3] * vertex[3] + vertex[4] * vertex[4] + vertex[5] * vertex[5]).sqrt();
                    assert!((length - 1.0).abs() < 1e-3, "mesh {slot} normal length {length}");
                    assert!((0.0..=1.0).contains(&vertex[6]));
                }
            }
        }
        for index in 0..INSTANCED_BATCHES {
            assert!(batch_mesh(index) < MESH_COUNT);
        }
    }

    #[test]
    fn warmup_touches_every_batch_and_helpers_write_extras() {
        let mut batches = RenderBatches::new(BatchCapacities { glow_sprites: 8, smoke_sprites: 8, ribbons: 8 });
        batches.prepare_warmup();
        assert_eq!(batches.draw_calls(), INSTANCED_BATCHES + 2);
        batches.begin();
        assert_eq!(batches.drawn_instances(), 0);
        batches.push_blob_shadow(10.0, 20.0, 35.0, 4.0, 5.0, 0.0, 0.5);
        batches.push_ground_glow(1.0, 2.0, 3.0, 4.0, 1.0, 0.1, 0.2, 0.3, 0.9);
        let decal = batches[Batch::Decal].used_data();
        assert!((decal[12] - (10.0 + 35.0 * SHADOW_OFFSET_X)).abs() < 1e-5);
        assert_eq!(&decal[20..24], &[0.5 / 1.5, 0.0, 1.0, 0.0]);
        assert_eq!(&batches[Batch::GroundGlow].used_data()[20..24], &[0.9, 1.0, 0.0, 0.0]);
    }
}

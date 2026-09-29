import { toNeonMesh, type NeonMesh } from "./geometry-kit";
import type { ScenePipelines } from "./gpu-pipelines";
import { createGeometryBuffer, InstancedBatch, type BatchGeometry } from "./instanced-batch";
import { EffectMode, NeonMode, SpriteMode } from "./shaders";
import {
  createBerserkerBody,
  createBerserkerSpikes,
  createBulwarkCore,
  createBulwarkShell,
  createDroneBody,
  createDronePad,
  createFlatQuad,
  createGunBarrel,
  createGunHead,
  createGunMuzzle,
  createLaserCradle,
  createLaserCrystal,
  createLevelPip,
  createMissile,
  createMissileLauncher,
  createOrbNode,
  createPackManJaw,
  createPortal,
  createRail,
  createRangeQuad,
  createRibbonQuad,
  createRunnerBody,
  createShard,
  createSlowCore,
  createSpawnGate,
  createSplitterBody,
  createSquareBody,
  createTankHull,
  createTankTurret,
  createTeslaCoil,
  createTowerBase,
  createTowerRim,
  createUpgradeRing,
  createTriangleBody,
  type FlatMesh,
} from "./models";
import { SpriteBatch } from "./sprite-batch";

export interface BatchCapacities {
  glowSprites: number;
  smokeSprites: number;
  ribbons: number;
}

const ENTITY_CAPACITY = 256;
const PIP_CAPACITY = 1024;
const SHARD_CAPACITY = 1536;
const DECAL_CAPACITY = 3072;
// One chevron per 30 units of road; the longest route needs about a hundred.
const ROAD_CHEVRON_CAPACITY = 256;
// Blob shadows fall away from the key light (see the renderer's sun placement): per unit height.
const SHADOW_OFFSET_X = 0.39;
const SHADOW_OFFSET_Z = 0.52;
const BLOB_SHADOW_Y = 0.6;
const SHADOW_HEIGHT_SPREAD = 70;

/** Vertex data for every batch, keyed by batch name. */
export type BatchMeshes = ReadonlyMap<string, NeonMesh | FlatMesh>;

/** Builds all procedural geometry on the CPU (no GPU objects), so it can run before the device exists. */
export function buildBatchMeshes(roadWidth: number): BatchMeshes {
  return new Map<string, NeonMesh | FlatMesh>([
    ["tower-base", toNeonMesh(createTowerBase())],
    ["tower-rim", toNeonMesh(createTowerRim())],
    ["upgrade-ring", toNeonMesh(createUpgradeRing())],
    ["level-pip", toNeonMesh(createLevelPip())],
    ["gun-head", toNeonMesh(createGunHead())],
    ["gun-barrel", toNeonMesh(createGunBarrel())],
    ["gun-muzzle", toNeonMesh(createGunMuzzle())],
    ["rail", toNeonMesh(createRail())],
    ["laser-crystal", toNeonMesh(createLaserCrystal())],
    ["laser-cradle", toNeonMesh(createLaserCradle())],
    ["missile-launcher", toNeonMesh(createMissileLauncher())],
    ["missile", toNeonMesh(createMissile())],
    ["slow-core", toNeonMesh(createSlowCore())],
    ["orb-node", toNeonMesh(createOrbNode())],
    ["drone-pad", toNeonMesh(createDronePad())],
    ["tesla-coil", toNeonMesh(createTeslaCoil())],
    ["drone-body", toNeonMesh(createDroneBody())],
    ["packman-jaw", toNeonMesh(createPackManJaw())],
    ["square-body", toNeonMesh(createSquareBody())],
    ["triangle-body", toNeonMesh(createTriangleBody())],
    ["tank-hull", toNeonMesh(createTankHull())],
    ["tank-turret", toNeonMesh(createTankTurret())],
    ["runner-body", toNeonMesh(createRunnerBody())],
    ["splitter-body", toNeonMesh(createSplitterBody())],
    ["berserker-body", toNeonMesh(createBerserkerBody())],
    ["berserker-spikes", toNeonMesh(createBerserkerSpikes())],
    ["bulwark-shell", toNeonMesh(createBulwarkShell())],
    ["bulwark-core", toNeonMesh(createBulwarkCore())],
    ["shard", toNeonMesh(createShard())],
    ["portal", toNeonMesh(createPortal())],
    ["spawn-gate", toNeonMesh(createSpawnGate(roadWidth))],
    ["flat-quad", createFlatQuad()],
    ["range-quad", createRangeQuad()],
    ["ribbon-quad", createRibbonQuad()],
  ]);
}

/**
 * Every drawable in the 3D board. Neon batches all share one pipeline, so this list is
 * a draw-call budget rather than a shader budget. Draw order follows the previous
 * three.js renderer: opaque parts, health bars (no depth test), then blended effects.
 */
export class RenderBatches {
  readonly towerBase: InstancedBatch;
  readonly towerRim: InstancedBatch;
  readonly upgradeRing: InstancedBatch;
  readonly pip: InstancedBatch;
  readonly gunHead: InstancedBatch;
  readonly gunBarrel: InstancedBatch;
  readonly gunMuzzle: InstancedBatch;
  readonly rail: InstancedBatch;
  readonly laserCrystal: InstancedBatch;
  readonly laserCradle: InstancedBatch;
  readonly missileLauncher: InstancedBatch;
  readonly missile: InstancedBatch;
  readonly slowCore: InstancedBatch;
  readonly orbNode: InstancedBatch;
  readonly dronePad: InstancedBatch;
  readonly teslaCoil: InstancedBatch;
  readonly droneBody: InstancedBatch;
  readonly packmanJaw: InstancedBatch;
  readonly squareBody: InstancedBatch;
  readonly triangleBody: InstancedBatch;
  readonly tankHull: InstancedBatch;
  readonly tankTurret: InstancedBatch;
  readonly runnerBody: InstancedBatch;
  readonly splitterBody: InstancedBatch;
  readonly berserkerBody: InstancedBatch;
  readonly berserkerSpikes: InstancedBatch;
  readonly bulwarkShell: InstancedBatch;
  readonly bulwarkCore: InstancedBatch;
  readonly shard: InstancedBatch;
  readonly portal: InstancedBatch;
  readonly spawnGate: InstancedBatch;
  readonly ribbon: InstancedBatch;
  readonly decal: InstancedBatch;
  readonly healthBar: InstancedBatch;
  readonly range: InstancedBatch;
  readonly groundGlow: InstancedBatch;
  readonly roadChevron: InstancedBatch;
  readonly glow: SpriteBatch;
  readonly smoke: SpriteBatch;
  private readonly instanced: InstancedBatch[] = [];
  private readonly sprites: SpriteBatch[] = [];
  private readonly opaque: InstancedBatch[] = [];
  private readonly transparent: (InstancedBatch | SpriteBatch)[];
  private readonly geometries: BatchGeometry[] = [];

  constructor(device: GPUDevice, meshes: BatchMeshes, capacities: BatchCapacities) {
    const geometry = (name: string): BatchGeometry => {
      const mesh = meshes.get(name);
      if (!mesh) {
        throw new Error(`Missing batch mesh: ${name}`);
      }
      return this.trackGeometry(createGeometryBuffer(device, name, mesh.vertices, mesh.vertexCount));
    };
    const neon = (name: string, capacity: number, mode: number): InstancedBatch => {
      const batch = this.addInstanced(new InstancedBatch(device, name, geometry(name), "neon", mode, capacity));
      this.opaque.push(batch);
      return batch;
    };
    const flat = (name: string, geometry: BatchGeometry, pipeline: keyof ScenePipelines, mode: number, capacity: number): InstancedBatch => (
      this.addInstanced(new InstancedBatch(device, name, geometry, pipeline, mode, capacity))
    );

    this.towerBase = neon("tower-base", ENTITY_CAPACITY, NeonMode.Metal);
    this.towerRim = neon("tower-rim", ENTITY_CAPACITY, NeonMode.Metal);
    this.upgradeRing = neon("upgrade-ring", ENTITY_CAPACITY, NeonMode.Metal);
    this.pip = neon("level-pip", PIP_CAPACITY, NeonMode.Metal);
    this.gunHead = neon("gun-head", ENTITY_CAPACITY, NeonMode.Metal);
    this.gunBarrel = neon("gun-barrel", ENTITY_CAPACITY, NeonMode.Metal);
    this.gunMuzzle = neon("gun-muzzle", ENTITY_CAPACITY, NeonMode.Metal);
    this.rail = neon("rail", ENTITY_CAPACITY, NeonMode.Metal);
    this.laserCrystal = neon("laser-crystal", ENTITY_CAPACITY, NeonMode.Metal);
    this.laserCradle = neon("laser-cradle", ENTITY_CAPACITY, NeonMode.Metal);
    this.missileLauncher = neon("missile-launcher", ENTITY_CAPACITY, NeonMode.Metal);
    this.missile = neon("missile", ENTITY_CAPACITY, NeonMode.Metal);
    this.slowCore = neon("slow-core", ENTITY_CAPACITY, NeonMode.Metal);
    this.orbNode = neon("orb-node", ENTITY_CAPACITY, NeonMode.Metal);
    this.dronePad = neon("drone-pad", ENTITY_CAPACITY, NeonMode.Metal);
    this.teslaCoil = neon("tesla-coil", ENTITY_CAPACITY, NeonMode.Metal);
    this.droneBody = neon("drone-body", ENTITY_CAPACITY, NeonMode.Metal);
    this.packmanJaw = neon("packman-jaw", ENTITY_CAPACITY, NeonMode.Creature);
    this.squareBody = neon("square-body", ENTITY_CAPACITY, NeonMode.Creature);
    this.triangleBody = neon("triangle-body", ENTITY_CAPACITY, NeonMode.Creature);
    this.tankHull = neon("tank-hull", ENTITY_CAPACITY, NeonMode.Creature);
    this.tankTurret = neon("tank-turret", ENTITY_CAPACITY, NeonMode.Creature);
    this.runnerBody = neon("runner-body", ENTITY_CAPACITY, NeonMode.Creature);
    this.splitterBody = neon("splitter-body", ENTITY_CAPACITY, NeonMode.Creature);
    this.berserkerBody = neon("berserker-body", ENTITY_CAPACITY, NeonMode.Creature);
    this.berserkerSpikes = neon("berserker-spikes", ENTITY_CAPACITY, NeonMode.Creature);
    this.bulwarkShell = neon("bulwark-shell", ENTITY_CAPACITY, NeonMode.Creature);
    this.bulwarkCore = neon("bulwark-core", ENTITY_CAPACITY, NeonMode.Creature);
    this.shard = neon("shard", SHARD_CAPACITY, NeonMode.Creature);
    this.portal = neon("portal", ENTITY_CAPACITY, NeonMode.Metal);
    this.spawnGate = neon("spawn-gate", ENTITY_CAPACITY, NeonMode.Metal);

    const flatQuad = geometry("flat-quad");
    const rangeQuad = geometry("range-quad");
    this.decal = flat("decal", flatQuad, "effect", EffectMode.Decal, DECAL_CAPACITY);
    this.range = flat("range", rangeQuad, "effect", EffectMode.Range, 4);
    this.groundGlow = flat("ground-glow", rangeQuad, "effect", EffectMode.GroundGlow, ENTITY_CAPACITY);
    this.roadChevron = flat("road-chevron", flatQuad, "effect", EffectMode.Chevron, ROAD_CHEVRON_CAPACITY);
    this.ribbon = flat("ribbon", geometry("ribbon-quad"), "effect", EffectMode.Ribbon, capacities.ribbons);
    this.healthBar = flat("health-bar", flatQuad, "healthBar", EffectMode.HealthBar, ENTITY_CAPACITY * 2);
    this.smoke = this.addSprite(new SpriteBatch(device, "smoke", capacities.smokeSprites, "sprite", SpriteMode.Smoke));
    this.glow = this.addSprite(new SpriteBatch(device, "glow", capacities.glowSprites, "sprite", SpriteMode.Glow));
    // Chevrons first: they are road paint, under every other blended layer.
    this.transparent = [this.roadChevron, this.ribbon, this.smoke, this.glow, this.decal, this.range, this.groundGlow];
  }

  begin(): void {
    for (const batch of this.instanced) {
      batch.begin();
    }
    for (const batch of this.sprites) {
      batch.begin();
    }
  }

  finish(): void {
    for (const batch of this.instanced) {
      batch.finish();
    }
    for (const batch of this.sprites) {
      batch.finish();
    }
  }

  /** Opaque neon parts; the board's ground and road draw between these and the rest. */
  drawOpaque(pass: GPURenderPassEncoder, pipelines: ScenePipelines): void {
    drawInOrder(pass, pipelines, this.opaque);
  }

  /** Health bars ignore depth, then the blended effect layers draw back to front by kind. */
  drawOverlays(pass: GPURenderPassEncoder, pipelines: ScenePipelines): void {
    drawInOrder(pass, pipelines, [this.healthBar]);
    drawInOrder(pass, pipelines, this.transparent);
  }

  /** Gives every batch one invisible instance so a warm-up frame touches every pipeline. */
  prepareWarmup(): void {
    for (const batch of this.instanced) {
      batch.begin();
      batch.pushYaw(0, -1000, 0, 0, 0, 0, 0, 0, 0, 0);
      batch.finish();
    }
    for (const batch of this.sprites) {
      batch.begin();
      batch.push(0, -1000, 0, 0, 0, 0, 0, 0, 0, 0, 0);
      batch.finish();
    }
  }

  /**
   * Soft blob shadow for an object `height` above the ground: offset along the key light,
   * wider and fainter the higher it flies. Replaces shadow maps, which cost a depth pass
   * every frame and several slow-compiling shader variants at startup.
   */
  pushBlobShadow(x: number, z: number, height: number, radiusX: number, radiusZ: number, yaw: number, strength: number): void {
    const spread = 1 + (Math.max(0, height) / SHADOW_HEIGHT_SPREAD);
    const slot = this.decal.pushYaw(
      x + (height * SHADOW_OFFSET_X),
      BLOB_SHADOW_Y,
      z + (height * SHADOW_OFFSET_Z),
      yaw,
      radiusX * 2 * spread,
      1,
      radiusZ * 2 * spread,
      0,
      0,
      0,
    );
    this.decal.setExtra(slot, 0, strength / spread);
    this.decal.setExtra(slot, 2, 1);
  }

  /** Flat glow on the ground: `shape` 0 is a soft disc, 1 a ring. */
  pushGroundGlow(x: number, y: number, z: number, radius: number, shape: number, red: number, green: number, blue: number, alpha: number): void {
    const slot = this.groundGlow.pushYaw(x, y, z, 0, radius, 1, radius, red, green, blue);
    this.groundGlow.setExtra(slot, 0, alpha);
    this.groundGlow.setExtra(slot, 1, shape);
  }

  /** Instanced draw calls the next scene pass will issue. */
  get drawCalls(): number {
    let total = 0;
    for (const batch of this.instanced) {
      total += batch.size > 0 ? 1 : 0;
    }
    for (const batch of this.sprites) {
      total += batch.size > 0 ? 1 : 0;
    }
    return total;
  }

  get drawnInstances(): number {
    let total = 0;
    for (const batch of this.instanced) {
      total += batch.size;
    }
    for (const batch of this.sprites) {
      total += batch.size;
    }
    return total;
  }

  dispose(): void {
    for (const batch of this.instanced) {
      batch.dispose();
    }
    for (const batch of this.sprites) {
      batch.dispose();
    }
    for (const geometry of this.geometries) {
      geometry.buffer.destroy();
    }
  }

  private trackGeometry(geometry: BatchGeometry): BatchGeometry {
    this.geometries.push(geometry);
    return geometry;
  }

  private addInstanced(batch: InstancedBatch): InstancedBatch {
    this.instanced.push(batch);
    return batch;
  }

  private addSprite(batch: SpriteBatch): SpriteBatch {
    this.sprites.push(batch);
    return batch;
  }
}

function drawInOrder(pass: GPURenderPassEncoder, pipelines: ScenePipelines, batches: readonly (InstancedBatch | SpriteBatch)[]): void {
  let bound: keyof ScenePipelines | undefined;
  for (const batch of batches) {
    if (batch.size === 0) {
      continue;
    }
    if (batch.pipeline !== bound) {
      pass.setPipeline(pipelines[batch.pipeline]);
      bound = batch.pipeline;
    }
    batch.draw(pass);
  }
}

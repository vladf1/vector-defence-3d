import type { BoardRenderer } from "../board-renderer";
import type { Game } from "../game-engine";
import { takeGpuDevice } from "../gpu-device";
import type { LevelRuntime } from "../level-runtime";
import { createRouteMotionPath } from "../route-path";
import type { FieldBounds, Point } from "../types";
import { BoardScene } from "./board-scene";
import { createBoardCameraRig, type CameraRig, type InspectView } from "./camera-rig";
import { EffectView } from "./effect-view";
import type { FrameContext } from "./frame-math";
import { FxSystem } from "./fx-system";
import { createPipelines, type GpuPipelines } from "./gpu-pipelines";
import { vec3 } from "./math";
import { MonsterView } from "./monster-view";
import { OverlayView } from "./overlay-view";
import { linearColor } from "./palette";
import { PlacementView } from "./placement-view";
import { PostProcessing } from "./post-processing";
import { createPuffBlobs } from "./puff-blobs";
import { ProjectileView } from "./projectile-view";
import { buildBatchMeshes, RenderBatches, type BatchMeshes } from "./render-batches";
import { ResolutionGovernor, selectRenderQuality, type RenderQuality } from "./render-quality";
import { createShaderSources, FrameLayout, MAX_POINT_LIGHTS, type SceneConstants } from "./shaders";
import { TowerView } from "./tower-view";
import { BufferUsage } from "./gpu-flags";

const MAX_FRAME_DELTA_SECONDS = 0.1;
const ROAD_BORDER_TOTAL = 3;
const BACKGROUND = linearColor("#010403");
const HEMISPHERE_INTENSITY = 0.55;
const SUN_INTENSITY = 2.7;
const RIM_INTENSITY = 0.7;
// Toward the lights from the field center (the three.js scene's sun and rim placements).
const SUN_OFFSET = vec3(-320, 820, -430);
const RIM_OFFSET = vec3(400, 260, 520);

/** Startup work that needs no GPU objects, so it can fill the wait for the device. */
interface CpuResources {
  readonly meshes: BatchMeshes;
}

/** True when `promise` has already settled (checked after one microtask turn). */
async function isSettled(promise: Promise<unknown>): Promise<boolean> {
  let settled = false;
  promise.then(() => { settled = true; }, () => { settled = true; });
  await Promise.resolve();
  return settled;
}

/** Milliseconds spent in each startup phase, for diagnosing slow devices. */
export interface StartupTimings {
  /** Until the GPU device is usable; includes geometryMs when that filled the wait. */
  deviceMs: number;
  /** Procedural meshes (CPU only). */
  geometryMs: number;
  setupMs: number;
  pipelineMs: number;
  warmupFrameMs: number;
  gpuDrainMs: number;
  totalMs: number;
  pipelines: number;
}

function scaledColor(css: string, intensity: number): [number, number, number] {
  const color = linearColor(css);
  return [color.r * intensity, color.g * intensity, color.b * intensity];
}

function direction(x: number, y: number, z: number): [number, number, number] {
  const length = Math.hypot(x, y, z) || 1;
  return [x / length, y / length, z / length];
}

/** Lights, field size, and smoke-puff blobs never change after startup; shaders bake them in. */
function createSceneConstants(fieldWidth: number, fieldHeight: number, roadWidth: number): SceneConstants {
  return {
    fieldWidth,
    fieldHeight,
    roadHalfWidth: (roadWidth + ROAD_BORDER_TOTAL) / 2,
    hemiSky: scaledColor("#4fb39a", HEMISPHERE_INTENSITY),
    hemiGround: scaledColor("#020504", HEMISPHERE_INTENSITY),
    keyColor: scaledColor("#e4fff4", SUN_INTENSITY),
    rimColor: scaledColor("#39d8ff", RIM_INTENSITY),
    keyDirection: direction(SUN_OFFSET.x, SUN_OFFSET.y, SUN_OFFSET.z),
    rimDirection: direction(RIM_OFFSET.x, RIM_OFFSET.y, RIM_OFFSET.z),
    puffBlobs: createPuffBlobs(),
  };
}

/** The board renderer plus the close-up framing used by debug pages and render scripts. */
export interface InspectableBoardRenderer extends BoardRenderer {
  /** Frames the render camera over a field point (no shake); null restores the board view. */
  inspect(view: InspectView | null): void;
  /** Shows or hides the level's road, portal, spawn gate, and motes (the ground stays). */
  setSceneryVisible(visible: boolean): void;
}

/**
 * Raw WebGPU board renderer. It never mutates the simulation: every frame it reads
 * `Game.runtime`, refills instanced batches, and records one command buffer (scene pass,
 * bloom chain, composite). All pipelines are created up front, asynchronously and in parallel.
 */
class WebGpuBoardRenderer implements InspectableBoardRenderer {
  private readonly rig: CameraRig;
  private readonly quality: RenderQuality;
  private readonly governor: ResolutionGovernor;
  private readonly overlay: OverlayView;
  private readonly viewDirection = vec3(0, -1, 0);
  private readonly frameData = new Float32Array(FrameLayout.floats);
  private device!: GPUDevice;
  private context!: GPUCanvasContext;
  private pipelines!: GpuPipelines;
  private frameBuffer!: GPUBuffer;
  private frameBindGroup!: GPUBindGroup;
  private batches!: RenderBatches;
  private board!: BoardScene;
  private fx!: FxSystem;
  private towers!: TowerView;
  private monsters!: MonsterView;
  private projectiles!: ProjectileView;
  private effects!: EffectView;
  private placement!: PlacementView;
  private post!: PostProcessing;
  private ready = false;
  private disposed = false;
  private activeRuntime?: LevelRuntime;
  private lastSimulationSeconds = 0;
  private frameIndex = 0;
  startupTimings: StartupTimings | null = null;
  /** Draw calls recorded by the most recent frame (scene, bloom, and composite). */
  frameDrawCalls = 0;
  // Dev-only `?shaderSalt=N` defeats GPU shader caches so benchmarks can measure first-visit compiles.
  private readonly shaderSalt = import.meta.env.DEV ? Number(new URLSearchParams(window.location.search).get("shaderSalt") ?? 0) : 0;
  private width = 1;
  private height = 1;

  constructor(
    private readonly canvas: HTMLCanvasElement,
    overlayCanvas: HTMLCanvasElement,
    private readonly game: Game,
    private readonly onDeviceLost: () => void,
  ) {
    this.quality = selectRenderQuality(game.profile);
    this.governor = new ResolutionGovernor(this.quality, window.devicePixelRatio || 1);
    this.rig = createBoardCameraRig(game.profile.fieldWidth, game.profile.fieldHeight);
    this.overlay = new OverlayView(overlayCanvas, this.rig, game);
  }

  async initialize(): Promise<void> {
    const startedAt = performance.now();
    const devicePending = takeGpuDevice();
    const { profile } = this.game;
    let geometryMs = 0;
    const buildCpuResources = (): CpuResources => {
      const start = performance.now();
      const resources = { meshes: buildBatchMeshes(profile.roadWidth) };
      geometryMs = performance.now() - start;
      return resources;
    };
    // Pipelines are the long pole on a cold start, so they go first when the device is
    // already here; otherwise the geometry is built while the device is still on its way.
    let cpuResources = await isSettled(devicePending) ? undefined : buildCpuResources();
    const device = await devicePending;
    this.device = device;
    void device.lost.then(() => {
      if (!this.disposed) {
        this.ready = false;
        this.onDeviceLost();
      }
    });
    const context = this.canvas.getContext("webgpu") as GPUCanvasContext | null;
    if (!context) {
      throw new Error("Unable to create a WebGPU canvas context.");
    }
    this.context = context;
    const canvasFormat = navigator.gpu.getPreferredCanvasFormat();
    context.configure({ device, format: canvasFormat, alphaMode: "opaque" });
    const deviceReadyAt = performance.now();

    // Pipelines compile in the GPU process while the CPU builds geometry below.
    const sources = createShaderSources(createSceneConstants(profile.fieldWidth, profile.fieldHeight, profile.roadWidth));
    const pipelinesReady = createPipelines(device, canvasFormat, this.quality.msaaSamples, this.shaderSalt, sources);

    cpuResources ??= buildCpuResources();
    this.frameBuffer = device.createBuffer({ label: "frame", size: this.frameData.byteLength, usage: BufferUsage.UNIFORM | BufferUsage.COPY_DST });
    this.batches = new RenderBatches(device, cpuResources.meshes, {
      glowSprites: this.quality.glowSprites,
      smokeSprites: this.quality.smokeSprites,
      ribbons: this.quality.ribbons,
    });
    this.board = new BoardScene(profile.fieldWidth, profile.fieldHeight, profile.roadWidth, device);
    this.fx = new FxSystem(this.rig, { particles: this.quality.fxParticles, lights: Math.min(MAX_POINT_LIGHTS, this.quality.flashLights) });
    this.towers = new TowerView(this.fx);
    this.monsters = new MonsterView(this.fx);
    this.projectiles = new ProjectileView(this.monsters);
    this.effects = new EffectView(this.fx, this.monsters, this.projectiles);
    this.placement = new PlacementView(this.game, this.towers);
    const setupDoneAt = performance.now();

    this.pipelines = await pipelinesReady;
    if (this.disposed) {
      return;
    }
    const pipelinesReadyAt = performance.now();
    this.frameBindGroup = device.createBindGroup({
      label: "frame",
      layout: this.pipelines.frameLayout,
      entries: [{ binding: 0, resource: { buffer: this.frameBuffer } }],
    });
    this.post = new PostProcessing(device, this.pipelines, context, this.quality.msaaSamples, this.quality.bloomResolution);
    this.resize();

    // Warm-up: one frame with every batch, the road, and the post chain, then a GPU drain.
    this.batches.prepareWarmup();
    this.board.setRoute(createRouteMotionPath([{ x: -400, y: -400 }, { x: -300, y: -400 }], 24, 7));
    this.renderFrame();
    const warmupRenderedAt = performance.now();
    await device.queue.onSubmittedWorkDone();
    const drainedAt = performance.now();
    this.board.setRoute(undefined);
    this.batches.begin();
    this.batches.finish();

    this.startupTimings = {
      deviceMs: deviceReadyAt - startedAt,
      geometryMs,
      setupMs: setupDoneAt - deviceReadyAt,
      pipelineMs: pipelinesReadyAt - setupDoneAt,
      warmupFrameMs: warmupRenderedAt - pipelinesReadyAt,
      gpuDrainMs: drainedAt - warmupRenderedAt,
      totalMs: drainedAt - startedAt,
      pipelines: Object.keys(this.pipelines.scene).length + Object.keys(this.pipelines.post).length,
    };
    this.ready = !this.disposed;
  }

  resize(): void {
    const rect = this.canvas.getBoundingClientRect();
    this.width = Math.max(1, Math.round(rect.width));
    this.height = Math.max(1, Math.round(rect.height));
    const devicePixelRatio = window.devicePixelRatio || 1;
    this.governor.updateViewport(this.width, this.height, devicePixelRatio);
    this.applyPixelRatio();
    this.rig.resize(this.width, this.height);
    this.syncViewDirection();
    this.overlay.resize(this.width, this.height, devicePixelRatio);
  }

  tiltBy(deltaRadians: number): boolean {
    if (!this.rig.tiltBy(deltaRadians)) {
      return false;
    }
    this.syncViewDirection();
    return true;
  }

  zoomAt(factor: number, clientX: number, clientY: number, surfaceRect: DOMRect): boolean {
    return this.rig.zoomAt(factor, clientX, clientY, surfaceRect);
  }

  panBetween(fromClientX: number, fromClientY: number, toClientX: number, toClientY: number, surfaceRect: DOMRect): boolean {
    return this.rig.panBetween(fromClientX, fromClientY, toClientX, toClientY, surfaceRect);
  }

  resetView(): boolean {
    if (!this.rig.resetView()) {
      return false;
    }
    this.syncViewDirection();
    return true;
  }

  private syncViewDirection(): void {
    const forward = this.rig.logicalCamera.forward;
    this.viewDirection.x = forward.x;
    this.viewDirection.y = forward.y;
    this.viewDirection.z = forward.z;
  }

  renderBackgroundLayer(): void {
    // Board geometry follows the runtime's route on the next draw.
  }

  draw(): void {
    if (!this.ready) {
      return;
    }

    const now = performance.now();
    if (this.governor.recordFrame(now, this.game.needsAnimationFrame())) {
      this.applyPixelRatio();
    }

    const runtime = this.game.runtime;
    if (runtime !== this.activeRuntime) {
      this.switchRuntime(runtime);
    }

    const time = this.game.simulationSeconds;
    const deltaSeconds = Math.min(MAX_FRAME_DELTA_SECONDS, Math.max(0, time - this.lastSimulationSeconds));
    this.lastSimulationSeconds = time;
    this.frameIndex += 1;
    const frame: FrameContext = {
      deltaSeconds,
      time,
      frame: this.frameIndex,
      viewDirection: this.viewDirection,
    };

    const batches = this.batches;
    this.board.notifyEscapes(runtime.escapesLeft);
    batches.begin();
    this.board.write(batches, frame);
    this.towers.write(runtime.towers, runtime.selectedTower, batches, frame);
    this.monsters.write(runtime, batches, frame);
    this.projectiles.write(runtime, batches, frame);
    this.effects.write(runtime, batches, frame);
    this.placement.write(batches, frame);
    this.fx.update(deltaSeconds);
    this.fx.write(batches);
    batches.finish();

    this.rig.update(deltaSeconds);
    this.renderFrame();
    this.overlay.draw();
  }

  inspect(view: InspectView | null): void {
    this.rig.inspect(view);
  }

  setSceneryVisible(visible: boolean): void {
    this.board.setSceneryVisible(visible);
  }

  getVisibleFieldBounds(): FieldBounds {
    return this.rig.fieldBounds;
  }

  isPointInUpgradeButton(point: Point): boolean {
    return this.overlay.isPointInUpgradeButton(point);
  }

  isPointInLaserLockButton(point: Point): boolean {
    return this.overlay.isPointInLaserLockButton(point);
  }

  clientToField(clientX: number, clientY: number, surfaceRect: DOMRect): Point | null {
    return this.rig.clientToField(clientX, clientY, surfaceRect);
  }

  dispose(): void {
    if (this.disposed) {
      return;
    }
    this.disposed = true;
    this.ready = false;
    this.batches?.dispose();
    this.board?.dispose();
    this.post?.dispose();
    this.frameBuffer?.destroy();
    this.context?.unconfigure();
    this.device?.destroy();
  }

  private applyPixelRatio(): void {
    const pixelRatio = this.governor.currentPixelRatio;
    this.canvas.width = Math.max(1, Math.floor(this.width * pixelRatio));
    this.canvas.height = Math.max(1, Math.floor(this.height * pixelRatio));
    this.post?.resize(this.canvas.width, this.canvas.height);
  }

  private switchRuntime(runtime: LevelRuntime): void {
    this.activeRuntime = runtime;
    this.lastSimulationSeconds = this.game.simulationSeconds;
    this.towers.reset();
    this.monsters.reset(runtime);
    this.projectiles.reset();
    this.effects.reset();
    this.fx.clear();
    this.board.setRoute(runtime.routePath);
  }

  private renderFrame(): void {
    const data = this.frameData;
    const camera = this.rig.renderCamera;
    data.set(camera.viewProjection, FrameLayout.viewProjection);
    data.set(camera.view, FrameLayout.view);
    data.set(camera.projection, FrameLayout.projection);
    data[FrameLayout.camera] = camera.position.x;
    data[FrameLayout.camera + 1] = camera.position.y;
    data[FrameLayout.camera + 2] = camera.position.z;
    data[FrameLayout.camera + 3] = this.lastSimulationSeconds;
    this.fx.writeLights({ data, positionOffset: FrameLayout.pointPosition, colorOffset: FrameLayout.pointColor, slots: MAX_POINT_LIGHTS });
    this.device.queue.writeBuffer(this.frameBuffer, 0, data);

    const pipelines = this.pipelines.scene;
    const encoder = this.device.createCommandEncoder({ label: "board-frame" });
    this.post.render(encoder, { r: BACKGROUND.r, g: BACKGROUND.g, b: BACKGROUND.b, a: 1 }, (pass) => {
      pass.setBindGroup(0, this.frameBindGroup);
      this.batches.drawOpaque(pass, pipelines);
      this.board.draw(pass, pipelines);
      this.batches.drawOverlays(pass, pipelines);
    });
    this.device.queue.submit([encoder.finish()]);
    this.frameDrawCalls = this.batches.drawCalls + this.board.drawCalls + this.post.drawCalls;
  }

  get pixelRatio(): number {
    return this.governor.currentPixelRatio;
  }
}

export async function createWebGpuBoardRenderer(
  canvas: HTMLCanvasElement,
  overlayCanvas: HTMLCanvasElement,
  game: Game,
  onDeviceLost: () => void,
): Promise<{ renderer: InspectableBoardRenderer; startupTimings: StartupTimings | null }> {
  const renderer = new WebGpuBoardRenderer(canvas, overlayCanvas, game, onDeviceLost);
  try {
    await renderer.initialize();
  } catch (error) {
    renderer.dispose();
    throw error;
  }
  return { renderer, startupTimings: renderer.startupTimings };
}

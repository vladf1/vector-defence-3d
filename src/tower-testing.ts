import { CampaignProgressStore } from "./campaign-progress";
import { MAX_TOWER_LEVEL } from "./constants";
import { DroneProjectile } from "./entities/projectiles/drone-projectile";
import { Drone } from "./entities/projectiles/drone";
import { GunProjectile } from "./entities/projectiles/gun-projectile";
import { Missile } from "./entities/projectiles/missile";
import { createMissileVisual } from "./entities/projectiles/missile-visuals";
import { SquareMonster } from "./entities/monsters/square-monster";
import { DroneTower } from "./entities/towers/drone-tower";
import { GunTower } from "./entities/towers/gun-tower";
import { LaserTower } from "./entities/towers/laser-tower";
import { MissileTower } from "./entities/towers/missile-tower";
import { SlowTower } from "./entities/towers/slow-tower";
import type { Tower, TowerClass } from "./entities/towers/tower";
import { TOWER_CLASSES } from "./entities/towers/tower-registry";
import type { GameAudio } from "./game-audio";
import { Game, createLevels } from "./game-engine";
import { LinearActiveCircleSweepCollisionIndex } from "./game-engine/collision-detection";
import { UpdateResult, type UpdateContext } from "./game-engine/update-context";
import { DESKTOP_GAME_PROFILE } from "./game-profile";
import { BOARD_TILT_RADIANS, type InspectView } from "./render3d/camera-rig";
import { createWebGpuBoardRenderer, type InspectableBoardRenderer } from "./render3d/webgpu-board-renderer";
import { GameState, type Point } from "./types";

/**
 * 3D tower and projectile sheet: every cell stages one subject alone in a fresh level
 * runtime (with the level's scenery hidden), lets the real 3D views settle, frames it with the board camera's close-up
 * `inspect(...)`, and copies the WebGPU frame into the table. The zoom dialog keeps the staged
 * scene and orbits the close-up camera: drag to rotate, scroll or pinch to zoom, double-click
 * to reset.
 */
interface SheetRow {
  readonly label: string;
  /** Field units that fit vertically in the close-up. */
  readonly visibleHeight: number;
  /** Presentation seconds to advance before capturing (lets effects develop). */
  readonly settleSeconds: number;
  stage(game: Game, level: number): void;
  /** Advances the staged simulation objects by one step while settling. */
  advance(game: Game, context: UpdateContext, result: UpdateResult): void;
  /** Field point the close-up centers on once settled. */
  focus(game: Game): Point;
}

const CELL_SIZE = 128;
const STEP_SECONDS = 1 / 60;
// The field center, where the ground grid is lit as in play; the level's scenery is hidden.
const STAGE: Point = { x: DESKTOP_GAME_PROFILE.fieldWidth / 2, y: DESKTOP_GAME_PROFILE.fieldHeight / 2 };
// Shots fly along -22.5 degrees toward a far target, so they are mid-flight when captured.
const SHOT_TARGET: Point = { x: STAGE.x + 400, y: STAGE.y - (400 * Math.tan(Math.PI / 8)) };
const TOWER_VISIBLE_HEIGHT = 58;
const PROJECTILE_VISIBLE_HEIGHT = 48;
const EXPLOSION_VISIBLE_HEIGHT = 150;
const TOWER_SETTLE_SECONDS = 0.1;
// Long enough for a tracer to stretch and a drone shot to drop from cruise altitude.
const SHOT_SETTLE_SECONDS = 0.06;
const EXPLOSION_SETTLE_SECONDS = 0.12;
const ROTATE_RADIANS_PER_PIXEL = 0.01;
const MAX_TILT_RADIANS = 1.45;
const WHEEL_ZOOM_PER_PIXEL = 0.0015;
const MIN_VISIBLE_HEIGHT = 6;
const MAX_VISIBLE_HEIGHT = 600;

const holdStill = (): void => {};
const focusStage = (): Point => STAGE;
const advanceShots = (game: Game, context: UpdateContext, result: UpdateResult): void => {
  for (const projectile of game.runtime.projectiles) {
    projectile.update(context, result);
  }
};
const focusShot = (game: Game): Point => game.runtime.projectiles[0] ?? STAGE;
const advanceParticles = (game: Game, context: UpdateContext): void => {
  for (const particle of game.runtime.particles) {
    particle.update(context);
  }
};

const tableTarget = queryRequiredElement<HTMLTableElement>("#tower-testing");
const statusTarget = queryRequiredElement<HTMLElement>("#tower-sheet-status");
const surfaceTarget = queryRequiredElement<HTMLElement>("#tower-render-surface");
const renderCanvas = queryRequiredElement<HTMLCanvasElement>("#tower-render-canvas");
const overlayCanvas = queryRequiredElement<HTMLCanvasElement>("#tower-render-overlay");
const previewDialogTarget = queryRequiredElement<HTMLDialogElement>("#tower-preview-dialog");
const previewCanvasTarget = queryRequiredElement<HTMLCanvasElement>("#tower-preview-canvas");
const previewLabelTarget = queryRequiredElement<HTMLElement>("#tower-preview-label");
const previewCloseTarget = queryRequiredElement<HTMLButtonElement>("#tower-preview-close");

/** The previous 2D sheet's poses: turrets at -45 degrees, a firing gun, a live laser beam. */
function poseTower(tower: Tower): void {
  if (tower instanceof GunTower) {
    tower.angle = -Math.PI / 4;
    tower.muzzleFlashSeconds = 0.04;
  } else if (tower instanceof LaserTower) {
    tower.angle = -Math.PI / 4;
    tower.beamAlpha = 0.72;
  } else if (tower instanceof MissileTower) {
    tower.angle = -Math.PI / 4;
  } else if (tower instanceof SlowTower) {
    tower.pulse = Math.PI / 2;
  }
}

const rows: SheetRow[] = [
  ...TOWER_CLASSES.map((towerClass) => createTowerRow(towerClass)),
  {
    label: "Projectile",
    visibleHeight: PROJECTILE_VISIBLE_HEIGHT,
    settleSeconds: SHOT_SETTLE_SECONDS,
    stage: (game, level) => {
      game.runtime.projectiles.push(new GunProjectile(STAGE, SHOT_TARGET, level));
    },
    advance: advanceShots,
    focus: focusShot,
  },
  {
    label: "Drone Projectile",
    visibleHeight: PROJECTILE_VISIBLE_HEIGHT,
    settleSeconds: SHOT_SETTLE_SECONDS,
    stage: (game, level) => {
      game.runtime.projectiles.push(new DroneProjectile(STAGE, SHOT_TARGET, level));
    },
    advance: advanceShots,
    focus: focusShot,
  },
  {
    label: "Missile",
    visibleHeight: PROJECTILE_VISIBLE_HEIGHT,
    settleSeconds: TOWER_SETTLE_SECONDS,
    stage: (game, level) => {
      const target = createTarget({ x: STAGE.x + 120, y: STAGE.y });
      game.runtime.missiles.push(new Missile(STAGE, target, level, createMissileVisual(level), -Math.PI / 8));
    },
    advance: holdStill,
    focus: focusStage,
  },
  {
    label: "Missile Explosion",
    visibleHeight: EXPLOSION_VISIBLE_HEIGHT,
    settleSeconds: EXPLOSION_SETTLE_SECONDS,
    advance: advanceParticles,
    focus: focusStage,
    stage: (game, level) => {
      // Detonate a real missile on a target at the stage; its particles drive the 3D blast.
      const target = createTarget(STAGE);
      const missile = new Missile(STAGE, target, level, createMissileVisual(level), 0);
      missile.speedPerSecond = 0;
      const result = new UpdateResult();
      missile.update(createUpdateContext(0, [target]), result);
      game.runtime.particles.push(...result.particles);
    },
  },
];

interface ZoomState {
  readonly row: SheetRow;
  readonly level: number;
  view: InspectView;
  renderQueued: boolean;
}

let zoomed: ZoomState | undefined;

void main();

async function main(): Promise<void> {
  const game = new Game(createLevels(DESKTOP_GAME_PROFILE.mode), { play() {} } as unknown as GameAudio, DESKTOP_GAME_PROFILE, new CampaignProgressStore(undefined));
  setSurfaceSize(CELL_SIZE);
  let renderer: InspectableBoardRenderer;
  try {
    ({ renderer } = await createWebGpuBoardRenderer(renderCanvas, overlayCanvas, game, () => {
      statusTarget.textContent = "The GPU device was lost; reload the page.";
    }));
  } catch (error) {
    statusTarget.textContent = `WebGPU is required for this sheet (${String(error)}).`;
    return;
  }
  game.setRenderer(renderer);
  // Cells stage subjects in a real level runtime; only the subject and the ground should show.
  renderer.setSceneryVisible(false);

  const cells = buildTable(tableTarget, (row, level) => openZoom(game, renderer, row, level));
  const started = performance.now();
  for (const [row, levelCanvases] of cells) {
    levelCanvases.forEach((canvas, level) => capture(game, renderer, row, level, canvas));
    // Let the table paint row by row.
    await new Promise(requestAnimationFrame);
  }
  statusTarget.textContent = `Rendered ${rows.length * (MAX_TOWER_LEVEL + 1)} cells with the WebGPU board renderer in ${Math.round(performance.now() - started)} ms. Click a cell to zoom.`;

  previewCloseTarget.addEventListener("click", () => previewDialogTarget.close());
  previewDialogTarget.addEventListener("click", (event) => {
    if (event.target === previewDialogTarget) {
      previewDialogTarget.close();
    }
  });
  previewDialogTarget.addEventListener("close", () => {
    zoomed = undefined;
    setSurfaceSize(CELL_SIZE);
    renderer.resize();
  });
  window.addEventListener("resize", () => {
    if (previewDialogTarget.open && zoomed) {
      sizeZoom(renderer);
      queueZoomRender(renderer);
    }
  });
  attachOrbitControls(renderer);
}

function createTowerRow(towerClass: TowerClass): SheetRow {
  return {
    label: towerClass.label,
    visibleHeight: TOWER_VISIBLE_HEIGHT,
    settleSeconds: TOWER_SETTLE_SECONDS,
    advance: holdStill,
    focus: focusStage,
    stage: (game, level) => {
      const tower = new towerClass(STAGE.x, STAGE.y);
      for (let upgrade = 0; upgrade < level; upgrade += 1) {
        tower.upgrade();
      }
      poseTower(tower);
      game.runtime.towers.push(tower);
      if (tower instanceof DroneTower) {
        const drone = new Drone(STAGE, level);
        drone.x = STAGE.x + 15;
        drone.y = STAGE.y - 12;
        game.runtime.drones.push(drone);
      }
    },
  };
}

/** Renders one cell into `target` (whose backing size sets the capture resolution). */
function capture(game: Game, renderer: InspectableBoardRenderer, row: SheetRow, level: number, target: HTMLCanvasElement): void {
  renderView(renderer, stageCell(game, renderer, row, level), target);
}

/** Stages one cell in a fresh runtime, lets it settle, and returns its default close-up. */
function stageCell(game: Game, renderer: InspectableBoardRenderer, row: SheetRow, level: number): InspectView {
  // A fresh runtime per cell: the renderer resets its views and effects on the switch.
  game.startLevelByIndex(0);
  game.setState(GameState.Paused);
  game.runtime.monsters.length = 0;
  row.stage(game, level);
  // First frame at the spawn state, so views record where shots and blasts start.
  renderer.draw();
  const steps = Math.round(row.settleSeconds / STEP_SECONDS);
  for (let step = 0; step < steps; step += 1) {
    row.advance(game, createUpdateContext(STEP_SECONDS, []), new UpdateResult());
    game.simulationSeconds += STEP_SECONDS;
    renderer.draw();
  }
  const focus = row.focus(game);
  return { x: focus.x, y: focus.y, visibleHeight: row.visibleHeight, yaw: 0, tilt: BOARD_TILT_RADIANS };
}

/** Draws the staged scene from `view` (the simulation clock is not advanced) into `target`. */
function renderView(renderer: InspectableBoardRenderer, view: InspectView, target: HTMLCanvasElement): void {
  renderer.inspect(view);
  renderer.draw();
  // The WebGPU canvas holds this frame until the task ends, so copy it now.
  const context2d = target.getContext("2d");
  if (!context2d) {
    throw new Error("Could not copy the WebGPU frame.");
  }
  context2d.drawImage(renderCanvas, 0, 0, target.width, target.height);
}

function buildTable(table: HTMLTableElement, onZoom: (row: SheetRow, level: number) => void): Map<SheetRow, HTMLCanvasElement[]> {
  table.replaceChildren();
  const headerRow = table.createTHead().insertRow();
  const rowHeading = document.createElement("th");
  rowHeading.scope = "col";
  rowHeading.textContent = "Render";
  headerRow.append(rowHeading);
  for (let level = 0; level <= MAX_TOWER_LEVEL; level += 1) {
    const levelHeading = document.createElement("th");
    levelHeading.scope = "col";
    levelHeading.textContent = `Level ${level + 1}`;
    headerRow.append(levelHeading);
  }

  const cells = new Map<SheetRow, HTMLCanvasElement[]>();
  const body = table.createTBody();
  const backingScale = Math.min(window.devicePixelRatio || 1, 2);
  for (const row of rows) {
    const tableRow = body.insertRow();
    const rowLabel = document.createElement("th");
    rowLabel.scope = "row";
    rowLabel.textContent = row.label;
    tableRow.append(rowLabel);
    const canvases: HTMLCanvasElement[] = [];
    for (let level = 0; level <= MAX_TOWER_LEVEL; level += 1) {
      const button = document.createElement("button");
      button.type = "button";
      button.className = "preview-button";
      button.setAttribute("aria-label", `Zoom ${row.label}, level ${level + 1}`);
      button.addEventListener("click", () => onZoom(row, level));
      const canvas = document.createElement("canvas");
      canvas.setAttribute("aria-hidden", "true");
      canvas.width = Math.round(CELL_SIZE * backingScale);
      canvas.height = Math.round(CELL_SIZE * backingScale);
      button.append(canvas);
      tableRow.insertCell().append(button);
      canvases.push(canvas);
    }
    cells.set(row, canvases);
  }
  return cells;
}

function openZoom(game: Game, renderer: InspectableBoardRenderer, row: SheetRow, level: number): void {
  previewLabelTarget.textContent = `${row.label} · Level ${level + 1} · drag to rotate, scroll or pinch to zoom`;
  previewCanvasTarget.setAttribute("aria-label", `${row.label}, level ${level + 1}`);
  previewDialogTarget.showModal();
  sizeZoom(renderer);
  // The staged scene stays in place while the dialog is open; only the camera moves.
  zoomed = { row, level, view: stageCell(game, renderer, row, level), renderQueued: false };
  renderView(renderer, zoomed.view, previewCanvasTarget);
}

function sizeZoom(renderer: InspectableBoardRenderer): void {
  const zoomSize = Math.min(previewDialogTarget.clientWidth, previewDialogTarget.clientHeight);
  const backingScale = Math.min(window.devicePixelRatio || 1, 2);
  previewCanvasTarget.width = Math.round(zoomSize * backingScale);
  previewCanvasTarget.height = Math.round(zoomSize * backingScale);
  setSurfaceSize(zoomSize);
  renderer.resize();
}

function queueZoomRender(renderer: InspectableBoardRenderer): void {
  const state = zoomed;
  if (!state || state.renderQueued) {
    return;
  }
  state.renderQueued = true;
  requestAnimationFrame(() => {
    state.renderQueued = false;
    if (zoomed === state) {
      renderView(renderer, state.view, previewCanvasTarget);
    }
  });
}

function updateZoomView(renderer: InspectableBoardRenderer, yawDelta: number, tiltDelta: number, zoomFactor: number): void {
  if (!zoomed) {
    return;
  }
  const view = zoomed.view;
  zoomed.view = {
    ...view,
    yaw: view.yaw + yawDelta,
    tilt: Math.min(MAX_TILT_RADIANS, Math.max(0, view.tilt + tiltDelta)),
    visibleHeight: Math.min(MAX_VISIBLE_HEIGHT, Math.max(MIN_VISIBLE_HEIGHT, view.visibleHeight * zoomFactor)),
  };
  queueZoomRender(renderer);
}

/** Drag rotates (horizontal = heading, vertical = tilt), wheel and two-finger pinch zoom. */
function attachOrbitControls(renderer: InspectableBoardRenderer): void {
  const pointers = new Map<number, Point>();
  let pinchDistance = 0;
  const currentPinchDistance = (): number => {
    const [a, b] = [...pointers.values()];
    return Math.hypot(a.x - b.x, a.y - b.y);
  };
  previewCanvasTarget.addEventListener("pointerdown", (event) => {
    previewCanvasTarget.setPointerCapture(event.pointerId);
    pointers.set(event.pointerId, { x: event.clientX, y: event.clientY });
    if (pointers.size === 2) {
      pinchDistance = currentPinchDistance();
    }
  });
  previewCanvasTarget.addEventListener("pointermove", (event) => {
    const previous = pointers.get(event.pointerId);
    if (!previous) {
      return;
    }
    pointers.set(event.pointerId, { x: event.clientX, y: event.clientY });
    if (pointers.size === 1) {
      updateZoomView(renderer, (event.clientX - previous.x) * ROTATE_RADIANS_PER_PIXEL, (event.clientY - previous.y) * ROTATE_RADIANS_PER_PIXEL, 1);
    } else if (pointers.size === 2) {
      const distance = currentPinchDistance();
      if (pinchDistance > 0 && distance > 0) {
        updateZoomView(renderer, 0, 0, pinchDistance / distance);
      }
      pinchDistance = distance;
    }
  });
  const release = (event: PointerEvent): void => {
    pointers.delete(event.pointerId);
    pinchDistance = pointers.size === 2 ? currentPinchDistance() : 0;
  };
  previewCanvasTarget.addEventListener("pointerup", release);
  previewCanvasTarget.addEventListener("pointercancel", release);
  previewCanvasTarget.addEventListener("wheel", (event) => {
    event.preventDefault();
    updateZoomView(renderer, 0, 0, Math.exp(event.deltaY * WHEEL_ZOOM_PER_PIXEL));
  }, { passive: false });
  previewCanvasTarget.addEventListener("dblclick", () => {
    if (!zoomed) {
      return;
    }
    zoomed.view = { ...zoomed.view, visibleHeight: zoomed.row.visibleHeight, yaw: 0, tilt: BOARD_TILT_RADIANS };
    queueZoomRender(renderer);
  });
}

function setSurfaceSize(size: number): void {
  surfaceTarget.style.width = `${size}px`;
  surfaceTarget.style.height = `${size}px`;
}

function createUpdateContext(deltaSeconds: number, activeMonsters: SquareMonster[]): UpdateContext {
  return {
    deltaSeconds,
    fieldWidth: DESKTOP_GAME_PROFILE.fieldWidth,
    fieldHeight: DESKTOP_GAME_PROFILE.fieldHeight,
    fieldBounds: { minX: -2000, minY: -2000, maxX: 2000, maxY: 2000 },
    activeMonsters,
    monsterCollisionIndex: new LinearActiveCircleSweepCollisionIndex(activeMonsters),
    activeDrones: [],
    droneAssignments: new Map(),
  };
}

function createTarget(point: Point): SquareMonster {
  const target = new SquareMonster([
    { x: point.x, y: point.y, totalDistance: 0 },
    { x: point.x + 1, y: point.y, totalDistance: 1 },
  ], 1);
  target.x = point.x;
  target.y = point.y;
  target.previousX = point.x;
  target.previousY = point.y;
  return target;
}

function queryRequiredElement<T extends Element>(selector: string): T {
  const target = document.querySelector<T>(selector);
  if (!target) {
    throw new Error(`Tower sheet element ${selector} is missing.`);
  }
  return target;
}

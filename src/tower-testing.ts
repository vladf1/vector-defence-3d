import { loadLabsEngine } from "./engine";
import type { TowerLab } from "./generated/engine-labs/engine.js";
import { takeGpuDevice } from "./gpu-device";

/**
 * 3D tower and projectile sheet: every cell stages one subject alone in a fresh level
 * runtime (with the level's scenery hidden), lets the real 3D views settle, frames it with the board camera's close-up
 * `inspect(...)`, and copies the WebGPU frame into the table. The staging lives in the
 * engine's `TowerLab` (`crates/web/src/labs.rs`). The zoom dialog keeps the staged
 * scene and orbits the close-up camera: drag to rotate, scroll or pinch to zoom, double-click
 * to reset.
 */
type Lab = TowerLab;

interface InspectView {
  x: number;
  y: number;
  /** Field units that fit vertically in the close-up. */
  visibleHeight: number;
  yaw: number;
  tilt: number;
}

interface SheetRow {
  readonly index: number;
  readonly label: string;
}

interface Point {
  x: number;
  y: number;
}

const CELL_SIZE = 128;
const ROTATE_RADIANS_PER_PIXEL = 0.01;
const MAX_TILT_RADIANS = 1.45;
const WHEEL_ZOOM_PER_PIXEL = 0.0015;
const MIN_VISIBLE_HEIGHT = 6;
const MAX_VISIBLE_HEIGHT = 600;

const tableTarget = queryRequiredElement<HTMLTableElement>("#tower-testing");
const statusTarget = queryRequiredElement<HTMLElement>("#tower-sheet-status");
const surfaceTarget = queryRequiredElement<HTMLElement>("#tower-render-surface");
const renderCanvas = queryRequiredElement<HTMLCanvasElement>("#tower-render-canvas");
const overlayCanvas = queryRequiredElement<HTMLCanvasElement>("#tower-render-overlay");
const previewDialogTarget = queryRequiredElement<HTMLDialogElement>("#tower-preview-dialog");
const previewCanvasTarget = queryRequiredElement<HTMLCanvasElement>("#tower-preview-canvas");
const previewLabelTarget = queryRequiredElement<HTMLElement>("#tower-preview-label");
const previewCloseTarget = queryRequiredElement<HTMLButtonElement>("#tower-preview-close");

interface ZoomState {
  readonly row: SheetRow;
  readonly level: number;
  view: InspectView;
  renderQueued: boolean;
}

let zoomed: ZoomState | undefined;
let rows: SheetRow[] = [];
let levelCount = 0;
const defaultVisibleHeights = new Map<SheetRow, number>();

void main();

async function main(): Promise<void> {
  setSurfaceSize(CELL_SIZE);
  let lab: Lab;
  try {
    const [engine, device] = await Promise.all([loadLabsEngine(), takeGpuDevice()]);
    void device.lost.then(() => {
      statusTarget.textContent = "The GPU device was lost; reload the page.";
    });
    lab = await engine.TowerLab.create(device, renderCanvas, overlayCanvas);
  } catch (error) {
    statusTarget.textContent = `WebGPU is required for this sheet (${String(error)}).`;
    return;
  }
  rows = lab.rowLabels().map((label, index) => ({ index, label }));
  levelCount = lab.levelCount();

  const cells = buildTable(tableTarget, (row, level) => openZoom(lab, row, level));
  const started = performance.now();
  for (const [row, levelCanvases] of cells) {
    levelCanvases.forEach((canvas, level) => capture(lab, row, level, canvas));
    // Let the table paint row by row.
    await new Promise(requestAnimationFrame);
  }
  statusTarget.textContent = `Rendered ${rows.length * levelCount} cells with the WebGPU board renderer in ${Math.round(performance.now() - started)} ms. Click a cell to zoom.`;

  previewCloseTarget.addEventListener("click", () => previewDialogTarget.close());
  previewDialogTarget.addEventListener("click", (event) => {
    if (event.target === previewDialogTarget) {
      previewDialogTarget.close();
    }
  });
  previewDialogTarget.addEventListener("close", () => {
    zoomed = undefined;
    setSurfaceSize(CELL_SIZE);
    lab.resize();
  });
  window.addEventListener("resize", () => {
    if (previewDialogTarget.open && zoomed) {
      sizeZoom(lab);
      queueZoomRender(lab);
    }
  });
  attachOrbitControls(lab);
}

/** Renders one cell into `target` (whose backing size sets the capture resolution). */
function capture(lab: Lab, row: SheetRow, level: number, target: HTMLCanvasElement): void {
  renderView(lab, stageCell(lab, row, level), target);
}

/** Stages one cell in a fresh runtime, lets it settle, and returns its default close-up. */
function stageCell(lab: Lab, row: SheetRow, level: number): InspectView {
  const [x, y, visibleHeight, yaw, tilt] = lab.stageCell(row.index, level);
  return { x, y, visibleHeight, yaw, tilt };
}

/** Draws the staged scene from `view` (the simulation clock is not advanced) into `target`. */
function renderView(lab: Lab, view: InspectView, target: HTMLCanvasElement): void {
  lab.renderView(view.x, view.y, view.visibleHeight, view.yaw, view.tilt);
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
  for (let level = 0; level < levelCount; level += 1) {
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
    for (let level = 0; level < levelCount; level += 1) {
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

function openZoom(lab: Lab, row: SheetRow, level: number): void {
  previewLabelTarget.textContent = `${row.label} · Level ${level + 1} · drag to rotate, scroll or pinch to zoom`;
  previewCanvasTarget.setAttribute("aria-label", `${row.label}, level ${level + 1}`);
  previewDialogTarget.showModal();
  sizeZoom(lab);
  // The staged scene stays in place while the dialog is open; only the camera moves.
  zoomed = { row, level, view: stageCell(lab, row, level), renderQueued: false };
  defaultVisibleHeights.set(row, zoomed.view.visibleHeight);
  renderView(lab, zoomed.view, previewCanvasTarget);
}

function sizeZoom(lab: Lab): void {
  const zoomSize = Math.min(previewDialogTarget.clientWidth, previewDialogTarget.clientHeight);
  const backingScale = Math.min(window.devicePixelRatio || 1, 2);
  previewCanvasTarget.width = Math.round(zoomSize * backingScale);
  previewCanvasTarget.height = Math.round(zoomSize * backingScale);
  setSurfaceSize(zoomSize);
  lab.resize();
}

function queueZoomRender(lab: Lab): void {
  const state = zoomed;
  if (!state || state.renderQueued) {
    return;
  }
  state.renderQueued = true;
  requestAnimationFrame(() => {
    state.renderQueued = false;
    if (zoomed === state) {
      renderView(lab, state.view, previewCanvasTarget);
    }
  });
}

function updateZoomView(lab: Lab, yawDelta: number, tiltDelta: number, zoomFactor: number): void {
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
  queueZoomRender(lab);
}

/** Drag rotates (horizontal = heading, vertical = tilt), wheel and two-finger pinch zoom. */
function attachOrbitControls(lab: Lab): void {
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
      updateZoomView(lab, (event.clientX - previous.x) * ROTATE_RADIANS_PER_PIXEL, (event.clientY - previous.y) * ROTATE_RADIANS_PER_PIXEL, 1);
    } else if (pointers.size === 2) {
      const distance = currentPinchDistance();
      if (pinchDistance > 0 && distance > 0) {
        updateZoomView(lab, 0, 0, pinchDistance / distance);
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
    updateZoomView(lab, 0, 0, Math.exp(event.deltaY * WHEEL_ZOOM_PER_PIXEL));
  }, { passive: false });
  previewCanvasTarget.addEventListener("dblclick", () => {
    if (!zoomed) {
      return;
    }
    zoomed.view = { ...zoomed.view, visibleHeight: defaultVisibleHeights.get(zoomed.row) ?? zoomed.view.visibleHeight, yaw: 0, tilt: lab.boardTiltRadians() };
    queueZoomRender(lab);
  });
}

function setSurfaceSize(size: number): void {
  surfaceTarget.style.width = `${size}px`;
  surfaceTarget.style.height = `${size}px`;
}

function queryRequiredElement<T extends Element>(selector: string): T {
  const target = document.querySelector<T>(selector);
  if (!target) {
    throw new Error(`Tower sheet element ${selector} is missing.`);
  }
  return target;
}

import type { FieldBounds, Point } from "../types";
import {
  mat4Identity,
  mat4Invert,
  mat4LookAtWorld,
  mat4Multiply,
  mat4Perspective,
  transformPointProjective,
  vec3,
  type Vec3,
} from "./math";

export interface CameraRigOptions {
  fieldWidth: number;
  fieldHeight: number;
  verticalFovDegrees: number;
  /** Tilt from straight down, toward the bottom of the field. */
  pitchRadians: number;
  /** Fraction of the view kept clear around the field. */
  margin: number;
}

const NEAR = 60;
const FAR = 4000;
// Close-ups scale the near plane with distance, so zooming in never clips the subject.
const INSPECT_NEAR_RATIO = 0.1;
const INSPECT_MIN_NEAR = 0.5;
const FIT_ITERATIONS = 24;
const SHAKE_DECAY_PER_SECOND = 1.9;
const MAX_SHAKE_OFFSET = 9;
const SHAKE_FREQUENCY = 31;
// Screen-up is world -Z (field y grows down the screen).
const CAMERA_UP = vec3(0, 0, -1);

/** A perspective camera as plain matrices (column-major, WebGPU clip depth 0..1). */
export class CameraState {
  readonly position = vec3(0, 0, 0);
  readonly world = mat4Identity();
  readonly view = mat4Identity();
  readonly projection = mat4Identity();
  readonly viewProjection = mat4Identity();
  readonly inverseViewProjection = mat4Identity();
  /** Normalized world-space forward vector. */
  readonly forward = vec3(0, -1, 0);

  /** Takes effect with the next `lookAt` or `copyOrientation`. */
  setPerspective(verticalFovRadians: number, aspect: number, near: number, far: number): void {
    mat4Perspective(this.projection, verticalFovRadians, aspect, near, far);
  }

  lookAt(eyeX: number, eyeY: number, eyeZ: number, target: Vec3, up: Vec3): void {
    this.position.x = eyeX;
    this.position.y = eyeY;
    this.position.z = eyeZ;
    mat4LookAtWorld(this.world, this.position, target, up);
    this.update();
  }

  /** Keeps the orientation of `source` from another position. */
  copyOrientation(source: CameraState, eyeX: number, eyeY: number, eyeZ: number): void {
    this.world.set(source.world);
    this.position.x = eyeX;
    this.position.y = eyeY;
    this.position.z = eyeZ;
    this.world[12] = eyeX;
    this.world[13] = eyeY;
    this.world[14] = eyeZ;
    this.update();
  }

  private update(): void {
    mat4Invert(this.view, this.world);
    mat4Multiply(this.viewProjection, this.projection, this.view);
    mat4Invert(this.inverseViewProjection, this.viewProjection);
    this.forward.x = -this.world[8];
    this.forward.y = -this.world[9];
    this.forward.z = -this.world[10];
  }
}

/**
 * A close-up orbit framing: the field point to center, how many field units fit vertically,
 * the heading around it (0 looks from the bottom of the field, as the board does), and the
 * tilt from straight down (the board uses `BOARD_TILT_RADIANS`).
 */
export interface InspectView {
  readonly x: number;
  readonly y: number;
  readonly visibleHeight: number;
  readonly yaw: number;
  readonly tilt: number;
}

const BOARD_FOV_DEGREES = 24;
export const BOARD_TILT_RADIANS = 0.25;
// The range players can tilt the board camera through (desktop), from straight down; the
// field always stays framed.
const MIN_BOARD_TILT_RADIANS = 0;
const MAX_BOARD_TILT_RADIANS = 0.6;
// Player zoom over the fitted framing (1 = the whole field); panning is limited so the view
// never drifts past the field's edges.
const MIN_BOARD_ZOOM = 1;
const MAX_BOARD_ZOOM = 4;
const BOARD_MARGIN = 0.006;

/** The board's camera framing; also used by headless checks for real visible bounds. */
export function createBoardCameraRig(fieldWidth: number, fieldHeight: number): CameraRig {
  return new CameraRig({
    fieldWidth,
    fieldHeight,
    verticalFovDegrees: BOARD_FOV_DEGREES,
    pitchRadians: BOARD_TILT_RADIANS,
    margin: BOARD_MARGIN,
  });
}

/**
 * Owns the logical camera used for picking and projection, plus a render camera that
 * adds screen shake. Field (x, y) maps to world (x, 0, y); screen-up is world -Z.
 */
export class CameraRig {
  readonly logicalCamera = new CameraState();
  readonly renderCamera = new CameraState();
  private readonly verticalFov: number;
  private readonly target = vec3(0, 0, 0);
  private readonly offsetDirection = vec3(0, 1, 0);
  private pitch: number;
  private zoom = MIN_BOARD_ZOOM;
  private panX = 0;
  private panZ = 0;
  private readonly fitTarget = vec3(0, 0, 0);
  private fitDistance = 1000;
  private aspect = 1;
  private distance = 1000;
  private trauma = 0;
  private shakeTime = 0;
  private viewportWidth = 1;
  private viewportHeight = 1;
  private readonly visibleBounds: FieldBounds = { minX: 0, minY: 0, maxX: 0, maxY: 0 };
  private readonly scratch = vec3(0, 0, 0);
  private inspection: InspectView | null = null;

  constructor(private readonly options: CameraRigOptions) {
    this.verticalFov = (options.verticalFovDegrees * Math.PI) / 180;
    this.pitch = options.pitchRadians;
    this.visibleBounds.maxX = options.fieldWidth;
    this.visibleBounds.maxY = options.fieldHeight;
  }

  get fieldBounds(): FieldBounds {
    return this.visibleBounds;
  }

  get tilt(): number {
    return this.pitch;
  }

  get zoomFactor(): number {
    return this.zoom;
  }

  /*
   * Player view controls (desktop). Picking and projection follow them; `fieldBounds` never
   * does, so they change only what the camera shows, not the rules. Each returns whether
   * the view changed.
   */

  /** Tilts the camera by `deltaRadians` within the player range, re-framing the field. */
  tiltBy(deltaRadians: number): boolean {
    const pitch = Math.min(MAX_BOARD_TILT_RADIANS, Math.max(MIN_BOARD_TILT_RADIANS, this.pitch + deltaRadians));
    if (pitch === this.pitch) {
      return false;
    }
    this.pitch = pitch;
    this.fitField(pitch);
    this.applyPlayerView();
    return true;
  }

  /** Zooms by `factor`, keeping the ground point under the client point in place. */
  zoomAt(factor: number, clientX: number, clientY: number, rect: DOMRect): boolean {
    const zoom = Math.min(MAX_BOARD_ZOOM, Math.max(MIN_BOARD_ZOOM, this.zoom * factor));
    if (zoom === this.zoom) {
      return false;
    }
    const before = this.clientToField(clientX, clientY, rect);
    this.zoom = zoom;
    this.applyPlayerView();
    const after = this.clientToField(clientX, clientY, rect);
    if (before && after) {
      this.panBy(before.x - after.x, before.y - after.y);
    }
    return true;
  }

  /** Pans so the ground point under `from` moves under `to` (grab-the-ground dragging). */
  panBetween(fromClientX: number, fromClientY: number, toClientX: number, toClientY: number, rect: DOMRect): boolean {
    const from = this.clientToField(fromClientX, fromClientY, rect);
    const to = this.clientToField(toClientX, toClientY, rect);
    return from !== null && to !== null && this.panBy(from.x - to.x, from.y - to.y);
  }

  /** Restores the default tilt, zoom, and pan. */
  resetView(): boolean {
    if (this.pitch === this.options.pitchRadians && this.zoom === MIN_BOARD_ZOOM && this.panX === 0 && this.panZ === 0) {
      return false;
    }
    this.pitch = this.options.pitchRadians;
    this.zoom = MIN_BOARD_ZOOM;
    this.panX = 0;
    this.panZ = 0;
    this.fitField(this.pitch);
    this.applyPlayerView();
    return true;
  }

  private panBy(deltaX: number, deltaZ: number): boolean {
    const panX = this.panX;
    const panZ = this.panZ;
    this.panX += deltaX;
    this.panZ += deltaZ;
    this.applyPlayerView();
    return this.panX !== panX || this.panZ !== panZ;
  }

  /** Places the logical camera: the fitted framing, zoomed and panned (pan clamped). */
  private applyPlayerView(): void {
    const { fieldWidth, fieldHeight } = this.options;
    const slack = 1 - (1 / this.zoom);
    const maxPanX = (fieldWidth / 2) * slack;
    const maxPanZ = (fieldHeight / 2) * slack;
    this.panX = Math.min(maxPanX, Math.max(-maxPanX, this.panX));
    this.panZ = Math.min(maxPanZ, Math.max(-maxPanZ, this.panZ));
    this.target.x = this.fitTarget.x + this.panX;
    this.target.y = this.fitTarget.y;
    this.target.z = this.fitTarget.z + this.panZ;
    this.distance = this.fitDistance / this.zoom;
    this.placeCamera();
    this.syncRenderCamera(0, 0);
  }

  resize(width: number, height: number): void {
    this.viewportWidth = Math.max(1, width);
    this.viewportHeight = Math.max(1, height);
    this.aspect = this.viewportWidth / this.viewportHeight;
    this.logicalCamera.setPerspective(this.verticalFov, this.aspect, NEAR, FAR);
    this.renderCamera.setPerspective(this.verticalFov, this.aspect, NEAR, FAR);
    // Gameplay bounds (placement, culling) always come from the default framing, so the
    // player's view changes only what the camera shows, never the rules.
    this.fitField(this.options.pitchRadians);
    this.updateVisibleBounds();
    if (this.pitch !== this.options.pitchRadians) {
      this.fitField(this.pitch);
    }
    this.applyPlayerView();
  }

  /**
   * Debug/render-script aid: frames the render camera tightly over a field point at the
   * same pitch, without screen shake. Picking keeps using the logical camera. Pass null to
   * restore.
   */
  inspect(view: InspectView | null): void {
    this.inspection = view;
    this.syncRenderCamera(0, 0);
  }

  /** Adds screen-shake trauma in 0..1; shake strength follows trauma squared. */
  addTrauma(amount: number): void {
    this.trauma = Math.min(1, this.trauma + amount);
  }

  update(deltaSeconds: number): void {
    if (this.trauma <= 0) {
      return;
    }

    this.shakeTime += deltaSeconds;
    this.trauma = Math.max(0, this.trauma - (SHAKE_DECAY_PER_SECOND * deltaSeconds));
    // Shake is in world units, so zoomed-in views scale it back to the same on-screen jolt.
    const strength = (this.trauma * this.trauma * MAX_SHAKE_OFFSET) / this.zoom;
    const t = this.shakeTime * SHAKE_FREQUENCY;
    const offsetX = (Math.sin(t * 1.13) + (Math.sin(t * 2.71) * 0.5)) * strength * 0.66;
    const offsetZ = (Math.cos(t * 0.97) + (Math.sin(t * 3.17) * 0.5)) * strength * 0.66;
    this.syncRenderCamera(offsetX, offsetZ);
  }

  /** Ray-casts a client-space point onto the ground plane. */
  clientToField(clientX: number, clientY: number, rect: DOMRect): Point | null {
    if (rect.width <= 0 || rect.height <= 0) {
      return null;
    }

    const ndcX = (((clientX - rect.left) / rect.width) * 2) - 1;
    const ndcY = 1 - (((clientY - rect.top) / rect.height) * 2);
    return this.ndcToGround(ndcX, ndcY);
  }

  /** Projects a world point into CSS pixels relative to the canvas. */
  projectToViewport(x: number, y: number, z: number, out: Point): Point {
    const projected = transformPointProjective(this.logicalCamera.viewProjection, x, y, z, this.scratch);
    out.x = (projected.x + 1) * 0.5 * this.viewportWidth;
    out.y = (1 - projected.y) * 0.5 * this.viewportHeight;
    return out;
  }

  private ndcToGround(ndcX: number, ndcY: number): Point | null {
    const camera = this.logicalCamera;
    const origin = camera.position;
    const point = transformPointProjective(camera.inverseViewProjection, ndcX, ndcY, 0.5, this.scratch);
    const dx = point.x - origin.x;
    const dy = point.y - origin.y;
    const dz = point.z - origin.z;
    const length = Math.hypot(dx, dy, dz) || 1;
    const directionY = dy / length;
    if (directionY >= -1e-6) {
      return null;
    }
    const distance = -origin.y / directionY;
    return {
      x: origin.x + ((dx / length) * distance),
      y: origin.z + ((dz / length) * distance),
    };
  }

  private placeCamera(): void {
    const { target, offsetDirection, distance } = this;
    this.logicalCamera.lookAt(
      target.x + (offsetDirection.x * distance),
      target.y + (offsetDirection.y * distance),
      target.z + (offsetDirection.z * distance),
      target,
      CAMERA_UP,
    );
  }

  /**
   * Fits the logical camera at `pitch`: finds the distance and look-at target that frame the
   * whole field with the requested margin, re-centering the (trapezoidal) projection
   * vertically, and records them as the base the player's zoom and pan apply to.
   */
  private fitField(pitch: number): void {
    const { fieldWidth, fieldHeight, margin } = this.options;
    this.offsetDirection.y = Math.cos(pitch);
    this.offsetDirection.z = Math.sin(pitch);
    const limit = 1 - margin;
    this.target.x = fieldWidth / 2;
    this.target.y = 0;
    this.target.z = fieldHeight / 2;
    const halfFov = this.verticalFov / 2;
    this.distance = (Math.max(fieldHeight, fieldWidth / this.aspect) / 2) / Math.tan(halfFov);

    const corners = [
      [0, 0],
      [fieldWidth, 0],
      [0, fieldHeight],
      [fieldWidth, fieldHeight],
    ] as const;
    for (let iteration = 0; iteration < FIT_ITERATIONS; iteration += 1) {
      this.placeCamera();
      let minX = Infinity;
      let maxX = -Infinity;
      let minY = Infinity;
      let maxY = -Infinity;
      for (const [x, z] of corners) {
        const projected = transformPointProjective(this.logicalCamera.viewProjection, x, 0, z, this.scratch);
        minX = Math.min(minX, projected.x);
        maxX = Math.max(maxX, projected.x);
        minY = Math.min(minY, projected.y);
        maxY = Math.max(maxY, projected.y);
      }
      const extent = Math.max((maxX - minX) / 2, (maxY - minY) / 2);
      const centerY = (maxY + minY) / 2;
      // Screen-up is -Z; shift the target so the projected field sits centered.
      const worldPerNdc = Math.tan(halfFov) * this.distance;
      this.target.z -= centerY * worldPerNdc * 0.9;
      this.distance *= 1 + ((extent / limit) - 1) * 0.9;
    }
    this.placeCamera();
    this.fitTarget.x = this.target.x;
    this.fitTarget.y = this.target.y;
    this.fitTarget.z = this.target.z;
    this.fitDistance = this.distance;
  }

  private updateVisibleBounds(): void {
    const topLeft = this.ndcToGround(-1, 1);
    const topRight = this.ndcToGround(1, 1);
    const bottomLeft = this.ndcToGround(-1, -1);
    const bottomRight = this.ndcToGround(1, -1);
    if (!topLeft || !topRight || !bottomLeft || !bottomRight) {
      return;
    }
    // Largest axis-aligned rectangle inside the visible trapezoid.
    this.visibleBounds.minX = Math.max(topLeft.x, bottomLeft.x);
    this.visibleBounds.maxX = Math.min(topRight.x, bottomRight.x);
    this.visibleBounds.minY = Math.max(topLeft.y, topRight.y);
    this.visibleBounds.maxY = Math.min(bottomLeft.y, bottomRight.y);
  }

  private syncRenderCamera(offsetX: number, offsetZ: number): void {
    const render = this.renderCamera;
    const inspection = this.inspection;
    if (inspection) {
      const distance = (inspection.visibleHeight / 2) / Math.tan(this.verticalFov / 2);
      const sinTilt = Math.sin(inspection.tilt);
      const cosTilt = Math.cos(inspection.tilt);
      const sinYaw = Math.sin(inspection.yaw);
      const cosYaw = Math.cos(inspection.yaw);
      render.setPerspective(this.verticalFov, this.aspect, Math.min(NEAR, Math.max(INSPECT_MIN_NEAR, distance * INSPECT_NEAR_RATIO)), FAR);
      // The up vector is the orbit direction's tilt derivative, so it never degenerates.
      render.lookAt(
        inspection.x + (sinYaw * sinTilt * distance),
        cosTilt * distance,
        inspection.y + (cosYaw * sinTilt * distance),
        vec3(inspection.x, 0, inspection.y),
        vec3(-sinYaw * cosTilt, sinTilt, -cosYaw * cosTilt),
      );
    } else {
      render.setPerspective(this.verticalFov, this.aspect, NEAR, FAR);
      const logical = this.logicalCamera.position;
      render.copyOrientation(this.logicalCamera, logical.x + offsetX, logical.y, logical.z + offsetZ);
    }
  }
}


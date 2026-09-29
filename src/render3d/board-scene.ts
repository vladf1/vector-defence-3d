import { getPathHeadingAngle, type PathEntry, type RouteMotionPath } from "../route-path";
import type { ScenePipelines } from "./gpu-pipelines";
import { hash01, type FrameContext } from "./frame-math";
import { linearColor } from "./palette";
import type { RenderBatches } from "./render-batches";
import { BufferUsage } from "./gpu-flags";
import { CHEVRON_ARM_SLOPE, CHEVRON_BAND, CHEVRON_SPAN } from "./shaders";

const ROAD_BORDER = 1.5;
const ROAD_BASE_Y = 0.22;
// Later road samples sit microscopically higher so self-crossing routes never z-fight.
const ROAD_LAYER_STEP = 0.00035;
const ROAD_LEAD_IN = 90;
const EXIT_PORTAL_RADIUS = 19;
const PORTAL_COLOR = linearColor("#b0ffe1");
const PORTAL_ALERT = linearColor("#ff6f62");
// Kept well below the monsters and towers so the exit reads as a landmark, not a light.
const PORTAL_INTENSITY = 0.55;
const PORTAL_RISING_RINGS = 2;
const GATE_COLOR = linearColor("#ff8f6a");
const MOTE_COLOR = linearColor("#7dffd4");
const MOTE_COUNT = 70;
// Road chevrons drift toward the exit; each sits just above the road sample it lies on.
const CHEVRON_SPACING = 30;
const CHEVRON_SPEED = 22;
const CHEVRON_LIFT = 0.03;
const CHEVRON_END_MARGIN = 6;
const CHEVRON_COLOR = { r: 0.03 * 0.22, g: 0.2 * 0.22, b: 0.15 * 0.22 };

// Longest authored route samples to ~1k entries; headroom keeps one buffer for all levels.
const ROAD_CAPACITY_ENTRIES = 4096;

const ROAD_VERTEX_FLOATS = 5;
const GROUND_VERTICES = 6;

/**
 * Samples the road centerline at ascending distances without rescanning: the lead-in before
 * the first entry extends the first segment backward, as the road ribbon does.
 */
class RoadCursor {
  x = 0;
  y = 0;
  /** The road ribbon slot under the sample, which sets its layer height. */
  slot = 0;
  /** The entry ending the segment under the sample (a search hint for path lookups). */
  index = 1;
  private entries: readonly PathEntry[] = [];
  private count = 0;

  reset(entries: readonly PathEntry[], count: number): void {
    this.entries = entries;
    this.count = count;
    this.index = 1;
  }

  seek(distance: number): void {
    const entries = this.entries;
    const first = entries[0];
    if (distance < first.totalDistance) {
      this.interpolate(first, entries[1], distance);
      this.slot = 0;
      return;
    }
    while (this.index < this.count - 1 && entries[this.index].totalDistance < distance) {
      this.index += 1;
    }
    this.interpolate(entries[this.index - 1], entries[this.index], Math.min(distance, entries[this.index].totalDistance));
    this.slot = this.index + 1;
  }

  private interpolate(start: PathEntry, stop: PathEntry, distance: number): void {
    const span = stop.totalDistance - start.totalDistance;
    const ratio = span > 0 ? (distance - start.totalDistance) / span : 0;
    this.x = start.x + ((stop.x - start.x) * ratio);
    this.y = start.y + ((stop.y - start.y) * ratio);
  }
}

/**
 * One persistent road ribbon (uv.x = distance, uv.y = -1..1 across) rewritten in place per
 * level: interleaved `position(3) uv(2)` pairs joined by a fixed index buffer.
 */
class RoadRibbon {
  private readonly vertices: Float32Array;
  private readonly vertexBuffer: GPUBuffer;
  private readonly indexBuffer: GPUBuffer;
  private indexCount = 0;

  constructor(private readonly device: GPUDevice) {
    const vertexCapacity = (ROAD_CAPACITY_ENTRIES + 1) * 2;
    this.vertices = new Float32Array(vertexCapacity * ROAD_VERTEX_FLOATS);
    this.vertexBuffer = device.createBuffer({ label: "road", size: this.vertices.byteLength, usage: BufferUsage.VERTEX | BufferUsage.COPY_DST });
    const indices = new Uint32Array(ROAD_CAPACITY_ENTRIES * 6);
    for (let slot = 0; slot < ROAD_CAPACITY_ENTRIES; slot += 1) {
      const a = slot * 2;
      indices.set([a, a + 2, a + 1, a + 1, a + 2, a + 3], slot * 6);
    }
    this.indexBuffer = device.createBuffer({ label: "road-indices", size: indices.byteLength, usage: BufferUsage.INDEX | BufferUsage.COPY_DST });
    device.queue.writeBuffer(this.indexBuffer, 0, indices);
  }

  write(route: RouteMotionPath, width: number): void {
    const entries = route.entries;
    const count = Math.min(entries.length, ROAD_CAPACITY_ENTRIES);
    const halfWidth = width / 2;
    const v = this.vertices;
    const tangentAt = (index: number): { x: number; y: number } => {
      const previous = entries[Math.max(0, index - 1)];
      const next = entries[Math.min(count - 1, index + 1)];
      const dx = next.x - previous.x;
      const dy = next.y - previous.y;
      const length = Math.hypot(dx, dy) || 1;
      return { x: dx / length, y: dy / length };
    };
    const writePair = (slot: number, x: number, y: number, tangentX: number, tangentY: number, distance: number): void => {
      const height = ROAD_BASE_Y + (slot * ROAD_LAYER_STEP);
      const offset = slot * ROAD_VERTEX_FLOATS * 2;
      v[offset] = x - (tangentY * halfWidth);
      v[offset + 1] = height;
      v[offset + 2] = y + (tangentX * halfWidth);
      v[offset + 3] = distance;
      v[offset + 4] = 1;
      v[offset + 5] = x + (tangentY * halfWidth);
      v[offset + 6] = height;
      v[offset + 7] = y - (tangentX * halfWidth);
      v[offset + 8] = distance;
      v[offset + 9] = -1;
    };

    const startTangent = tangentAt(0);
    writePair(0, entries[0].x - (startTangent.x * ROAD_LEAD_IN), entries[0].y - (startTangent.y * ROAD_LEAD_IN), startTangent.x, startTangent.y, -ROAD_LEAD_IN);
    for (let index = 0; index < count; index += 1) {
      const tangent = tangentAt(index);
      writePair(index + 1, entries[index].x, entries[index].y, tangent.x, tangent.y, entries[index].totalDistance);
    }
    this.device.queue.writeBuffer(this.vertexBuffer, 0, v.buffer, 0, (count + 1) * 2 * ROAD_VERTEX_FLOATS * 4);
    this.indexCount = Math.max(0, count) * 6;
  }

  clear(): void {
    this.indexCount = 0;
  }

  draw(pass: GPURenderPassEncoder): void {
    if (this.indexCount === 0) {
      return;
    }
    pass.setVertexBuffer(0, this.vertexBuffer);
    pass.setIndexBuffer(this.indexBuffer, "uint32");
    pass.drawIndexed(this.indexCount);
  }

  dispose(): void {
    this.vertexBuffer.destroy();
    this.indexBuffer.destroy();
  }
}

/** Static battlefield: ground, the route's road, the exit portal, and the spawn gate. */
export class BoardScene {
  private readonly roadRibbon: RoadRibbon;
  private route: RouteMotionPath | undefined;
  private exitAlert = 0;
  private lastEscapesLeft = -1;
  private sceneryVisible = true;
  private readonly chevronCursor = new RoadCursor();

  constructor(
    private readonly fieldWidth: number,
    private readonly fieldHeight: number,
    private readonly roadWidth: number,
    device: GPUDevice,
  ) {
    this.roadRibbon = new RoadRibbon(device);
  }

  get routePath(): RouteMotionPath | undefined {
    return this.route;
  }

  setRoute(route: RouteMotionPath | undefined): void {
    if (route === this.route) {
      return;
    }
    this.route = route;
    this.lastEscapesLeft = -1;
    if (route !== undefined && route.entries.length >= 2) {
      this.roadRibbon.write(route, this.roadWidth + (ROAD_BORDER * 2));
    } else {
      this.roadRibbon.clear();
    }
  }

  notifyEscapes(escapesLeft: number): void {
    if (this.lastEscapesLeft >= 0 && escapesLeft < this.lastEscapesLeft) {
      this.exitAlert = 1;
    }
    this.lastEscapesLeft = escapesLeft;
  }

  /**
   * Road chevrons as rigid quads, so turns cannot bend them. Each is centered on the road and
   * takes the route's analytic heading at its center, the same smooth heading monsters steer
   * by, so it turns through a curve instead of snapping between path segments.
   */
  private writeChevrons(route: RouteMotionPath, batches: RenderBatches, frame: FrameContext): void {
    const entries = route.entries;
    const count = Math.min(entries.length, ROAD_CAPACITY_ENTRIES);
    const halfWidth = (this.roadWidth / 2) + ROAD_BORDER;
    const back = halfWidth * CHEVRON_SPAN * CHEVRON_ARM_SLOPE;
    const length = CHEVRON_BAND + back;
    const width = halfWidth * CHEVRON_SPAN * 2;
    // The quad's center sits this far ahead of the chevron's apex (see the effect shader).
    const centerAhead = (length / 2) - back;
    const end = entries[count - 1].totalDistance - CHEVRON_END_MARGIN;
    const offset = (frame.time * CHEVRON_SPEED) % CHEVRON_SPACING;
    const firstDistance = entries[0].totalDistance;
    const leadHeading = getPathHeadingAngle(entries, firstDistance, 1);
    const cursor = this.chevronCursor;
    cursor.reset(entries, count);
    for (
      let apex = offset - (Math.floor((offset + ROAD_LEAD_IN) / CHEVRON_SPACING) * CHEVRON_SPACING);
      apex <= end;
      apex += CHEVRON_SPACING
    ) {
      const center = apex + centerAhead;
      cursor.seek(center);
      const heading = center < firstDistance ? leadHeading : getPathHeadingAngle(entries, center, cursor.index);
      batches.roadChevron.pushYaw(
        cursor.x,
        ROAD_BASE_Y + (cursor.slot * ROAD_LAYER_STEP) + CHEVRON_LIFT,
        cursor.y,
        -heading,
        length,
        1,
        width,
        CHEVRON_COLOR.r,
        CHEVRON_COLOR.g,
        CHEVRON_COLOR.b,
      );
    }
  }

  /** Stateless drifting energy motes: positions are pure functions of time and index. */
  private writeMotes(batches: RenderBatches, frame: FrameContext): void {
    const { fieldWidth, fieldHeight } = this;
    for (let index = 0; index < MOTE_COUNT; index += 1) {
      const seedX = hash01(index * 3.1);
      const seedY = hash01((index * 7.7) + 1);
      const speed = 3 + (hash01(index * 5.3) * 6);
      const x = (((seedX * fieldWidth) + (frame.time * speed)) % (fieldWidth + 80)) - 40;
      const y = (seedY * fieldHeight) + (Math.sin((frame.time * 0.4) + (index * 1.7)) * 14);
      const height = 12 + (hash01(index * 2.3) * 40) + (Math.sin((frame.time * 0.7) + index) * 5);
      const twinkle = 0.35 + (Math.sin((frame.time * (1.3 + seedX)) + (index * 2.1)) * 0.35);
      const size = 1.6 + (hash01(index * 9.1) * 2.2);
      batches.glow.push(x, height, y, 0, size, size, 0, MOTE_COLOR.r, MOTE_COLOR.g, MOTE_COLOR.b, Math.max(0, twinkle) * 0.55);
    }
  }

  /** Hides the road, exit portal, spawn gate, and ambient motes (the ground stays). */
  setSceneryVisible(visible: boolean): void {
    this.sceneryVisible = visible;
  }

  write(batches: RenderBatches, frame: FrameContext): void {
    if (!this.sceneryVisible) {
      return;
    }
    this.writeMotes(batches, frame);
    const route = this.route;
    if (!route || route.entries.length < 2) {
      return;
    }
    this.writeChevrons(route, batches, frame);
    this.exitAlert = Math.max(0, this.exitAlert - (frame.deltaSeconds * 1.6));
    const exit = route.entries[route.entries.length - 1];
    const pulse = (0.92 + (Math.sin(frame.time * 3.1) * 0.08)) * PORTAL_INTENSITY;
    const alert = this.exitAlert;
    const red = (PORTAL_COLOR.r + ((PORTAL_ALERT.r - PORTAL_COLOR.r) * alert)) * pulse * (1 + alert);
    const green = (PORTAL_COLOR.g + ((PORTAL_ALERT.g - PORTAL_COLOR.g) * alert)) * pulse * (1 + alert);
    const blue = (PORTAL_COLOR.b + ((PORTAL_ALERT.b - PORTAL_COLOR.b) * alert)) * pulse * (1 + alert);
    batches.portal.pushYaw(exit.x, 0.3, exit.y, frame.time * 0.35, EXIT_PORTAL_RADIUS, EXIT_PORTAL_RADIUS, EXIT_PORTAL_RADIUS, red, green, blue);
    batches.pushGroundGlow(exit.x, 0.9, exit.y, EXIT_PORTAL_RADIUS * 1.5, 0, red * 0.18, green * 0.18, blue * 0.18, 1);
    for (let index = 0; index < PORTAL_RISING_RINGS; index += 1) {
      const phase = ((frame.time * 0.4) + (index / PORTAL_RISING_RINGS)) % 1;
      const radius = EXIT_PORTAL_RADIUS * (1 - phase);
      batches.glow.push(exit.x, 1 + (phase * 10), exit.y, 0, radius * 1.4, radius * 1.4, 1, red * 0.5, green * 0.5, blue * 0.5, phase * (1 - phase) * 1.6);
    }

    const start = route.entries[0];
    const next = route.entries[Math.min(route.entries.length - 1, 3)];
    const angle = Math.atan2(next.y - start.y, next.x - start.x);
    const gateGlow = 0.8 + (Math.sin(frame.time * 4.2) * 0.2);
    batches.spawnGate.pushYaw(start.x, 0, start.y, -angle, 1, 1, 1, GATE_COLOR.r * gateGlow, GATE_COLOR.g * gateGlow, GATE_COLOR.b * gateGlow);
  }

  get drawCalls(): number {
    return this.sceneryVisible && this.route && this.route.entries.length >= 2 ? 2 : 1;
  }

  /** The ground is a shader-generated quad around the field (no vertex buffers). */
  draw(pass: GPURenderPassEncoder, pipelines: ScenePipelines): void {
    pass.setPipeline(pipelines.ground);
    pass.draw(GROUND_VERTICES);
    if (!this.sceneryVisible) {
      return;
    }
    pass.setPipeline(pipelines.road);
    this.roadRibbon.draw(pass);
  }

  dispose(): void {
    this.roadRibbon.dispose();
  }
}

/**
 * Every shader the board uses, generated once at startup with the fixed scene constants
 * (lights, field size, smoke-puff blobs) baked in as WGSL constants.
 *
 * Safari compiles shaders serially, a fragment stage that reads a uniform buffer (~130 ms)
 * or a texture (~250 ms) compiles far slower than math-only code (~5 ms), and every distinct
 * pipeline state (blend state included: Apple GPUs blend in the shader) adds compile work.
 * So: effects share one module and one premultiplied-alpha pipeline, as do sprites (a
 * per-instance mode selects the look, and alpha 0 makes a layer additive), smoke puffs are
 * evaluated procedurally instead of sampled, ground and road receive lighting through flat
 * varyings, and the bloom/composite passes share one module. Only the neon parts (per-pixel
 * point lights) and the post chain read resources from fragment code.
 *
 * Lighting mirrors the previous three.js Lambert setup: albedo / pi times hemisphere +
 * directional + point irradiance, plus emissive.
 */

/** Float offsets inside the Frame uniform buffer (keep in sync with `FRAME`). */
export const FrameLayout = {
  viewProjection: 0,
  view: 16,
  projection: 32,
  camera: 48,
  pointPosition: 52,
  pointColor: 68,
  floats: 84,
} as const;

export const MAX_POINT_LIGHTS = 4;

/**
 * Per-instance look selector of the neon module (written to the instance tint's w): metal
 * parts (towers, scenery) get a white sheen; creature parts (monsters and their debris) take
 * sheen and rim in their own color, like the original's colored outlines.
 */
export const NeonMode = {
  Metal: 0,
  Creature: 1,
} as const;

/** Per-instance look selector of the effect module (written to the instance tint's w). */
export const EffectMode = {
  Ribbon: 0,
  Decal: 1,
  HealthBar: 2,
  Range: 3,
  GroundGlow: 4,
  Chevron: 5,
} as const;

/**
 * Road chevron shape, shared by the effect shader and the board scene's placement: a V band
 * `CHEVRON_BAND` long whose arms sweep back `CHEVRON_ARM_SLOPE` per unit across, out to
 * `CHEVRON_SPAN` of the road half width.
 */
export const CHEVRON_BAND = 6;
export const CHEVRON_ARM_SLOPE = 0.85;
export const CHEVRON_SPAN = 0.66;

/** Per-sprite look selector of the sprite module (written to the sprite shape's w). */
export const SpriteMode = {
  Glow: 0,
  Smoke: 1,
} as const;

/** Pass selector of the post module (written to the pass parameters). */
export const PostMode = {
  Downsample: 0,
  Blur: 1,
  Composite: 2,
} as const;

/** Floats per post-pass parameter block: texel (2), direction (2), threshold, mode, pad (2). */
export const POST_PARAMS_FLOATS = 8;

type Vec3Tuple = readonly [number, number, number];

export interface SceneConstants {
  readonly fieldWidth: number;
  readonly fieldHeight: number;
  readonly roadHalfWidth: number;
  /** Linear RGB already multiplied by the light intensity. */
  readonly hemiSky: Vec3Tuple;
  readonly hemiGround: Vec3Tuple;
  readonly keyColor: Vec3Tuple;
  readonly rimColor: Vec3Tuple;
  /** Normalized, pointing toward the light. */
  readonly keyDirection: Vec3Tuple;
  readonly rimDirection: Vec3Tuple;
  /** Smoke-puff atlas blobs, 4 floats each: cell-local x, y (pixels), radius, alpha. */
  readonly puffBlobs: readonly number[];
}

export interface ShaderSources {
  readonly neon: string;
  readonly ground: string;
  readonly road: string;
  readonly effect: string;
  readonly sprite: string;
  readonly post: string;
}

const float = (value: number): string => {
  const text = Number.isInteger(value) ? value.toFixed(1) : String(Number(value.toPrecision(7)));
  return text.includes("e") ? value.toFixed(9) : text;
};
const vec3 = ([x, y, z]: Vec3Tuple): string => `vec3f(${float(x)}, ${float(y)}, ${float(z)})`;

const COMMON = /* wgsl */ `
const INV_PI = 0.3183098861837907;
const TWO_PI = 6.283185307179586;

// Hermite smoothstep that is well defined for reversed edges.
fn sstep(e0: f32, e1: f32, x: f32) -> f32 {
  let t = clamp((x - e0) / (e1 - e0), 0.0, 1.0);
  return t * t * (3.0 - 2.0 * t);
}
`;

const FRAME = /* wgsl */ `
struct Frame {
  viewProjection: mat4x4f,
  view: mat4x4f,
  projection: mat4x4f,
  camera: vec4f,
  pointPosition: array<vec4f, 4>,
  pointColor: array<vec4f, 4>,
};

@group(0) @binding(0) var<uniform> frame: Frame;
`;

const LIGHTING = /* wgsl */ `
struct PointLights {
  position: array<vec4f, 4>,
  color: array<vec4f, 4>,
};

fn lambert(albedo: vec3f, n: vec3f, worldPosition: vec3f, lights: PointLights) -> vec3f {
  let hemiWeight = n.y * 0.5 + 0.5;
  var irradiance = mix(HEMI_GROUND, HEMI_SKY, hemiWeight);
  irradiance += saturate(dot(n, KEY_DIRECTION)) * KEY_COLOR;
  irradiance += saturate(dot(n, RIM_DIRECTION)) * RIM_COLOR;
  for (var index = 0u; index < 4u; index++) {
    let toLight = lights.position[index].xyz - worldPosition;
    let distance = max(length(toLight), 1e-4);
    let cutoff = lights.position[index].w;
    let decay = lights.color[index].w;
    let windowed = saturate(1.0 - pow(distance / cutoff, 4.0));
    let falloff = (1.0 / max(pow(distance, decay), 0.01)) * windowed * windowed;
    irradiance += saturate(dot(n, toLight / distance)) * lights.color[index].rgb * falloff;
  }
  return albedo * irradiance * INV_PI;
}
`;

// Ground and road fragments light themselves from these flat varyings (the vertex stage
// copies the frame's point lights), so their fragment code reads no buffers.
const LIGHT_VARYINGS = /* wgsl */ `
  @location(8) @interpolate(flat) light0: vec4f,
  @location(9) @interpolate(flat) light1: vec4f,
  @location(10) @interpolate(flat) light2: vec4f,
  @location(11) @interpolate(flat) light3: vec4f,
  @location(12) @interpolate(flat) lightColor0: vec4f,
  @location(13) @interpolate(flat) lightColor1: vec4f,
  @location(14) @interpolate(flat) lightColor2: vec4f,
  @location(15) @interpolate(flat) lightColor3: vec4f,
`;

const COPY_LIGHTS = /* wgsl */ `
  out.light0 = frame.pointPosition[0];
  out.light1 = frame.pointPosition[1];
  out.light2 = frame.pointPosition[2];
  out.light3 = frame.pointPosition[3];
  out.lightColor0 = frame.pointColor[0];
  out.lightColor1 = frame.pointColor[1];
  out.lightColor2 = frame.pointColor[2];
  out.lightColor3 = frame.pointColor[3];
`;

const VARYING_LIGHTS = /* wgsl */ `PointLights(
    array<vec4f, 4>(in.light0, in.light1, in.light2, in.light3),
    array<vec4f, 4>(in.lightColor0, in.lightColor1, in.lightColor2, in.lightColor3),
  )`;

const INSTANCE = /* wgsl */ `
struct InstanceIn {
  @location(3) m0: vec4f,
  @location(4) m1: vec4f,
  @location(5) m2: vec4f,
  @location(6) m3: vec4f,
  @location(7) tint: vec4f,
  @location(8) extra: vec4f,
};

fn instanceMatrix(instance: InstanceIn) -> mat4x4f {
  return mat4x4f(instance.m0, instance.m1, instance.m2, instance.m3);
}

// Instance matrices are rotation x scale, so dividing each column by its squared length is
// the exact inverse transpose for normals.
fn instanceNormal(instance: InstanceIn, n: vec3f) -> vec3f {
  let ax = instance.m0.xyz;
  let ay = instance.m1.xyz;
  let az = instance.m2.xyz;
  return normalize(ax * (n.x / (dot(ax, ax) + 1e-8)) + ay * (n.y / (dot(ay, ay) + 1e-8)) + az * (n.z / (dot(az, az) + 1e-8)));
}
`;

// The former 256 px canvas atlas, evaluated per pixel: soft radial blobs composited with
// source-over, then faded by a radial mask. Coordinates are cell-local canvas pixels
// (0..128, y down); cells 0-3 are the atlas quadrants in reading order.
const PUFF = /* wgsl */ `
fn puffCoverage(cell: u32, pixel: vec2f) -> f32 {
  var alpha = 0.0;
  for (var index = 0u; index < 34u; index++) {
    let blob = PUFF_BLOBS[cell * 34u + index];
    // The blob gradient (a, 0.45a at 0.55, 0 at the rim) is linear in distance.
    let coverage = blob.w * max(0.0, 1.0 - distance(pixel, blob.xy) / blob.z);
    alpha += coverage * (1.0 - alpha);
  }
  let mask = 1.0 - clamp((distance(pixel, vec2f(64.0)) - 23.04) / 38.4, 0.0, 1.0);
  return alpha * mask;
}
`;

const NEON = /* wgsl */ `
struct VertexIn {
  @location(0) position: vec3f,
  @location(1) normal: vec3f,
  @location(2) glow: f32,
};

struct Varying {
  @builtin(position) clip: vec4f,
  @location(0) world: vec3f,
  @location(1) normal: vec3f,
  @location(2) tint: vec3f,
  @location(3) glow: f32,
  @location(4) @interpolate(flat) creature: f32,
};

const KEY_LIGHT = vec3f(-0.33, 0.85, -0.44);

@vertex fn vertexMain(vertex: VertexIn, instance: InstanceIn) -> Varying {
  let world = instanceMatrix(instance) * vec4f(vertex.position, 1.0);
  var out: Varying;
  out.clip = frame.viewProjection * world;
  out.world = world.xyz;
  out.normal = instanceNormal(instance, vertex.normal);
  out.tint = instance.tint.rgb;
  out.glow = vertex.glow;
  out.creature = select(0.0, 1.0, instance.tint.w > 0.5);
  return out;
}

@fragment fn fragmentMain(in: Varying) -> @location(0) vec4f {
  let n = normalize(in.normal);
  let viewDirection = normalize(frame.camera.xyz - in.world);
  let rim = pow(1.0 - saturate(dot(n, viewDirection)), 2.6);
  // Cheap Blinn highlight from the key light keeps dark bodies glossy.
  let highlight = pow(saturate(dot(n, normalize(normalize(KEY_LIGHT) + viewDirection))), 36.0) * (1.0 - in.glow);
  let albedo = in.tint * mix(0.032, 1.0, in.glow);
  // Creatures take their sheen in their own color, so bodies read as the monster's color
  // rather than grey metal.
  let sheen = mix(vec3f(0.06), in.tint * 0.22, in.creature);
  let emissive = in.tint * (in.glow * 1.25 + rim * mix(0.6, 0.9, in.creature)) + sheen * highlight;
  return vec4f(lambert(albedo, n, in.world, PointLights(frame.pointPosition, frame.pointColor)) + emissive, 1.0);
}
`;

const GROUND = /* wgsl */ `
struct Varying {
  @builtin(position) clip: vec4f,
  @location(0) world: vec3f,
${LIGHT_VARYINGS}
};

const GRID_SPACING = 35.0;
const GROUND_HALF_SIZE = 3000.0;

@vertex fn vertexMain(@builtin(vertex_index) index: u32) -> Varying {
  // Counter-clockwise seen from above (+Y), so back-face culling keeps the top side.
  var corners = array<vec2f, 6>(vec2f(-1.0, 1.0), vec2f(1.0, 1.0), vec2f(1.0, -1.0), vec2f(-1.0, 1.0), vec2f(1.0, -1.0), vec2f(-1.0, -1.0));
  let point = corners[index] * GROUND_HALF_SIZE + FIELD * 0.5;
  var out: Varying;
  out.world = vec3f(point.x, 0.0, point.y);
  out.clip = frame.viewProjection * vec4f(out.world, 1.0);
${COPY_LIGHTS}
  return out;
}

fn gridLine(coordinate: vec2f) -> f32 {
  let distance = abs(fract(coordinate - 0.5) - 0.5) / fwidth(coordinate);
  return 1.0 - min(min(distance.x, distance.y), 1.0);
}

@fragment fn fragmentMain(in: Varying) -> @location(0) vec4f {
  let field = in.world.xz;
  let cell = field / GRID_SPACING;
  let minorLine = gridLine(cell);
  let majorLine = gridLine(field / (GRID_SPACING * 5.0));
  let panelNoise = fract(sin(dot(floor(cell), vec2f(12.9898, 78.233))) * 43758.5453);
  let focusDistance = length((field - FIELD * 0.5) / (FIELD * 0.64));
  let focus = 1.0 - sstep(0.62, 1.55, focusDistance);
  let panelShade = panelNoise * 0.28 + 0.86;
  let albedo = vec3f(0.0045, 0.0095, 0.008) * panelShade * (focus * 0.6 + 0.4);
  let emissive = vec3f(0.004, 0.02, 0.016) * (minorLine * 0.75 + majorLine * 0.9) * (focus * 0.8 + 0.2);
  return vec4f(lambert(albedo, vec3f(0.0, 1.0, 0.0), in.world, ${VARYING_LIGHTS}) + emissive, 1.0);
}
`;

const ROAD = /* wgsl */ `
struct VertexIn {
  @location(0) position: vec3f,
  @location(1) uv: vec2f,
};

struct Varying {
  @builtin(position) clip: vec4f,
  @location(0) world: vec3f,
  @location(1) uv: vec2f,
  @location(2) @interpolate(flat) time: f32,
${LIGHT_VARYINGS}
};

@vertex fn vertexMain(vertex: VertexIn) -> Varying {
  var out: Varying;
  out.world = vertex.position;
  out.uv = vertex.uv;
  out.clip = frame.viewProjection * vec4f(vertex.position, 1.0);
  out.time = frame.camera.w;
${COPY_LIGHTS}
  return out;
}

@fragment fn fragmentMain(in: Varying) -> @location(0) vec4f {
  let along = in.uv.x;
  let across = abs(in.uv.y);
  let edge = sstep(0.8, 0.9, across) * (1.0 - sstep(0.95, 1.0, across));
  let channelShade = mix(1.0, 0.5, sstep(0.25, 0.92, across));
  let albedo = vec3f(0.0035, 0.024, 0.02) * channelShade;
  let emissive = vec3f(0.018, 0.15, 0.115) * edge;
  return vec4f(lambert(albedo, vec3f(0.0, 1.0, 0.0), in.world, ${VARYING_LIGHTS}) + emissive, 1.0);
}
`;

/**
 * Flat instanced effects; the instance tint's w selects the look (see `EffectMode`). Output
 * is premultiplied: alpha 0 adds light, alpha a blends normally (decals).
 */
const EFFECT = /* wgsl */ `
struct VertexIn {
  @location(0) position: vec3f,
  @location(1) uv: vec2f,
};

struct Varying {
  @builtin(position) clip: vec4f,
  @location(0) uv: vec2f,
  @location(1) @interpolate(flat) tint: vec4f,
  @location(2) @interpolate(flat) extra: vec4f,
  @location(3) @interpolate(flat) time: f32,
};

@vertex fn vertexMain(vertex: VertexIn, instance: InstanceIn) -> Varying {
  var out: Varying;
  out.clip = frame.viewProjection * (instanceMatrix(instance) * vec4f(vertex.position, 1.0));
  out.uv = vertex.uv;
  out.tint = instance.tint;
  out.extra = instance.extra;
  out.time = frame.camera.w;
  return out;
}

// Additive energy ribbon: soft glow with a hot core; extra.x fades the tail.
fn ribbon(in: Varying) -> vec4f {
  let across = abs(in.uv.y - 0.5) * 2.0;
  let glow = pow(saturate(1.0 - across), 2.2);
  let core = sstep(0.38, 0.0, across);
  let tail = mix(1.0, sstep(0.0, 1.0, in.uv.x), in.extra.x);
  return vec4f(in.tint.rgb * ((glow * 0.55 + core * 1.3) * tail), 0.0);
}

// Ground decals: extra = (alpha, soft box, soft disc, -); otherwise a noisy scorch blotch.
fn decal(in: Varying) -> vec4f {
  let centered = (in.uv - 0.5) * 2.0;
  let radius = length(centered);
  var blotch = 0.0;
  // The box and disc flags are 0 or 1, so the blotch only matters when both are off.
  if (in.extra.y < 1.0 && in.extra.z < 1.0) {
    let noise = puffCoverage(2u, vec2f(in.uv.x, 1.0 - in.uv.y) * 128.0);
    blotch = saturate(sstep(1.0, 0.15, radius) * 0.75 + noise * 1.6 - 0.3) * (1.0 - sstep(0.82, 1.0, radius));
  }
  let box = sstep(1.0, 0.55, abs(centered.x)) * sstep(1.0, 0.5, abs(centered.y));
  let disc = pow(saturate(1.0 - radius), 1.5);
  let mask = mix(mix(blotch, box, in.extra.y), disc, in.extra.z);
  let alpha = in.extra.x * mask;
  return vec4f(in.tint.rgb * alpha, alpha);
}

// Tower range: dashed rotating rim over a faint radial fill.
fn range(in: Varying) -> vec4f {
  let centered = (in.uv - 0.5) * 2.0;
  let radius = length(centered);
  let edge = sstep(0.95, 0.982, radius) * (1.0 - sstep(0.988, 1.0, radius));
  let angle = atan2(centered.y, centered.x);
  let dash = sstep(0.3, 0.5, fract(angle / TWO_PI * 64.0 + in.time * 0.35));
  let inside = 1.0 - step(1.0, radius);
  let fill = inside * (pow(radius, 4.0) * 0.12 + 0.022);
  return vec4f(in.tint.rgb * (edge * mix(0.4, 1.0, dash) + fill), 0.0);
}

// Road chevron in its own quad, turned to the road's heading, so turns cannot bend it. The
// quad runs from the arm tips (uv.x = 0) to just past the nose (uv.x = 1).
fn chevron(in: Varying) -> vec4f {
  let back = ROAD_HALF_WIDTH * ${float(CHEVRON_SPAN)} * ${float(CHEVRON_ARM_SLOPE)};
  let along = in.uv.x * (${float(CHEVRON_BAND)} + back) - back;
  let across = abs(in.uv.y - 0.5) * 2.0 * ROAD_HALF_WIDTH * ${float(CHEVRON_SPAN)};
  let band = along + across * ${float(CHEVRON_ARM_SLOPE)};
  let shape = sstep(0.0, 1.5, band) * (1.0 - sstep(2.7, ${float(CHEVRON_BAND)}, band));
  let span = 1.0 - sstep(0.42, ${float(CHEVRON_SPAN)}, across / ROAD_HALF_WIDTH);
  return vec4f(in.tint.rgb * (shape * span), 0.0);
}

// Flat glow on the ground: extra = (alpha, shape: 0 disc / 1 ring).
fn groundGlow(in: Varying) -> vec4f {
  let distance = length(in.uv - 0.5) * 2.0;
  let disc = pow(saturate(1.0 - distance), 2.0);
  let ring = sstep(0.62, 0.84, distance) * (1.0 - sstep(0.87, 1.0, distance));
  return vec4f(in.tint.rgb * (in.extra.x * mix(disc, ring, in.extra.y)), 0.0);
}

@fragment fn fragmentMain(in: Varying) -> @location(0) vec4f {
  var color: vec4f;
  switch u32(in.tint.w + 0.5) {
    case ${EffectMode.Ribbon}u: { color = ribbon(in); }
    case ${EffectMode.Decal}u: { color = decal(in); }
    case ${EffectMode.HealthBar}u: { color = vec4f(in.tint.rgb, 1.0); }
    case ${EffectMode.Range}u: { color = range(in); }
    case ${EffectMode.Chevron}u: { color = chevron(in); }
    default: { color = groundGlow(in); }
  }
  return color;
}
`;

/**
 * Camera-facing sprites; the shape's w selects the look (see `SpriteMode`). Output is
 * premultiplied: glows add light (alpha 0), smoke blends normally.
 */
const SPRITE = /* wgsl */ `
struct SpriteIn {
  @location(0) transform: vec4f,
  @location(1) shape: vec4f,
  @location(2) color: vec4f,
};

struct Varying {
  @builtin(position) clip: vec4f,
  @location(0) uv: vec2f,
  @location(1) @interpolate(flat) color: vec4f,
  @location(2) @interpolate(flat) shape: vec2f,
};

// Camera-facing quad: the corner offset is applied in view space, rotated by transform.w.
@vertex fn vertexMain(@builtin(vertex_index) index: u32, sprite: SpriteIn) -> Varying {
  var corners = array<vec2f, 6>(vec2f(-0.5, -0.5), vec2f(0.5, -0.5), vec2f(0.5, 0.5), vec2f(-0.5, -0.5), vec2f(0.5, 0.5), vec2f(-0.5, 0.5));
  let corner = corners[index];
  let center = frame.view * vec4f(sprite.transform.xyz, 1.0);
  let scaled = corner * sprite.shape.xy;
  let c = cos(sprite.transform.w);
  let s = sin(sprite.transform.w);
  let rotated = vec2f(scaled.x * c - scaled.y * s, scaled.x * s + scaled.y * c);
  var out: Varying;
  out.clip = frame.projection * vec4f(center.xy + rotated, center.zw);
  out.uv = corner + 0.5;
  out.color = sprite.color;
  out.shape = sprite.shape.zw;
  return out;
}

@fragment fn fragmentMain(in: Varying) -> @location(0) vec4f {
  if (u32(in.shape.y + 0.5) == ${SpriteMode.Smoke}u) {
    // Smoke puff from one atlas quadrant: shape 0/1 are the lower cells, 2/3 the upper.
    let selector = u32(in.shape.x + 0.5);
    let cell = (selector % 2u) + 2u * (1u - selector / 2u);
    let alpha = in.color.a * puffCoverage(cell, vec2f(in.uv.x, 1.0 - in.uv.y) * 128.0);
    return vec4f(in.color.rgb * alpha, alpha);
  }
  // Additive glow: soft dot with a hot core, or a ring when shape = 1.
  let distance = length(in.uv - 0.5) * 2.0;
  let soft = pow(saturate(1.0 - distance), 2.0);
  let core = sstep(0.3, 0.0, distance);
  let ring = sstep(0.66, 0.82, distance) * (1.0 - sstep(0.86, 1.0, distance));
  let intensity = mix(soft * 0.7 + core * 0.9, ring, in.shape.x);
  return vec4f(in.color.rgb * (in.color.a * intensity), 0.0);
}
`;

/**
 * Bloom and composite passes in one module; the pass parameters select the pass.
 * Downsample: 4-tap box (16-texel footprint) with a soft brightness threshold (0 = none).
 * Blur: one direction of a linear-sampled 9-tap Gaussian.
 * Composite: scene + two bloom levels, vignette, ACES filmic tone mapping, sRGB encoding.
 */
const POST = /* wgsl */ `
struct Varying {
  @builtin(position) clip: vec4f,
  @location(0) uv: vec2f,
};

struct Params {
  texel: vec2f,
  direction: vec2f,
  threshold: f32,
  mode: f32,
  unused: vec2f,
};

@group(0) @binding(0) var linearSampler: sampler;
@group(0) @binding(1) var source: texture_2d<f32>;
@group(0) @binding(2) var fineBloom: texture_2d<f32>;
@group(0) @binding(3) var wideBloom: texture_2d<f32>;
@group(0) @binding(4) var<uniform> params: Params;

const BLOOM_STRENGTH = 0.8;
const FINE_BLOOM_WEIGHT = 0.9;
const WIDE_BLOOM_WEIGHT = 1.2;
const VIGNETTE_STRENGTH = 0.4;
const VIVID_HOT_START = 2.0;
const VIVID_HOT_END = 5.0;

@vertex fn vertexMain(@builtin(vertex_index) index: u32) -> Varying {
  var positions = array<vec2f, 3>(vec2f(-1.0, -1.0), vec2f(3.0, -1.0), vec2f(-1.0, 3.0));
  let position = positions[index];
  var out: Varying;
  out.clip = vec4f(position, 0.0, 1.0);
  out.uv = vec2f((position.x + 1.0) * 0.5, (1.0 - position.y) * 0.5);
  return out;
}

fn sampleSource(uv: vec2f) -> vec3f {
  return textureSampleLevel(source, linearSampler, uv, 0.0).rgb;
}

fn downsample(uv: vec2f) -> vec4f {
  let t = params.texel;
  let average = (
    sampleSource(uv + t * vec2f(-1.0, -1.0))
    + sampleSource(uv + t * vec2f(1.0, -1.0))
    + sampleSource(uv + t * vec2f(-1.0, 1.0))
    + sampleSource(uv + t * vec2f(1.0, 1.0))
  ) * 0.25;
  let brightness = max(max(average.r, average.g), average.b);
  return vec4f(average * (max(brightness - params.threshold, 0.0) / max(brightness, 1e-4)), 1.0);
}

fn blur(uv: vec2f) -> vec4f {
  let step = params.texel * params.direction;
  var color = sampleSource(uv) * 0.2270270270;
  let near = step * 1.3846153846;
  let far = step * 3.2307692308;
  color += (sampleSource(uv + near) + sampleSource(uv - near)) * 0.3162162162;
  color += (sampleSource(uv + far) + sampleSource(uv - far)) * 0.0702702703;
  return vec4f(color, 1.0);
}

fn rrtAndOdtFit(color: vec3f) -> vec3f {
  let a = color * (color + 0.0245786) - 0.000090537;
  let b = color * (color * 0.983729 + 0.4329510) + 0.238081;
  return a / b;
}

fn acesFilmic(input: vec3f) -> vec3f {
  let inputMatrix = mat3x3f(0.59719, 0.35458, 0.04823, 0.07600, 0.90834, 0.01566, 0.02840, 0.13383, 0.83777);
  let outputMatrix = mat3x3f(1.60475, -0.53108, -0.07367, -0.10208, 1.10813, -0.00605, -0.00327, -0.07276, 1.07602);
  // Matrices are written row by row (as in the reference ACES fit), so vectors multiply on the left.
  var color = input / 0.6;
  color = color * inputMatrix;
  color = rrtAndOdtFit(color);
  color = color * outputMatrix;
  return saturate(color);
}

// Per-channel ACES bleaches saturated brights toward white, which washed the neon trims out
// to pastel. Tone-map the brightest channel instead (keeping hue and saturation), and hand
// over to ACES only for white-hot values such as explosion cores and hit flashes.
fn toneMap(color: vec3f) -> vec3f {
  let peak = max(max(color.r, color.g), color.b);
  let vivid = color * (acesFilmic(vec3f(peak)).g / max(peak, 1e-4));
  return mix(vivid, acesFilmic(color), sstep(VIVID_HOT_START, VIVID_HOT_END, peak));
}

fn srgbEncode(color: vec3f) -> vec3f {
  let curve = pow(color, vec3f(0.41666)) * 1.055 - 0.055;
  return select(curve, color * 12.92, color <= vec3f(0.0031308));
}

fn composite(in: Varying) -> vec4f {
  let screen = in.clip.xy / vec2f(textureDimensions(source));
  let bloom = textureSampleLevel(fineBloom, linearSampler, in.uv, 0.0).rgb * FINE_BLOOM_WEIGHT
    + textureSampleLevel(wideBloom, linearSampler, in.uv, 0.0).rgb * WIDE_BLOOM_WEIGHT;
  let vignette = 1.0 - sstep(0.38, 0.98, length((screen - 0.5) * vec2f(1.12, 1.0))) * VIGNETTE_STRENGTH;
  let color = (sampleSource(in.uv) + bloom * BLOOM_STRENGTH) * vignette;
  return vec4f(srgbEncode(toneMap(color)), 1.0);
}

@fragment fn fragmentMain(in: Varying) -> @location(0) vec4f {
  let mode = u32(params.mode + 0.5);
  if (mode == ${PostMode.Downsample}u) {
    return downsample(in.uv);
  }
  if (mode == ${PostMode.Blur}u) {
    return blur(in.uv);
  }
  return composite(in);
}
`;

/** Builds every shader module's source with the scene constants baked in. */
export function createShaderSources(constants: SceneConstants): ShaderSources {
  const blobs: string[] = [];
  for (let offset = 0; offset < constants.puffBlobs.length; offset += 4) {
    const [x, y, radius, alpha] = constants.puffBlobs.slice(offset, offset + 4);
    blobs.push(`vec4f(${float(x)}, ${float(y)}, ${float(radius)}, ${float(alpha)})`);
  }
  const roadConstants = /* wgsl */ `
const ROAD_HALF_WIDTH = ${float(constants.roadHalfWidth)};
`;
  const sceneConstants = /* wgsl */ `
const FIELD = vec2f(${float(constants.fieldWidth)}, ${float(constants.fieldHeight)});
${roadConstants}const HEMI_SKY = ${vec3(constants.hemiSky)};
const HEMI_GROUND = ${vec3(constants.hemiGround)};
const KEY_DIRECTION = ${vec3(constants.keyDirection)};
const KEY_COLOR = ${vec3(constants.keyColor)};
const RIM_DIRECTION = ${vec3(constants.rimDirection)};
const RIM_COLOR = ${vec3(constants.rimColor)};
`;
  const puffBlobs = /* wgsl */ `
const PUFF_BLOBS = array<vec4f, ${blobs.length}>(${blobs.join(", ")});
`;
  const lit = COMMON + sceneConstants + FRAME + LIGHTING;
  return {
    neon: lit + INSTANCE + NEON,
    ground: lit + GROUND,
    road: lit + ROAD,
    effect: COMMON + roadConstants + FRAME + INSTANCE + puffBlobs + PUFF + EFFECT,
    sprite: COMMON + FRAME + puffBlobs + PUFF + SPRITE,
    post: COMMON + POST,
  };
}

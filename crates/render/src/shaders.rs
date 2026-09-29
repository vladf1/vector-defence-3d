//! Every shader the board uses, generated once at startup with the fixed scene constants
//! (lights, field size, smoke-puff blobs) baked in as WGSL constants.
//!
//! Safari compiles shaders serially, a fragment stage that reads a uniform buffer (~130 ms)
//! or a texture (~250 ms) compiles far slower than math-only code (~5 ms), and every distinct
//! pipeline state (blend state included: Apple GPUs blend in the shader) adds compile work.
//! So: effects share one module and one premultiplied-alpha pipeline, as do sprites (a
//! per-instance mode selects the look, and alpha 0 makes a layer additive), smoke puffs are
//! evaluated procedurally instead of sampled, ground and road receive lighting through flat
//! varyings, and the bloom/composite passes share one module. Only the neon parts (per-pixel
//! point lights) and the post chain read resources from fragment code.
//!
//! Lighting mirrors the previous three.js Lambert setup: albedo / pi times hemisphere +
//! directional + point irradiance, plus emissive.
//!
//! The WGSL is the TypeScript renderer's text: static pieces are `&'static str`s (joined at
//! compile time with `concat!` where one embeds another) and only the scene constants and
//! blobs are formatted at startup.
use std::borrow::Cow;

/// Float offsets inside the Frame uniform buffer (keep in sync with `FRAME`).
pub mod frame_layout {
    pub const VIEW_PROJECTION: usize = 0;
    pub const VIEW: usize = 16;
    pub const PROJECTION: usize = 32;
    pub const CAMERA: usize = 48;
    pub const POINT_POSITION: usize = 52;
    pub const POINT_COLOR: usize = 68;
    pub const FLOATS: usize = 84;
}

pub const MAX_POINT_LIGHTS: usize = 4;

/// Per-instance look selector of the neon module (written to the instance tint's w): metal
/// parts (towers, scenery) get a white sheen; creature parts (monsters and their debris) take
/// sheen and rim in their own color, like the original's colored outlines.
pub mod neon_mode {
    pub const METAL: f32 = 0.0;
    pub const CREATURE: f32 = 1.0;
}

/// Per-instance look selector of the effect module (written to the instance tint's w).
pub mod effect_mode {
    pub const RIBBON: f32 = 0.0;
    pub const DECAL: f32 = 1.0;
    pub const HEALTH_BAR: f32 = 2.0;
    pub const RANGE: f32 = 3.0;
    pub const GROUND_GLOW: f32 = 4.0;
    pub const CHEVRON: f32 = 5.0;
}

/// Road chevron shape, shared by the effect shader and the board scene's placement: a V band
/// `CHEVRON_BAND` long whose arms sweep back `CHEVRON_ARM_SLOPE` per unit across, out to
/// `CHEVRON_SPAN` of the road half width. (The effect shader spells these values out.)
pub const CHEVRON_BAND: f32 = 6.0;
pub const CHEVRON_ARM_SLOPE: f32 = 0.85;
pub const CHEVRON_SPAN: f32 = 0.66;

/// Per-sprite look selector of the sprite module (written to the sprite shape's w).
pub mod sprite_mode {
    pub const GLOW: f32 = 0.0;
    pub const SMOKE: f32 = 1.0;
}

/// Pass selector of the post module (written to the pass parameters).
pub mod post_mode {
    pub const DOWNSAMPLE: f32 = 0.0;
    pub const BLUR: f32 = 1.0;
    pub const COMPOSITE: f32 = 2.0;
}

/// Floats per post-pass parameter block: texel (2), direction (2), threshold, mode, pad (2).
pub const POST_PARAMS_FLOATS: usize = 8;

pub type Vec3Tuple = [f64; 3];

pub struct SceneConstants {
    pub field_width: f64,
    pub field_height: f64,
    pub road_half_width: f64,
    /// Linear RGB already multiplied by the light intensity.
    pub hemi_sky: Vec3Tuple,
    pub hemi_ground: Vec3Tuple,
    pub key_color: Vec3Tuple,
    pub rim_color: Vec3Tuple,
    /// Normalized, pointing toward the light.
    pub key_direction: Vec3Tuple,
    pub rim_direction: Vec3Tuple,
    /// Smoke-puff atlas blobs, 4 floats each: cell-local x, y (pixels), radius, alpha.
    pub puff_blobs: Vec<f64>,
}

pub struct ShaderSources {
    pub neon: String,
    pub ground: String,
    pub road: String,
    pub effect: String,
    pub sprite: String,
    pub post: String,
}

/// Appends `value` like the original's `float()`: integers keep a `.0`; anything else is
/// rounded to 7 significant digits with trailing zeros dropped (JavaScript's
/// `String(Number(value.toPrecision(7)))`), without pulling in float formatting.
pub fn push_float(out: &mut String, value: f64) {
    if value.fract() == 0.0 && value.abs() < 1e15 {
        push_integer(out, value);
        out.push_str(".0");
        return;
    }
    if value < 0.0 {
        out.push('-');
    }
    let magnitude = value.abs();
    let exponent = magnitude.log10().floor() as i32;
    let mut digits = (magnitude * 10f64.powi(6 - exponent)).round() as u64;
    let mut point = exponent + 1;
    if digits >= 10_000_000 {
        digits /= 10;
        point += 1;
    }
    let mut text = [b'0'; 7];
    for slot in text.iter_mut().rev() {
        *slot = b'0' + (digits % 10) as u8;
        digits /= 10;
    }
    let used = text.iter().rposition(|&digit| digit != b'0').map_or(0, |last| last + 1);
    if point <= 0 {
        out.push_str("0.");
        for _ in point..0 {
            out.push('0');
        }
        out.extend(text[..used].iter().map(|&digit| digit as char));
        return;
    }
    let point = point as usize;
    for (index, &digit) in text.iter().enumerate() {
        if index == point {
            if index >= used {
                return;
            }
            out.push('.');
        }
        if index >= point && index >= used {
            return;
        }
        out.push(digit as char);
    }
    for _ in text.len()..point {
        out.push('0');
    }
}

pub(crate) fn push_integer(out: &mut String, value: f64) {
    if value < 0.0 {
        out.push('-');
    }
    let mut magnitude = value.abs() as u64;
    let mut digits = [0u8; 20];
    let mut count = 0;
    loop {
        digits[count] = b'0' + (magnitude % 10) as u8;
        count += 1;
        magnitude /= 10;
        if magnitude == 0 {
            break;
        }
    }
    out.extend(digits[..count].iter().rev().map(|&digit| digit as char));
}

fn push_vec3(out: &mut String, [x, y, z]: Vec3Tuple) {
    out.push_str("vec3f(");
    push_float(out, x);
    out.push_str(", ");
    push_float(out, y);
    out.push_str(", ");
    push_float(out, z);
    out.push(')');
}

const COMMON: &str = r#"
const INV_PI = 0.3183098861837907;
const TWO_PI = 6.283185307179586;

// Hermite smoothstep that is well defined for reversed edges.
fn sstep(e0: f32, e1: f32, x: f32) -> f32 {
  let t = clamp((x - e0) / (e1 - e0), 0.0, 1.0);
  return t * t * (3.0 - 2.0 * t);
}
"#;

const FRAME: &str = r#"
struct Frame {
  viewProjection: mat4x4f,
  view: mat4x4f,
  projection: mat4x4f,
  camera: vec4f,
  pointPosition: array<vec4f, 4>,
  pointColor: array<vec4f, 4>,
};

@group(0) @binding(0) var<uniform> frame: Frame;
"#;

const LIGHTING: &str = r#"
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
"#;

// Ground and road fragments light themselves from these flat varyings (the vertex stage
// copies the frame's point lights), so their fragment code reads no buffers.
macro_rules! light_varyings {
    () => {
        r#"
  @location(8) @interpolate(flat) light0: vec4f,
  @location(9) @interpolate(flat) light1: vec4f,
  @location(10) @interpolate(flat) light2: vec4f,
  @location(11) @interpolate(flat) light3: vec4f,
  @location(12) @interpolate(flat) lightColor0: vec4f,
  @location(13) @interpolate(flat) lightColor1: vec4f,
  @location(14) @interpolate(flat) lightColor2: vec4f,
  @location(15) @interpolate(flat) lightColor3: vec4f,
"#
    };
}

macro_rules! copy_lights {
    () => {
        r#"
  out.light0 = frame.pointPosition[0];
  out.light1 = frame.pointPosition[1];
  out.light2 = frame.pointPosition[2];
  out.light3 = frame.pointPosition[3];
  out.lightColor0 = frame.pointColor[0];
  out.lightColor1 = frame.pointColor[1];
  out.lightColor2 = frame.pointColor[2];
  out.lightColor3 = frame.pointColor[3];
"#
    };
}

macro_rules! varying_lights {
    () => {
        r#"PointLights(
    array<vec4f, 4>(in.light0, in.light1, in.light2, in.light3),
    array<vec4f, 4>(in.lightColor0, in.lightColor1, in.lightColor2, in.lightColor3),
  )"#
    };
}

const INSTANCE: &str = r#"
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
"#;

// The former 256 px canvas atlas, evaluated per pixel: soft radial blobs composited with
// source-over, then faded by a radial mask. Coordinates are cell-local canvas pixels
// (0..128, y down); cells 0-3 are the atlas quadrants in reading order.
const PUFF: &str = r#"
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
"#;

const NEON: &str = r#"
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
"#;

const GROUND: &str = concat!(
    r#"
struct Varying {
  @builtin(position) clip: vec4f,
  @location(0) world: vec3f,
"#,
    light_varyings!(),
    r#"
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
"#,
    copy_lights!(),
    r#"
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
  return vec4f(lambert(albedo, vec3f(0.0, 1.0, 0.0), in.world, "#,
    varying_lights!(),
    r#") + emissive, 1.0);
}
"#
);

const ROAD: &str = concat!(
    r#"
struct VertexIn {
  @location(0) position: vec3f,
  @location(1) uv: vec2f,
};

struct Varying {
  @builtin(position) clip: vec4f,
  @location(0) world: vec3f,
  @location(1) uv: vec2f,
  @location(2) @interpolate(flat) time: f32,
"#,
    light_varyings!(),
    r#"
};

@vertex fn vertexMain(vertex: VertexIn) -> Varying {
  var out: Varying;
  out.world = vertex.position;
  out.uv = vertex.uv;
  out.clip = frame.viewProjection * vec4f(vertex.position, 1.0);
  out.time = frame.camera.w;
"#,
    copy_lights!(),
    r#"
  return out;
}

@fragment fn fragmentMain(in: Varying) -> @location(0) vec4f {
  let along = in.uv.x;
  let across = abs(in.uv.y);
  let edge = sstep(0.8, 0.9, across) * (1.0 - sstep(0.95, 1.0, across));
  let channelShade = mix(1.0, 0.5, sstep(0.25, 0.92, across));
  let albedo = vec3f(0.0035, 0.024, 0.02) * channelShade;
  let emissive = vec3f(0.018, 0.15, 0.115) * edge;
  return vec4f(lambert(albedo, vec3f(0.0, 1.0, 0.0), in.world, "#,
    varying_lights!(),
    r#") + emissive, 1.0);
}
"#
);

/// Flat instanced effects; the instance tint's w selects the look (see `effect_mode`). Output
/// is premultiplied: alpha 0 adds light, alpha a blends normally (decals). The chevron numbers
/// are `CHEVRON_SPAN` (0.66), `CHEVRON_ARM_SLOPE` (0.85), and `CHEVRON_BAND` (6.0); the case
/// labels are the `effect_mode` values.
const EFFECT: &str = r#"
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
  let back = ROAD_HALF_WIDTH * 0.66 * 0.85;
  let along = in.uv.x * (6.0 + back) - back;
  let across = abs(in.uv.y - 0.5) * 2.0 * ROAD_HALF_WIDTH * 0.66;
  let band = along + across * 0.85;
  let shape = sstep(0.0, 1.5, band) * (1.0 - sstep(2.7, 6.0, band));
  let span = 1.0 - sstep(0.42, 0.66, across / ROAD_HALF_WIDTH);
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
    case 0u: { color = ribbon(in); }
    case 1u: { color = decal(in); }
    case 2u: { color = vec4f(in.tint.rgb, 1.0); }
    case 3u: { color = range(in); }
    case 5u: { color = chevron(in); }
    default: { color = groundGlow(in); }
  }
  return color;
}
"#;

/// Camera-facing sprites; the shape's w selects the look (see `sprite_mode`; smoke is `1u`).
/// Output is premultiplied: glows add light (alpha 0), smoke blends normally.
const SPRITE: &str = r#"
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
  if (u32(in.shape.y + 0.5) == 1u) {
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
"#;

/// Bloom and composite passes in one module; the pass parameters select the pass (the
/// `post_mode` values: downsample `0u`, blur `1u`, otherwise composite).
/// Downsample: 4-tap box (16-texel footprint) with a soft brightness threshold (0 = none).
/// Blur: one direction of a linear-sampled 9-tap Gaussian.
/// Composite: scene + two bloom levels, vignette, ACES filmic tone mapping, sRGB encoding.
const POST: &str = r#"
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
  if (mode == 0u) {
    return downsample(in.uv);
  }
  if (mode == 1u) {
    return blur(in.uv);
  }
  return composite(in);
}
"#;

fn join(pieces: &[&str]) -> String {
    let mut out = String::with_capacity(pieces.iter().map(|piece| piece.len()).sum());
    for piece in pieces {
        out.push_str(piece);
    }
    out
}

/// Builds every shader module's source with the scene constants baked in.
pub fn create_shader_sources(constants: &SceneConstants) -> ShaderSources {
    let blob_count = constants.puff_blobs.len() / 4;
    let mut puff_blobs = String::with_capacity(blob_count * 48 + 64);
    puff_blobs.push_str("\nconst PUFF_BLOBS = array<vec4f, ");
    push_integer(&mut puff_blobs, blob_count as f64);
    puff_blobs.push_str(">(");
    for (index, blob) in constants.puff_blobs.as_chunks::<4>().0.iter().enumerate() {
        if index > 0 {
            puff_blobs.push_str(", ");
        }
        puff_blobs.push_str("vec4f(");
        for (component, &value) in blob.iter().enumerate() {
            if component > 0 {
                puff_blobs.push_str(", ");
            }
            push_float(&mut puff_blobs, value);
        }
        puff_blobs.push(')');
    }
    puff_blobs.push_str(");\n");

    let mut road_constants = String::from("\nconst ROAD_HALF_WIDTH = ");
    push_float(&mut road_constants, constants.road_half_width);
    road_constants.push_str(";\n");

    let mut scene_constants = String::from("\nconst FIELD = vec2f(");
    push_float(&mut scene_constants, constants.field_width);
    scene_constants.push_str(", ");
    push_float(&mut scene_constants, constants.field_height);
    scene_constants.push_str(");\n");
    scene_constants.push_str(&road_constants);
    let lights: [(&str, Vec3Tuple); 6] = [
        ("const HEMI_SKY = ", constants.hemi_sky),
        ("\nconst HEMI_GROUND = ", constants.hemi_ground),
        ("\nconst KEY_DIRECTION = ", constants.key_direction),
        ("\nconst KEY_COLOR = ", constants.key_color),
        ("\nconst RIM_DIRECTION = ", constants.rim_direction),
        ("\nconst RIM_COLOR = ", constants.rim_color),
    ];
    for (prefix, value) in lights {
        scene_constants.push_str(prefix);
        push_vec3(&mut scene_constants, value);
        scene_constants.push(';');
    }
    scene_constants.push('\n');

    ShaderSources {
        neon: join(&[COMMON, &scene_constants, FRAME, LIGHTING, INSTANCE, NEON]),
        ground: join(&[COMMON, &scene_constants, FRAME, LIGHTING, GROUND]),
        road: join(&[COMMON, &scene_constants, FRAME, LIGHTING, ROAD]),
        effect: join(&[COMMON, &road_constants, FRAME, INSTANCE, &puff_blobs, PUFF, EFFECT]),
        sprite: join(&[COMMON, FRAME, &puff_blobs, PUFF, SPRITE]),
        post: join(&[COMMON, POST]),
    }
}

const FRAGMENT_SIGNATURE: &str = "@fragment fn fragmentMain(in: Varying) -> @location(0) vec4f";

/// Benchmark aid: a nonzero salt adds an inert constant to every fragment output so GPU
/// driver shader caches miss, making first-visit compile cost measurable on demand.
pub fn salt_shader(code: &str, salt: u32) -> Cow<'_, str> {
    if salt == 0 {
        return Cow::Borrowed(code);
    }
    // A plain byte scan keeps the string-search machinery out of the Wasm build.
    let signature = FRAGMENT_SIGNATURE.as_bytes();
    let position = code.as_bytes().windows(signature.len()).position(|window| window == signature);
    // Non-panicking slicing (the signature is ASCII, so the bounds are char boundaries).
    let (Some(head), Some(tail)) =
        position.map_or((None, None), |start| (code.get(..start), code.get(start + signature.len()..)))
    else {
        return Cow::Borrowed(code);
    };
    let mut out = String::with_capacity(code.len() + 128);
    out.push_str(head);
    out.push_str("fn fragmentBody(in: Varying) -> vec4f");
    out.push_str(tail);
    out.push_str(
        "\n@fragment fn fragmentMain(in: Varying) -> @location(0) vec4f {\n  return fragmentBody(in) + vec4f(",
    );
    push_integer(&mut out, salt as f64);
    out.push_str(".0 * 1e-12);\n}\n");
    Cow::Owned(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn formatted(value: f64) -> String {
        let mut out = String::new();
        push_float(&mut out, value);
        out
    }

    #[test]
    fn floats_format_like_the_original() {
        assert_eq!(formatted(800.0), "800.0");
        assert_eq!(formatted(-3.0), "-3.0");
        assert_eq!(formatted(12.0), "12.0");
        assert_eq!(formatted(0.66), "0.66");
        assert_eq!(formatted(0.5), "0.5");
        assert_eq!(formatted(12.25), "12.25");
        assert_eq!(formatted(0.123456789), "0.1234568");
        assert_eq!(formatted(-0.0004563219), "-0.0004563219");
        assert_eq!(formatted(76.543216), "76.54322");
        assert_eq!(formatted(9.99999996), "10");
        assert_eq!(formatted(1234.56789), "1234.568");
    }

    #[test]
    fn salting_wraps_the_fragment_entry_point() {
        let salted = salt_shader("@fragment fn fragmentMain(in: Varying) -> @location(0) vec4f {\n}\n", 3);
        assert!(salted.starts_with("fn fragmentBody(in: Varying) -> vec4f {"));
        assert!(salted.contains("return fragmentBody(in) + vec4f(3.0 * 1e-12);"));
        assert_eq!(salt_shader("x", 0), "x");
    }
}

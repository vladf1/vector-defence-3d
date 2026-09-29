import type { ShaderSources } from "./shaders";
import { NEON_VERTEX_FLOATS } from "./geometry-kit";
import { ShaderStage } from "./gpu-flags";

export const HDR_FORMAT: GPUTextureFormat = "rgba16float";
export const DEPTH_FORMAT: GPUTextureFormat = "depth24plus";

/** Floats per instance: column-major transform (16), tint + mode (4), extras (4). */
export const INSTANCE_FLOATS = 24;
/** Floats per sprite: position + rotation (4), size + shape + mode (4), color (4). */
export const SPRITE_FLOATS = 12;
export const FLAT_VERTEX_FLOATS = 5;

export interface ScenePipelines {
  readonly neon: GPURenderPipeline;
  readonly ground: GPURenderPipeline;
  readonly road: GPURenderPipeline;
  /** Ribbons, decals, tower ranges, and ground glows (premultiplied alpha). */
  readonly effect: GPURenderPipeline;
  /** Health bars: the effect module drawn opaque over everything. */
  readonly healthBar: GPURenderPipeline;
  /** Glow and smoke sprites (premultiplied alpha). */
  readonly sprite: GPURenderPipeline;
}

export interface PostPipelines {
  /** Downsample and blur passes (HDR targets); the pass parameters pick the pass. */
  readonly bloom: GPURenderPipeline;
  /** Composite onto the canvas. */
  readonly composite: GPURenderPipeline;
}

export interface GpuPipelines {
  readonly frameLayout: GPUBindGroupLayout;
  readonly postLayout: GPUBindGroupLayout;
  readonly scene: ScenePipelines;
  readonly post: PostPipelines;
}

const FLOAT_BYTES = 4;

const INSTANCE_BUFFER: GPUVertexBufferLayout = {
  arrayStride: INSTANCE_FLOATS * FLOAT_BYTES,
  stepMode: "instance",
  attributes: [3, 4, 5, 6, 7, 8].map((shaderLocation, index) => ({ shaderLocation, offset: index * 16, format: "float32x4" as const })),
};

const NEON_VERTEX_BUFFER: GPUVertexBufferLayout = {
  arrayStride: NEON_VERTEX_FLOATS * FLOAT_BYTES,
  attributes: [
    { shaderLocation: 0, offset: 0, format: "float32x3" },
    { shaderLocation: 1, offset: 12, format: "float32x3" },
    { shaderLocation: 2, offset: 24, format: "float32" },
  ],
};

const FLAT_VERTEX_BUFFER: GPUVertexBufferLayout = {
  arrayStride: FLAT_VERTEX_FLOATS * FLOAT_BYTES,
  attributes: [
    { shaderLocation: 0, offset: 0, format: "float32x3" },
    { shaderLocation: 1, offset: 12, format: "float32x2" },
  ],
};

const SPRITE_BUFFER: GPUVertexBufferLayout = {
  arrayStride: SPRITE_FLOATS * FLOAT_BYTES,
  stepMode: "instance",
  attributes: [0, 1, 2].map((shaderLocation) => ({ shaderLocation, offset: shaderLocation * 16, format: "float32x4" as const })),
};

// Shaders output premultiplied color, so alpha 0 is additive and alpha a is normal blending:
// one blend state (one pipeline) serves both kinds of layers.
const PREMULTIPLIED: GPUBlendState = {
  color: { srcFactor: "one", dstFactor: "one-minus-src-alpha", operation: "add" },
  alpha: { srcFactor: "one", dstFactor: "one-minus-src-alpha", operation: "add" },
};

type DepthMode = "opaque" | "transparent" | "overlay";

const DEPTH_MODES: Record<DepthMode, GPUDepthStencilState> = {
  opaque: { format: DEPTH_FORMAT, depthWriteEnabled: true, depthCompare: "less-equal" },
  transparent: { format: DEPTH_FORMAT, depthWriteEnabled: false, depthCompare: "less-equal" },
  overlay: { format: DEPTH_FORMAT, depthWriteEnabled: false, depthCompare: "always" },
};

/**
 * Benchmark aid: a nonzero salt adds an inert constant to every fragment output so GPU
 * driver shader caches miss, making first-visit compile cost measurable on demand.
 */
function saltShader(code: string, salt: number): string {
  if (salt === 0) {
    return code;
  }
  return `${code.replace("@fragment fn fragmentMain(in: Varying) -> @location(0) vec4f", "fn fragmentBody(in: Varying) -> vec4f")}
@fragment fn fragmentMain(in: Varying) -> @location(0) vec4f {
  return fragmentBody(in) + vec4f(${salt.toFixed(1)} * 1e-12);
}
`;
}

/**
 * Starts every pipeline the board will ever use at once, and nothing compiles after
 * startup: materials never vary per entity (instance data carries identity), so this fixed
 * set covers the whole game. WebKit compiles serially and pays for every distinct module and
 * pipeline state, so modules are shared and pipeline states kept to eight.
 */
export async function createPipelines(
  device: GPUDevice,
  canvasFormat: GPUTextureFormat,
  sampleCount: number,
  shaderSalt: number,
  sources: ShaderSources,
): Promise<GpuPipelines> {
  const frameLayout = device.createBindGroupLayout({
    label: "frame",
    entries: [{ binding: 0, visibility: ShaderStage.VERTEX | ShaderStage.FRAGMENT, buffer: { type: "uniform" } }],
  });
  const postLayout = device.createBindGroupLayout({
    label: "post",
    entries: [
      { binding: 0, visibility: ShaderStage.FRAGMENT, sampler: { type: "filtering" } },
      { binding: 1, visibility: ShaderStage.FRAGMENT, texture: { sampleType: "float" } },
      { binding: 2, visibility: ShaderStage.FRAGMENT, texture: { sampleType: "float" } },
      { binding: 3, visibility: ShaderStage.FRAGMENT, texture: { sampleType: "float" } },
      { binding: 4, visibility: ShaderStage.FRAGMENT, buffer: { type: "uniform" } },
    ],
  });
  const sceneLayout = device.createPipelineLayout({ bindGroupLayouts: [frameLayout] });
  const postPipelineLayout = device.createPipelineLayout({ bindGroupLayouts: [postLayout] });
  const multisample: GPUMultisampleState = { count: Math.max(1, sampleCount) };
  const module = (label: string, code: string): GPUShaderModule => device.createShaderModule({ label, code: saltShader(code, shaderSalt) });
  const neonModule = module("neon", sources.neon);
  const groundModule = module("ground", sources.ground);
  const roadModule = module("road", sources.road);
  const effectModule = module("effect", sources.effect);
  const spriteModule = module("sprite", sources.sprite);
  const postModule = module("post", sources.post);

  const scene = (
    label: string,
    shader: GPUShaderModule,
    buffers: GPUVertexBufferLayout[],
    depth: DepthMode,
    blend: GPUBlendState | undefined,
  ): Promise<GPURenderPipeline> => device.createRenderPipelineAsync({
    label,
    layout: sceneLayout,
    vertex: { module: shader, entryPoint: "vertexMain", buffers },
    fragment: { module: shader, entryPoint: "fragmentMain", targets: [{ format: HDR_FORMAT, blend }] },
    primitive: { topology: "triangle-list", cullMode: "back", frontFace: "ccw" },
    depthStencil: DEPTH_MODES[depth],
    multisample,
  });

  const post = (label: string, format: GPUTextureFormat): Promise<GPURenderPipeline> => device.createRenderPipelineAsync({
    label,
    layout: postPipelineLayout,
    vertex: { module: postModule, entryPoint: "vertexMain" },
    fragment: { module: postModule, entryPoint: "fragmentMain", targets: [{ format }] },
    primitive: { topology: "triangle-list" },
  });

  const instancedFlat = [FLAT_VERTEX_BUFFER, INSTANCE_BUFFER];
  const [neon, ground, road, effect, healthBar, sprite, bloom, composite] = await Promise.all([
    scene("neon", neonModule, [NEON_VERTEX_BUFFER, INSTANCE_BUFFER], "opaque", undefined),
    scene("ground", groundModule, [], "opaque", undefined),
    scene("road", roadModule, [FLAT_VERTEX_BUFFER], "opaque", undefined),
    scene("effect", effectModule, instancedFlat, "transparent", PREMULTIPLIED),
    scene("health-bar", effectModule, instancedFlat, "overlay", undefined),
    scene("sprite", spriteModule, [SPRITE_BUFFER], "transparent", PREMULTIPLIED),
    post("bloom", HDR_FORMAT),
    post("composite", canvasFormat),
  ]);

  return {
    frameLayout,
    postLayout,
    scene: { neon, ground, road, effect, healthBar, sprite },
    post: { bloom, composite },
  };
}

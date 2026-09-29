import { DEPTH_FORMAT, HDR_FORMAT, type GpuPipelines } from "./gpu-pipelines";
import { BufferUsage, TextureUsage } from "./gpu-flags";
import { POST_PARAMS_FLOATS, PostMode } from "./shaders";

const BLOOM_THRESHOLD = 0.85;
const FULLSCREEN_TRIANGLE_VERTICES = 3;
// Params: texel (2), direction (2), threshold, mode, pad (2).
const DIRECTION_OFFSET = 2;
const THRESHOLD_OFFSET = 4;
const MODE_OFFSET = 5;

interface SampledPass {
  readonly params: GPUBuffer;
  readonly values: Float32Array;
  bindGroup?: GPUBindGroup;
}

interface Targets {
  readonly width: number;
  readonly height: number;
  readonly sceneColor: GPUTexture;
  readonly sceneMultisample: GPUTexture | null;
  readonly depth: GPUTexture;
  readonly fine: GPUTexture;
  readonly fineScratch: GPUTexture;
  readonly wide: GPUTexture;
  readonly wideScratch: GPUTexture;
  readonly sceneView: GPUTextureView;
  readonly sceneMultisampleView: GPUTextureView | null;
  readonly depthView: GPUTextureView;
  readonly fineView: GPUTextureView;
  readonly fineScratchView: GPUTextureView;
  readonly wideView: GPUTextureView;
  readonly wideScratchView: GPUTextureView;
}

/**
 * Lean HDR bloom: scene -> 1/4 prefilter -> blur -> 1/8 -> blur, composited with ACES tone
 * mapping, sRGB encoding, and a vignette straight onto the canvas. One shader module covers
 * all seven passes (per-pass parameters pick the pass); unused texture slots hold a 1x1
 * placeholder.
 */
export class PostProcessing {
  /** Bloom and composite passes, one full-screen draw each. */
  readonly drawCalls = 7;

  private readonly sampler: GPUSampler;
  private readonly placeholder: GPUTexture;
  private readonly placeholderView: GPUTextureView;
  private readonly prefilter: SampledPass;
  private readonly downsample: SampledPass;
  private readonly fineHorizontal: SampledPass;
  private readonly fineVertical: SampledPass;
  private readonly wideHorizontal: SampledPass;
  private readonly wideVertical: SampledPass;
  private readonly compositePass: SampledPass;
  private targets?: Targets;

  constructor(
    private readonly device: GPUDevice,
    private readonly pipelines: GpuPipelines,
    private readonly context: GPUCanvasContext,
    private readonly msaaSamples: number,
    private readonly bloomResolution: number,
  ) {
    this.sampler = device.createSampler({ label: "post", magFilter: "linear", minFilter: "linear" });
    this.placeholder = device.createTexture({ label: "post-placeholder", size: [1, 1], format: HDR_FORMAT, usage: TextureUsage.TEXTURE_BINDING });
    this.placeholderView = this.placeholder.createView();
    this.prefilter = this.createPass("prefilter", PostMode.Downsample, 0, 0, BLOOM_THRESHOLD);
    this.downsample = this.createPass("downsample", PostMode.Downsample, 0, 0, 0);
    this.fineHorizontal = this.createPass("fine-horizontal", PostMode.Blur, 1, 0, 0);
    this.fineVertical = this.createPass("fine-vertical", PostMode.Blur, 0, 1, 0);
    this.wideHorizontal = this.createPass("wide-horizontal", PostMode.Blur, 1, 0, 0);
    this.wideVertical = this.createPass("wide-vertical", PostMode.Blur, 0, 1, 0);
    this.compositePass = this.createPass("composite", PostMode.Composite, 0, 0, 0);
  }

  /** Matches every target to the canvas drawing-buffer size (in device pixels). */
  resize(width: number, height: number): void {
    width = Math.max(1, width);
    height = Math.max(1, height);
    if (this.targets && this.targets.width === width && this.targets.height === height) {
      return;
    }
    this.destroyTargets();
    const device = this.device;
    const fineWidth = Math.max(1, Math.round(width * this.bloomResolution * 0.5));
    const fineHeight = Math.max(1, Math.round(height * this.bloomResolution * 0.5));
    const wideWidth = Math.max(1, Math.round(fineWidth / 2));
    const wideHeight = Math.max(1, Math.round(fineHeight / 2));
    const sampled = TextureUsage.RENDER_ATTACHMENT | TextureUsage.TEXTURE_BINDING;
    const hdr = (label: string, w: number, h: number): GPUTexture => device.createTexture({ label, size: [w, h], format: HDR_FORMAT, usage: sampled });
    const multisampled = this.msaaSamples > 1;
    const sampleCount = multisampled ? this.msaaSamples : 1;
    const sceneColor = hdr("scene-color", width, height);
    const sceneMultisample = multisampled
      ? device.createTexture({ label: "scene-msaa", size: [width, height], format: HDR_FORMAT, sampleCount, usage: TextureUsage.RENDER_ATTACHMENT })
      : null;
    const depth = device.createTexture({ label: "scene-depth", size: [width, height], format: DEPTH_FORMAT, sampleCount, usage: TextureUsage.RENDER_ATTACHMENT });
    const fine = hdr("bloom-fine", fineWidth, fineHeight);
    const fineScratch = hdr("bloom-fine-scratch", fineWidth, fineHeight);
    const wide = hdr("bloom-wide", wideWidth, wideHeight);
    const wideScratch = hdr("bloom-wide-scratch", wideWidth, wideHeight);
    const targets: Targets = {
      width,
      height,
      sceneColor,
      sceneMultisample,
      depth,
      fine,
      fineScratch,
      wide,
      wideScratch,
      sceneView: sceneColor.createView(),
      sceneMultisampleView: sceneMultisample?.createView() ?? null,
      depthView: depth.createView(),
      fineView: fine.createView(),
      fineScratchView: fineScratch.createView(),
      wideView: wide.createView(),
      wideScratchView: wideScratch.createView(),
    };
    this.targets = targets;

    this.bindPass(this.prefilter, targets.sceneView, 1 / width, 1 / height, this.placeholderView, this.placeholderView);
    this.bindPass(this.fineHorizontal, targets.fineView, 1 / fineWidth, 1 / fineHeight, this.placeholderView, this.placeholderView);
    this.bindPass(this.fineVertical, targets.fineScratchView, 1 / fineWidth, 1 / fineHeight, this.placeholderView, this.placeholderView);
    this.bindPass(this.downsample, targets.fineView, 1 / fineWidth, 1 / fineHeight, this.placeholderView, this.placeholderView);
    this.bindPass(this.wideHorizontal, targets.wideView, 1 / wideWidth, 1 / wideHeight, this.placeholderView, this.placeholderView);
    this.bindPass(this.wideVertical, targets.wideScratchView, 1 / wideWidth, 1 / wideHeight, this.placeholderView, this.placeholderView);
    this.bindPass(this.compositePass, targets.sceneView, 1 / width, 1 / height, targets.fineView, targets.wideView);
  }

  /** Records the scene pass (via `drawScene`) and the whole post chain into `encoder`. */
  render(encoder: GPUCommandEncoder, clearColor: GPUColor, drawScene: (pass: GPURenderPassEncoder) => void): void {
    const targets = this.targets;
    if (!targets) {
      return;
    }
    const multisampleView = targets.sceneMultisampleView;
    const scenePass = encoder.beginRenderPass({
      label: "scene",
      colorAttachments: [{
        view: multisampleView ?? targets.sceneView,
        resolveTarget: multisampleView ? targets.sceneView : undefined,
        clearValue: clearColor,
        loadOp: "clear",
        storeOp: multisampleView ? "discard" : "store",
      }],
      depthStencilAttachment: {
        view: targets.depthView,
        depthClearValue: 1,
        depthLoadOp: "clear",
        depthStoreOp: "discard",
      },
    });
    drawScene(scenePass);
    scenePass.end();

    const { bloom, composite } = this.pipelines.post;
    this.runPass(encoder, bloom, this.prefilter, targets.fineView);
    this.runPass(encoder, bloom, this.fineHorizontal, targets.fineScratchView);
    this.runPass(encoder, bloom, this.fineVertical, targets.fineView);
    this.runPass(encoder, bloom, this.downsample, targets.wideView);
    this.runPass(encoder, bloom, this.wideHorizontal, targets.wideScratchView);
    this.runPass(encoder, bloom, this.wideVertical, targets.wideView);

    const output = encoder.beginRenderPass({
      label: "composite",
      colorAttachments: [{ view: this.context.getCurrentTexture().createView(), loadOp: "clear", clearValue: [0, 0, 0, 1], storeOp: "store" }],
    });
    output.setPipeline(composite);
    output.setBindGroup(0, this.compositePass.bindGroup ?? null);
    output.draw(FULLSCREEN_TRIANGLE_VERTICES);
    output.end();
  }

  dispose(): void {
    this.destroyTargets();
    for (const pass of [this.prefilter, this.downsample, this.fineHorizontal, this.fineVertical, this.wideHorizontal, this.wideVertical, this.compositePass]) {
      pass.params.destroy();
    }
    this.placeholder.destroy();
  }

  private createPass(label: string, mode: number, directionX: number, directionY: number, threshold: number): SampledPass {
    const values = new Float32Array(POST_PARAMS_FLOATS);
    values[DIRECTION_OFFSET] = directionX;
    values[DIRECTION_OFFSET + 1] = directionY;
    values[THRESHOLD_OFFSET] = threshold;
    values[MODE_OFFSET] = mode;
    const params = this.device.createBuffer({ label, size: values.byteLength, usage: BufferUsage.UNIFORM | BufferUsage.COPY_DST });
    return { params, values };
  }

  private bindPass(pass: SampledPass, source: GPUTextureView, texelX: number, texelY: number, fine: GPUTextureView, wide: GPUTextureView): void {
    pass.values[0] = texelX;
    pass.values[1] = texelY;
    this.device.queue.writeBuffer(pass.params, 0, pass.values);
    pass.bindGroup = this.device.createBindGroup({
      label: pass.params.label,
      layout: this.pipelines.postLayout,
      entries: [
        { binding: 0, resource: this.sampler },
        { binding: 1, resource: source },
        { binding: 2, resource: fine },
        { binding: 3, resource: wide },
        { binding: 4, resource: { buffer: pass.params } },
      ],
    });
  }

  private runPass(encoder: GPUCommandEncoder, pipeline: GPURenderPipeline, pass: SampledPass, target: GPUTextureView): void {
    const renderPass = encoder.beginRenderPass({
      colorAttachments: [{ view: target, loadOp: "clear", clearValue: [0, 0, 0, 1], storeOp: "store" }],
    });
    renderPass.setPipeline(pipeline);
    renderPass.setBindGroup(0, pass.bindGroup ?? null);
    renderPass.draw(FULLSCREEN_TRIANGLE_VERTICES);
    renderPass.end();
  }

  private destroyTargets(): void {
    const targets = this.targets;
    if (!targets) {
      return;
    }
    for (const texture of [targets.sceneColor, targets.sceneMultisample, targets.depth, targets.fine, targets.fineScratch, targets.wide, targets.wideScratch]) {
      texture?.destroy();
    }
    this.targets = undefined;
  }
}

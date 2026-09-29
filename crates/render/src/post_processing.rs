//! HDR targets and the bloom/composite pass chain.
use wasm_bindgen::JsValue;
use web_sys::{
    GpuBindGroup, GpuBindGroupDescriptor, GpuBindGroupEntry, GpuBindGroupLayout, GpuBuffer, GpuBufferDescriptor,
    GpuCanvasContext, GpuColorDict, GpuCommandEncoder, GpuDevice, GpuExtent3dDict, GpuFilterMode, GpuLoadOp,
    GpuRenderPassColorAttachment, GpuRenderPassDepthStencilAttachment, GpuRenderPassDescriptor, GpuRenderPassEncoder,
    GpuRenderPipeline, GpuSampler, GpuSamplerDescriptor, GpuStoreOp, GpuTexture, GpuTextureDescriptor, GpuTextureView,
};

use crate::gpu_flags::{buffer_usage, f32_bytes, texture_usage};
use crate::gpu_pipelines::{DEPTH_FORMAT, GpuPipelines, HDR_FORMAT};
use crate::shaders::{POST_PARAMS_FLOATS, post_mode};

const BLOOM_THRESHOLD: f32 = 0.85;
const FULLSCREEN_TRIANGLE_VERTICES: u32 = 3;
// Params: texel (2), direction (2), threshold, mode, pad (2).
const DIRECTION_OFFSET: usize = 2;
const THRESHOLD_OFFSET: usize = 4;
const MODE_OFFSET: usize = 5;

struct SampledPass {
    label: &'static str,
    params: GpuBuffer,
    values: [f32; POST_PARAMS_FLOATS],
    bind_group: Option<GpuBindGroup>,
    /// The pass descriptor, rebuilt with the targets (bloom passes only).
    descriptor: Option<GpuRenderPassDescriptor>,
}

struct Targets {
    width: u32,
    height: u32,
    textures: Vec<GpuTexture>,
    /// Scene pass descriptor: MSAA color (resolving into the scene color) and depth.
    scene_pass: GpuRenderPassDescriptor,
}

fn nullable<T: wasm_bindgen::JsGeneric>(value: T) -> js_sys::JsNullable<T> {
    js_sys::JsNullable::wrap(value)
}

fn clear_black() -> GpuColorDict {
    GpuColorDict::new(1.0, 0.0, 0.0, 0.0)
}

/// Lean HDR bloom: scene -> 1/4 prefilter -> blur -> 1/8 -> blur, composited with ACES tone
/// mapping, sRGB encoding, and a vignette straight onto the canvas. One shader module covers
/// all seven passes (per-pass parameters pick the pass); unused texture slots hold a 1x1
/// placeholder.
///
/// Render pass descriptors are built once per resize and reused every frame; only the
/// composite pass's canvas view is swapped in per frame.
pub struct PostProcessing {
    device: GpuDevice,
    context: GpuCanvasContext,
    post_layout: GpuBindGroupLayout,
    msaa_samples: u32,
    bloom_resolution: f64,
    sampler: GpuSampler,
    placeholder: GpuTexture,
    placeholder_view: GpuTextureView,
    /// prefilter, fine horizontal, fine vertical, downsample, wide horizontal, wide vertical.
    bloom_passes: [SampledPass; 6],
    composite_pass: SampledPass,
    composite_attachment: GpuRenderPassColorAttachment,
    composite_descriptor: GpuRenderPassDescriptor,
    targets: Option<Targets>,
}

impl PostProcessing {
    /// Bloom and composite passes, one full-screen draw each.
    pub const DRAW_CALLS: u32 = 7;

    pub fn new(
        device: &GpuDevice,
        pipelines: &GpuPipelines,
        context: &GpuCanvasContext,
        msaa_samples: u32,
        bloom_resolution: f64,
    ) -> Result<Self, JsValue> {
        let sampler_descriptor = GpuSamplerDescriptor::new();
        sampler_descriptor.set_label("post");
        sampler_descriptor.set_mag_filter(GpuFilterMode::Linear);
        sampler_descriptor.set_min_filter(GpuFilterMode::Linear);
        let sampler = device.create_sampler_with_descriptor(&sampler_descriptor);
        let placeholder = create_texture(device, "post-placeholder", 1, 1, 1, texture_usage::TEXTURE_BINDING, false)?;
        let placeholder_view = placeholder.create_view()?;
        let pass = |label, mode, direction: [f32; 2], threshold| create_pass(device, label, mode, direction, threshold);
        let bloom_passes = [
            pass("prefilter", post_mode::DOWNSAMPLE, [0.0, 0.0], BLOOM_THRESHOLD)?,
            pass("fine-horizontal", post_mode::BLUR, [1.0, 0.0], 0.0)?,
            pass("fine-vertical", post_mode::BLUR, [0.0, 1.0], 0.0)?,
            pass("downsample", post_mode::DOWNSAMPLE, [0.0, 0.0], 0.0)?,
            pass("wide-horizontal", post_mode::BLUR, [1.0, 0.0], 0.0)?,
            pass("wide-vertical", post_mode::BLUR, [0.0, 1.0], 0.0)?,
        ];
        let composite_pass = pass("composite", post_mode::COMPOSITE, [0.0, 0.0], 0.0)?;
        let composite_attachment = GpuRenderPassColorAttachment::new_with_gpu_texture_view(
            GpuLoadOp::Clear,
            GpuStoreOp::Store,
            &placeholder_view,
        );
        composite_attachment.set_clear_value_gpu_color_dict(&clear_black());
        let composite_descriptor = GpuRenderPassDescriptor::new(&[nullable(composite_attachment.clone())]);
        composite_descriptor.set_label("composite");
        Ok(PostProcessing {
            device: device.clone(),
            context: context.clone(),
            post_layout: pipelines.post_layout.clone(),
            msaa_samples,
            bloom_resolution,
            sampler,
            placeholder,
            placeholder_view,
            bloom_passes,
            composite_pass,
            composite_attachment,
            composite_descriptor,
            targets: None,
        })
    }

    /// Matches every target to the canvas drawing-buffer size (in device pixels).
    pub fn resize(&mut self, width: u32, height: u32, clear: &GpuColorDict) -> Result<(), JsValue> {
        let width = width.max(1);
        let height = height.max(1);
        if self.targets.as_ref().is_some_and(|targets| targets.width == width && targets.height == height) {
            return Ok(());
        }
        self.destroy_targets();
        let device = &self.device;
        let round = |value: f64| (value.round() as u32).max(1);
        let fine_width = round(width as f64 * self.bloom_resolution * 0.5);
        let fine_height = round(height as f64 * self.bloom_resolution * 0.5);
        let wide_width = round(fine_width as f64 / 2.0);
        let wide_height = round(fine_height as f64 / 2.0);
        let sampled = texture_usage::RENDER_ATTACHMENT | texture_usage::TEXTURE_BINDING;
        let hdr = |label, w, h| create_texture(device, label, w, h, 1, sampled, false);
        let multisampled = self.msaa_samples > 1;
        let sample_count = if multisampled { self.msaa_samples } else { 1 };
        let scene_color = hdr("scene-color", width, height)?;
        let scene_multisample = if multisampled {
            Some(create_texture(
                device,
                "scene-msaa",
                width,
                height,
                sample_count,
                texture_usage::RENDER_ATTACHMENT,
                false,
            )?)
        } else {
            None
        };
        let depth =
            create_texture(device, "scene-depth", width, height, sample_count, texture_usage::RENDER_ATTACHMENT, true)?;
        let fine = hdr("bloom-fine", fine_width, fine_height)?;
        let fine_scratch = hdr("bloom-fine-scratch", fine_width, fine_height)?;
        let wide = hdr("bloom-wide", wide_width, wide_height)?;
        let wide_scratch = hdr("bloom-wide-scratch", wide_width, wide_height)?;
        let scene_view = scene_color.create_view()?;
        let depth_view = depth.create_view()?;
        let fine_view = fine.create_view()?;
        let fine_scratch_view = fine_scratch.create_view()?;
        let wide_view = wide.create_view()?;
        let wide_scratch_view = wide_scratch.create_view()?;

        let color = match &scene_multisample {
            Some(multisample) => {
                let attachment = GpuRenderPassColorAttachment::new_with_gpu_texture_view(
                    GpuLoadOp::Clear,
                    GpuStoreOp::Discard,
                    &multisample.create_view()?,
                );
                attachment.set_resolve_target_gpu_texture_view(&scene_view);
                attachment
            }
            None => GpuRenderPassColorAttachment::new_with_gpu_texture_view(
                GpuLoadOp::Clear,
                GpuStoreOp::Store,
                &scene_view,
            ),
        };
        color.set_clear_value_gpu_color_dict(clear);
        let depth_attachment = GpuRenderPassDepthStencilAttachment::new_with_gpu_texture_view(&depth_view);
        depth_attachment.set_depth_clear_value(1.0);
        depth_attachment.set_depth_load_op(GpuLoadOp::Clear);
        depth_attachment.set_depth_store_op(GpuStoreOp::Discard);
        let scene_pass = GpuRenderPassDescriptor::new(&[nullable(color)]);
        scene_pass.set_label("scene");
        scene_pass.set_depth_stencil_attachment(&depth_attachment);

        let fine_texel = [1.0 / fine_width as f32, 1.0 / fine_height as f32];
        let wide_texel = [1.0 / wide_width as f32, 1.0 / wide_height as f32];
        let placeholder = self.placeholder_view.clone();
        // (source, texel, target) per bloom pass, in `bloom_passes` order.
        let bloom: [(&GpuTextureView, [f32; 2], &GpuTextureView); 6] = [
            (&scene_view, [1.0 / width as f32, 1.0 / height as f32], &fine_view),
            (&fine_view, fine_texel, &fine_scratch_view),
            (&fine_scratch_view, fine_texel, &fine_view),
            (&fine_view, fine_texel, &wide_view),
            (&wide_view, wide_texel, &wide_scratch_view),
            (&wide_scratch_view, wide_texel, &wide_view),
        ];
        for (pass, (source, texel, target)) in self.bloom_passes.iter_mut().zip(bloom) {
            bind_pass(device, &self.post_layout, &self.sampler, pass, source, texel, [&placeholder, &placeholder])?;
            let attachment =
                GpuRenderPassColorAttachment::new_with_gpu_texture_view(GpuLoadOp::Clear, GpuStoreOp::Store, target);
            attachment.set_clear_value_gpu_color_dict(&clear_black());
            pass.descriptor = Some(GpuRenderPassDescriptor::new(&[nullable(attachment)]));
        }
        let texel = [1.0 / width as f32, 1.0 / height as f32];
        let composite = &mut self.composite_pass;
        bind_pass(device, &self.post_layout, &self.sampler, composite, &scene_view, texel, [&fine_view, &wide_view])?;

        let mut textures = vec![scene_color, depth, fine, fine_scratch, wide, wide_scratch];
        textures.extend(scene_multisample);
        self.targets = Some(Targets { width, height, textures, scene_pass });
        Ok(())
    }

    /// Records the scene pass (via `draw_scene`) and the whole post chain into `encoder`.
    pub fn render(
        &self,
        encoder: &GpuCommandEncoder,
        pipelines: &GpuPipelines,
        draw_scene: impl FnOnce(&GpuRenderPassEncoder),
    ) -> Result<(), JsValue> {
        let Some(targets) = &self.targets else {
            return Ok(());
        };
        let scene_pass = encoder.begin_render_pass(&targets.scene_pass)?;
        draw_scene(&scene_pass);
        scene_pass.end();

        for pass in &self.bloom_passes {
            if let Some(descriptor) = &pass.descriptor {
                run_pass(encoder, &pipelines.bloom, pass, descriptor)?;
            }
        }

        let canvas_view = self.context.get_current_texture()?.create_view()?;
        self.composite_attachment.set_view_gpu_texture_view(&canvas_view);
        run_pass(encoder, &pipelines.composite, &self.composite_pass, &self.composite_descriptor)
    }

    pub fn dispose(&mut self) {
        self.destroy_targets();
        for pass in self.bloom_passes.iter().chain([&self.composite_pass]) {
            pass.params.destroy();
        }
        self.placeholder.destroy();
    }

    fn destroy_targets(&mut self) {
        if let Some(targets) = self.targets.take() {
            for texture in &targets.textures {
                texture.destroy();
            }
        }
    }
}

fn create_texture(
    device: &GpuDevice,
    label: &str,
    width: u32,
    height: u32,
    sample_count: u32,
    usage: u32,
    depth: bool,
) -> Result<GpuTexture, JsValue> {
    let size = GpuExtent3dDict::new(width);
    size.set_height(height);
    let format = if depth { DEPTH_FORMAT } else { HDR_FORMAT };
    let descriptor = GpuTextureDescriptor::new_with_gpu_extent_3d_dict(format, &size, usage);
    descriptor.set_label(label);
    if sample_count > 1 {
        descriptor.set_sample_count(sample_count);
    }
    device.create_texture(&descriptor)
}

fn create_pass(
    device: &GpuDevice,
    label: &'static str,
    mode: f32,
    direction: [f32; 2],
    threshold: f32,
) -> Result<SampledPass, JsValue> {
    let mut values = [0.0; POST_PARAMS_FLOATS];
    values[DIRECTION_OFFSET] = direction[0];
    values[DIRECTION_OFFSET + 1] = direction[1];
    values[THRESHOLD_OFFSET] = threshold;
    values[MODE_OFFSET] = mode;
    let descriptor =
        GpuBufferDescriptor::new((POST_PARAMS_FLOATS * 4) as u32, buffer_usage::UNIFORM | buffer_usage::COPY_DST);
    descriptor.set_label(label);
    let params = device.create_buffer(&descriptor)?;
    Ok(SampledPass { label, params, values, bind_group: None, descriptor: None })
}

/// Writes the pass's texel size and binds its source (plus the bloom levels, for the composite).
fn bind_pass(
    device: &GpuDevice,
    layout: &GpuBindGroupLayout,
    sampler: &GpuSampler,
    pass: &mut SampledPass,
    source: &GpuTextureView,
    texel: [f32; 2],
    [fine, wide]: [&GpuTextureView; 2],
) -> Result<(), JsValue> {
    pass.values[0] = texel[0];
    pass.values[1] = texel[1];
    device.queue().write_buffer_with_u32_and_u8_slice(&pass.params, 0, f32_bytes(&pass.values))?;
    let entries = [
        GpuBindGroupEntry::new(0, sampler),
        GpuBindGroupEntry::new_with_gpu_texture_view(1, source),
        GpuBindGroupEntry::new_with_gpu_texture_view(2, fine),
        GpuBindGroupEntry::new_with_gpu_texture_view(3, wide),
        GpuBindGroupEntry::new_with_gpu_buffer(4, &pass.params),
    ];
    let descriptor = GpuBindGroupDescriptor::new(&entries, layout);
    descriptor.set_label(pass.label);
    pass.bind_group = Some(device.create_bind_group(&descriptor));
    Ok(())
}

fn run_pass(
    encoder: &GpuCommandEncoder,
    pipeline: &GpuRenderPipeline,
    pass: &SampledPass,
    descriptor: &GpuRenderPassDescriptor,
) -> Result<(), JsValue> {
    let render_pass = encoder.begin_render_pass(descriptor)?;
    render_pass.set_pipeline(pipeline);
    render_pass.set_bind_group(0, pass.bind_group.as_ref());
    render_pass.draw(FULLSCREEN_TRIANGLE_VERTICES);
    render_pass.end();
    Ok(())
}

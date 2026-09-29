//! The eight render pipelines, created asynchronously in one parallel batch at startup.

/// Floats per instance: column-major transform (16), tint + mode (4), extras (4).
pub const INSTANCE_FLOATS: usize = 24;
/// Floats per sprite: position + rotation (4), size + shape + mode (4), color (4).
pub const SPRITE_FLOATS: usize = 12;
pub const FLAT_VERTEX_FLOATS: usize = 5;
/// Scene (6) plus post (2) pipelines: the whole set the board ever uses.
pub const PIPELINE_COUNT: u32 = 8;

/// The scene pipeline a batch draws with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ScenePipeline {
    Neon,
    Ground,
    Road,
    /// Ribbons, decals, tower ranges, and ground glows (premultiplied alpha).
    Effect,
    /// Health bars: the effect module drawn opaque over everything.
    HealthBar,
    /// Glow and smoke sprites (premultiplied alpha).
    Sprite,
}

#[cfg(target_arch = "wasm32")]
pub use gpu::*;

#[cfg(target_arch = "wasm32")]
mod gpu {
    use js_sys::{Array, Promise};
    use wasm_bindgen::{JsCast, JsValue};
    use web_sys::{
        GpuBindGroupLayout, GpuBindGroupLayoutDescriptor, GpuBindGroupLayoutEntry, GpuBlendComponent, GpuBlendFactor,
        GpuBlendOperation, GpuBlendState, GpuBufferBindingLayout, GpuBufferBindingType, GpuColorTargetState,
        GpuCompareFunction, GpuCullMode, GpuDepthStencilState, GpuDevice, GpuFragmentState, GpuFrontFace,
        GpuMultisampleState, GpuPipelineLayout, GpuPipelineLayoutDescriptor, GpuPrimitiveState, GpuPrimitiveTopology,
        GpuRenderPipeline, GpuRenderPipelineDescriptor, GpuSamplerBindingLayout, GpuSamplerBindingType,
        GpuShaderModule, GpuShaderModuleDescriptor, GpuTextureBindingLayout, GpuTextureFormat, GpuTextureSampleType,
        GpuVertexAttribute, GpuVertexBufferLayout, GpuVertexFormat, GpuVertexState, GpuVertexStepMode,
    };

    use super::{FLAT_VERTEX_FLOATS, INSTANCE_FLOATS, SPRITE_FLOATS, ScenePipeline};
    use crate::geometry_kit::NEON_VERTEX_FLOATS;
    use crate::gpu_flags::shader_stage;
    use crate::shaders::{ShaderSources, salt_shader};

    pub const HDR_FORMAT: GpuTextureFormat = GpuTextureFormat::Rgba16float;
    pub const DEPTH_FORMAT: GpuTextureFormat = GpuTextureFormat::Depth24plus;

    const FLOAT_BYTES: u32 = 4;

    pub struct GpuPipelines {
        pub frame_layout: GpuBindGroupLayout,
        pub post_layout: GpuBindGroupLayout,
        /// Indexed by `ScenePipeline as usize`.
        pub scene: Vec<GpuRenderPipeline>,
        /// Downsample and blur passes (HDR targets); the pass parameters pick the pass.
        pub bloom: GpuRenderPipeline,
        /// Composite onto the canvas.
        pub composite: GpuRenderPipeline,
    }

    impl GpuPipelines {
        pub fn scene(&self, pipeline: ScenePipeline) -> &GpuRenderPipeline {
            &self.scene[pipeline as usize]
        }
    }

    /// Pipelines whose compiles are under way; `finish` waits for all of them.
    pub struct PendingPipelines {
        frame_layout: GpuBindGroupLayout,
        post_layout: GpuBindGroupLayout,
        all: Promise,
    }

    impl PendingPipelines {
        pub async fn finish(self) -> Result<GpuPipelines, JsValue> {
            let resolved: Array = wasm_bindgen_futures::JsFuture::from(self.all).await?.unchecked_into();
            let mut pipelines: Vec<GpuRenderPipeline> =
                resolved.iter().map(|pipeline| pipeline.unchecked_into()).collect();
            let composite = pipelines.pop().ok_or_else(|| JsValue::from_str("missing composite pipeline"))?;
            let bloom = pipelines.pop().ok_or_else(|| JsValue::from_str("missing bloom pipeline"))?;
            Ok(GpuPipelines {
                frame_layout: self.frame_layout,
                post_layout: self.post_layout,
                scene: pipelines,
                bloom,
                composite,
            })
        }
    }

    fn attribute(format: GpuVertexFormat, offset: u32, location: u32) -> GpuVertexAttribute {
        GpuVertexAttribute::new(format, offset, location)
    }

    fn instance_buffer() -> GpuVertexBufferLayout {
        let attributes: Vec<GpuVertexAttribute> = [3, 4, 5, 6, 7, 8]
            .iter()
            .enumerate()
            .map(|(index, &location)| attribute(GpuVertexFormat::Float32x4, index as u32 * 16, location))
            .collect();
        let layout = GpuVertexBufferLayout::new(INSTANCE_FLOATS as u32 * FLOAT_BYTES, &attributes);
        layout.set_step_mode(GpuVertexStepMode::Instance);
        layout
    }

    fn neon_vertex_buffer() -> GpuVertexBufferLayout {
        GpuVertexBufferLayout::new(
            NEON_VERTEX_FLOATS as u32 * FLOAT_BYTES,
            &[
                attribute(GpuVertexFormat::Float32x3, 0, 0),
                attribute(GpuVertexFormat::Float32x3, 12, 1),
                attribute(GpuVertexFormat::Float32, 24, 2),
            ],
        )
    }

    fn flat_vertex_buffer() -> GpuVertexBufferLayout {
        GpuVertexBufferLayout::new(
            FLAT_VERTEX_FLOATS as u32 * FLOAT_BYTES,
            &[attribute(GpuVertexFormat::Float32x3, 0, 0), attribute(GpuVertexFormat::Float32x2, 12, 1)],
        )
    }

    fn sprite_buffer() -> GpuVertexBufferLayout {
        let attributes: Vec<GpuVertexAttribute> =
            (0..3).map(|location| attribute(GpuVertexFormat::Float32x4, location * 16, location)).collect();
        let layout = GpuVertexBufferLayout::new(SPRITE_FLOATS as u32 * FLOAT_BYTES, &attributes);
        layout.set_step_mode(GpuVertexStepMode::Instance);
        layout
    }

    // Shaders output premultiplied color, so alpha 0 is additive and alpha a is normal blending:
    // one blend state (one pipeline) serves both kinds of layers.
    fn premultiplied() -> GpuBlendState {
        let component = GpuBlendComponent::new();
        component.set_src_factor(GpuBlendFactor::One);
        component.set_dst_factor(GpuBlendFactor::OneMinusSrcAlpha);
        component.set_operation(GpuBlendOperation::Add);
        GpuBlendState::new(&component, &component)
    }

    #[derive(Clone, Copy)]
    enum DepthMode {
        Opaque,
        Transparent,
        Overlay,
    }

    fn depth_state(mode: DepthMode) -> GpuDepthStencilState {
        let state = GpuDepthStencilState::new(DEPTH_FORMAT);
        let (write, compare) = match mode {
            DepthMode::Opaque => (true, GpuCompareFunction::LessEqual),
            DepthMode::Transparent => (false, GpuCompareFunction::LessEqual),
            DepthMode::Overlay => (false, GpuCompareFunction::Always),
        };
        state.set_depth_write_enabled(write);
        state.set_depth_compare(compare);
        state
    }

    fn layout_entry(binding: u32, visibility: u32) -> GpuBindGroupLayoutEntry {
        GpuBindGroupLayoutEntry::new(binding, visibility)
    }

    fn uniform_entry(binding: u32, visibility: u32) -> GpuBindGroupLayoutEntry {
        let entry = layout_entry(binding, visibility);
        let buffer = GpuBufferBindingLayout::new();
        buffer.set_type(GpuBufferBindingType::Uniform);
        entry.set_buffer(&buffer);
        entry
    }

    fn texture_entry(binding: u32) -> GpuBindGroupLayoutEntry {
        let entry = layout_entry(binding, shader_stage::FRAGMENT);
        let texture = GpuTextureBindingLayout::new();
        texture.set_sample_type(GpuTextureSampleType::Float);
        entry.set_texture(&texture);
        entry
    }

    fn bind_group_layout(
        device: &GpuDevice,
        label: &str,
        entries: &[GpuBindGroupLayoutEntry],
    ) -> Result<GpuBindGroupLayout, JsValue> {
        let descriptor = GpuBindGroupLayoutDescriptor::new(entries);
        descriptor.set_label(label);
        device.create_bind_group_layout(&descriptor)
    }

    fn pipeline_layout(device: &GpuDevice, layout: &GpuBindGroupLayout) -> GpuPipelineLayout {
        device.create_pipeline_layout(&GpuPipelineLayoutDescriptor::new(&[js_sys::JsNullable::wrap(layout.clone())]))
    }

    fn nullable<T: wasm_bindgen::JsGeneric>(value: T) -> js_sys::JsNullable<T> {
        js_sys::JsNullable::wrap(value)
    }

    struct SceneBuilder<'a> {
        device: &'a GpuDevice,
        layout: GpuPipelineLayout,
        multisample: GpuMultisampleState,
        primitive: GpuPrimitiveState,
    }

    impl SceneBuilder<'_> {
        fn start(
            &self,
            label: &str,
            module: &GpuShaderModule,
            buffers: &[GpuVertexBufferLayout],
            depth: DepthMode,
            blend: Option<&GpuBlendState>,
        ) -> Promise {
            let vertex = GpuVertexState::new(module);
            vertex.set_entry_point("vertexMain");
            let buffers: Vec<_> = buffers.iter().map(|buffer| nullable(buffer.clone())).collect();
            vertex.set_buffers(&buffers);
            let target = GpuColorTargetState::new(HDR_FORMAT);
            if let Some(blend) = blend {
                target.set_blend(blend);
            }
            let fragment = GpuFragmentState::new(module, &[nullable(target)]);
            fragment.set_entry_point("fragmentMain");
            let descriptor = GpuRenderPipelineDescriptor::new(&self.layout, &vertex);
            descriptor.set_label(label);
            descriptor.set_fragment(&fragment);
            descriptor.set_primitive(&self.primitive);
            descriptor.set_depth_stencil(&depth_state(depth));
            descriptor.set_multisample(&self.multisample);
            self.device.create_render_pipeline_async(&descriptor).unchecked_into()
        }
    }

    /// Starts every pipeline the board will ever use at once, and nothing compiles after
    /// startup: materials never vary per entity (instance data carries identity), so this fixed
    /// set covers the whole game. WebKit compiles serially and pays for every distinct module and
    /// pipeline state, so modules are shared and pipeline states kept to eight.
    ///
    /// This runs synchronously so the compiles are already under way when it returns (unlike
    /// an `async fn`, which would not start until polled); await `PendingPipelines::finish`.
    pub fn start_pipelines(
        device: &GpuDevice,
        canvas_format: GpuTextureFormat,
        sample_count: u32,
        shader_salt: u32,
        sources: &ShaderSources,
    ) -> Result<PendingPipelines, JsValue> {
        let frame_layout =
            bind_group_layout(device, "frame", &[uniform_entry(0, shader_stage::VERTEX | shader_stage::FRAGMENT)])?;
        let sampler_entry = layout_entry(0, shader_stage::FRAGMENT);
        let sampler = GpuSamplerBindingLayout::new();
        sampler.set_type(GpuSamplerBindingType::Filtering);
        sampler_entry.set_sampler(&sampler);
        let post_layout = bind_group_layout(
            device,
            "post",
            &[
                sampler_entry,
                texture_entry(1),
                texture_entry(2),
                texture_entry(3),
                uniform_entry(4, shader_stage::FRAGMENT),
            ],
        )?;
        let multisample = GpuMultisampleState::new();
        multisample.set_count(sample_count.max(1));
        let primitive = GpuPrimitiveState::new();
        primitive.set_topology(GpuPrimitiveTopology::TriangleList);
        primitive.set_cull_mode(GpuCullMode::Back);
        primitive.set_front_face(GpuFrontFace::Ccw);
        let scene = SceneBuilder { device, layout: pipeline_layout(device, &frame_layout), multisample, primitive };

        let module = |label: &str, code: &str| {
            let descriptor = GpuShaderModuleDescriptor::new(&salt_shader(code, shader_salt));
            descriptor.set_label(label);
            device.create_shader_module(&descriptor)
        };
        let neon_module = module("neon", &sources.neon);
        let ground_module = module("ground", &sources.ground);
        let road_module = module("road", &sources.road);
        let effect_module = module("effect", &sources.effect);
        let sprite_module = module("sprite", &sources.sprite);
        let post_module = module("post", &sources.post);

        let post_pipeline_layout = pipeline_layout(device, &post_layout);
        let post_primitive = GpuPrimitiveState::new();
        post_primitive.set_topology(GpuPrimitiveTopology::TriangleList);
        let post = |label: &str, format: GpuTextureFormat| -> Promise {
            let vertex = GpuVertexState::new(&post_module);
            vertex.set_entry_point("vertexMain");
            let fragment = GpuFragmentState::new(&post_module, &[nullable(GpuColorTargetState::new(format))]);
            fragment.set_entry_point("fragmentMain");
            let descriptor = GpuRenderPipelineDescriptor::new(&post_pipeline_layout, &vertex);
            descriptor.set_label(label);
            descriptor.set_fragment(&fragment);
            descriptor.set_primitive(&post_primitive);
            device.create_render_pipeline_async(&descriptor).unchecked_into()
        };

        let blend = premultiplied();
        let instance = instance_buffer();
        let instanced_flat = [flat_vertex_buffer(), instance.clone()];
        // Order: the `ScenePipeline` variants, then bloom and composite.
        let promises = [
            scene.start("neon", &neon_module, &[neon_vertex_buffer(), instance], DepthMode::Opaque, None),
            scene.start("ground", &ground_module, &[], DepthMode::Opaque, None),
            scene.start("road", &road_module, &[flat_vertex_buffer()], DepthMode::Opaque, None),
            scene.start("effect", &effect_module, &instanced_flat, DepthMode::Transparent, Some(&blend)),
            scene.start("health-bar", &effect_module, &instanced_flat, DepthMode::Overlay, None),
            scene.start("sprite", &sprite_module, &[sprite_buffer()], DepthMode::Transparent, Some(&blend)),
            post("bloom", HDR_FORMAT),
            post("composite", canvas_format),
        ];
        let list = Array::new();
        for promise in &promises {
            list.push(promise);
        }
        Ok(PendingPipelines { frame_layout, post_layout, all: Promise::all(&list) })
    }
}

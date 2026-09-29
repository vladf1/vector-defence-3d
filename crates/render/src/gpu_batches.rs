//! GPU side of `RenderBatches`: static geometry buffers, per-batch instance buffers, one
//! upload of each batch's used range per frame, and the draw order.
use wasm_bindgen::JsValue;
use web_sys::{GpuBuffer, GpuBufferDescriptor, GpuDevice, GpuQueue, GpuRenderPassEncoder};

use crate::geometry_kit::Mesh;
use crate::gpu_flags::{buffer_usage, f32_bytes};
use crate::gpu_pipelines::{GpuPipelines, INSTANCE_FLOATS, SPRITE_FLOATS, ScenePipeline};
use crate::render_batches::{
    DrawItem, INSTANCED_BATCHES, RenderBatches, TRANSPARENT_ORDER, batch_mesh, opaque_batches,
};

const FLOAT_BYTES: u32 = 4;
const QUAD_VERTICES: u32 = 6;

/// Static vertex data a batch draws per instance.
struct BatchGeometry {
    buffer: GpuBuffer,
    vertex_count: u32,
}

fn create_buffer(device: &GpuDevice, label: &str, bytes: usize, usage: u32) -> Result<GpuBuffer, JsValue> {
    let descriptor = GpuBufferDescriptor::new(bytes as u32, usage);
    descriptor.set_label(label);
    device.create_buffer(&descriptor)
}

pub struct GpuBatches {
    geometries: Vec<BatchGeometry>,
    instances: Vec<GpuBuffer>,
    uploaded: [u32; INSTANCED_BATCHES],
    /// Smoke, then glow.
    sprites: [GpuBuffer; 2],
    sprites_uploaded: [u32; 2],
}

impl GpuBatches {
    pub fn new(device: &GpuDevice, meshes: &[Mesh], batches: &RenderBatches) -> Result<Self, JsValue> {
        let queue = device.queue();
        let mut geometries = Vec::with_capacity(meshes.len());
        for mesh in meshes {
            let bytes = f32_bytes(&mesh.vertices);
            let buffer = create_buffer(device, "geometry", bytes.len(), buffer_usage::VERTEX | buffer_usage::COPY_DST)?;
            queue.write_buffer_with_u32_and_u8_slice(&buffer, 0, bytes)?;
            geometries.push(BatchGeometry { buffer, vertex_count: mesh.vertex_count });
        }
        let usage = buffer_usage::VERTEX | buffer_usage::COPY_DST;
        let mut instances = Vec::with_capacity(INSTANCED_BATCHES);
        for batch in batches.instanced() {
            let bytes = batch.capacity * INSTANCE_FLOATS * FLOAT_BYTES as usize;
            instances.push(create_buffer(device, batch.name, bytes, usage)?);
        }
        let sprite_bytes = |capacity: usize| capacity * SPRITE_FLOATS * FLOAT_BYTES as usize;
        let sprites = [
            create_buffer(device, batches.smoke.name, sprite_bytes(batches.smoke.capacity), usage)?,
            create_buffer(device, batches.glow.name, sprite_bytes(batches.glow.capacity), usage)?,
        ];
        Ok(GpuBatches { geometries, instances, uploaded: [0; INSTANCED_BATCHES], sprites, sprites_uploaded: [0; 2] })
    }

    /// Uploads every batch's used range (one `writeBuffer` per non-empty batch).
    pub fn upload(&mut self, queue: &GpuQueue, batches: &RenderBatches) {
        for (index, batch) in batches.instanced().iter().enumerate() {
            self.uploaded[index] = batch.size() as u32;
            if batch.size() > 0 {
                let _ =
                    queue.write_buffer_with_u32_and_u8_slice(&self.instances[index], 0, f32_bytes(batch.used_data()));
            }
        }
        for (slot, batch) in [&batches.smoke, &batches.glow].into_iter().enumerate() {
            self.sprites_uploaded[slot] = batch.size() as u32;
            if batch.size() > 0 {
                let _ = queue.write_buffer_with_u32_and_u8_slice(&self.sprites[slot], 0, f32_bytes(batch.used_data()));
            }
        }
    }

    /// Opaque neon parts; the board's ground and road draw between these and the rest.
    pub fn draw_opaque(&self, pass: &GpuRenderPassEncoder, pipelines: &GpuPipelines, batches: &RenderBatches) {
        let mut bound = None;
        for index in opaque_batches() {
            self.draw_instanced(pass, pipelines, batches, index, &mut bound);
        }
    }

    /// Health bars ignore depth, then the blended effect layers draw back to front by kind.
    pub fn draw_overlays(&self, pass: &GpuRenderPassEncoder, pipelines: &GpuPipelines, batches: &RenderBatches) {
        let mut bound = None;
        self.draw_instanced(pass, pipelines, batches, crate::render_batches::Batch::HealthBar as usize, &mut bound);
        let mut bound = None;
        for item in TRANSPARENT_ORDER {
            match item {
                DrawItem::Instanced(batch) => self.draw_instanced(pass, pipelines, batches, batch as usize, &mut bound),
                DrawItem::Smoke => self.draw_sprites(pass, pipelines, 0, &mut bound),
                DrawItem::Glow => self.draw_sprites(pass, pipelines, 1, &mut bound),
            }
        }
    }

    fn bind(
        pass: &GpuRenderPassEncoder,
        pipelines: &GpuPipelines,
        pipeline: ScenePipeline,
        bound: &mut Option<ScenePipeline>,
    ) {
        if *bound != Some(pipeline) {
            pass.set_pipeline(pipelines.scene(pipeline));
            *bound = Some(pipeline);
        }
    }

    fn draw_instanced(
        &self,
        pass: &GpuRenderPassEncoder,
        pipelines: &GpuPipelines,
        batches: &RenderBatches,
        index: usize,
        bound: &mut Option<ScenePipeline>,
    ) {
        let count = self.uploaded[index];
        if count == 0 {
            return;
        }
        Self::bind(pass, pipelines, batches.instanced()[index].pipeline, bound);
        let geometry = &self.geometries[batch_mesh(index)];
        pass.set_vertex_buffer(0, Some(&geometry.buffer));
        let bytes = count * INSTANCE_FLOATS as u32 * FLOAT_BYTES;
        pass.set_vertex_buffer_with_u32_and_u32(1, Some(&self.instances[index]), 0, bytes);
        pass.draw_with_instance_count(geometry.vertex_count, count);
    }

    fn draw_sprites(
        &self,
        pass: &GpuRenderPassEncoder,
        pipelines: &GpuPipelines,
        slot: usize,
        bound: &mut Option<ScenePipeline>,
    ) {
        let count = self.sprites_uploaded[slot];
        if count == 0 {
            return;
        }
        Self::bind(pass, pipelines, ScenePipeline::Sprite, bound);
        let bytes = count * SPRITE_FLOATS as u32 * FLOAT_BYTES;
        pass.set_vertex_buffer_with_u32_and_u32(0, Some(&self.sprites[slot]), 0, bytes);
        pass.draw_with_instance_count(QUAD_VERTICES, count);
    }

    pub fn dispose(&self) {
        for buffer in self.instances.iter().chain(&self.sprites) {
            buffer.destroy();
        }
        for geometry in &self.geometries {
            geometry.buffer.destroy();
        }
    }
}

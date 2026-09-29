//! CPU side of the camera-facing sprite batches.
use crate::gpu_pipelines::SPRITE_FLOATS;

/// Camera-facing quads drawn as one instanced draw call. Each sprite is
/// `x, y, z, rotation | width, height, shape, mode | r, g, b, a` (linear color).
#[derive(Clone, Debug)]
pub struct SpriteBatch {
    pub name: &'static str,
    pub capacity: usize,
    shader_mode: f32,
    data: Vec<f32>,
    count: usize,
}

impl SpriteBatch {
    /// `shader_mode` is written to every sprite's shape w: the sprite module's look selector
    /// (see `sprite_mode`).
    pub fn new(name: &'static str, capacity: usize, shader_mode: f32) -> Self {
        SpriteBatch { name, capacity, shader_mode, data: vec![0.0; capacity * SPRITE_FLOATS], count: 0 }
    }

    pub fn size(&self) -> usize {
        self.count
    }

    pub fn begin(&mut self) {
        self.count = 0;
    }

    pub fn used_data(&self) -> &[f32] {
        &self.data[..self.count * SPRITE_FLOATS]
    }

    #[allow(clippy::too_many_arguments)]
    pub fn push(
        &mut self,
        x: f32,
        y: f32,
        z: f32,
        rotation: f32,
        width: f32,
        height: f32,
        shape: f32,
        red: f32,
        green: f32,
        blue: f32,
        alpha: f32,
    ) {
        let index = self.count;
        if index >= self.capacity {
            return;
        }
        self.data[index * SPRITE_FLOATS..(index + 1) * SPRITE_FLOATS].copy_from_slice(&[
            x,
            y,
            z,
            rotation,
            width,
            height,
            shape,
            self.shader_mode,
            red,
            green,
            blue,
            alpha,
        ]);
        self.count = index + 1;
    }
}

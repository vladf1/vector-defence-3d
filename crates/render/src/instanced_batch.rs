//! CPU side of a fixed-capacity instanced draw, refilled every frame.
use crate::gpu_pipelines::{INSTANCE_FLOATS, ScenePipeline};

/// Per-instance layout: column-major transform (16), linear RGB tint + mode (4), extras (4).
pub const INSTANCE_STRIDE: usize = INSTANCE_FLOATS;
const TINT_OFFSET: usize = 16;
const EXTRA_OFFSET: usize = 20;

/// A fixed-capacity instanced draw refilled every frame. Instances are written straight
/// into one preallocated array and uploaded with a single write of the used range (see
/// `GpuBatches`), so steady-state drawing allocates nothing.
#[derive(Clone, Debug)]
pub struct InstancedBatch {
    pub name: &'static str,
    pub pipeline: ScenePipeline,
    pub capacity: usize,
    shader_mode: f32,
    data: Vec<f32>,
    count: usize,
}

impl InstancedBatch {
    /// `shader_mode` is written to every instance's tint w: the neon or effect module's look
    /// selector (see `neon_mode` and `effect_mode`).
    pub fn new(name: &'static str, pipeline: ScenePipeline, shader_mode: f32, capacity: usize) -> Self {
        InstancedBatch { name, pipeline, capacity, shader_mode, data: vec![0.0; capacity * INSTANCE_STRIDE], count: 0 }
    }

    pub fn size(&self) -> usize {
        self.count
    }

    pub fn begin(&mut self) {
        self.count = 0;
    }

    /// The instances written since `begin`, ready to upload.
    pub fn used_data(&self) -> &[f32] {
        &self.data[..self.count * INSTANCE_STRIDE]
    }

    /// Reserves the next instance slot, or `None` when the batch is full.
    fn next_slot(&mut self) -> Option<(usize, &mut [f32])> {
        let index = self.count;
        if index >= self.capacity {
            return None;
        }
        self.count = index + 1;
        Some((index, &mut self.data[index * INSTANCE_STRIDE..(index + 1) * INSTANCE_STRIDE]))
    }

    /// Translation, rotation about world up (+Y), and axis scale.
    #[allow(clippy::too_many_arguments)]
    pub fn push_yaw(
        &mut self,
        x: f32,
        y: f32,
        z: f32,
        yaw: f32,
        scale_x: f32,
        scale_y: f32,
        scale_z: f32,
        red: f32,
        green: f32,
        blue: f32,
    ) -> Option<usize> {
        let mode = self.shader_mode;
        let (index, m) = self.next_slot()?;
        let (sin, cos) = yaw.sin_cos();
        m[..16].copy_from_slice(&[
            cos * scale_x,
            0.0,
            -sin * scale_x,
            0.0,
            0.0,
            scale_y,
            0.0,
            0.0,
            sin * scale_z,
            0.0,
            cos * scale_z,
            0.0,
            x,
            y,
            z,
            1.0,
        ]);
        finish_instance(m, mode, red, green, blue);
        Some(index)
    }

    /// Translation, unit-quaternion rotation, and axis scale.
    #[allow(clippy::too_many_arguments)]
    pub fn push_quaternion(
        &mut self,
        x: f32,
        y: f32,
        z: f32,
        qx: f32,
        qy: f32,
        qz: f32,
        qw: f32,
        scale_x: f32,
        scale_y: f32,
        scale_z: f32,
        red: f32,
        green: f32,
        blue: f32,
    ) -> Option<usize> {
        let mode = self.shader_mode;
        let (index, m) = self.next_slot()?;
        let (x2, y2, z2) = (qx + qx, qy + qy, qz + qz);
        let (xx, xy, xz) = (qx * x2, qx * y2, qx * z2);
        let (yy, yz, zz) = (qy * y2, qy * z2, qz * z2);
        let (wx, wy, wz) = (qw * x2, qw * y2, qw * z2);
        m[..16].copy_from_slice(&[
            (1.0 - (yy + zz)) * scale_x,
            (xy + wz) * scale_x,
            (xz - wy) * scale_x,
            0.0,
            (xy - wz) * scale_y,
            (1.0 - (xx + zz)) * scale_y,
            (yz + wx) * scale_y,
            0.0,
            (xz + wy) * scale_z,
            (yz - wx) * scale_z,
            (1.0 - (xx + yy)) * scale_z,
            0.0,
            x,
            y,
            z,
            1.0,
        ]);
        finish_instance(m, mode, red, green, blue);
        Some(index)
    }

    /// Arbitrary orthogonal basis: columns are the world-space images of the local X, Y,
    /// and Z axes (lengths are the axis scales).
    #[allow(clippy::too_many_arguments)]
    pub fn push_basis(
        &mut self,
        x: f32,
        y: f32,
        z: f32,
        axis_x: [f32; 3],
        axis_y: [f32; 3],
        axis_z: [f32; 3],
        red: f32,
        green: f32,
        blue: f32,
    ) -> Option<usize> {
        let mode = self.shader_mode;
        let (index, m) = self.next_slot()?;
        m[..16].copy_from_slice(&[
            axis_x[0], axis_x[1], axis_x[2], 0.0, axis_y[0], axis_y[1], axis_y[2], 0.0, axis_z[0], axis_z[1],
            axis_z[2], 0.0, x, y, z, 1.0,
        ]);
        finish_instance(m, mode, red, green, blue);
        Some(index)
    }

    /// Sets one extra component of an instance returned by a push (ignored for `None`).
    pub fn set_extra(&mut self, index: Option<usize>, component: usize, value: f32) {
        if let Some(index) = index {
            self.data[index * INSTANCE_STRIDE + EXTRA_OFFSET + component] = value;
        }
    }
}

fn finish_instance(m: &mut [f32], mode: f32, red: f32, green: f32, blue: f32) {
    m[TINT_OFFSET..].copy_from_slice(&[red, green, blue, mode, 0.0, 0.0, 0.0, 0.0]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pushes_fill_until_capacity() {
        let mut batch = InstancedBatch::new("test", ScenePipeline::Neon, 1.0, 2);
        let first = batch.push_yaw(1.0, 2.0, 3.0, 0.0, 2.0, 3.0, 4.0, 0.5, 0.6, 0.7);
        assert_eq!(first, Some(0));
        batch.set_extra(first, 1, 9.0);
        let data = batch.used_data();
        assert_eq!(&data[..16], &[2.0, 0.0, -0.0, 0.0, 0.0, 3.0, 0.0, 0.0, 0.0, 0.0, 4.0, 0.0, 1.0, 2.0, 3.0, 1.0]);
        assert_eq!(&data[16..], &[0.5, 0.6, 0.7, 1.0, 0.0, 9.0, 0.0, 0.0]);
        assert_eq!(batch.push_quaternion(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0), Some(1));
        assert_eq!(batch.push_basis(0.0, 0.0, 0.0, [1.0; 3], [1.0; 3], [1.0; 3], 0.0, 0.0, 0.0), None);
        batch.set_extra(None, 0, 1.0);
        assert_eq!(batch.size(), 2);
        batch.begin();
        assert!(batch.used_data().is_empty());
    }
}

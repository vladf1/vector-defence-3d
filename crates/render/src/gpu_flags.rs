//! WebGPU usage/visibility bit flags (fixed by the spec), as plain constants.

pub mod buffer_usage {
    pub const COPY_DST: u32 = 0x0008;
    pub const INDEX: u32 = 0x0010;
    pub const VERTEX: u32 = 0x0020;
    pub const UNIFORM: u32 = 0x0040;
}

pub mod texture_usage {
    pub const COPY_DST: u32 = 0x02;
    pub const TEXTURE_BINDING: u32 = 0x04;
    pub const RENDER_ATTACHMENT: u32 = 0x10;
}

pub mod shader_stage {
    pub const VERTEX: u32 = 0x1;
    pub const FRAGMENT: u32 = 0x2;
}

/// The bytes of an `f32` slice, for `GPUQueue.writeBuffer` (wasm-bindgen passes a view of
/// Wasm memory, so nothing is copied on the Rust side).
pub fn f32_bytes(values: &[f32]) -> &[u8] {
    // SAFETY: f32 has no padding or invalid bit patterns, u8 has alignment 1, and the byte
    // length is exactly the slice's size in memory.
    unsafe { std::slice::from_raw_parts(values.as_ptr().cast::<u8>(), std::mem::size_of_val(values)) }
}

/// The bytes of a `u32` slice (index buffers).
pub fn u32_bytes(values: &[u32]) -> &[u8] {
    // SAFETY: as for `f32_bytes`.
    unsafe { std::slice::from_raw_parts(values.as_ptr().cast::<u8>(), std::mem::size_of_val(values)) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_views_are_little_endian_copies() {
        assert_eq!(f32_bytes(&[1.0]), &1.0f32.to_le_bytes());
        assert_eq!(u32_bytes(&[1, 2]).len(), 8);
    }
}

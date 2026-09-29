//! Seeded smoke-puff blob parameters (the former canvas atlas), baked into the shaders.

const PUFF_CELL_SIZE: f64 = 128.0;
const PUFF_CELLS: usize = 4;
pub const PUFF_BLOB_COUNT: usize = 34;

/// A 32-bit LCG (`Math.imul(state, 1664525) + 1013904223`), matching the original seed stream.
struct SeededRandom(u32);

impl SeededRandom {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        self.0 as f64 / 4_294_967_296.0
    }
}

/// The soft, lumpy smoke puffs of the former 2x2 canvas atlas, as seeded blob parameters
/// (4 floats per blob: cell-local x, y in canvas pixels, radius, alpha; cells in reading
/// order). The shaders evaluate them per pixel (`puffCoverage`) instead of sampling a
/// texture: smoke sprites use a cell each, and scorch decals reuse cell 2 as a noisy mask.
pub fn create_puff_blobs() -> Vec<f64> {
    let mut random = SeededRandom(0x5eed);
    let center = PUFF_CELL_SIZE / 2.0;
    let mut blobs = Vec::with_capacity(PUFF_CELLS * PUFF_BLOB_COUNT * 4);
    for _cell in 0..PUFF_CELLS {
        for _blob in 0..PUFF_BLOB_COUNT {
            let angle = random.next() * std::f64::consts::PI * 2.0;
            let distance = random.next().powf(0.7) * PUFF_CELL_SIZE * 0.26;
            let radius = PUFF_CELL_SIZE * (0.08 + random.next() * 0.17);
            let alpha = 0.16 + random.next() * 0.22;
            blobs.extend([center + angle.cos() * distance, center + angle.sin() * distance, radius, alpha]);
        }
    }
    blobs
}

//! The simulation's `Math.random()`: one process-wide generator (Wasm is single threaded).
//! Seeded from the page at startup; render scripts reseed it for repeatable frames.
use std::cell::Cell;

thread_local! {
    static STATE: Cell<u64> = const { Cell::new(0x9E37_79B9_7F4A_7C15) };
}

/// Restarts the sequence; any seed (including 0) gives a usable stream.
pub fn seed(value: u64) {
    STATE.with(|state| state.set(value ^ 0x9E37_79B9_7F4A_7C15));
}

/// A uniform value in `[0, 1)` (SplitMix64, 53 bits).
pub fn random() -> f64 {
    STATE.with(|state| {
        let next = state.get().wrapping_add(0x9E37_79B9_7F4A_7C15);
        state.set(next);
        let mut z = next;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        (z >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    })
}

pub fn random_range(min: f64, max: f64) -> f64 {
    min + random() * (max - min)
}

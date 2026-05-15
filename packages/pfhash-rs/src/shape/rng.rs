//! Tiny deterministic RNG for shape hill-climb. Uses xoshiro256** + SplitMix64
//! to keep the crate dependency-free.
//!
//! NOTE: the byte sequence produced by shape encoders depends on RNG draws,
//! so output bytes will NOT match Python's reference vectors (Python uses
//! numpy's PCG64-DXSM). The Codec / SPEC contract is preserved regardless —
//! Python and Rust shape bytes both round-trip through the SPEC decoder.

#[derive(Debug, Clone)]
pub struct Rng {
    state: [u64; 4],
    // Cached spare for Box-Muller normal()
    cached_normal: Option<f64>,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        // Splat the seed via SplitMix64 to fill 256 state bits.
        let mut sm = SplitMix64 { state: seed };
        Self {
            state: [sm.next(), sm.next(), sm.next(), sm.next()],
            cached_normal: None,
        }
    }

    pub fn next_u64(&mut self) -> u64 {
        // xoshiro256** algorithm.
        let result = self.state[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = self.state[1] << 17;
        self.state[2] ^= self.state[0];
        self.state[3] ^= self.state[1];
        self.state[1] ^= self.state[2];
        self.state[0] ^= self.state[3];
        self.state[2] ^= t;
        self.state[3] = self.state[3].rotate_left(45);
        result
    }

    /// Uniform float in `[0.0, 1.0)` with full 53-bit mantissa precision.
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// Uniform integer in `[low, high)`. `high > low` required.
    pub fn range(&mut self, low: i64, high: i64) -> i64 {
        debug_assert!(high > low);
        let span = (high - low) as u64;
        let scaled = self.next_u64() % span;
        low + scaled as i64
    }

    /// Inclusive uniform integer in `[low, high]`.
    pub fn range_inclusive(&mut self, low: i64, high: i64) -> i64 {
        self.range(low, high + 1)
    }

    /// Standard normal (mean=0, std=1) via Box-Muller. Caches the second draw.
    pub fn normal(&mut self) -> f64 {
        if let Some(v) = self.cached_normal.take() {
            return v;
        }
        // Polar Box-Muller (rejection)
        loop {
            let u1 = self.next_f64() * 2.0 - 1.0;
            let u2 = self.next_f64() * 2.0 - 1.0;
            let s = u1 * u1 + u2 * u2;
            if s < 1.0 && s > 0.0 {
                let factor = (-2.0 * s.ln() / s).sqrt();
                self.cached_normal = Some(u2 * factor);
                return u1 * factor;
            }
        }
    }

    /// Pick one of two values: -1 or +1.
    pub fn sign(&mut self) -> i32 {
        if self.next_u64() & 1 == 0 { -1 } else { 1 }
    }
}

struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

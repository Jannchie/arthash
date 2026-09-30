//! sRGB ↔ linear RGB. SPEC §4.1.

use std::sync::OnceLock;

/// Forward transform on uint8 input → f32 linear in [0, 1].
pub fn srgb_u8_to_linear(c: u8) -> f32 {
    let s = c as f32 / 255.0;
    if s <= 0.04045 {
        s / 12.92
    } else {
        ((s + 0.055) / 1.055).powf(2.4)
    }
}

/// Element-wise forward transform on an RGB u8 slice → flat f32 linear vec.
pub fn srgb_u8_slice_to_linear(rgb_u8: &[u8]) -> Vec<f32> {
    rgb_u8.iter().map(|&c| srgb_u8_to_linear(c)).collect()
}

/// Inverse transform on f32 linear → u8 sRGB.
///
/// Table-driven and bit-identical to [`linear_to_srgb_u8_exact`] (the SPEC
/// formula): see [`SrgbLut`]. Decoders call this once per output channel, so
/// the per-call `powf` it replaces dominated shape-mode decode time.
#[inline]
pub fn linear_to_srgb_u8(lin: f32) -> u8 {
    srgb_lut().lookup(lin)
}

/// Reference inverse transform — the SPEC formula, one `powf` per call.
/// [`linear_to_srgb_u8`] is derived from (and must stay equal to) this.
pub fn linear_to_srgb_u8_exact(lin: f32) -> u8 {
    let c = lin.clamp(0.0, 1.0);
    let s = if c <= 0.0031308 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (s * 255.0 + 0.5).clamp(0.0, 255.0) as u8
}

/// Bucket count over `[0, 1]`. A power of two, so every bucket's lower edge
/// `b / BUCKETS` is an exact f32 and `(x * BUCKETS) as usize` is exact.
const BUCKETS: usize = 4096;

/// Exact step-function form of [`linear_to_srgb_u8_exact`].
///
/// The reference is a monotone staircase from `[0, 1]` onto `0..=255`, so it
/// is fully described by its 255 jump points: `thresh[k]` is the smallest f32
/// whose output is `≥ k`. Those are found by bisecting the reference itself
/// over f32 bit patterns — so the table reproduces whatever this platform's
/// `powf` returns, last bit included, rather than an idealised curve.
/// `bucket_start[b]` is the output at bucket `b`'s lower edge; a lookup starts
/// there and advances past the thresholds inside the bucket — at most one in
/// practice (the curve's steepest slope is ≈0.8 steps per bucket), checked
/// branch-free for two, with a loop fallback that never runs.
struct SrgbLut {
    /// `thresh[k]` for `k ∈ 1..=255`; `thresh[0] = 0`, and the `+∞` padding
    /// at 256/257 caps lookups at 255 without bounds checks on `k`.
    thresh: [f32; 258],
    bucket_start: [u8; BUCKETS],
}

impl SrgbLut {
    fn build() -> Self {
        let mut thresh = [f32::INFINITY; 258];
        thresh[0] = 0.0;
        // Positive finite f32s order the same as their bit patterns, so a
        // bisection over bits finds the exact first input reaching `k`.
        let mut lo_bits = 0.0f32.to_bits();
        for (k, slot) in thresh.iter_mut().enumerate().take(256).skip(1) {
            let mut lo = lo_bits; // f(lo) < k
            let mut hi = 1.0f32.to_bits(); // f(1.0) = 255 ≥ k
            while hi - lo > 1 {
                let mid = lo + (hi - lo) / 2;
                if linear_to_srgb_u8_exact(f32::from_bits(mid)) as usize >= k {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            *slot = f32::from_bits(hi);
            lo_bits = lo;
        }
        let mut bucket_start = [0u8; BUCKETS];
        let mut k = 0usize;
        for (b, slot) in bucket_start.iter_mut().enumerate() {
            let edge = b as f32 / BUCKETS as f32;
            while k < 255 && thresh[k + 1] <= edge {
                k += 1;
            }
            *slot = k as u8;
        }
        Self { thresh, bucket_start }
    }

    #[inline]
    fn lookup(&self, lin: f32) -> u8 {
        // Mirrors the reference's clamp. NaN survives `clamp`, maps to bucket
        // 0 and fails every `>=`, giving 0 — same as the reference.
        let c = lin.clamp(0.0, 1.0);
        let b = ((c * BUCKETS as f32) as usize).min(BUCKETS - 1);
        let s = self.bucket_start[b] as usize;
        // Thresholds are sorted, so these two compares count the jumps in
        // `(s, s + 2]` that `c` has passed.
        let mut k = s + (c >= self.thresh[s + 1]) as usize + (c >= self.thresh[s + 2]) as usize;
        while k < 255 && c >= self.thresh[k + 1] {
            k += 1;
        }
        k as u8
    }
}

fn srgb_lut() -> &'static SrgbLut {
    static LUT: OnceLock<SrgbLut> = OnceLock::new();
    LUT.get_or_init(SrgbLut::build)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lut_matches_reference_at_every_jump() {
        let lut = srgb_lut();
        for k in 1..256 {
            let t = lut.thresh[k];
            let below = f32::from_bits(t.to_bits() - 1);
            assert_eq!(linear_to_srgb_u8(t), linear_to_srgb_u8_exact(t), "at thresh[{k}]");
            assert_eq!(linear_to_srgb_u8(below), linear_to_srgb_u8_exact(below), "below thresh[{k}]");
            assert_eq!(linear_to_srgb_u8_exact(t) as usize, k);
        }
    }

    #[test]
    fn lut_matches_reference_on_dense_sweep_and_specials() {
        // Every 997th f32 bit pattern in [0, 1] (~1M points), plus
        // out-of-range and non-finite inputs.
        let one = 1.0f32.to_bits();
        let mut bits = 0u32;
        while bits <= one {
            let x = f32::from_bits(bits);
            assert_eq!(linear_to_srgb_u8(x), linear_to_srgb_u8_exact(x), "x = {x:e}");
            bits += 997;
        }
        for x in [
            -1.0, -0.0, 1.0, 1.5, f32::MIN_POSITIVE, f32::MAX, f32::MIN,
            f32::INFINITY, f32::NEG_INFINITY, f32::NAN, 0.0031308, 0.5,
        ] {
            assert_eq!(linear_to_srgb_u8(x), linear_to_srgb_u8_exact(x), "x = {x:e}");
        }
    }
}

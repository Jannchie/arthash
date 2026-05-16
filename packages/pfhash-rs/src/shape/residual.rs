//! Per-pixel residual map + weighted sampling.
//!
//! The residual at pixel `(x, y)` is `||target - canvas||²` (sum of squared
//! per-channel deltas). Sampling proportional to residual concentrates init
//! candidates on regions the current canvas hasn't fit well, which lets us
//! reach equal-or-better quality with far fewer Stage-1 random samples.
//!
//! Storage is a flat cumulative-distribution function `cdf[i] = Σ_{j<i} r_j`
//! of length `h·w + 1`, where pixels are scanned in row-major order. Sampling
//! is `O(log(h·w))` via binary search on the CDF.

use super::rng::Rng;

pub struct Residual {
    pub h: usize,
    pub w: usize,
    cdf: Vec<f32>,
}

impl Residual {
    /// Build from a `(target, canvas)` pair. O(h·w).
    pub fn build(target: &[f32], canvas: &[f32], h: u32, w: u32) -> Self {
        let h_us = h as usize;
        let w_us = w as usize;
        let mut me = Self {
            h: h_us,
            w: w_us,
            cdf: vec![0.0; h_us * w_us + 1],
        };
        me.rebuild(target, canvas);
        me
    }

    /// Full rebuild — call after every `apply_*` commit. Target is constant
    /// across the fit, so the caller passes the same `target` slice each time.
    pub fn rebuild(&mut self, target: &[f32], canvas: &[f32]) {
        let mut acc = 0.0f32;
        let n = self.h * self.w;
        for i in 0..n {
            let p = i * 3;
            let dr = target[p] - canvas[p];
            let dg = target[p + 1] - canvas[p + 1];
            let db = target[p + 2] - canvas[p + 2];
            acc += dr * dr + dg * dg + db * db;
            self.cdf[i + 1] = acc;
        }
    }

    /// Sample one pixel — half uniform, half proportional to residual.
    ///
    /// Pure residual-weighted sampling clusters too aggressively when residual
    /// is concentrated in a single region (e.g. solid-color blocks), starving
    /// the rest of the canvas of init attempts. Blending uniform 50/50 keeps
    /// the discovery property of the original uniform init while still biasing
    /// toward unfit regions on smoother inputs.
    pub fn sample(&self, rng: &mut Rng) -> (i32, i32) {
        let n = self.h * self.w;
        let total = self.cdf[n];
        // Uniform half (also the fallback path when residual sums to zero).
        if total <= 0.0 || (rng.next_f64() as f32) < 0.5 {
            let i = rng.range(0, n as i64) as usize;
            return ((i % self.w) as i32, (i / self.w) as i32);
        }
        let u = (rng.next_f64() as f32) * total;
        let idx = self.cdf[1..].partition_point(|&v| v <= u).min(n - 1);
        ((idx % self.w) as i32, (idx / self.w) as i32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_falls_back_to_uniform_when_zero() {
        let (h, w) = (4u32, 4u32);
        let buf = vec![0.5f32; (h * w * 3) as usize];
        let r = Residual::build(&buf, &buf, h, w);
        let mut rng = Rng::new(42);
        // Should not panic and should return in-range coordinates.
        for _ in 0..100 {
            let (x, y) = r.sample(&mut rng);
            assert!(x >= 0 && x < w as i32);
            assert!(y >= 0 && y < h as i32);
        }
    }

    #[test]
    fn sample_concentrates_on_high_residual_pixel() {
        // With 50/50 uniform blend, the high-residual pixel still gets the
        // residual-weighted half (≈50%) plus its uniform share (1/n), while
        // every other pixel only gets 1/n. So the hot pixel should dominate
        // the sample distribution at small canvas sizes.
        let (h, w) = (8u32, 8u32);
        let target = vec![0.0f32; (h * w * 3) as usize];
        let mut canvas = vec![0.0f32; (h * w * 3) as usize];
        let hot = (5, 3);
        let p = ((hot.1 * w as i32 + hot.0) * 3) as usize;
        canvas[p] = 1.0;
        canvas[p + 1] = 1.0;
        canvas[p + 2] = 1.0;
        let r = Residual::build(&target, &canvas, h, w);
        let mut rng = Rng::new(7);
        let trials = 4000;
        let mut hits = 0;
        for _ in 0..trials {
            if r.sample(&mut rng) == hot {
                hits += 1;
            }
        }
        // Expected ≈ 0.5 + 0.5/64 ≈ 51%. Allow generous slack for RNG noise.
        let rate = hits as f64 / trials as f64;
        assert!(rate > 0.4 && rate < 0.6, "hot-pixel rate {rate} out of range");
    }

    #[test]
    fn rebuild_picks_up_canvas_change() {
        // After rebuild the new hot pixel should dominate; before it shouldn't.
        let (h, w) = (8u32, 8u32);
        let target = vec![0.0f32; (h * w * 3) as usize];
        let mut canvas = vec![0.0f32; (h * w * 3) as usize];
        let mut r = Residual::build(&target, &canvas, h, w);
        let mut rng = Rng::new(7);
        let hot = (2, 6);
        let p = ((hot.1 * w as i32 + hot.0) * 3) as usize;
        canvas[p] = 1.0;
        canvas[p + 1] = 1.0;
        canvas[p + 2] = 1.0;
        r.rebuild(&target, &canvas);
        let trials = 4000;
        let mut hits = 0;
        for _ in 0..trials {
            if r.sample(&mut rng) == hot {
                hits += 1;
            }
        }
        let rate = hits as f64 / trials as f64;
        assert!(rate > 0.4 && rate < 0.6, "post-rebuild rate {rate} out of range");
    }
}

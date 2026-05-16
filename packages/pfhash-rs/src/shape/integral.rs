//! Per-row prefix-sum tables for the five quantities the ΔSSE closed form
//! needs. Lets `eval_circle` / `eval_triangle` skip the per-pixel inner
//! loop and instead compute each row's contribution via two array lookups
//! plus a subtraction — turning O(bbox_area) evaluation into O(bbox_height).
//!
//! ## What's stored
//!
//! For each row `y` and column `x ∈ [0, w]`, we keep the cumulative sum of
//! columns `[0, x)` for every (channel, series) pair:
//!
//! * `t [y][x][c] = Σ_{j<x} target[y, j, c]`
//! * `t2[y][x][c] = Σ_{j<x} target[y, j, c]²`
//! * `c [y][x][c] = Σ_{j<x} canvas[y, j, c]`
//! * `c2[y][x][c] = Σ_{j<x} canvas[y, j, c]²`
//! * `tc[y][x][c] = Σ_{j<x} target[y, j, c] · canvas[y, j, c]`
//!
//! A span sum on row `y` from `[xL, xR]` (inclusive) is `t[y][xR+1] - t[y][xL]`.
//!
//! Target tables (`t`, `t2`) are built once per `fit_*` since target is
//! constant. Canvas-dependent tables (`c`, `c2`, `tc`) are rebuilt for the
//! affected row range after each `apply_*` commit.
//!
//! ## Numerical equivalence (caveat)
//!
//! The prefix-sum approach accumulates `target` and `canvas` left-to-right
//! across the entire row, then subtracts to extract a span — whereas the
//! original `Accum::push` only sums values inside the span. The two are
//! algebraically identical but **not** bit-equal in IEEE-754 f64 due to
//! reassociation. The relative error stays below `~n·ε_f64 ≈ 10⁻¹⁴`,
//! which is far below the f32 ΔSSE resolution `≈ 10⁻⁷` — so the f32 result
//! matches the original in all non-near-tie cases.

use super::raster::{EvalResult, ShapeSums};
#[cfg(feature = "bench-counters")]
use super::raster::counters;

/// All five row-wise prefix sums.
///
/// Layout: each `Vec<f64>` has length `h * row_stride` where
/// `row_stride = (w + 1) * 3`. Indexed as `y * row_stride + x * 3 + c`.
/// The `x = 0` slot is always zero (empty prefix); valid lookups range
/// over `x ∈ [0, w]`.
pub struct Integral {
    pub h: usize,
    pub w: usize,
    row_stride: usize,
    t: Vec<f64>,
    t2: Vec<f64>,
    c: Vec<f64>,
    c2: Vec<f64>,
    tc: Vec<f64>,
}

impl Integral {
    /// Build tables for a fresh `target` + `canvas` pair. O(h·w).
    pub fn build(target: &[f32], canvas: &[f32], h: u32, w: u32) -> Self {
        let h_us = h as usize;
        let w_us = w as usize;
        let row_stride = (w_us + 1) * 3;
        let total = h_us * row_stride;
        let mut me = Self {
            h: h_us,
            w: w_us,
            row_stride,
            t: vec![0.0; total],
            t2: vec![0.0; total],
            c: vec![0.0; total],
            c2: vec![0.0; total],
            tc: vec![0.0; total],
        };
        for y in 0..h_us {
            me.rebuild_row(target, canvas, y);
        }
        me
    }

    /// Rebuild canvas-dependent series for rows `[ymin, ymax]` (inclusive).
    /// Target series (`t`, `t2`) are NOT touched — they never change.
    /// Caller is responsible for clamping ymin/ymax to `[0, h-1]`.
    pub fn update_canvas_rows(
        &mut self,
        target: &[f32],
        canvas: &[f32],
        ymin: i32,
        ymax: i32,
    ) {
        let lo = ymin.max(0) as usize;
        let hi = (ymax as usize).min(self.h.saturating_sub(1));
        for y in lo..=hi {
            self.rebuild_row_canvas_only(target, canvas, y);
        }
    }

    /// Initial build of all five series for one row.
    fn rebuild_row(&mut self, target: &[f32], canvas: &[f32], y: usize) {
        let base = y * self.row_stride;
        // x = 0 slot already zero.
        let mut acc_t = [0.0f64; 3];
        let mut acc_t2 = [0.0f64; 3];
        let mut acc_c = [0.0f64; 3];
        let mut acc_c2 = [0.0f64; 3];
        let mut acc_tc = [0.0f64; 3];
        for x in 0..self.w {
            let pix = (y * self.w + x) * 3;
            for ch in 0..3 {
                let tv = target[pix + ch] as f64;
                let cv = canvas[pix + ch] as f64;
                acc_t[ch] += tv;
                acc_t2[ch] += tv * tv;
                acc_c[ch] += cv;
                acc_c2[ch] += cv * cv;
                acc_tc[ch] += tv * cv;
            }
            let slot = base + (x + 1) * 3;
            for ch in 0..3 {
                self.t[slot + ch] = acc_t[ch];
                self.t2[slot + ch] = acc_t2[ch];
                self.c[slot + ch] = acc_c[ch];
                self.c2[slot + ch] = acc_c2[ch];
                self.tc[slot + ch] = acc_tc[ch];
            }
        }
    }

    /// Rebuild only canvas-dependent series for one row (T, T² are static).
    fn rebuild_row_canvas_only(&mut self, target: &[f32], canvas: &[f32], y: usize) {
        let base = y * self.row_stride;
        let mut acc_c = [0.0f64; 3];
        let mut acc_c2 = [0.0f64; 3];
        let mut acc_tc = [0.0f64; 3];
        // Reset x=0 slot (already zero from build) — defensive.
        for ch in 0..3 {
            self.c[base + ch] = 0.0;
            self.c2[base + ch] = 0.0;
            self.tc[base + ch] = 0.0;
        }
        for x in 0..self.w {
            let pix = (y * self.w + x) * 3;
            for ch in 0..3 {
                let tv = target[pix + ch] as f64;
                let cv = canvas[pix + ch] as f64;
                acc_c[ch] += cv;
                acc_c2[ch] += cv * cv;
                acc_tc[ch] += tv * cv;
            }
            let slot = base + (x + 1) * 3;
            for ch in 0..3 {
                self.c[slot + ch] = acc_c[ch];
                self.c2[slot + ch] = acc_c2[ch];
                self.tc[slot + ch] = acc_tc[ch];
            }
        }
    }

    /// Span-difference: add row `y`'s sums over columns `[xL, xR]` into the
    /// supplied accumulators. Caller ensures `0 ≤ xL ≤ xR < w`.
    #[inline]
    fn add_span(
        &self,
        y: usize,
        x_l: usize,
        x_r: usize,
        s_t: &mut [f64; 3],
        s_c: &mut [f64; 3],
        s_t2: &mut [f64; 3],
        s_c2: &mut [f64; 3],
        s_tc: &mut [f64; 3],
    ) {
        let base = y * self.row_stride;
        let lo = base + x_l * 3;
        let hi = base + (x_r + 1) * 3;
        for ch in 0..3 {
            s_t[ch] += self.t[hi + ch] - self.t[lo + ch];
            s_t2[ch] += self.t2[hi + ch] - self.t2[lo + ch];
            s_c[ch] += self.c[hi + ch] - self.c[lo + ch];
            s_c2[ch] += self.c2[hi + ch] - self.c2[lo + ch];
            s_tc[ch] += self.tc[hi + ch] - self.tc[lo + ch];
        }
    }
}

/// Largest non-negative integer `x` with `x² ≤ n`. Returns 0 for `n ≤ 0`.
#[inline]
fn isqrt_i64(n: i64) -> i64 {
    if n < 2 {
        return n.max(0);
    }
    let mut x = (n as f64).sqrt() as i64;
    while x > 0 && x.saturating_mul(x) > n {
        x -= 1;
    }
    while (x + 1).saturating_mul(x + 1) <= n {
        x += 1;
    }
    x
}

#[inline]
fn div_floor_i64(a: i64, b: i64) -> i64 {
    let q = a / b;
    let r = a % b;
    if r != 0 && (r < 0) != (b < 0) {
        q - 1
    } else {
        q
    }
}

#[inline]
fn div_ceil_i64(a: i64, b: i64) -> i64 {
    let q = a / b;
    let r = a % b;
    if r != 0 && (r > 0) == (b > 0) {
        q + 1
    } else {
        q
    }
}

/// Circle ΔSSE via row-wise integral lookup. O(2r+1) rows × O(1) per row.
pub fn eval_circle_integral(
    integral: &Integral,
    h: u32,
    w: u32,
    cx: i32,
    cy: i32,
    r: i32,
    alpha: f32,
    palette: Option<&[f32]>,
) -> EvalResult {
    #[cfg(feature = "bench-counters")]
    counters::EVAL_CIRCLE.with(|c| c.set(c.get() + 1));
    collect_circle_sums_integral(integral, h, w, cx, cy, r).finalize(alpha, palette)
}

/// Collect per-channel sums for a circle via the integral path — the
/// expensive part of `eval_circle_integral`, without the α-dependent
/// finalize. Used to amortize α-sweep cost on a fixed geometry.
pub fn collect_circle_sums_integral(
    integral: &Integral,
    h: u32,
    w: u32,
    cx: i32,
    cy: i32,
    r: i32,
) -> ShapeSums {
    let mut sums = ShapeSums::new();
    if r <= 0 {
        return sums;
    }
    let ymin = (cy - r).max(0);
    let ymax = (cy + r).min(h as i32 - 1);
    if ymin > ymax {
        return sums;
    }
    let r2 = (r as i64) * (r as i64);
    for y in ymin..=ymax {
        let dy = (y - cy) as i64;
        let lim = r2 - dy * dy;
        if lim < 0 {
            continue;
        }
        let dx = isqrt_i64(lim) as i32;
        let x_l = (cx - dx).max(0);
        let x_r = (cx + dx).min(w as i32 - 1);
        if x_l > x_r {
            continue;
        }
        integral.add_span(
            y as usize,
            x_l as usize,
            x_r as usize,
            &mut sums.s_t,
            &mut sums.s_c,
            &mut sums.s_t2,
            &mut sums.s_c2,
            &mut sums.s_tc,
        );
        sums.count += (x_r - x_l + 1) as u32;
    }
    #[cfg(feature = "bench-counters")]
    counters::PIXELS_TOUCHED.with(|c| c.set(c.get() + sums.count as u64));
    sums
}

/// Triangle ΔSSE via per-row analytic span (intersection of three
/// half-planes) plus integral lookup.
#[allow(clippy::too_many_arguments)]
pub fn eval_triangle_integral(
    integral: &Integral,
    h: u32,
    w: u32,
    vx0: i32,
    vy0: i32,
    vx1: i32,
    vy1: i32,
    vx2: i32,
    vy2: i32,
    alpha: f32,
    palette: Option<&[f32]>,
) -> EvalResult {
    #[cfg(feature = "bench-counters")]
    counters::EVAL_TRIANGLE.with(|c| c.set(c.get() + 1));
    collect_triangle_sums_integral(integral, h, w, vx0, vy0, vx1, vy1, vx2, vy2)
        .finalize(alpha, palette)
}

/// Triangle analogue of [`collect_circle_sums_integral`].
#[allow(clippy::too_many_arguments)]
pub fn collect_triangle_sums_integral(
    integral: &Integral,
    h: u32,
    w: u32,
    vx0: i32,
    vy0: i32,
    vx1: i32,
    vy1: i32,
    vx2: i32,
    vy2: i32,
) -> ShapeSums {
    let mut sums = ShapeSums::new();
    let area2 = (vx1 - vx0) * (vy2 - vy0) - (vy1 - vy0) * (vx2 - vx0);
    if area2 == 0 {
        return sums;
    }
    let ymin = vy0.min(vy1).min(vy2).max(0);
    let ymax = vy0.max(vy1).max(vy2).min(h as i32 - 1);
    if ymin > ymax {
        return sums;
    }
    let sign_pos = area2 > 0;

    // Edge `i` from vertex i to vertex (i+1)%3. Inside is `e_i ≥ 0` if
    // sign_pos else `e_i ≤ 0`. Negate (a_i, c0_i, c_step_i) in the latter
    // case so the per-row logic always solves `a·x + c ≥ 0`.
    let edges = [
        (vx0, vy0, vx1, vy1),
        (vx1, vy1, vx2, vy2),
        (vx2, vy2, vx0, vy0),
    ];
    let sign = if sign_pos { 1i64 } else { -1i64 };
    let mut a = [0i64; 3];
    let mut c_row = [0i64; 3];
    let mut c_step = [0i64; 3];
    for (i, &(x_i, y_i, x_j, y_j)) in edges.iter().enumerate() {
        let a_i = -(y_j - y_i) as i64;
        let step = (x_j - x_i) as i64;
        let c0 = (y_j - y_i) as i64 * x_i as i64
            + (x_j - x_i) as i64 * (ymin - y_i) as i64;
        a[i] = sign * a_i;
        c_step[i] = sign * step;
        c_row[i] = sign * c0;
    }

    for y in ymin..=ymax {
        let mut x_lo: i64 = 0;
        let mut x_hi: i64 = (w as i64) - 1;
        let mut empty = false;
        for i in 0..3 {
            let ai = a[i];
            let ci = c_row[i];
            if ai > 0 {
                x_lo = x_lo.max(div_ceil_i64(-ci, ai));
            } else if ai < 0 {
                x_hi = x_hi.min(div_floor_i64(-ci, ai));
            } else if ci < 0 {
                empty = true;
                break;
            }
        }
        if !empty && x_lo <= x_hi {
            let x_l = x_lo.max(0) as usize;
            let x_r = (x_hi as usize).min(integral.w - 1);
            if x_l <= x_r {
                integral.add_span(
                    y as usize,
                    x_l,
                    x_r,
                    &mut sums.s_t,
                    &mut sums.s_c,
                    &mut sums.s_t2,
                    &mut sums.s_c2,
                    &mut sums.s_tc,
                );
                sums.count += (x_r - x_l + 1) as u32;
            }
        }
        // Step c_row by c_step (∂e/∂y).
        for i in 0..3 {
            c_row[i] += c_step[i];
        }
    }
    #[cfg(feature = "bench-counters")]
    counters::PIXELS_TOUCHED.with(|c| c.set(c.get() + sums.count as u64));
    sums
}

/// Rotated-rect (or general convex 4-vertex polygon) ΔSSE via the same
/// per-row half-plane span as the triangle path, with one extra edge.
#[allow(clippy::too_many_arguments)]
pub fn eval_quad_integral(
    integral: &Integral,
    h: u32,
    w: u32,
    v: [(i32, i32); 4],
    alpha: f32,
    palette: Option<&[f32]>,
) -> EvalResult {
    #[cfg(feature = "bench-counters")]
    counters::EVAL_ROTRECT.with(|c| c.set(c.get() + 1));
    collect_quad_sums_integral(integral, h, w, v).finalize(alpha, palette)
}

/// Quad analogue of [`collect_triangle_sums_integral`]. Vertices may be in
/// either winding order — orientation is auto-detected from the signed
/// area of the (v0, v1, v2) sub-triangle.
pub fn collect_quad_sums_integral(
    integral: &Integral,
    h: u32,
    w: u32,
    v: [(i32, i32); 4],
) -> ShapeSums {
    let mut sums = ShapeSums::new();
    let area2 = (v[1].0 - v[0].0) * (v[2].1 - v[0].1)
        - (v[1].1 - v[0].1) * (v[2].0 - v[0].0);
    if area2 == 0 {
        return sums;
    }
    let ymin = v.iter().map(|p| p.1).min().unwrap().max(0);
    let ymax = v.iter().map(|p| p.1).max().unwrap().min(h as i32 - 1);
    if ymin > ymax {
        return sums;
    }
    let sign_pos = area2 > 0;
    let sign = if sign_pos { 1i64 } else { -1i64 };

    let mut a = [0i64; 4];
    let mut c_row = [0i64; 4];
    let mut c_step = [0i64; 4];
    for i in 0..4 {
        let (x_i, y_i) = v[i];
        let (x_j, y_j) = v[(i + 1) % 4];
        let a_i = -(y_j - y_i) as i64;
        let step = (x_j - x_i) as i64;
        let c0 = (y_j - y_i) as i64 * x_i as i64
            + (x_j - x_i) as i64 * (ymin - y_i) as i64;
        a[i] = sign * a_i;
        c_step[i] = sign * step;
        c_row[i] = sign * c0;
    }

    for y in ymin..=ymax {
        let mut x_lo: i64 = 0;
        let mut x_hi: i64 = (w as i64) - 1;
        let mut empty = false;
        for i in 0..4 {
            let ai = a[i];
            let ci = c_row[i];
            if ai > 0 {
                x_lo = x_lo.max(div_ceil_i64(-ci, ai));
            } else if ai < 0 {
                x_hi = x_hi.min(div_floor_i64(-ci, ai));
            } else if ci < 0 {
                empty = true;
                break;
            }
        }
        if !empty && x_lo <= x_hi {
            let x_l = x_lo.max(0) as usize;
            let x_r = (x_hi as usize).min(integral.w - 1);
            if x_l <= x_r {
                integral.add_span(
                    y as usize, x_l, x_r,
                    &mut sums.s_t, &mut sums.s_c,
                    &mut sums.s_t2, &mut sums.s_c2, &mut sums.s_tc,
                );
                sums.count += (x_r - x_l + 1) as u32;
            }
        }
        for i in 0..4 {
            c_row[i] += c_step[i];
        }
    }
    #[cfg(feature = "bench-counters")]
    counters::PIXELS_TOUCHED.with(|c| c.set(c.get() + sums.count as u64));
    sums
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shape::raster::{eval_circle, eval_triangle};

    fn synth(target: &mut [f32], canvas: &mut [f32], h: u32, w: u32, seed: u64) {
        // deterministic xorshift-ish fill so tests don't depend on libstd RNG.
        let mut s = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut next = || -> f32 {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            ((s & 0xFFFF) as f32) / 65535.0
        };
        for i in 0..(h * w * 3) as usize {
            target[i] = next();
            canvas[i] = next();
        }
    }

    #[test]
    fn circle_eval_matches_baseline() {
        let (h, w) = (24u32, 24u32);
        let mut target = vec![0.0f32; (h * w * 3) as usize];
        let mut canvas = vec![0.0f32; (h * w * 3) as usize];
        synth(&mut target, &mut canvas, h, w, 0xDEAD_BEEF);
        let integral = Integral::build(&target, &canvas, h, w);

        // A battery of (cx, cy, r, alpha) cases including edge-clipped ones.
        let cases: [(i32, i32, i32, f32); 8] = [
            (12, 12, 5, 0.5),
            (0, 0, 3, 0.3),
            (23, 23, 4, 0.9),
            (12, 12, 12, 0.5),
            (5, 18, 7, 0.6),
            (12, 0, 4, 0.7),
            (-2, 12, 5, 0.4),  // partially off-canvas
            (25, 25, 6, 0.8),  // mostly off-canvas
        ];
        for (cx, cy, r, alpha) in cases {
            let base = eval_circle(&target, &canvas, h as i32, w as i32, cx, cy, r, alpha, None);
            let opt = eval_circle_integral(&integral, h, w, cx, cy, r, alpha, None);
            let diff = (base.delta_sse - opt.delta_sse).abs();
            assert!(
                diff < 1e-3,
                "circle eval mismatch at ({cx},{cy},r={r},α={alpha}): base={} opt={} diff={}",
                base.delta_sse, opt.delta_sse, diff
            );
            for ch in 0..3 {
                assert!(
                    (base.color[ch] - opt.color[ch]).abs() < 1e-4,
                    "color mismatch ch {} at ({cx},{cy},r={r},α={alpha})",
                    ch
                );
            }
        }
    }

    #[test]
    fn triangle_eval_matches_baseline() {
        let (h, w) = (24u32, 24u32);
        let mut target = vec![0.0f32; (h * w * 3) as usize];
        let mut canvas = vec![0.0f32; (h * w * 3) as usize];
        synth(&mut target, &mut canvas, h, w, 0xCAFE_F00D);
        let integral = Integral::build(&target, &canvas, h, w);

        let cases: [([(i32, i32); 3], f32); 6] = [
            ([(2, 2), (20, 4), (10, 18)], 0.5),
            ([(0, 0), (23, 0), (12, 23)], 0.7),
            ([(5, 5), (15, 5), (10, 15)], 0.3),
            ([(8, 1), (1, 22), (22, 22)], 0.9),
            // Negative-orientation winding (vertices clockwise).
            ([(20, 4), (2, 2), (10, 18)], 0.5),
            // Partially off-canvas.
            ([(-5, 5), (15, 5), (10, 25)], 0.6),
        ];
        for (verts, alpha) in cases {
            let base = eval_triangle(
                &target, &canvas, h as i32, w as i32,
                verts[0].0, verts[0].1, verts[1].0, verts[1].1, verts[2].0, verts[2].1,
                alpha, None,
            );
            let opt = eval_triangle_integral(
                &integral, h, w,
                verts[0].0, verts[0].1, verts[1].0, verts[1].1, verts[2].0, verts[2].1,
                alpha, None,
            );
            let diff = (base.delta_sse - opt.delta_sse).abs();
            assert!(
                diff < 1e-3,
                "triangle eval mismatch at {:?},α={}: base={} opt={} diff={}",
                verts, alpha, base.delta_sse, opt.delta_sse, diff
            );
        }
    }

    #[test]
    fn update_rows_after_canvas_change() {
        let (h, w) = (16u32, 16u32);
        let mut target = vec![0.0f32; (h * w * 3) as usize];
        let mut canvas = vec![0.0f32; (h * w * 3) as usize];
        synth(&mut target, &mut canvas, h, w, 0x1234_5678);
        let mut integral = Integral::build(&target, &canvas, h, w);

        // Mutate canvas in rows 4..8.
        for y in 4..8 {
            for x in 0..w {
                let p = ((y * w + x) * 3) as usize;
                canvas[p] = 0.7;
                canvas[p + 1] = 0.3;
                canvas[p + 2] = 0.5;
            }
        }
        integral.update_canvas_rows(&target, &canvas, 4, 7);

        // Verify a circle that intersects rows 4..8 evaluates the same as
        // the freshly-built baseline.
        let opt = eval_circle_integral(&integral, h, w, 8, 6, 5, 0.5, None);
        let base = eval_circle(&target, &canvas, h as i32, w as i32, 8, 6, 5, 0.5, None);
        let diff = (base.delta_sse - opt.delta_sse).abs();
        assert!(diff < 1e-3, "post-update mismatch: base={} opt={} diff={}", base.delta_sse, opt.delta_sse, diff);
    }
}

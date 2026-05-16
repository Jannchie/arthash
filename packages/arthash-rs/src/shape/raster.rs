//! Scanline rasterizers + ΔSSE evaluators for CIRCLE and TRIANGLE.
//!
//! Each evaluator returns `(delta_sse, color_linear, palette_idx)`. ΔSSE is
//! negative when adding the shape improves the canvas. Color is either the
//! analytic optimal continuous-mode color, or the best palette entry. These
//! are the inner kernels of the hill-climb loop — keep them tight.
//!
//! ## Palette mode optimization
//!
//! Per-shape SSE for a candidate color `p` is
//! `sse(p) = const - 2α(p·s_t) + 2α(1-α)(p·s_c) + α²|p|²·cnt`. Substituting
//! the continuous-optimal `p* = (s_t − (1−α)·s_c) / (α·cnt)` gives
//! `sse(p) = sse(p*) + α²·cnt · |p − p*|²`, so minimizing SSE over a palette
//! ⊂ ℝ³ collapses to nearest-neighbor of `p*` under Euclidean distance.
//! `palette::PaletteIndex` does that lookup in O(1) (or O(K) for small K),
//! replacing the original per-eval O(K) SSE scan.

/// Optional benchmarking counters. When `bench-counters` is enabled, every
/// `eval_*` call and every `Accum::push` bumps a thread-local — letting an
/// external bench attribute hill-climb cost. Zero-cost when the feature
/// is off (no code is generated for the increments).
pub mod counters {
    #[cfg(feature = "bench-counters")]
    use std::cell::Cell;

    #[cfg(feature = "bench-counters")]
    thread_local! {
        pub(crate) static EVAL_CIRCLE: Cell<u64> = const { Cell::new(0) };
        pub(crate) static EVAL_TRIANGLE: Cell<u64> = const { Cell::new(0) };
        pub(crate) static EVAL_RECT: Cell<u64> = const { Cell::new(0) };
        pub(crate) static EVAL_SQUARE: Cell<u64> = const { Cell::new(0) };
        pub(crate) static EVAL_ROTRECT: Cell<u64> = const { Cell::new(0) };
        pub(crate) static PIXELS_TOUCHED: Cell<u64> = const { Cell::new(0) };
    }

    #[derive(Clone, Copy, Debug, Default)]
    pub struct Snapshot {
        pub eval_circle: u64,
        pub eval_triangle: u64,
        pub eval_rect: u64,
        pub eval_square: u64,
        pub eval_rotrect: u64,
        pub pixels_touched: u64,
    }

    pub fn reset() {
        #[cfg(feature = "bench-counters")]
        {
            EVAL_CIRCLE.with(|c| c.set(0));
            EVAL_TRIANGLE.with(|c| c.set(0));
            EVAL_RECT.with(|c| c.set(0));
            EVAL_SQUARE.with(|c| c.set(0));
            EVAL_ROTRECT.with(|c| c.set(0));
            PIXELS_TOUCHED.with(|c| c.set(0));
        }
    }

    pub fn snapshot() -> Snapshot {
        #[cfg(feature = "bench-counters")]
        {
            Snapshot {
                eval_circle: EVAL_CIRCLE.with(|c| c.get()),
                eval_triangle: EVAL_TRIANGLE.with(|c| c.get()),
                eval_rect: EVAL_RECT.with(|c| c.get()),
                eval_square: EVAL_SQUARE.with(|c| c.get()),
                eval_rotrect: EVAL_ROTRECT.with(|c| c.get()),
                pixels_touched: PIXELS_TOUCHED.with(|c| c.get()),
            }
        }
        #[cfg(not(feature = "bench-counters"))]
        {
            Snapshot::default()
        }
    }
}

/// Result of a single shape evaluation.
#[derive(Clone, Copy, Debug)]
pub struct EvalResult {
    pub delta_sse: f32,
    pub color: [f32; 3],
    pub pidx: u32,
}

use super::palette::PaletteIndex;

/// Evaluate one CIRCLE candidate via bounding-box scan.
pub fn eval_circle(
    target: &[f32],
    canvas: &[f32],
    th: i32,
    tw: i32,
    cx: i32,
    cy: i32,
    r: i32,
    alpha: f32,
    palette: Option<&PaletteIndex>,
) -> EvalResult {
    #[cfg(feature = "bench-counters")]
    counters::EVAL_CIRCLE.with(|c| c.set(c.get() + 1));
    collect_circle_sums(target, canvas, th, tw, cx, cy, r).finalize(alpha, palette)
}

/// Collect the per-channel sums (`ShapeSums`) for one circle candidate — the
/// expensive part of `eval_circle`, without the α-dependent finalize. Useful
/// when α-sweeping a fixed geometry: collect once, finalize K times.
pub fn collect_circle_sums(
    target: &[f32],
    canvas: &[f32],
    th: i32,
    tw: i32,
    cx: i32,
    cy: i32,
    r: i32,
) -> ShapeSums {
    if r <= 0 {
        return ShapeSums::new();
    }
    let xmin = (cx - r).max(0);
    let xmax = (cx + r).min(tw - 1);
    let ymin = (cy - r).max(0);
    let ymax = (cy + r).min(th - 1);
    if xmin > xmax || ymin > ymax {
        return ShapeSums::new();
    }
    let r2 = r * r;
    let mut accum = ShapeSums::new();
    for y in ymin..=ymax {
        let dy = y - cy;
        let dy2 = dy * dy;
        let row_off = (y * tw) as usize * 3;
        for x in xmin..=xmax {
            let dx = x - cx;
            if dx * dx + dy2 <= r2 {
                let p = row_off + (x as usize) * 3;
                accum.push(&target[p..p + 3], &canvas[p..p + 3]);
            }
        }
    }
    accum
}

/// Evaluate one TRIANGLE candidate via incremental edge functions.
pub fn eval_triangle(
    target: &[f32],
    canvas: &[f32],
    th: i32,
    tw: i32,
    vx0: i32,
    vy0: i32,
    vx1: i32,
    vy1: i32,
    vx2: i32,
    vy2: i32,
    alpha: f32,
    palette: Option<&PaletteIndex>,
) -> EvalResult {
    #[cfg(feature = "bench-counters")]
    counters::EVAL_TRIANGLE.with(|c| c.set(c.get() + 1));
    collect_triangle_sums(target, canvas, th, tw, vx0, vy0, vx1, vy1, vx2, vy2)
        .finalize(alpha, palette)
}

/// Triangle counterpart of [`collect_circle_sums`].
#[allow(clippy::too_many_arguments)]
pub fn collect_triangle_sums(
    target: &[f32],
    canvas: &[f32],
    th: i32,
    tw: i32,
    vx0: i32,
    vy0: i32,
    vx1: i32,
    vy1: i32,
    vx2: i32,
    vy2: i32,
) -> ShapeSums {
    let area2 = (vx1 - vx0) * (vy2 - vy0) - (vy1 - vy0) * (vx2 - vx0);
    if area2 == 0 {
        return ShapeSums::new();
    }
    let xmin = vx0.min(vx1).min(vx2).max(0);
    let xmax = vx0.max(vx1).max(vx2).min(tw - 1);
    let ymin = vy0.min(vy1).min(vy2).max(0);
    let ymax = vy0.max(vy1).max(vy2).min(th - 1);
    if xmin > xmax || ymin > ymax {
        return ShapeSums::new();
    }
    let sign_pos = area2 > 0;

    let d_e0_dx = -(vy1 - vy0);
    let d_e0_dy = vx1 - vx0;
    let d_e1_dx = -(vy2 - vy1);
    let d_e1_dy = vx2 - vx1;
    let d_e2_dx = -(vy0 - vy2);
    let d_e2_dy = vx0 - vx2;

    let mut row_e0 = (vx1 - vx0) * (ymin - vy0) - (vy1 - vy0) * (xmin - vx0);
    let mut row_e1 = (vx2 - vx1) * (ymin - vy1) - (vy2 - vy1) * (xmin - vx1);
    let mut row_e2 = (vx0 - vx2) * (ymin - vy2) - (vy0 - vy2) * (xmin - vx2);

    let mut accum = ShapeSums::new();
    for y in ymin..=ymax {
        let mut e0 = row_e0;
        let mut e1 = row_e1;
        let mut e2 = row_e2;
        let row_off = (y * tw) as usize * 3;
        for x in xmin..=xmax {
            let inside = if sign_pos {
                e0 >= 0 && e1 >= 0 && e2 >= 0
            } else {
                e0 <= 0 && e1 <= 0 && e2 <= 0
            };
            if inside {
                let p = row_off + (x as usize) * 3;
                accum.push(&target[p..p + 3], &canvas[p..p + 3]);
            }
            e0 += d_e0_dx;
            e1 += d_e1_dx;
            e2 += d_e2_dx;
        }
        row_e0 += d_e0_dy;
        row_e1 += d_e1_dy;
        row_e2 += d_e2_dy;
    }
    accum
}

/// Per-channel sums sufficient to evaluate ΔSSE for any α (and any palette
/// entry) on a given shape geometry. Public so callers can amortize the
/// expensive scan once, then evaluate K alpha levels cheaply via
/// `finalize`. See `circle::fit_primitive` α-sweep for the reference use.
#[derive(Clone, Copy, Debug, Default)]
pub struct ShapeSums {
    pub s_t: [f64; 3],
    pub s_c: [f64; 3],
    pub s_t2: [f64; 3],
    pub s_c2: [f64; 3],
    pub s_tc: [f64; 3],
    pub count: u32,
}

impl ShapeSums {
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    fn push(&mut self, t: &[f32], c: &[f32]) {
        #[cfg(feature = "bench-counters")]
        counters::PIXELS_TOUCHED.with(|c| c.set(c.get() + 1));
        for i in 0..3 {
            let tv = t[i] as f64;
            let cv = c[i] as f64;
            self.s_t[i] += tv;
            self.s_c[i] += cv;
            self.s_t2[i] += tv * tv;
            self.s_c2[i] += cv * cv;
            self.s_tc[i] += tv * cv;
        }
        self.count += 1;
    }

    /// Evaluate ΔSSE + optimal color at the given α. Geometry-independent
    /// (only depends on the sums), so an α-sweep on a fixed geometry can
    /// reuse the same `ShapeSums` instance instead of re-scanning pixels.
    pub fn finalize(&self, alpha: f32, palette: Option<&PaletteIndex>) -> EvalResult {
        if self.count == 0 {
            return EvalResult { delta_sse: 0.0, color: [0.0; 3], pidx: 0 };
        }
        let cnt = self.count as f64;
        let one_ma = (1.0 - alpha) as f64;
        let alpha = alpha as f64;

        let sse_before = (0..3)
            .map(|i| self.s_t2[i] - 2.0 * self.s_tc[i] + self.s_c2[i])
            .sum::<f64>();

        // Closed-form continuous optimum (unclamped). Used directly in the
        // None branch (after clamping to [0, 1]) and as the NN query point
        // in the palette branch.
        let denom = alpha * cnt;
        let mut p_opt = [0.0f64; 3];
        if denom > 0.0 {
            for (i, slot) in p_opt.iter_mut().enumerate() {
                *slot = (self.s_t[i] - one_ma * self.s_c[i]) / denom;
            }
        }

        let (color, pidx) = match palette {
            None => (
                [
                    p_opt[0].clamp(0.0, 1.0) as f32,
                    p_opt[1].clamp(0.0, 1.0) as f32,
                    p_opt[2].clamp(0.0, 1.0) as f32,
                ],
                0u32,
            ),
            Some(pal) => {
                let (idx, c) = pal.nearest([
                    p_opt[0] as f32,
                    p_opt[1] as f32,
                    p_opt[2] as f32,
                ]);
                (c, idx)
            }
        };

        // ΔSSE for the chosen color. Identical algebraic form for both
        // branches: substitute `color` for `p` in `sse(p)`.
        let p = [color[0] as f64, color[1] as f64, color[2] as f64];
        let sse_after: f64 = (0..3)
            .map(|i| {
                self.s_t2[i] - 2.0 * one_ma * self.s_tc[i]
                    + one_ma * one_ma * self.s_c2[i]
                    - 2.0 * alpha * p[i] * self.s_t[i]
                    + 2.0 * alpha * one_ma * p[i] * self.s_c[i]
                    + alpha * alpha * p[i] * p[i] * cnt
            })
            .sum();

        EvalResult {
            delta_sse: (sse_after - sse_before) as f32,
            color,
            pidx,
        }
    }
}

/// Apply (commit) a CIRCLE onto the canvas. Used during fitting + decoding.
pub fn apply_circle(
    canvas: &mut [f32],
    th: i32,
    tw: i32,
    cx: i32,
    cy: i32,
    r: i32,
    alpha: f32,
    color: &[f32; 3],
) {
    if r <= 0 {
        return;
    }
    let xmin = (cx - r).max(0);
    let xmax = (cx + r).min(tw - 1);
    let ymin = (cy - r).max(0);
    let ymax = (cy + r).min(th - 1);
    let r2 = r * r;
    let one_ma = 1.0 - alpha;
    for y in ymin..=ymax {
        let dy = y - cy;
        let dy2 = dy * dy;
        let row_off = (y * tw) as usize * 3;
        for x in xmin..=xmax {
            let dx = x - cx;
            if dx * dx + dy2 <= r2 {
                let p = row_off + (x as usize) * 3;
                canvas[p] = one_ma * canvas[p] + alpha * color[0];
                canvas[p + 1] = one_ma * canvas[p + 1] + alpha * color[1];
                canvas[p + 2] = one_ma * canvas[p + 2] + alpha * color[2];
            }
        }
    }
}

/// Apply (commit) an axis-aligned RECT onto the canvas.
pub fn apply_rect(
    canvas: &mut [f32],
    th: i32,
    tw: i32,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    alpha: f32,
    color: &[f32; 3],
) {
    let xmin = x0.min(x1).max(0);
    let xmax = x0.max(x1).min(tw - 1);
    let ymin = y0.min(y1).max(0);
    let ymax = y0.max(y1).min(th - 1);
    if xmin > xmax || ymin > ymax {
        return;
    }
    let one_ma = 1.0 - alpha;
    for y in ymin..=ymax {
        let row_off = (y * tw) as usize * 3;
        for x in xmin..=xmax {
            let p = row_off + (x as usize) * 3;
            canvas[p] = one_ma * canvas[p] + alpha * color[0];
            canvas[p + 1] = one_ma * canvas[p + 1] + alpha * color[1];
            canvas[p + 2] = one_ma * canvas[p + 2] + alpha * color[2];
        }
    }
}

/// Range of rows touched by an axis-aligned rect, clamped to canvas. Used to
/// trigger row-wise integral updates after [`apply_rect`].
pub fn rect_row_range(y0: i32, y1: i32, h: u32) -> (i32, i32) {
    let ymin = y0.min(y1).max(0);
    let ymax = y0.max(y1).min(h as i32 - 1);
    (ymin, ymax)
}

/// Apply (commit) a rotated rect — generic 4-vertex convex polygon scanline.
/// Vertices may be supplied in either winding order; the half-plane test
/// auto-detects sign from the (oriented) signed area of the first triangle.
pub fn apply_quad(
    canvas: &mut [f32],
    th: i32,
    tw: i32,
    v: [(i32, i32); 4],
    alpha: f32,
    color: &[f32; 3],
) {
    let xmin = v.iter().map(|p| p.0).min().unwrap().max(0);
    let xmax = v.iter().map(|p| p.0).max().unwrap().min(tw - 1);
    let ymin = v.iter().map(|p| p.1).min().unwrap().max(0);
    let ymax = v.iter().map(|p| p.1).max().unwrap().min(th - 1);
    if xmin > xmax || ymin > ymax {
        return;
    }
    let area2 = (v[1].0 - v[0].0) * (v[2].1 - v[0].1)
        - (v[1].1 - v[0].1) * (v[2].0 - v[0].0);
    if area2 == 0 {
        return;
    }
    let sign_pos = area2 > 0;

    // Precompute 4 edge functions e_i(x, y) = (x_{i+1}-x_i)(y-y_i) - (y_{i+1}-y_i)(x-x_i).
    // Inside is e_i ≥ 0 for sign_pos else ≤ 0.
    let mut a = [0i32; 4]; // ∂e/∂x
    let mut b = [0i32; 4]; // ∂e/∂y
    let mut row_e = [0i32; 4]; // value at (xmin, ymin)
    for i in 0..4 {
        let j = (i + 1) % 4;
        a[i] = -(v[j].1 - v[i].1);
        b[i] = v[j].0 - v[i].0;
        row_e[i] = (v[j].0 - v[i].0) * (ymin - v[i].1)
            - (v[j].1 - v[i].1) * (xmin - v[i].0);
    }
    let one_ma = 1.0 - alpha;
    for y in ymin..=ymax {
        let mut e = row_e;
        let row_off = (y * tw) as usize * 3;
        for x in xmin..=xmax {
            let inside = if sign_pos {
                e[0] >= 0 && e[1] >= 0 && e[2] >= 0 && e[3] >= 0
            } else {
                e[0] <= 0 && e[1] <= 0 && e[2] <= 0 && e[3] <= 0
            };
            if inside {
                let p = row_off + (x as usize) * 3;
                canvas[p] = one_ma * canvas[p] + alpha * color[0];
                canvas[p + 1] = one_ma * canvas[p + 1] + alpha * color[1];
                canvas[p + 2] = one_ma * canvas[p + 2] + alpha * color[2];
            }
            for k in 0..4 {
                e[k] += a[k];
            }
        }
        for k in 0..4 {
            row_e[k] += b[k];
        }
    }
}

/// Row range touched by a quad, clamped to canvas. For row-wise integral
/// updates after [`apply_quad`].
pub fn quad_row_range(v: [(i32, i32); 4], h: u32) -> (i32, i32) {
    let ymin = v.iter().map(|p| p.1).min().unwrap().max(0);
    let ymax = v.iter().map(|p| p.1).max().unwrap().min(h as i32 - 1);
    (ymin, ymax)
}

/// Apply (commit) a TRIANGLE onto the canvas using the same edge functions.
pub fn apply_triangle(
    canvas: &mut [f32],
    th: i32,
    tw: i32,
    v: [(i32, i32); 3],
    alpha: f32,
    color: &[f32; 3],
) {
    let (vx0, vy0) = v[0];
    let (vx1, vy1) = v[1];
    let (vx2, vy2) = v[2];
    let area2 = (vx1 - vx0) * (vy2 - vy0) - (vy1 - vy0) * (vx2 - vx0);
    if area2 == 0 {
        return;
    }
    let xmin = vx0.min(vx1).min(vx2).max(0);
    let xmax = vx0.max(vx1).max(vx2).min(tw - 1);
    let ymin = vy0.min(vy1).min(vy2).max(0);
    let ymax = vy0.max(vy1).max(vy2).min(th - 1);
    if xmin > xmax || ymin > ymax {
        return;
    }
    let sign_pos = area2 > 0;
    let d_e0_dx = -(vy1 - vy0);
    let d_e0_dy = vx1 - vx0;
    let d_e1_dx = -(vy2 - vy1);
    let d_e1_dy = vx2 - vx1;
    let d_e2_dx = -(vy0 - vy2);
    let d_e2_dy = vx0 - vx2;
    let mut row_e0 = (vx1 - vx0) * (ymin - vy0) - (vy1 - vy0) * (xmin - vx0);
    let mut row_e1 = (vx2 - vx1) * (ymin - vy1) - (vy2 - vy1) * (xmin - vx1);
    let mut row_e2 = (vx0 - vx2) * (ymin - vy2) - (vy0 - vy2) * (xmin - vx2);
    let one_ma = 1.0 - alpha;
    for y in ymin..=ymax {
        let mut e0 = row_e0;
        let mut e1 = row_e1;
        let mut e2 = row_e2;
        let row_off = (y * tw) as usize * 3;
        for x in xmin..=xmax {
            let inside = if sign_pos {
                e0 >= 0 && e1 >= 0 && e2 >= 0
            } else {
                e0 <= 0 && e1 <= 0 && e2 <= 0
            };
            if inside {
                let p = row_off + (x as usize) * 3;
                canvas[p] = one_ma * canvas[p] + alpha * color[0];
                canvas[p + 1] = one_ma * canvas[p + 1] + alpha * color[1];
                canvas[p + 2] = one_ma * canvas[p + 2] + alpha * color[2];
            }
            e0 += d_e0_dx;
            e1 += d_e1_dx;
            e2 += d_e2_dx;
        }
        row_e0 += d_e0_dy;
        row_e1 += d_e1_dy;
        row_e2 += d_e2_dy;
    }
}

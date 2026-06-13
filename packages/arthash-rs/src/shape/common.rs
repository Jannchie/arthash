//! Small helpers shared by the primitive-style shape fitters
//! (circle / triangle / square / rect / rotated-rect). Previously each module
//! carried its own byte-identical copy of these.

use super::palette::PaletteIndex;
use super::raster::{EvalResult, ShapeSums};

/// α held fixed during the shape-only hill-climb (matches primitive's α=128).
pub(crate) const FIXED_HILL_CLIMB_ALPHA: f32 = 0.5;

/// Mean linear-RGB of `target` (row-major `(_, _, 3)`), accumulated in f64.
///
/// The accumulation order is load-bearing: this value becomes the encoded
/// background color, so it must stay bit-identical across every shape mode —
/// do not reorder the sum or change the f64 intermediate.
pub(crate) fn mean_rgb(target: &[f32]) -> [f32; 3] {
    let n = target.len() / 3;
    let mut acc = [0.0f64; 3];
    for i in 0..n {
        acc[0] += target[i * 3] as f64;
        acc[1] += target[i * 3 + 1] as f64;
        acc[2] += target[i * 3 + 2] as f64;
    }
    [
        (acc[0] / n as f64) as f32,
        (acc[1] / n as f64) as f32,
        (acc[2] / n as f64) as f32,
    ]
}

/// Allocate an `h×w×3` linear-RGB canvas pre-filled with the background color.
/// Per-pixel writes are independent, so fill order doesn't affect any result.
pub(crate) fn filled_canvas(bg: [f32; 3], h: u32, w: u32) -> Vec<f32> {
    let n = (h * w) as usize;
    let mut canvas = vec![0.0f32; n * 3];
    for i in 0..n {
        canvas[i * 3] = bg[0];
        canvas[i * 3 + 1] = bg[1];
        canvas[i * 3 + 2] = bg[2];
    }
    canvas
}

/// Sweep every quantized α level over a fixed geometry's pre-collected sums,
/// returning the `(α, eval)` with the lowest ΔSSE — the shared Stage-3 of
/// every primitive fitter. `alpha_levels` must be non-empty; the first level
/// is the fallback when (impossibly) no level beats `+∞`.
///
/// Bit-identical to the per-module inline sweeps it replaced: same strict `<`
/// compare, same `alpha_levels` traversal order, same `finalize` math.
pub(crate) fn alpha_sweep(
    sums: &ShapeSums,
    alpha_levels: &[f32],
    palette: Option<&PaletteIndex>,
) -> (f32, EvalResult) {
    let mut best_delta = f32::INFINITY;
    let mut best_alpha = alpha_levels[0];
    let mut best = EvalResult { delta_sse: 0.0, color: [0.0; 3], pidx: 0 };
    for &a in alpha_levels {
        let res = sums.finalize(a, palette);
        if res.delta_sse < best_delta {
            best_delta = res.delta_sse;
            best_alpha = a;
            best = res;
        }
    }
    (best_alpha, best)
}

/// Exact total SSE between `target` and `canvas` (row-major linear-RGB f32 of
/// equal length), accumulated in f64 for a drift-free refinement accept test.
pub(crate) fn sse_total(target: &[f32], canvas: &[f32]) -> f64 {
    target
        .iter()
        .zip(canvas)
        .map(|(t, c)| {
            let d = (*t - *c) as f64;
            d * d
        })
        .sum()
}

/// Joint refinement (backfitting) shared by every primitive shape mode.
///
/// After the greedy fit, revisit each shape once per pass: render the canvas
/// without it, re-search a fresh shape against that canvas, and keep the
/// replacement only when it lowers the *exact total SSE* of the full picture.
/// Accepted shapes migrate to the end of the paint order. Because only strict
/// improvements are accepted, repeated passes converge; `break` on the first
/// fully-rejected pass.
///
/// Quantization-aware by construction: `apply_quantized` is expected to wire-
/// quantize the shape before rasterizing, so both the baseline canvas and the
/// candidate test reflect exactly what the decoder will render — a continuous-
/// domain win that evaporates under quantization is never accepted. `search`
/// likewise returns an already-quantized candidate, so the stored shape equals
/// the tested one.
///
/// `passes == 0` (the default) returns immediately without touching `shapes`
/// or drawing from any RNG the caller threaded into `search`, preserving the
/// historical greedy output bit-for-bit.
pub(crate) fn refine_shapes<S>(
    target: &[f32],
    bg: [f32; 3],
    h: u32,
    w: u32,
    shapes: &mut Vec<S>,
    passes: u32,
    apply_quantized: impl Fn(&mut [f32], &S),
    mut search: impl FnMut(&[f32]) -> Option<S>,
) {
    if passes == 0 || shapes.len() < 2 {
        return;
    }
    let render = |shapes: &[S], skip: usize| -> Vec<f32> {
        let mut cv = filled_canvas(bg, h, w);
        for (k, s) in shapes.iter().enumerate() {
            if k != skip {
                apply_quantized(&mut cv, s);
            }
        }
        cv
    };
    for _ in 0..passes {
        let mut any_accepted = false;
        let n = shapes.len();
        let mut j = 0usize;
        for _ in 0..n {
            let old_total = sse_total(target, &render(shapes, usize::MAX));
            let canvas_wo = render(shapes, j);
            let mut accepted = false;
            if let Some(cand) = search(&canvas_wo) {
                let mut cv = canvas_wo;
                apply_quantized(&mut cv, &cand);
                if sse_total(target, &cv) + 1e-9 < old_total {
                    shapes.remove(j);
                    shapes.push(cand);
                    accepted = true;
                }
            }
            if accepted {
                any_accepted = true;
            } else {
                j += 1;
            }
        }
        if !any_accepted {
            break;
        }
    }
}

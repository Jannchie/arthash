//! RECT mode — axis-aligned rectangles.
//!
//! Each shape stores (cx, cy, w, h, color, alpha). The fit uses the same
//! primitive-style two-stage search as CIRCLE / TRIANGLE: residual-weighted
//! init, Gaussian hill-climb with `normal_step`, α-sweep finalize. Eval is
//! O(1) via [`super::integral2d::Integral2D`] — the Viola-Jones 4-corner
//! lookup over a 2D prefix-sum image.

use super::integral2d::{collect_rect_sums_integral, eval_rect_integral, Integral2D};
use super::options::SearchOptions;
use super::quant::{
    alpha_to_q, aspect_code, dim_to_q, q_to_alpha, q_to_dim, quant_xy, read_color, write_color,
};
use super::raster::{apply_rect, EvalResult};
use super::residual::Residual;
use super::rng::Rng;
use crate::bitio::{BitReader, BitWriter};
use crate::codec::Codec;

/// α held fixed during the shape-only hill-climb.
const FIXED_HILL_CLIMB_ALPHA: f32 = 0.5;

#[derive(Clone, Debug)]
pub struct Rect {
    pub cx: i32,
    pub cy: i32,
    pub w: i32,
    pub h: i32,
    pub alpha: f32,
    pub color: [f32; 3],
    pub pidx: u32,
}

/// Convert (cx, cy, w, h) → inclusive pixel bounds `[x0..x1] × [y0..y1]`.
#[inline]
fn rect_bounds(cx: i32, cy: i32, w: i32, h: i32) -> (i32, i32, i32, i32) {
    let hw = w / 2;
    let hh = h / 2;
    let x0 = cx - hw;
    let y0 = cy - hh;
    let x1 = x0 + (w - 1).max(0);
    let y1 = y0 + (h - 1).max(0);
    (x0, y0, x1, y1)
}

fn mean_rgb(target: &[f32]) -> [f32; 3] {
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

pub fn fit_rects(
    target: &[f32],
    h: u32,
    w: u32,
    codec: &Codec,
    seed: u64,
    search: &SearchOptions,
) -> ([f32; 3], Vec<Rect>) {
    let bg = mean_rgb(target);
    let mut canvas = vec![0.0f32; (h * w * 3) as usize];
    for i in 0..(h * w) as usize {
        canvas[i * 3] = bg[0];
        canvas[i * 3 + 1] = bg[1];
        canvas[i * 3 + 2] = bg[2];
    }
    let mut integral = Integral2D::build(target, &canvas, h, w);
    let mut residual = Residual::build(target, &canvas, h, w);
    let mut rng = Rng::new(seed);
    let alpha_levels = codec.alpha_levels_owned();
    let palette = codec.palette_linear();
    let pal_ref = palette.as_deref();

    let long_edge = w.max(h);
    let sigma_pos = (2u32).max(long_edge * 6 / 100) as f64;
    let sigma_wh = (1u32).max(long_edge * 6 / 100) as f64;
    let init_max = (2u32).max(long_edge * 25 / 100) as i64;
    let dim_min = 1i32;
    let dim_max = w.max(h) as i32;

    let use_max_age = search.hill_climb_max_age.is_some();
    let hard_cap = if use_max_age { 10_000 } else { search.hill_climb_steps };
    let null_color = bg;
    let null_alpha = alpha_levels[0];

    let mut rects: Vec<Rect> = Vec::with_capacity(codec.n_shapes as usize);

    for _ in 0..codec.n_shapes {
        let mut best_delta_climb: f32 = -1e-3;
        let mut best_geom: Option<(i32, i32, i32, i32)> = None;

        for _attempt in 0..search.n_attempts {
            // Stage 1 — residual-anchored init.
            let mut best_d = f32::INFINITY;
            let mut best_init: Option<(i32, i32, i32, i32)> = None;
            for _ in 0..search.n_random {
                let (cx, cy) = residual.sample(&mut rng);
                let rw = rng.range(1, init_max + 1) as i32;
                let rh = rng.range(1, init_max + 1) as i32;
                let (x0, y0, x1, y1) = rect_bounds(cx, cy, rw, rh);
                let res = eval_rect_integral(
                    &integral, x0, y0, x1, y1, FIXED_HILL_CLIMB_ALPHA, pal_ref,
                );
                if res.delta_sse < best_d {
                    best_d = res.delta_sse;
                    best_init = Some((cx, cy, rw, rh));
                }
            }
            let Some((mut cx, mut cy, mut rw, mut rh)) = best_init else { continue };

            // Stage 2 — Gaussian hill-climb. Perturb one of {cx, cy, w, h}
            // with a normal_step (no wasted dn=0 evals).
            let mut best_local_delta = best_d;
            let mut best_local_geom = (cx, cy, rw, rh);
            let mut age: u32 = 0;
            for _ in 0..hard_cap {
                let which = rng.range(0, 4);
                let (mut ncx, mut ncy, mut nw, mut nh) = (cx, cy, rw, rh);
                match which {
                    0 => ncx = (cx + rng.normal_step(sigma_pos)).clamp(0, w as i32 - 1),
                    1 => ncy = (cy + rng.normal_step(sigma_pos)).clamp(0, h as i32 - 1),
                    2 => nw = (rw + rng.normal_step(sigma_wh)).clamp(dim_min, dim_max),
                    _ => nh = (rh + rng.normal_step(sigma_wh)).clamp(dim_min, dim_max),
                }
                let (x0, y0, x1, y1) = rect_bounds(ncx, ncy, nw, nh);
                let res = eval_rect_integral(
                    &integral, x0, y0, x1, y1, FIXED_HILL_CLIMB_ALPHA, pal_ref,
                );
                if res.delta_sse < best_local_delta {
                    cx = ncx; cy = ncy; rw = nw; rh = nh;
                    best_local_delta = res.delta_sse;
                    best_local_geom = (cx, cy, rw, rh);
                    age = 0;
                } else {
                    age += 1;
                    if let Some(max_age) = search.hill_climb_max_age {
                        if age >= max_age {
                            break;
                        }
                    }
                }
            }
            if best_local_delta < best_delta_climb {
                best_delta_climb = best_local_delta;
                best_geom = Some(best_local_geom);
            }
        }

        // Stage 3 — α-sweep on fixed geometry.
        let (cx, cy, rw, rh, alpha, color, pidx) = match best_geom {
            None => (
                w as i32 / 2,
                h as i32 / 2,
                (w / 4).max(2) as i32,
                (h / 4).max(2) as i32,
                null_alpha,
                null_color,
                0u32,
            ),
            Some((cx, cy, rw, rh)) => {
                let (x0, y0, x1, y1) = rect_bounds(cx, cy, rw, rh);
                let sums = collect_rect_sums_integral(&integral, x0, y0, x1, y1);
                let mut best_a_delta = f32::INFINITY;
                let mut chosen = EvalResult { delta_sse: 0.0, color: null_color, pidx: 0 };
                let mut chosen_alpha = null_alpha;
                for &a in &alpha_levels {
                    let res = sums.finalize(a, pal_ref);
                    if res.delta_sse < best_a_delta {
                        best_a_delta = res.delta_sse;
                        chosen = res;
                        chosen_alpha = a;
                    }
                }
                (cx, cy, rw, rh, chosen_alpha, chosen.color, chosen.pidx)
            }
        };

        let (x0, y0, x1, y1) = rect_bounds(cx, cy, rw, rh);
        apply_rect(&mut canvas, h as i32, w as i32, x0, y0, x1, y1, alpha, &color);
        integral.update_canvas(target, &canvas);
        residual.rebuild(target, &canvas);
        rects.push(Rect { cx, cy, w: rw, h: rh, alpha, color, pidx });
    }
    (bg, rects)
}

pub fn encode_body(bw: &mut BitWriter, rects: &[Rect], tw: u32, th: u32, codec: &Codec) {
    let alpha_levels = codec.alpha_levels_owned();
    for r in rects {
        let (x_q, y_q) = quant_xy(r.cx as f32, r.cy as f32, tw, th, codec.cx_bits, codec.cy_bits);
        let w_q = dim_to_q(r.w as f32, tw, codec.r_bits);
        let h_q = dim_to_q(r.h as f32, th, codec.r_bits);
        bw.write(x_q, codec.cx_bits);
        bw.write(y_q, codec.cy_bits);
        bw.write(w_q, codec.r_bits);
        bw.write(h_q, codec.r_bits);
        write_color(bw, &r.color, r.pidx, codec);
        bw.write(alpha_to_q(r.alpha, &alpha_levels), codec.alpha_bits);
    }
}

pub fn decode_render(br: &mut BitReader, codec: &Codec, w: u32, h: u32, canvas: &mut [f32]) {
    let x_max = (1u32 << codec.cx_bits) - 1;
    let y_max = (1u32 << codec.cy_bits) - 1;
    let alpha_levels = codec.alpha_levels_owned();
    for _ in 0..codec.n_shapes {
        let x_q = br.read(codec.cx_bits);
        let y_q = br.read(codec.cy_bits);
        let w_q = br.read(codec.r_bits);
        let h_q = br.read(codec.r_bits);
        let color = read_color(br, codec);
        let a_q = br.read(codec.alpha_bits);
        let cx = (x_q as f32 / x_max as f32 * (w - 1) as f32).round() as i32;
        let cy = (y_q as f32 / y_max as f32 * (h - 1) as f32).round() as i32;
        let rw = q_to_dim(w_q, w, codec.r_bits).round() as i32;
        let rh = q_to_dim(h_q, h, codec.r_bits).round() as i32;
        let alpha = q_to_alpha(a_q, &alpha_levels);
        let (x0, y0, x1, y1) = rect_bounds(cx, cy, rw, rh);
        apply_rect(canvas, h as i32, w as i32, x0, y0, x1, y1, alpha, &color);
    }
}

pub fn encode_rect(
    target: &[f32],
    th: u32,
    tw: u32,
    w_orig: u32,
    h_orig: u32,
    codec: &Codec,
    seed: u64,
    search: &SearchOptions,
) -> Vec<u8> {
    let (bg, rects) = fit_rects(target, th, tw, codec, seed, search);
    let mut bw = BitWriter::new();
    bw.write(aspect_code(w_orig, h_orig), 8);
    let pidx = if codec.is_palette_mode() {
        let pal = codec.palette_linear().unwrap();
        let mut best_k = 0usize;
        let mut best_d = f32::INFINITY;
        let k = pal.len() / 3;
        for ki in 0..k {
            let d = (0..3).map(|i| (pal[ki * 3 + i] - bg[i]).powi(2)).sum::<f32>();
            if d < best_d { best_d = d; best_k = ki; }
        }
        best_k as u32
    } else {
        0
    };
    write_color(&mut bw, &bg, pidx, codec);
    encode_body(&mut bw, &rects, tw, th, codec);
    bw.finish()
}

/// Decoder helper: read one rect from the body stream, returning the
/// resolved (cx, cy, w, h, color, alpha) for SVG / external use. Mirrors
/// the bit layout of `encode_body` exactly.
pub fn decode_rect_at(
    br: &mut BitReader,
    codec: &Codec,
    w: u32,
    h: u32,
) -> (i32, i32, i32, i32, [f32; 3], f32) {
    let x_max = ((1u32 << codec.cx_bits) - 1) as f32;
    let y_max = ((1u32 << codec.cy_bits) - 1) as f32;
    let alpha_levels = codec.alpha_levels_owned();
    let w_m1 = (w as f32 - 1.0).max(0.0);
    let h_m1 = (h as f32 - 1.0).max(0.0);
    let x_q = br.read(codec.cx_bits);
    let y_q = br.read(codec.cy_bits);
    let w_q = br.read(codec.r_bits);
    let h_q = br.read(codec.r_bits);
    let color = read_color(br, codec);
    let alpha = q_to_alpha(br.read(codec.alpha_bits), &alpha_levels);
    let cx = ((x_q as f32) / x_max * w_m1).round() as i32;
    let cy = ((y_q as f32) / y_max * h_m1).round() as i32;
    let rw = q_to_dim(w_q, w, codec.r_bits).round() as i32;
    let rh = q_to_dim(h_q, h, codec.r_bits).round() as i32;
    (cx, cy, rw, rh, color, alpha)
}

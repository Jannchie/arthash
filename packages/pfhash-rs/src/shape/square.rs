//! SQUARE mode — axis-aligned squares.
//!
//! Same bit layout as CIRCLE (cx, cy, s, color, alpha) — the only difference
//! is the rasterized shape. Reuses the [`super::integral2d`] 2D-integral O(1)
//! eval path, with the search restricted to a single side parameter `s`
//! instead of independent w and h.

use super::integral2d::{collect_rect_sums_integral, eval_rect_integral, Integral2D};
use super::options::SearchOptions;
use super::quant::{
    alpha_to_q, aspect_code, q_to_alpha, q_to_r, quant_xy, r_to_q, read_color, write_color,
};
use super::raster::{apply_rect, EvalResult};
use super::residual::Residual;
use super::rng::Rng;
use crate::bitio::{BitReader, BitWriter};
use crate::codec::Codec;

const FIXED_HILL_CLIMB_ALPHA: f32 = 0.5;

#[derive(Clone, Debug)]
pub struct Square {
    pub cx: i32,
    pub cy: i32,
    pub s: i32,
    pub alpha: f32,
    pub color: [f32; 3],
    pub pidx: u32,
}

#[inline]
fn square_bounds(cx: i32, cy: i32, s: i32) -> (i32, i32, i32, i32) {
    let hs = s / 2;
    let x0 = cx - hs;
    let y0 = cy - hs;
    let x1 = x0 + (s - 1).max(0);
    let y1 = y0 + (s - 1).max(0);
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

pub fn fit_squares(
    target: &[f32],
    h: u32,
    w: u32,
    codec: &Codec,
    seed: u64,
    search: &SearchOptions,
) -> ([f32; 3], Vec<Square>) {
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
    let sigma_s = (1u32).max(long_edge * 6 / 100) as f64;
    let s_init_max = (2u32).max(long_edge * 25 / 100) as i64;
    let s_max_global = w.min(h).max(1) as i32;

    let use_max_age = search.hill_climb_max_age.is_some();
    let hard_cap = if use_max_age { 10_000 } else { search.hill_climb_steps };
    let null_color = bg;
    let null_alpha = alpha_levels[0];

    let mut squares: Vec<Square> = Vec::with_capacity(codec.n_shapes as usize);

    for _ in 0..codec.n_shapes {
        let mut best_delta_climb: f32 = -1e-3;
        let mut best_geom: Option<(i32, i32, i32)> = None;

        for _attempt in 0..search.n_attempts {
            // Stage 1.
            let mut best_d = f32::INFINITY;
            let mut best_init: Option<(i32, i32, i32)> = None;
            for _ in 0..search.n_random {
                let (cx, cy) = residual.sample(&mut rng);
                let s = rng.range(1, s_init_max + 1) as i32;
                let (x0, y0, x1, y1) = square_bounds(cx, cy, s);
                let res = eval_rect_integral(
                    &integral, x0, y0, x1, y1, FIXED_HILL_CLIMB_ALPHA, pal_ref,
                );
                if res.delta_sse < best_d {
                    best_d = res.delta_sse;
                    best_init = Some((cx, cy, s));
                }
            }
            let Some((mut cx, mut cy, mut s)) = best_init else { continue };

            // Stage 2.
            let mut best_local_delta = best_d;
            let mut best_local_geom = (cx, cy, s);
            let mut age: u32 = 0;
            for _ in 0..hard_cap {
                let which = rng.range(0, 3);
                let (mut ncx, mut ncy, mut ns) = (cx, cy, s);
                match which {
                    0 => ncx = (cx + rng.normal_step(sigma_pos)).clamp(0, w as i32 - 1),
                    1 => ncy = (cy + rng.normal_step(sigma_pos)).clamp(0, h as i32 - 1),
                    _ => ns = (s + rng.normal_step(sigma_s)).clamp(1, s_max_global),
                }
                let (x0, y0, x1, y1) = square_bounds(ncx, ncy, ns);
                let res = eval_rect_integral(
                    &integral, x0, y0, x1, y1, FIXED_HILL_CLIMB_ALPHA, pal_ref,
                );
                if res.delta_sse < best_local_delta {
                    cx = ncx; cy = ncy; s = ns;
                    best_local_delta = res.delta_sse;
                    best_local_geom = (cx, cy, s);
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

        // Stage 3.
        let (cx, cy, s, alpha, color, pidx) = match best_geom {
            None => (
                w as i32 / 2,
                h as i32 / 2,
                (w.min(h) / 4).max(2) as i32,
                null_alpha,
                null_color,
                0u32,
            ),
            Some((cx, cy, s)) => {
                let (x0, y0, x1, y1) = square_bounds(cx, cy, s);
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
                (cx, cy, s, chosen_alpha, chosen.color, chosen.pidx)
            }
        };

        let (x0, y0, x1, y1) = square_bounds(cx, cy, s);
        apply_rect(&mut canvas, h as i32, w as i32, x0, y0, x1, y1, alpha, &color);
        integral.update_canvas(target, &canvas);
        residual.rebuild(target, &canvas);
        squares.push(Square { cx, cy, s, alpha, color, pidx });
    }
    (bg, squares)
}

pub fn encode_body(bw: &mut BitWriter, squares: &[Square], tw: u32, th: u32, codec: &Codec) {
    let alpha_levels = codec.alpha_levels_owned();
    for sq in squares {
        let (x_q, y_q) = quant_xy(sq.cx as f32, sq.cy as f32, tw, th, codec.cx_bits, codec.cy_bits);
        let s_q = r_to_q(sq.s as f32, tw, th, codec.r_bits);
        bw.write(x_q, codec.cx_bits);
        bw.write(y_q, codec.cy_bits);
        bw.write(s_q, codec.r_bits);
        write_color(bw, &sq.color, sq.pidx, codec);
        bw.write(alpha_to_q(sq.alpha, &alpha_levels), codec.alpha_bits);
    }
}

pub fn decode_render(br: &mut BitReader, codec: &Codec, w: u32, h: u32, canvas: &mut [f32]) {
    let x_max = (1u32 << codec.cx_bits) - 1;
    let y_max = (1u32 << codec.cy_bits) - 1;
    let alpha_levels = codec.alpha_levels_owned();
    for _ in 0..codec.n_shapes {
        let x_q = br.read(codec.cx_bits);
        let y_q = br.read(codec.cy_bits);
        let s_q = br.read(codec.r_bits);
        let color = read_color(br, codec);
        let a_q = br.read(codec.alpha_bits);
        let cx = (x_q as f32 / x_max as f32 * (w - 1) as f32).round() as i32;
        let cy = (y_q as f32 / y_max as f32 * (h - 1) as f32).round() as i32;
        let s = q_to_r(s_q, w, h, codec.r_bits).round() as i32;
        let alpha = q_to_alpha(a_q, &alpha_levels);
        let (x0, y0, x1, y1) = square_bounds(cx, cy, s);
        apply_rect(canvas, h as i32, w as i32, x0, y0, x1, y1, alpha, &color);
    }
}

pub fn encode_square(
    target: &[f32],
    th: u32,
    tw: u32,
    w_orig: u32,
    h_orig: u32,
    codec: &Codec,
    seed: u64,
    search: &SearchOptions,
) -> Vec<u8> {
    let (bg, squares) = fit_squares(target, th, tw, codec, seed, search);
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
    encode_body(&mut bw, &squares, tw, th, codec);
    bw.finish()
}

/// Decoder helper for SVG / introspection: reads one square's fields from
/// the body stream in the same bit layout as `encode_body`.
pub fn decode_square_at(
    br: &mut BitReader,
    codec: &Codec,
    w: u32,
    h: u32,
) -> (i32, i32, i32, [f32; 3], f32) {
    let x_max = ((1u32 << codec.cx_bits) - 1) as f32;
    let y_max = ((1u32 << codec.cy_bits) - 1) as f32;
    let alpha_levels = codec.alpha_levels_owned();
    let w_m1 = (w as f32 - 1.0).max(0.0);
    let h_m1 = (h as f32 - 1.0).max(0.0);
    let x_q = br.read(codec.cx_bits);
    let y_q = br.read(codec.cy_bits);
    let s_q = br.read(codec.r_bits);
    let color = read_color(br, codec);
    let alpha = q_to_alpha(br.read(codec.alpha_bits), &alpha_levels);
    let cx = ((x_q as f32) / x_max * w_m1).round() as i32;
    let cy = ((y_q as f32) / y_max * h_m1).round() as i32;
    let s = q_to_r(s_q, w, h, codec.r_bits).round() as i32;
    (cx, cy, s, color, alpha)
}

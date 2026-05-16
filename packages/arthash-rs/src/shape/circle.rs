//! CIRCLE mode — SQIP-style overlapping circles. SPEC §5.2.
//!
//! Primitive-style fit: tiny random init scaled to canvas, α-decoupled
//! Gaussian hill-climb, m independent attempts. After the climb, sweep all
//! quantized alphas to pick the best for the chosen geometry.

use super::integral::{collect_circle_sums_integral, eval_circle_integral, Integral};
use super::options::{SearchOptions, Strategy};
use super::quant::{
    alpha_to_q, aspect_code, q_to_alpha, q_to_r, quant_xy, r_to_q, read_color, write_color,
};
use super::raster::{apply_circle, EvalResult};
use super::residual::Residual;
use super::rng::Rng;
use crate::bitio::{BitReader, BitWriter};
use crate::codec::Codec;

/// Range of rows touched by a circle of radius `r` centered at `cy`, clamped
/// to `[0, h-1]`. Used to update the integral image after `apply_circle`.
#[inline]
fn circle_row_range(cy: i32, r: i32, h: u32) -> (i32, i32) {
    let ymin = (cy - r).max(0);
    let ymax = (cy + r).min(h as i32 - 1);
    (ymin, ymax)
}

/// Internal record carried out of `fit_circles` for `encode_body`.
#[derive(Clone, Debug)]
pub struct Circle {
    pub cx: i32,
    pub cy: i32,
    pub r: i32,
    pub alpha: f32,
    pub color: [f32; 3],
    pub pidx: u32,
}

/// α held fixed during the shape-only hill-climb (matches primitive's α=128).
const FIXED_HILL_CLIMB_ALPHA: f32 = 0.5;

/// Greedy fit of exactly `codec.n_shapes` circles. Returns (background,
/// circles).  `target` is row-major `(h, w, 3)` float32 linear-RGB.
pub fn fit_circles(
    target: &[f32],
    h: u32,
    w: u32,
    codec: &Codec,
    seed: u64,
    search: &SearchOptions,
) -> ([f32; 3], Vec<Circle>) {
    match search.strategy {
        Strategy::Primitive => fit_primitive(target, h, w, codec, seed, search),
        Strategy::TopkUniform => fit_topk_uniform(target, h, w, codec, seed, search),
    }
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

fn fit_primitive(
    target: &[f32],
    h: u32,
    w: u32,
    codec: &Codec,
    seed: u64,
    search: &SearchOptions,
) -> ([f32; 3], Vec<Circle>) {
    let bg = mean_rgb(target);
    let mut canvas = vec![0.0f32; (h * w * 3) as usize];
    for i in 0..(h * w) as usize {
        canvas[i * 3] = bg[0];
        canvas[i * 3 + 1] = bg[1];
        canvas[i * 3 + 2] = bg[2];
    }
    let mut integral = Integral::build(target, &canvas, h, w);
    let mut residual = Residual::build(target, &canvas, h, w);
    let mut rng = Rng::new(seed);
    let alpha_levels = codec.alpha_levels_owned();
    let palette = super::palette::from_codec(codec);
    let pal_ref = palette.as_ref();

    let long_edge = w.max(h);
    let sigma_pos = (2u32).max(long_edge * 6 / 100) as f64;
    let sigma_r = (1u32).max(long_edge * 6 / 100) as f64;
    let r_init_max = (2u32).max(long_edge * 12 / 100) as i64;
    let r_max_global = (1u32).max(long_edge) as i32;

    let use_max_age = search.hill_climb_max_age.is_some();
    let hard_cap = if use_max_age {
        10_000
    } else {
        search.hill_climb_steps
    };

    let null_color = bg;
    let null_alpha = alpha_levels[0];
    let fallback_radius = (2i32).max((w.min(h) / 4) as i32);
    let mut circles: Vec<Circle> = Vec::with_capacity(codec.n_shapes as usize);

    for _ in 0..codec.n_shapes {
        let mut best_delta_climb: f32 = -1e-3;
        let mut best_geom: Option<(i32, i32, i32)> = None;

        for _attempt in 0..search.n_attempts {
            // Stage 1: pick single best of n_random tiny-start candidates,
            // with centers sampled proportional to current residual.
            let mut best_d = f32::INFINITY;
            let mut best_init: Option<(i32, i32, i32)> = None;
            for _ in 0..search.n_random {
                let (cx, cy) = residual.sample(&mut rng);
                let r = rng.range(1, r_init_max + 1) as i32;
                let res = eval_circle_integral(
                    &integral, h, w, cx, cy, r, FIXED_HILL_CLIMB_ALPHA, pal_ref,
                );
                if res.delta_sse < best_d {
                    best_d = res.delta_sse;
                    best_init = Some((cx, cy, r));
                }
            }
            let Some((mut cx, mut cy, mut r)) = best_init else { continue };

            // Stage 2: Gaussian hill-climb on shape only (α fixed).
            let mut best_local_delta = best_d;
            let mut best_local_geom = (cx, cy, r);
            let mut age: u32 = 0;
            for _ in 0..hard_cap {
                let which = rng.range(0, 3);
                let (mut ncx, mut ncy, mut nr) = (cx, cy, r);
                match which {
                    0 => ncx = (cx + rng.normal_step(sigma_pos)).clamp(0, w as i32 - 1),
                    1 => ncy = (cy + rng.normal_step(sigma_pos)).clamp(0, h as i32 - 1),
                    _ => nr = (r + rng.normal_step(sigma_r)).clamp(1, r_max_global),
                }
                let res = eval_circle_integral(
                    &integral, h, w, ncx, ncy, nr, FIXED_HILL_CLIMB_ALPHA, pal_ref,
                );
                if res.delta_sse < best_local_delta {
                    cx = ncx;
                    cy = ncy;
                    r = nr;
                    best_local_delta = res.delta_sse;
                    best_local_geom = (cx, cy, r);
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

        // Stage 3: alpha sweep — collect once, finalize K times.
        let (cx, cy, r, alpha, color, pidx) = match best_geom {
            None => (
                w as i32 / 2,
                h as i32 / 2,
                fallback_radius,
                null_alpha,
                null_color,
                0u32,
            ),
            Some((cx, cy, r)) => {
                let sums = collect_circle_sums_integral(&integral, h, w, cx, cy, r);
                let mut best_a_delta = f32::INFINITY;
                let mut chosen = EvalResult {
                    delta_sse: 0.0,
                    color: null_color,
                    pidx: 0,
                };
                let mut chosen_alpha = null_alpha;
                for &a in &alpha_levels {
                    let res = sums.finalize(a, pal_ref);
                    if res.delta_sse < best_a_delta {
                        best_a_delta = res.delta_sse;
                        chosen = res;
                        chosen_alpha = a;
                    }
                }
                (cx, cy, r, chosen_alpha, chosen.color, chosen.pidx)
            }
        };

        apply_circle(&mut canvas, h as i32, w as i32, cx, cy, r, alpha, &color);
        let (ymin, ymax) = circle_row_range(cy, r, h);
        integral.update_canvas_rows(target, &canvas, ymin, ymax);
        residual.rebuild(target, &canvas);
        circles.push(Circle { cx, cy, r, alpha, color, pidx });
    }
    (bg, circles)
}

fn fit_topk_uniform(
    target: &[f32],
    h: u32,
    w: u32,
    codec: &Codec,
    seed: u64,
    search: &SearchOptions,
) -> ([f32; 3], Vec<Circle>) {
    // Historical strategy — log-spaced radii, uniform random pool, top-K
    // hill-climb with [-step, step] perturbation + step-decay.
    let bg = mean_rgb(target);
    let mut canvas = vec![0.0f32; (h * w * 3) as usize];
    for i in 0..(h * w) as usize {
        canvas[i * 3] = bg[0];
        canvas[i * 3 + 1] = bg[1];
        canvas[i * 3 + 2] = bg[2];
    }
    let mut integral = Integral::build(target, &canvas, h, w);
    let mut rng = Rng::new(seed);
    let alpha_levels = codec.alpha_levels_owned();
    let palette = super::palette::from_codec(codec);
    let pal_ref = palette.as_ref();

    let r_min = (4u32).max(w.min(h) / 8);
    let r_max = (r_min + 1).max(w.max(h));
    let n_radii = 16usize;
    let mut radii: Vec<i32> = Vec::new();
    let r_min_f = r_min as f32;
    let r_max_f = r_max as f32;
    let mut seen = std::collections::HashSet::new();
    for i in 0..n_radii {
        let t = (i as f32) / ((n_radii - 1) as f32);
        let r = (r_min_f * (r_max_f / r_min_f).powf(t)).round() as i32;
        let r = r.clamp(r_min as i32, r_max as i32);
        if seen.insert(r) {
            radii.push(r);
        }
    }

    let mut circles: Vec<Circle> = Vec::with_capacity(codec.n_shapes as usize);
    let null_color = bg;
    let null_alpha = alpha_levels[0];

    let use_max_age = search.hill_climb_max_age.is_some();
    let hard_cap = if use_max_age { 10_000 } else { search.hill_climb_steps };
    let step_divisor_pivot = (1u32).max(hard_cap / 3);

    for _ in 0..codec.n_shapes {
        let mut best_delta = -1e-3f32;
        let mut best: Option<Circle> = None;

        for _attempt in 0..search.n_attempts {
            let mut candidates: Vec<(f32, i32, i32, i32, f32, [f32; 3], u32, usize, usize)> =
                Vec::with_capacity(search.n_random as usize);
            for _ in 0..search.n_random {
                let cx = rng.range(0, w as i64) as i32;
                let cy = rng.range(0, h as i64) as i32;
                let r_idx = rng.range(0, radii.len() as i64) as usize;
                let r = radii[r_idx];
                let a_idx = rng.range(0, alpha_levels.len() as i64) as usize;
                let alpha = alpha_levels[a_idx];
                let res = eval_circle_integral(&integral, h, w, cx, cy, r, alpha, pal_ref);
                candidates.push((res.delta_sse, cx, cy, r, alpha, res.color, res.pidx, r_idx, a_idx));
            }
            candidates.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

            for &(d0, mut cx, mut cy, mut r, mut alpha, mut color, mut pidx, mut r_idx, mut a_idx) in
                candidates.iter().take(search.n_topk as usize)
            {
                let mut best_local_delta = d0;
                let mut best_local = Circle { cx, cy, r, alpha, color, pidx };
                let mut step = (2u32).max(w.max(h) / 12) as i32;
                let mut age = 0u32;
                for i in 0..hard_cap {
                    let which = rng.range(0, 4);
                    let (mut ncx, mut ncy, mut nr, mut nalpha) = (cx, cy, r, alpha);
                    let (mut nr_idx, mut na_idx) = (r_idx, a_idx);
                    match which {
                        0 => ncx = cx + rng.range_inclusive(-(step as i64), step as i64) as i32,
                        1 => ncy = cy + rng.range_inclusive(-(step as i64), step as i64) as i32,
                        2 => {
                            let delta: isize = if rng.next_u64() & 1 == 0 { -1 } else { 1 };
                            nr_idx = ((r_idx as isize + delta).clamp(0, radii.len() as isize - 1)) as usize;
                            nr = radii[nr_idx];
                        }
                        _ => {
                            let len = alpha_levels.len() as isize;
                            let delta: isize = if rng.next_u64() & 1 == 0 { -1 } else { 1 };
                            na_idx = ((a_idx as isize + delta).rem_euclid(len)) as usize;
                            nalpha = alpha_levels[na_idx];
                        }
                    }
                    let res = eval_circle_integral(&integral, h, w, ncx, ncy, nr, nalpha, pal_ref);
                    if res.delta_sse < best_local_delta {
                        cx = ncx; cy = ncy; r = nr; alpha = nalpha;
                        r_idx = nr_idx; a_idx = na_idx;
                        color = res.color; pidx = res.pidx;
                        best_local_delta = res.delta_sse;
                        best_local = Circle { cx, cy, r, alpha, color, pidx };
                        age = 0;
                    } else {
                        age += 1;
                        if let Some(max_age) = search.hill_climb_max_age {
                            if age >= max_age {
                                break;
                            }
                        }
                        if !use_max_age && i > 0 && i % step_divisor_pivot == 0 {
                            step = (1).max(step / 2);
                        }
                    }
                }
                if best_local_delta < best_delta {
                    best_delta = best_local_delta;
                    best = Some(best_local);
                }
            }
        }
        let chosen = best.unwrap_or(Circle {
            cx: (w / 2) as i32,
            cy: (h / 2) as i32,
            r: radii[0],
            alpha: null_alpha,
            color: null_color,
            pidx: 0,
        });
        apply_circle(
            &mut canvas, h as i32, w as i32,
            chosen.cx, chosen.cy, chosen.r, chosen.alpha, &chosen.color,
        );
        let (ymin, ymax) = circle_row_range(chosen.cy, chosen.r, h);
        integral.update_canvas_rows(target, &canvas, ymin, ymax);
        circles.push(chosen);
    }
    (bg, circles)
}

pub fn encode_body(
    bw: &mut BitWriter,
    circles: &[Circle],
    tw: u32,
    th: u32,
    codec: &Codec,
) {
    for c in circles {
        let (x_q, y_q) = quant_xy(c.cx as f32, c.cy as f32, tw, th, codec.cx_bits, codec.cy_bits);
        let r_q = r_to_q(c.r as f32, tw, th, codec.r_bits);
        let a_q = alpha_to_q(c.alpha, &codec.alpha_levels_owned());
        bw.write(x_q, codec.cx_bits);
        bw.write(y_q, codec.cy_bits);
        bw.write(r_q, codec.r_bits);
        write_color(bw, &c.color, c.pidx, codec);
        bw.write(a_q, codec.alpha_bits);
    }
}

/// Decode + render N circles onto `canvas` (linear-RGB float32, h*w*3).
pub fn decode_render(br: &mut BitReader, codec: &Codec, w: u32, h: u32, canvas: &mut [f32]) {
    let x_max = (1u32 << codec.cx_bits) - 1;
    let y_max = (1u32 << codec.cy_bits) - 1;
    let alpha_levels = codec.alpha_levels_owned();
    for _ in 0..codec.n_shapes {
        let x_q = br.read(codec.cx_bits);
        let y_q = br.read(codec.cy_bits);
        let r_q = br.read(codec.r_bits);
        let color = read_color(br, codec);
        let a_q = br.read(codec.alpha_bits);
        let cx = (x_q as f32 / x_max as f32 * (w - 1) as f32).round() as i32;
        let cy = (y_q as f32 / y_max as f32 * (h - 1) as f32).round() as i32;
        let r = q_to_r(r_q, w, h, codec.r_bits).round() as i32;
        let alpha = q_to_alpha(a_q, &alpha_levels);
        apply_circle(canvas, h as i32, w as i32, cx, cy, r, alpha, &color);
    }
}

/// Encode an RGB thumbnail at native shape resolution → bytes (header + body).
pub fn encode_circle(
    target: &[f32],
    th: u32,
    tw: u32,
    w_orig: u32,
    h_orig: u32,
    codec: &Codec,
    seed: u64,
    search: &SearchOptions,
) -> Vec<u8> {
    let (bg, circles) = fit_circles(target, th, tw, codec, seed, search);
    let mut bw = BitWriter::new();
    bw.write(aspect_code(w_orig, h_orig), 8);
    // Choose bg palette index for palette mode (nearest). One-shot lookup,
    // so skip the LUT build that `PaletteIndex` would amortize.
    let pidx = super::palette::nearest_in_codec(codec, bg).unwrap_or(0);
    write_color(&mut bw, &bg, pidx, codec);
    encode_body(&mut bw, &circles, tw, th, codec);
    bw.finish()
}

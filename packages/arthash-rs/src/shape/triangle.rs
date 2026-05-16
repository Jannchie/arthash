//! TRIANGLE mode — primitive-style mosaic. SPEC §5.3.
//!
//! Primitive-style fit: tiny-clustered random init, α-decoupled Gaussian
//! hill-climb with retry-on-invalid, m attempts. Uses a pre-quantization
//! 17° min-angle gate (absorbs 5-bit grid snap noise) to keep decoded
//! triangles ≥ 15° — matches Python's discipline.

use super::integral::{collect_triangle_sums_integral, eval_triangle_integral, Integral};
use super::options::{SearchOptions, Strategy};
use super::quant::{
    alpha_to_q, aspect_code, q_to_alpha, quant_xy, read_color, write_color,
};
use super::raster::apply_triangle;
use super::rng::Rng;
use crate::bitio::{BitReader, BitWriter};
use crate::codec::Codec;

#[inline]
fn triangle_row_range(verts: [(i32, i32); 3], h: u32) -> (i32, i32) {
    let ymin = verts[0].1.min(verts[1].1).min(verts[2].1).max(0);
    let ymax = verts[0].1.max(verts[1].1).max(verts[2].1).min(h as i32 - 1);
    (ymin, ymax)
}

#[derive(Clone, Debug)]
pub struct Triangle {
    pub verts: [(i32, i32); 3],
    pub alpha: f32,
    pub color: [f32; 3],
    pub pidx: u32,
}

const FIXED_HILL_CLIMB_ALPHA: f32 = 0.5;

/// sin²(17°) — pre-quant threshold (absorbs 5-bit grid snap noise so the
/// stored geometry has ≥ 15° internal angles).
const SIN_MIN_SQ: f64 = 0.0854708354;

/// Pure-integer check: all three internal angles ≥ 17° (pre-quant). For
/// each vertex A with neighbours B, C:
///     sin²(angle_A) = (AB × AC)² / (|AB|² · |AC|²)
fn all_angles_ok(x0: i32, y0: i32, x1: i32, y1: i32, x2: i32, y2: i32) -> bool {
    #[inline]
    fn check(ax: i64, ay: i64, bx: i64, by: i64) -> bool {
        let cross = ax * by - ay * bx;
        let l1 = ax * ax + ay * ay;
        let l2 = bx * bx + by * by;
        if l1 == 0 || l2 == 0 {
            return false;
        }
        (cross as f64).powi(2) >= SIN_MIN_SQ * (l1 as f64) * (l2 as f64)
    }
    let (x0, y0, x1, y1, x2, y2) = (x0 as i64, y0 as i64, x1 as i64, y1 as i64, x2 as i64, y2 as i64);
    check(x1 - x0, y1 - y0, x2 - x0, y2 - y0)
        && check(x0 - x1, y0 - y1, x2 - x1, y2 - y1)
        && check(x0 - x2, y0 - y2, x1 - x2, y1 - y2)
}

fn random_triangle_small(rng: &mut Rng, h: u32, w: u32) -> [(i32, i32); 3] {
    let spread = (2u32).max(w.max(h) * 6 / 100) as i64;
    let mut last = [(0, 0); 3];
    for _ in 0..64 {
        let cx = rng.range(0, w as i64) as i32;
        let cy = rng.range(0, h as i64) as i32;
        let x2 = (cx as i64 + rng.range_inclusive(-spread, spread)).clamp(0, w as i64 - 1) as i32;
        let y2 = (cy as i64 + rng.range_inclusive(-spread, spread)).clamp(0, h as i64 - 1) as i32;
        let x3 = (cx as i64 + rng.range_inclusive(-spread, spread)).clamp(0, w as i64 - 1) as i32;
        let y3 = (cy as i64 + rng.range_inclusive(-spread, spread)).clamp(0, h as i64 - 1) as i32;
        let v = [(cx, cy), (x2, y2), (x3, y3)];
        last = v;
        if all_angles_ok(cx, cy, x2, y2, x3, y3) {
            return v;
        }
    }
    last
}

#[allow(clippy::too_many_arguments)]
fn hill_climb_gaussian(
    integral: &Integral,
    h: u32,
    w: u32,
    mut verts: [(i32, i32); 3],
    fixed_alpha: f32,
    palette: Option<&super::palette::PaletteIndex>,
    n_steps: u32,
    rng: &mut Rng,
    sigma: f64,
    max_age: Option<u32>,
) -> ([(i32, i32); 3], f32, [f32; 3], u32) {
    let res0 = eval_triangle_integral(
        integral, h, w,
        verts[0].0, verts[0].1, verts[1].0, verts[1].1, verts[2].0, verts[2].1,
        fixed_alpha, palette,
    );
    let mut best_delta = res0.delta_sse;
    let mut best_color = res0.color;
    let mut best_pidx = res0.pidx;
    let mut age: u32 = 0;
    for _ in 0..n_steps {
        let which = rng.range(0, 3) as usize;
        // Retry-on-invalid: same loop as primitive's Mutate().
        let mut new_verts: Option<[(i32, i32); 3]> = None;
        for _retry in 0..16 {
            let mut cand = verts;
            let dx = (rng.normal() * sigma) as i32;
            let dy = (rng.normal() * sigma) as i32;
            cand[which].0 = (cand[which].0 + dx).clamp(0, w as i32 - 1);
            cand[which].1 = (cand[which].1 + dy).clamp(0, h as i32 - 1);
            if all_angles_ok(
                cand[0].0, cand[0].1, cand[1].0, cand[1].1, cand[2].0, cand[2].1,
            ) {
                new_verts = Some(cand);
                break;
            }
        }
        let Some(cand) = new_verts else {
            age += 1;
            if let Some(max_age) = max_age {
                if age >= max_age {
                    break;
                }
            }
            continue;
        };
        let res = eval_triangle_integral(
            integral, h, w,
            cand[0].0, cand[0].1, cand[1].0, cand[1].1, cand[2].0, cand[2].1,
            fixed_alpha, palette,
        );
        if res.delta_sse < best_delta {
            verts = cand;
            best_delta = res.delta_sse;
            best_color = res.color;
            best_pidx = res.pidx;
            age = 0;
        } else {
            age += 1;
            if let Some(max_age) = max_age {
                if age >= max_age {
                    break;
                }
            }
        }
    }
    (verts, best_delta, best_color, best_pidx)
}

fn pick_best_alpha(
    integral: &Integral,
    h: u32,
    w: u32,
    verts: [(i32, i32); 3],
    alpha_levels: &[f32],
    palette: Option<&super::palette::PaletteIndex>,
) -> (f32, f32, [f32; 3], u32) {
    // Collect once, finalize K times — geometry is fixed.
    let sums = collect_triangle_sums_integral(
        integral, h, w,
        verts[0].0, verts[0].1, verts[1].0, verts[1].1, verts[2].0, verts[2].1,
    );
    let mut best_delta = f32::INFINITY;
    let mut best_alpha = alpha_levels[0];
    let mut best_color = [0.0f32; 3];
    let mut best_pidx = 0u32;
    for &a in alpha_levels {
        let res = sums.finalize(a, palette);
        if res.delta_sse < best_delta {
            best_delta = res.delta_sse;
            best_alpha = a;
            best_color = res.color;
            best_pidx = res.pidx;
        }
    }
    (best_alpha, best_delta, best_color, best_pidx)
}

pub fn fit_triangles(
    target: &[f32],
    h: u32,
    w: u32,
    codec: &Codec,
    seed: u64,
    search: &SearchOptions,
) -> ([f32; 3], Vec<Triangle>) {
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
) -> ([f32; 3], Vec<Triangle>) {
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

    let sigma = (2.0f64).max(w.max(h) as f64 * 6.0 / 100.0);
    let use_max_age = search.hill_climb_max_age.is_some();
    let hard_cap = if use_max_age { 10_000 } else { search.hill_climb_steps };

    let null_alpha = alpha_levels[0];
    let fallback_verts: [(i32, i32); 3] = [(0, 0), (1, 0), (0, 1)];

    let mut triangles: Vec<Triangle> = Vec::with_capacity(codec.n_shapes as usize);

    for _ in 0..codec.n_shapes {
        let mut best_delta_climb: f32 = -1e-3;
        let mut best_verts: Option<[(i32, i32); 3]> = None;

        for _attempt in 0..search.n_attempts {
            // Stage 1: best of n_random tiny-cluster random.
            let mut best_d = f32::INFINITY;
            let mut best_init: Option<[(i32, i32); 3]> = None;
            for _ in 0..search.n_random {
                let v = random_triangle_small(&mut rng, h, w);
                let res = eval_triangle_integral(
                    &integral, h, w,
                    v[0].0, v[0].1, v[1].0, v[1].1, v[2].0, v[2].1,
                    FIXED_HILL_CLIMB_ALPHA, pal_ref,
                );
                if res.delta_sse < best_d {
                    best_d = res.delta_sse;
                    best_init = Some(v);
                }
            }
            let Some(v0) = best_init else { continue };

            // Stage 2: Gaussian hill climb.
            let (v, d, _c, _p) = hill_climb_gaussian(
                &integral, h, w, v0,
                FIXED_HILL_CLIMB_ALPHA, pal_ref,
                hard_cap, &mut rng, sigma, search.hill_climb_max_age,
            );
            if d < best_delta_climb {
                best_delta_climb = d;
                best_verts = Some(v);
            }
        }

        let (verts, alpha, color, pidx) = match best_verts {
            None => (fallback_verts, null_alpha, bg, 0u32),
            Some(v) => {
                let (a, _d, c, p) = pick_best_alpha(&integral, h, w, v, &alpha_levels, pal_ref);
                (v, a, c, p)
            }
        };
        apply_triangle(&mut canvas, h as i32, w as i32, verts, alpha, &color);
        let (ymin, ymax) = triangle_row_range(verts, h);
        integral.update_canvas_rows(target, &canvas, ymin, ymax);
        triangles.push(Triangle { verts, alpha, color, pidx });
    }
    (bg, triangles)
}

fn fit_topk_uniform(
    target: &[f32],
    h: u32,
    w: u32,
    codec: &Codec,
    seed: u64,
    search: &SearchOptions,
) -> ([f32; 3], Vec<Triangle>) {
    // Uniform random pool over full canvas + off-canvas margin, top-K
    // uniform-step climb. Historical arthash mode.
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

    let margin = (w.max(h) / 4) as i64;
    let use_max_age = search.hill_climb_max_age.is_some();
    let hard_cap = if use_max_age { 10_000 } else { search.hill_climb_steps };
    let pivot = (1u32).max(hard_cap / 3);
    let null_alpha = alpha_levels[0];
    let fallback_verts: [(i32, i32); 3] = [(0, 0), (1, 0), (0, 1)];

    let mut triangles: Vec<Triangle> = Vec::with_capacity(codec.n_shapes as usize);

    for _ in 0..codec.n_shapes {
        let mut best_delta = -1e-3f32;
        let mut best_t: Option<Triangle> = None;

        for _attempt in 0..search.n_attempts {
            let mut candidates: Vec<(f32, [(i32, i32); 3], f32, [f32; 3], u32)> =
                Vec::with_capacity(search.n_random as usize);
            for _ in 0..search.n_random {
                let v = [
                    (
                        rng.range(-margin, w as i64 + margin) as i32,
                        rng.range(-margin, h as i64 + margin) as i32,
                    ),
                    (
                        rng.range(-margin, w as i64 + margin) as i32,
                        rng.range(-margin, h as i64 + margin) as i32,
                    ),
                    (
                        rng.range(-margin, w as i64 + margin) as i32,
                        rng.range(-margin, h as i64 + margin) as i32,
                    ),
                ];
                let a_idx = rng.range(0, alpha_levels.len() as i64) as usize;
                let alpha = alpha_levels[a_idx];
                let res = eval_triangle_integral(
                    &integral, h, w,
                    v[0].0, v[0].1, v[1].0, v[1].1, v[2].0, v[2].1, alpha, pal_ref,
                );
                candidates.push((res.delta_sse, v, alpha, res.color, res.pidx));
            }
            candidates.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

            for &(_d, mut verts, mut alpha, mut color, mut pidx) in
                candidates.iter().take(search.n_topk as usize)
            {
                let (mut step, mut age) = ((2u32).max(w.max(h) / 10) as i32, 0u32);
                let mut a_idx = alpha_levels
                    .iter()
                    .enumerate()
                    .min_by(|(_, a), (_, b)| {
                        (alpha - **a)
                            .abs()
                            .partial_cmp(&(alpha - **b).abs())
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .map(|(i, _)| i)
                    .unwrap_or(0);
                let mut best_local = Triangle { verts, alpha, color, pidx };
                let mut best_local_delta = _d;
                for i in 0..hard_cap {
                    let which = rng.range(0, 7);
                    let mut nv = verts;
                    let mut na = alpha;
                    let mut na_idx = a_idx;
                    if which < 6 {
                        let vi = (which / 2) as usize;
                        let coord = (which % 2) as usize;
                        let delta = rng.range_inclusive(-(step as i64), step as i64) as i32;
                        if coord == 0 {
                            nv[vi].0 += delta;
                        } else {
                            nv[vi].1 += delta;
                        }
                    } else {
                        let len = alpha_levels.len() as isize;
                        let s: isize = if rng.next_u64() & 1 == 0 { -1 } else { 1 };
                        na_idx = ((a_idx as isize + s).rem_euclid(len)) as usize;
                        na = alpha_levels[na_idx];
                    }
                    let res = eval_triangle_integral(
                        &integral, h, w,
                        nv[0].0, nv[0].1, nv[1].0, nv[1].1, nv[2].0, nv[2].1, na, pal_ref,
                    );
                    if res.delta_sse < best_local_delta {
                        verts = nv;
                        alpha = na;
                        if which >= 6 {
                            a_idx = na_idx;
                        }
                        color = res.color;
                        pidx = res.pidx;
                        best_local_delta = res.delta_sse;
                        best_local = Triangle { verts, alpha, color, pidx };
                        age = 0;
                    } else {
                        age += 1;
                        if let Some(max_age) = search.hill_climb_max_age {
                            if age >= max_age {
                                break;
                            }
                        }
                        if !use_max_age && i > 0 && i % pivot == 0 {
                            step = (1).max(step / 2);
                        }
                    }
                }
                if best_local_delta < best_delta {
                    best_delta = best_local_delta;
                    best_t = Some(best_local);
                }
            }
        }
        let chosen = best_t.unwrap_or(Triangle {
            verts: fallback_verts,
            alpha: null_alpha,
            color: bg,
            pidx: 0,
        });
        apply_triangle(
            &mut canvas, h as i32, w as i32,
            chosen.verts, chosen.alpha, &chosen.color,
        );
        let (ymin, ymax) = triangle_row_range(chosen.verts, h);
        integral.update_canvas_rows(target, &canvas, ymin, ymax);
        triangles.push(chosen);
    }
    (bg, triangles)
}

pub fn encode_body(
    bw: &mut BitWriter,
    triangles: &[Triangle],
    tw: u32,
    th: u32,
    codec: &Codec,
) {
    let alpha_levels = codec.alpha_levels_owned();
    for t in triangles {
        for vi in 0..3 {
            let (vx, vy) = t.verts[vi];
            let (x_q, y_q) = quant_xy(vx as f32, vy as f32, tw, th, codec.cx_bits, codec.cy_bits);
            bw.write(x_q, codec.cx_bits);
            bw.write(y_q, codec.cy_bits);
        }
        write_color(bw, &t.color, t.pidx, codec);
        bw.write(alpha_to_q(t.alpha, &alpha_levels), codec.alpha_bits);
    }
}

pub fn decode_render(br: &mut BitReader, codec: &Codec, w: u32, h: u32, canvas: &mut [f32]) {
    let x_max = (1u32 << codec.cx_bits) - 1;
    let y_max = (1u32 << codec.cy_bits) - 1;
    let alpha_levels = codec.alpha_levels_owned();
    for _ in 0..codec.n_shapes {
        let mut verts = [(0i32, 0i32); 3];
        for vert in verts.iter_mut() {
            let x_q = br.read(codec.cx_bits);
            let y_q = br.read(codec.cy_bits);
            vert.0 = (x_q as f32 / x_max as f32 * (w - 1) as f32).round() as i32;
            vert.1 = (y_q as f32 / y_max as f32 * (h - 1) as f32).round() as i32;
        }
        let color = read_color(br, codec);
        let alpha = q_to_alpha(br.read(codec.alpha_bits), &alpha_levels);
        apply_triangle(canvas, h as i32, w as i32, verts, alpha, &color);
    }
}

pub fn encode_triangle(
    target: &[f32],
    th: u32,
    tw: u32,
    w_orig: u32,
    h_orig: u32,
    codec: &Codec,
    seed: u64,
    search: &SearchOptions,
) -> Vec<u8> {
    let (bg, triangles) = fit_triangles(target, th, tw, codec, seed, search);
    let mut bw = BitWriter::new();
    bw.write(aspect_code(w_orig, h_orig), 8);
    let pidx = super::palette::nearest_in_codec(codec, bg).unwrap_or(0);
    write_color(&mut bw, &bg, pidx, codec);
    encode_body(&mut bw, &triangles, tw, th, codec);
    bw.finish()
}

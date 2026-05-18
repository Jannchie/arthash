//! ROTATED_RECT mode — arbitrary-angle rectangles. SPEC §5.6.
//!
//! Five geometric DoF: (cx, cy, w, h, θ). Eval uses the existing row-wise
//! [`super::integral::Integral`] via 4 half-plane spans per row — same
//! pattern as triangle, with one extra edge. The 2D Viola-Jones 4-corner
//! lookup only works for axis-aligned rects, so rotated rects fall back to
//! the row-integral path.
//!
//! θ is stored in `[0, π)` (centro-symmetry: rotating a rect by π yields the
//! same shape). The byte width is `codec.theta_bits` — default 5 bits = 32
//! levels ≈ 5.6° resolution.

use super::integral::{collect_quad_sums_integral, eval_quad_integral, Integral};
use super::options::SearchOptions;
use super::quant::{
    alpha_to_q, aspect_code, dim_to_q, q_to_alpha, q_to_dim, q_to_theta, quant_xy, read_color,
    theta_to_q, write_color,
};
use super::raster::{apply_quad, apply_rotated_rounded_rect_aa, quad_row_range, EvalResult};
use super::residual::Residual;
use super::rng::Rng;
use crate::bitio::{BitReader, BitWriter};
use crate::codec::CodecConfig as Codec;

const FIXED_HILL_CLIMB_ALPHA: f32 = 0.5;

#[derive(Clone, Debug)]
pub struct RotRect {
    pub cx: i32,
    pub cy: i32,
    pub w: i32,
    pub h: i32,
    pub theta: f32,
    pub alpha: f32,
    pub color: [f32; 3],
    pub pidx: u32,
}

/// 4 corner vertices in CCW order (for theta=0, w>0, h>0). The signed-area
/// auto-detection in `collect_quad_sums_integral` handles negative w/h or
/// CW winding without callers needing to normalize.
fn rotrect_verts(cx: i32, cy: i32, w: i32, h: i32, theta: f32) -> [(i32, i32); 4] {
    let hw = w as f32 / 2.0;
    let hh = h as f32 / 2.0;
    let c = theta.cos();
    let s = theta.sin();
    // Local-frame corners, going CCW from bottom-left when theta = 0.
    let local = [(-hw, -hh), (hw, -hh), (hw, hh), (-hw, hh)];
    let mut out = [(0i32, 0i32); 4];
    for (i, &(lx, ly)) in local.iter().enumerate() {
        let wx = lx * c - ly * s;
        let wy = lx * s + ly * c;
        out[i] = ((cx as f32 + wx).round() as i32, (cy as f32 + wy).round() as i32);
    }
    out
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

pub fn fit_rotrects(
    target: &[f32],
    h: u32,
    w: u32,
    codec: &Codec,
    seed: u64,
    search: &SearchOptions,
) -> ([f32; 3], Vec<RotRect>) {
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
    let sigma_wh = (1u32).max(long_edge * 6 / 100) as f64;
    let sigma_theta: f64 = std::f64::consts::PI / 12.0; // ≈15° per step
    let init_max = (2u32).max(long_edge * 25 / 100) as i64;
    let dim_max = w.max(h) as i32;

    let use_max_age = search.hill_climb_max_age.is_some();
    let hard_cap = if use_max_age { 10_000 } else { search.hill_climb_steps };
    let null_color = bg;
    let null_alpha = alpha_levels[0];

    let mut rects: Vec<RotRect> = Vec::with_capacity(codec.n_shapes as usize);

    for _ in 0..codec.n_shapes {
        let mut best_delta_climb: f32 = -1e-3;
        let mut best_geom: Option<(i32, i32, i32, i32, f32)> = None;

        for _attempt in 0..search.n_attempts {
            // Stage 1.
            let mut best_d = f32::INFINITY;
            let mut best_init: Option<(i32, i32, i32, i32, f32)> = None;
            for _ in 0..search.n_random {
                let (cx, cy) = residual.sample(&mut rng);
                let rw = rng.range(2, init_max + 1) as i32;
                let rh = rng.range(2, init_max + 1) as i32;
                let theta = rng.next_f64() as f32 * std::f32::consts::PI;
                let verts = rotrect_verts(cx, cy, rw, rh, theta);
                let res = eval_quad_integral(
                    &integral, h, w, verts, FIXED_HILL_CLIMB_ALPHA, pal_ref,
                );
                if res.delta_sse < best_d {
                    best_d = res.delta_sse;
                    best_init = Some((cx, cy, rw, rh, theta));
                }
            }
            let Some((mut cx, mut cy, mut rw, mut rh, mut theta)) = best_init else { continue };

            // Stage 2 — 5 axes (cx, cy, w, h, theta).
            let mut best_local_delta = best_d;
            let mut best_local_geom = (cx, cy, rw, rh, theta);
            let mut age: u32 = 0;
            for _ in 0..hard_cap {
                let which = rng.range(0, 5);
                let (mut ncx, mut ncy, mut nw, mut nh, mut nt) = (cx, cy, rw, rh, theta);
                match which {
                    0 => ncx = (cx + rng.normal_step(sigma_pos)).clamp(0, w as i32 - 1),
                    1 => ncy = (cy + rng.normal_step(sigma_pos)).clamp(0, h as i32 - 1),
                    2 => nw = (rw + rng.normal_step(sigma_wh)).clamp(2, dim_max),
                    3 => nh = (rh + rng.normal_step(sigma_wh)).clamp(2, dim_max),
                    _ => {
                        let dt = (rng.normal() * sigma_theta) as f32;
                        nt = (theta + dt).rem_euclid(std::f32::consts::PI);
                    }
                }
                let verts = rotrect_verts(ncx, ncy, nw, nh, nt);
                let res = eval_quad_integral(
                    &integral, h, w, verts, FIXED_HILL_CLIMB_ALPHA, pal_ref,
                );
                if res.delta_sse < best_local_delta {
                    cx = ncx; cy = ncy; rw = nw; rh = nh; theta = nt;
                    best_local_delta = res.delta_sse;
                    best_local_geom = (cx, cy, rw, rh, theta);
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

        // Stage 3 — α-sweep.
        let (cx, cy, rw, rh, theta, alpha, color, pidx) = match best_geom {
            None => (
                w as i32 / 2,
                h as i32 / 2,
                (w / 4).max(2) as i32,
                (h / 4).max(2) as i32,
                0.0f32,
                null_alpha,
                null_color,
                0u32,
            ),
            Some((cx, cy, rw, rh, theta)) => {
                let verts = rotrect_verts(cx, cy, rw, rh, theta);
                let sums = collect_quad_sums_integral(&integral, h, w, verts);
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
                (cx, cy, rw, rh, theta, chosen_alpha, chosen.color, chosen.pidx)
            }
        };

        let verts = rotrect_verts(cx, cy, rw, rh, theta);
        apply_quad(&mut canvas, h as i32, w as i32, verts, alpha, &color);
        let (ymin, ymax) = quad_row_range(verts, h);
        integral.update_canvas_rows(target, &canvas, ymin, ymax);
        residual.rebuild(target, &canvas);
        rects.push(RotRect { cx, cy, w: rw, h: rh, theta, alpha, color, pidx });
    }
    (bg, rects)
}

pub fn encode_body(bw: &mut BitWriter, rects: &[RotRect], tw: u32, th: u32, codec: &Codec) {
    let alpha_levels = codec.alpha_levels_owned();
    for r in rects {
        let (x_q, y_q) = quant_xy(r.cx as f32, r.cy as f32, tw, th, codec.cx_bits, codec.cy_bits);
        let w_q = dim_to_q(r.w as f32, tw, codec.r_bits);
        let h_q = dim_to_q(r.h as f32, th, codec.r_bits);
        let t_q = theta_to_q(r.theta, codec.theta_bits);
        bw.write(x_q, codec.cx_bits);
        bw.write(y_q, codec.cy_bits);
        bw.write(w_q, codec.r_bits);
        bw.write(h_q, codec.r_bits);
        bw.write(t_q, codec.theta_bits);
        write_color(bw, &r.color, r.pidx, codec);
        bw.write(alpha_to_q(r.alpha, &alpha_levels), codec.alpha_bits);
    }
}

/// Decode + render rotated rects into `canvas`. `corner_radius` is in canvas
/// pixel units; `0` keeps the existing hard-edge `apply_quad` fast path
/// byte-identical to pre-0.3.0 output.
pub fn decode_render(
    br: &mut BitReader,
    codec: &Codec,
    w: u32,
    h: u32,
    canvas: &mut [f32],
    corner_radius: f32,
) {
    let x_max = (1u32 << codec.cx_bits) - 1;
    let y_max = (1u32 << codec.cy_bits) - 1;
    let alpha_levels = codec.alpha_levels_owned();
    let use_aa = corner_radius > 0.0;
    for _ in 0..codec.n_shapes {
        let x_q = br.read(codec.cx_bits);
        let y_q = br.read(codec.cy_bits);
        let w_q = br.read(codec.r_bits);
        let h_q = br.read(codec.r_bits);
        let t_q = br.read(codec.theta_bits);
        let color = read_color(br, codec);
        let a_q = br.read(codec.alpha_bits);
        let cx = (x_q as f32 / x_max as f32 * (w - 1) as f32).round() as i32;
        let cy = (y_q as f32 / y_max as f32 * (h - 1) as f32).round() as i32;
        let rw = q_to_dim(w_q, w, codec.r_bits).round() as i32;
        let rh = q_to_dim(h_q, h, codec.r_bits).round() as i32;
        let theta = q_to_theta(t_q, codec.theta_bits);
        let alpha = q_to_alpha(a_q, &alpha_levels);
        if use_aa {
            apply_rotated_rounded_rect_aa(
                canvas, h as i32, w as i32,
                cx as f32, cy as f32, rw as f32, rh as f32,
                theta, corner_radius, alpha, &color,
            );
        } else {
            let verts = rotrect_verts(cx, cy, rw, rh, theta);
            apply_quad(canvas, h as i32, w as i32, verts, alpha, &color);
        }
    }
}

pub fn encode_rotrect(
    target: &[f32],
    th: u32,
    tw: u32,
    w_orig: u32,
    h_orig: u32,
    codec: &Codec,
    seed: u64,
    search: &SearchOptions,
) -> Vec<u8> {
    let (bg, rects) = fit_rotrects(target, th, tw, codec, seed, search);
    let mut bw = BitWriter::new();
    bw.write(aspect_code(w_orig, h_orig), 8);
    let pidx = super::palette::nearest_in_codec(codec, bg).unwrap_or(0);
    write_color(&mut bw, &bg, pidx, codec);
    encode_body(&mut bw, &rects, tw, th, codec);
    bw.finish()
}

/// Decoder helper for SVG: reads one rotated rect's fields and returns the
/// resolved (cx, cy, w, h, θ_deg, color, alpha). Theta is returned in
/// **degrees** since SVG's `rotate()` takes degrees.
pub fn decode_rotrect_at(
    br: &mut BitReader,
    codec: &Codec,
    w: u32,
    h: u32,
) -> (i32, i32, i32, i32, f32, [f32; 3], f32) {
    let x_max = ((1u32 << codec.cx_bits) - 1) as f32;
    let y_max = ((1u32 << codec.cy_bits) - 1) as f32;
    let alpha_levels = codec.alpha_levels_owned();
    let w_m1 = (w as f32 - 1.0).max(0.0);
    let h_m1 = (h as f32 - 1.0).max(0.0);
    let x_q = br.read(codec.cx_bits);
    let y_q = br.read(codec.cy_bits);
    let w_q = br.read(codec.r_bits);
    let h_q = br.read(codec.r_bits);
    let t_q = br.read(codec.theta_bits);
    let color = read_color(br, codec);
    let alpha = q_to_alpha(br.read(codec.alpha_bits), &alpha_levels);
    let cx = ((x_q as f32) / x_max * w_m1).round() as i32;
    let cy = ((y_q as f32) / y_max * h_m1).round() as i32;
    let rw = q_to_dim(w_q, w, codec.r_bits).round() as i32;
    let rh = q_to_dim(h_q, h, codec.r_bits).round() as i32;
    let theta_rad = q_to_theta(t_q, codec.theta_bits);
    let theta_deg = theta_rad.to_degrees();
    (cx, cy, rw, rh, theta_deg, color, alpha)
}

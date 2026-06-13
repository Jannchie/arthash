//! RECT mode — axis-aligned rectangles. SPEC §5.5.
//!
//! Each shape stores (cx, cy, w, h, color, alpha). The fit uses the same
//! primitive-style two-stage search as CIRCLE / TRIANGLE: residual-weighted
//! init, Gaussian hill-climb with `normal_step`, α-sweep finalize. Eval is
//! O(1) via [`super::integral2d::Integral2D`] — the Viola-Jones 4-corner
//! lookup over a 2D prefix-sum image.

use super::integral2d::{collect_rect_sums_integral, eval_rect_integral, Integral2D};
use super::options::SearchOptions;
use super::quant::{
    alpha_to_q, aspect_code, dequant_xy, dim_to_q, q_to_alpha, q_to_dim, quant_xy, read_color,
    write_color,
};
use super::common::{alpha_sweep, filled_canvas, mean_rgb, refine_shapes, FIXED_HILL_CLIMB_ALPHA};
use super::raster::{apply_rect, apply_rounded_rect_aa};
use super::residual::Residual;
use super::rng::Rng;
use crate::bitio::{BitReader, BitWriter};
use crate::codec::CodecConfig as Codec;

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

pub fn fit_rects(
    target: &[f32],
    h: u32,
    w: u32,
    codec: &Codec,
    seed: u64,
    search: &SearchOptions,
) -> ([f32; 3], Vec<Rect>) {
    let bg = mean_rgb(target);
    let mut canvas = filled_canvas(bg, h, w);
    let mut integral = Integral2D::build(target, &canvas, h, w);
    let mut residual = Residual::build(target, &canvas, h, w);
    let mut rng = Rng::new(seed);
    let alpha_levels = codec.alpha_levels_owned();
    let palette = super::palette::from_codec(codec);
    let pal_ref = palette.as_ref();

    let long_edge = w.max(h);
    let params = RectSearchParams {
        sigma_pos: (2u32).max(long_edge * 6 / 100) as f64,
        sigma_wh: (1u32).max(long_edge * 6 / 100) as f64,
        init_max: (2u32).max(long_edge * 25 / 100) as i64,
        dim_min: 1i32,
        dim_max: w.max(h) as i32,
        hard_cap: if search.hill_climb_max_age.is_some() { 10_000 } else { search.hill_climb_steps },
    };
    let null_color = bg;
    let null_alpha = alpha_levels[0];

    let mut rects: Vec<Rect> = Vec::with_capacity(codec.n_shapes as usize);

    for _ in 0..codec.n_shapes {
        let r = search_rect(&integral, &residual, &mut rng, h, w, search, pal_ref, &alpha_levels, &params)
            .unwrap_or(Rect {
                cx: w as i32 / 2,
                cy: h as i32 / 2,
                w: (w / 4).max(2) as i32,
                h: (h / 4).max(2) as i32,
                alpha: null_alpha,
                color: null_color,
                pidx: 0,
            });
        let (x0, y0, x1, y1) = rect_bounds(r.cx, r.cy, r.w, r.h);
        apply_rect(&mut canvas, h as i32, w as i32, x0, y0, x1, y1, r.alpha, &r.color);
        integral.update_canvas_from_row(target, &canvas, y0);
        residual.rebuild_from(target, &canvas, y0.max(0) as usize * w as usize);
        rects.push(r);
    }

    // Joint refinement (backfitting) — see `common::refine_shapes`.
    let apply_quantized = |cv: &mut [f32], r: &Rect| {
        let q = quantize_rect(r, w, h, codec, &alpha_levels);
        let (x0, y0, x1, y1) = rect_bounds(q.cx, q.cy, q.w, q.h);
        apply_rect(cv, h as i32, w as i32, x0, y0, x1, y1, q.alpha, &q.color);
    };
    let do_search = |canvas_wo: &[f32]| -> Option<Rect> {
        let integral_wo = Integral2D::build(target, canvas_wo, h, w);
        let residual_wo = Residual::build(target, canvas_wo, h, w);
        search_rect(&integral_wo, &residual_wo, &mut rng, h, w, search, pal_ref, &alpha_levels, &params)
            .map(|r| quantize_rect(&r, w, h, codec, &alpha_levels))
    };
    refine_shapes(target, bg, h, w, &mut rects, search.refine_passes, apply_quantized, do_search);
    (bg, rects)
}

/// Canvas-derived constants for the primitive-style rect search.
struct RectSearchParams {
    sigma_pos: f64,
    sigma_wh: f64,
    init_max: i64,
    dim_min: i32,
    dim_max: i32,
    hard_cap: u32,
}

/// Stages 1–3 of the primitive fit for a single rect against `integral`/
/// `residual`. Returns the swept-α rect or None when nothing beats the
/// improvement threshold. Extracted verbatim from the greedy loop — the RNG
/// draw order must not change, or default outputs stop being byte-identical.
#[allow(clippy::too_many_arguments)]
fn search_rect(
    integral: &Integral2D,
    residual: &Residual,
    rng: &mut Rng,
    h: u32,
    w: u32,
    search: &SearchOptions,
    pal_ref: Option<&super::palette::PaletteIndex>,
    alpha_levels: &[f32],
    params: &RectSearchParams,
) -> Option<Rect> {
    let RectSearchParams { sigma_pos, sigma_wh, init_max, dim_min, dim_max, hard_cap } = *params;
    let mut best_delta_climb: f32 = -1e-3;
    let mut best_geom: Option<(i32, i32, i32, i32)> = None;

    for _attempt in 0..search.n_attempts {
        // Stage 1 — residual-anchored init.
        let mut best_d = f32::INFINITY;
        let mut best_init: Option<(i32, i32, i32, i32)> = None;
        for _ in 0..search.n_random {
            let (cx, cy) = residual.sample(rng);
            let rw = rng.range(1, init_max + 1) as i32;
            let rh = rng.range(1, init_max + 1) as i32;
            let (x0, y0, x1, y1) = rect_bounds(cx, cy, rw, rh);
            let res = eval_rect_integral(integral, x0, y0, x1, y1, FIXED_HILL_CLIMB_ALPHA, pal_ref);
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
            let res = eval_rect_integral(integral, x0, y0, x1, y1, FIXED_HILL_CLIMB_ALPHA, pal_ref);
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
    best_geom.map(|(cx, cy, rw, rh)| {
        let (x0, y0, x1, y1) = rect_bounds(cx, cy, rw, rh);
        let sums = collect_rect_sums_integral(integral, x0, y0, x1, y1);
        let (chosen_alpha, chosen) = alpha_sweep(&sums, alpha_levels, pal_ref);
        Rect { cx, cy, w: rw, h: rh, alpha: chosen_alpha, color: chosen.color, pidx: chosen.pidx }
    })
}

/// Round-trip a rect through the wire bit layout so refinement judges the
/// shape the decoder will actually render, not its continuous-domain ideal.
fn quantize_rect(r: &Rect, w: u32, h: u32, codec: &Codec, alpha_levels: &[f32]) -> Rect {
    let mut bw = BitWriter::new();
    let (x_q, y_q) = quant_xy(r.cx as f32, r.cy as f32, w, h, codec.cx_bits, codec.cy_bits);
    bw.write(x_q, codec.cx_bits);
    bw.write(y_q, codec.cy_bits);
    bw.write(dim_to_q(r.w as f32, w, codec.r_bits), codec.r_bits);
    bw.write(dim_to_q(r.h as f32, h, codec.r_bits), codec.r_bits);
    write_color(&mut bw, &r.color, r.pidx, codec);
    bw.write(alpha_to_q(r.alpha, alpha_levels), codec.alpha_bits);
    let bytes = bw.finish();
    let mut br = BitReader::new(&bytes);
    let x_q = br.read(codec.cx_bits);
    let y_q = br.read(codec.cy_bits);
    let w_q = br.read(codec.r_bits);
    let h_q = br.read(codec.r_bits);
    let color = read_color(&mut br, codec);
    let a_q = br.read(codec.alpha_bits);
    let (cx, cy) = dequant_xy(x_q, y_q, w, h, codec.cx_bits, codec.cy_bits);
    Rect {
        cx,
        cy,
        w: q_to_dim(w_q, w, codec.r_bits).round() as i32,
        h: q_to_dim(h_q, h, codec.r_bits).round() as i32,
        alpha: q_to_alpha(a_q, alpha_levels),
        color,
        pidx: r.pidx,
    }
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

/// Decode + render rects into `canvas`. `corner_radius` is in canvas pixel
/// units; `0` keeps the existing hard-edge fast path byte-identical to
/// pre-0.3.0 output. Positive values switch to AA rounded-rect rendering.
pub fn decode_render(
    br: &mut BitReader,
    codec: &Codec,
    w: u32,
    h: u32,
    canvas: &mut [f32],
    corner_radius: f32,
) {
    let alpha_levels = codec.alpha_levels_owned();
    let use_aa = corner_radius > 0.0;
    for _ in 0..codec.n_shapes {
        let x_q = br.read(codec.cx_bits);
        let y_q = br.read(codec.cy_bits);
        let w_q = br.read(codec.r_bits);
        let h_q = br.read(codec.r_bits);
        let color = read_color(br, codec);
        let a_q = br.read(codec.alpha_bits);
        let (cx, cy) = dequant_xy(x_q, y_q, w, h, codec.cx_bits, codec.cy_bits);
        let rw = q_to_dim(w_q, w, codec.r_bits).round() as i32;
        let rh = q_to_dim(h_q, h, codec.r_bits).round() as i32;
        let alpha = q_to_alpha(a_q, &alpha_levels);
        if use_aa {
            apply_rounded_rect_aa(
                canvas, h as i32, w as i32,
                cx as f32, cy as f32, rw as f32, rh as f32,
                corner_radius, alpha, &color,
            );
        } else {
            let (x0, y0, x1, y1) = rect_bounds(cx, cy, rw, rh);
            apply_rect(canvas, h as i32, w as i32, x0, y0, x1, y1, alpha, &color);
        }
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
    let pidx = super::palette::nearest_in_codec(codec, bg).unwrap_or(0);
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
    let alpha_levels = codec.alpha_levels_owned();
    let x_q = br.read(codec.cx_bits);
    let y_q = br.read(codec.cy_bits);
    let w_q = br.read(codec.r_bits);
    let h_q = br.read(codec.r_bits);
    let color = read_color(br, codec);
    let alpha = q_to_alpha(br.read(codec.alpha_bits), &alpha_levels);
    let (cx, cy) = dequant_xy(x_q, y_q, w, h, codec.cx_bits, codec.cy_bits);
    let rw = q_to_dim(w_q, w, codec.r_bits).round() as i32;
    let rh = q_to_dim(h_q, h, codec.r_bits).round() as i32;
    (cx, cy, rw, rh, color, alpha)
}

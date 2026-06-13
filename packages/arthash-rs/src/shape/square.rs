//! SQUARE mode — axis-aligned squares. SPEC §5.4.
//!
//! Same bit layout as CIRCLE (cx, cy, s, color, alpha) — the only difference
//! is the rasterized shape. Reuses the [`super::integral2d`] 2D-integral O(1)
//! eval path, with the search restricted to a single side parameter `s`
//! instead of independent w and h.

use super::integral2d::{collect_rect_sums_integral, eval_rect_integral, Integral2D};
use super::options::SearchOptions;
use super::quant::{
    alpha_to_q, aspect_code, dequant_xy, q_to_alpha, q_to_r, quant_xy, r_to_q, read_color,
    write_color,
};
use super::common::{alpha_sweep, filled_canvas, mean_rgb, refine_shapes, FIXED_HILL_CLIMB_ALPHA};
use super::raster::{apply_rect, apply_rounded_rect_aa};
use super::residual::Residual;
use super::rng::Rng;
use crate::bitio::{BitReader, BitWriter};
use crate::codec::CodecConfig as Codec;

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

pub fn fit_squares(
    target: &[f32],
    h: u32,
    w: u32,
    codec: &Codec,
    seed: u64,
    search: &SearchOptions,
) -> ([f32; 3], Vec<Square>) {
    let bg = mean_rgb(target);
    let mut canvas = filled_canvas(bg, h, w);
    let mut integral = Integral2D::build(target, &canvas, h, w);
    let mut residual = Residual::build(target, &canvas, h, w);
    let mut rng = Rng::new(seed);
    let alpha_levels = codec.alpha_levels_owned();
    let palette = super::palette::from_codec(codec);
    let pal_ref = palette.as_ref();

    let long_edge = w.max(h);
    let params = SquareSearchParams {
        sigma_pos: (2u32).max(long_edge * 6 / 100) as f64,
        sigma_s: (1u32).max(long_edge * 6 / 100) as f64,
        s_init_max: (2u32).max(long_edge * 25 / 100) as i64,
        s_max_global: w.min(h).max(1) as i32,
        hard_cap: if search.hill_climb_max_age.is_some() { 10_000 } else { search.hill_climb_steps },
    };
    let null_color = bg;
    let null_alpha = alpha_levels[0];

    let mut squares: Vec<Square> = Vec::with_capacity(codec.n_shapes as usize);

    for _ in 0..codec.n_shapes {
        let sq = search_square(&integral, &residual, &mut rng, h, w, search, pal_ref, &alpha_levels, &params)
            .unwrap_or(Square {
                cx: w as i32 / 2,
                cy: h as i32 / 2,
                s: (w.min(h) / 4).max(2) as i32,
                alpha: null_alpha,
                color: null_color,
                pidx: 0,
            });
        let (x0, y0, x1, y1) = square_bounds(sq.cx, sq.cy, sq.s);
        apply_rect(&mut canvas, h as i32, w as i32, x0, y0, x1, y1, sq.alpha, &sq.color);
        integral.update_canvas_from_row(target, &canvas, y0);
        residual.rebuild_from(target, &canvas, y0.max(0) as usize * w as usize);
        squares.push(sq);
    }

    // Joint refinement (backfitting) — see `common::refine_shapes`.
    let apply_quantized = |cv: &mut [f32], sq: &Square| {
        let q = quantize_square(sq, w, h, codec, &alpha_levels);
        let (x0, y0, x1, y1) = square_bounds(q.cx, q.cy, q.s);
        apply_rect(cv, h as i32, w as i32, x0, y0, x1, y1, q.alpha, &q.color);
    };
    let do_search = |canvas_wo: &[f32]| -> Option<Square> {
        let integral_wo = Integral2D::build(target, canvas_wo, h, w);
        let residual_wo = Residual::build(target, canvas_wo, h, w);
        search_square(&integral_wo, &residual_wo, &mut rng, h, w, search, pal_ref, &alpha_levels, &params)
            .map(|sq| quantize_square(&sq, w, h, codec, &alpha_levels))
    };
    refine_shapes(target, bg, h, w, &mut squares, search.refine_passes, apply_quantized, do_search);
    (bg, squares)
}

/// Canvas-derived constants for the primitive-style square search.
struct SquareSearchParams {
    sigma_pos: f64,
    sigma_s: f64,
    s_init_max: i64,
    s_max_global: i32,
    hard_cap: u32,
}

/// Stages 1–3 of the primitive fit for a single square against `integral`/
/// `residual`. Returns the swept-α square or None. Extracted verbatim from the
/// greedy loop — the RNG draw order must not change.
#[allow(clippy::too_many_arguments)]
fn search_square(
    integral: &Integral2D,
    residual: &Residual,
    rng: &mut Rng,
    h: u32,
    w: u32,
    search: &SearchOptions,
    pal_ref: Option<&super::palette::PaletteIndex>,
    alpha_levels: &[f32],
    params: &SquareSearchParams,
) -> Option<Square> {
    let SquareSearchParams { sigma_pos, sigma_s, s_init_max, s_max_global, hard_cap } = *params;
    let mut best_delta_climb: f32 = -1e-3;
    let mut best_geom: Option<(i32, i32, i32)> = None;

    for _attempt in 0..search.n_attempts {
        // Stage 1.
        let mut best_d = f32::INFINITY;
        let mut best_init: Option<(i32, i32, i32)> = None;
        for _ in 0..search.n_random {
            let (cx, cy) = residual.sample(rng);
            let s = rng.range(1, s_init_max + 1) as i32;
            let (x0, y0, x1, y1) = square_bounds(cx, cy, s);
            let res = eval_rect_integral(integral, x0, y0, x1, y1, FIXED_HILL_CLIMB_ALPHA, pal_ref);
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
            let res = eval_rect_integral(integral, x0, y0, x1, y1, FIXED_HILL_CLIMB_ALPHA, pal_ref);
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
    best_geom.map(|(cx, cy, s)| {
        let (x0, y0, x1, y1) = square_bounds(cx, cy, s);
        let sums = collect_rect_sums_integral(integral, x0, y0, x1, y1);
        let (chosen_alpha, chosen) = alpha_sweep(&sums, alpha_levels, pal_ref);
        Square { cx, cy, s, alpha: chosen_alpha, color: chosen.color, pidx: chosen.pidx }
    })
}

/// Round-trip a square through the wire bit layout so refinement judges the
/// shape the decoder will actually render, not its continuous-domain ideal.
fn quantize_square(sq: &Square, w: u32, h: u32, codec: &Codec, alpha_levels: &[f32]) -> Square {
    let mut bw = BitWriter::new();
    let (x_q, y_q) = quant_xy(sq.cx as f32, sq.cy as f32, w, h, codec.cx_bits, codec.cy_bits);
    bw.write(x_q, codec.cx_bits);
    bw.write(y_q, codec.cy_bits);
    bw.write(r_to_q(sq.s as f32, w, h, codec.r_bits), codec.r_bits);
    write_color(&mut bw, &sq.color, sq.pidx, codec);
    bw.write(alpha_to_q(sq.alpha, alpha_levels), codec.alpha_bits);
    let bytes = bw.finish();
    let mut br = BitReader::new(&bytes);
    let x_q = br.read(codec.cx_bits);
    let y_q = br.read(codec.cy_bits);
    let s_q = br.read(codec.r_bits);
    let color = read_color(&mut br, codec);
    let a_q = br.read(codec.alpha_bits);
    let (cx, cy) = dequant_xy(x_q, y_q, w, h, codec.cx_bits, codec.cy_bits);
    Square {
        cx,
        cy,
        s: q_to_r(s_q, w, h, codec.r_bits).round() as i32,
        alpha: q_to_alpha(a_q, alpha_levels),
        color,
        pidx: sq.pidx,
    }
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

/// Decode + render squares into `canvas`. `corner_radius` is in canvas pixel
/// units; `0` keeps the existing hard-edge fast path byte-identical to
/// pre-0.3.0 output.
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
        let s_q = br.read(codec.r_bits);
        let color = read_color(br, codec);
        let a_q = br.read(codec.alpha_bits);
        let (cx, cy) = dequant_xy(x_q, y_q, w, h, codec.cx_bits, codec.cy_bits);
        let s = q_to_r(s_q, w, h, codec.r_bits).round() as i32;
        let alpha = q_to_alpha(a_q, &alpha_levels);
        if use_aa {
            apply_rounded_rect_aa(
                canvas, h as i32, w as i32,
                cx as f32, cy as f32, s as f32, s as f32,
                corner_radius, alpha, &color,
            );
        } else {
            let (x0, y0, x1, y1) = square_bounds(cx, cy, s);
            apply_rect(canvas, h as i32, w as i32, x0, y0, x1, y1, alpha, &color);
        }
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
    let pidx = super::palette::nearest_in_codec(codec, bg).unwrap_or(0);
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
    let alpha_levels = codec.alpha_levels_owned();
    let x_q = br.read(codec.cx_bits);
    let y_q = br.read(codec.cy_bits);
    let s_q = br.read(codec.r_bits);
    let color = read_color(br, codec);
    let alpha = q_to_alpha(br.read(codec.alpha_bits), &alpha_levels);
    let (cx, cy) = dequant_xy(x_q, y_q, w, h, codec.cx_bits, codec.cy_bits);
    let s = q_to_r(s_q, w, h, codec.r_bits).round() as i32;
    (cx, cy, s, color, alpha)
}

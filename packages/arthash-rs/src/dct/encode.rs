//! V4 DCT encoder. SPEC §5.1.

use super::colorspace::{
    linear_rgb_to_oklab_channels, rgb_to_oklab_channels, rgb_u8_to_linear_planes,
};

/// AC compander powers (encoder applies `sign(c)·|c|^p`, decoder inverts).
const COMPANDER_POWER_L: f32 = 0.6;
const COMPANDER_POWER_PQ: f32 = 0.5;
const COMPANDER_POWER_A: f32 = 0.6;

/// DC compander power for chroma (a, b). L_DC stays uniform.
const DC_COMPANDER_POWER_PQ: f32 = 0.4;

/// Grid-search candidates for the load factor `alpha = declared_scale / max(|ac|)`.
fn alpha_grid() -> [f32; 19] {
    let mut g = [0.0f32; 19];
    let n = 18.0_f32;
    for (i, slot) in g.iter_mut().enumerate() {
        *slot = 0.55 + (1.00 - 0.55) * (i as f32) / n;
    }
    g
}

/// DCT-II cosine basis matrix of shape `(k, n)`:
///     basis[c, x] = cos((π/n) · c · (x + 0.5)).
fn cosine_basis(n: usize, k: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; k * n];
    let pi_over_n = std::f32::consts::PI / (n as f32);
    for c in 0..k {
        for x in 0..n {
            out[c * n + x] = (pi_over_n * (c as f32) * ((x as f32) + 0.5)).cos();
        }
    }
    out
}

/// Triangular mask: indices (cy, cx) such that `cx*ny < nx*(ny-cy)`.
/// Returned in cy-outer, cx-inner row-major order.
fn triangular_indices(nx: usize, ny: usize) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    for cy in 0..ny {
        for cx in 0..nx {
            if cx * ny < nx * (ny - cy) {
                out.push((cy, cx));
            }
        }
    }
    out
}

/// Project a single channel onto DCT-II and pick the triangular-mask values.
/// Returns (dc, ac vec).
///
/// Implementation: two SIMD GEMMs via `matrixmultiply::sgemm`.
///   tmp(ny × w) = cy_basis(ny × h)  · channel(h × w)
///   f  (ny × nx) = tmp     (ny × w)  · cx_basisᵀ(w × nx)
fn dct_channel_raw(channel: &[f32], h: usize, w: usize, nx: usize, ny: usize) -> (f32, Vec<f32>) {
    let cx_basis = cosine_basis(w, nx); // (nx, w) row-major
    let cy_basis = cosine_basis(h, ny); // (ny, h) row-major

    let mut tmp = vec![0.0f32; ny * w];
    unsafe {
        // C = α·A·B + β·C, all row-major (rsa = lda, csa = 1 for row-major).
        matrixmultiply::sgemm(
            ny, h, w,
            1.0,
            cy_basis.as_ptr(), h as isize, 1,
            channel.as_ptr(),  w as isize, 1,
            0.0,
            tmp.as_mut_ptr(),  w as isize, 1,
        );
    }

    let inv = 1.0 / ((w as f32) * (h as f32));
    let mut f = vec![0.0f32; ny * nx];
    unsafe {
        // f = tmp(ny×w) · cx_basisᵀ(w×nx). cx_basis is (nx × w) row-major,
        // so treating it as (w × nx) requires swapping the strides:
        //   row-stride = 1, col-stride = w.
        matrixmultiply::sgemm(
            ny, w, nx,
            inv,
            tmp.as_ptr(),       w as isize, 1,
            cx_basis.as_ptr(),  1, w as isize,
            0.0,
            f.as_mut_ptr(),     nx as isize, 1,
        );
    }

    let mask = triangular_indices(nx, ny);
    let dc = f[mask[0].0 * nx + mask[0].1];
    let mut ac = Vec::with_capacity(mask.len() - 1);
    for &(cy_i, cx_i) in mask.iter().skip(1) {
        ac.push(f[cy_i * nx + cx_i]);
    }
    (dc, ac)
}

#[inline]
fn compand(x: f32, p: f32) -> f32 {
    x.signum() * x.abs().powf(p)
}

#[inline]
fn expand(y: f32, p: f32) -> f32 {
    y.signum() * y.abs().powf(1.0 / p)
}

/// Nearest-neighbor nibble assignment, optionally companded.
/// `power = 1.0` skips the compander.
fn nearest_nibbles(raw_ac: &[f32], declared_scale: f32, power: f32) -> Vec<u8> {
    if declared_scale <= 0.0 || raw_ac.is_empty() {
        return vec![8u8; raw_ac.len()];
    }
    let scale_c = declared_scale.powf(power);
    let mut levels = [0.0f32; 16];
    for (i, slot) in levels.iter_mut().enumerate() {
        *slot = ((i as f32) / 7.5 - 1.0) * scale_c;
    }
    raw_ac
        .iter()
        .map(|&c| {
            let cc = if power != 1.0 { compand(c, power) } else { c };
            let mut best = 0u8;
            let mut best_d = f32::INFINITY;
            for (i, &lv) in levels.iter().enumerate() {
                let d = (cc - lv).abs();
                if d < best_d {
                    best_d = d;
                    best = i as u8;
                }
            }
            best
        })
        .collect()
}

/// Grid-search the declared (quantized) AC scale that minimizes SSE under
/// the companded 4-bit nibble quantizer. `q=0` (drop all AC) is a candidate.
fn search_optimal_scale(raw_ac: &[f32], scale_bits: u32, power: f32) -> f32 {
    if raw_ac.is_empty() {
        return 0.0;
    }
    let max_abs = raw_ac.iter().fold(0.0f32, |m, &v| m.max(v.abs()));
    if max_abs <= 0.0 {
        return 0.0;
    }
    let steps = (1u32 << scale_bits) - 1;

    let sse_drop: f32 = raw_ac.iter().map(|&v| v * v).sum();
    let mut best_q: u32 = 0;
    let mut best_sse = sse_drop;

    let q_anchor = ((steps as f32) * max_abs).round() as i64;
    let mut candidates: std::collections::BTreeSet<i64> = alpha_grid()
        .iter()
        .map(|&a| ((steps as f32) * a * max_abs).round() as i64)
        .collect();
    for q in (q_anchor - 1).max(0)..(q_anchor + 3) {
        candidates.insert(q);
    }

    for q in candidates {
        if q <= 0 || q > steps as i64 {
            // Python's set may contain values > steps; they are valid candidates,
            // but for byte-exact match we replicate "any positive q tried".
            // Re-check: Python doesn't cap q, just iterates the set. We mirror.
            if q <= 0 {
                continue;
            }
        }
        let declared_scale = q as f32 / steps as f32;
        let nibbles = nearest_nibbles(raw_ac, declared_scale, power);
        let scale_c = declared_scale.powf(power);
        let sse: f32 = raw_ac
            .iter()
            .zip(nibbles.iter())
            .map(|(&c, &n)| {
                let c_hat_c = ((n as f32) / 7.5 - 1.0) * scale_c;
                let c_hat = if power != 1.0 { expand(c_hat_c, power) } else { c_hat_c };
                let d = c - c_hat;
                d * d
            })
            .sum();
        if sse < best_sse {
            best_sse = sse;
            best_q = q as u32;
        }
    }
    best_q as f32 / steps as f32
}

/// Encode an RGBA buffer to DCT-mode bytes.
///
/// Input: row-major `(h, w, 4)` u8 RGBA at the target_size long-edge (≤ 100).
pub fn encode_dct(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
    let w = w as usize;
    let h = h as usize;
    assert_eq!(rgba.len(), w * h * 4, "RGBA buffer length mismatch");

    // Step 1: separate channels + premultiply per Python's V4 pipeline.
    //   alpha = rgba[3]/255; a_over_255 = alpha/255
    //   avg = (a_over_255 * channel).sum() / alpha.sum() if any opacity
    let n = w * h;
    let mut r = vec![0.0f32; n];
    let mut g = vec![0.0f32; n];
    let mut b = vec![0.0f32; n];
    let mut alpha = vec![0.0f32; n];
    let mut a_over_255 = vec![0.0f32; n];
    for i in 0..n {
        r[i] = rgba[i * 4] as f32;
        g[i] = rgba[i * 4 + 1] as f32;
        b[i] = rgba[i * 4 + 2] as f32;
        alpha[i] = (rgba[i * 4 + 3] as f32) / 255.0;
        a_over_255[i] = alpha[i] / 255.0;
    }
    let mut avg_r = 0.0f32;
    let mut avg_g = 0.0f32;
    let mut avg_b = 0.0f32;
    let mut avg_a = 0.0f32;
    for i in 0..n {
        avg_r += a_over_255[i] * r[i];
        avg_g += a_over_255[i] * g[i];
        avg_b += a_over_255[i] * b[i];
        avg_a += alpha[i];
    }
    if avg_a > 0.0 {
        avg_r /= avg_a;
        avg_g /= avg_a;
        avg_b /= avg_a;
    }
    let has_alpha = avg_a < (w * h) as f32;

    // Aspect (SPEC §4.2)
    let raw_aspect = (w as f32) / (h as f32);
    let aspect_code: i32 = ((raw_aspect.log2() + 3.0) / 6.0 * 254.0)
        .round()
        .clamp(0.0, 254.0) as i32;
    let quant_aspect: f32 = 2.0f32.powf((aspect_code as f32) / 254.0 * 6.0 - 3.0);

    // Derive (lx, ly)
    let l_limit: f32 = if has_alpha { 5.0 } else { 7.0 };
    let (lx, ly) = if quant_aspect >= 1.0 {
        let lx = l_limit as usize;
        let ly = (l_limit / quant_aspect).round().max(1.0) as usize;
        (lx, ly)
    } else {
        let lx = (l_limit * quant_aspect).round().max(1.0) as usize;
        let ly = l_limit as usize;
        (lx, ly)
    };

    // Premultiplied channels for color projection: avg * (1-α) + a_over_255 * c
    let mut rr = vec![0.0f32; n];
    let mut gg = vec![0.0f32; n];
    let mut bb = vec![0.0f32; n];
    for i in 0..n {
        let one_minus_a = 1.0 - alpha[i];
        rr[i] = avg_r * one_minus_a + a_over_255[i] * r[i];
        gg[i] = avg_g * one_minus_a + a_over_255[i] * g[i];
        bb[i] = avg_b * one_minus_a + a_over_255[i] * b[i];
    }

    let (l_ch, p_ch, q_ch) = rgb_to_oklab_channels(&rr, &gg, &bb);

    let (l_dc, l_ac) = dct_channel_raw(&l_ch, h, w, lx.max(3), ly.max(3));
    let (p_dc, p_ac) = dct_channel_raw(&p_ch, h, w, 3, 3);
    let (q_dc, q_ac) = dct_channel_raw(&q_ch, h, w, 3, 3);
    let (a_dc, a_ac) = if has_alpha {
        dct_channel_raw(&alpha, h, w, 5, 5)
    } else {
        (1.0, Vec::new())
    };

    // Per-channel scale search.
    let l_scale = search_optimal_scale(&l_ac, 5, COMPANDER_POWER_L);
    let p_scale = search_optimal_scale(&p_ac, 4, COMPANDER_POWER_PQ);
    let q_scale = search_optimal_scale(&q_ac, 4, COMPANDER_POWER_PQ);
    let a_scale = if has_alpha {
        search_optimal_scale(&a_ac, 4, COMPANDER_POWER_A)
    } else {
        1.0
    };

    // DC quantization
    let p_dc_companded = compand(p_dc, DC_COMPANDER_POWER_PQ);
    let q_dc_companded = compand(q_dc, DC_COMPANDER_POWER_PQ);

    let l_dc_q = (63.0 * l_dc).round().clamp(0.0, 63.0) as u32;
    let p_dc_q = (31.5 + 31.5 * p_dc_companded).round().clamp(0.0, 63.0) as u32;
    let q_dc_q = (31.5 + 31.5 * q_dc_companded).round().clamp(0.0, 63.0) as u32;
    let l_scale_q = (31.0 * l_scale).round().clamp(0.0, 31.0) as u32;
    let has_alpha_bit: u32 = if has_alpha { 1 } else { 0 };
    let header24: u32 = l_dc_q
        | (p_dc_q << 6)
        | (q_dc_q << 12)
        | (l_scale_q << 18)
        | (has_alpha_bit << 23);

    let p_scale_q = (15.0 * p_scale).round().clamp(0.0, 15.0) as u32;
    let q_scale_q = (15.0 * q_scale).round().clamp(0.0, 15.0) as u32;
    let header16: u32 = (aspect_code as u32) | (p_scale_q << 8) | (q_scale_q << 12);

    let mut out: Vec<u8> = vec![
        (header24 & 0xff) as u8,
        ((header24 >> 8) & 0xff) as u8,
        ((header24 >> 16) & 0xff) as u8,
        (header16 & 0xff) as u8,
        ((header16 >> 8) & 0xff) as u8,
    ];

    let mut is_odd = false;
    let push_nibble = |out: &mut Vec<u8>, u: u8, is_odd: &mut bool| {
        if *is_odd {
            *out.last_mut().unwrap() |= u << 4;
        } else {
            out.push(u & 0x0f);
        }
        *is_odd = !*is_odd;
    };

    if has_alpha {
        let a_dc_q = (15.0 * a_dc).round().clamp(0.0, 15.0) as u32;
        let a_scale_q = (15.0 * a_scale).round().clamp(0.0, 15.0) as u32;
        out.push((a_dc_q | (a_scale_q << 4)) as u8);
    }

    for (raw, scale, power) in [
        (&l_ac, l_scale, COMPANDER_POWER_L),
        (&p_ac, p_scale, COMPANDER_POWER_PQ),
        (&q_ac, q_scale, COMPANDER_POWER_PQ),
    ] {
        for &n in nearest_nibbles(raw, scale, power).iter() {
            push_nibble(&mut out, n, &mut is_odd);
        }
    }

    if has_alpha {
        for &n in nearest_nibbles(&a_ac, a_scale, COMPANDER_POWER_A).iter() {
            push_nibble(&mut out, n, &mut is_odd);
        }
    }

    out
}

/// Specialised fast path for fully-opaque RGB input (the most common shape
/// of call — `encode_rgb` packs α=255 unconditionally). Skips:
///   * alpha extraction / a_over_255 / avg accumulation
///   * the premultiplication pass (rr/gg/bb collapse to channel/255)
///   * the alpha DCT projection + nibble pass
///   * the per-pixel `powf(2.4)` in srgb→linear (uses a 256-LUT keyed by u8)
///
/// Produces byte-identical output to `encode_dct` for opaque input.
pub fn encode_dct_rgb_opaque(w: u32, h: u32, rgb: &[u8]) -> Vec<u8> {
    let w = w as usize;
    let h = h as usize;
    let n = w * h;
    assert_eq!(rgb.len(), n * 3, "RGB buffer length mismatch");

    // sRGB u8 → linear-RGB f32 via 256-LUT (3 lookups per pixel, no powf).
    let (rl, gl, bl) = rgb_u8_to_linear_planes(rgb, n);
    let (l_ch, p_ch, q_ch) = linear_rgb_to_oklab_channels(&rl, &gl, &bl);

    // Aspect (SPEC §4.2)
    let raw_aspect = (w as f32) / (h as f32);
    let aspect_code: i32 = ((raw_aspect.log2() + 3.0) / 6.0 * 254.0)
        .round()
        .clamp(0.0, 254.0) as i32;
    let quant_aspect: f32 = 2.0f32.powf((aspect_code as f32) / 254.0 * 6.0 - 3.0);

    // Derive (lx, ly) — has_alpha is always false here, so l_limit = 7.
    let l_limit: f32 = 7.0;
    let (lx, ly) = if quant_aspect >= 1.0 {
        (l_limit as usize, (l_limit / quant_aspect).round().max(1.0) as usize)
    } else {
        ((l_limit * quant_aspect).round().max(1.0) as usize, l_limit as usize)
    };

    let (l_dc, l_ac) = dct_channel_raw(&l_ch, h, w, lx.max(3), ly.max(3));
    let (p_dc, p_ac) = dct_channel_raw(&p_ch, h, w, 3, 3);
    let (q_dc, q_ac) = dct_channel_raw(&q_ch, h, w, 3, 3);

    let l_scale = search_optimal_scale(&l_ac, 5, COMPANDER_POWER_L);
    let p_scale = search_optimal_scale(&p_ac, 4, COMPANDER_POWER_PQ);
    let q_scale = search_optimal_scale(&q_ac, 4, COMPANDER_POWER_PQ);

    let p_dc_companded = compand(p_dc, DC_COMPANDER_POWER_PQ);
    let q_dc_companded = compand(q_dc, DC_COMPANDER_POWER_PQ);

    let l_dc_q = (63.0 * l_dc).round().clamp(0.0, 63.0) as u32;
    let p_dc_q = (31.5 + 31.5 * p_dc_companded).round().clamp(0.0, 63.0) as u32;
    let q_dc_q = (31.5 + 31.5 * q_dc_companded).round().clamp(0.0, 63.0) as u32;
    let l_scale_q = (31.0 * l_scale).round().clamp(0.0, 31.0) as u32;
    let header24: u32 =
        l_dc_q | (p_dc_q << 6) | (q_dc_q << 12) | (l_scale_q << 18); // has_alpha bit = 0

    let p_scale_q = (15.0 * p_scale).round().clamp(0.0, 15.0) as u32;
    let q_scale_q = (15.0 * q_scale).round().clamp(0.0, 15.0) as u32;
    let header16: u32 = (aspect_code as u32) | (p_scale_q << 8) | (q_scale_q << 12);

    let mut out: Vec<u8> = Vec::with_capacity(5 + (l_ac.len() + 16).div_ceil(2));
    out.push((header24 & 0xff) as u8);
    out.push(((header24 >> 8) & 0xff) as u8);
    out.push(((header24 >> 16) & 0xff) as u8);
    out.push((header16 & 0xff) as u8);
    out.push(((header16 >> 8) & 0xff) as u8);

    let mut is_odd = false;
    let mut push_nibble = |out: &mut Vec<u8>, u: u8| {
        if is_odd {
            *out.last_mut().unwrap() |= u << 4;
        } else {
            out.push(u & 0x0f);
        }
        is_odd = !is_odd;
    };

    for (raw, scale, power) in [
        (&l_ac, l_scale, COMPANDER_POWER_L),
        (&p_ac, p_scale, COMPANDER_POWER_PQ),
        (&q_ac, q_scale, COMPANDER_POWER_PQ),
    ] {
        for &n in nearest_nibbles(raw, scale, power).iter() {
            push_nibble(&mut out, n);
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn triangular_3x3() {
        // SPEC §5.1.3: mask is `cx*ny < nx*(ny-cy)`. For (3,3):
        //   cy=0: cx<3 → {0,1,2}; cy=1: cx<2 → {0,1}; cy=2: cx<1 → {0}.
        // (Note: SPEC's table value 9 in this row is a documentation typo;
        // the actual mask is 6 entries, of which AC = 5.)
        let m = triangular_indices(3, 3);
        assert_eq!(m.len(), 6);
    }

    #[test]
    fn cosine_basis_first_row_is_one() {
        let b = cosine_basis(4, 2);
        for x in 0..4 {
            assert!((b[x] - 1.0).abs() < 1e-6);
        }
    }
}

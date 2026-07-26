//! Post-rasterization render primitives — Gaussian blur + `RenderStyle`.
//!
//! These primitives sit *outside* the byte format. They modify how a decoded
//! hash is presented (corner rounding, Gaussian blur) without affecting the
//! bytes themselves. The corresponding SVG path emits the same visual effects
//! via SVG primitives (`rx`/`ry`, `<feGaussianBlur>`), so `(hash, codec,
//! style)` produces the same visual across `decode` and `to_svg`.
//!
//! Scope:
//!  * `RenderStyle` — pair of (`blur`, `corner_radius`), both in output-pixel
//!    units. `corner_radius` only honored by rect/square/rotrect.
//!  * `gaussian_blur_rgba8` — sRGB-space two-pass separable Gaussian on a
//!    row-major RGBA u8 buffer. Hand-rolled to keep deps minimal and to
//!    match the browser's default `feGaussianBlur` color space (sRGB unless
//!    `color-interpolation-filters="linearRGB"` is explicitly set; arthash's
//!    emitted SVG uses the browser default).
//!
//! See `shape::raster::apply_rounded_rect_aa` / `apply_rotated_rounded_rect_aa`
//! for the corner-rounding rasterizers — those operate on the linear f32
//! canvas before sRGB conversion.

/// Per-call visual styling, independent of the codec byte format.
///
/// All fields are in **output pixel** units (same as the viewBox / `base_size`
/// long edge). The fast path is `RenderStyle::default()` (both zero): the
/// renderer bypasses the new primitives entirely and matches pre-0.3.0 output
/// byte-for-byte.
#[derive(Clone, Copy, Debug, Default)]
pub struct RenderStyle {
    /// Gaussian blur stdDeviation, in output-pixel units. `0` = sharp.
    pub blur: f32,
    /// Corner-radius for `rect` / `square` / `rotrect`, in output-pixel units.
    /// `0` = sharp corners. Silently ignored for non-rect-family codecs.
    pub corner_radius: f32,
}

impl RenderStyle {
    /// True when no rendering effect is requested — callers should take the
    /// zero-cost fast path (skip rounded-rect AA, skip blur convolution).
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.blur <= 0.0 && self.corner_radius <= 0.0
    }
}

/// 8×8 Bayer matrix for ordered dithering, values 0..63 (standard recursive
/// construction). Tiled over the output; deterministic, so dithered decodes
/// stay reproducible across calls, platforms, and bindings.
const BAYER8: [[u8; 8]; 8] = [
    [0, 32, 8, 40, 2, 34, 10, 42],
    [48, 16, 56, 24, 50, 18, 58, 26],
    [12, 44, 4, 36, 14, 46, 6, 38],
    [60, 28, 52, 20, 62, 30, 54, 22],
    [3, 35, 11, 43, 1, 33, 9, 41],
    [51, 19, 59, 27, 49, 17, 57, 25],
    [15, 47, 7, 39, 13, 45, 5, 37],
    [63, 31, 55, 23, 61, 29, 53, 21],
];

/// Ordered-dither quantization threshold for output pixel `(x, y)`, in
/// `(0, 1)` with mean exactly 0.5 — so `floor(v·255 + t)` is an unbiased
/// replacement for `floor(v·255 + 0.5)` (plain rounding) that trades ±½ LSB
/// of spatial noise for the banding steps in smooth gradients.
#[inline]
pub(crate) fn bayer_threshold(x: usize, y: usize) -> f32 {
    (BAYER8[y & 7][x & 7] as f32 + 0.5) / 64.0
}

/// Quantize `v` (already scaled to the 0..255 range) to u8: plain rounding,
/// or ordered dithering with the Bayer threshold at `(x, y)`. With
/// `dither = false` this is `floor(v + 0.5)` — byte-identical to the
/// historical rounding for the non-negative values all callers produce.
/// Every f32→u8 quantization that supports dithering goes through here so
/// the threshold convention cannot drift between call sites.
#[inline]
pub(crate) fn quant_u8(v: f32, x: usize, y: usize, dither: bool) -> u8 {
    let t = if dither { bayer_threshold(x, y) } else { 0.5 };
    (v + t).floor().clamp(0.0, 255.0) as u8
}

/// Quantize an RGBA buffer to a fixed sRGB palette (flat `[r,g,b]·K` bytes),
/// nearest-color by squared sRGB distance. With `dither`, each pixel is
/// offset by the Bayer threshold — scaled to the palette's average
/// nearest-neighbor spacing — before the nearest lookup, producing the
/// classic ordered-dither look; without it, hard posterized regions.
///
/// `scale` is the dither dot pitch in output pixels: the Bayer matrix is
/// sampled at `(x/scale, y/scale)`, so one threshold cell covers a
/// `scale×scale` block. `0` = auto — `max(w, h) / 128`, min 1 — because at
/// high output resolutions a 1-px pattern reads as fine noise; a coarser
/// pitch restores the chunky retro halftone look.
///
/// This is a render-time effect (like blur / corner rounding): DCT hashes
/// carry no palette in their bytes, so a palette on a DCT codec is purely
/// consensus display knowledge. Alpha is untouched.
pub(crate) fn palette_dither_rgba8(
    rgba: &mut [u8],
    w: u32,
    h: u32,
    palette: &[u8],
    dither: bool,
    scale: u32,
) {
    let k = palette.len() / 3;
    if k == 0 {
        return;
    }
    let scale = if scale == 0 {
        (w.max(h) / 128).max(1) as usize
    } else {
        scale as usize
    };
    // Dither amplitude: mean distance from each entry to its nearest other
    // entry. This approximates the local quantization step of the palette, so
    // the threshold offset spans roughly one "color step" — enough to blend
    // adjacent entries, not enough to jump across the whole gamut.
    let spread = if !dither || k < 2 {
        0.0
    } else {
        let mut sum = 0.0f32;
        for i in 0..k {
            let mut best = f32::MAX;
            for j in 0..k {
                if i == j {
                    continue;
                }
                let dr = palette[i * 3] as f32 - palette[j * 3] as f32;
                let dg = palette[i * 3 + 1] as f32 - palette[j * 3 + 1] as f32;
                let db = palette[i * 3 + 2] as f32 - palette[j * 3 + 2] as f32;
                best = best.min(dr * dr + dg * dg + db * db);
            }
            sum += best.sqrt();
        }
        sum / (k as f32)
    };
    let w = w as usize;
    let pal_f: Vec<[f32; 3]> = palette
        .chunks_exact(3)
        .map(|c| [c[0] as f32, c[1] as f32, c[2] as f32])
        .collect();
    // The threshold offset is constant within a `scale`-row band, so build
    // one row of offsets per band instead of dividing per pixel. Stays all
    // zeros when dithering is off.
    let mut off_row = vec![0.0f32; w];
    let mut prev_band = usize::MAX;
    // 1-entry memo: smooth DCT/blur input means long runs of identical
    // (rgb, offset), which skip the O(K) nearest scan entirely.
    let mut memo: Option<([u8; 3], f32, usize)> = None;
    for y in 0..h as usize {
        if dither {
            let band = y / scale;
            if band != prev_band {
                prev_band = band;
                for (x, off) in off_row.iter_mut().enumerate() {
                    *off = (bayer_threshold(x / scale, band) - 0.5) * spread;
                }
            }
        }
        for (x, &off) in off_row.iter().enumerate() {
            let p = (y * w + x) * 4;
            let rgb = [rgba[p], rgba[p + 1], rgba[p + 2]];
            let best = match memo {
                Some((m_rgb, m_off, m_best)) if m_rgb == rgb && m_off == off => m_best,
                _ => {
                    let r = rgb[0] as f32 + off;
                    let g = rgb[1] as f32 + off;
                    let b = rgb[2] as f32 + off;
                    let mut best = 0usize;
                    let mut best_d = f32::MAX;
                    for (i, c) in pal_f.iter().enumerate() {
                        let dr = r - c[0];
                        let dg = g - c[1];
                        let db = b - c[2];
                        let d = dr * dr + dg * dg + db * db;
                        if d < best_d {
                            best_d = d;
                            best = i;
                        }
                    }
                    memo = Some((rgb, off, best));
                    best
                }
            };
            rgba[p] = palette[best * 3];
            rgba[p + 1] = palette[best * 3 + 1];
            rgba[p + 2] = palette[best * 3 + 2];
        }
    }
}

/// Build a normalized 1D Gaussian kernel of radius `ceil(3σ)`. The kernel
/// sums to 1.0; values outside the radius contribute < 0.012 of the peak.
fn build_gaussian_kernel(sigma: f32) -> (Vec<f32>, i32) {
    let radius = (sigma * 3.0).ceil().max(1.0) as i32;
    let len = (radius * 2 + 1) as usize;
    let mut kernel = vec![0.0f32; len];
    let two_sigma2 = 2.0 * sigma * sigma;
    let mut sum = 0.0f32;
    for i in -radius..=radius {
        let v = (-(i as f32) * (i as f32) / two_sigma2).exp();
        kernel[(i + radius) as usize] = v;
        sum += v;
    }
    for v in kernel.iter_mut() {
        *v /= sum;
    }
    (kernel, radius)
}

/// Two-pass separable Gaussian blur on a row-major RGBA u8 buffer.
///
/// `sigma` is in pixel units; `sigma <= 0` returns immediately (no-op).
/// Edge handling: clamp (replicate edge pixels), matching `feGaussianBlur`'s
/// default `edgeMode="duplicate"` behavior.
///
/// The implementation:
///  * Convolves R/G/B in sRGB space (matches browser default
///    `color-interpolation-filters: sRGB` for `<feGaussianBlur>`).
///  * Leaves alpha untouched — shape modes produce fully opaque output, and
///    blurring alpha would create halo bleed at hash edges (visually wrong
///    for placeholder use).
pub fn gaussian_blur_rgba8(rgba: &mut [u8], w: u32, h: u32, sigma: f32) {
    gaussian_blur_rgba8_dither(rgba, w, h, sigma, false);
}

/// [`gaussian_blur_rgba8`] with optional ordered dithering at the final
/// f32→u8 write-back. Blurring re-creates smooth gradients from quantized
/// input; rounding them back to 8-bit re-introduces banding, which the
/// Bayer threshold breaks up. `dither = false` is byte-identical to
/// [`gaussian_blur_rgba8`].
pub(crate) fn gaussian_blur_rgba8_dither(rgba: &mut [u8], w: u32, h: u32, sigma: f32, dither: bool) {
    if sigma <= 0.0 {
        return;
    }
    let w = w as i32;
    let h = h as i32;
    if w <= 0 || h <= 0 {
        return;
    }
    let (kernel, radius) = build_gaussian_kernel(sigma);

    // Two intermediate f32 buffers for the separable passes. Alpha is read
    // but not convolved — we copy it through unchanged for output.
    let n = (w * h) as usize;
    let mut src = vec![0.0f32; n * 3];
    for i in 0..n {
        src[i * 3] = rgba[i * 4] as f32;
        src[i * 3 + 1] = rgba[i * 4 + 1] as f32;
        src[i * 3 + 2] = rgba[i * 4 + 2] as f32;
    }
    let mut tmp = vec![0.0f32; n * 3];

    // Horizontal pass: src → tmp.
    for y in 0..h {
        let row_off = (y * w) as usize * 3;
        for x in 0..w {
            let mut acc = [0.0f32; 3];
            for k in -radius..=radius {
                let sx = (x + k).clamp(0, w - 1);
                let p = row_off + (sx as usize) * 3;
                let kw = kernel[(k + radius) as usize];
                acc[0] += src[p] * kw;
                acc[1] += src[p + 1] * kw;
                acc[2] += src[p + 2] * kw;
            }
            let q = row_off + (x as usize) * 3;
            tmp[q] = acc[0];
            tmp[q + 1] = acc[1];
            tmp[q + 2] = acc[2];
        }
    }

    // Vertical pass: tmp → src (reuse).
    for y in 0..h {
        for x in 0..w {
            let mut acc = [0.0f32; 3];
            for k in -radius..=radius {
                let sy = (y + k).clamp(0, h - 1);
                let p = ((sy * w) as usize + x as usize) * 3;
                let kw = kernel[(k + radius) as usize];
                acc[0] += tmp[p] * kw;
                acc[1] += tmp[p + 1] * kw;
                acc[2] += tmp[p + 2] * kw;
            }
            let q = ((y * w) as usize + x as usize) * 3;
            src[q] = acc[0];
            src[q + 1] = acc[1];
            src[q + 2] = acc[2];
        }
    }

    // Write back to RGBA u8. Alpha channel is preserved.
    for y in 0..h as usize {
        for x in 0..w as usize {
            let i = y * w as usize + x;
            rgba[i * 4] = quant_u8(src[i * 3], x, y, dither);
            rgba[i * 4 + 1] = quant_u8(src[i * 3 + 1], x, y, dither);
            rgba[i * 4 + 2] = quant_u8(src[i * 3 + 2], x, y, dither);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blur_zero_sigma_is_noop() {
        let mut rgba = vec![100u8; 16 * 16 * 4];
        rgba[0] = 250;
        let copy = rgba.clone();
        gaussian_blur_rgba8(&mut rgba, 16, 16, 0.0);
        assert_eq!(rgba, copy);
    }

    #[test]
    fn blur_preserves_uniform_color() {
        // Constant input → constant output (every kernel sums to 1.0).
        let mut rgba = vec![0u8; 32 * 32 * 4];
        for i in 0..(32 * 32) {
            rgba[i * 4] = 120;
            rgba[i * 4 + 1] = 80;
            rgba[i * 4 + 2] = 200;
            rgba[i * 4 + 3] = 255;
        }
        gaussian_blur_rgba8(&mut rgba, 32, 32, 4.0);
        for i in 0..(32 * 32) {
            assert_eq!(rgba[i * 4], 120);
            assert_eq!(rgba[i * 4 + 1], 80);
            assert_eq!(rgba[i * 4 + 2], 200);
            assert_eq!(rgba[i * 4 + 3], 255);
        }
    }

    #[test]
    fn blur_preserves_alpha() {
        let mut rgba = vec![100u8; 16 * 16 * 4];
        for i in 0..(16 * 16) {
            rgba[i * 4 + 3] = if i % 2 == 0 { 200 } else { 50 };
        }
        let alpha_before: Vec<u8> = (0..16 * 16).map(|i| rgba[i * 4 + 3]).collect();
        gaussian_blur_rgba8(&mut rgba, 16, 16, 2.0);
        let alpha_after: Vec<u8> = (0..16 * 16).map(|i| rgba[i * 4 + 3]).collect();
        assert_eq!(alpha_before, alpha_after, "alpha must pass through blur unchanged");
    }

    #[test]
    fn blur_impulse_spreads_to_neighbors() {
        // Impulse: single bright pixel at center, everything else 0. After
        // blur the peak must drop (energy spreads), and immediate neighbors
        // must light up.
        let w = 33;
        let h = 33;
        let mut rgba = vec![0u8; (w * h * 4) as usize];
        let cx = (w / 2) as usize;
        let cy = (h / 2) as usize;
        let center = (cy * w as usize + cx) * 4;
        rgba[center] = 255;
        rgba[center + 1] = 255;
        rgba[center + 2] = 255;
        for i in 0..(w * h) as usize {
            rgba[i * 4 + 3] = 255;
        }
        gaussian_blur_rgba8(&mut rgba, w, h, 1.5);
        let peak = rgba[center];
        // Two passes of separable Gaussian = full 2D Gaussian with peak
        // ratio 1/(2π·σ²). At σ=1.5 → 255·0.0707 ≈ 18.
        assert!(peak < 255, "peak should drop below input after blur, got {peak}");
        assert!((15..=25).contains(&peak), "expected 2D Gaussian peak ≈ 18, got {peak}");
        let neighbor = rgba[(cy * w as usize + cx + 1) * 4];
        assert!(neighbor > 0, "immediate neighbor should receive energy");
        assert!(neighbor < peak, "neighbor should be dimmer than peak");
        // Far-away pixel (top-left corner, index 0) should be essentially zero
        // (Gaussian falls off fast).
        let far = rgba[0];
        assert!(far < 5, "far corner should be ~0, got {far}");
    }

    #[test]
    fn render_style_default_is_empty() {
        let s = RenderStyle::default();
        assert!(s.is_empty());
        assert_eq!(s.blur, 0.0);
        assert_eq!(s.corner_radius, 0.0);
    }

    #[test]
    fn render_style_with_blur_not_empty() {
        let s = RenderStyle { blur: 1.0, corner_radius: 0.0 };
        assert!(!s.is_empty());
    }
}

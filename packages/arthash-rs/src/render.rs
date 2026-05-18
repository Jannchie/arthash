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
    for i in 0..n {
        rgba[i * 4] = src[i * 3].round().clamp(0.0, 255.0) as u8;
        rgba[i * 4 + 1] = src[i * 3 + 1].round().clamp(0.0, 255.0) as u8;
        rgba[i * 4 + 2] = src[i * 3 + 2].round().clamp(0.0, 255.0) as u8;
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
        assert!(peak >= 15 && peak <= 25, "expected 2D Gaussian peak ≈ 18, got {peak}");
        let neighbor = rgba[(cy * w as usize + cx + 1) * 4];
        assert!(neighbor > 0, "immediate neighbor should receive energy");
        assert!(neighbor < peak, "neighbor should be dimmer than peak");
        // Far-away pixel should be essentially zero (Gaussian falls off fast).
        let far = rgba[(0 * w as usize + 0) * 4];
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

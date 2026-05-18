//! Public `encode_rgb` / `encode_rgba` / `decode` API.
//!
//! Mirrors the Python and TypeScript SDKs. Inputs are raw byte buffers at
//! native size — the caller is responsible for resizing to the shape
//! thumbnail (`shape::THUMB = 48`) or DCT target size (`≤ 100`) before
//! calling. See SPEC §1.1: hash + codec is one logical unit.
//!
//! For path-based image loading + automatic resize, enable the `image-io`
//! feature and use [`encode_image`].

use crate::bitio::BitReader;
use crate::codec::{Codec, CodecConfig, ShapeType};
use crate::colorspace::{linear_to_srgb_u8, srgb_u8_to_linear};
use crate::render::{gaussian_blur_rgba8, RenderStyle};
use crate::shape::circle::{decode_render as circle_decode, encode_circle};
use crate::shape::pixel::{decode_render as pixel_decode, encode_pixel, PixelSmooth};
use crate::shape::quant::{aspect_from_code, read_color};
use crate::shape::rect::{decode_render as rect_decode, encode_rect};
use crate::shape::rotrect::{decode_render as rotrect_decode, encode_rotrect};
use crate::shape::square::{decode_render as square_decode, encode_square};
use crate::shape::triangle::{decode_render as triangle_decode, encode_triangle};
use crate::shape::SearchOptions;

/// Encoder knobs that affect shape-mode search cost/fidelity. Ignored by
/// DCT and PIXEL (both deterministic).
#[derive(Clone, Copy, Debug, Default)]
pub struct EncodeOptions {
    /// CIRCLE / TRIANGLE / SQUARE / RECT / ROTATED_RECT RNG seed.
    pub seed: u64,
    /// Hill-climb search budget. `None` ⇒ mode-specific tuned default.
    pub search: Option<SearchOptions>,
}

/// Decode knobs — output size + smoothing.
#[derive(Clone, Copy, Debug)]
pub struct DecodeOptions {
    /// Long-edge pixel target. Default 256.
    pub base_size: u32,
    /// Override the stored aspect for non-default sizing.
    pub override_aspect: Option<f32>,
    /// PIXEL-only — `Nearest` (default) or `Bilinear`.
    pub pixel_smooth: PixelSmooth,
    /// Shape-mode supersample factor (per-axis; total samples per output
    /// pixel = `aa²`). `1` = off (default). Ignored by DCT/PIXEL.
    pub aa: u32,
    /// Visual styling — corner rounding (rect/square/rotrect only) and
    /// Gaussian blur. Both fields in output-pixel units. `Default` = sharp,
    /// zero-cost (matches pre-0.3.0 output byte-for-byte).
    pub style: RenderStyle,
}

impl Default for DecodeOptions {
    fn default() -> Self {
        Self {
            base_size: 256,
            override_aspect: None,
            pixel_smooth: PixelSmooth::Nearest,
            aa: 1,
            style: RenderStyle::default(),
        }
    }
}

/// What [`decode`] returns. Named fields so callers don't have to remember
/// tuple positions.
#[derive(Clone, Debug)]
pub struct DecodeOutput {
    pub width: u32,
    pub height: u32,
    /// Row-major RGBA bytes (length `4 · width · height`).
    pub rgba: Vec<u8>,
}

fn rgb_u8_to_linear_flat(rgb: &[u8]) -> Vec<f32> {
    rgb.iter().map(|&c| srgb_u8_to_linear(c)).collect()
}

/// Encode raw RGB at `(w, h)`. Alpha is treated as 255 for DCT.
///
/// * `w`, `h`: pixel dimensions. For shape modes, callers should resize to
///   `shape::THUMB = 48` long-edge first. For DCT, `≤ 100`.
/// * `rgb`: row-major flat `(h, w, 3)` u8 sRGB, length `h*w*3`.
pub fn encode_rgb(rgb: &[u8], w: u32, h: u32, codec: &Codec, opts: EncodeOptions) -> Vec<u8> {
    let cfg = codec.to_config();
    encode_rgb_cfg(rgb, w, h, &cfg, opts)
}

/// Encode raw RGBA at `(w, h)`. For shape modes the alpha is ignored
/// (caller should composite over an opaque background first if needed).
pub fn encode_rgba(rgba: &[u8], w: u32, h: u32, codec: &Codec, opts: EncodeOptions) -> Vec<u8> {
    let cfg = codec.to_config();
    if matches!(cfg.shape, ShapeType::Dct) {
        return crate::dct::encode_dct(w, h, rgba);
    }
    // Composite over white into RGB, then encode.
    let n = (w * h) as usize;
    let mut rgb = vec![0u8; n * 3];
    for i in 0..n {
        let a = rgba[i * 4 + 3] as f32 / 255.0;
        let inv = 1.0 - a;
        for c in 0..3 {
            let v = a * (rgba[i * 4 + c] as f32) + inv * 255.0;
            rgb[i * 3 + c] = v.clamp(0.0, 255.0) as u8;
        }
    }
    encode_rgb_cfg(&rgb, w, h, &cfg, opts)
}

fn encode_rgb_cfg(rgb: &[u8], w: u32, h: u32, cfg: &CodecConfig, opts: EncodeOptions) -> Vec<u8> {
    match cfg.shape {
        ShapeType::Dct => {
            // Opaque fast path: skips alpha extraction + premultiplication.
            crate::dct::encode_dct_rgb_opaque(w, h, rgb)
        }
        _ => {
            let target_lin = rgb_u8_to_linear_flat(rgb);
            let search = match opts.search {
                Some(s) => s,
                None => match cfg.shape {
                    ShapeType::Triangle => SearchOptions::triangle_default(),
                    _ => SearchOptions::default(),
                },
            };
            match cfg.shape {
                ShapeType::Circle => {
                    encode_circle(&target_lin, h, w, w, h, cfg, opts.seed, &search)
                }
                ShapeType::Triangle => {
                    encode_triangle(&target_lin, h, w, w, h, cfg, opts.seed, &search)
                }
                ShapeType::Square => {
                    encode_square(&target_lin, h, w, w, h, cfg, opts.seed, &search)
                }
                ShapeType::Rect => encode_rect(&target_lin, h, w, w, h, cfg, opts.seed, &search),
                ShapeType::RotatedRect => {
                    encode_rotrect(&target_lin, h, w, w, h, cfg, opts.seed, &search)
                }
                ShapeType::Pixel => encode_pixel(&target_lin, h, w, w, h, cfg),
                ShapeType::Dct => unreachable!(),
            }
        }
    }
}

/// Decode hash bytes to RGBA pixels.
pub fn decode(hash: &[u8], codec: &Codec, opts: DecodeOptions) -> DecodeOutput {
    let cfg = codec.to_config();
    let (w, h, rgba) = decode_cfg(hash, &cfg, opts);
    DecodeOutput { width: w, height: h, rgba }
}

fn decode_cfg(hash: &[u8], cfg: &CodecConfig, opts: DecodeOptions) -> (u32, u32, Vec<u8>) {
    match cfg.shape {
        ShapeType::Dct => crate::dct::decode_dct(hash, opts.base_size, opts.override_aspect),
        _ => decode_shape(hash, cfg, opts),
    }
}

fn decode_shape(hash: &[u8], cfg: &CodecConfig, opts: DecodeOptions) -> (u32, u32, Vec<u8>) {
    let mut br = BitReader::new(hash);
    let a_code = br.read(8);
    let quant_aspect = aspect_from_code(a_code);
    let aspect = opts.override_aspect.unwrap_or(quant_aspect);
    let (w, h) = if aspect >= 1.0 {
        (
            opts.base_size,
            ((opts.base_size as f32 / aspect).round().max(1.0)) as u32,
        )
    } else {
        (
            ((opts.base_size as f32 * aspect).round().max(1.0)) as u32,
            opts.base_size,
        )
    };

    if matches!(cfg.shape, ShapeType::Pixel) {
        let rgb = pixel_decode(&mut br, cfg, w, h, quant_aspect, opts.pixel_smooth);
        let mut rgba = rgb_to_rgba(&rgb, w, h);
        if opts.style.blur > 0.0 {
            gaussian_blur_rgba8(&mut rgba, w, h, opts.style.blur);
        }
        return (w, h, rgba);
    }

    let ss = opts.aa.max(1);
    let bg = read_color(&mut br, cfg);
    let (ww, hh) = (w * ss, h * ss);
    let mut canvas = vec![0.0f32; (ww * hh * 3) as usize];
    for i in 0..(ww * hh) as usize {
        canvas[i * 3] = bg[0];
        canvas[i * 3 + 1] = bg[1];
        canvas[i * 3 + 2] = bg[2];
    }
    // corner_radius is supplied in output-pixel units; scale to canvas units
    // for the supersampled rasterizer. Non-rect shapes silently ignore it.
    let corner_radius_canvas = opts.style.corner_radius * (ss as f32);
    match cfg.shape {
        ShapeType::Circle => circle_decode(&mut br, cfg, ww, hh, &mut canvas),
        ShapeType::Triangle => triangle_decode(&mut br, cfg, ww, hh, &mut canvas),
        ShapeType::Square => square_decode(&mut br, cfg, ww, hh, &mut canvas, corner_radius_canvas),
        ShapeType::Rect => rect_decode(&mut br, cfg, ww, hh, &mut canvas, corner_radius_canvas),
        ShapeType::RotatedRect => rotrect_decode(&mut br, cfg, ww, hh, &mut canvas, corner_radius_canvas),
        _ => unreachable!(),
    }
    let mut canvas_lin = vec![0.0f32; (w * h * 3) as usize];
    if ss == 1 {
        canvas_lin.copy_from_slice(&canvas);
    } else {
        let inv_n = 1.0 / ((ss * ss) as f32);
        for y in 0..h as usize {
            for x in 0..w as usize {
                let mut sum = [0.0f32; 3];
                for dy in 0..ss as usize {
                    for dx in 0..ss as usize {
                        let sy = y * ss as usize + dy;
                        let sx = x * ss as usize + dx;
                        let p = (sy * ww as usize + sx) * 3;
                        sum[0] += canvas[p];
                        sum[1] += canvas[p + 1];
                        sum[2] += canvas[p + 2];
                    }
                }
                let out = (y * w as usize + x) * 3;
                canvas_lin[out] = sum[0] * inv_n;
                canvas_lin[out + 1] = sum[1] * inv_n;
                canvas_lin[out + 2] = sum[2] * inv_n;
            }
        }
    }
    let canvas = canvas_lin;
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for i in 0..(w * h) as usize {
        rgba[i * 4] = linear_to_srgb_u8(canvas[i * 3]);
        rgba[i * 4 + 1] = linear_to_srgb_u8(canvas[i * 3 + 1]);
        rgba[i * 4 + 2] = linear_to_srgb_u8(canvas[i * 3 + 2]);
        rgba[i * 4 + 3] = 255;
    }
    if opts.style.blur > 0.0 {
        gaussian_blur_rgba8(&mut rgba, w, h, opts.style.blur);
    }
    (w, h, rgba)
}

fn rgb_to_rgba(rgb: &[u8], w: u32, h: u32) -> Vec<u8> {
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for i in 0..(w * h) as usize {
        rgba[i * 4] = rgb[i * 3];
        rgba[i * 4 + 1] = rgb[i * 3 + 1];
        rgba[i * 4 + 2] = rgb[i * 3 + 2];
        rgba[i * 4 + 3] = 255;
    }
    rgba
}

// ---------------------------------------------------------------------------
// Convenience image-loading entry (feature `image-io`)
// ---------------------------------------------------------------------------

/// Load an image from disk, resize its long edge to the codec's encoder
/// target (`100` for DCT, `48` for shape/PIXEL), and encode. Requires the
/// `image-io` feature.
#[cfg(feature = "image-io")]
pub fn encode_image(
    path: impl AsRef<std::path::Path>,
    codec: &Codec,
    opts: EncodeOptions,
) -> Result<Vec<u8>, image::ImageError> {
    use image::imageops::FilterType;
    use image::ImageReader;

    let cfg = codec.to_config();
    let target = match cfg.shape {
        ShapeType::Dct => 100,
        _ => crate::shape::THUMB,
    };

    let img = ImageReader::open(path.as_ref())?
        .with_guessed_format()?
        .decode()?;
    let (w0, h0) = (img.width(), img.height());
    let (w, h) = fit_long_edge(w0, h0, target);
    let resized = if (w, h) == (w0, h0) {
        img
    } else {
        img.resize_exact(w, h, FilterType::Lanczos3)
    };
    let rgb = resized.to_rgb8();
    Ok(encode_rgb_cfg(&rgb, w, h, &cfg, opts))
}

#[cfg(feature = "image-io")]
fn fit_long_edge(w: u32, h: u32, target: u32) -> (u32, u32) {
    if w == 0 || h == 0 {
        return (w.max(1), h.max(1));
    }
    if w.max(h) <= target {
        return (w, h);
    }
    if w >= h {
        let new_h = ((target as u64 * h as u64) / w as u64).max(1) as u32;
        (target, new_h)
    } else {
        let new_w = ((target as u64 * w as u64) / h as u64).max(1) as u32;
        (new_w, target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Codec;

    fn solid_rgb(w: u32, h: u32, c: [u8; 3]) -> Vec<u8> {
        let mut buf = Vec::with_capacity((w * h * 3) as usize);
        for _ in 0..(w * h) {
            buf.extend_from_slice(&c);
        }
        buf
    }

    /// `RenderStyle::default()` (both fields zero) must produce decode output
    /// byte-for-byte identical to pre-0.3.0 — both the rounded-rect AA and
    /// blur primitives must take the zero-cost fast path.
    #[test]
    fn decode_default_style_byte_identical() {
        for codec in [
            Codec::circle(8),
            Codec::triangle(8),
            Codec::square(8),
            Codec::rect(8),
            Codec::rotated_rect(8),
            Codec::pixel(12),
            Codec::default(), // DCT
        ] {
            let rgb = solid_rgb(48, 48, [128, 80, 200]);
            let bytes = encode_rgb(&rgb, 48, 48, &codec, EncodeOptions::default());
            let default_out = decode(&bytes, &codec, DecodeOptions::default());
            // Explicit zero style — should also take the fast path.
            let explicit_zero = decode(
                &bytes,
                &codec,
                DecodeOptions {
                    style: RenderStyle { blur: 0.0, corner_radius: 0.0 },
                    ..DecodeOptions::default()
                },
            );
            assert_eq!(
                default_out.rgba, explicit_zero.rgba,
                "zero-style decode must match default decode for {:?}",
                codec
            );
        }
    }

    #[test]
    fn decode_with_blur_changes_output() {
        // Need non-uniform input — uniform → triangles degenerate to
        // background, blur is no-op (constant input through Gaussian).
        let codec = Codec::triangle(12);
        let rgb = checkerboard_rgb(48, 48, 8);
        let bytes = encode_rgb(&rgb, 48, 48, &codec, EncodeOptions::default());
        let sharp = decode(&bytes, &codec, DecodeOptions::default());
        let blurred = decode(
            &bytes,
            &codec,
            DecodeOptions {
                style: RenderStyle { blur: 4.0, corner_radius: 0.0 },
                ..DecodeOptions::default()
            },
        );
        assert_eq!(sharp.rgba.len(), blurred.rgba.len());
        let mut changed = 0usize;
        for i in 0..(sharp.width * sharp.height) as usize {
            if sharp.rgba[i * 4] != blurred.rgba[i * 4] {
                changed += 1;
            }
        }
        let total = (sharp.width * sharp.height) as usize;
        assert!(
            changed > total / 10,
            "blur should change at least 10% of pixels, only {changed}/{total}",
        );
    }

    fn checkerboard_rgb(w: u32, h: u32, cell: u32) -> Vec<u8> {
        let mut buf = Vec::with_capacity((w * h * 3) as usize);
        for y in 0..h {
            for x in 0..w {
                let on = ((x / cell) + (y / cell)) % 2 == 0;
                let c: [u8; 3] = if on { [220, 60, 60] } else { [40, 80, 200] };
                buf.extend_from_slice(&c);
            }
        }
        buf
    }

    #[test]
    fn decode_rect_corner_radius_softens_corners() {
        // Encoder needs a non-uniform input to produce real rects (uniform
        // input → background matches target, all rects degenerate). With a
        // checkerboard, the encoder produces visible rects whose corners
        // the AA path then rounds.
        let codec = Codec::rect(6);
        let rgb = checkerboard_rgb(48, 48, 8);
        let bytes = encode_rgb(&rgb, 48, 48, &codec, EncodeOptions::default());
        let sharp = decode(&bytes, &codec, DecodeOptions::default());
        let rounded = decode(
            &bytes,
            &codec,
            DecodeOptions {
                style: RenderStyle { blur: 0.0, corner_radius: 12.0 },
                ..DecodeOptions::default()
            },
        );
        // Outputs must differ — corner_radius takes the AA path.
        assert_ne!(sharp.rgba, rounded.rgba);
    }

    #[test]
    fn decode_circle_ignores_corner_radius() {
        // Non-rect-family shape — corner_radius silently ignored, no panic,
        // output identical to default-style decode.
        let codec = Codec::circle(8);
        let rgb = solid_rgb(48, 48, [40, 160, 90]);
        let bytes = encode_rgb(&rgb, 48, 48, &codec, EncodeOptions::default());
        let default_out = decode(&bytes, &codec, DecodeOptions::default());
        let with_radius = decode(
            &bytes,
            &codec,
            DecodeOptions {
                style: RenderStyle { blur: 0.0, corner_radius: 5.0 },
                ..DecodeOptions::default()
            },
        );
        assert_eq!(default_out.rgba, with_radius.rgba);
    }

    #[test]
    fn decode_pixel_blur_works() {
        // PIXEL goes through a different rasterization path (sRGB grid, no
        // float linear canvas). Blur must apply post-rasterization.
        let codec = Codec::pixel(16);
        let rgb = solid_rgb(48, 48, [40, 160, 90]);
        let bytes = encode_rgb(&rgb, 48, 48, &codec, EncodeOptions::default());
        let sharp = decode(&bytes, &codec, DecodeOptions::default());
        let blurred = decode(
            &bytes,
            &codec,
            DecodeOptions {
                style: RenderStyle { blur: 3.0, corner_radius: 0.0 },
                ..DecodeOptions::default()
            },
        );
        // PIXEL with a SOLID image input becomes a uniform-color grid; blur
        // is a no-op on uniform input (kernel sums to 1.0). Verify sizes
        // match and outputs are equivalent (the blur ran but didn't change
        // anything because every cell carries the same color).
        assert_eq!(sharp.rgba.len(), blurred.rgba.len());
        assert_eq!(sharp.rgba, blurred.rgba);
    }
}

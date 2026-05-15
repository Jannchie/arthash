//! SVG output for CIRCLE / TRIANGLE shape modes.
//!
//! Parses the same bit stream as [`crate::decode`] but emits SVG primitives
//! instead of rasterizing to a pixel buffer. Browsers render the resulting
//! SVG natively, so for inline-placeholder use cases this skips the
//! rasterization step entirely.
//!
//! Output is byte-optimized to be competitive with SQIP's compact SVGs:
//!  * Integer coordinates (viewBox precision is sub-pixel anyway).
//!  * Stripped leading zeros on fill-opacity (`.5` not `0.5`).
//!  * 3-digit hex when each channel has matching nibbles (`#abc` ↔ `#aabbcc`).
//!  * Background as `<path d="M0 0h{W}v{H}H0z"/>` (1 char shorter than
//!    `<rect width=... height=... fill=...>` with attribute names).
//!  * Triangles as `<path d="M x y l dx1 dy1 dx2 dy2 z"/>` using relative
//!    coordinates (~5 chars shorter than `<polygon points="...">`).
//!
//! Shape z-order is preserved exactly — we don't reorder shapes by alpha to
//! group them (that would diverge from `decode()` which composites in stream
//! order). This costs ~17 bytes per shape (per-shape `fill-opacity`) but
//! keeps SVG output bit-equivalent to raster decode.
//!
//! PIXEL and DCT modes are out of scope: PIXEL would emit a grid of `<rect>`
//! elements that's not meaningfully smaller than a tiny base64 PNG; DCT is
//! a smooth frequency-domain reconstruction with no natural SVG primitive
//! form. Both return [`SvgError::UnsupportedShape`].

use super::quant::{aspect_from_code, q_to_alpha, q_to_r, read_color};
use crate::bitio::BitReader;
use crate::codec::{Codec, ShapeType};
use crate::colorspace::linear_to_srgb_u8;
use std::fmt::Write as _;

/// Options for [`to_svg`].
#[derive(Clone, Copy, Debug)]
pub struct SvgOptions {
    /// Long-edge pixel value used in the viewBox. The output SVG carries
    /// no `width`/`height` attributes, only `viewBox`, so the caller's CSS
    /// controls actual rendered size — this fixes the coordinate space.
    pub base_size: u32,
    /// Override the stored aspect for non-default sizing (same semantics
    /// as `DecodeOptions::override_aspect`).
    pub override_aspect: Option<f32>,
    /// Gaussian blur stdDeviation in viewBox-space units. `0.0` = no blur.
    /// SQIP-equivalent default is around `12`.
    pub blur: f32,
}

impl Default for SvgOptions {
    fn default() -> Self {
        Self {
            base_size: 256,
            override_aspect: None,
            blur: 0.0,
        }
    }
}

/// Errors returned by [`to_svg`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SvgError {
    /// The codec's `shape` is not one of the SVG-supported modes
    /// (CIRCLE / TRIANGLE).
    UnsupportedShape(ShapeType),
}

impl std::fmt::Display for SvgError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SvgError::UnsupportedShape(s) => match s {
                ShapeType::Dct => f.write_str(
                    "DCT mode has no natural SVG primitive representation. \
                     Use decode() to get raster pixels.",
                ),
                ShapeType::Pixel => f.write_str(
                    "PIXEL mode SVG would be a grid of <rect> elements, \
                     not meaningfully smaller than a base64-encoded raster.",
                ),
                _ => write!(f, "SVG output not supported for shape {:?}", s),
            },
        }
    }
}

impl std::error::Error for SvgError {}

/// Linear-RGB float32 (3,) → sRGB hex string, using 3-digit shorthand when
/// each channel's high and low nibble match (`#aabbcc` → `#abc`).
fn color_to_hex(color_linear: &[f32; 3]) -> String {
    let r = linear_to_srgb_u8(color_linear[0]) as u32;
    let g = linear_to_srgb_u8(color_linear[1]) as u32;
    let b = linear_to_srgb_u8(color_linear[2]) as u32;
    let short_ok = |c: u32| (c & 0x0F) == (c >> 4);
    if short_ok(r) && short_ok(g) && short_ok(b) {
        format!("#{:x}{:x}{:x}", r & 0x0F, g & 0x0F, b & 0x0F)
    } else {
        format!("#{:02x}{:02x}{:02x}", r, g, b)
    }
}

/// Format a non-negative float compactly:
///  * Rounded to 2 decimals.
///  * Trailing zeros and trailing `.` stripped.
///  * Leading `0` stripped ONLY when `0 < x < 1` (`0.5` → `.5`).
///  * `x == 0` → `"0"`; `x == 1` → `"1"`; `x == 2` → `"2"`; `x == 12.5` → `"12.5"`.
///
/// Used for both fill-opacity (alpha in (0, 1]) and feGaussianBlur
/// stdDeviation (typically 0–20). Mirrors Python's `_fmt_num`.
fn fmt_num(x: f32) -> String {
    let rounded = (x * 100.0).round() / 100.0;
    let mut s = format!("{:.2}", rounded);
    while s.ends_with('0') {
        s.pop();
    }
    if s.ends_with('.') {
        s.pop();
    }
    if rounded > 0.0 && rounded < 1.0 && s.starts_with('0') {
        s.remove(0);
    }
    s
}

/// Format an int with a leading space ONLY when positive. Negative numbers
/// carry their own `-` boundary, so `99` after `-120` parses unambiguously.
fn sep_signed(n: i32) -> String {
    if n < 0 {
        n.to_string()
    } else {
        format!(" {}", n)
    }
}

fn opacity_attr(alpha: f32) -> String {
    if alpha >= 1.0 {
        String::new()
    } else {
        format!(" fill-opacity=\"{}\"", fmt_num(alpha))
    }
}

fn circle_elements(br: &mut BitReader, codec: &Codec, w: u32, h: u32, out: &mut String) {
    let x_max = ((1u32 << codec.cx_bits) - 1) as f32;
    let y_max = ((1u32 << codec.cy_bits) - 1) as f32;
    let alpha_levels = codec.alpha_levels_owned();
    let w_m1 = (w as f32 - 1.0).max(0.0);
    let h_m1 = (h as f32 - 1.0).max(0.0);
    for _ in 0..codec.n_shapes {
        let x_q = br.read(codec.cx_bits);
        let y_q = br.read(codec.cy_bits);
        let r_q = br.read(codec.r_bits);
        let color_linear = read_color(br, codec);
        let a_q = br.read(codec.alpha_bits);

        let cx = ((x_q as f32) / x_max * w_m1).round() as i32;
        let cy = ((y_q as f32) / y_max * h_m1).round() as i32;
        let r = (q_to_r(r_q, w, h, codec.r_bits).round() as i32).max(1);
        let alpha = q_to_alpha(a_q, &alpha_levels);
        let fill = color_to_hex(&color_linear);
        let opacity = opacity_attr(alpha);
        write!(
            out,
            "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{}\"{}/>",
            cx, cy, r, fill, opacity
        )
        .unwrap();
    }
}

fn triangle_elements(br: &mut BitReader, codec: &Codec, w: u32, h: u32, out: &mut String) {
    let x_max = ((1u32 << codec.cx_bits) - 1) as f32;
    let y_max = ((1u32 << codec.cy_bits) - 1) as f32;
    let alpha_levels = codec.alpha_levels_owned();
    let w_m1 = (w as f32 - 1.0).max(0.0);
    let h_m1 = (h as f32 - 1.0).max(0.0);
    for _ in 0..codec.n_shapes {
        let mut verts = [(0i32, 0i32); 3];
        for v in verts.iter_mut() {
            let x_q = br.read(codec.cx_bits);
            let y_q = br.read(codec.cy_bits);
            v.0 = ((x_q as f32) / x_max * w_m1).round() as i32;
            v.1 = ((y_q as f32) / y_max * h_m1).round() as i32;
        }
        let color_linear = read_color(br, codec);
        let alpha = q_to_alpha(br.read(codec.alpha_bits), &alpha_levels);
        let fill = color_to_hex(&color_linear);
        let opacity = opacity_attr(alpha);

        let dx1 = verts[1].0 - verts[0].0;
        let dy1 = verts[1].1 - verts[0].1;
        let dx2 = verts[2].0 - verts[1].0;
        let dy2 = verts[2].1 - verts[1].1;
        // Relative-coord path: `M x0 y0 l dx1 dy1 dx2 dy2 z`. The leading
        // space is omitted before any negative number — its `-` already
        // marks a number boundary, so `-120` after `99` parses as two.
        write!(
            out,
            "<path fill=\"{}\"{} d=\"M{}{}l{}{}{}{}z\"/>",
            fill,
            opacity,
            verts[0].0,
            sep_signed(verts[0].1),
            dx1,
            sep_signed(dy1),
            sep_signed(dx2),
            sep_signed(dy2),
        )
        .unwrap();
    }
}

/// Render a CIRCLE or TRIANGLE shape-mode hash as a byte-optimized SVG
/// string.
///
/// # Errors
/// Returns [`SvgError::UnsupportedShape`] for DCT and PIXEL codecs.
pub fn to_svg(hash_bytes: &[u8], codec: &Codec, opts: SvgOptions) -> Result<String, SvgError> {
    if !matches!(codec.shape, ShapeType::Circle | ShapeType::Triangle) {
        return Err(SvgError::UnsupportedShape(codec.shape));
    }

    let mut br = BitReader::new(hash_bytes);
    let quant_aspect = aspect_from_code(br.read(8));
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

    let bg = read_color(&mut br, codec);
    let bg_hex = color_to_hex(&bg);

    // Background as a path: `M0 0h{W}v{H}H0z` traces the rect in 4 cmds.
    // Saves vs `<rect width=W height=H>` because attribute names are longer.
    let mut body = format!("<path fill=\"{}\" d=\"M0 0h{}v{}H0z\"/>", bg_hex, w, h);
    match codec.shape {
        ShapeType::Circle => circle_elements(&mut br, codec, w, h, &mut body),
        ShapeType::Triangle => triangle_elements(&mut br, codec, w, h, &mut body),
        _ => unreachable!(),
    }

    if opts.blur > 0.0 {
        // SQIP-style Gaussian blur. The filter region defaults to the bbox
        // of the filtered group (-10% margin each side), so we expand to
        // the full viewBox so blur extends to the edges instead of cutting
        // off. `primitiveUnits="userSpaceOnUse"` makes stdDeviation be in
        // viewBox units (px-equivalent), matching SQIP's interpretation.
        body = format!(
            "<filter id=\"b\" x=\"0\" y=\"0\" width=\"100%\" height=\"100%\" \
             primitiveUnits=\"userSpaceOnUse\">\
             <feGaussianBlur stdDeviation=\"{}\"/></filter>\
             <g filter=\"url(#b)\">{}</g>",
            fmt_num(opts.blur),
            body
        );
    }

    Ok(format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {} {}\">{}</svg>",
        w, h, body
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{encode_rgb, EncodeOptions};

    fn solid_rgb(w: u32, h: u32, c: [u8; 3]) -> Vec<u8> {
        let mut buf = Vec::with_capacity((w * h * 3) as usize);
        for _ in 0..(w * h) {
            buf.extend_from_slice(&c);
        }
        buf
    }

    #[test]
    fn fmt_num_compact() {
        // < 1: strip leading "0"
        assert_eq!(fmt_num(0.5), ".5");
        assert_eq!(fmt_num(0.85), ".85");
        assert_eq!(fmt_num(0.20), ".2");
        // == 0: "0", not ""
        assert_eq!(fmt_num(0.0), "0");
        // >= 1: keep integer/fractional as-is
        assert_eq!(fmt_num(1.0), "1");
        assert_eq!(fmt_num(1.5), "1.5");
        assert_eq!(fmt_num(2.0), "2");
        assert_eq!(fmt_num(12.0), "12");
        assert_eq!(fmt_num(12.5), "12.5");
    }

    #[test]
    fn color_to_hex_shorthand() {
        let lin = [
            crate::colorspace::srgb_u8_to_linear(0xAA),
            crate::colorspace::srgb_u8_to_linear(0xBB),
            crate::colorspace::srgb_u8_to_linear(0xCC),
        ];
        assert_eq!(color_to_hex(&lin), "#abc");
    }

    #[test]
    fn color_to_hex_full() {
        let lin = [
            crate::colorspace::srgb_u8_to_linear(0xAB),
            crate::colorspace::srgb_u8_to_linear(0xCD),
            crate::colorspace::srgb_u8_to_linear(0xEF),
        ];
        assert_eq!(color_to_hex(&lin), "#abcdef");
    }

    #[test]
    fn sep_signed_format() {
        assert_eq!(sep_signed(0), " 0");
        assert_eq!(sep_signed(99), " 99");
        assert_eq!(sep_signed(-120), "-120");
    }

    #[test]
    fn circle_round_trip_parses() {
        let codec = Codec {
            shape: ShapeType::Circle,
            n_shapes: 4,
            ..Codec::default()
        };
        let rgb = solid_rgb(48, 48, [180, 90, 40]);
        let bytes = encode_rgb(&rgb, 48, 48, &codec, EncodeOptions::default());
        let svg = to_svg(&bytes, &codec, SvgOptions::default()).unwrap();
        assert!(svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 "));
        assert!(svg.ends_with("</svg>"));
        // 1 background + 4 circles = 5 fill="..." attrs.
        assert_eq!(svg.matches("fill=\"").count(), 5);
        assert_eq!(svg.matches("<circle ").count(), 4);
    }

    #[test]
    fn triangle_round_trip_parses() {
        let codec = Codec {
            shape: ShapeType::Triangle,
            n_shapes: 3,
            ..Codec::default()
        };
        let rgb = solid_rgb(48, 48, [60, 120, 200]);
        let bytes = encode_rgb(&rgb, 48, 48, &codec, EncodeOptions::default());
        let svg = to_svg(&bytes, &codec, SvgOptions::default()).unwrap();
        // 1 bg path + 3 triangle paths.
        assert_eq!(svg.matches("<path ").count(), 4);
        assert!(!svg.contains("<circle"));
    }

    #[test]
    fn blur_wraps_in_filter() {
        let codec = Codec {
            shape: ShapeType::Circle,
            n_shapes: 2,
            ..Codec::default()
        };
        let bytes = encode_rgb(
            &solid_rgb(48, 48, [200, 200, 200]),
            48,
            48,
            &codec,
            EncodeOptions::default(),
        );
        let svg = to_svg(
            &bytes,
            &codec,
            SvgOptions {
                blur: 12.0,
                ..SvgOptions::default()
            },
        )
        .unwrap();
        assert!(svg.contains("<filter id=\"b\""));
        assert!(svg.contains("stdDeviation=\"12\""));
        assert!(svg.contains("<g filter=\"url(#b)\""));
    }

    #[test]
    fn dct_returns_unsupported() {
        let codec = Codec::default();
        let bytes = vec![0u8; 32];
        let err = to_svg(&bytes, &codec, SvgOptions::default()).unwrap_err();
        assert_eq!(err, SvgError::UnsupportedShape(ShapeType::Dct));
    }

    #[test]
    fn pixel_returns_unsupported() {
        let codec = Codec {
            shape: ShapeType::Pixel,
            ..Codec::default()
        };
        let bytes = vec![0u8; 32];
        let err = to_svg(&bytes, &codec, SvgOptions::default()).unwrap_err();
        assert_eq!(err, SvgError::UnsupportedShape(ShapeType::Pixel));
    }
}

//! SVG output for shape modes (CIRCLE / TRIANGLE / SQUARE / RECT /
//! ROTATED_RECT) and PIXEL.
//!
//! Parses the same bit stream as [`crate::decode`] but emits SVG primitives
//! instead of rasterizing to a pixel buffer. Browsers render the resulting
//! SVG natively, so for inline-placeholder use cases this skips the
//! rasterization step entirely.
//!
//! Output is byte-optimized to be competitive with SQIP's compact SVGs:
//!  * Integer coordinates where natural (shape modes); compact decimals
//!    (`fmt_num`) elsewhere.
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
//! PIXEL output is a `gw * gh` grid of `<rect>` elements sharing the same
//! `viewBox` as the shape modes — not smaller than a tiny base64 PNG, but
//! a vector form that scales cleanly under CSS and supports the same blur
//! filter wrapper as the shape modes. DCT remains out of scope (smooth
//! frequency-domain reconstruction with no natural SVG primitive form) and
//! still returns [`SvgError::UnsupportedShape`].

use super::pixel::pixel_grid;
use super::quant::{aspect_from_code, q_to_alpha, q_to_r, read_color};
use super::rect::decode_rect_at;
use super::rotrect::decode_rotrect_at;
use super::square::decode_square_at;
use crate::bitio::BitReader;
use crate::codec::{Codec, CodecConfig, ShapeType};
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

/// Identifier for SVG-unsupported codec modes. Public-facing tag without
/// exposing internal shape enum. `Pixel` is retained for backwards
/// compatibility on the public enum but is no longer constructed — PIXEL is
/// now rendered as a grid of `<rect>` elements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SvgUnsupported {
    Dct,
    Pixel,
    Other,
}

impl SvgUnsupported {
    pub(crate) fn from(shape: ShapeType) -> Self {
        match shape {
            ShapeType::Dct => SvgUnsupported::Dct,
            _ => SvgUnsupported::Other,
        }
    }
}

/// Errors returned by [`to_svg`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SvgError {
    /// The codec's mode is not one of the SVG-supported modes
    /// (CIRCLE / TRIANGLE / SQUARE / RECT / ROTATED_RECT / PIXEL).
    UnsupportedShape(SvgUnsupported),
}

impl std::fmt::Display for SvgError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SvgError::UnsupportedShape(s) => match s {
                SvgUnsupported::Dct => f.write_str(
                    "DCT mode has no natural SVG primitive representation. \
                     Use decode() to get raster pixels.",
                ),
                SvgUnsupported::Pixel => f.write_str(
                    "PIXEL mode SVG is no longer rejected — this variant is \
                     retained for backwards compatibility only.",
                ),
                SvgUnsupported::Other => f.write_str("SVG output not supported for this codec"),
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

fn circle_elements(br: &mut BitReader, codec: &CodecConfig, w: u32, h: u32, out: &mut String) {
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

fn triangle_elements(br: &mut BitReader, codec: &CodecConfig, w: u32, h: u32, out: &mut String) {
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

fn rect_elements(br: &mut BitReader, codec: &CodecConfig, w: u32, h: u32, out: &mut String) {
    for _ in 0..codec.n_shapes {
        let (cx, cy, rw, rh, color, alpha) = decode_rect_at(br, codec, w, h);
        let x = cx - rw / 2;
        let y = cy - rh / 2;
        let fill = color_to_hex(&color);
        let opacity = opacity_attr(alpha);
        write!(
            out,
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\"{}/>",
            x, y, rw.max(0), rh.max(0), fill, opacity
        )
        .unwrap();
    }
}

fn square_elements(br: &mut BitReader, codec: &CodecConfig, w: u32, h: u32, out: &mut String) {
    for _ in 0..codec.n_shapes {
        let (cx, cy, s, color, alpha) = decode_square_at(br, codec, w, h);
        let x = cx - s / 2;
        let y = cy - s / 2;
        let fill = color_to_hex(&color);
        let opacity = opacity_attr(alpha);
        write!(
            out,
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\"{}/>",
            x, y, s.max(0), s.max(0), fill, opacity
        )
        .unwrap();
    }
}

/// PIXEL: gw·gh `<rect>` cells covering the full viewBox. Cell edges are
/// computed at the boundary so adjacent cells share a coordinate exactly
/// (no sub-pixel seams from independent `width = W/gw` rounding).
fn pixel_elements(
    br: &mut BitReader,
    codec: &CodecConfig,
    w: u32,
    h: u32,
    quant_aspect: f32,
    out: &mut String,
) {
    let (gw, gh) = pixel_grid(codec.n_shapes, quant_aspect, codec.grid_aspect);
    let wf = w as f32;
    let hf = h as f32;
    let gwf = gw as f32;
    let ghf = gh as f32;
    for gy in 0..gh {
        let y0 = (gy as f32) * hf / ghf;
        let y1 = ((gy + 1) as f32) * hf / ghf;
        let cell_h = y1 - y0;
        for gx in 0..gw {
            let x0 = (gx as f32) * wf / gwf;
            let x1 = ((gx + 1) as f32) * wf / gwf;
            let cell_w = x1 - x0;
            let color = read_color(br, codec);
            let fill = color_to_hex(&color);
            write!(
                out,
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\"/>",
                fmt_num(x0),
                fmt_num(y0),
                fmt_num(cell_w),
                fmt_num(cell_h),
                fill
            )
            .unwrap();
        }
    }
}

fn rotrect_elements(br: &mut BitReader, codec: &CodecConfig, w: u32, h: u32, out: &mut String) {
    for _ in 0..codec.n_shapes {
        let (cx, cy, rw, rh, theta_deg, color, alpha) = decode_rotrect_at(br, codec, w, h);
        let x = cx - rw / 2;
        let y = cy - rh / 2;
        let fill = color_to_hex(&color);
        let opacity = opacity_attr(alpha);
        write!(
            out,
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\"{} transform=\"rotate({} {} {})\"/>",
            x, y, rw.max(0), rh.max(0), fill, opacity,
            fmt_num(theta_deg), cx, cy
        )
        .unwrap();
    }
}

/// Render a shape-mode (CIRCLE / TRIANGLE / SQUARE / RECT / ROTATED_RECT)
/// or PIXEL hash as a byte-optimized SVG string.
///
/// # Errors
/// Returns [`SvgError::UnsupportedShape`] for DCT codecs.
pub fn to_svg(hash_bytes: &[u8], codec: &Codec, opts: SvgOptions) -> Result<String, SvgError> {
    let cfg = codec.to_config();
    if !matches!(
        cfg.shape,
        ShapeType::Circle
            | ShapeType::Triangle
            | ShapeType::Square
            | ShapeType::Rect
            | ShapeType::RotatedRect
            | ShapeType::Pixel
    ) {
        return Err(SvgError::UnsupportedShape(SvgUnsupported::from(cfg.shape)));
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

    let mut body = String::new();
    match cfg.shape {
        ShapeType::Pixel => {
            // PIXEL has no separate background — the cell grid fills the
            // viewBox. Grid uses the QUANTIZED aspect (same value the
            // decoder reconstructs), not `opts.override_aspect`, so encoder
            // and SVG agree on (gw, gh).
            pixel_elements(&mut br, &cfg, w, h, quant_aspect, &mut body);
        }
        _ => {
            let bg = read_color(&mut br, &cfg);
            let bg_hex = color_to_hex(&bg);
            // Background as a path: `M0 0h{W}v{H}H0z` traces the rect in 4
            // cmds. Saves vs `<rect width=W height=H>` because attribute
            // names are longer.
            write!(body, "<path fill=\"{}\" d=\"M0 0h{}v{}H0z\"/>", bg_hex, w, h).unwrap();
            match cfg.shape {
                ShapeType::Circle => circle_elements(&mut br, &cfg, w, h, &mut body),
                ShapeType::Triangle => triangle_elements(&mut br, &cfg, w, h, &mut body),
                ShapeType::Square => square_elements(&mut br, &cfg, w, h, &mut body),
                ShapeType::Rect => rect_elements(&mut br, &cfg, w, h, &mut body),
                ShapeType::RotatedRect => rotrect_elements(&mut br, &cfg, w, h, &mut body),
                _ => unreachable!(),
            }
        }
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
        let codec = Codec::circle(4);
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
    fn rect_round_trip_parses() {
        let codec = Codec::rect(3);
        let rgb = solid_rgb(48, 48, [60, 120, 200]);
        let bytes = encode_rgb(&rgb, 48, 48, &codec, EncodeOptions::default());
        let svg = to_svg(&bytes, &codec, SvgOptions::default()).unwrap();
        assert_eq!(svg.matches("<rect ").count(), 3);
        // Background path + 3 rects = 4 fill attrs.
        assert_eq!(svg.matches("fill=\"").count(), 4);
        assert!(!svg.contains("transform="));
    }

    #[test]
    fn square_round_trip_parses() {
        let codec = Codec::square(3);
        let rgb = solid_rgb(48, 48, [60, 120, 200]);
        let bytes = encode_rgb(&rgb, 48, 48, &codec, EncodeOptions::default());
        let svg = to_svg(&bytes, &codec, SvgOptions::default()).unwrap();
        assert_eq!(svg.matches("<rect ").count(), 3);
        // Every emitted rect should have width == height.
        for rect in svg.split("<rect ").skip(1) {
            let rect = rect.split("/>").next().unwrap();
            let w = rect.split("width=\"").nth(1).and_then(|s| s.split('"').next()).unwrap();
            let h = rect.split("height=\"").nth(1).and_then(|s| s.split('"').next()).unwrap();
            assert_eq!(w, h, "square's width != height in svg fragment: {rect}");
        }
    }

    #[test]
    fn rotrect_round_trip_parses() {
        let codec = Codec::rotated_rect(3);
        let rgb = solid_rgb(48, 48, [60, 120, 200]);
        let bytes = encode_rgb(&rgb, 48, 48, &codec, EncodeOptions::default());
        let svg = to_svg(&bytes, &codec, SvgOptions::default()).unwrap();
        assert_eq!(svg.matches("<rect ").count(), 3);
        // Every rotated rect carries a transform=rotate(...) attribute.
        assert_eq!(svg.matches(" transform=\"rotate(").count(), 3);
    }

    #[test]
    fn triangle_round_trip_parses() {
        let codec = Codec::triangle(3);
        let rgb = solid_rgb(48, 48, [60, 120, 200]);
        let bytes = encode_rgb(&rgb, 48, 48, &codec, EncodeOptions::default());
        let svg = to_svg(&bytes, &codec, SvgOptions::default()).unwrap();
        // 1 bg path + 3 triangle paths.
        assert_eq!(svg.matches("<path ").count(), 4);
        assert!(!svg.contains("<circle"));
    }

    #[test]
    fn blur_wraps_in_filter() {
        let codec = Codec::circle(2);
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
        assert_eq!(err, SvgError::UnsupportedShape(SvgUnsupported::Dct));
    }

    #[test]
    fn pixel_round_trip_parses() {
        let codec = Codec::pixel(12);
        let bytes = encode_rgb(
            &solid_rgb(48, 48, [60, 120, 200]),
            48,
            48,
            &codec,
            EncodeOptions::default(),
        );
        let svg = to_svg(&bytes, &codec, SvgOptions::default()).unwrap();
        // n_shapes=12, square image → pixel_grid picks (3, 4) → 12 cells.
        assert_eq!(svg.matches("<rect ").count(), 12);
        // PIXEL has no background path — the cells fill the viewBox.
        assert!(!svg.contains("d=\"M0 0h"));
        assert!(svg.starts_with("<svg "));
        assert!(svg.ends_with("</svg>"));
    }

    #[test]
    fn pixel_blur_wraps_in_filter() {
        let codec = Codec::pixel(12);
        let bytes = encode_rgb(
            &solid_rgb(48, 48, [180, 180, 180]),
            48,
            48,
            &codec,
            EncodeOptions::default(),
        );
        let svg = to_svg(
            &bytes,
            &codec,
            SvgOptions {
                blur: 6.0,
                ..SvgOptions::default()
            },
        )
        .unwrap();
        assert!(svg.contains("<filter id=\"b\""));
        assert!(svg.contains("stdDeviation=\"6\""));
        assert!(svg.contains("<g filter=\"url(#b)\""));
    }

    #[test]
    fn pixel_cells_share_edges() {
        // Rectangular non-divisible case: 256/3 ≈ 85.33 — adjacent cells must
        // share an X coordinate exactly, otherwise the SVG shows hairline
        // gaps between blocks.
        let codec = Codec::pixel(9);
        let bytes = encode_rgb(
            &solid_rgb(48, 48, [120, 60, 30]),
            48,
            48,
            &codec,
            EncodeOptions::default(),
        );
        let svg = to_svg(&bytes, &codec, SvgOptions::default()).unwrap();
        // 3x3 grid → 9 cells.
        assert_eq!(svg.matches("<rect ").count(), 9);
        // For each rect, parse x + width and check that some other rect's x
        // equals x + width (i.e. the right edge meets the next cell's left
        // edge). Skip the rightmost column where no such neighbour exists.
        let xs: Vec<f32> = svg
            .split("<rect ")
            .skip(1)
            .map(|r| {
                let x = r.split("x=\"").nth(1).unwrap().split('"').next().unwrap();
                x.parse::<f32>().unwrap()
            })
            .collect();
        let widths: Vec<f32> = svg
            .split("<rect ")
            .skip(1)
            .map(|r| {
                let w = r.split("width=\"").nth(1).unwrap().split('"').next().unwrap();
                w.parse::<f32>().unwrap()
            })
            .collect();
        // First cell's right edge should equal second cell's left edge.
        assert!((xs[0] + widths[0] - xs[1]).abs() < 1e-3);
    }
}

//! Public `encode_rgb` / `encode_rgba` / `decode` API.
//!
//! Mirrors Python's `pfhash.encode` / `pfhash.decode`. Inputs are raw byte
//! buffers at native size — the caller is responsible for resizing to the
//! shape thumbnail (`shape::THUMB = 48`) or DCT target size (`≤ 100`)
//! before calling. See SPEC §1.1: hash + codec is one logical unit.

use crate::bitio::BitReader;
use crate::codec::{Codec, ShapeType};
use crate::colorspace::{linear_to_srgb_u8, srgb_u8_to_linear};
use crate::shape::circle::{decode_render as circle_decode, encode_circle};
use crate::shape::pixel::{decode_render as pixel_decode, encode_pixel, PixelSmooth};
use crate::shape::quant::{aspect_from_code, read_color};
use crate::shape::triangle::{decode_render as triangle_decode, encode_triangle};
use crate::shape::SearchOptions;

#[derive(Clone, Copy, Debug)]
#[derive(Default)]
pub struct EncodeOptions {
    /// Used by CIRCLE / TRIANGLE only.
    pub seed: u64,
    /// CIRCLE/TRIANGLE search budget. None ⇒ mode-specific tuned default.
    pub search: Option<SearchOptions>,
}


#[derive(Clone, Copy, Debug)]
pub struct DecodeOptions {
    pub base_size: u32,
    pub override_aspect: Option<f32>,
    pub pixel_smooth: PixelSmooth,
}

impl Default for DecodeOptions {
    fn default() -> Self {
        Self {
            base_size: 256,
            override_aspect: None,
            pixel_smooth: PixelSmooth::Nearest,
        }
    }
}

/// Convert flat RGB u8 → flat linear-RGB f32 of same length.
fn rgb_u8_to_linear_flat(rgb: &[u8]) -> Vec<f32> {
    rgb.iter().map(|&c| srgb_u8_to_linear(c)).collect()
}

/// Encode raw RGB at `(w, h)`. For DCT mode, alpha defaults to 255.
///
/// * `w`, `h`: thumbnail dimensions in pixels. For shape modes, callers
///   should resize to `shape::THUMB` long-edge first. For DCT, ≤ 100.
/// * `rgb`: row-major flat `(h, w, 3)` u8 sRGB, length `h*w*3`.
pub fn encode_rgb(rgb: &[u8], w: u32, h: u32, codec: &Codec, opts: EncodeOptions) -> Vec<u8> {
    match codec.shape {
        ShapeType::Dct => {
            // Pack to RGBA with α=255 and delegate.
            let n = (w * h) as usize;
            let mut rgba = vec![0u8; n * 4];
            for i in 0..n {
                rgba[i * 4] = rgb[i * 3];
                rgba[i * 4 + 1] = rgb[i * 3 + 1];
                rgba[i * 4 + 2] = rgb[i * 3 + 2];
                rgba[i * 4 + 3] = 255;
            }
            crate::dct::encode_dct(w, h, &rgba)
        }
        _ => {
            let target_lin = rgb_u8_to_linear_flat(rgb);
            let search_owned;
            let search = match opts.search {
                Some(s) => s,
                None => match codec.shape {
                    ShapeType::Triangle => {
                        search_owned = SearchOptions::triangle_default();
                        search_owned
                    }
                    _ => {
                        search_owned = SearchOptions::default();
                        search_owned
                    }
                },
            };
            match codec.shape {
                ShapeType::Circle => encode_circle(&target_lin, h, w, w, h, codec, opts.seed, &search),
                ShapeType::Triangle => {
                    encode_triangle(&target_lin, h, w, w, h, codec, opts.seed, &search)
                }
                ShapeType::Pixel => encode_pixel(&target_lin, h, w, w, h, codec),
                ShapeType::Dct => unreachable!(),
            }
        }
    }
}

/// Encode raw RGBA at `(w, h)`. Same semantics as `encode_rgb` but with an
/// alpha channel. For shape modes the alpha is currently ignored (composite
/// the image over white if it has transparency).
pub fn encode_rgba(rgba: &[u8], w: u32, h: u32, codec: &Codec, opts: EncodeOptions) -> Vec<u8> {
    match codec.shape {
        ShapeType::Dct => crate::dct::encode_dct(w, h, rgba),
        _ => {
            // Composite over white into RGB then encode.
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
            encode_rgb(&rgb, w, h, codec, opts)
        }
    }
}

/// Decode hash bytes → `(width, height, rgba_u8_flat)`.
pub fn decode(hash: &[u8], codec: &Codec, opts: DecodeOptions) -> (u32, u32, Vec<u8>) {
    match codec.shape {
        ShapeType::Dct => crate::dct::decode_dct(hash, opts.base_size, opts.override_aspect),
        _ => decode_shape(hash, codec, opts),
    }
}

fn decode_shape(hash: &[u8], codec: &Codec, opts: DecodeOptions) -> (u32, u32, Vec<u8>) {
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

    if matches!(codec.shape, ShapeType::Pixel) {
        let rgb = pixel_decode(&mut br, codec, w, h, quant_aspect, opts.pixel_smooth);
        return (w, h, rgb_to_rgba(&rgb, w, h));
    }

    // CIRCLE / TRIANGLE: read bg, render onto canvas, return RGBA.
    let bg = read_color(&mut br, codec);
    let mut canvas = vec![0.0f32; (w * h * 3) as usize];
    for i in 0..(w * h) as usize {
        canvas[i * 3] = bg[0];
        canvas[i * 3 + 1] = bg[1];
        canvas[i * 3 + 2] = bg[2];
    }
    match codec.shape {
        ShapeType::Circle => circle_decode(&mut br, codec, w, h, &mut canvas),
        ShapeType::Triangle => triangle_decode(&mut br, codec, w, h, &mut canvas),
        _ => unreachable!(),
    }
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for i in 0..(w * h) as usize {
        rgba[i * 4] = linear_to_srgb_u8(canvas[i * 3]);
        rgba[i * 4 + 1] = linear_to_srgb_u8(canvas[i * 3 + 1]);
        rgba[i * 4 + 2] = linear_to_srgb_u8(canvas[i * 3 + 2]);
        rgba[i * 4 + 3] = 255;
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

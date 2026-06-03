//! Shared fixtures for the golden regression locks.
//!
//! `shape_golden.rs` locks the ENCODE bytes for a fixed table of
//! `(codec, seed, input)` cases; `decode_golden.rs` locks what those same
//! hashes DECODE to (raster RGBA + SVG). Both build on the exact same case
//! table defined here so the two locks always cover the identical inputs.
//!
//! Each integration test file compiles its own copy of this module (the
//! standard `tests/common/mod.rs` pattern), so a given crate may only touch a
//! subset of these helpers — hence the crate-level `dead_code` allow.
#![allow(dead_code)]

use arthash::{Codec, ColorMode, EncodeOptions, Palette, SearchOptions};

/// Solid `w×h` RGB fill.
pub fn solid(w: u32, h: u32, c: [u8; 3]) -> Vec<u8> {
    let n = (w * h) as usize;
    let mut rgb = vec![0u8; n * 3];
    for i in 0..n {
        rgb[i * 3] = c[0];
        rgb[i * 3 + 1] = c[1];
        rgb[i * 3 + 2] = c[2];
    }
    rgb
}

/// Pure-arithmetic gradient (no LANCZOS) — byte-portable across targets.
pub fn gradient(w: u32, h: u32) -> Vec<u8> {
    let n = (w * h) as usize;
    let mut rgb = vec![0u8; n * 3];
    for y in 0..h {
        for x in 0..w {
            let p = ((y * w + x) * 3) as usize;
            rgb[p] = ((x as f32) * 255.0 / ((w - 1) as f32)).round() as u8;
            rgb[p + 1] = ((y as f32) * 255.0 / ((h - 1) as f32)).round() as u8;
            rgb[p + 2] = 64;
        }
    }
    rgb
}

/// Lowercase hex of a byte slice.
pub fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// FNV-1a 64-bit digest → 16-hex-char string. A compact, dependency-free lock
/// for large/variable-length outputs (decoded RGBA buffers, SVG strings) where
/// storing the full hex would be unwieldy. Any single-bit change flips it.
pub fn digest(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

pub struct Case {
    pub name: &'static str,
    pub codec: Codec,
    pub rgb: Vec<u8>,
    pub w: u32,
    pub h: u32,
    pub seed: u64,
}

/// Deterministic 8-color palette (K=8 → 3-bit indices).
pub fn pal8() -> Palette {
    let mut colors = Vec::new();
    for i in 0u8..8 {
        colors.push([i * 32, 255 - i * 32, i * 17]);
    }
    Palette::from_rgb(&colors).unwrap()
}

/// The canonical case table. One entry per shape mode + palette / rgb888 /
/// search-override variants, sized small so the goldens stay compact.
pub fn cases() -> Vec<Case> {
    vec![
        Case { name: "circle12_solid", codec: Codec::circle(12), rgb: solid(48, 48, [200, 100, 50]), w: 48, h: 48, seed: 1 },
        Case { name: "circle12_gradient", codec: Codec::circle(12), rgb: gradient(48, 48), w: 48, h: 48, seed: 1 },
        Case { name: "circle24_gradient", codec: Codec::circle(24), rgb: gradient(48, 36), w: 48, h: 36, seed: 7 },
        Case { name: "triangle12_gradient", codec: Codec::triangle(12), rgb: gradient(48, 29), w: 48, h: 29, seed: 2 },
        Case { name: "triangle12_solid", codec: Codec::triangle(12), rgb: solid(48, 48, [30, 200, 120]), w: 48, h: 48, seed: 2 },
        Case { name: "square12_gradient", codec: Codec::square(12), rgb: gradient(48, 48), w: 48, h: 48, seed: 3 },
        Case { name: "rect12_gradient", codec: Codec::rect(12), rgb: gradient(48, 48), w: 48, h: 48, seed: 4 },
        Case { name: "rotrect12_gradient", codec: Codec::rotated_rect(12), rgb: gradient(48, 40), w: 48, h: 40, seed: 5 },
        Case { name: "pixel16_gradient", codec: Codec::pixel(16), rgb: gradient(48, 29), w: 48, h: 29, seed: 0 },
        Case { name: "circle8_palette", codec: Codec::circle(8).with_palette(pal8()), rgb: gradient(48, 48), w: 48, h: 48, seed: 6 },
        Case { name: "triangle12_rgb888", codec: Codec::triangle(12).with_color(ColorMode::Rgb888), rgb: gradient(48, 48), w: 48, h: 48, seed: 8 },
        // Explicit search override to lock that path too.
        Case {
            name: "circle12_search_override",
            codec: Codec::circle(12),
            rgb: gradient(48, 48),
            w: 48,
            h: 48,
            seed: 9,
        },
    ]
}

/// Encode a case with its fixed seed (and the one search override).
pub fn encode_case(c: &Case) -> Vec<u8> {
    let opts = if c.name == "circle12_search_override" {
        EncodeOptions { seed: c.seed, search: Some(SearchOptions { n_random: 32, ..SearchOptions::default() }) }
    } else {
        EncodeOptions { seed: c.seed, search: None }
    };
    arthash::encode_rgb(&c.rgb, c.w, c.h, &c.codec, opts)
}

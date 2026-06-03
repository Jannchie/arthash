//! Decode + SVG OUTPUT regression lock.
//!
//! `shape_golden.rs` locks the ENCODE bytes for the shared `cases()` table.
//! This file locks the other half: what those same hashes DECODE to — the
//! rasterized RGBA buffer at a fixed `base_size`, and the rendered SVG string.
//!
//! Why it exists: the decode side has THREE coordinate-dequant implementations
//! (`decode_render`, the `decode_<shape>_at` helpers, and the inline SVG
//! element emitters). Before this lock, nothing caught a change in rendered
//! output — only the encode bytes were guarded. Any refactor that unifies or
//! touches those paths now has a byte-exact tripwire on both the raster and
//! SVG results.
//!
//! SAME-TOOLCHAIN caveat (as `shape_golden.rs`): decode itself is RNG-free,
//! but its input hash comes from `encode_case`, whose search trajectory uses
//! libm `ln` — so this lock inherits the same cross-target sensitivity.

mod common;
use common::{cases, digest, encode_case, Case};

use arthash::{decode, to_svg, DecodeOptions, SvgOptions};

/// Small fixed decode size keeps the digested buffer cheap while still
/// exercising the full per-shape rasterizer.
const DECODE_BASE: u32 = 32;
const SVG_BASE: u32 = 64;

fn decode_digest(c: &Case) -> String {
    let hash = encode_case(c);
    let out = decode(
        &hash,
        &c.codec,
        DecodeOptions { base_size: DECODE_BASE, ..Default::default() },
    );
    // Fold w/h into the digest so a dimension change is caught too.
    let mut buf = Vec::with_capacity(8 + out.rgba.len());
    buf.extend_from_slice(&out.width.to_le_bytes());
    buf.extend_from_slice(&out.height.to_le_bytes());
    buf.extend_from_slice(&out.rgba);
    digest(&buf)
}

fn svg_digest(c: &Case) -> String {
    let hash = encode_case(c);
    let svg = to_svg(
        &hash,
        &c.codec,
        SvgOptions { base_size: SVG_BASE, ..Default::default() },
    )
    .expect("all shape/PIXEL cases support SVG");
    digest(svg.as_bytes())
}

/// (case name, FNV-1a digest of `[w, h, rgba]` decoded at `DECODE_BASE`).
const DECODE_GOLDEN: &[(&str, &str)] = &[
    ("circle12_solid", "4df2641f33c5afc5"),
    ("circle12_gradient", "c0b2435044159e5b"),
    ("circle24_gradient", "c42bdc0775aff2be"),
    ("triangle12_gradient", "b14c304a7238c56d"),
    ("triangle12_solid", "1b297ad511b187c5"),
    ("square12_gradient", "ab8e33a3c1b88773"),
    ("rect12_gradient", "627c18d68e8b1ac2"),
    ("rotrect12_gradient", "c322fe6fd9da68bc"),
    ("pixel16_gradient", "374dcde1402e9aa6"),
    ("circle8_palette", "5da1deead26168bd"),
    ("triangle12_rgb888", "9da84a7b5ef7a138"),
    ("circle12_search_override", "f001d22b6c1a86eb"),
];

/// (case name, FNV-1a digest of the SVG string at `SVG_BASE`).
const SVG_GOLDEN: &[(&str, &str)] = &[
    ("circle12_solid", "46648868d1e0675f"),
    ("circle12_gradient", "4604da83bf258379"),
    ("circle24_gradient", "ddb0759a3199cd25"),
    ("triangle12_gradient", "8099c17810586ad7"),
    ("triangle12_solid", "16fe9bf53cbe98fb"),
    ("square12_gradient", "46f7c789ca5811ba"),
    ("rect12_gradient", "b14e262f60a6ef06"),
    ("rotrect12_gradient", "c0d1fc3a7b4ba330"),
    ("pixel16_gradient", "a1e2339bf347d245"),
    ("circle8_palette", "d2232eb7664cb7b6"),
    ("triangle12_rgb888", "f97dd97c4f882a50"),
    ("circle12_search_override", "57c3128f2876c2f5"),
];

#[test]
fn decode_render_golden() {
    if DECODE_GOLDEN.is_empty() {
        eprintln!("decode_golden: DECODE_GOLDEN empty; run `cargo test emit_decode_svg_golden -- --nocapture`");
        return;
    }
    for c in cases() {
        let got = decode_digest(&c);
        let want = DECODE_GOLDEN
            .iter()
            .find(|(n, _)| *n == c.name)
            .unwrap_or_else(|| panic!("no decode golden for {}: actual={}", c.name, got));
        assert_eq!(
            got, want.1,
            "decode render golden mismatch for {}\n  expected: {}\n  actual:   {}",
            c.name, want.1, got
        );
    }
}

#[test]
fn svg_output_golden() {
    if SVG_GOLDEN.is_empty() {
        eprintln!("decode_golden: SVG_GOLDEN empty; run `cargo test emit_decode_svg_golden -- --nocapture`");
        return;
    }
    for c in cases() {
        let got = svg_digest(&c);
        let want = SVG_GOLDEN
            .iter()
            .find(|(n, _)| *n == c.name)
            .unwrap_or_else(|| panic!("no svg golden for {}: actual={}", c.name, got));
        assert_eq!(
            got, want.1,
            "svg output golden mismatch for {}\n  expected: {}\n  actual:   {}",
            c.name, want.1, got
        );
    }
}

/// Run with `cargo test emit_decode_svg_golden -- --nocapture` to regenerate
/// both tables, then paste them above. Always passes; it only prints.
#[test]
fn emit_decode_svg_golden() {
    println!("\n----- BEGIN DECODE_GOLDEN -----");
    for c in cases() {
        println!("    (\"{}\", \"{}\"),", c.name, decode_digest(&c));
    }
    println!("----- END DECODE_GOLDEN -----");
    println!("----- BEGIN SVG_GOLDEN -----");
    for c in cases() {
        println!("    (\"{}\", \"{}\"),", c.name, svg_digest(&c));
    }
    println!("----- END SVG_GOLDEN -----\n");
}

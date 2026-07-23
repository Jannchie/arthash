//! Behavior locks for `DecodeOptions::dither` (ordered Bayer 8×8 dithering).
//!
//! Three invariants:
//!  1. `dither: true` moves each channel by AT MOST 1 LSB relative to the
//!     undithered output (ordered dithering only shifts the rounding
//!     threshold within one quantization step).
//!  2. Dithered output is deterministic (Bayer matrix, no RNG).
//!  3. Output that dithering does not apply to (sharp shape modes) is
//!     untouched. (`dither: false` byte-stability itself is locked by
//!     decode_golden.rs, which decodes with default options.)

mod common;

use arthash::{
    decode, encode_rgb, Codec, CodecConfig, DecodeOptions, EncodeOptions, RenderStyle, ShapeType,
};
use common::{gradient, pal8};

fn dct_hash() -> (Vec<u8>, Codec) {
    let codec = Codec::dct();
    let rgb = gradient(96, 64);
    let hash = encode_rgb(&rgb, 96, 64, &codec, EncodeOptions::default());
    (hash, codec)
}

#[test]
fn dct_dither_changes_output_but_stays_within_one_lsb() {
    let (hash, codec) = dct_hash();
    let plain = decode(&hash, &codec, DecodeOptions::default());
    let dithered = decode(
        &hash,
        &codec,
        DecodeOptions { dither: true, ..DecodeOptions::default() },
    );
    assert_eq!(plain.rgba.len(), dithered.rgba.len());

    let mut changed = 0usize;
    for (a, b) in plain.rgba.iter().zip(dithered.rgba.iter()) {
        let d = (*a as i16 - *b as i16).abs();
        assert!(d <= 1, "dither may shift a channel by at most 1 LSB, got {d}");
        if d != 0 {
            changed += 1;
        }
    }
    // A smooth DCT gradient must actually get dithered — many channels land
    // near a quantization boundary, so a healthy fraction should move.
    assert!(
        changed > plain.rgba.len() / 20,
        "expected >5% of channels to move, only {changed}/{}",
        plain.rgba.len()
    );
}

#[test]
fn dct_dither_is_deterministic() {
    let (hash, codec) = dct_hash();
    let opts = DecodeOptions { dither: true, ..DecodeOptions::default() };
    let a = decode(&hash, &codec, opts);
    let b = decode(&hash, &codec, opts);
    assert_eq!(a.rgba, b.rgba, "Bayer dithering must be reproducible");
}

#[test]
fn shape_blur_dither_stays_within_one_lsb() {
    // Blur re-creates smooth gradients on shape output; the dithered
    // write-back must obey the same ≤1 LSB envelope.
    let codec = Codec::triangle(12);
    let rgb = gradient(48, 48);
    let hash = encode_rgb(&rgb, 48, 48, &codec, EncodeOptions { seed: 1, search: None });
    let style = RenderStyle { blur: 4.0, corner_radius: 0.0 };
    let plain = decode(
        &hash,
        &codec,
        DecodeOptions { style, ..DecodeOptions::default() },
    );
    let dithered = decode(
        &hash,
        &codec,
        DecodeOptions { style, dither: true, ..DecodeOptions::default() },
    );
    let mut changed = 0usize;
    for (a, b) in plain.rgba.iter().zip(dithered.rgba.iter()) {
        let d = (*a as i16 - *b as i16).abs();
        assert!(d <= 1, "blur+dither may shift a channel by at most 1 LSB, got {d}");
        if d != 0 {
            changed += 1;
        }
    }
    assert!(changed > 0, "blur+dither should move at least some channels");
}

/// DCT codec carrying a render-time palette (only reachable via `Codec::Raw`
/// — the DCT byte format itself has no palette field). Returns the codec and
/// the palette's flat RGB bytes for output assertions. `pal8()`'s 8 colors
/// are well separated, so nearest-lookup is unambiguous.
fn dct_palette_codec() -> (Codec, Vec<u8>) {
    let bytes = pal8().as_bytes().to_vec();
    let codec = Codec::Raw(CodecConfig {
        shape: ShapeType::Dct,
        palette: Some(bytes.clone()),
        palette_k: Some(8),
        ..CodecConfig::default()
    });
    (codec, bytes)
}

fn assert_all_in_palette(rgba: &[u8], palette: &[u8], ctx: &str) {
    for px in rgba.chunks_exact(4) {
        assert!(
            palette.chunks_exact(3).any(|c| c == &px[..3]),
            "pixel {:?} ({ctx}) is not a palette color",
            &px[..3]
        );
    }
}

#[test]
fn dct_palette_output_only_contains_palette_colors() {
    let (hash, _) = dct_hash();
    let (codec, palette) = dct_palette_codec();
    for dither in [false, true] {
        let out = decode(&hash, &codec, DecodeOptions { dither, ..DecodeOptions::default() });
        assert_all_in_palette(&out.rgba, &palette, &format!("dither={dither}"));
    }
}

#[test]
fn dct_palette_dither_visibly_changes_output() {
    // Unlike the ±1 LSB anti-banding pass, palette dithering swaps whole
    // palette entries — a large fraction of pixels must change.
    let (hash, _) = dct_hash();
    let (codec, _) = dct_palette_codec();
    let hard = decode(&hash, &codec, DecodeOptions::default());
    let dithered = decode(
        &hash,
        &codec,
        DecodeOptions { dither: true, ..DecodeOptions::default() },
    );
    let n = (hard.width * hard.height) as usize;
    let mut changed = 0usize;
    for i in 0..n {
        if hard.rgba[i * 4..i * 4 + 3] != dithered.rgba[i * 4..i * 4 + 3] {
            changed += 1;
        }
    }
    assert!(
        changed > n / 20,
        "palette dither should visibly change >5% of pixels, got {changed}/{n}"
    );
}

#[test]
fn dct_palette_dither_scale_changes_dot_pitch() {
    // Explicit pitches must produce different patterns, and every output
    // pixel must still be a palette color regardless of pitch.
    let (hash, _) = dct_hash();
    let (codec, palette) = dct_palette_codec();
    let fine = decode(
        &hash,
        &codec,
        DecodeOptions { dither: true, dither_scale: 1, ..DecodeOptions::default() },
    );
    let chunky = decode(
        &hash,
        &codec,
        DecodeOptions { dither: true, dither_scale: 8, ..DecodeOptions::default() },
    );
    assert_ne!(fine.rgba, chunky.rgba, "dot pitch must change the pattern");
    assert_all_in_palette(&chunky.rgba, &palette, "scale 8");
}

#[test]
fn shape_without_blur_ignores_dither() {
    // Sharp shape output quantizes through the linear→sRGB LUT path, which
    // dither deliberately leaves alone — piecewise-flat regions would only
    // gain noise.
    let codec = Codec::triangle(12);
    let rgb = gradient(48, 48);
    let hash = encode_rgb(&rgb, 48, 48, &codec, EncodeOptions { seed: 1, search: None });
    let plain = decode(&hash, &codec, DecodeOptions::default());
    let dithered = decode(
        &hash,
        &codec,
        DecodeOptions { dither: true, ..DecodeOptions::default() },
    );
    assert_eq!(plain.rgba, dithered.rgba, "sharp shape output must not be dithered");
}

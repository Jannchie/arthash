//! Property / invariant / fuzz tests for the encoder + decoder.
//!
//! Unlike `tests/vectors.rs` (byte-exact DCT conformance) and
//! `tests/shape_golden.rs` (same-toolchain shape byte lock), these assert
//! *structural* invariants that must hold on EVERY target regardless of libm:
//!
//!   * determinism — same codec + input + seed ⇒ identical bytes;
//!   * length — shape/PIXEL hash length == `Codec::bytes_total()`;
//!   * robustness — encode→decode never panics and produces a self-consistent
//!     RGBA buffer across degenerate sizes (1×1, 1×N, N×1) and a spread of `n`
//!     (including non-power-of-two) via a deterministic PRNG (no external
//!     `proptest` dependency).

use arthash::{decode, encode_rgb, Codec, DecodeOptions, EncodeOptions};

/// Deterministic xorshift64* — avoids pulling in a dev-dependency just to get
/// reproducible pseudo-random pixels.
fn rand_rgb(w: u32, h: u32, seed: u64) -> Vec<u8> {
    let mut s = seed | 1;
    let n = (w * h * 3) as usize;
    let mut out = vec![0u8; n];
    for byte in out.iter_mut() {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        *byte = (s >> 33) as u8;
    }
    out
}

/// A representative spread of codecs: every shape family at a few `n`
/// (including 2, a non-power-of-two 13, and a large 64), plus DCT.
fn all_codecs() -> Vec<(String, Codec)> {
    let mut v = Vec::new();
    for n in [2u32, 12, 13, 64] {
        v.push((format!("circle{n}"), Codec::circle(n)));
        v.push((format!("triangle{n}"), Codec::triangle(n)));
        v.push((format!("square{n}"), Codec::square(n)));
        v.push((format!("rect{n}"), Codec::rect(n)));
        v.push((format!("rotrect{n}"), Codec::rotated_rect(n)));
        v.push((format!("pixel{n}"), Codec::pixel(n.max(4))));
    }
    v.push(("dct".to_string(), Codec::dct()));
    v
}

#[test]
fn encode_is_deterministic() {
    let rgb = rand_rgb(48, 48, 0xABCD_1234);
    for (name, codec) in all_codecs() {
        let opts = EncodeOptions { seed: 123, search: None };
        let a = encode_rgb(&rgb, 48, 48, &codec, opts);
        let b = encode_rgb(&rgb, 48, 48, &codec, opts);
        assert_eq!(a, b, "encode not deterministic for {name}");
    }
}

#[test]
fn shape_hash_len_matches_bytes_total() {
    let rgb = rand_rgb(48, 48, 0x5151_7777);
    for (name, codec) in all_codecs() {
        // DCT's `bytes_total` is a fixed-format upper estimate computed
        // separately from the actual frequency-domain serializer; only the
        // shape/PIXEL family has the exact `header + n·per_shape` relation.
        if matches!(codec, Codec::Dct) {
            continue;
        }
        let hash = encode_rgb(&rgb, 48, 48, &codec, EncodeOptions::default());
        assert_eq!(
            hash.len(),
            codec.bytes_total(),
            "hash length != bytes_total() for {name}"
        );
    }
}

#[test]
fn fuzz_encode_decode_no_panic() {
    // Degenerate and odd sizes — the encoder is documented to accept raw
    // buffers at the caller's resolution, so it must survive 1×1 / strips /
    // non-square without panicking.
    let sizes: &[(u32, u32)] = &[(1, 1), (1, 7), (7, 1), (2, 3), (3, 2), (16, 16), (48, 29), (64, 64)];
    for (name, codec) in all_codecs() {
        for &(w, h) in sizes {
            for seed in [1u64, 0x9E37_79B9] {
                let rgb = rand_rgb(w, h, seed ^ ((w as u64) << 20) ^ (h as u64));
                let hash = encode_rgb(&rgb, w, h, &codec, EncodeOptions { seed, search: None });
                assert!(!hash.is_empty(), "empty hash for {name} at {w}x{h}");
                let out = decode(
                    &hash,
                    &codec,
                    DecodeOptions { base_size: 96, ..DecodeOptions::default() },
                );
                assert_eq!(
                    out.rgba.len(),
                    (out.width * out.height * 4) as usize,
                    "rgba length inconsistent for {name} at {w}x{h}"
                );
                assert!(out.width > 0 && out.height > 0, "zero dim for {name} at {w}x{h}");
            }
        }
    }
}

#[test]
fn decode_truncated_hash_does_not_panic() {
    // BitReader is documented to zero-fill past the buffer end; decoding a
    // deliberately truncated hash must degrade gracefully, never panic.
    let rgb = rand_rgb(48, 48, 0xDEAD_BEEF);
    for (name, codec) in all_codecs() {
        let hash = encode_rgb(&rgb, 48, 48, &codec, EncodeOptions::default());
        for cut in [0usize, 1, hash.len() / 2] {
            let truncated = &hash[..cut.min(hash.len())];
            let out = decode(truncated, &codec, DecodeOptions::default());
            assert_eq!(out.rgba.len(), (out.width * out.height * 4) as usize, "{name} cut={cut}");
        }
    }
}

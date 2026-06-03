//! Micro-benchmark for arthash-rs. Measures encode + decode for the four
//! modes on a synthetic gradient input. Run with:
//!
//!     cargo run --release --example bench
//!
//! Output is one JSON object per line (NDJSON) on stdout — easy to diff
//! against the Python reference in scripts/bench_py.py.
//!
//! Methodology:
//!   * Inputs: deterministic synthetic gradient at the mode's natural
//!     thumbnail size (DCT at 100x100, shape modes at 48x48).
//!   * Each measurement does N warmup iters + M timed iters; we report
//!     median + p95 in microseconds, plus throughput.
//!   * We measure end-to-end `encode_rgb` / `decode` — i.e. the same
//!     surface a PyO3 / napi-rs binding would expose.

// Bench scratch: inline `(label, codec-factory, size)` tuple tables — clarity
// over lint compliance here.
#![allow(clippy::type_complexity)]

use std::time::Instant;

use arthash::{decode, encode_rgb, Codec, DecodeOptions, EncodeOptions};

fn gradient_rgb(w: u32, h: u32) -> Vec<u8> {
    let n = (w * h) as usize;
    let mut rgb = vec![0u8; n * 3];
    for y in 0..h {
        for x in 0..w {
            let p = ((y * w + x) * 3) as usize;
            rgb[p] = ((x as f32) * 255.0 / ((w - 1).max(1) as f32)).round() as u8;
            rgb[p + 1] = ((y as f32) * 255.0 / ((h - 1).max(1) as f32)).round() as u8;
            rgb[p + 2] = (((x + y) as f32) * 0.3) as u8;
        }
    }
    rgb
}

#[derive(Clone, Copy)]
struct Stats {
    median_us: f64,
    p95_us: f64,
    min_us: f64,
    iters: usize,
}

fn measure<F: FnMut()>(mut f: F, warmup: usize, iters: usize) -> Stats {
    for _ in 0..warmup {
        f();
    }
    // Batch within each sample so per-call `Instant::now()` overhead — which
    // on Windows is ~100 ns and was inflating sub-millisecond medians by up
    // to ~30 % — averages out. Matches the JS / PyO3 harnesses.
    let batch = if iters >= 50 { 50 } else { 1 };
    let mut samples = Vec::with_capacity(iters);
    for _ in 0..iters {
        let t0 = Instant::now();
        for _ in 0..batch {
            f();
        }
        let dt = t0.elapsed().as_secs_f64() * 1e6 / (batch as f64);
        samples.push(dt);
    }
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = samples[samples.len() / 2];
    let p95 = samples[(samples.len() as f64 * 0.95) as usize];
    let min = samples[0];
    Stats {
        median_us: median,
        p95_us: p95,
        min_us: min,
        iters,
    }
}

fn report(mode: &str, op: &str, w: u32, h: u32, s: Stats, extra: &str) {
    let pixels = (w * h) as f64;
    let mpix_s = pixels / s.median_us; // pixels/us == megapixels/s
    println!(
        "{{\"impl\":\"rust\",\"mode\":\"{}\",\"op\":\"{}\",\"w\":{},\"h\":{},\"median_us\":{:.2},\"p95_us\":{:.2},\"min_us\":{:.2},\"iters\":{},\"mpix_per_s\":{:.3}{}}}",
        mode, op, w, h, s.median_us, s.p95_us, s.min_us, s.iters, mpix_s, extra
    );
}

fn main() {
    // DCT — input is 100x100 (DCT default target).
    {
        let (w, h) = (100u32, 100u32);
        let rgb = gradient_rgb(w, h);
        let codec = Codec::dct();
        let enc_opts = EncodeOptions::default();

        let mut hash: Vec<u8> = Vec::new();
        let s = measure(
            || {
                hash = encode_rgb(&rgb, w, h, &codec, enc_opts);
            },
            30,
            200,
        );
        let extra = format!(",\"hash_bytes\":{}", hash.len());
        report("dct", "encode", w, h, s, &extra);

        let dec_opts = DecodeOptions {
            base_size: 256,
            ..Default::default()
        };
        let s = measure(
            || {
                let _ = decode(&hash, &codec, dec_opts);
            },
            10,
            50,
        );
        report("dct", "decode", w, h, s, "");
    }

    // Shape modes — 48x48 thumbnails.
    let shapes: [(&str, fn(u32) -> Codec, u32); 6] = [
        ("circle", Codec::circle, 12),
        ("triangle", Codec::triangle, 12),
        ("square", Codec::square, 12),
        ("rect", Codec::rect, 12),
        ("rotrect", Codec::rotated_rect, 12),
        ("pixel", Codec::pixel, 12),
    ];
    for (name, make, n_shapes) in shapes {
        let (w, h) = (48u32, 48u32);
        let rgb = gradient_rgb(w, h);
        let codec = make(n_shapes);
        let enc_opts = EncodeOptions::default();

        let (warmup, iters) = if name == "pixel" {
            (10, 100)
        } else {
            (10, 60)
        };

        let mut hash: Vec<u8> = Vec::new();
        let s = measure(
            || {
                hash = encode_rgb(&rgb, w, h, &codec, enc_opts);
            },
            warmup,
            iters,
        );
        let extra = format!(",\"hash_bytes\":{}", hash.len());
        report(name, "encode", w, h, s, &extra);

        let dec_opts = DecodeOptions {
            base_size: 256,
            ..Default::default()
        };
        let s = measure(
            || {
                let _ = decode(&hash, &codec, dec_opts);
            },
            5,
            30,
        );
        report(name, "decode", w, h, s, "");
    }
}

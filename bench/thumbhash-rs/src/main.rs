//! Benchmark the official thumbhash Rust crate (evanw/thumbhash@0.1.0) on
//! the same 100x100 gradient as the other benches. Output: NDJSON to stdout.

use std::time::Instant;

fn gradient(w: usize, h: usize) -> Vec<u8> {
    let mut rgba = vec![0u8; w * h * 4];
    for y in 0..h {
        for x in 0..w {
            let p = (y * w + x) * 4;
            rgba[p] = ((x as f32) * 255.0 / (w.saturating_sub(1).max(1) as f32)).round() as u8;
            rgba[p + 1] = ((y as f32) * 255.0 / (h.saturating_sub(1).max(1) as f32)).round() as u8;
            rgba[p + 2] = ((x + y) as f32 * 0.3).min(255.0) as u8;
            rgba[p + 3] = 255;
        }
    }
    rgba
}

fn measure<F: FnMut()>(mut f: F, warmup: usize, iters: usize) -> (f64, f64, f64) {
    for _ in 0..warmup {
        f();
    }
    let mut samples = Vec::with_capacity(iters);
    for _ in 0..iters {
        let t0 = Instant::now();
        f();
        samples.push(t0.elapsed().as_secs_f64() * 1e6);
    }
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let med = samples[samples.len() / 2];
    let p95 = samples[(samples.len() as f64 * 0.95) as usize];
    let min = samples[0];
    (med, p95, min)
}

fn report(mode: &str, op: &str, w: usize, h: usize, s: (f64, f64, f64), iters: usize, extra: &str) {
    let mpix = (w * h) as f64 / s.0;
    println!(
        "{{\"impl\":\"rust-thumbhash\",\"mode\":\"{}\",\"op\":\"{}\",\"w\":{},\"h\":{},\"median_us\":{:.2},\"p95_us\":{:.2},\"min_us\":{:.2},\"iters\":{},\"mpix_per_s\":{:.3}{}}}",
        mode, op, w, h, s.0, s.1, s.2, iters, mpix, extra
    );
}

fn main() {
    let w = 100usize;
    let h = 100usize;
    let rgba = gradient(w, h);
    let mut hash: Vec<u8> = Vec::new();
    let s = measure(
        || {
            hash = thumbhash::rgba_to_thumb_hash(w, h, &rgba);
        },
        30,
        200,
    );
    let extra = format!(",\"hash_bytes\":{}", hash.len());
    report("dct", "encode", w, h, s, 200, &extra);

    let s = measure(
        || {
            let _ = thumbhash::thumb_hash_to_rgba(&hash).unwrap();
        },
        10,
        50,
    );
    report("dct", "decode_default", w, h, s, 50, "");
}

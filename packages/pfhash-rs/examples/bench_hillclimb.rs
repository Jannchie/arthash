//! Targeted micro-benchmark for the CIRCLE / TRIANGLE hill-climb hot path.
//!
//! Two-step workflow:
//!
//!   # Step 1 — accurate timing (no instrumentation overhead).
//!   cargo run --release --example bench_hillclimb -- \
//!       --label=baseline --out=bench/hillclimb-baseline.ndjson
//!
//!   # Step 2 — eval/pixel counts (counters perturb timing; ignore *_us here).
//!   cargo run --release --features bench-counters \
//!       --example bench_hillclimb -- \
//!       --label=baseline-counts --out=bench/hillclimb-baseline-counts.ndjson
//!
//! Each line of output is one NDJSON record. A downstream script joins
//! the timing and counts rows by `(image, shape, codec)` for the ablation
//! table.
//!
//! Three test images are exercised on every (shape, codec) combination:
//!   * `gradient` — smooth ramp (best case: large connected regions).
//!   * `quadrants` — 4 hard-edged color blocks (lots of useful shapes).
//!   * `noise`     — deterministic PRNG (adversarial: no structure).
//!
//! All shape budgets use the SDK's mode-specific defaults
//! (`SearchOptions::default()` for circle, `triangle_default()` for triangle).

use std::time::Instant;

use pfhash::shape::raster::counters;
use pfhash::shape::SearchOptions;
use pfhash::{encode_rgb, Codec, EncodeOptions, ShapeType};

const W: u32 = 48;
const H: u32 = 48;
const N_SHAPES: u32 = 12;
const WARMUP: usize = 5;
const ITERS: usize = 60;

fn gradient_rgb(w: u32, h: u32) -> Vec<u8> {
    let n = (w * h) as usize;
    let mut rgb = vec![0u8; n * 3];
    for y in 0..h {
        for x in 0..w {
            let p = ((y * w + x) * 3) as usize;
            rgb[p] = ((x as f32) * 255.0 / ((w - 1).max(1) as f32)).round() as u8;
            rgb[p + 1] = ((y as f32) * 255.0 / ((h - 1).max(1) as f32)).round() as u8;
            rgb[p + 2] = 64;
        }
    }
    rgb
}

/// Four solid color quadrants — sharp edges, easy for shape fitting.
fn quadrants_rgb(w: u32, h: u32) -> Vec<u8> {
    let n = (w * h) as usize;
    let mut rgb = vec![0u8; n * 3];
    let colors: [[u8; 3]; 4] = [
        [220, 60, 60],
        [60, 200, 90],
        [70, 100, 230],
        [240, 200, 80],
    ];
    for y in 0..h {
        for x in 0..w {
            let qx = if x < w / 2 { 0 } else { 1 };
            let qy = if y < h / 2 { 0 } else { 1 };
            let c = colors[qy * 2 + qx];
            let p = ((y * w + x) * 3) as usize;
            rgb[p] = c[0];
            rgb[p + 1] = c[1];
            rgb[p + 2] = c[2];
        }
    }
    rgb
}

/// Deterministic xorshift "noise" — no exploitable structure.
fn noise_rgb(w: u32, h: u32) -> Vec<u8> {
    let n = (w * h) as usize;
    let mut rgb = vec![0u8; n * 3];
    let mut state: u64 = 0xC0FFEE_F00D_BEEFu64;
    for slot in rgb.iter_mut() {
        // xorshift64
        let mut x = state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        state = x;
        *slot = (x & 0xFF) as u8;
    }
    rgb
}

#[derive(Clone, Copy)]
struct TimedRun {
    median_us: f64,
    p95_us: f64,
    min_us: f64,
}

fn search_for(shape: ShapeType) -> SearchOptions {
    match shape {
        ShapeType::Triangle => SearchOptions::triangle_default(),
        _ => SearchOptions::default(),
    }
}

fn timed(rgb: &[u8], w: u32, h: u32, codec: &Codec) -> TimedRun {
    let opts = EncodeOptions { seed: 0, search: None };
    for _ in 0..WARMUP {
        let h = encode_rgb(rgb, w, h, codec, opts);
        std::hint::black_box(h);
    }
    let mut samples = Vec::with_capacity(ITERS);
    for _ in 0..ITERS {
        let t0 = Instant::now();
        let bytes = encode_rgb(rgb, w, h, codec, opts);
        let dt = t0.elapsed().as_secs_f64() * 1e6;
        std::hint::black_box(bytes);
        samples.push(dt);
    }
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = samples[samples.len() / 2];
    let p95 = samples[(samples.len() as f64 * 0.95) as usize];
    let min = samples[0];
    TimedRun { median_us: median, p95_us: p95, min_us: min }
}

fn parse_args() -> (String, Option<String>) {
    let mut label = String::from("run");
    let mut out: Option<String> = None;
    for arg in std::env::args().skip(1) {
        if let Some(v) = arg.strip_prefix("--label=") {
            label = v.to_string();
        } else if let Some(v) = arg.strip_prefix("--out=") {
            out = Some(v.to_string());
        }
    }
    (label, out)
}

fn main() {
    let (label, out_path) = parse_args();
    let counters_enabled = cfg!(feature = "bench-counters");

    let images: Vec<(&str, Vec<u8>)> = vec![
        ("gradient", gradient_rgb(W, H)),
        ("quadrants", quadrants_rgb(W, H)),
        ("noise", noise_rgb(W, H)),
    ];
    let shapes: Vec<(&str, ShapeType)> = vec![
        ("circle", ShapeType::Circle),
        ("triangle", ShapeType::Triangle),
    ];

    let mut lines: Vec<String> = Vec::new();
    for (img_name, rgb) in &images {
        for (shape_name, shape) in &shapes {
            let codec = Codec { shape: *shape, n_shapes: N_SHAPES, ..Codec::default() };
            let _search = search_for(*shape); // documents intent; SDK uses same default

            // Eval-count snapshot (deterministic given seed). Done first so
            // counter state is clean.
            counters::reset();
            let hash = encode_rgb(rgb, W, H, &codec, EncodeOptions::default());
            let snap = counters::snapshot();

            let t = timed(rgb, W, H, &codec);

            let total_eval = snap.eval_circle + snap.eval_triangle;
            let avg_px = if total_eval > 0 {
                snap.pixels_touched as f64 / total_eval as f64
            } else {
                0.0
            };
            let mpix_per_s_eq = if t.median_us > 0.0 {
                (W * H) as f64 / t.median_us
            } else {
                0.0
            };

            let line = format!(
                "{{\"label\":\"{label}\",\"counters\":{counters_enabled},\"image\":\"{img}\",\"shape\":\"{sh}\",\"w\":{W},\"h\":{H},\"n_shapes\":{ns},\"median_us\":{m:.1},\"p95_us\":{p:.1},\"min_us\":{mn:.1},\"mpix_per_s\":{mp:.3},\"eval_circle\":{ec},\"eval_triangle\":{et},\"eval_total\":{tot},\"pixels_touched\":{px},\"avg_pixels_per_eval\":{ap:.1},\"hash_bytes\":{hb},\"hash_hex\":\"{hh}\"}}",
                label = label,
                counters_enabled = counters_enabled,
                img = img_name,
                sh = shape_name,
                W = W,
                H = H,
                ns = N_SHAPES,
                m = t.median_us,
                p = t.p95_us,
                mn = t.min_us,
                mp = mpix_per_s_eq,
                ec = snap.eval_circle,
                et = snap.eval_triangle,
                tot = total_eval,
                px = snap.pixels_touched,
                ap = avg_px,
                hb = hash.len(),
                hh = hex_encode(&hash),
            );
            println!("{}", line);
            lines.push(line);
        }
    }

    if let Some(path) = out_path {
        if let Some(parent) = std::path::Path::new(&path).parent() {
            if !parent.as_os_str().is_empty() {
                let _ = std::fs::create_dir_all(parent);
            }
        }
        std::fs::write(&path, lines.join("\n") + "\n").expect("write out");
        eprintln!("wrote {}", path);
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

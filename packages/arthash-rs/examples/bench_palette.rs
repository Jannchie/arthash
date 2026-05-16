//! Compare encode time across continuous-color and palette modes on the
//! same hill-climb-heavy shape. Confirms the palette-mode `finalize` fast
//! path actually beats the old O(K) scan.
//!
//! Run with:
//!     cargo run --release --example bench_palette

use std::time::Instant;

use arthash::{encode_rgb, Codec, EncodeOptions, ShapeType};

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

fn rgb_cube(rb: u32, gb: u32, bb: u32) -> Vec<u8> {
    let (rn, gn, bn) = (1u32 << rb, 1u32 << gb, 1u32 << bb);
    let mut out = Vec::with_capacity((rn * gn * bn * 3) as usize);
    let lv = |i: u32, n: u32| -> u8 {
        if n == 1 { 128 } else { ((i as f32 / (n - 1) as f32) * 255.0).round() as u8 }
    };
    for r in 0..rn {
        for g in 0..gn {
            for b in 0..bn {
                out.push(lv(r, rn));
                out.push(lv(g, gn));
                out.push(lv(b, bn));
            }
        }
    }
    out
}

fn time_encode(rgb: &[u8], w: u32, h: u32, codec: &Codec, iters: usize) -> f64 {
    // Warmup.
    for _ in 0..3 {
        encode_rgb(rgb, w, h, codec, EncodeOptions::default());
    }
    let mut samples: Vec<f64> = (0..iters)
        .map(|_| {
            let t0 = Instant::now();
            encode_rgb(rgb, w, h, codec, EncodeOptions::default());
            t0.elapsed().as_secs_f64() * 1000.0
        })
        .collect();
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    samples[samples.len() / 2]
}

fn main() {
    let (w, h) = (48u32, 48u32);
    let rgb = gradient_rgb(w, h);
    let iters = 11;

    let shapes = [
        (ShapeType::Circle, "circle"),
        (ShapeType::Triangle, "triangle"),
        (ShapeType::Pixel, "pixel"),
    ];

    println!("shape         color-mode          median_ms");
    println!("------------- ------------------- ---------");

    for (shape, name) in shapes {
        // Continuous color baselines.
        for (bits, label) in [(16u32, "RGB-565"), (24, "RGB-888")] {
            let codec = Codec {
                shape,
                n_shapes: 12,
                color_bits: bits,
                ..Codec::default()
            };
            let ms = time_encode(&rgb, w, h, &codec, iters);
            println!("{:13} {:19} {:9.2}", name, label, ms);
        }
        // Palette modes (auto-N).
        for (rb, gb, bb, label) in [
            (1u32, 0, 0, "auto-1 (K=2)"),
            (2, 1, 1, "auto-4 (K=16)"),
            (3, 3, 2, "auto-8 (K=256)"),
        ] {
            let palette = rgb_cube(rb, gb, bb);
            let codec = Codec {
                shape,
                n_shapes: 12,
                palette: Some(palette),
                ..Codec::default()
            };
            let ms = time_encode(&rgb, w, h, &codec, iters);
            println!("{:13} {:19} {:9.2}", name, label, ms);
        }
        println!();
    }
}

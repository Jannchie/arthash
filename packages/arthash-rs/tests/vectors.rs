//! Conformance: replay `docs/test-vectors/vectors.json` against the Rust SDK.
//!
//! Scope:
//!  * DCT vectors → byte-exact match (no RNG, no resize for inputs ≤ target_size).
//!  * PIXEL/SHAPE vectors → not asserted (PIL LANCZOS resize + numpy PCG64 RNG
//!    are not byte-portable across stacks). Round-trip tests cover those modes.

use std::path::PathBuf;

use arthash::{
    decode, encode_rgb, Codec, CodecConfig, DecodeOptions, EncodeOptions, ShapeType,
};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn vectors_path() -> PathBuf {
    repo_root().join("docs").join("test-vectors").join("vectors.json")
}

fn build_input(spec: &serde_json::Value) -> (u32, u32, Vec<u8>) {
    let kind = spec["kind"].as_str().unwrap();
    let h = spec["h"].as_u64().unwrap() as u32;
    let w = spec["w"].as_u64().unwrap() as u32;
    let n = (h * w) as usize;
    let mut rgb = vec![0u8; n * 3];
    match kind {
        "solid" => {
            let rgb_arr = &spec["rgb"].as_array().unwrap();
            let r = rgb_arr[0].as_u64().unwrap() as u8;
            let g = rgb_arr[1].as_u64().unwrap() as u8;
            let b = rgb_arr[2].as_u64().unwrap() as u8;
            for i in 0..n {
                rgb[i * 3] = r;
                rgb[i * 3 + 1] = g;
                rgb[i * 3 + 2] = b;
            }
        }
        "gradient" => {
            for y in 0..h {
                for x in 0..w {
                    let p = ((y * w + x) * 3) as usize;
                    rgb[p] = ((x as f32) * 255.0 / ((w - 1) as f32)).round() as u8;
                    rgb[p + 1] = ((y as f32) * 255.0 / ((h - 1) as f32)).round() as u8;
                    rgb[p + 2] = 64;
                }
            }
        }
        "random" => {
            // numpy PCG64-DXSM sequence is not reproduced here; tests that
            // use this input are skipped for byte-equality assertions.
            for (i, byte) in rgb.iter_mut().enumerate().take(n * 3) {
                *byte = (i as u8).wrapping_mul(137);
            }
        }
        _ => panic!("unknown input kind: {}", kind),
    }
    (w, h, rgb)
}

fn build_codec(spec: &serde_json::Value) -> Codec {
    let shape = ShapeType::from_str(spec["shape"].as_str().unwrap()).unwrap();
    let mut cfg = CodecConfig {
        shape,
        n_shapes: spec["n_shapes"].as_u64().unwrap() as u32,
        cx_bits: spec["cx_bits"].as_u64().unwrap() as u32,
        cy_bits: spec["cy_bits"].as_u64().unwrap() as u32,
        r_bits: spec["r_bits"].as_u64().unwrap() as u32,
        alpha_bits: spec["alpha_bits"].as_u64().unwrap() as u32,
        color_bits: spec["color_bits"].as_u64().unwrap() as u32,
        theta_bits: spec
            .get("theta_bits")
            .and_then(|v| v.as_u64())
            .unwrap_or(5) as u32,
        palette: None,
        palette_k: None,
        alpha_levels: None,
        grid_aspect: None,
    };
    if let Some(pal_hex) = spec.get("palette_hex").and_then(|v| v.as_array()) {
        let mut pal = Vec::with_capacity(pal_hex.len() * 3);
        for h in pal_hex {
            let s = h.as_str().unwrap();
            pal.push(u8::from_str_radix(&s[0..2], 16).unwrap());
            pal.push(u8::from_str_radix(&s[2..4], 16).unwrap());
            pal.push(u8::from_str_radix(&s[4..6], 16).unwrap());
        }
        cfg.palette = Some(pal);
        if let Some(k) = spec.get("palette_k").and_then(|v| v.as_u64()) {
            cfg.palette_k = Some(k as usize);
        }
    }
    if let Some(alphas) = spec.get("alpha_levels").and_then(|v| v.as_array()) {
        cfg.alpha_levels = Some(alphas.iter().map(|a| a.as_f64().unwrap() as f32).collect());
    }
    if let Some(ga) = spec.get("grid_aspect").and_then(|v| v.as_f64()) {
        cfg.grid_aspect = Some(ga as f32);
    }
    Codec::Raw(cfg)
}

fn load_vectors() -> Vec<serde_json::Value> {
    let path = vectors_path();
    if !path.exists() {
        return Vec::new();
    }
    let text = std::fs::read_to_string(&path).unwrap();
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    v["vectors"].as_array().cloned().unwrap_or_default()
}

#[test]
fn dct_vectors_byte_exact() {
    let mut tested = 0;
    for vec in load_vectors() {
        let codec_spec = &vec["codec"];
        if codec_spec["shape"].as_str() != Some("dct") {
            continue;
        }
        // DCT vectors that need RNG-based input (random kind) cannot be
        // reproduced without numpy's PCG64. Skip those for byte equality
        // (currently none in the vectors file, but keep the guard).
        let name = vec["name"].as_str().unwrap();
        let input = &vec["input"];
        if input["kind"].as_str() == Some("random") {
            // Random DCT inputs depend on the byte-exact pixel sequence,
            // which the Rust test harness fakes. Skip.
            continue;
        }
        let (w, h, rgb) = build_input(input);
        let codec = build_codec(codec_spec);
        let actual = encode_rgb(&rgb, w, h, &codec, EncodeOptions::default());
        let expected_hex = vec["expected_hex"].as_str().unwrap();
        let expected: Vec<u8> = (0..expected_hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&expected_hex[i..i + 2], 16).unwrap())
            .collect();
        assert_eq!(
            actual, expected,
            "vector {} mismatch\n  expected: {}\n  actual:   {}",
            name,
            expected_hex,
            hex_encode(&actual)
        );
        tested += 1;
    }
    assert!(tested > 0, "no DCT vectors exercised");
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

#[test]
fn dct_roundtrip_solid_red_100x100() {
    let n = 100 * 100;
    let mut rgb = vec![0u8; n * 3];
    for i in 0..n {
        rgb[i * 3] = 255;
    }
    let codec = Codec::dct();
    let hash = encode_rgb(&rgb, 100, 100, &codec, EncodeOptions::default());
    assert!(!hash.is_empty());
    let opts = DecodeOptions { base_size: 64, ..DecodeOptions::default() };
    let out = decode(&hash, &codec, opts);
    assert!(out.width > 0 && out.height > 0);
    assert_eq!(out.rgba.len(), (out.width * out.height * 4) as usize);
    // Most pixels should be reddish.
    let mut red_count = 0u32;
    for i in 0..(out.width * out.height) as usize {
        if out.rgba[i * 4] > 200 && out.rgba[i * 4 + 1] < 80 {
            red_count += 1;
        }
    }
    let total = out.width * out.height;
    assert!(red_count * 2 > total, "expected mostly red output");
}

#[test]
fn pixel_roundtrip_gradient() {
    let (w, h) = (48u32, 29u32);
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
    let codec = Codec::pixel(12);
    let hash = encode_rgb(&rgb, w, h, &codec, EncodeOptions::default());
    let opts = DecodeOptions { base_size: 128, ..DecodeOptions::default() };
    let out = decode(&hash, &codec, opts);
    let (out_w, out_h, rgba) = (out.width, out.height, out.rgba);
    assert_eq!(rgba.len(), (out_w * out_h * 4) as usize);
    // Sanity: red should increase left→right.
    let mid_y = out_h / 2;
    let left = rgba[((mid_y * out_w) * 4) as usize] as i32;
    let right = rgba[(((mid_y + 1) * out_w - 1) * 4) as usize] as i32;
    assert!(right > left + 20, "gradient lost: left={}, right={}", left, right);
}

#[test]
fn circle_roundtrip_solid() {
    let (w, h) = (48u32, 48u32);
    let n = (w * h) as usize;
    let mut rgb = vec![0u8; n * 3];
    for i in 0..n {
        rgb[i * 3] = 200;
        rgb[i * 3 + 1] = 100;
        rgb[i * 3 + 2] = 50;
    }
    let codec = Codec::circle(6);
    let hash = encode_rgb(&rgb, w, h, &codec, EncodeOptions::default());
    let opts = DecodeOptions { base_size: 128, ..DecodeOptions::default() };
    let out = decode(&hash, &codec, opts);
    let (out_w, out_h, rgba) = (out.width, out.height, out.rgba);
    assert_eq!(rgba.len(), (out_w * out_h * 4) as usize);
    // Check center pixel — should be the dominant color.
    let mid = (((out_h / 2) * out_w + out_w / 2) * 4) as usize;
    assert!(rgba[mid] > 150, "expected reddish center, got {}", rgba[mid]);
}

#[test]
fn rect_roundtrip_gradient() {
    let (w, h) = (48u32, 48u32);
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
    let codec = Codec::rect(6);
    let hash = encode_rgb(&rgb, w, h, &codec, EncodeOptions::default());
    assert!(!hash.is_empty());
    let opts = DecodeOptions { base_size: 128, ..DecodeOptions::default() };
    let out = decode(&hash, &codec, opts);
    let (out_w, out_h, rgba) = (out.width, out.height, out.rgba);
    assert_eq!(rgba.len(), (out_w * out_h * 4) as usize);
    // Gradient should still go left→right red-ish.
    let mid_y = out_h / 2;
    let left = rgba[((mid_y * out_w) * 4) as usize] as i32;
    let right = rgba[(((mid_y + 1) * out_w - 1) * 4) as usize] as i32;
    assert!(right > left, "gradient direction lost: left={left}, right={right}");
}

#[test]
fn square_roundtrip_solid() {
    let (w, h) = (48u32, 48u32);
    let n = (w * h) as usize;
    let mut rgb = vec![0u8; n * 3];
    for i in 0..n {
        rgb[i * 3] = 200;
        rgb[i * 3 + 1] = 100;
        rgb[i * 3 + 2] = 50;
    }
    let codec = Codec::square(6);
    let hash = encode_rgb(&rgb, w, h, &codec, EncodeOptions::default());
    assert!(!hash.is_empty());
    let opts = DecodeOptions { base_size: 128, ..DecodeOptions::default() };
    let out = decode(&hash, &codec, opts);
    let (out_w, out_h, rgba) = (out.width, out.height, out.rgba);
    assert_eq!(rgba.len(), (out_w * out_h * 4) as usize);
    let mid = (((out_h / 2) * out_w + out_w / 2) * 4) as usize;
    assert!(rgba[mid] > 150, "expected reddish center, got {}", rgba[mid]);
}

#[test]
fn rotrect_roundtrip_gradient() {
    let (w, h) = (48u32, 48u32);
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
    let codec = Codec::rotated_rect(6);
    let hash = encode_rgb(&rgb, w, h, &codec, EncodeOptions::default());
    assert!(!hash.is_empty());
    let opts = DecodeOptions { base_size: 128, ..DecodeOptions::default() };
    let out = decode(&hash, &codec, opts);
    assert_eq!(out.rgba.len(), (out.width * out.height * 4) as usize);
}

#[test]
fn triangle_roundtrip_gradient() {
    let (w, h) = (48u32, 29u32);
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
    let codec = Codec::triangle(6);
    let hash = encode_rgb(&rgb, w, h, &codec, EncodeOptions::default());
    assert!(!hash.is_empty());
    let opts = DecodeOptions { base_size: 128, ..DecodeOptions::default() };
    let out = decode(&hash, &codec, opts);
    let (out_w, out_h, rgba) = (out.width, out.height, out.rgba);
    assert_eq!(rgba.len(), (out_w * out_h * 4) as usize);
}

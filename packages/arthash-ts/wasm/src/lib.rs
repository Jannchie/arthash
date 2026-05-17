//! wasm-bindgen wrapper around `arthash` (Rust core). Exposes the same set
//! of primitives the PyO3 binding exposes — `encodeRgb`, `encodeRgba`,
//! `decode`, `toSvg`.
//!
//! Codec config crosses the FFI boundary as a plain JS object that maps to
//! the SPEC field layout (`{shape, n_shapes, palette, …}`). The Rust side
//! converts it into the internal `CodecConfig` and wraps in `Codec::Raw`;
//! the JS-facing TypeScript SDK exposes a friendlier discriminated-union API
//! on top.

use arthash::{
    decode as rs_decode, encode_rgb as rs_encode_rgb, encode_rgba as rs_encode_rgba,
    to_svg as rs_to_svg, Codec, CodecConfig, DecodeOptions, EncodeOptions, PixelSmooth,
    SearchOptions, ShapeType, Strategy, SvgError, SvgOptions,
};
use serde::Deserialize;
use wasm_bindgen::prelude::*;

#[derive(Deserialize)]
#[serde(default)]
#[serde(rename_all = "snake_case")]
struct CodecOptions {
    shape: String,
    n_shapes: u32,
    cx_bits: u32,
    cy_bits: u32,
    r_bits: u32,
    alpha_bits: u32,
    color_bits: u32,
    theta_bits: u32,
    palette: Option<Vec<u8>>,
    palette_k: Option<usize>,
    grid_aspect: Option<f32>,
}

impl Default for CodecOptions {
    fn default() -> Self {
        Self {
            shape: "dct".to_string(),
            n_shapes: 12,
            cx_bits: 5,
            cy_bits: 5,
            r_bits: 4,
            alpha_bits: 3,
            color_bits: 16,
            theta_bits: 5,
            palette: None,
            palette_k: None,
            grid_aspect: None,
        }
    }
}

impl CodecOptions {
    fn into_codec(self) -> Result<Codec, JsValue> {
        let shape = ShapeType::from_str(&self.shape)
            .ok_or_else(|| JsError::new(&format!("unknown shape: {}", self.shape)))?;
        let palette = self.palette.filter(|p| !p.is_empty());
        if let Some(ref p) = palette {
            if p.len() % 3 != 0 {
                return Err(JsError::new("palette length must be a multiple of 3").into());
            }
        }
        let cfg = CodecConfig {
            shape,
            n_shapes: self.n_shapes,
            cx_bits: self.cx_bits,
            cy_bits: self.cy_bits,
            r_bits: self.r_bits,
            alpha_bits: self.alpha_bits,
            color_bits: self.color_bits,
            theta_bits: self.theta_bits,
            palette,
            palette_k: self.palette_k,
            alpha_levels: None,
            grid_aspect: self.grid_aspect,
        };
        Ok(Codec::Raw(cfg))
    }
}

fn parse_codec(js: JsValue) -> Result<Codec, JsValue> {
    let opts: CodecOptions = if js.is_undefined() || js.is_null() {
        CodecOptions::default()
    } else {
        serde_wasm_bindgen::from_value(js).map_err(|e| JsError::new(&e.to_string()))?
    };
    opts.into_codec()
}

#[derive(Deserialize)]
#[serde(default)]
#[serde(rename_all = "snake_case")]
struct SearchOptionsJs {
    strategy: String,
    n_random: u32,
    n_topk: u32,
    hill_climb_steps: u32,
    hill_climb_max_age: Option<u32>,
    n_attempts: u32,
}

impl Default for SearchOptionsJs {
    fn default() -> Self {
        let s = SearchOptions::default();
        Self {
            strategy: "primitive".to_string(),
            n_random: s.n_random,
            n_topk: s.n_topk,
            hill_climb_steps: s.hill_climb_steps,
            hill_climb_max_age: s.hill_climb_max_age,
            n_attempts: s.n_attempts,
        }
    }
}

fn parse_search(js: JsValue) -> Result<Option<SearchOptions>, JsValue> {
    if js.is_undefined() || js.is_null() {
        return Ok(None);
    }
    let opts: SearchOptionsJs =
        serde_wasm_bindgen::from_value(js).map_err(|e| JsError::new(&e.to_string()))?;
    let strategy = match opts.strategy.as_str() {
        "primitive" => Strategy::Primitive,
        "topk_uniform" | "topk-uniform" => Strategy::TopkUniform,
        other => return Err(JsError::new(&format!("unknown search strategy: {other}")).into()),
    };
    Ok(Some(SearchOptions {
        strategy,
        n_random: opts.n_random,
        n_topk: opts.n_topk,
        hill_climb_steps: opts.hill_climb_steps,
        hill_climb_max_age: opts.hill_climb_max_age,
        n_attempts: opts.n_attempts,
    }))
}

/// Encode an RGB buffer (row-major, 3 bytes per pixel).
#[wasm_bindgen(js_name = encodeRgb)]
pub fn encode_rgb(
    rgb: &[u8],
    w: u32,
    h: u32,
    codec: JsValue,
    seed: u64,
    search: JsValue,
) -> Result<Vec<u8>, JsValue> {
    let c = parse_codec(codec)?;
    let s = parse_search(search)?;
    let bytes = rs_encode_rgb(rgb, w, h, &c, EncodeOptions { seed, search: s });
    Ok(bytes)
}

/// Encode an RGBA buffer (row-major, 4 bytes per pixel). For shape modes,
/// alpha is composited over white before encoding (since shape codecs don't
/// carry image alpha).
#[wasm_bindgen(js_name = encodeRgba)]
pub fn encode_rgba(
    rgba: &[u8],
    w: u32,
    h: u32,
    codec: JsValue,
    seed: u64,
    search: JsValue,
) -> Result<Vec<u8>, JsValue> {
    let c = parse_codec(codec)?;
    let s = parse_search(search)?;
    let bytes = rs_encode_rgba(rgba, w, h, &c, EncodeOptions { seed, search: s });
    Ok(bytes)
}

/// Decode result for [`decode`]. JS sees `{ w, h, rgba }`.
#[wasm_bindgen]
pub struct DecodeResult {
    #[wasm_bindgen(readonly)]
    pub w: u32,
    #[wasm_bindgen(readonly)]
    pub h: u32,
    rgba: Vec<u8>,
}

#[wasm_bindgen]
impl DecodeResult {
    /// Returns a freshly-allocated Uint8Array containing the RGBA bytes
    /// (row-major, 4 bytes per pixel).
    #[wasm_bindgen(getter)]
    pub fn rgba(&self) -> Vec<u8> {
        self.rgba.clone()
    }
}

/// Decode a hash to RGBA pixels.
#[wasm_bindgen]
pub fn decode(
    hash: &[u8],
    codec: JsValue,
    base_size: u32,
    override_aspect: Option<f32>,
    aa: Option<u32>,
    pixel_smooth: Option<String>,
) -> Result<DecodeResult, JsValue> {
    let c = parse_codec(codec)?;
    let ps = match pixel_smooth.as_deref() {
        None | Some("nearest") => PixelSmooth::Nearest,
        Some("bilinear") => PixelSmooth::Bilinear,
        Some(other) => return Err(JsError::new(&format!("unknown pixel_smooth: {other}")).into()),
    };
    let out = rs_decode(
        hash,
        &c,
        DecodeOptions {
            base_size,
            override_aspect,
            pixel_smooth: ps,
            aa: aa.unwrap_or(1).max(1),
        },
    );
    Ok(DecodeResult { w: out.width, h: out.height, rgba: out.rgba })
}

/// Render a shape-mode hash as a compact SVG string.
#[wasm_bindgen(js_name = toSvg)]
pub fn to_svg(
    hash: &[u8],
    codec: JsValue,
    base_size: u32,
    override_aspect: Option<f32>,
    blur: f32,
) -> Result<String, JsValue> {
    let c = parse_codec(codec)?;
    rs_to_svg(
        hash,
        &c,
        SvgOptions {
            base_size,
            override_aspect,
            blur,
        },
    )
    .map_err(|e: SvgError| JsError::new(&e.to_string()).into())
}

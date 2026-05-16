//! wasm-bindgen wrapper around `pfhash` (Rust core). Exposes the same three
//! primitives the PyO3 binding exposes: `encode_rgb`, `decode`, `to_svg`.
//!
//! Codec config is passed as a plain JS object; we deserialize into a
//! `CodecOptions` struct via `serde-wasm-bindgen` and rebuild a `pfhash::Codec`
//! on the Rust side. Palette mode is exposed via `palette` (flat sRGB bytes,
//! length = 3·K) — consensus knowledge the caller injects at encode + decode
//! time; the palette itself never enters the hash bytes.

use pfhash::shape::pixel::PixelSmooth;
use pfhash::{
    decode as rs_decode, encode_rgb as rs_encode_rgb, to_svg as rs_to_svg, Codec, DecodeOptions,
    EncodeOptions, ShapeType, SvgError, SvgOptions,
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
    /// Flat sRGB bytes, length = 3·K. `None`/empty ⇒ continuous color mode.
    palette: Option<Vec<u8>>,
    /// Effective K. Defaults to `palette.len() / 3` when palette is set.
    palette_k: Option<usize>,
}

impl Default for CodecOptions {
    fn default() -> Self {
        let c = Codec::default();
        Self {
            shape: "dct".to_string(),
            n_shapes: c.n_shapes,
            cx_bits: c.cx_bits,
            cy_bits: c.cy_bits,
            r_bits: c.r_bits,
            alpha_bits: c.alpha_bits,
            color_bits: c.color_bits,
            palette: None,
            palette_k: None,
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
        Ok(Codec {
            shape,
            n_shapes: self.n_shapes,
            cx_bits: self.cx_bits,
            cy_bits: self.cy_bits,
            r_bits: self.r_bits,
            alpha_bits: self.alpha_bits,
            color_bits: self.color_bits,
            palette,
            palette_k: self.palette_k,
            ..Codec::default()
        })
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

/// Encode an RGB buffer (row-major, 3 bytes per pixel) into a pfhash byte
/// string.
#[wasm_bindgen(js_name = encodeRgb)]
pub fn encode_rgb(rgb: &[u8], w: u32, h: u32, codec: JsValue, seed: u64) -> Result<Vec<u8>, JsValue> {
    let c = parse_codec(codec)?;
    let bytes = rs_encode_rgb(rgb, w, h, &c, EncodeOptions { seed, search: None });
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

/// Decode a hash to RGBA pixels. `base_size` is the long-edge target
/// (default 256). `override_aspect` lets callers force a known aspect.
#[wasm_bindgen]
pub fn decode(
    hash: &[u8],
    codec: JsValue,
    base_size: u32,
    override_aspect: Option<f32>,
) -> Result<DecodeResult, JsValue> {
    let c = parse_codec(codec)?;
    let (w, h, rgba) = rs_decode(
        hash,
        &c,
        DecodeOptions {
            base_size,
            override_aspect,
            pixel_smooth: PixelSmooth::Nearest,
        },
    );
    Ok(DecodeResult { w, h, rgba })
}

/// Render a CIRCLE or TRIANGLE hash as a compact SVG string.
///
/// Throws on DCT/PIXEL modes (their representations have no natural SVG
/// primitive form).
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

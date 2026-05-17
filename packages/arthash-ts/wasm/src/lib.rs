//! wasm-bindgen wrapper around `arthash` (Rust core). Exposes the same set
//! of primitives the PyO3 binding exposes — `encodeRgb`, `encodeRgba`,
//! `decode`, `toSvg`.
//!
//! Codec config crosses the FFI boundary as a plain JS object that maps to
//! the SPEC field layout (`{shape, n_shapes, palette, …}`). Parsing is done
//! by hand via `js_sys::Reflect` — see `parse_codec` / `parse_search` — to
//! avoid pulling in `serde-wasm-bindgen` + `serde_derive`, which together
//! cost ~40 KB of wasm. The JS-facing TypeScript SDK exposes a friendlier
//! discriminated-union API on top.

use arthash::{
    decode as rs_decode, encode_rgb as rs_encode_rgb, encode_rgba as rs_encode_rgba,
    to_svg as rs_to_svg, Codec, CodecConfig, DecodeOptions, EncodeOptions, PixelSmooth,
    SearchOptions, ShapeType, Strategy, SvgError, SvgOptions,
};
use js_sys::{Array, Reflect};
use wasm_bindgen::prelude::*;

// ---------------------------------------------------------------------------
// JS-object parsing helpers
//
// All `get_*` helpers treat missing keys, `undefined`, and `null` the same
// way (caller-supplied fallback). This is one step looser than serde — which
// distinguishes missing-vs-null for `Option<T>` fields — but the TS SDK
// never sends `null` for these fields, so the simplification is lossless in
// practice.
// ---------------------------------------------------------------------------

fn get(obj: &JsValue, key: &str) -> JsValue {
    Reflect::get(obj, &JsValue::from_str(key)).unwrap_or(JsValue::UNDEFINED)
}

fn get_str(obj: &JsValue, key: &str) -> Option<String> {
    get(obj, key).as_string()
}

fn get_u32(obj: &JsValue, key: &str) -> Option<u32> {
    get(obj, key).as_f64().map(|f| f as u32)
}

fn get_usize(obj: &JsValue, key: &str) -> Option<usize> {
    get(obj, key).as_f64().map(|f| f as usize)
}

fn get_f32(obj: &JsValue, key: &str) -> Option<f32> {
    get(obj, key).as_f64().map(|f| f as f32)
}

fn get_u8_array(obj: &JsValue, key: &str) -> Option<Vec<u8>> {
    let v = get(obj, key);
    if v.is_undefined() || v.is_null() {
        return None;
    }
    let arr = Array::from(&v);
    let len = arr.length() as usize;
    let mut out = Vec::with_capacity(len);
    for i in 0..len as u32 {
        out.push(arr.get(i).as_f64()? as u8);
    }
    Some(out)
}

fn parse_codec(js: JsValue) -> Result<Codec, JsValue> {
    let (shape_str, cfg_partial) = if js.is_undefined() || js.is_null() {
        ("dct".to_string(), CodecConfigDefaults::default())
    } else {
        (
            get_str(&js, "shape").unwrap_or_else(|| "dct".to_string()),
            CodecConfigDefaults {
                n_shapes: get_u32(&js, "n_shapes").unwrap_or(12),
                cx_bits: get_u32(&js, "cx_bits").unwrap_or(5),
                cy_bits: get_u32(&js, "cy_bits").unwrap_or(5),
                r_bits: get_u32(&js, "r_bits").unwrap_or(4),
                alpha_bits: get_u32(&js, "alpha_bits").unwrap_or(3),
                color_bits: get_u32(&js, "color_bits").unwrap_or(16),
                theta_bits: get_u32(&js, "theta_bits").unwrap_or(5),
                palette: get_u8_array(&js, "palette"),
                palette_k: get_usize(&js, "palette_k"),
                grid_aspect: get_f32(&js, "grid_aspect"),
            },
        )
    };

    let shape = ShapeType::from_str(&shape_str)
        .ok_or_else(|| JsError::new(&format!("unknown shape: {shape_str}")))?;
    let palette = cfg_partial.palette.filter(|p| !p.is_empty());
    if let Some(ref p) = palette {
        if p.len() % 3 != 0 {
            return Err(JsError::new("palette length must be a multiple of 3").into());
        }
    }
    Ok(Codec::Raw(CodecConfig {
        shape,
        n_shapes: cfg_partial.n_shapes,
        cx_bits: cfg_partial.cx_bits,
        cy_bits: cfg_partial.cy_bits,
        r_bits: cfg_partial.r_bits,
        alpha_bits: cfg_partial.alpha_bits,
        color_bits: cfg_partial.color_bits,
        theta_bits: cfg_partial.theta_bits,
        palette,
        palette_k: cfg_partial.palette_k,
        alpha_levels: None,
        grid_aspect: cfg_partial.grid_aspect,
    }))
}

struct CodecConfigDefaults {
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

impl Default for CodecConfigDefaults {
    fn default() -> Self {
        Self {
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

fn parse_search(js: JsValue) -> Result<Option<SearchOptions>, JsValue> {
    if js.is_undefined() || js.is_null() {
        return Ok(None);
    }
    let defaults = SearchOptions::default();
    let strategy_str = get_str(&js, "strategy").unwrap_or_else(|| "primitive".to_string());
    let strategy = match strategy_str.as_str() {
        "primitive" => Strategy::Primitive,
        "topk_uniform" | "topk-uniform" => Strategy::TopkUniform,
        other => return Err(JsError::new(&format!("unknown search strategy: {other}")).into()),
    };
    Ok(Some(SearchOptions {
        strategy,
        n_random: get_u32(&js, "n_random").unwrap_or(defaults.n_random),
        n_topk: get_u32(&js, "n_topk").unwrap_or(defaults.n_topk),
        hill_climb_steps: get_u32(&js, "hill_climb_steps").unwrap_or(defaults.hill_climb_steps),
        hill_climb_max_age: get_u32(&js, "hill_climb_max_age").or(defaults.hill_climb_max_age),
        n_attempts: get_u32(&js, "n_attempts").unwrap_or(defaults.n_attempts),
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
    Ok(DecodeResult {
        w: out.width,
        h: out.height,
        rgba: out.rgba,
    })
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

//! PyO3 binding for arthash-rs.
//!
//! Exposes a flat function surface (`encode_rgb`, `encode_rgba`, `decode`,
//! `to_svg`) consumed by the thin Python wrapper that lives next to this
//! crate (see `python/arthash/`). The wrapper owns the user-facing
//! `Codec` / `SearchOptions` dataclasses and converts them to plain dicts
//! before crossing the FFI boundary — this keeps the binding small and
//! lets Python keep ownership of validation + derived properties.

use numpy::PyArray1;
use arthash::shape::options::Strategy;
use arthash::shape::pixel::PixelSmooth;
use arthash::shape::SearchOptions;
use arthash::{
    decode as rs_decode, encode_rgb as rs_encode_rgb, encode_rgba as rs_encode_rgba,
    to_svg as rs_to_svg, Codec, DecodeOptions, EncodeOptions, ShapeType, SvgOptions,
};
use pyo3::exceptions::{PyNotImplementedError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict};

fn parse_shape(s: &str) -> PyResult<ShapeType> {
    ShapeType::from_str(s)
        .ok_or_else(|| PyValueError::new_err(format!("unknown shape: {}", s)))
}

fn parse_pixel_smooth(s: &str) -> PyResult<PixelSmooth> {
    match s {
        "nearest" => Ok(PixelSmooth::Nearest),
        "bilinear" => Ok(PixelSmooth::Bilinear),
        _ => Err(PyValueError::new_err(format!(
            "unknown pixel_smooth: {} (want 'nearest'|'bilinear')",
            s
        ))),
    }
}

fn parse_strategy(s: &str) -> PyResult<Strategy> {
    match s {
        "primitive" => Ok(Strategy::Primitive),
        "topk_uniform" | "topk-uniform" => Ok(Strategy::TopkUniform),
        _ => Err(PyValueError::new_err(format!(
            "unknown search strategy: {} (want 'primitive'|'topk_uniform')",
            s
        ))),
    }
}

fn codec_from_dict(d: Option<&Bound<'_, PyDict>>) -> PyResult<Codec> {
    let mut codec = Codec::default();
    let Some(d) = d else { return Ok(codec) };

    if let Some(v) = d.get_item("shape")? {
        codec.shape = parse_shape(v.extract::<String>()?.as_str())?;
    }
    if let Some(v) = d.get_item("n_shapes")? {
        codec.n_shapes = v.extract()?;
    }
    if let Some(v) = d.get_item("cx_bits")? {
        codec.cx_bits = v.extract()?;
    }
    if let Some(v) = d.get_item("cy_bits")? {
        codec.cy_bits = v.extract()?;
    }
    if let Some(v) = d.get_item("r_bits")? {
        codec.r_bits = v.extract()?;
    }
    if let Some(v) = d.get_item("alpha_bits")? {
        codec.alpha_bits = v.extract()?;
    }
    if let Some(v) = d.get_item("color_bits")? {
        codec.color_bits = v.extract()?;
    }
    if let Some(v) = d.get_item("theta_bits")? {
        codec.theta_bits = v.extract()?;
    }
    if let Some(v) = d.get_item("palette")? {
        let pal: Vec<u8> = v.extract()?;
        codec.palette = Some(pal);
    }
    if let Some(v) = d.get_item("palette_k")? {
        codec.palette_k = Some(v.extract()?);
    }
    if let Some(v) = d.get_item("alpha_levels")? {
        let levels: Vec<f32> = v.extract()?;
        codec.alpha_levels = Some(levels);
    }
    if let Some(v) = d.get_item("grid_aspect")? {
        if !v.is_none() {
            codec.grid_aspect = Some(v.extract()?);
        }
    }
    Ok(codec)
}

fn search_from_dict(d: Option<&Bound<'_, PyDict>>) -> PyResult<Option<SearchOptions>> {
    let Some(d) = d else { return Ok(None) };
    if d.is_empty() {
        return Ok(None);
    }
    let mut s = SearchOptions::default();
    if let Some(v) = d.get_item("strategy")? {
        s.strategy = parse_strategy(v.extract::<String>()?.as_str())?;
    }
    if let Some(v) = d.get_item("n_random")? {
        s.n_random = v.extract()?;
    }
    if let Some(v) = d.get_item("n_topk")? {
        s.n_topk = v.extract()?;
    }
    if let Some(v) = d.get_item("hill_climb_steps")? {
        s.hill_climb_steps = v.extract()?;
    }
    if let Some(v) = d.get_item("hill_climb_max_age")? {
        s.hill_climb_max_age = if v.is_none() { None } else { Some(v.extract()?) };
    }
    if let Some(v) = d.get_item("n_attempts")? {
        s.n_attempts = v.extract()?;
    }
    Ok(Some(s))
}

#[pyfunction]
#[pyo3(signature = (rgb, w, h, codec=None, seed=0, search=None))]
fn encode_rgb<'py>(
    py: Python<'py>,
    rgb: &[u8],
    w: u32,
    h: u32,
    codec: Option<&Bound<'py, PyDict>>,
    seed: u64,
    search: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyBytes>> {
    let codec = codec_from_dict(codec)?;
    let search = search_from_dict(search)?;
    let opts = EncodeOptions { seed, search };
    let bytes = py.detach(|| rs_encode_rgb(rgb, w, h, &codec, opts));
    Ok(PyBytes::new(py, &bytes))
}

#[pyfunction]
#[pyo3(signature = (rgba, w, h, codec=None, seed=0, search=None))]
fn encode_rgba<'py>(
    py: Python<'py>,
    rgba: &[u8],
    w: u32,
    h: u32,
    codec: Option<&Bound<'py, PyDict>>,
    seed: u64,
    search: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyBytes>> {
    let codec = codec_from_dict(codec)?;
    let search = search_from_dict(search)?;
    let opts = EncodeOptions { seed, search };
    let bytes = py.detach(|| rs_encode_rgba(rgba, w, h, &codec, opts));
    Ok(PyBytes::new(py, &bytes))
}

#[pyfunction]
#[pyo3(signature = (hash, codec=None, base_size=256, override_aspect=None, pixel_smooth="nearest"))]
fn decode<'py>(
    py: Python<'py>,
    hash: &[u8],
    codec: Option<&Bound<'py, PyDict>>,
    base_size: u32,
    override_aspect: Option<f32>,
    pixel_smooth: &str,
) -> PyResult<(u32, u32, Bound<'py, PyBytes>)> {
    let codec = codec_from_dict(codec)?;
    let opts = DecodeOptions {
        base_size,
        override_aspect,
        pixel_smooth: parse_pixel_smooth(pixel_smooth)?,
        aa: 1,
    };
    let (w, h, rgba) = py.detach(|| rs_decode(hash, &codec, opts));
    Ok((w, h, PyBytes::new(py, &rgba)))
}

/// Same as `decode` but returns the RGBA as a numpy ndarray to spare a copy
/// when callers will reshape immediately.
#[pyfunction]
#[pyo3(signature = (hash, codec=None, base_size=256, override_aspect=None, pixel_smooth="nearest"))]
fn decode_to_numpy<'py>(
    py: Python<'py>,
    hash: &[u8],
    codec: Option<&Bound<'py, PyDict>>,
    base_size: u32,
    override_aspect: Option<f32>,
    pixel_smooth: &str,
) -> PyResult<(u32, u32, Bound<'py, PyArray1<u8>>)> {
    let codec = codec_from_dict(codec)?;
    let opts = DecodeOptions {
        base_size,
        override_aspect,
        pixel_smooth: parse_pixel_smooth(pixel_smooth)?,
        aa: 1,
    };
    let (w, h, rgba) = py.detach(|| rs_decode(hash, &codec, opts));
    Ok((w, h, PyArray1::from_vec(py, rgba)))
}

#[pyfunction]
#[pyo3(signature = (hash, codec=None, base_size=256, override_aspect=None, blur=0.0))]
fn to_svg(
    py: Python<'_>,
    hash: &[u8],
    codec: Option<&Bound<'_, PyDict>>,
    base_size: u32,
    override_aspect: Option<f32>,
    blur: f32,
) -> PyResult<String> {
    let codec = codec_from_dict(codec)?;
    let opts = SvgOptions {
        base_size,
        override_aspect,
        blur,
    };
    py.detach(|| rs_to_svg(hash, &codec, opts))
        .map_err(|e| match e {
            arthash::SvgError::UnsupportedShape(_) => PyNotImplementedError::new_err(e.to_string()),
        })
}

#[pymodule]
fn _native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(encode_rgb, m)?)?;
    m.add_function(wrap_pyfunction!(encode_rgba, m)?)?;
    m.add_function(wrap_pyfunction!(decode, m)?)?;
    m.add_function(wrap_pyfunction!(decode_to_numpy, m)?)?;
    m.add_function(wrap_pyfunction!(to_svg, m)?)?;
    Ok(())
}

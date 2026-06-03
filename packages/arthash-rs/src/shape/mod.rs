//! Shape modes: CIRCLE / TRIANGLE / SQUARE / RECT / ROTATED_RECT / PIXEL.
//!
//! All modes share an aspect-coded header + per-shape body, encoded
//! LSB-first via `bitio`. See SPEC §5.2 (CIRCLE), §5.3 (TRIANGLE),
//! §5.4 (SQUARE), §5.5 (RECT), §5.6 (ROTATED_RECT), §5.7 (PIXEL).
//!
//! Submodules below are crate-internal — the public API is the `Codec` enum
//! + `encode_rgb` / `decode` / `to_svg` re-exports at the crate root.

pub(crate) mod circle;
pub(crate) mod common;
pub(crate) mod integral;
pub(crate) mod integral2d;
pub mod options; // SearchOptions / Strategy are part of the public API.
pub(crate) mod palette;
pub mod pixel; // PixelSmooth is part of the public API.
pub(crate) mod quant;
pub mod raster; // counters / EvalResult exposed for bench harnesses.
pub(crate) mod rect;
pub(crate) mod residual;
pub(crate) mod rng;
pub(crate) mod rotrect;
pub(crate) mod square;
pub mod svg; // `to_svg` is part of the public API.
pub(crate) mod triangle;

pub use options::SearchOptions;

/// Encoder thumbnail long-edge — search-quality knob, NOT byte-format.
/// Mirrors Python's THUMB=48 default. Only consumed by the `image-io`
/// resize path in [`crate::api::encode_image`], so it is gated to match.
#[cfg(feature = "image-io")]
pub(crate) const THUMB: u32 = 48;

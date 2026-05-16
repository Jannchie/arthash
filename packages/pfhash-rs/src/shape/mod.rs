//! Shape modes: CIRCLE / TRIANGLE / PIXEL.
//!
//! All three modes share an aspect-coded header + per-shape body, encoded
//! LSB-first via `bitio`. See SPEC §5.2 / §5.3 / §5.4.

pub mod circle;
pub mod integral;
pub mod options;
pub mod pixel;
pub mod quant;
pub mod raster;
pub mod residual;
pub mod rng;
pub mod svg;
pub mod triangle;

pub use options::SearchOptions;

/// Encoder thumbnail long-edge — search-quality knob, NOT byte-format.
/// Mirrors Python's THUMB=48 default.
pub const THUMB: u32 = 48;

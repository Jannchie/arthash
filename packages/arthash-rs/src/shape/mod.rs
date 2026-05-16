//! Shape modes: CIRCLE / TRIANGLE / SQUARE / RECT / ROTATED_RECT / PIXEL.
//!
//! All modes share an aspect-coded header + per-shape body, encoded
//! LSB-first via `bitio`. See SPEC §5.2 (CIRCLE), §5.3 (TRIANGLE),
//! §5.4 (SQUARE), §5.5 (RECT), §5.6 (ROTATED_RECT), §5.7 (PIXEL).

pub mod circle;
pub mod integral;
pub mod integral2d;
pub mod options;
pub mod pixel;
pub mod quant;
pub mod raster;
pub mod rect;
pub mod residual;
pub mod rng;
pub mod rotrect;
pub mod square;
pub mod svg;
pub mod triangle;

pub use options::SearchOptions;

/// Encoder thumbnail long-edge — search-quality knob, NOT byte-format.
/// Mirrors Python's THUMB=48 default.
pub const THUMB: u32 = 48;

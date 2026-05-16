// The shape rasterizers (circle / triangle / raster.rs) pass many primitive
// parameters through deep call chains, and a couple of inner buffers use
// complex tuple types. Refactoring into context structs adds indirection
// without clarifying anything — silence these two clippy lints crate-wide.
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

//! arthash — placeholder-image hash family (Rust SDK).
//!
//! Four modes share one `Codec` API:
//!
//! * `Dct`      — V4 thumbhash-style hash (~21 B). Default codec.
//! * `Circle`   — SQIP-style overlapping circles.
//! * `Triangle` — Primitive-style triangle mosaic.
//! * `Pixel`    — Retro-palette pixel mosaic.
//!
//! The byte format is defined by `docs/SPEC.md`; this crate implements
//! that specification.
//!
//! ```ignore
//! use arthash::{encode_rgb, decode, Codec, ShapeType};
//!
//! // DCT mode (default codec).
//! let codec = Codec::default();
//! let bytes = encode_rgb(&rgb_data, w, h, &codec, Default::default());
//! let (out_w, out_h, rgba) = decode(&bytes, &codec, 256, None, Default::default());
//! ```
//!
//! Inputs are RAW RGB/RGBA buffers (`&[u8]`) at the encoder's target
//! resolution. The library does **not** load images or resize them — the
//! caller supplies the thumbnail (or full image, for DCT at native size
//! ≤ target_size). Enable the `image-io` feature for convenience helpers.

pub mod bitio;
pub mod codec;
pub mod colorspace;
pub mod dct;
pub mod shape;

mod api;

pub use api::{decode, encode_rgb, encode_rgba, DecodeOptions, EncodeOptions};
pub use codec::{Codec, ShapeType};
pub use shape::options::SearchOptions;
pub use shape::svg::{to_svg, SvgError, SvgOptions};

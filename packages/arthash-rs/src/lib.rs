// The shape rasterizers (circle / triangle / raster.rs) pass many primitive
// parameters through deep call chains, and a couple of inner buffers use
// complex tuple types. Refactoring into context structs adds indirection
// without clarifying anything — silence these two clippy lints crate-wide.
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

//! arthash — placeholder-image hash family (Rust SDK).
//!
//! Construct a [`Codec`] via its factory methods, then call [`encode_rgb`] /
//! [`encode_rgba`] / [`decode`]. All seven modes share the same surface:
//!
//! ```ignore
//! use arthash::{Codec, encode_rgb, decode, EncodeOptions, DecodeOptions};
//!
//! let codec = Codec::dct();                   // or Codec::triangle(64), etc.
//! let bytes = encode_rgb(&rgb, w, h, &codec, EncodeOptions::default());
//! let out = decode(&bytes, &codec, DecodeOptions::default());
//! // out.width / out.height / out.rgba
//! ```
//!
//! Inputs are RAW RGB/RGBA buffers (`&[u8]`) at the encoder's target
//! resolution. The core does NOT load images or resize them — the caller
//! supplies a thumbnail (`48 px` long-edge for shape modes, `≤ 100 px` for
//! DCT). Enable the `image-io` feature for the [`encode_image`] convenience
//! that reads a file path and resizes for you.

pub mod bitio;
pub mod codec;
pub mod colorspace;
pub mod dct;
pub mod render;
pub mod shape;

mod api;

pub use api::{
    decode, encode_rgb, encode_rgba, try_decode, try_encode_rgb, try_encode_rgba, DecodeError,
    DecodeOptions, DecodeOutput, EncodeError, EncodeOptions,
};
pub use codec::{Codec, CodecError, ColorMode, MAX_PALETTE_K, MIN_PALETTE_K, Palette, Preset};
pub use render::{gaussian_blur_rgba8, RenderStyle};
#[doc(hidden)]
pub use codec::{CodecConfig, ShapeType};
pub use shape::options::{SearchOptions, Strategy};
pub use shape::pixel::PixelSmooth;
pub use shape::svg::{to_svg, SvgError, SvgOptions, SvgUnsupported};

#[cfg(feature = "image-io")]
pub use api::encode_image;

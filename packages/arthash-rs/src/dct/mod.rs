//! DCT mode (V4). SPEC §5.1.

pub mod colorspace;
pub mod encode;
pub mod decode;

pub use encode::{encode_dct, encode_dct_rgb_opaque};
pub use decode::decode_dct;

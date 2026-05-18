---
name: arthash-rust
description: arthash Rust SDK — the canonical implementation. Used as-is by the Python and TypeScript SDKs via PyO3 / wasm-bindgen.
---

# arthash — Rust SDK

`cargo add arthash`. The canonical core; Python and TypeScript SDKs both wrap this same crate.

## Recommended starting point

Start with the default rectangle codec — small bytes, SVG output, no palette setup:

```rust
use arthash::{Codec, encode_rgb, decode, to_svg,
              EncodeOptions, DecodeOptions, SvgOptions};

let codec = Codec::rect(24);        // ~119 B, recognisable rectangle mosaic

let hash: Vec<u8> = encode_rgb(&rgb, w, h, &codec, EncodeOptions::default());
let out = decode(&hash, &codec, DecodeOptions { base_size: 512, ..Default::default() });
let svg: String = to_svg(&hash, &codec, SvgOptions { base_size: 512, blur: 4.0, ..Default::default() })?;
```

`Codec::rect(24)` is a good default for most placeholder slots: ~120 B, SVG output you can write straight into a template, no palette setup. Scale `n` up (48 / 64) for hero images, down (12) when you want to match a thumbhash-like 60-byte budget.

## End-to-end — encode, store, decode

```rust
use arthash::{Codec, encode_rgb, decode, to_svg,
              EncodeOptions, DecodeOptions, SvgOptions};
use base64::{engine::general_purpose::STANDARD, Engine as _};

let codec = Codec::rect(24);

// `encode_rgb` takes a pre-thumbnailed buffer (48 px long-edge for shape modes).
// For path-based loading + resize, enable the `image-io` feature — see below.
let hash: Vec<u8> = encode_rgb(&rgb_thumb, w_thumb, h_thumb, &codec, EncodeOptions::default());

// Persist as base64 in a regular text column:
let stored: String = STANDARD.encode(&hash);

// Later: decode the placeholder.
let restored: Vec<u8> = STANDARD.decode(&stored)?;
let out = decode(&restored, &codec,
                 DecodeOptions { base_size: 512, ..Default::default() });
// out.width, out.height, out.rgba (Vec<u8>, RGBA8)

// Or render the same codec as an inline SVG (shape modes only):
let svg: String = to_svg(&restored, &codec,
                         SvgOptions { base_size: 512, blur: 4.0, ..Default::default() })?;
```

## Codec factories and presets

```rust
Codec::rect(24)               // axis-aligned rectangles — recommended default
Codec::square(24)             // squares
Codec::rotated_rect(24)       // rotated rectangles
Codec::triangle(24)           // triangle mosaic
Codec::circle(24)             // overlapping circles
Codec::pixel(16)              // palette pixel mosaic
Codec::dct()                  // blurry frequency-domain placeholder — see below
Preset::LargeTriangle.codec()   // (or any other named preset)
```

DCT mode is intentionally not the recommended default — reach for it only when you specifically want a **blurry, blurhash/thumbhash-style** look at the smallest possible byte budget. `to_svg` on a DCT hash returns `Err`; render via `decode` and paint to PNG / a canvas instead.

Note that `Codec::default()` historically returns `Codec::Dct` for backwards-compatible serialization — for new code, always construct the codec explicitly so the choice is visible at the call site.

## Palettes

```rust
use arthash::{Codec, palettes};

let codec = Codec::rect(24).with_palette(palettes::PICO8);   // 4-bit colour per rect
let hash = encode_rgb(&rgb, w, h, &codec, EncodeOptions::default());
```

Custom palettes: pass a `&[u8]` of `K` RGB triplets where `K ∈ {2, 4, 8, 16, 32}`. The decoder MUST receive the same palette. Bundled palettes (`PICO8`, `NES`, `GAMEBOY`, `MORANDI`, …) are stable across versions.

## Optional features

```toml
[dependencies]
arthash = { version = "0.2", features = ["image-io", "serde"] }
```

- `image-io` — adds the `image` crate dependency and unlocks `encode_image(path, &codec, opts)` so you don't have to write your own resize. Recommended for one-off scripts and tests; usually skipped in services that already have a thumbnail pipeline.
- `serde` — derives `Serialize` / `Deserialize` for `Codec` and `Preset`, useful when persisting the codec discriminator alongside the hash.

## Common Rust-side pitfalls

- **Decoding with a different codec than was used to encode.** The bytes are not self-describing — you must pair `(hash, &codec)`. Persist the codec (or its `Preset` name) alongside the hash if you support more than one.
- **Calling `to_svg` on a `DCT` or `PIXEL` hash.** Returns `Err`; both modes are inherently raster.
- **Passing a full-resolution image to `encode_rgb`.** It does not resize; performance degrades sharply for no quality gain. Resize to the codec's target thumbnail size first (48 px long-edge for shape modes, ≤ 100 px for DCT) or enable `image-io` and use `encode_image`.
- **Relying on `Codec::default()` for new code.** It returns `Codec::Dct` for backwards compatibility — always construct explicitly so the choice is visible at the call site.

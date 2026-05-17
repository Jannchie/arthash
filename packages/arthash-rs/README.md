# arthash (Rust)

Rust SDK for arthash — the canonical implementation of the byte-format
contract in [`docs/SPEC.md`](../../docs/SPEC.md). The Python SDK
(`packages/arthash-py`) is a PyO3 wrapper around this crate; the TypeScript
SDK (`packages/arthash-ts`) is a wasm-bindgen wrapper. Seven modes share one
`Codec` API, hash bytes are not self-describing, decoder needs the same
`Codec`.

Status: **functional, conformance-tested against the cross-language vectors at
`docs/test-vectors/vectors.json`**.

## Modes

| Mode            | Look                                       | Typical bytes |
|-----------------|--------------------------------------------|---------------|
| `Dct`           | thumbhash-style blurry thumbnail (V4)      | ~21           |
| `Circle`        | SQIP-style overlapping circles             | 8–24          |
| `Triangle`      | Primitive-style triangle mosaic            | 12–32         |
| `Square`        | Axis-aligned squares (cx, cy, side)        | 8–24          |
| `Rect`          | Axis-aligned rectangles (cx, cy, w, h)     | 10–28         |
| `RotatedRect`   | Rotated rectangles (+theta)                | 12–32         |
| `Pixel`         | Retro-palette pixel mosaic                 | 8–32          |

## Quick start

```rust
use arthash::{Codec, Preset, encode_rgb, decode, EncodeOptions, DecodeOptions};

let rgb: Vec<u8> = /* row-major H*W*3 sRGB u8 */;

// Build a codec via the factory methods …
let codec = Codec::triangle(64);
// … or pick a named preset
let codec = Preset::DetailTriangle.codec();

let hash = encode_rgb(&rgb, w, h, &codec, EncodeOptions::default());

let out = decode(&hash, &codec, DecodeOptions {
    base_size: 256,
    ..Default::default()
});
// out.width / out.height / out.rgba (length 4·width·height)
```

### Codec factories

`Codec::dct()`, `Codec::circle(n)`, `Codec::triangle(n)`, `Codec::square(n)`,
`Codec::rect(n)`, `Codec::rotated_rect(n)`, `Codec::pixel(n)`. Each variant
ignores fields that don't apply to its mode. Use builders to customize:

```rust
use arthash::{Codec, ColorMode, Palette};

let palette = Palette::from_rgb(&[[0,0,0], /* … */]).unwrap();
let codec = Codec::triangle(24).with_palette(palette);
let codec = Codec::pixel(16).with_color(ColorMode::Rgb888).with_grid_aspect(1.5);
```

### Loading images

The core crate takes raw RGB/RGBA buffers. Enable the `image-io` feature for
a `encode_image(path, codec, opts)` convenience that reads + resizes for you.

### Input expectations

The crate takes **already-thumbnailed** pixel buffers. For shape modes that
means resize to `48` long-edge before calling. For DCT, `≤ 100` long-edge.
The crate does **not** load images or resize — that keeps the core
dependency-free. The optional `image-io` feature bundles the `image` crate
for path-based loading via `encode_image(path, codec, opts)`.

## Conformance

Cross-language test vectors live at `docs/test-vectors/vectors.json`. The
Rust SDK asserts byte-exact match for **DCT vectors with non-random inputs**
(solid + gradient). Two categories are not byte-portable across stacks and
are exercised only via round-trip tests:

* `random` inputs — Python uses numpy's PCG64-DXSM RNG which has no
  byte-stable cross-platform port. Rust uses xoshiro256** internally.
* `circle` / `triangle` modes on any input — the encoder's hill-climb
  uses RNG draws; different RNG → different bytes (still SPEC-valid).
* PIXEL mode on resized inputs — depends on PIL's LANCZOS, which the Rust
  side doesn't reproduce. Native-size inputs round-trip correctly.

Run the conformance suite:

```sh
cargo test
```

## Algorithm parity with Python

The shape encoders mirror Python's `_shape/` layout:

* Primitive-style search by default: tiny canvas-scaled random init,
  α-decoupled Gaussian hill-climb, m independent attempts, post-sweep over
  quantized alphas. Same constants (σ = 6% of long-edge, r_init_max = 12%).
* Triangle validity: pure-integer cross-product check at sin²(17°) on the
  pre-quantization geometry, absorbing 5-bit grid snap noise.
* In-canvas-only search — vertices stay in `[0, w-1]×[0, h-1]` to prevent
  encoder clip from collapsing distinct off-canvas vertices.

## Dependencies

Zero runtime deps for the core. `serde_json` is dev-only (for parsing the
conformance vectors). `image` is optional behind the `image-io` feature.

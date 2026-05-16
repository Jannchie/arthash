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
use arthash::{Codec, ShapeType, encode_rgb, decode, EncodeOptions, DecodeOptions};

let rgb: Vec<u8> = /* row-major H*W*3 sRGB u8 */;
let codec = Codec::default();  // DCT, 12 shapes, 5/5 bit coords...

let hash = encode_rgb(&rgb, w, h, &codec, EncodeOptions::default());

let (out_w, out_h, rgba) = decode(&hash, &codec, DecodeOptions {
    base_size: 256,
    ..Default::default()
});
```

### Input expectations

The crate takes **already-thumbnailed** pixel buffers. For shape modes that
means resize to `arthash::shape::THUMB = 48` long-edge before calling. For
DCT, ≤ 100 long-edge. The crate does **not** load images or resize — that
keeps the core dependency-free. An optional `image-io` feature bundles the
`image` crate for path-based loading.

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

# arthash

> **preview fingerprint hash** — a personal hobby project by Jianqi Pan.

Compact placeholder hashes (~6–32 bytes) for thumbnail and preview workflows.
Tiny enough to inline into JSON; rich enough to render as a recognisable
preview while the real image loads.

## Highlights

- **Written in Rust.** One canonical implementation; everything else is a
  thin binding.
- **Bindings for the web and Python.** `arthash` ships a `wasm-bindgen`
  build for browsers / Node; `arthash` on PyPI is a PyO3 wheel covering
  Linux / macOS / Windows × Python 3.11–3.13 via a single abi3 build.
- **Fast.** Encode in ~0.8 ms, decode a 256-pixel placeholder in ~2 ms on a
  laptop — including a 6× faster decode-at-display-size path than the
  reference thumbhash Go port (see [Benchmarks](#benchmarks)).
- **Artful SVG placeholders.** Seven modes share one Codec API: thumbhash-style
  DCT blur plus six shape modes (CIRCLE / TRIANGLE / SQUARE / RECT /
  ROTATED_RECT / PIXEL) that decode to a clean, configurable SVG —
  palette, shape count, blur and seed are all tunable.

| Mode            | Look                                          | Typical bytes |
| --------------- | --------------------------------------------- | ------------- |
| `DCT`           | Oklab thumbhash-style blurry thumbnail        | ~21           |
| `CIRCLE`        | SQIP-style overlapping circles, SVG out       | 8–24          |
| `TRIANGLE`      | Primitive-style triangle mosaic, SVG out      | 12–32         |
| `SQUARE`        | Axis-aligned squares (cx, cy, side), SVG out  | 8–24          |
| `RECT`          | Axis-aligned rectangles (cx, cy, w, h), SVG   | 10–28         |
| `ROTATED_RECT`  | Rotated rectangles (+theta), SVG out          | 12–32         |
| `PIXEL`         | Retro-palette pixel mosaic                    | 8–32          |

The byte format is pinned in [`docs/SPEC.md`](./docs/SPEC.md).

## Benchmarks

Numbers below come from [`docs/benchmarks/CROSS_IMPL.md`](./docs/benchmarks/CROSS_IMPL.md),
which runs arthash against the published reference implementations on a
100×100 input on the same machine — no need to re-bench separately.

**Encode** (image → bytes, median)

| Implementation                             |   Time | Bytes |
| ------------------------------------------ | -----: | ----: |
| thumbhash · Rust crate (`evanw/thumbhash`) | 308 µs |    24 |
| thumbhash · Go (`go.n16f.net/thumbhash`)   | 415 µs |    24 |
| **arthash · Rust**                          | 796 µs |    24 |
| **arthash · Python (PyO3)**                 | 620 µs |    24 |
| thumbhash · Python (PyPI `thumbhash`)      |  25 ms |    24 |

**Decode at 256 long-edge** (bytes → RGBA)

| Implementation             |    Time |
| -------------------------- | ------: |
| **arthash · Rust**          | 2.06 ms |
| **arthash · Python (PyO3)** | 2.60 ms |
| thumbhash · Go @ 256       | 12.2 ms |

thumbhash's Rust crate decodes faster than arthash *at its default ~32 px
output*; the comparison only becomes apples-to-apples once you ask for a
display-sized buffer, which is what placeholders actually need.

**Visual quality vs SQIP** at matched output (256 long-edge, PSNR):

| Output                   |   Bytes |    PSNR |
| ------------------------ | ------: | ------: |
| sqip · 12 primitives SVG | ~1100 B | 24.4 dB |
| **arthash · DCT**         |    17 B | 23.3 dB |
| thumbhash                |    17 B | 22.9 dB |
| **arthash · TRIANGLE 12** |    77 B | 21.4 dB |
| **arthash · CIRCLE 12**   |    53 B | 20.7 dB |

arthash matches thumbhash's visual quality bit-for-bit budget (slightly
ahead, ≤0.5 dB) and reaches within 3 dB of sqip's 12-primitive SVG at
**1/20th the size**.

## Quick start

### Rust

```rust
use arthash::{Codec, encode_rgb, decode, EncodeOptions, DecodeOptions};
let codec = Codec::default();
let hash = encode_rgb(&rgb, w, h, &codec, EncodeOptions::default());
let (out_w, out_h, rgba) = decode(&hash, &codec, DecodeOptions {
    base_size: 256, ..Default::default()
});
```

### Python

```python
from arthash import Codec, ShapeType, encode, decode, to_svg

# DCT — thumbhash-style blurry placeholder
hash_bytes = encode("photo.jpg")
w, h, rgba = decode(hash_bytes, base_size=256)

# Shape mode → SVG
codec = Codec(shape=ShapeType.TRIANGLE, n_shapes=12)
hash_bytes = encode("photo.jpg", codec, seed=0)
svg = to_svg(hash_bytes, codec, base_size=256, blur=8.0)
```

### TypeScript (browser / Node, wasm)

```ts
import { init, encode, decode, toSvg, Shape } from "arthash";

// One-time wasm load (~70 KB gzip). Safe to call repeatedly.
await init();

const opts = { shape: Shape.CIRCLE, nShapes: 12 };
const hash = encode(rgbBytes, width, height, opts);
const { w, h, rgba } = decode(hash, { ...opts, baseSize: 256 });
const svg = toSvg(hash, { ...opts, baseSize: 256, blur: 8 });
```

`Shape` covers `DCT`, `CIRCLE`, `TRIANGLE`, `SQUARE`, `RECT`, `ROTATED_RECT`,
`PIXEL` — see [`packages/arthash-ts/README.md`](./packages/arthash-ts/README.md).

## Repository layout

```
packages/
├── arthash-rs/          Rust SDK (canonical implementation)
├── arthash-py/          Python SDK — PyO3 binding
├── arthash-ts/          TypeScript SDK — wasm-bindgen binding
└── arthash-playground/  Vue playground

docs/
├── SPEC.md             authoritative byte-format contract
└── benchmarks/         RESULTS.md, CROSS_IMPL.md, NDJSON
```

## License

MIT.

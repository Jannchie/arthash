# arthash

> **preview fingerprint hash** — a personal hobby project by Jianqi Pan.

Compact placeholder hashes (~6–32 bytes) for thumbnail and preview workflows.
Tiny enough to inline into JSON; rich enough to render as a recognisable
preview while the real image loads.

## Highlights

- **Written in Rust.** One canonical implementation; everything else is a
  thin binding.
- **Bindings for the web and Python.** `@arthash/ts` ships a `wasm-bindgen`
  build for browsers / Node; `arthash` on PyPI is a PyO3 wheel covering
  Linux / macOS / Windows × Python 3.11–3.13 via a single abi3 build.
- **Fast.** Encode in ~0.8 ms, decode a 256-pixel placeholder in ~2 ms on a
  laptop — including a 6× faster decode-at-display-size path than the
  reference thumbhash Go port (see [Benchmarks](#benchmarks)).
- **Artful SVG placeholders.** Four modes share one Codec API: thumbhash-style
  DCT blur plus three SQIP-flavoured shape modes (CIRCLE / TRIANGLE / PIXEL)
  that decode to a clean, configurable SVG — palette, shape count, blur and
  seed are all tunable.

| Mode       | Look                                     | Typical bytes |
| ---------- | ---------------------------------------- | ------------- |
| `DCT`      | Oklab thumbhash-style blurry thumbnail   | ~21           |
| `CIRCLE`   | SQIP-style overlapping circles, SVG out  | 8–24          |
| `TRIANGLE` | Primitive-style triangle mosaic, SVG out | 12–32         |
| `PIXEL`    | Retro-palette pixel mosaic               | 8–32          |

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
import { encode, decode, toSvg, Codec, ShapeType } from "@arthash/ts";

const hash = await encode(imageData, new Codec({ shape: ShapeType.CIRCLE, nShapes: 12 }));
const svg = await toSvg(hash, { baseSize: 256, blur: 8 });
```

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

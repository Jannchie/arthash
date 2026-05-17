<p align="center">
  <img src="./docs/site/public/icon.png" width="128" height="128" alt="arthash">
</p>

<h1 align="center">arthash</h1>

<p align="center">
  <a href="https://codetime.dev"><img src="https://shields.jannchie.com/endpoint?style=social&color=0284c7&url=https%3A%2F%2Fcodetime.dev%2Fv3%2Fusers%2Fshield%3Fuid%3D2%26tag%3Darthash" alt="CodeTime Badge"></a>
</p>

> **English** · [中文](./README.zh-CN.md)

A compact placeholder-image hash — 17 B to 400 B per image, enough to render a recognisable preview while the real image loads.

Written in Rust at the core. Python and TypeScript share the same Rust code (via PyO3 wheel / wasm-bindgen), and hashes produced by any one binding decode on any other.

## What it replaces

| If you use            | Switch to arthash's        | Main wins                                                                |
| --------------------- | -------------------------- | ------------------------------------------------------------------------ |
| blurhash / thumbhash  | `DCT` mode                 | same byte budget, +0.4 dB PSNR; JS encode 1.9× / decode 1.4×             |
| sqip (primitive only) | `TRIANGLE` / `CIRCLE` mode | 1/9 – 1/16 the size; 50–67× faster encode; runs natively in browser wasm |

shape / PIXEL modes can also take an external palette, dropping per-shape colour to 4 bit and giving the output a natural visual style (brand palette, retro, Morandi, ...).

## Quick start

### Rust

```rust
use arthash::{Codec, Preset, encode_rgb, decode, EncodeOptions, DecodeOptions};

// Named preset (recommended)
let codec = Preset::DetailTriangle.codec();          // triangle, n=64
let hash = encode_rgb(&rgb, w, h, &codec, EncodeOptions::default());
let out = decode(&hash, &codec, DecodeOptions::default());
// out.width / out.height / out.rgba

// Or build by factory
let codec = Codec::triangle(64);
// Codec::dct(), Codec::circle(n), Codec::square(n), Codec::rect(n),
// Codec::rotated_rect(n), Codec::pixel(n)
```

### Python (PyO3 wheel)

```python
from arthash import Codec, Preset, encode, decode, to_svg

# DCT — thumbhash-style blurry placeholder
hash_bytes = encode("photo.jpg")
w, h, rgba = decode(hash_bytes, base_size=256)   # rgba shape (h, w, 4)

# Named preset
codec = Codec.preset(Preset.DETAIL_TRIANGLE)
hash_bytes = encode("photo.jpg", codec)
svg = to_svg(hash_bytes, codec, base_size=512, blur=8.0)

# Factory + palette
from arthash.palettes import PICO8
codec = Codec.triangle(n=24, palette=PICO8)
hash_bytes = encode("photo.jpg", codec)
```

### TypeScript (wasm-bindgen, browser / Node)

```ts
import { encode, decode, toSvg, codec, Preset, encodeImage } from "arthash";

// Wasm loads automatically on first call. Optionally `await init()` to preload.

// Named preset
const c = codec.preset(Preset.DetailTriangle);   // triangle, n=64
const hash = await encode(rgbBytes, width, height, c);
const { w, h, rgba } = await decode(hash, c);
const svg = await toSvg(hash, c, { baseSize: 512, blur: 8 });

// Browser convenience: load image, resize, encode in one call
const hash2 = await encodeImage(imageUrlOrBlob, c);
```

Factories: `codec.dct()` / `.circle({ n })` / `.triangle({ n })` / `.square({ n })` / `.rect({ n })` / `.rotatedRect({ n })` / `.pixel({ n })`. See [`packages/arthash-ts/README.md`](./packages/arthash-ts/README.md).

**Frontend footprint**: ~67 KB brotli / ~93 KB gzip for the wasm core on first load (HTTP-cached after), then ~6 KB of SDK in your bundle. The wasm is monolithic — it ships every codec mode plus the hill-climb encoder, not a decode-only build (a decode-only build would shave ~15–20 KB brotli; [open an issue](https://github.com/Jannchie/arthash/issues) if you need it). Full breakdown in [`packages/arthash-ts/README.md`](./packages/arthash-ts/README.md#footprint).

## Modes & byte counts

| Mode           | Look                               | n=12 bytes | n=64 bytes |
| -------------- | ---------------------------------- | ---------: | ---------: |
| `DCT`          | blurry thumbnail (thumbhash-style) |      17–24 |          — |
| `PIXEL`        | palette pixel mosaic               |         25 |        129 |
| `CIRCLE`       | overlapping circles, SVG out       |         53 |        267 |
| `SQUARE`       | axis-aligned squares, SVG out      |         53 |        267 |
| `RECT`         | axis-aligned rectangles, SVG out   |         59 |        299 |
| `ROTATED_RECT` | rotated rectangles, SVG out        |         66 |        339 |
| `TRIANGLE`     | triangle mosaic, SVG out           |         77 |        395 |

The playground default is `TRIANGLE n=64 / baseSize 512 / RGB-565`, a reasonable starting point. If you need smaller bytes, lower `n` (n=24 → 150 B, n=12 → 77 B). For a specific visual style, swap in a palette. For minimum size plus a blurry look, switch to DCT (≤ 24 B).

## Relation to thumbhash and sqip

**thumbhash** (Evan Wallace, 2023) is the successor to blurhash — same DCT-encoded blurry-thumbnail idea, but with a tighter bit layout, ~24 bytes per image, pure JS. arthash's `DCT` mode targets the same niche.

**sqip** (Tobias Baldauf, 2017) is a Node plugin framework. The most widely used plugin is `sqip-plugin-primitive`, which calls the Go [primitive](https://github.com/fogleman/primitive) tool to hill-climb N geometric shapes onto the image and outputs an SVG string. Typical use: build-time generation, inlined into HTML as an LQIP. arthash's shape modes target the primitive part of sqip.

### Feature comparison

| Feature                           |     arthash     |        thumbhash         |          sqip           |
| --------------------------------- | :-------------: | :----------------------: | :---------------------: |
| DCT blurry thumbnail (17–24 B)    |        ✅        |            ✅             |            ❌            |
| Geometric SVG primitives          |   ✅ 5 shapes    |            ❌             |   ✅ multiple plugins    |
| Pixel mosaic                      |        ✅        |            ❌             |            ❌            |
| External palette (colour → 4 bit) |        ✅        |            ❌             |            ❌            |
| Potrace-style SVG tracing         |        ❌        |            ❌             | ✅ `sqip-plugin-potrace` |
| WebP output                       |        ❌        |            ❌             |   ✅ via some plugins    |
| Decode to arbitrary output size   |        ✅        |     ⚠️ default ~32 px     |     ✅ (SVG, vector)     |
| Web / browser wasm                |        ✅        |        ✅ pure JS         | ❌ (needs Go subprocess) |
| Python binding                    |  ✅ PyO3 wheel   | ⚠️ pure-Python 80× slower |            ❌            |
| Rust crate                        |        ✅        |            ✅             |            ❌            |
| Deployment                        | request / build |     request / build      |     build-time only     |

arthash **does not** cover sqip's Potrace tracing mode (bitmap contour → SVG path), nor does it produce WebP / data-URI output. If you need those, sqip is still the better fit.

## Benchmarks

Same machine, 100×100 input. JS numbers are measured with [`bench/js-cross/`](./bench/js-cross/) on Node 22; raw NDJSON in [`docs/benchmarks/js_cross_*.ndjson`](./docs/benchmarks/). Native numbers come from [`docs/benchmarks/CROSS_IMPL.md`](./docs/benchmarks/CROSS_IMPL.md). All tables are sorted by speed; the *vs baseline* column is `baseline_time / row_time` (so a value > 1 means faster).

### encode: DCT 24 B output

JS (baseline = thumbhash-js):

| Impl                | median |     vs baseline | bytes |
| ------------------- | -----: | --------------: | ----: |
| arthash · ts (wasm) | 279 µs | **1.9× faster** |    24 |
| thumbhash · JS      | 532 µs | 1.0× (baseline) |    24 |

Native (baseline = thumbhash-rust, the fastest non-arthash impl):

| Impl                      | median |      vs baseline | bytes |
| ------------------------- | -----: | ---------------: | ----: |
| arthash · Python (PyO3)   | 242 µs | **1.27× faster** |    24 |
| arthash · Rust            | 243 µs | **1.27× faster** |    24 |
| thumbhash · Rust (evanw)  | 308 µs |  1.0× (baseline) |    24 |
| thumbhash · Go (n16f.net) | 415 µs |     0.74× slower |    24 |
| thumbhash · Python (PyPI) |  25 ms |    0.012× slower |    24 |

arthash Rust and PyO3 are essentially tied (min ~228 µs / median ~243 µs) — PyO3 only adds a thin GIL/PyBytes wrapper, whose µs-level overhead is lost in batch-measurement noise.

### decode: DCT 24 B → RGBA

JS (baseline = thumbhash-js at its default ~32 px output):

| Impl                | output size |      median |     vs baseline |
| ------------------- | ----------: | ----------: | --------------: |
| arthash · ts (wasm) |      ~32 px |      116 µs | **1.4× faster** |
| thumbhash · JS      |      ~32 px |      165 µs | 1.0× (baseline) |
| arthash · ts (wasm) |      256 px |     6.69 ms | *(N/A on opp.)* |
| arthash · ts (wasm) |      512 px |    26.22 ms | *(N/A on opp.)* |
| thumbhash · JS      |        256+ | unsupported |               — |

thumbhash's JS decode API only outputs ~32 px; to go larger you have to CSS-upscale (blurry). arthash IDCTs directly to the target size, skipping the upsample step on the client.

Native @ 256 px (baseline = thumbhash-go @ 256):

| Impl                    |  median |     vs baseline |
| ----------------------- | ------: | --------------: |
| arthash · Rust          | 2.06 ms | **5.9× faster** |
| arthash · Python (PyO3) | 2.60 ms | **4.7× faster** |
| thumbhash · Go @ 256    | 12.2 ms | 1.0× (baseline) |

thumbhash's Rust crate at its native ~32 px default is faster than arthash; but as soon as you ask for a display-sized buffer (the actual placeholder scenario), arthash overtakes it by 6×.

### encode: shape modes vs sqip

JS, baseline = sqip-node at the same `n`. arthash's lead grows with `n` — sqip is linear and IPC-bound (re-hill-climb per primitive), while arthash uses integral images / SSE incremental updates with sub-linear search cost.

**Encode time**:

| Impl                           |     n=12 (ratio) |     n=24 (ratio) |      n=64 (ratio) |
| ------------------------------ | ---------------: | ---------------: | ----------------: |
| arthash · ts TRIANGLE          | 5.1 ms (**56×**) | 7.9 ms (**56×**) | 15.2 ms (**67×**) |
| arthash · ts CIRCLE            |     5.3 ms (54×) |     7.2 ms (62×) |     15.5 ms (66×) |
| sqip · primitive-triangle @0.3 |           284 ms |           446 ms |           1015 ms |

**Output size**:

| Impl                           |           n=12 (ratio) |            n=24 (ratio) |            n=64 (ratio) |
| ------------------------------ | ---------------------: | ----------------------: | ----------------------: |
| arthash · ts CIRCLE            | 53 B (**16× smaller**) | 102 B (**15× smaller**) | 267 B (**14× smaller**) |
| arthash · ts TRIANGLE          |     77 B (11× smaller) |     150 B (10× smaller) |      395 B (9× smaller) |
| sqip · primitive-triangle @0.3 |                  842 B |                  1482 B |                  3650 B |

sqip spawns a Go primitive subprocess per call, so it can't run in the browser; that makes it best suited to **build-time generation**. arthash, via wasm-bindgen, is happy to encode **at request time**.

### Visual quality (256 long-edge, PSNR, sorted by PSNR desc)

| Output                   |   bytes |    PSNR |
| ------------------------ | ------: | ------: |
| sqip · 12 primitives SVG | ~1100 B | 24.4 dB |
| arthash · DCT            |    17 B | 23.3 dB |
| thumbhash                |    17 B | 22.9 dB |
| arthash · TRIANGLE 12    |    77 B | 21.4 dB |
| arthash · CIRCLE 12      |    53 B | 20.7 dB |

At a 17 B budget arthash DCT beats thumbhash by 0.4 dB; arthash TRIANGLE 12 reaches 21.4 dB at 77 B, which is **1/14 the size of sqip's 1100 B / 24.4 dB output for a 3 dB quality drop**.

## How arthash saves bits

Every bit goes to image data; no header overhead. The savings stack in four layers:

**1. No header — two-sided consensus codec.**
The byte stream is not self-describing: no magic number, no mode tag, no bit-widths. Mode, shape count, quantization bit widths, palette are all in the Codec, configured on both encode and decode. We give up "self-description" in exchange for "every bit is image data".

**2. Bit packing, last byte zero-padded.**
LSB-first bit packing. Hash length is fully determined by the codec (`ceil((header_bits + n_shapes × per_shape_bits) / 8)`), wasting at most 7 padding bits.

**3. DCT mode — frequency-domain + perceptual-space squeeze.**
- **Oklab, not sRGB** — quantising in a perceptually uniform space makes each bit yield a more stable visual delta.
- **AB_SCALE = 5** — stretches the a/b channels' dynamic range to match L, so the shared 4-bit AC quantiser wastes no codes.
- **Compander (signed power function)** — `^0.6` for L, `^0.5` for a/b, `^0.6` for α; pre-quantisation distributions become more uniform, the 4-bit nibble works harder.
- **Triangular mask** — only stores coefficients with `cx · ny < nx · (ny − cy)` (the upper-left triangle), dropping high-frequency naturally.
- **DC 6 bit, AC 4 bit, AC scale 4–5 bit** — per-channel adaptive scale, effectively a per-image quant table.
- **`(lx, ly)` derived from aspect** — luma grid shape comes from aspect_code, not stored in the hash.

**4. Shape / PIXEL — frugal geometry and colour.**
- **Log-scale radius quantisation** — radius bucketed in log space across `[min(w,h)/24, max(w,h)]`; 4 bit covers all visually distinguishable scales.
- **RGB-565 vs RGB-888** — default 16 bit saves 8 bit per shape vs 24 bit.
- **Palette mode** — colour field shrinks from 16/24 bit to `log₂(K)` bit; K=16 means 4 bit per colour.
- **Discrete alpha levels** — 3 bit indexes 8 alpha levels, default `linspace(0.20, 0.90, 8)`; very low alpha contributes too little visual delta to be worth a code, dropped.
- **`theta ∈ [0, π)` half-step bias** — rotated rects are π-symmetric so theta only covers half a turn; the `+0.5` bias at decode centres the bucket, halving quantisation error.
- **8-bit aspect code covers 1/8 – 8** — 255 evenly log-spaced levels, code 255 reserved.
- **PIXEL grid shape derived from aspect** — `grid_w × grid_h = n_shapes`, grid shape from aspect, not stored in the hash.

The byte format is pinned in [`docs/SPEC.md`](./docs/SPEC.md).

## Repository layout

```
packages/
├── arthash-rs/          Rust SDK (canonical implementation)
├── arthash-py/          Python SDK — PyO3 binding
├── arthash-ts/          TypeScript SDK — wasm-bindgen binding
└── arthash-playground/  Vue playground

bench/
├── js-cross/            JS cross-impl bench (arthash wasm vs thumbhash-js vs sqip)
├── sqip/                sqip invocation script
└── thumbhash-js/        thumbhash JS bench

docs/
├── SPEC.md              byte-format authority
└── benchmarks/          RESULTS.md, CROSS_IMPL.md, NDJSON
```

## License

MIT.

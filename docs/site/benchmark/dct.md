# DCT vs thumbhash

arthash's `DCT` mode directly targets the same niche as
[thumbhash](https://evanw.github.io/thumbhash/) and
[blurhash](https://blurha.sh/): a tiny (~20 B) blurry placeholder. Same byte
budget, better quality, faster encode and decode.

## Encode @ 24 B output

### JS (baseline = thumbhash-js)

| Impl                | median |     vs baseline | bytes |
| ------------------- | -----: | --------------: | ----: |
| arthash · ts (wasm) | 279 µs | **1.9× faster** |    24 |
| thumbhash · JS      | 532 µs | 1.0× (baseline) |    24 |

### Native (baseline = thumbhash-rust)

| Impl                      | median |      vs baseline | bytes |
| ------------------------- | -----: | ---------------: | ----: |
| arthash · Python (PyO3)   | 242 µs | **1.27× faster** |    24 |
| arthash · Rust            | 243 µs | **1.27× faster** |    24 |
| thumbhash · Rust (evanw)  | 308 µs |  1.0× (baseline) |    24 |
| thumbhash · Go (n16f.net) | 415 µs |     0.74× slower |    24 |
| thumbhash · Python (PyPI) |  25 ms |    0.012× slower |    24 |

arthash Rust and PyO3 are essentially tied (min ~228 µs / median ~243 µs) — PyO3
only adds a thin GIL/PyBytes wrapper, whose µs-level overhead is lost in
batch-measurement noise.

## Decode @ DCT 24 B → RGBA

### JS (baseline = thumbhash-js at its default ~32 px output)

| Impl                | output size |      median |     vs baseline |
| ------------------- | ----------: | ----------: | --------------: |
| arthash · ts (wasm) |      ~32 px |      116 µs | **1.4× faster** |
| thumbhash · JS      |      ~32 px |      165 µs | 1.0× (baseline) |
| arthash · ts (wasm) |      256 px |     6.69 ms | *(N/A on opp.)* |
| arthash · ts (wasm) |      512 px |    26.22 ms | *(N/A on opp.)* |
| thumbhash · JS      |        256+ | unsupported |               — |

::: tip Why this matters in production
thumbhash's JS decode API only outputs ~32 px; to go larger you have to
CSS-upscale (blurry). arthash IDCTs directly to the target size, skipping the
upsample step on the client.
:::

### Native @ 256 px (baseline = thumbhash-go @ 256)

| Impl                    |  median |     vs baseline |
| ----------------------- | ------: | --------------: |
| arthash · Rust          | 2.06 ms | **5.9× faster** |
| arthash · Python (PyO3) | 2.60 ms | **4.7× faster** |
| thumbhash · Go @ 256    | 12.2 ms | 1.0× (baseline) |

thumbhash's Rust crate at its native ~32 px default is faster than arthash; but
as soon as you ask for a display-sized buffer (the actual placeholder
scenario), arthash overtakes it by ~6×.

## Why arthash DCT is faster

- **No second-stage upsample.** arthash IDCTs straight to the requested target
  size, skipping the bilinear upsample that thumbhash needs to escape its 32 px
  default.
- **Single-pass Oklab dequantisation.** No separate sRGB → linear stage; the
  IDCT runs in perceptual space and the final clip-to-sRGB is fused into the
  output writer.
- **Wasm SIMD where available.** The bundled wasm enables SIMD via
  `wasm32-unknown-unknown` target flags; modern browsers and Node 22 take the
  fast path automatically.

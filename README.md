# pfhash

**Placeholder-image hash family** — compact (~6–32 byte) hashes designed to
look plausible when blown up to display size while a real image loads. Four
modes share one Codec API:

| Mode | Look | Typical bytes |
|---|---|---|
| `DCT` (V4) | thumbhash-style blurry thumbnail, Oklab + companded AC | ~21 |
| `CIRCLE` | SQIP-style overlapping circles | 8–24 |
| `TRIANGLE` | Primitive-style triangle mosaic | 12–32 |
| `PIXEL` | Retro-palette pixel mosaic | 8–32 |

The byte format is defined in [`docs/SPEC.md`](./docs/SPEC.md). Every SDK
implements against that spec — if an implementation disagrees with the SPEC,
the implementation is wrong.

Status: **R&D, V4 (May 2026)**. Format is not yet stable — expect changes
before a 1.0 release.

## Repository layout

```
pfhash/
├── docs/
│   ├── SPEC.md                       authoritative byte-format contract
│   ├── benchmarks/                   results: RESULTS.md, CROSS_IMPL.md, NDJSON
│   └── test-vectors/vectors.json     cross-language conformance vectors
│
├── packages/
│   ├── pfhash-rs/                    Rust SDK (canonical implementation)
│   ├── pfhash-py/                    Python SDK — PyO3 binding to pfhash-rs
│   ├── pfhash-ts/                    TypeScript SDK (wasm-bindgen wrapper around pfhash-rs)
│   ├── pfhash-playground/            Vue playground (pnpm filter @pfhash/playground)
│   └── pfhash-research/              R&D playground (gitignored)
│       ├── primitive-bench/          Go primitive-shape micro-bench
│       └── sqip-bench/               Node sqip baseline bench
│
├── bench/                            cross-impl latency benches (NDJSON output)
│   ├── thumbhash/                    Go reference (go.n16f.net/thumbhash)
│   ├── thumbhash-js/                 npm thumbhash@0.1.1
│   ├── thumbhash-rs/                 evanw/thumbhash Rust crate
│   ├── thumbhash-py/                 PyPI thumbhash
│   └── sqip/                         sqip 1.0-beta.2 visual baseline
│
└── scripts/                          driver scripts that consume bench/
    ├── bench_py.py                   pfhash Python timing
    ├── compare_bench.py              merge rust + python NDJSON → RESULTS.md
    ├── thumbhash_cross_impl.py       merge everything → CROSS_IMPL.md
    └── visual_compare.py             grid PNG: pfhash modes vs thumbhash vs sqip
```

`pnpm-workspace.yaml` governs the JS / TS / Rust packages; `uv` governs the
Python packages.

## Quick start

### Python

```python
from pfhash import Codec, ShapeType, encode, decode

# DCT mode (default) — thumbhash-style blurry placeholder
hash_bytes = encode("photo.jpg")                          # ~21 bytes
w, h, rgba = decode(hash_bytes, base_size=256)

# Shape mode
codec = Codec(shape=ShapeType.TRIANGLE, n_shapes=12)
hash_bytes = encode("photo.jpg", codec, seed=0)           # ~77 bytes
w, h, pixels = decode(hash_bytes, codec, base_size=256)
```

### Rust

```rust
use pfhash::{Codec, encode_rgb, decode, EncodeOptions, DecodeOptions};
let codec = Codec::default();
let hash = encode_rgb(&rgb, w, h, &codec, EncodeOptions::default());
let (out_w, out_h, rgba) = decode(&hash, &codec, DecodeOptions {
    base_size: 256, ..Default::default()
});
```

### Building the Python wheel locally

```sh
pnpm run develop:py
# or:
uv run maturin develop --uv --manifest-path packages/pfhash-py/Cargo.toml
```

CI builds wheels for Linux / macOS / Windows × Python 3.11–3.13 via
`.github/workflows/wheels.yml`.

## What pfhash does differently from thumbhash

| Improvement | Mechanism | Why it helps |
|---|---|---|
| **Aspect ratio** stored to 1.7% | 8-bit log-aspect code in `header16` (replaces thash's 3-bit `lx/ly`) | thash's 7-step aspect quantization rounds 16:9 to either 1.4 or 1.75 — a 6.7–17% error visible as stretched content. pfhash stores it directly. |
| **Color space** Oklab | sRGB → linear RGB → Oklab `(L, a, b)`, then DCT each channel | Oklab is perceptually uniform — equal quantization steps correspond to equal perceived color differences. ThumbHash's `(L=mean, P, Q)` ignores sRGB gamma and human luma sensitivity. |
| **Per-image scale search** | For each channel, grid-search the declared AC scale that minimizes reconstruction SSE in the format's 4-bit quantizer | thash uses `max(\|ac\|)` for the encode-side scale but reads back the *quantized* scale, creating a systematic bias. pfhash closes that gap. |
| **Power compander on AC** | Quantize `sign(c)·\|c\|^p` instead of `c`. `p_L=0.6`, `p_PQ=0.5` (tuned on corpus). | AC coefficients are super-Gaussian (excess kurtosis 16–60). Companding gives 4-bit quantization finer steps near zero — where most coefficients live. |
| **Power compander on a/b DC** | `p_dc=0.7` on chroma DC values | a/b DC distributions on natural images are peaked near zero. Compander widens precision in that region. |
| **Nearest-neighbor nibble** | For a given declared scale, each AC coefficient is assigned to its nearest of the 16 reconstruction levels (in companded space) | thash uses `round + clip` which only equals nearest-neighbor when encode-scale matches declared-scale. |

Plus three modes thumbhash doesn't have: CIRCLE / TRIANGLE / PIXEL for
SQIP-style geometric placeholders at similar byte budgets.

## Benchmarks

Full numbers in [`docs/benchmarks/`](./docs/benchmarks/):

- [`RESULTS.md`](./docs/benchmarks/RESULTS.md) — pfhash Python vs Rust vs PyO3
- [`CROSS_IMPL.md`](./docs/benchmarks/CROSS_IMPL.md) — pfhash vs thumbhash (Go/JS/Rust/Py) vs sqip

Headline numbers on a 100×100 input (median of 200 iters):

| Impl | Encode | Decode @256 |
|---|---:|---:|
| thumbhash · Rust crate | **308 µs** | n/a (~32 px native) |
| thumbhash · Go | 415 µs | 12.2 ms |
| pfhash · Rust (this crate) | 796 µs | **2.06 ms** |
| pfhash · Python (PyO3 → Rust core) | 925 µs | 2.22 ms |

The Python row is now the same Rust core through a PyO3 wrapper; the gap to
native Rust is FFI overhead + Pillow ingestion, not algorithmic.

pfhash decode @256 is 6× faster than thumbhash Go at the same output size
because pfhash uses SGEMM-vectorized IDCT. thumbhash's Rust crate is 2.6×
faster on encode because it uses a smaller coefficient support (3×4 vs
pfhash's 7×7) — same algorithm, different quality/speed trade-off.

Visual quality is indistinguishable between pfhash DCT and thumbhash
(≤0.5 dB PSNR gap at matched byte budgets) — see the PNG grids in
`docs/benchmarks/visual_*.png`.

## Setup

```sh
pnpm install                     # JS/TS workspaces
uv sync --all-groups             # Python workspace + research deps
cargo build --manifest-path packages/pfhash-rs/Cargo.toml
```

`uv sync` pulls `thash` from a sibling checkout at `../thumbhash-py` (see
`[tool.uv.sources]` in `pyproject.toml`) — needed only for the research
benchmark comparisons.

## Tests

```sh
pnpm test                        # runs Python + Rust suites
# or directly:
uv run pytest packages/pfhash-py/tests
cargo test --manifest-path packages/pfhash-rs/Cargo.toml
```

The Rust crate asserts byte-exact match against the Python reference for
DCT vectors with non-random inputs. Shape modes use round-trip tests
(byte-identical output across stacks is not expected — the encoder uses
RNG hill-climb, and different RNGs give different SPEC-valid bytes).

## License

MIT.

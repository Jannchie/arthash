# pfhash

Placeholder-image hash family. Four modes share a unified Codec API:

| Shape    | Bytes (typical) | Notes |
|----------|-----------------|-------|
| DCT      | ~21 B           | ThumbHash V4 derivative. Default. |
| CIRCLE   | varies          | SQIP-style overlapping circles. |
| TRIANGLE | varies          | fogleman/primitive-style triangle mosaic. |
| PIXEL    | varies          | Retro-palette pixel mosaic. |

The implementation is a thin Python wrapper around the `pfhash-rs` Rust crate
exposed via PyO3 — encode/decode/SVG all run in native code.

## Install (from source)

```bash
maturin develop --uv -m packages/pfhash-py/Cargo.toml
```

## Quick start

```python
from pfhash import encode, decode, to_svg, Codec, ShapeType

# DCT (default)
h = encode("photo.jpg")
w, hh, rgba = decode(h, base_size=256)

# Shape modes
codec = Codec(shape=ShapeType.CIRCLE, n_shapes=12)
h = encode("photo.jpg", codec, seed=0)
w, hh, rgb = decode(h, codec, base_size=256)        # (h, w, 3) RGB ndarray
svg = to_svg(h, codec, base_size=256, blur=8.0)     # circle/triangle only
```

## Layout

- `python/pfhash/` — public Python API (`Codec`, `ShapeType`, `SearchOptions`,
  `palettes`, `encode`, `decode`, `to_svg`).
- `src/lib.rs` — PyO3 binding to the `pfhash` Rust crate. Compiled into
  `pfhash._native`.
- `tests/` — pytest suite covering codec validation, V4 round-trip, shape
  round-trip, SVG generation, search-options, and the cross-language test
  vectors at `docs/test-vectors/vectors.json`.

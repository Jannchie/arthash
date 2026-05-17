# arthash

Placeholder-image hash family. Seven modes share a unified Codec API:

| Shape          | Bytes (typical) | Notes |
|----------------|-----------------|-------|
| DCT            | ~21 B           | ThumbHash V4 derivative. Default. |
| CIRCLE         | varies          | SQIP-style overlapping circles. |
| TRIANGLE       | varies          | fogleman/primitive-style triangle mosaic. |
| SQUARE         | varies          | Axis-aligned squares (cx, cy, side). |
| RECT           | varies          | Axis-aligned rectangles (cx, cy, w, h). |
| ROTATED_RECT   | varies          | Rotated rectangles — `theta_bits` tunes angle steps. |
| PIXEL          | varies          | Retro-palette pixel mosaic. |

The implementation is a thin Python wrapper around the `arthash-rs` Rust crate
exposed via PyO3 — encode/decode/SVG all run in native code.

## Install (from source)

```bash
maturin develop --uv -m packages/arthash-py/Cargo.toml
```

## Quick start

```python
from arthash import encode, decode, to_svg, Codec, Preset
from arthash.palettes import PICO8

# DCT (default)
h = encode("photo.jpg")
w, hh, rgba = decode(h, base_size=256)              # (h, w, 4) RGBA ndarray

# Named preset
codec = Codec.preset(Preset.DETAIL_TRIANGLE)        # triangle, n=64
h = encode("photo.jpg", codec)

# Factory + palette
codec = Codec.triangle(n=24, palette=PICO8)
h = encode("photo.jpg", codec, seed=0)
svg = to_svg(h, codec, base_size=256, blur=8.0)     # circle/triangle/etc.
```

`decode` always returns `(width, height, ndarray(h, w, 4))` regardless of
codec — alpha is 255 except for DCT-with-alpha sources.

## Layout

- `python/arthash/` — public Python API (`Codec`, `ShapeType`, `SearchOptions`,
  `palettes`, `encode`, `decode`, `to_svg`).
- `src/lib.rs` — PyO3 binding to the `arthash` Rust crate. Compiled into
  `arthash._native`.
- `tests/` — pytest suite covering codec validation, V4 round-trip, shape
  round-trip, SVG generation, search-options, and the cross-language test
  vectors at `docs/test-vectors/vectors.json`.

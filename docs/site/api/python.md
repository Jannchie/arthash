# Python API

Package: [`arthash`](https://pypi.org/project/arthash/) on PyPI. PyO3 wheel —
the heavy lifting still happens in Rust.

```python
from arthash import (
    Codec, Preset, Palette,
    encode, decode, to_svg,
)
from arthash import palettes
```

## Top-level functions

### `encode(image, codec=None, *, seed=0, search=None) -> bytes`

Encode an image into a hash. `image` accepts:

| Type             | Behaviour                                                        |
| ---------------- | ---------------------------------------------------------------- |
| `str` / `Path`   | File path; opened with PIL                                       |
| `bytes`          | Encoded image bytes (PNG / JPEG / …); opened with PIL            |
| `PIL.Image`      | Used directly                                                    |
| `numpy.ndarray`  | `H×W×3` (RGB) or `H×W×4` (RGBA), `uint8`                         |

When `codec` is `None`, defaults to `Codec.dct()`.

### `decode(hash_bytes, codec=None, *, base_size=256, override_aspect=None, aa=None, pixel_smooth=None)`

Returns `(width, height, rgba)` where `rgba` is a `numpy.ndarray` of shape
`(h, w, 4)` and dtype `uint8`. When `codec` is `None`, defaults to `Codec.dct()`.

### `to_svg(hash_bytes, codec, *, base_size=256, override_aspect=None, blur=0.0) -> str`

Render a shape-mode hash as an SVG string. Only supports `CIRCLE` / `TRIANGLE`
/ `SQUARE` / `RECT` / `ROTATED_RECT`. DCT and PIXEL raise `ValueError`.

## `Codec`

The byte-format contract. The same `Codec` must be used for both encode and
decode.

### Factories

```python
Codec.dct()
Codec.circle(n=12, palette=None)
Codec.triangle(n=12, palette=None)
Codec.square(n=12, palette=None)
Codec.rect(n=12, palette=None)
Codec.rotated_rect(n=12, theta_bits=5, palette=None)
Codec.pixel(n=12, grid_aspect=None, palette=None)
```

All factories accept an optional `palette` keyword to switch to palette colour
mode. `Codec.dct()` ignores palettes.

### `Codec.preset(p)`

```python
Codec.preset(Preset.LARGE_TRIANGLE)
```

### `Codec.raw(...)`

Low-level escape hatch exposing every SPEC field.

```python
Codec.raw(
    shape="triangle",
    n_shapes=12,
    cx_bits=5, cy_bits=5,
    r_bits=4, alpha_bits=3, color_bits=16,
    theta_bits=5,
    palette=None, palette_k=None,
    grid_aspect=None,
)
```

### Methods

| Method                       | Description                                            |
| ---------------------------- | ------------------------------------------------------ |
| `codec.bytes_total()`        | Total hash bytes for this codec                        |
| `codec.is_palette_mode()`    | `bool` — whether colour comes from a palette           |
| `codec.with_palette(p)`      | Return a clone with palette colour mode                |

## `Preset`

```python
# Size axis: small (n=12, pixel n=16) / medium (n=24) / large (n=64)
Preset.DCT
Preset.SMALL_TRIANGLE
Preset.SMALL_CIRCLE
Preset.SMALL_PIXEL
Preset.SMALL_RECT
Preset.SMALL_SQUARE
Preset.MEDIUM_TRIANGLE
Preset.MEDIUM_CIRCLE
Preset.MEDIUM_PIXEL
Preset.MEDIUM_RECT
Preset.MEDIUM_SQUARE
Preset.LARGE_TRIANGLE
Preset.LARGE_CIRCLE
Preset.LARGE_PIXEL
Preset.LARGE_RECT
Preset.LARGE_SQUARE

# Deprecated pre-0.3 aliases — kept for source compatibility.
# Preset.TINY_DCT, Preset.PLACEHOLDER_*, Preset.DETAIL_*
```

## `Palette`

```python
Palette.from_rgb([(r, g, b), ...])   # K must be a power of 2 in [2, 1024]
Palette.from_hex(["#aabbcc", ...])

# Bundled constants
from arthash.palettes import PICO8, GAMEBOY, NES, MORANDI, MONO
```

## `EncodeOptions` / search budget

The `search` keyword on `encode` accepts a dict mirroring the Rust struct:

```python
encode(
    img, codec,
    seed=0,
    search={
        "strategy": "primitive",       # or "topk_uniform"
        "n_random": 64,
        "n_topk": 8,
        "hill_climb_steps": 100,
        "hill_climb_max_age": None,
        "n_attempts": 3,
    },
)
```

Like the TS binding, these affect encoder cost and quality only — the byte
format is identical regardless.

## Examples

```python
from arthash import Codec, Preset, encode, decode, to_svg
from arthash.palettes import PICO8

# 1. Smallest — DCT, ~21 bytes
hash_bytes = encode("photo.jpg")
print(len(hash_bytes))                              # ~21

# 2. Named preset
codec = Codec.preset(Preset.LARGE_TRIANGLE)
hash_bytes = encode("photo.jpg", codec)
svg = to_svg(hash_bytes, codec, base_size=512, blur=8.0)

# 3. Palette mode — retro look
codec = Codec.triangle(n=24, palette=PICO8)
hash_bytes = encode("photo.jpg", codec)

# 4. Decode at display size
w, h, rgba = decode(hash_bytes, codec, base_size=512)
# rgba shape (512, 512, 4)
```

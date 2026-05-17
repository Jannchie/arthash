---
name: arthash-python
description: arthash Python SDK — PyO3 wheel. NumPy-aware encode/decode, PIL-friendly inputs. Same byte format as the TypeScript and Rust SDKs.
---

# arthash — Python SDK

`pip install arthash` (or `uv add arthash`). A PyO3 wheel that wraps `arthash-rs` directly; encode/decode performance is within microseconds of the Rust crate.

## Runtime requirements

- Python ≥ 3.9.
- Pre-built wheels are published for CPython 3.9–3.13 on Linux x86_64 / aarch64, macOS x86_64 / arm64, and Windows x86_64. On other targets, the source build needs a Rust toolchain.
- `PIL` (Pillow) is used for file/array input resizing and is declared as a runtime dependency.

## Recommended starting point

Start with the default rectangle codec — small bytes, SVG output, no palette setup:

```python
from arthash import Codec, encode, decode, to_svg

codec = Codec.rect(n=24)                            # ~119 B, recognisable rectangle mosaic

hash_bytes = encode("photo.jpg", codec)
w, h, rgba = decode(hash_bytes, codec, base_size=512)   # ndarray shape (h, w, 4)
svg = to_svg(hash_bytes, codec, base_size=512, blur=4.0)
```

`Codec.rect(n=24)` is a good default for most placeholder slots: ~120 B, SVG output you can write straight into a template, no palette setup. Scale `n` up (48 / 64) for hero images, down (12) when you want to match a thumbhash-like 60-byte budget.

## End-to-end — encode, store, decode

```python
import base64
from arthash import Codec, encode, decode, to_svg

codec = Codec.rect(n=24)

# `encode` accepts a file path, a PIL.Image, or a numpy ndarray (H, W, 3|4).
hash_bytes: bytes = encode("photo.jpg", codec)

# Persist as base64 in a regular text column:
stored = base64.b64encode(hash_bytes).decode()

# Later: decode the placeholder.
restored = base64.b64decode(stored)
w, h, rgba = decode(restored, codec, base_size=512)
# rgba is an ndarray of shape (h, w, 4), dtype uint8 — Pillow / OpenCV-ready.

# Or render the same codec as an inline SVG (shape modes only):
svg: str = to_svg(restored, codec, base_size=512, blur=4.0)
```

## Codec factories and presets

```python
Codec.rect(n=24)             # axis-aligned rectangles — recommended default
Codec.square(n=24)           # squares
Codec.rotated_rect(n=24)     # rotated rectangles
Codec.triangle(n=24)         # triangle mosaic
Codec.circle(n=24)           # overlapping circles
Codec.pixel(n=16)            # palette pixel mosaic
Codec.dct()                  # blurry frequency-domain placeholder — see below
Codec.preset(Preset.MEDIUM_RECT)   # (or any other named preset)
```

DCT mode is intentionally not the default — reach for it only when you specifically want a **blurry, blurhash/thumbhash-style** look at the smallest possible byte budget. `to_svg` on a DCT hash raises; render via `decode` and paint to PNG / a canvas instead.

## Palettes

```python
from arthash import Codec
from arthash.palettes import PICO8        # also: NES, GAMEBOY, MORANDI, …

codec = Codec.rect(n=24, palette=PICO8)   # 4-bit colour per rect
hash_bytes = encode("photo.jpg", codec)
```

Custom palettes: pass a `(K, 3)` uint8 ndarray where `K ∈ {2, 4, 8, 16, 32}`. The decoder MUST receive the same palette — `arthash.palettes.list_presets()` / `get(name)` give you the bundled ones.

## Working with NumPy and PIL

```python
import numpy as np
from PIL import Image
from arthash import Codec, encode

codec = Codec.rect(n=24)

# Direct numpy input — H x W x 3 (RGB) or H x W x 4 (RGBA), dtype uint8.
img = np.asarray(Image.open("photo.jpg").convert("RGB"))
hash_bytes = encode(img, codec)

# PIL image input — auto-converted to RGB.
hash_bytes = encode(Image.open("photo.jpg"), codec)
```

`encode` resizes the input internally to the codec's expected thumbnail size (48 px long-edge for shape/PIXEL modes, ≤ 100 px for DCT). You can pass full-resolution input; you don't need to thumbnail first.

For the low-level path that does **not** resize, use `encode_rgb(rgb_bytes, width, height, codec)` — your buffer must already match the encoder's expected thumbnail size.

## Common Python-side pitfalls

- **Decoding with a different codec than was used to encode.** The bytes are not self-describing — you must pair `(hash, codec)`. Store the codec discriminator alongside the hash if you support more than one.
- **Calling `to_svg` on a `DCT` or `PIXEL` hash.** Raises — both modes are inherently raster.
- **Passing the wrong shape to `encode_rgb`.** The buffer length must equal `width * height * 3`; mismatches raise `ValueError`.
- **Expecting decode to upscale crisply.** It won't — arthash is a placeholder, not a super-resolution tool.

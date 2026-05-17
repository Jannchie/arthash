"""Unified encode/decode/to_svg dispatcher — adapts the Rust `_native`
extension to the Python-friendly API expected by callers (PIL/path/ndarray
inputs, dataclass codec, numpy outputs).

Public API:
    encode(image, codec=DEFAULT_CODEC, *, seed=0, target_size=None, search=None) -> bytes
    decode(hash_bytes, codec=DEFAULT_CODEC, *, base_size=256, override_aspect=None,
           pixel_smooth="nearest") -> (w, h, rgba_ndarray)
    to_svg(hash_bytes, codec=DEFAULT_CODEC, *, base_size=256, override_aspect=None,
           blur=0.0) -> str

Return type of `decode`:
    Always `(w, h, np.ndarray (h, w, 4) RGBA uint8)`. Alpha is 255 for every
    codec except DCT-with-alpha; the unified shape lets callers write the
    same downstream code regardless of codec mode.
"""

from __future__ import annotations

from pathlib import Path
from typing import Optional, Tuple, Union

import numpy as np
from PIL import Image as PILImage

from . import _native
from ._codec import DEFAULT_CODEC, Codec, ShapeType
from ._search import SearchOptions

# Encoder-side thumbnail targets (search-quality knobs, not byte-format).
DCT_THUMB = 100
SHAPE_THUMB = 48

ImageInput = Union[str, Path, PILImage.Image, np.ndarray]


def _load_rgb_thumb(image: ImageInput, target_size: int) -> Tuple[np.ndarray, int, int]:
    """(image, target) → (rgb_array (h, w, 3) uint8, w_orig, h_orig)."""
    if isinstance(image, (str, Path)):
        with PILImage.open(image) as im:
            im = im.convert("RGB")
            w_orig, h_orig = im.size
    elif isinstance(image, PILImage.Image):
        im = image.convert("RGB")
        w_orig, h_orig = im.size
    else:
        arr = np.asarray(image)
        if arr.ndim == 3 and arr.shape[2] == 4:
            arr = arr[..., :3]
        h_orig, w_orig = arr.shape[:2]
        im = PILImage.fromarray(arr).convert("RGB")

    longest = max(w_orig, h_orig)
    if longest > target_size:
        scale = target_size / longest
        tw = max(1, round(w_orig * scale))
        th = max(1, round(h_orig * scale))
        if (tw, th) != im.size:
            im = im.resize((tw, th), PILImage.LANCZOS)
    return np.asarray(im, dtype=np.uint8), w_orig, h_orig


def encode(
    image: ImageInput,
    codec: Codec = DEFAULT_CODEC,
    *,
    seed: int = 0,
    target_size: Optional[int] = None,
    search: Optional[SearchOptions] = None,
) -> bytes:
    """Encode an image to a placeholder hash under the given Codec.

    The same Codec value MUST be passed to `decode` — the byte stream is NOT
    self-describing. See docs/SPEC.md for byte layouts.

    `target_size` overrides the encoder thumbnail long-edge. `None` ⇒ the
    codec-natural default (100 for DCT, 48 for shape / PIXEL).

    `search` (a SearchOptions instance) controls the per-shape search budget
    for CIRCLE/TRIANGLE/SQUARE/RECT/ROTATED_RECT. Ignored for DCT/PIXEL.
    """
    if not isinstance(codec, Codec):
        raise TypeError(f"codec must be a Codec instance; got {type(codec).__name__}")

    default_target = DCT_THUMB if codec.shape == ShapeType.DCT else SHAPE_THUMB
    target = default_target if target_size is None else int(target_size)
    arr, _w_orig, _h_orig = _load_rgb_thumb(image, target)
    h, w = arr.shape[:2]
    rgb_bytes = arr.tobytes()

    codec_dict = codec.to_native_dict()
    search_dict = search.to_native_dict() if search is not None else None

    return _native.encode_rgb(rgb_bytes, w, h, codec_dict, seed, search_dict)


def decode(
    hash_bytes: bytes,
    codec: Codec = DEFAULT_CODEC,
    *,
    base_size: int = 256,
    override_aspect: Optional[float] = None,
    pixel_smooth: str = "nearest",
    aa: int = 1,
) -> Tuple[int, int, np.ndarray]:
    """Decode hash bytes to an RGBA preview at `base_size` long-edge.

    Returns `(width, height, rgba_ndarray)` where `rgba_ndarray.shape == (h, w, 4)`
    uint8. Alpha is 255 for every codec mode except DCT-with-alpha.

    `aa` (shape-mode supersample factor, 1 = off). `pixel_smooth` is
    `"nearest"` (default) or `"bilinear"` — PIXEL only.
    """
    if not isinstance(codec, Codec):
        raise TypeError(f"codec must be a Codec instance; got {type(codec).__name__}")

    codec_dict = codec.to_native_dict()
    w, h, rgba_bytes = _native.decode(
        bytes(hash_bytes),
        codec_dict,
        int(base_size),
        override_aspect,
        pixel_smooth,
        int(aa),
    )
    arr = np.frombuffer(rgba_bytes, dtype=np.uint8).reshape(h, w, 4).copy()
    return w, h, arr


def to_svg(
    hash_bytes: bytes,
    codec: Codec = DEFAULT_CODEC,
    *,
    base_size: int = 256,
    override_aspect: Optional[float] = None,
    blur: float = 0.0,
) -> str:
    """Render a shape-mode hash as a compact SVG string.

    Supported shapes: CIRCLE, TRIANGLE, SQUARE, RECT, ROTATED_RECT.
    Raises NotImplementedError for DCT and PIXEL (no natural SVG form).
    """
    if not isinstance(codec, Codec):
        raise TypeError(f"codec must be a Codec instance; got {type(codec).__name__}")

    codec_dict = codec.to_native_dict()
    return _native.to_svg(
        bytes(hash_bytes),
        codec_dict,
        int(base_size),
        override_aspect,
        float(blur),
    )

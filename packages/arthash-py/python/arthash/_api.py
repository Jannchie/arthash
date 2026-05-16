"""Unified encode/decode/to_svg dispatcher — adapts the Rust `_native`
extension to the Python-friendly API expected by callers (PIL/path/ndarray
inputs, dataclass codec, numpy outputs).

Public API:
    encode(image, codec=DEFAULT_CODEC, *, seed=0, target_size=100, search=None) -> bytes
    decode(hash_bytes, codec=DEFAULT_CODEC, *, base_size=256, override_aspect=None,
           aa=True, pixel_smooth="nearest") -> (w, h, pixels)
    to_svg(hash_bytes, codec=DEFAULT_CODEC, *, base_size=256, override_aspect=None,
           blur=0.0) -> str

Return type of `decode`:
    DCT mode   → pixels: bytes (raw RGBA buffer of length 4*w*h)
    shape mode → pixels: np.ndarray of shape (h, w, 3) RGB uint8

`aa` is accepted for backward compatibility but ignored (the Rust core
always uses anti-aliased shape rasterization).
"""

from __future__ import annotations

from pathlib import Path
from typing import Optional, Tuple, Union

import numpy as np
from PIL import Image as PILImage

from . import _native
from ._codec import DEFAULT_CODEC, Codec, ShapeType
from ._search import SearchOptions

DCT_THUMB = 100
SHAPE_THUMB = 48

ImageInput = Union[str, Path, PILImage.Image, np.ndarray]


def _load_rgb_thumb(image: ImageInput, target_size: int) -> Tuple[np.ndarray, int, int]:
    """(image, target) → (rgb_array (h, w, 3) uint8, w_orig, h_orig).

    The image is resized so `max(w, h) == target_size` using Lanczos. If
    already smaller, it's returned at its native size.
    """
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
    target_size: int = DCT_THUMB,
    search: Optional[SearchOptions] = None,
) -> bytes:
    """Encode an image to a placeholder hash under the given Codec.

    The same Codec value MUST be passed to `decode` — the byte stream is NOT
    self-describing. See docs/SPEC.md for byte layouts.

    `search` (a SearchOptions instance) controls the per-shape search budget
    for CIRCLE/TRIANGLE modes. Ignored for DCT/PIXEL. None ⇒ Rust default.
    """
    if not isinstance(codec, Codec):
        raise TypeError(f"codec must be a Codec instance; got {type(codec).__name__}")

    target = target_size if codec.shape == ShapeType.DCT else SHAPE_THUMB
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
    aa: bool = True,  # noqa: ARG001 — accepted for API parity, ignored by Rust core
    pixel_smooth: str = "nearest",
):
    """Decode hash bytes to a preview image at `base_size` long-edge.

    DCT returns bytes (raw RGBA); shape modes return a (h, w, 3) RGB ndarray.
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
    )

    if codec.shape == ShapeType.DCT:
        return w, h, rgba_bytes

    # Shape modes historically returned (h, w, 3) RGB.
    arr = np.frombuffer(rgba_bytes, dtype=np.uint8).reshape(h, w, 4)
    return w, h, np.ascontiguousarray(arr[..., :3])


def to_svg(
    hash_bytes: bytes,
    codec: Codec = DEFAULT_CODEC,
    *,
    base_size: int = 256,
    override_aspect: Optional[float] = None,
    blur: float = 0.0,
) -> str:
    """Render a CIRCLE / TRIANGLE hash as a compact SVG string.

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

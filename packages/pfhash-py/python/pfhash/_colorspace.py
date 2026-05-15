"""Minimal sRGB → linear-RGB helper used by `Codec.palette_linear`.

The full color-space machinery now lives in Rust; we keep this tiny piece
in Python because `palette_linear` is a derived property of the Codec
dataclass and is consumed by Python-side callers (research scripts that
inspect what the palette looks like in linear space).
"""

from __future__ import annotations

import numpy as np


def srgb_u8_to_linear(arr: np.ndarray) -> np.ndarray:
    """Per-pixel sRGB u8 → linear-RGB float32, matching the SPEC formula."""
    x = arr.astype(np.float32) / 255.0
    out = np.where(x <= 0.04045, x / 12.92, ((x + 0.055) / 1.055) ** 2.4)
    return out.astype(np.float32)

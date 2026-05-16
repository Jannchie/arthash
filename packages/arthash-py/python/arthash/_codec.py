"""Codec dataclass + ShapeType enum — see docs/SPEC.md.

The Codec is the byte-format contract shared between encoder and decoder.
Two codecs are byte-compatible iff their `shape`, `n_shapes`, all bit widths,
and `palette[:palette_k]` agree. Hash bytes contain only image-dependent
data; the Codec itself is consensus knowledge, not stored in the hash.

This module is a thin Python layer in front of the Rust codec: the dataclass
validates fields and exposes derived properties (`bytes_total`, etc.), but
all actual encode/decode work crosses the FFI boundary via `_native`.
"""

from __future__ import annotations

import enum
import math
from dataclasses import dataclass
from functools import cached_property
from typing import Optional

import numpy as np


class ShapeType(str, enum.Enum):
    DCT = "dct"
    CIRCLE = "circle"
    TRIANGLE = "triangle"
    PIXEL = "pixel"
    SQUARE = "square"
    RECT = "rect"
    ROTATED_RECT = "rotrect"


VALID_PALETTE_K = {2, 4, 8, 16, 32, 64, 128, 256, 512, 1024}


@dataclass(frozen=True)
class Codec:
    """Byte-format spec. See docs/SPEC.md §2 for full field semantics."""

    shape: ShapeType = ShapeType.DCT

    n_shapes: int = 12
    cx_bits: int = 5
    cy_bits: int = 5
    # CIRCLE: radius bits. SQUARE: side bits. RECT/ROTATED_RECT: per-axis
    # extent bits (width and height each get this many).
    r_bits: int = 4
    alpha_bits: int = 3
    color_bits: int = 16            # 16 = RGB-565, 24 = RGB-888
    # ROTATED_RECT only: bits for theta in [0, π). Unused by other modes.
    theta_bits: int = 5

    palette: Optional[np.ndarray] = None     # (K, 3) uint8 sRGB
    palette_k: Optional[int] = None
    alpha_levels: Optional[np.ndarray] = None
    grid_aspect: Optional[float] = None      # PIXEL only

    def __post_init__(self) -> None:
        if isinstance(self.shape, str):
            object.__setattr__(self, "shape", ShapeType(self.shape))

        if self.palette is not None:
            if self.palette.dtype != np.uint8 or self.palette.shape[-1] != 3:
                raise ValueError(
                    f"palette must be (K, 3) uint8 sRGB; got shape={self.palette.shape} "
                    f"dtype={self.palette.dtype}"
                )
            K_total = int(self.palette.shape[0])
            effective = self.palette_k if self.palette_k is not None else K_total
            if effective > K_total:
                raise ValueError(f"palette_k={effective} > palette length={K_total}")
            if effective not in VALID_PALETTE_K:
                raise ValueError(
                    f"palette_k must be a power of 2 in {sorted(VALID_PALETTE_K)}; got {effective}"
                )
            object.__setattr__(self, "palette_k", effective)
        elif self.palette_k is not None:
            raise ValueError("palette_k requires palette to be set")

        if self.color_bits not in (16, 24):
            raise ValueError("color_bits must be 16 (RGB-565) or 24 (RGB-888)")

        n_alpha = 1 << self.alpha_bits
        if self.alpha_levels is None:
            object.__setattr__(
                self, "alpha_levels",
                np.linspace(0.20, 0.90, n_alpha, dtype=np.float32),
            )
        elif len(self.alpha_levels) != n_alpha:
            raise ValueError(
                f"alpha_levels length ({len(self.alpha_levels)}) must equal "
                f"1<<alpha_bits ({n_alpha})"
            )

    # ---------- derived properties ----------

    @property
    def is_palette_mode(self) -> bool:
        return self.palette is not None

    @cached_property
    def palette_bits(self) -> int:
        if not self.is_palette_mode:
            return 0
        return int(round(math.log2(int(self.palette_k))))

    @property
    def color_field_bits(self) -> int:
        return self.palette_bits if self.is_palette_mode else self.color_bits

    @property
    def per_shape_bits(self) -> int:
        cx, cy, r = self.cx_bits, self.cy_bits, self.r_bits
        col, a = self.color_field_bits, self.alpha_bits
        if self.shape in (ShapeType.CIRCLE, ShapeType.SQUARE):
            # circle radius / square side share the same single-extent layout
            return cx + cy + r + col + a
        if self.shape == ShapeType.RECT:
            # independent width + height
            return cx + cy + 2 * r + col + a
        if self.shape == ShapeType.ROTATED_RECT:
            return cx + cy + 2 * r + self.theta_bits + col + a
        if self.shape == ShapeType.TRIANGLE:
            return 3 * (cx + cy) + col + a
        if self.shape == ShapeType.PIXEL:
            return col
        if self.shape == ShapeType.DCT:
            return 0
        raise ValueError(f"unknown shape: {self.shape}")

    @property
    def header_bits(self) -> int:
        if self.shape == ShapeType.DCT:
            return 40
        if self.shape == ShapeType.PIXEL:
            return 8
        return 8 + self.color_field_bits

    def bytes_total(self, has_alpha: bool = False) -> int:
        if self.shape == ShapeType.DCT:
            header = 48 if has_alpha else 40
            n_l_max = 28
            return math.ceil((header + 4 * (n_l_max + 16 + (24 if has_alpha else 0))) / 8)
        bits = self.header_bits + self.n_shapes * self.per_shape_bits
        return math.ceil(bits / 8)

    @cached_property
    def palette_linear(self) -> Optional[np.ndarray]:
        if not self.is_palette_mode:
            return None
        # Linear-RGB palette[:palette_k] as float32 (K, 3).
        from ._colorspace import srgb_u8_to_linear
        active = self.palette[: self.palette_k]
        return srgb_u8_to_linear(active.reshape(-1, 1, 3)).reshape(-1, 3).astype(np.float32)

    # ---------- FFI helper ----------

    def to_native_dict(self) -> dict:
        """Flatten into the dict shape `_native` expects. Used by `_api`."""
        out: dict = {
            "shape": self.shape.value,
            "n_shapes": int(self.n_shapes),
            "cx_bits": int(self.cx_bits),
            "cy_bits": int(self.cy_bits),
            "r_bits": int(self.r_bits),
            "alpha_bits": int(self.alpha_bits),
            "color_bits": int(self.color_bits),
            "theta_bits": int(self.theta_bits),
        }
        if self.palette is not None:
            pal = np.ascontiguousarray(self.palette, dtype=np.uint8)
            out["palette"] = bytes(pal.reshape(-1).tolist())
            if self.palette_k is not None:
                out["palette_k"] = int(self.palette_k)
        if self.alpha_levels is not None:
            out["alpha_levels"] = [float(x) for x in self.alpha_levels]
        if self.grid_aspect is not None:
            out["grid_aspect"] = float(self.grid_aspect)
        return out


DEFAULT_CODEC = Codec()

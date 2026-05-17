"""Codec dataclass + ShapeType / Preset enums — see docs/SPEC.md.

The Codec is the byte-format contract shared between encoder and decoder.
Two codecs are byte-compatible iff their `shape`, `n_shapes`, all bit widths,
and `palette[:palette_k]` agree. Hash bytes contain only image-dependent
data; the Codec itself is consensus knowledge, not stored in the hash.

Construct via the factory methods (recommended):

    Codec.dct()
    Codec.triangle(n=64)
    Codec.triangle(n=64, palette=PICO8)
    Codec.pixel(n=16, palette=PICO8, grid_aspect=1.5)
    Codec.preset(Preset.DETAIL_TRIANGLE)
"""

from __future__ import annotations

import enum
import math
from dataclasses import dataclass, replace
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


class Preset(str, enum.Enum):
    """Named codec recipes — battle-tested defaults you can drop in without
    understanding the byte format. See `Codec.preset()`.

    Roughly ordered by byte budget within each style: `TINY_DCT` (~21 B) →
    `PLACEHOLDER_*` (~50–80 B) → `MEDIUM_*` (~150 B) → `DETAIL_*` (~270–400 B).
    Actual byte counts vary ±1 B with image aspect.
    """

    TINY_DCT = "tiny_dct"
    PLACEHOLDER_TRIANGLE = "placeholder_triangle"
    PLACEHOLDER_CIRCLE = "placeholder_circle"
    PLACEHOLDER_PIXEL = "placeholder_pixel"
    MEDIUM_TRIANGLE = "medium_triangle"
    DETAIL_TRIANGLE = "detail_triangle"
    DETAIL_CIRCLE = "detail_circle"


VALID_PALETTE_K = {2, 4, 8, 16, 32, 64, 128, 256, 512, 1024}


@dataclass(frozen=True)
class Codec:
    """Byte-format spec. See docs/SPEC.md §2 for full field semantics.

    Prefer the factory methods (`Codec.dct()`, `Codec.triangle(n=64)`, etc.)
    over the raw dataclass constructor — they document which fields apply to
    each shape, while the constructor accepts every field for advanced use.
    """

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

    # ---------- factory methods ----------

    @classmethod
    def dct(cls) -> "Codec":
        """V4 thumbhash-style placeholder (~21 B). Default codec."""
        return cls(shape=ShapeType.DCT)

    @classmethod
    def circle(
        cls,
        n: int = 12,
        *,
        palette: Optional[np.ndarray] = None,
        color_bits: int = 16,
    ) -> "Codec":
        """SQIP-style overlapping circles."""
        return cls(
            shape=ShapeType.CIRCLE, n_shapes=n,
            palette=palette, color_bits=color_bits,
        )

    @classmethod
    def triangle(
        cls,
        n: int = 12,
        *,
        palette: Optional[np.ndarray] = None,
        color_bits: int = 16,
    ) -> "Codec":
        """Primitive-style triangle mosaic."""
        return cls(
            shape=ShapeType.TRIANGLE, n_shapes=n,
            palette=palette, color_bits=color_bits,
        )

    @classmethod
    def square(
        cls,
        n: int = 12,
        *,
        palette: Optional[np.ndarray] = None,
        color_bits: int = 16,
    ) -> "Codec":
        """Axis-aligned squares."""
        return cls(
            shape=ShapeType.SQUARE, n_shapes=n,
            palette=palette, color_bits=color_bits,
        )

    @classmethod
    def rect(
        cls,
        n: int = 12,
        *,
        palette: Optional[np.ndarray] = None,
        color_bits: int = 16,
    ) -> "Codec":
        """Axis-aligned rectangles."""
        return cls(
            shape=ShapeType.RECT, n_shapes=n,
            palette=palette, color_bits=color_bits,
        )

    @classmethod
    def rotated_rect(
        cls,
        n: int = 12,
        *,
        theta_bits: int = 5,
        palette: Optional[np.ndarray] = None,
        color_bits: int = 16,
    ) -> "Codec":
        """Rotated rectangles. `theta_bits` tunes the angle step (5 ⇒ ~5.6°)."""
        return cls(
            shape=ShapeType.ROTATED_RECT, n_shapes=n, theta_bits=theta_bits,
            palette=palette, color_bits=color_bits,
        )

    @classmethod
    def pixel(
        cls,
        n: int = 12,
        *,
        palette: Optional[np.ndarray] = None,
        color_bits: int = 16,
        grid_aspect: Optional[float] = None,
    ) -> "Codec":
        """Retro pixel mosaic. `grid_aspect` pins the grid shape."""
        return cls(
            shape=ShapeType.PIXEL, n_shapes=n,
            palette=palette, color_bits=color_bits,
            grid_aspect=grid_aspect,
        )

    @classmethod
    def preset(cls, p: Preset) -> "Codec":
        """Named codec recipe — see [`Preset`]."""
        if p == Preset.TINY_DCT:
            return cls.dct()
        if p == Preset.PLACEHOLDER_TRIANGLE:
            return cls.triangle(12)
        if p == Preset.PLACEHOLDER_CIRCLE:
            return cls.circle(12)
        if p == Preset.PLACEHOLDER_PIXEL:
            return cls.pixel(16)
        if p == Preset.MEDIUM_TRIANGLE:
            return cls.triangle(24)
        if p == Preset.DETAIL_TRIANGLE:
            return cls.triangle(64)
        if p == Preset.DETAIL_CIRCLE:
            return cls.circle(64)
        raise ValueError(f"unknown preset: {p}")

    # ---------- byte-compatibility check ----------

    def is_byte_compatible_with(self, other: "Codec") -> bool:
        """True iff hashes produced by `self` decode correctly under `other`.

        Compares only the SPEC fields that drive the byte layout (shape,
        n_shapes, all bit widths, active palette bytes). Stylistic deltas
        (whether you constructed via a factory or raw kwargs) don't matter.
        """
        if not isinstance(other, Codec):
            return NotImplemented  # type: ignore[return-value]
        if self.shape != other.shape:
            return False
        for f in ("n_shapes", "cx_bits", "cy_bits", "r_bits",
                  "alpha_bits", "color_bits", "theta_bits"):
            if getattr(self, f) != getattr(other, f):
                return False
        if self.is_palette_mode != other.is_palette_mode:
            return False
        if self.is_palette_mode:
            if self.palette_k != other.palette_k:
                return False
            a = self.palette[: self.palette_k]
            b = other.palette[: other.palette_k]
            if not np.array_equal(a, b):
                return False
        return True

    # ---------- serialization ----------

    def to_dict(self) -> dict:
        """JSON-safe dict for persistence (e.g. storing codec metadata in a
        DB column). Symmetric with [`Codec.from_dict`]."""
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
            out["palette_hex"] = [
                "{:02x}{:02x}{:02x}".format(int(r), int(g), int(b))
                for r, g, b in self.palette[: self.palette_k]
            ]
            out["palette_k"] = int(self.palette_k)
        if self.grid_aspect is not None:
            out["grid_aspect"] = float(self.grid_aspect)
        return out

    @classmethod
    def from_dict(cls, d: dict) -> "Codec":
        """Inverse of [`to_dict`]."""
        kwargs: dict = {
            "shape": ShapeType(d["shape"]),
            "n_shapes": int(d.get("n_shapes", 12)),
            "cx_bits": int(d.get("cx_bits", 5)),
            "cy_bits": int(d.get("cy_bits", 5)),
            "r_bits": int(d.get("r_bits", 4)),
            "alpha_bits": int(d.get("alpha_bits", 3)),
            "color_bits": int(d.get("color_bits", 16)),
            "theta_bits": int(d.get("theta_bits", 5)),
        }
        if "palette_hex" in d:
            hexes = d["palette_hex"]
            pal = np.zeros((len(hexes), 3), dtype=np.uint8)
            for i, h in enumerate(hexes):
                pal[i] = [int(h[0:2], 16), int(h[2:4], 16), int(h[4:6], 16)]
            kwargs["palette"] = pal
            if "palette_k" in d:
                kwargs["palette_k"] = int(d["palette_k"])
        if "grid_aspect" in d:
            kwargs["grid_aspect"] = float(d["grid_aspect"])
        return cls(**kwargs)

    # ---------- builders ----------

    def with_palette(self, palette: np.ndarray) -> "Codec":
        """Return a copy of this codec switched to palette indexing."""
        return replace(self, palette=palette)

    def with_color_bits(self, bits: int) -> "Codec":
        """Return a copy with continuous-color mode at the given bit depth."""
        return replace(self, palette=None, palette_k=None, color_bits=bits)

    # ---------- validation ----------

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
            return cx + cy + r + col + a
        if self.shape == ShapeType.RECT:
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
            out["palette"] = pal.reshape(-1).tobytes()
            if self.palette_k is not None:
                out["palette_k"] = int(self.palette_k)
        if self.alpha_levels is not None:
            out["alpha_levels"] = [float(x) for x in self.alpha_levels]
        if self.grid_aspect is not None:
            out["grid_aspect"] = float(self.grid_aspect)
        return out


DEFAULT_CODEC = Codec()

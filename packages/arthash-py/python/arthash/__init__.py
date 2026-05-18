"""arthash — placeholder-image hash family (PyO3 binding to arthash-rs).

Seven modes share one Codec API:

    * DCT          — V4 thumbhash-style hash (~21 B). Default codec.
    * CIRCLE       — SQIP-style overlapping circles.
    * TRIANGLE     — Primitive-style triangle mosaic.
    * SQUARE       — Axis-aligned squares (cx, cy, side).
    * RECT         — Axis-aligned rectangles (cx, cy, w, h).
    * ROTATED_RECT — Rotated rectangles (cx, cy, w, h, theta).
    * PIXEL        — Retro-palette pixel mosaic.

Quick start:

    from arthash import encode, decode, large_triangle

    hash_bytes = encode("photo.jpg", large_triangle())
    w, h, rgba = decode(hash_bytes, large_triangle(), base_size=512)

Or use the lower-level explicit Codec construction:

    from arthash import Codec, Preset

    codec = Codec.preset(Preset.LARGE_TRIANGLE)
    codec = Codec.triangle(n=24)

See docs/SPEC.md for the byte-format contract. The heavy work happens in
the bundled Rust extension; this Python layer owns input adaptation,
Codec validation, and the unified return-type contract.
"""

import warnings

from . import palettes
from .__about__ import __version__
from ._api import RenderStyle, decode, encode, to_svg
from ._codec import DEFAULT_CODEC, Codec, Preset, ShapeType
from ._search import DEFAULT_SEARCH, SearchOptions


# ---------- top-level preset shortcuts ----------
# These mirror Preset.* but spare you the `Codec.preset(Preset.X)` ceremony
# for the common case of "just give me the recommended codec for this slot".

def dct() -> Codec:
    """V4 thumbhash-style placeholder (~21 B). Smallest, blurriest."""
    return Codec.preset(Preset.DCT)


def small_triangle() -> Codec:
    """12-triangle mosaic, ~77 B. Lightweight SVG-friendly placeholder."""
    return Codec.preset(Preset.SMALL_TRIANGLE)


def small_circle() -> Codec:
    """12-circle mosaic, ~53 B. SQIP-style overlapping circles."""
    return Codec.preset(Preset.SMALL_CIRCLE)


def small_pixel() -> Codec:
    """16-cell PIXEL mosaic, ~33 B. Lo-fi mosaic look."""
    return Codec.preset(Preset.SMALL_PIXEL)


def small_rect() -> Codec:
    """12-rectangle mosaic. Axis-aligned rectangles."""
    return Codec.preset(Preset.SMALL_RECT)


def small_square() -> Codec:
    """12-square mosaic. Axis-aligned squares."""
    return Codec.preset(Preset.SMALL_SQUARE)


def medium_triangle() -> Codec:
    """24-triangle mosaic, ~150 B. Middle ground between Small and Large."""
    return Codec.preset(Preset.MEDIUM_TRIANGLE)


def medium_circle() -> Codec:
    """24-circle mosaic, ~102 B. Middle ground with circular brush feel."""
    return Codec.preset(Preset.MEDIUM_CIRCLE)


def medium_pixel() -> Codec:
    """24-cell PIXEL mosaic, ~49 B. Medium lo-fi mosaic."""
    return Codec.preset(Preset.MEDIUM_PIXEL)


def medium_rect() -> Codec:
    """24-rectangle mosaic. Middle ground axis-aligned rectangles."""
    return Codec.preset(Preset.MEDIUM_RECT)


def medium_square() -> Codec:
    """24-square mosaic. Middle ground axis-aligned squares."""
    return Codec.preset(Preset.MEDIUM_SQUARE)


def large_triangle() -> Codec:
    """64-triangle mosaic, ~395 B. Detail level — playground default."""
    return Codec.preset(Preset.LARGE_TRIANGLE)


def large_circle() -> Codec:
    """64-circle mosaic, ~267 B. Detail-level circular brush feel."""
    return Codec.preset(Preset.LARGE_CIRCLE)


def large_pixel() -> Codec:
    """64-cell PIXEL mosaic, ~129 B. Detail-level lo-fi mosaic."""
    return Codec.preset(Preset.LARGE_PIXEL)


def large_rect() -> Codec:
    """64-rectangle mosaic. Detail-level axis-aligned rectangles."""
    return Codec.preset(Preset.LARGE_RECT)


def large_square() -> Codec:
    """64-square mosaic. Detail-level axis-aligned squares."""
    return Codec.preset(Preset.LARGE_SQUARE)


# ---------- deprecated pre-0.3 shortcuts ----------
# Kept for source compatibility; emit DeprecationWarning on call.
# Will be removed in 1.0.

def tiny_dct() -> Codec:
    """Deprecated alias for [`dct`]."""
    warnings.warn(
        "`tiny_dct()` is deprecated, use `dct()` instead.",
        DeprecationWarning, stacklevel=2,
    )
    return dct()


def placeholder_triangle() -> Codec:
    """Deprecated alias for [`small_triangle`]."""
    warnings.warn(
        "`placeholder_triangle()` is deprecated, use `small_triangle()` instead.",
        DeprecationWarning, stacklevel=2,
    )
    return small_triangle()


def placeholder_circle() -> Codec:
    """Deprecated alias for [`small_circle`]."""
    warnings.warn(
        "`placeholder_circle()` is deprecated, use `small_circle()` instead.",
        DeprecationWarning, stacklevel=2,
    )
    return small_circle()


def placeholder_pixel() -> Codec:
    """Deprecated alias for [`small_pixel`]."""
    warnings.warn(
        "`placeholder_pixel()` is deprecated, use `small_pixel()` instead.",
        DeprecationWarning, stacklevel=2,
    )
    return small_pixel()


def detail_triangle() -> Codec:
    """Deprecated alias for [`large_triangle`]."""
    warnings.warn(
        "`detail_triangle()` is deprecated, use `large_triangle()` instead.",
        DeprecationWarning, stacklevel=2,
    )
    return large_triangle()


def detail_circle() -> Codec:
    """Deprecated alias for [`large_circle`]."""
    warnings.warn(
        "`detail_circle()` is deprecated, use `large_circle()` instead.",
        DeprecationWarning, stacklevel=2,
    )
    return large_circle()


def detail_pixel() -> Codec:
    """Deprecated alias for [`large_pixel`]."""
    warnings.warn(
        "`detail_pixel()` is deprecated, use `large_pixel()` instead.",
        DeprecationWarning, stacklevel=2,
    )
    return large_pixel()


__all__ = [
    "__version__",
    "Codec",
    "DEFAULT_CODEC",
    "DEFAULT_SEARCH",
    "Preset",
    "RenderStyle",
    "SearchOptions",
    "ShapeType",
    "decode",
    "encode",
    "palettes",
    "to_svg",
    # active preset shortcuts
    "dct",
    "small_triangle",
    "small_circle",
    "small_pixel",
    "small_rect",
    "small_square",
    "medium_triangle",
    "medium_circle",
    "medium_pixel",
    "medium_rect",
    "medium_square",
    "large_triangle",
    "large_circle",
    "large_pixel",
    "large_rect",
    "large_square",
    # deprecated pre-0.3 shortcuts
    "tiny_dct",
    "placeholder_triangle",
    "placeholder_circle",
    "placeholder_pixel",
    "detail_triangle",
    "detail_circle",
    "detail_pixel",
]

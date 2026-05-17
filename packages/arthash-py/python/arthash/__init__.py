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

    from arthash import encode, decode, detail_triangle

    hash_bytes = encode("photo.jpg", detail_triangle())
    w, h, rgba = decode(hash_bytes, detail_triangle(), base_size=512)

Or use the lower-level explicit Codec construction:

    from arthash import Codec, Preset

    codec = Codec.preset(Preset.DETAIL_TRIANGLE)
    codec = Codec.triangle(n=24)

See docs/SPEC.md for the byte-format contract. The heavy work happens in
the bundled Rust extension; this Python layer owns input adaptation,
Codec validation, and the unified return-type contract.
"""

from . import palettes
from .__about__ import __version__
from ._api import decode, encode, to_svg
from ._codec import DEFAULT_CODEC, Codec, Preset, ShapeType
from ._search import DEFAULT_SEARCH, SearchOptions


# ---------- top-level preset shortcuts ----------
# These mirror Preset.* but spare you the `Codec.preset(Preset.X)` ceremony
# for the common case of "just give me the recommended codec for this slot".

def tiny_dct() -> Codec:
    """V4 thumbhash-style placeholder (~21 B). Smallest, blurriest."""
    return Codec.preset(Preset.TINY_DCT)


def placeholder_triangle() -> Codec:
    """12-triangle mosaic, ~77 B. Lightweight SVG-friendly placeholder."""
    return Codec.preset(Preset.PLACEHOLDER_TRIANGLE)


def detail_triangle() -> Codec:
    """64-triangle mosaic, ~395 B. Playground default — recognisable preview."""
    return Codec.preset(Preset.DETAIL_TRIANGLE)


def placeholder_circle() -> Codec:
    """12-circle mosaic, ~53 B. SQIP-style overlapping circles."""
    return Codec.preset(Preset.PLACEHOLDER_CIRCLE)


def placeholder_pixel() -> Codec:
    """16-cell PIXEL mosaic, ~33 B. Lo-fi mosaic look."""
    return Codec.preset(Preset.PLACEHOLDER_PIXEL)


def medium_triangle() -> Codec:
    """24-triangle mosaic, ~150 B. Middle ground between Placeholder and Detail."""
    return Codec.preset(Preset.MEDIUM_TRIANGLE)


def medium_circle() -> Codec:
    """24-circle mosaic, ~102 B. Middle ground with circular brush feel."""
    return Codec.preset(Preset.MEDIUM_CIRCLE)


def medium_pixel() -> Codec:
    """24-cell PIXEL mosaic, ~49 B. Medium lo-fi mosaic."""
    return Codec.preset(Preset.MEDIUM_PIXEL)


def detail_circle() -> Codec:
    """64-circle mosaic, ~267 B. Detail-level circular brush feel."""
    return Codec.preset(Preset.DETAIL_CIRCLE)


def detail_pixel() -> Codec:
    """64-cell PIXEL mosaic, ~129 B. Detail-level lo-fi mosaic."""
    return Codec.preset(Preset.DETAIL_PIXEL)


__all__ = [
    "__version__",
    "Codec",
    "DEFAULT_CODEC",
    "DEFAULT_SEARCH",
    "Preset",
    "SearchOptions",
    "ShapeType",
    "decode",
    "encode",
    "palettes",
    "to_svg",
    # preset shortcuts
    "tiny_dct",
    "placeholder_triangle",
    "placeholder_circle",
    "placeholder_pixel",
    "medium_triangle",
    "medium_circle",
    "medium_pixel",
    "detail_triangle",
    "detail_circle",
    "detail_pixel",
]

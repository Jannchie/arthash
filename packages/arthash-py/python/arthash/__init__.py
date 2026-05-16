"""arthash — placeholder-image hash family (PyO3 binding to arthash-rs).

Four modes share one Codec API:

    * DCT      — V4 thumbhash-style hash (~21 B). Default codec.
    * CIRCLE   — SQIP-style overlapping circles.
    * TRIANGLE — Primitive-style triangle mosaic.
    * PIXEL    — Retro-palette pixel mosaic.

Quick start:

    from arthash import encode, decode, Codec, ShapeType

    hash_bytes = encode("photo.jpg")
    w, h, rgba = decode(hash_bytes, base_size=256)

    codec = Codec(shape=ShapeType.CIRCLE, n_shapes=12)
    hash_bytes = encode("photo.jpg", codec)
    w, h, rgb = decode(hash_bytes, codec, base_size=256)

See docs/SPEC.md for the byte-format contract. The heavy work happens in
the bundled Rust extension; this Python layer owns input adaptation,
Codec validation, and the historical return-type contract.
"""

from . import palettes
from .__about__ import __version__
from ._api import decode, encode, to_svg
from ._codec import DEFAULT_CODEC, Codec, ShapeType
from ._search import DEFAULT_SEARCH, SearchOptions

__all__ = [
    "__version__",
    "Codec",
    "DEFAULT_CODEC",
    "DEFAULT_SEARCH",
    "SearchOptions",
    "ShapeType",
    "decode",
    "encode",
    "palettes",
    "to_svg",
]

"""Codec dataclass validation + derived-property tests. See SPEC §2."""

from __future__ import annotations

import numpy as np
import pytest

from pfhash import Codec, DEFAULT_CODEC, ShapeType
from pfhash.palettes import PICO8


# ----------------------------- defaults -----------------------------

def test_default_codec_values():
    c = Codec()
    assert c.shape == ShapeType.DCT
    assert c.n_shapes == 12
    assert c.cx_bits == 5
    assert c.cy_bits == 5
    assert c.r_bits == 4
    assert c.alpha_bits == 3
    assert c.color_bits == 16
    assert c.palette is None


def test_default_codec_singleton():
    """DEFAULT_CODEC is a sharable frozen instance."""
    assert DEFAULT_CODEC.shape == ShapeType.DCT


# ----------------------------- validation -----------------------------

def test_string_shape_coerces_to_enum():
    c = Codec(shape="circle")
    assert c.shape == ShapeType.CIRCLE


def test_invalid_color_bits_rejected():
    with pytest.raises(ValueError, match="color_bits"):
        Codec(shape=ShapeType.CIRCLE, color_bits=12)


def test_palette_must_be_uint8():
    bad = np.zeros((16, 3), dtype=np.float32)
    with pytest.raises(ValueError, match="uint8"):
        Codec(shape=ShapeType.CIRCLE, palette=bad)


def test_palette_must_be_kx3():
    bad = np.zeros((16, 4), dtype=np.uint8)
    with pytest.raises(ValueError, match=r"palette must"):
        Codec(shape=ShapeType.CIRCLE, palette=bad)


def test_palette_k_must_be_power_of_two():
    pal = np.zeros((24, 3), dtype=np.uint8)
    with pytest.raises(ValueError, match="power of 2"):
        Codec(shape=ShapeType.CIRCLE, palette=pal)  # 24 is not a power of 2


def test_palette_k_overflows_palette():
    pal = np.zeros((16, 3), dtype=np.uint8)
    with pytest.raises(ValueError, match="palette_k=32 > palette length=16"):
        Codec(shape=ShapeType.CIRCLE, palette=pal, palette_k=32)


def test_palette_k_without_palette_rejected():
    with pytest.raises(ValueError, match="palette_k requires palette"):
        Codec(shape=ShapeType.CIRCLE, palette_k=16)


def test_alpha_levels_wrong_length_rejected():
    bad = np.array([0.5, 0.7], dtype=np.float32)  # 2 levels, but 1<<3 = 8 expected
    with pytest.raises(ValueError, match="alpha_levels length"):
        Codec(shape=ShapeType.CIRCLE, alpha_bits=3, alpha_levels=bad)


# ----------------------------- derived properties -----------------------------

def test_per_shape_bits_circle():
    c = Codec(shape=ShapeType.CIRCLE)
    # cx(5) + cy(5) + r(4) + color(16, no palette) + alpha(3) = 33
    assert c.per_shape_bits == 33


def test_per_shape_bits_triangle():
    c = Codec(shape=ShapeType.TRIANGLE)
    # 3*(5+5) + 16 + 3 = 49
    assert c.per_shape_bits == 49


def test_per_shape_bits_pixel():
    c = Codec(shape=ShapeType.PIXEL)
    # Just one color, no alpha
    assert c.per_shape_bits == 16  # color_bits


def test_palette_bits():
    c = Codec(shape=ShapeType.CIRCLE, palette=PICO8)
    assert c.palette_bits == 4  # log2(16)
    assert c.color_field_bits == 4


def test_palette_k_subset():
    """palette_k smaller than palette length uses only the first K."""
    pal = PICO8  # K=16
    c = Codec(shape=ShapeType.PIXEL, palette=pal, palette_k=8)
    assert c.palette_bits == 3
    assert c.palette_k == 8
    # palette_linear must reflect the subset
    assert c.palette_linear.shape == (8, 3)


def test_bytes_total_pixel_continuous():
    c = Codec(shape=ShapeType.PIXEL, n_shapes=12, color_bits=16)
    # header(8) + 12 cells × 16 bits = 8 + 192 = 200 bits = 25 bytes
    assert c.bytes_total() == 25


def test_bytes_total_pixel_palette():
    c = Codec(shape=ShapeType.PIXEL, n_shapes=16, palette=PICO8)
    # header(8) + 16 cells × 4 bits = 8 + 64 = 72 bits = 9 bytes
    assert c.bytes_total() == 9


def test_bytes_total_circle_continuous():
    c = Codec(shape=ShapeType.CIRCLE, n_shapes=8)
    # header(8 + 16) + 8 × 33 = 24 + 264 = 288 bits = 36 bytes
    assert c.bytes_total() == 36


def test_bytes_total_triangle_palette():
    c = Codec(shape=ShapeType.TRIANGLE, n_shapes=6, palette=PICO8)
    # header(8 + 4) + 6 × (3*10 + 4 + 3) = 12 + 6*37 = 12 + 222 = 234 bits = 30 bytes
    assert c.bytes_total() == 30


# ----------------------------- frozen behavior -----------------------------

def test_codec_is_frozen():
    c = Codec()
    with pytest.raises(Exception):
        c.n_shapes = 99  # type: ignore[misc]

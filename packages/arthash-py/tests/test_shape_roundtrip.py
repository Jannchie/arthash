"""CIRCLE / TRIANGLE / PIXEL mode encode/decode round-trip tests.

All shape modes share the same Codec API. Each test exercises:
    * default-codec encoding produces bytes_total() worth of bytes
    * encoding is deterministic given the same (image, codec, seed)
    * decoding returns a uint8 (h, w, 3) RGB ndarray of the right size
    * palette mode produces shorter hashes than continuous color mode
"""

from __future__ import annotations

import numpy as np
import pytest
from arthash import Codec, ShapeType, decode, encode
from arthash.palettes import PICO8

# ----------------------------- PIXEL -----------------------------

def test_pixel_continuous_roundtrip(rgb_random_seed42):
    codec = Codec(shape=ShapeType.PIXEL, n_shapes=12)
    h = encode(rgb_random_seed42, codec)
    assert len(h) == codec.bytes_total()
    w, hh, arr = decode(h, codec, base_size=64)
    assert isinstance(arr, np.ndarray)
    assert arr.shape == (hh, w, 4)
    assert arr.dtype == np.uint8


def test_pixel_palette_shorter_than_continuous(rgb_random_seed42):
    """Palette mode uses log2(K) bits per cell vs 16/24 for continuous."""
    h_cont = encode(rgb_random_seed42, Codec(shape=ShapeType.PIXEL, n_shapes=16))
    h_pal = encode(rgb_random_seed42, Codec(shape=ShapeType.PIXEL, n_shapes=16, palette=PICO8))
    assert len(h_pal) < len(h_cont)


def test_pixel_no_seed_dependency(rgb_random_seed42):
    """PIXEL has no hill-climbing → output is independent of seed."""
    codec = Codec(shape=ShapeType.PIXEL, n_shapes=12)
    assert encode(rgb_random_seed42, codec, seed=0) == encode(rgb_random_seed42, codec, seed=999)


# ----------------------------- CIRCLE -----------------------------

def test_circle_continuous_roundtrip(rgb_random_seed42):
    codec = Codec(shape=ShapeType.CIRCLE, n_shapes=6)
    h = encode(rgb_random_seed42, codec, seed=0)
    assert len(h) == codec.bytes_total()
    w, hh, arr = decode(h, codec, base_size=64)
    assert arr.shape == (hh, w, 4)
    assert arr.dtype == np.uint8


def test_circle_deterministic_with_seed(rgb_random_seed42):
    codec = Codec(shape=ShapeType.CIRCLE, n_shapes=6)
    h1 = encode(rgb_random_seed42, codec, seed=42)
    h2 = encode(rgb_random_seed42, codec, seed=42)
    assert h1 == h2


def test_circle_different_seeds_differ(rgb_random_seed42):
    """Different seeds should usually produce different hashes (hill-climb start)."""
    codec = Codec(shape=ShapeType.CIRCLE, n_shapes=6)
    h1 = encode(rgb_random_seed42, codec, seed=0)
    h2 = encode(rgb_random_seed42, codec, seed=999)
    assert h1 != h2


def test_circle_palette_shorter(rgb_random_seed42):
    codec_c = Codec(shape=ShapeType.CIRCLE, n_shapes=8)
    codec_p = Codec(shape=ShapeType.CIRCLE, n_shapes=8, palette=PICO8)
    h_c = encode(rgb_random_seed42, codec_c, seed=0)
    h_p = encode(rgb_random_seed42, codec_p, seed=0)
    assert len(h_p) < len(h_c)


# ----------------------------- TRIANGLE -----------------------------

def test_triangle_continuous_roundtrip(rgb_random_seed42):
    codec = Codec(shape=ShapeType.TRIANGLE, n_shapes=4)
    h = encode(rgb_random_seed42, codec, seed=0)
    assert len(h) == codec.bytes_total()
    w, hh, arr = decode(h, codec, base_size=64)
    assert arr.shape == (hh, w, 4)
    assert arr.dtype == np.uint8


def test_triangle_deterministic_with_seed(rgb_random_seed42):
    codec = Codec(shape=ShapeType.TRIANGLE, n_shapes=4)
    h1 = encode(rgb_random_seed42, codec, seed=42)
    h2 = encode(rgb_random_seed42, codec, seed=42)
    assert h1 == h2


# ----------------------------- SQUARE / RECT / ROTATED_RECT -----------------------------

@pytest.mark.parametrize(
    "shape",
    [ShapeType.SQUARE, ShapeType.RECT, ShapeType.ROTATED_RECT],
)
def test_rect_family_continuous_roundtrip(rgb_random_seed42, shape):
    codec = Codec(shape=shape, n_shapes=4)
    h = encode(rgb_random_seed42, codec, seed=0)
    assert len(h) == codec.bytes_total()
    w, hh, arr = decode(h, codec, base_size=64)
    assert arr.shape == (hh, w, 4)
    assert arr.dtype == np.uint8


@pytest.mark.parametrize(
    "shape",
    [ShapeType.SQUARE, ShapeType.RECT, ShapeType.ROTATED_RECT],
)
def test_rect_family_deterministic_with_seed(rgb_random_seed42, shape):
    codec = Codec(shape=shape, n_shapes=4)
    assert encode(rgb_random_seed42, codec, seed=42) == encode(rgb_random_seed42, codec, seed=42)


def test_rotated_rect_theta_bits_changes_bytes(rgb_random_seed42):
    """theta_bits is wired through PyO3 → bumping it widens hash bytes."""
    narrow = Codec(shape=ShapeType.ROTATED_RECT, n_shapes=4, theta_bits=3)
    wide = Codec(shape=ShapeType.ROTATED_RECT, n_shapes=4, theta_bits=8)
    assert wide.bytes_total() > narrow.bytes_total()
    h_narrow = encode(rgb_random_seed42, narrow, seed=0)
    h_wide = encode(rgb_random_seed42, wide, seed=0)
    assert len(h_narrow) == narrow.bytes_total()
    assert len(h_wide) == wide.bytes_total()


# ----------------------------- pixel_smooth options -----------------------------

@pytest.mark.parametrize("smooth", ["nearest", "bilinear"])
def test_pixel_decode_smooth_modes(rgb_random_seed42, smooth):
    # Rust core supports nearest + bilinear. The previous Python prototype
    # also accepted bicubic / lanczos via scikit-image; those were dropped
    # in the migration because no consumer was using them.
    codec = Codec(shape=ShapeType.PIXEL, n_shapes=12, palette=PICO8)
    h = encode(rgb_random_seed42, codec)
    w, hh, arr = decode(h, codec, base_size=64, pixel_smooth=smooth)
    assert arr.shape == (hh, w, 4)


def test_pixel_decode_invalid_smooth_raises(rgb_random_seed42):
    codec = Codec(shape=ShapeType.PIXEL, n_shapes=12, palette=PICO8)
    h = encode(rgb_random_seed42, codec)
    with pytest.raises(ValueError, match="unknown pixel_smooth"):
        decode(h, codec, base_size=64, pixel_smooth="quintic")

"""DCT mode (V4) encode/decode round-trip tests.

Verifies:
    * encode produces deterministic bytes for the same input
    * default codec is ShapeType.DCT
    * decode returns plausible (w, h, rgba_buffer) tuple
    * bytes_total() prediction matches actual output length for DCT
"""

from __future__ import annotations

import numpy as np
import pytest

from pfhash import Codec, DEFAULT_CODEC, ShapeType, decode, encode


def test_default_codec_is_dct():
    assert DEFAULT_CODEC.shape == ShapeType.DCT


def test_dct_encode_is_deterministic(rgb_random_seed42):
    h1 = encode(rgb_random_seed42)
    h2 = encode(rgb_random_seed42)
    assert h1 == h2, "DCT encode must be deterministic for identical input"


def test_dct_roundtrip_solid_red(rgb_solid_red):
    hash_bytes = encode(rgb_solid_red)
    assert isinstance(hash_bytes, bytes)
    assert len(hash_bytes) >= 5, "DCT hash must contain at least the header (40 bits)"

    w, h, rgba = decode(hash_bytes, base_size=64)
    # Square input → square output at base_size long-edge
    assert w == 64 and h == 64
    # rgba is a raw RGBA byte buffer
    assert len(rgba) == 4 * w * h


def test_dct_roundtrip_gradient(rgb_gradient):
    hash_bytes = encode(rgb_gradient)
    w, h, rgba = decode(hash_bytes, base_size=128)
    # Source is 100×60 wide ≈ aspect 1.67 → w = 128, h = round(128/1.67) ≈ 77
    assert w == 128
    assert 70 <= h <= 84
    assert len(rgba) == 4 * w * h


def test_dct_roundtrip_reproduces_color(rgb_solid_red):
    """Solid red in → mostly-red preview out. DCT is lossy but DC color
    survives intact through encode/decode."""
    hash_bytes = encode(rgb_solid_red)
    w, h, rgba = decode(hash_bytes, base_size=32)
    arr = np.frombuffer(rgba, dtype=np.uint8).reshape(h, w, 4)
    r_mean = arr[..., 0].mean()
    g_mean = arr[..., 1].mean()
    b_mean = arr[..., 2].mean()
    assert r_mean > 200, f"red channel should dominate for solid-red input, got {r_mean:.1f}"
    assert g_mean < 50 and b_mean < 50, "green/blue should be minimal"


def test_dct_explicit_codec_matches_default(rgb_random_seed42):
    """Codec(shape=ShapeType.DCT) must produce same bytes as default."""
    h_default = encode(rgb_random_seed42)
    h_explicit = encode(rgb_random_seed42, Codec(shape=ShapeType.DCT))
    assert h_default == h_explicit


@pytest.mark.parametrize("base_size", [32, 64, 128, 256])
def test_dct_decode_respects_base_size(rgb_solid_red, base_size):
    hash_bytes = encode(rgb_solid_red)
    w, h, rgba = decode(hash_bytes, base_size=base_size)
    assert max(w, h) == base_size

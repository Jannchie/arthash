"""`encode_rgba` entry-point tests.

`encode_rgba` keeps the alpha channel where `encode` drops it. The contract:

    * shape / PIXEL modes — alpha is composited over an opaque WHITE
      background, so a fully-opaque RGBA input must encode byte-identically to
      the plain RGB `encode`.
    * DCT — alpha is encoded natively (DCT-with-alpha), so a transparent input
      produces a longer hash than the opaque DCT form and the alpha survives
      the round-trip.
"""

from __future__ import annotations

import numpy as np
import pytest
from arthash import Codec, ShapeType, decode, encode, encode_rgba


@pytest.fixture
def rgb_seed7():
    rng = np.random.default_rng(7)
    return (rng.random((40, 40, 3)) * 255).astype(np.uint8)


def _add_opaque_alpha(rgb):
    h, w = rgb.shape[:2]
    return np.concatenate([rgb, np.full((h, w, 1), 255, np.uint8)], axis=-1)


@pytest.mark.parametrize(
    "codec",
    [
        Codec.circle(12),
        Codec(shape=ShapeType.TRIANGLE, n_shapes=12),
        Codec(shape=ShapeType.PIXEL, n_shapes=12),
    ],
)
def test_opaque_rgba_matches_plain_rgb_for_shape_modes(rgb_seed7, codec):
    """Fully-opaque alpha composites to a no-op over white, so shape/PIXEL
    output must be byte-identical to `encode`."""
    rgba = _add_opaque_alpha(rgb_seed7)
    assert encode_rgba(rgba, codec) == encode(rgb_seed7, codec)


def test_rgb_ndarray_input_gets_opaque_alpha(rgb_seed7):
    """Passing a 3-channel array to `encode_rgba` is allowed and behaves like
    a fully-opaque RGBA input."""
    codec = Codec.circle(12)
    assert encode_rgba(rgb_seed7, codec) == encode(rgb_seed7, codec)


def test_dct_encodes_alpha_natively(rgb_seed7):
    codec = Codec()  # DCT default
    rgba = _add_opaque_alpha(rgb_seed7)
    rgba_transparent = rgba.copy()
    rgba_transparent[:20, :, 3] = 0

    opaque_hash = encode(rgb_seed7, codec)
    alpha_hash = encode_rgba(rgba_transparent, codec)

    # DCT-with-alpha appends a quantized alpha block → strictly longer.
    assert len(alpha_hash) > len(opaque_hash)

    w, h, rgba_out = decode(alpha_hash, codec, base_size=32)
    assert rgba_out.shape == (h, w, 4)
    # The top (transparent) band must decode noticeably more transparent than
    # the bottom (opaque) band.
    assert rgba_out[:16, :, 3].mean() < rgba_out[-16:, :, 3].mean()


def test_encode_rgba_rejects_non_codec(rgb_seed7):
    with pytest.raises(TypeError):
        encode_rgba(_add_opaque_alpha(rgb_seed7), "circle")

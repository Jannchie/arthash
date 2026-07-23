"""`decode(dither=True)`: ordered Bayer 8x8 dithering at 8-bit quantization.

Dither is render-time only — it never touches the hash bytes. Default off
keeps decode output byte-stable; on, it may move each channel by at most
1 LSB (it only shifts the rounding threshold within one quantization step).
"""
import numpy as np
from arthash import Codec, RenderStyle, ShapeType, dct, decode, encode


def test_dither_default_off_is_byte_identical(rgb_gradient):
    codec = dct()
    h = encode(rgb_gradient, codec)
    _, _, plain = decode(h, codec)
    _, _, explicit_off = decode(h, codec, dither=False)
    np.testing.assert_array_equal(plain, explicit_off)


def test_dct_dither_within_one_lsb_and_deterministic(rgb_gradient):
    codec = dct()
    h = encode(rgb_gradient, codec)
    _, _, plain = decode(h, codec)
    _, _, dithered = decode(h, codec, dither=True)
    diff = np.abs(plain.astype(np.int16) - dithered.astype(np.int16))
    assert diff.max() <= 1
    # A smooth DCT gradient should actually get dithered somewhere.
    assert (diff != 0).sum() > 0
    _, _, again = decode(h, codec, dither=True)
    np.testing.assert_array_equal(dithered, again)


def test_shape_blur_dither_within_one_lsb(rgb_gradient):
    codec = Codec.triangle(n=12)
    h = encode(rgb_gradient, codec, seed=1)
    style = RenderStyle(blur=4.0)
    _, _, plain = decode(h, codec, style=style)
    _, _, dithered = decode(h, codec, style=style, dither=True)
    diff = np.abs(plain.astype(np.int16) - dithered.astype(np.int16))
    assert diff.max() <= 1


def test_sharp_shape_ignores_dither(rgb_gradient):
    codec = Codec.triangle(n=12)
    h = encode(rgb_gradient, codec, seed=1)
    _, _, plain = decode(h, codec)
    _, _, dithered = decode(h, codec, dither=True)
    np.testing.assert_array_equal(plain, dithered)


def test_dct_palette_render_quantizes_and_dithers(rgb_gradient):
    """A palette on a DCT codec is render-time display knowledge: decode
    quantizes to those colors (hard posterize; ordered dither with
    `dither=True`). Encode ignores it — same hash bytes either way."""
    palette = np.array(
        [[i * 32, 255 - i * 32, i * 17] for i in range(8)], dtype=np.uint8
    )
    plain_codec = Codec.dct()
    pal_codec = Codec(shape=ShapeType.DCT, palette=palette, palette_k=8)
    h = encode(rgb_gradient, plain_codec)
    assert h == encode(rgb_gradient, pal_codec)

    _, _, hard = decode(h, pal_codec)
    _, _, dithered = decode(h, pal_codec, dither=True)
    pal_set = {tuple(c) for c in palette}
    for out in (hard, dithered):
        colors = {tuple(px) for px in out[..., :3].reshape(-1, 3)}
        assert colors <= pal_set
    assert (hard != dithered).any(), "palette dither should visibly change output"

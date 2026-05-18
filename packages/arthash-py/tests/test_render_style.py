"""RenderStyle: blur + corner_radius on decode().

Style is independent of the codec byte format — the same `(hash, codec)`
with different styles produces visually distinct output without changing
the hash bytes. Default RenderStyle is zero-cost (fast-path).
"""
import warnings

import numpy as np
import pytest

from arthash import (
    Codec,
    RenderStyle,
    ShapeType,
    decode,
    encode,
    large_rect,
    large_triangle,
)


@pytest.fixture
def checkerboard_image() -> np.ndarray:
    h, w, cell = 48, 48, 8
    arr = np.empty((h, w, 3), dtype=np.uint8)
    for y in range(h):
        for x in range(w):
            on = ((x // cell) + (y // cell)) % 2 == 0
            arr[y, x] = (220, 60, 60) if on else (40, 80, 200)
    return arr


def test_default_style_byte_identical_to_no_style(checkerboard_image):
    """`RenderStyle()` (both fields zero) must take the fast path and
    produce decode output byte-identical to no style at all."""
    for codec in [
        Codec.triangle(n=12),
        Codec.circle(n=12),
        Codec.rect(n=12),
        Codec.square(n=12),
        Codec.rotated_rect(n=12),
        Codec.pixel(n=16),
        Codec.dct(),
    ]:
        h = encode(checkerboard_image, codec, seed=0)
        _, _, no_style = decode(h, codec)
        _, _, with_default = decode(h, codec, style=RenderStyle())
        np.testing.assert_array_equal(no_style, with_default)


def test_blur_changes_output(checkerboard_image):
    codec = large_triangle()
    h = encode(checkerboard_image, codec, seed=0)
    _, _, sharp = decode(h, codec)
    _, _, blurred = decode(h, codec, style=RenderStyle(blur=4))
    # At σ=4, blur should affect most pixels of a textured hash.
    diff_count = np.sum(np.any(sharp != blurred, axis=-1))
    total = sharp.shape[0] * sharp.shape[1]
    assert diff_count > total // 10


def test_corner_radius_changes_rect_output(checkerboard_image):
    codec = large_rect()
    h = encode(checkerboard_image, codec, seed=0)
    _, _, sharp = decode(h, codec)
    _, _, rounded = decode(h, codec, style=RenderStyle(corner_radius=12))
    # AA rounded path produces different pixel values than hard-edge.
    assert not np.array_equal(sharp, rounded)


def test_corner_radius_on_circle_warns_and_ignored(checkerboard_image):
    codec = Codec(shape=ShapeType.CIRCLE, n_shapes=8)
    h = encode(checkerboard_image, codec, seed=0)
    _, _, no_style = decode(h, codec)
    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        _, _, with_radius = decode(h, codec, style=RenderStyle(corner_radius=5))
    # corner_radius is dropped → output identical to no-style.
    np.testing.assert_array_equal(no_style, with_radius)
    assert any(
        issubclass(w.category, UserWarning) and "corner_radius" in str(w.message)
        for w in caught
    )


def test_corner_radius_on_triangle_warns_and_ignored(checkerboard_image):
    codec = Codec(shape=ShapeType.TRIANGLE, n_shapes=8)
    h = encode(checkerboard_image, codec, seed=0)
    _, _, no_style = decode(h, codec)
    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        _, _, with_radius = decode(h, codec, style=RenderStyle(corner_radius=5))
    np.testing.assert_array_equal(no_style, with_radius)
    assert any(issubclass(w.category, UserWarning) for w in caught)


def test_corner_radius_on_pixel_warns_and_ignored(checkerboard_image):
    codec = Codec(shape=ShapeType.PIXEL, n_shapes=12)
    h = encode(checkerboard_image, codec, seed=0)
    _, _, no_style = decode(h, codec)
    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        _, _, with_radius = decode(h, codec, style=RenderStyle(corner_radius=4))
    np.testing.assert_array_equal(no_style, with_radius)
    assert any(issubclass(w.category, UserWarning) for w in caught)


def test_pixel_blur_works(checkerboard_image):
    """PIXEL takes a different rasterization path (sRGB, not the float linear
    canvas the shape modes use). Blur must still apply post-rasterization."""
    codec = Codec(shape=ShapeType.PIXEL, n_shapes=16)
    h = encode(checkerboard_image, codec, seed=0)
    _, _, sharp = decode(h, codec)
    _, _, blurred = decode(h, codec, style=RenderStyle(blur=2))
    # PIXEL with textured input → cells have multiple colors → blur smears.
    assert not np.array_equal(sharp, blurred)


def test_render_style_dataclass_defaults():
    s = RenderStyle()
    assert s.blur == 0.0
    assert s.corner_radius == 0.0


def test_render_style_field_assignment():
    s = RenderStyle(blur=4.0, corner_radius=2.0)
    assert s.blur == 4.0
    assert s.corner_radius == 2.0

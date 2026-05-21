"""Codec dataclass validation + derived-property tests. See SPEC §2."""

from __future__ import annotations

import numpy as np
import pytest

from arthash import Codec, DEFAULT_CODEC, Preset, ShapeType
from arthash.palettes import PICO8


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


def test_palette_k_accepts_non_pow2():
    # K = 24 (non-power-of-2) is now valid. Codec uses ceil(log₂K) = 5 bits
    # per index, with bit patterns 24..31 unused.
    pal = np.zeros((24, 3), dtype=np.uint8)
    c = Codec(shape=ShapeType.CIRCLE, palette=pal)
    assert c.palette_k == 24
    assert c.palette_bits == 5


def test_palette_k_must_be_in_range():
    # K = 1 is too small (need at least 2 entries).
    pal = np.zeros((1, 3), dtype=np.uint8)
    with pytest.raises(ValueError, match=r"palette_k must be in"):
        Codec(shape=ShapeType.CIRCLE, palette=pal)


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


def test_per_shape_bits_square_matches_circle():
    """SQUARE shares CIRCLE's bit layout (single extent param)."""
    assert (
        Codec(shape=ShapeType.SQUARE).per_shape_bits
        == Codec(shape=ShapeType.CIRCLE).per_shape_bits
    )


def test_per_shape_bits_rect():
    c = Codec(shape=ShapeType.RECT)
    # cx(5) + cy(5) + 2*r(4) + color(16) + alpha(3) = 37
    assert c.per_shape_bits == 37


def test_per_shape_bits_rotated_rect_uses_theta_bits():
    c = Codec(shape=ShapeType.ROTATED_RECT)
    # RECT(37) + theta_bits(5) = 42
    assert c.per_shape_bits == 42
    # bumping theta_bits widens the per-shape budget
    c2 = Codec(shape=ShapeType.ROTATED_RECT, theta_bits=8)
    assert c2.per_shape_bits == 45


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


# ----------------------------- factory methods -----------------------------

def test_dct_factory():
    c = Codec.dct()
    assert c.shape == ShapeType.DCT


def test_triangle_factory_sets_n():
    c = Codec.triangle(64)
    assert c.shape == ShapeType.TRIANGLE
    assert c.n_shapes == 64


def test_circle_factory_with_palette():
    c = Codec.circle(8, palette=PICO8)
    assert c.shape == ShapeType.CIRCLE
    assert c.is_palette_mode


def test_rotated_rect_factory_default_theta_bits_5():
    c = Codec.rotated_rect(8)
    assert c.theta_bits == 5
    c2 = Codec.rotated_rect(8, theta_bits=7)
    assert c2.theta_bits == 7


def test_pixel_factory_grid_aspect():
    c = Codec.pixel(16, grid_aspect=1.5)
    assert c.grid_aspect == 1.5


def test_preset_large_triangle_is_n64():
    c = Codec.preset(Preset.LARGE_TRIANGLE)
    assert c.shape == ShapeType.TRIANGLE
    assert c.n_shapes == 64


def test_preset_rect_and_square_variants():
    # New rect / square presets reach the right factory at all 3 size tiers.
    assert Codec.preset(Preset.SMALL_RECT).shape == ShapeType.RECT
    assert Codec.preset(Preset.SMALL_RECT).n_shapes == 12
    assert Codec.preset(Preset.MEDIUM_RECT).n_shapes == 24
    assert Codec.preset(Preset.LARGE_RECT).n_shapes == 64
    assert Codec.preset(Preset.SMALL_SQUARE).shape == ShapeType.SQUARE
    assert Codec.preset(Preset.LARGE_SQUARE).n_shapes == 64


def test_preset_deprecated_aliases_equivalent():
    # Deprecated aliases produce byte-compatible codecs to their replacements.
    pairs = [
        (Preset.TINY_DCT, Preset.DCT),
        (Preset.PLACEHOLDER_TRIANGLE, Preset.SMALL_TRIANGLE),
        (Preset.PLACEHOLDER_CIRCLE, Preset.SMALL_CIRCLE),
        (Preset.PLACEHOLDER_PIXEL, Preset.SMALL_PIXEL),
        (Preset.DETAIL_TRIANGLE, Preset.LARGE_TRIANGLE),
        (Preset.DETAIL_CIRCLE, Preset.LARGE_CIRCLE),
        (Preset.DETAIL_PIXEL, Preset.LARGE_PIXEL),
    ]
    for old, new in pairs:
        assert Codec.preset(old).is_byte_compatible_with(Codec.preset(new)), (
            f"{old.name} != {new.name}"
        )


def test_with_palette_returns_new_codec():
    c = Codec.triangle(12)
    assert not c.is_palette_mode
    c2 = c.with_palette(PICO8)
    assert c2.is_palette_mode
    # original unchanged
    assert not c.is_palette_mode


def test_with_color_bits_drops_palette():
    c = Codec.triangle(12, palette=PICO8)
    assert c.is_palette_mode
    c2 = c.with_color_bits(24)
    assert not c2.is_palette_mode
    assert c2.color_bits == 24


# ----------------------------- top-level shortcuts -----------------------------

def test_top_level_shortcuts_match_preset():
    from arthash import (
        dct, small_triangle, small_circle, small_pixel,
        small_rect, small_square,
        medium_triangle, medium_circle, medium_pixel,
        medium_rect, medium_square,
        large_triangle, large_circle, large_pixel,
        large_rect, large_square,
    )
    pairs = [
        (dct, Preset.DCT),
        (small_triangle, Preset.SMALL_TRIANGLE),
        (small_circle, Preset.SMALL_CIRCLE),
        (small_pixel, Preset.SMALL_PIXEL),
        (small_rect, Preset.SMALL_RECT),
        (small_square, Preset.SMALL_SQUARE),
        (medium_triangle, Preset.MEDIUM_TRIANGLE),
        (medium_circle, Preset.MEDIUM_CIRCLE),
        (medium_pixel, Preset.MEDIUM_PIXEL),
        (medium_rect, Preset.MEDIUM_RECT),
        (medium_square, Preset.MEDIUM_SQUARE),
        (large_triangle, Preset.LARGE_TRIANGLE),
        (large_circle, Preset.LARGE_CIRCLE),
        (large_pixel, Preset.LARGE_PIXEL),
        (large_rect, Preset.LARGE_RECT),
        (large_square, Preset.LARGE_SQUARE),
    ]
    for fn, p in pairs:
        assert fn().is_byte_compatible_with(Codec.preset(p)), f"{fn.__name__} != {p.name}"


def test_deprecated_shortcuts_emit_warning():
    import pytest
    from arthash import (
        tiny_dct, placeholder_triangle, placeholder_circle, placeholder_pixel,
        detail_triangle, detail_circle, detail_pixel,
    )
    # Each deprecated shortcut emits DeprecationWarning and still returns a
    # codec byte-compatible with its new-name replacement.
    cases = [
        (tiny_dct, Preset.DCT),
        (placeholder_triangle, Preset.SMALL_TRIANGLE),
        (placeholder_circle, Preset.SMALL_CIRCLE),
        (placeholder_pixel, Preset.SMALL_PIXEL),
        (detail_triangle, Preset.LARGE_TRIANGLE),
        (detail_circle, Preset.LARGE_CIRCLE),
        (detail_pixel, Preset.LARGE_PIXEL),
    ]
    for fn, replacement in cases:
        with pytest.warns(DeprecationWarning):
            c = fn()
        assert c.is_byte_compatible_with(Codec.preset(replacement)), fn.__name__


# ----------------------------- byte compatibility -----------------------------

def test_factory_vs_kwargs_byte_compatible():
    a = Codec.triangle(24)
    b = Codec(shape=ShapeType.TRIANGLE, n_shapes=24)
    assert a.is_byte_compatible_with(b)


def test_different_codecs_not_byte_compatible():
    assert not Codec.triangle(12).is_byte_compatible_with(Codec.triangle(64))
    assert not Codec.triangle(12).is_byte_compatible_with(Codec.circle(12))


def test_palette_byte_compatibility_checks_active_bytes():
    pal = PICO8
    a = Codec.circle(8, palette=pal)
    b = Codec.circle(8, palette=pal)
    assert a.is_byte_compatible_with(b)


# ----------------------------- to_dict / from_dict -----------------------------

def test_to_dict_from_dict_roundtrip_basic():
    c = Codec.triangle(24)
    d = c.to_dict()
    c2 = Codec.from_dict(d)
    assert c.is_byte_compatible_with(c2)


def test_to_dict_from_dict_roundtrip_palette():
    c = Codec.circle(8, palette=PICO8)
    d = c.to_dict()
    assert "palette_hex" in d
    assert d["palette_k"] == 16
    c2 = Codec.from_dict(d)
    assert c.is_byte_compatible_with(c2)


def test_to_dict_pixel_with_grid_aspect():
    c = Codec.pixel(16, grid_aspect=1.5)
    d = c.to_dict()
    assert d["grid_aspect"] == 1.5
    c2 = Codec.from_dict(d)
    assert c2.grid_aspect == 1.5

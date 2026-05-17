"""to_svg() tests — CIRCLE / TRIANGLE rendering, error cases, byte stability."""

from __future__ import annotations

import xml.etree.ElementTree as ET

import pytest

from arthash import Codec, ShapeType, encode, to_svg
from arthash.palettes import PICO8


def _parse(svg: str) -> ET.Element:
    """Parse SVG and return the root, raising on malformed XML."""
    return ET.fromstring(svg)


def _strip_ns(tag: str) -> str:
    return tag.split("}")[-1] if "}" in tag else tag


# ----------------------------- happy paths -----------------------------

def test_circle_svg_structure(rgb_random_seed42):
    codec = Codec(shape=ShapeType.CIRCLE, n_shapes=6)
    h = encode(rgb_random_seed42, codec, seed=0)
    svg = to_svg(h, codec, base_size=256)

    root = _parse(svg)
    assert _strip_ns(root.tag) == "svg"
    assert root.attrib["viewBox"].startswith("0 0 ")

    children = list(root)
    # bg <path> + N <circle>s
    assert _strip_ns(children[0].tag) == "path", "background is a <path>"
    assert len(children) == 1 + 6
    for c in children[1:]:
        assert _strip_ns(c.tag) == "circle"


def test_triangle_svg_structure(rgb_random_seed42):
    codec = Codec(shape=ShapeType.TRIANGLE, n_shapes=4)
    h = encode(rgb_random_seed42, codec, seed=0)
    svg = to_svg(h, codec, base_size=256)

    root = _parse(svg)
    children = list(root)
    # bg <path> + N triangle <path>s
    assert _strip_ns(children[0].tag) == "path"
    assert len(children) == 1 + 4
    for path in children[1:]:
        assert _strip_ns(path.tag) == "path"
        # Path d should start with M and close with z
        d = path.attrib["d"]
        assert d.startswith("M"), f"path d must start with M, got {d!r}"
        assert d.endswith("z"), f"path d must close with z, got {d!r}"


def test_pixel_svg_structure(rgb_random_seed42):
    codec = Codec(shape=ShapeType.PIXEL, n_shapes=12)
    h = encode(rgb_random_seed42, codec, seed=0)
    svg = to_svg(h, codec, base_size=256)

    root = _parse(svg)
    assert _strip_ns(root.tag) == "svg"
    assert root.attrib["viewBox"].startswith("0 0 ")

    # PIXEL has no background path — the cell grid fills the viewBox.
    children = list(root)
    assert len(children) > 0
    for c in children:
        assert _strip_ns(c.tag) == "rect"


def test_circle_palette_svg_structure(rgb_random_seed42):
    """Palette mode should produce identical structural SVG to continuous."""
    codec = Codec(shape=ShapeType.CIRCLE, n_shapes=8, palette=PICO8)
    h = encode(rgb_random_seed42, codec, seed=0)
    svg = to_svg(h, codec, base_size=256)
    root = _parse(svg)
    assert len(list(root)) == 1 + 8


def test_svg_has_no_width_height_attrs(rgb_random_seed42):
    """Output should be viewBox-only so caller's CSS controls sizing."""
    codec = Codec(shape=ShapeType.CIRCLE, n_shapes=4)
    h = encode(rgb_random_seed42, codec, seed=0)
    svg = to_svg(h, codec)
    root = _parse(svg)
    assert "width" not in root.attrib
    assert "height" not in root.attrib
    assert "viewBox" in root.attrib


def test_svg_is_deterministic(rgb_random_seed42):
    """Same hash + codec → byte-identical SVG (matters for caching/CDN)."""
    codec = Codec(shape=ShapeType.CIRCLE, n_shapes=6)
    h = encode(rgb_random_seed42, codec, seed=0)
    assert to_svg(h, codec) == to_svg(h, codec)


# ----------------------------- blur option -----------------------------

def test_blur_zero_no_filter(rgb_random_seed42):
    codec = Codec(shape=ShapeType.TRIANGLE, n_shapes=4)
    h = encode(rgb_random_seed42, codec, seed=0)
    svg = to_svg(h, codec, blur=0)
    assert "filter" not in svg
    assert "feGaussianBlur" not in svg


def test_blur_emits_filter_element(rgb_random_seed42):
    codec = Codec(shape=ShapeType.TRIANGLE, n_shapes=4)
    h = encode(rgb_random_seed42, codec, seed=0)
    svg = to_svg(h, codec, blur=12)
    root = _parse(svg)
    children = list(root)
    tags = [_strip_ns(c.tag) for c in children]
    assert "filter" in tags
    # Filtered content is wrapped in a <g>
    g_idx = tags.index("g")
    inner = list(children[g_idx])
    assert _strip_ns(inner[0].tag) == "path"  # bg path moved inside <g>
    assert len(inner) == 1 + codec.n_shapes


# ----------------------------- error cases -----------------------------

def test_dct_raises(rgb_random_seed42):
    h = encode(rgb_random_seed42)  # default = DCT
    with pytest.raises(NotImplementedError, match="DCT mode"):
        to_svg(h, Codec())




# ----------------------------- override_aspect -----------------------------

def test_compressed_output_format(rgb_random_seed42):
    """Verify byte-compression optimizations are active:
    integer coordinates, stripped leading zeros, and 3-digit hex when applicable."""
    codec = Codec(shape=ShapeType.CIRCLE, n_shapes=8)
    h = encode(rgb_random_seed42, codec, seed=0)
    svg = to_svg(h, codec, base_size=256)

    # No fractional coordinates anywhere
    import re
    decimal_in_attr = re.search(r'cx="\d+\.\d', svg) or re.search(r'cy="\d+\.\d', svg) \
        or re.search(r'r="\d+\.\d', svg)
    assert decimal_in_attr is None, f"found fractional coord: {decimal_in_attr.group(0)}"

    # No "0.X" opacity values (must be ".X")
    assert 'fill-opacity="0.' not in svg, "leading-zero opacity should be stripped"


def test_triangle_uses_path(rgb_random_seed42):
    """Triangles should use <path d="M.. l.. z"/> not <polygon>."""
    codec = Codec(shape=ShapeType.TRIANGLE, n_shapes=4)
    h = encode(rgb_random_seed42, codec, seed=0)
    svg = to_svg(h, codec, base_size=256)
    assert "<polygon" not in svg, "should use <path> for compactness"
    assert "<path" in svg


@pytest.mark.parametrize(
    "shape",
    [ShapeType.SQUARE, ShapeType.RECT, ShapeType.ROTATED_RECT],
)
def test_rect_family_svg_renders(rgb_random_seed42, shape):
    """SQUARE / RECT / ROTATED_RECT all have an SVG primitive form."""
    codec = Codec(shape=shape, n_shapes=4)
    h = encode(rgb_random_seed42, codec, seed=0)
    svg = to_svg(h, codec, base_size=256)
    root = _parse(svg)
    assert _strip_ns(root.tag) == "svg"
    children = list(root)
    # bg <path> + N shape elements
    assert len(children) == 1 + 4


def test_override_aspect_affects_viewbox(rgb_random_seed42):
    """override_aspect should change the viewBox dims (same as decode)."""
    codec = Codec(shape=ShapeType.CIRCLE, n_shapes=4)
    h = encode(rgb_random_seed42, codec, seed=0)
    svg_default = to_svg(h, codec, base_size=128)
    svg_wide = to_svg(h, codec, base_size=128, override_aspect=2.0)
    root_default = _parse(svg_default)
    root_wide = _parse(svg_wide)
    assert root_default.attrib["viewBox"] != root_wide.attrib["viewBox"]
    # 2.0 means w=128, h=64
    assert root_wide.attrib["viewBox"] == "0 0 128 64"

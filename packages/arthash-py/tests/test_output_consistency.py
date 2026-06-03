"""Cross-output visual consistency: `decode()` raster vs `to_svg()` → resvg.

The contract: for the same `(hash, codec, style)`, all output paths must
produce visually equivalent results. Bit-exact match is not feasible
(SVG `<feGaussianBlur>` is engine-dependent; AA approximations differ),
but mean-absolute-difference across all channels should stay below ~8.

The test rasterizes the SVG through resvg (Rust-backed, deterministic) to
sidestep browser variability. Skips if `resvg_py` isn't installed.
"""
from __future__ import annotations

import io
from typing import Tuple

import numpy as np
import pytest
from arthash import Codec, RenderStyle, decode, encode, to_svg

resvg_py = pytest.importorskip("resvg_py")
PIL = pytest.importorskip("PIL")
from PIL import Image  # noqa: E402


def _svg_to_rgba(svg: str, w: int, h: int) -> np.ndarray:
    png_bytes = resvg_py.svg_to_bytes(svg_string=svg, width=w, height=h)
    img = Image.open(io.BytesIO(bytes(png_bytes))).convert("RGBA")
    arr = np.asarray(img, dtype=np.uint8)
    assert arr.shape == (h, w, 4), f"unexpected resvg shape {arr.shape}"
    return arr


def _mean_abs_diff(a: np.ndarray, b: np.ndarray) -> float:
    """Mean absolute difference across RGB channels (alpha excluded — shape
    codecs always emit 255, and resvg's premultiply may leave edges with
    slightly different alpha that's not meaningful for placeholder use)."""
    diff = np.abs(a[..., :3].astype(np.int16) - b[..., :3].astype(np.int16))
    return float(diff.mean())


@pytest.fixture
def textured_image() -> np.ndarray:
    """Non-uniform input so the encoder produces real shapes (uniform input
    → all shapes degenerate to background, making the comparison vacuous)."""
    rng = np.random.default_rng(42)
    arr = (rng.uniform(0, 255, (48, 48, 3))).astype(np.uint8)
    return arr


CONSISTENCY_THRESHOLD = 16.0
"""Mean-abs-diff ceiling across RGB channels (0-255). Hard threshold —
SVG-vs-raster has irreducible differences from AA / blur filter
implementations, plus our SVG emits integer coords (the rasterizer
operates at float precision). 16 / 255 ≈ 6% mean error is comfortably
inside what's invisible at thumbnail viewing size."""


@pytest.mark.parametrize(
    "factory",
    [
        ("circle", lambda: Codec.circle(n=12)),
        ("triangle", lambda: Codec.triangle(n=12)),
        ("rect", lambda: Codec.rect(n=12)),
        ("square", lambda: Codec.square(n=12)),
    ],
    ids=lambda x: x[0] if isinstance(x, tuple) else None,
)
def test_decode_vs_svg_no_style(factory: Tuple[str, callable], textured_image):
    """Default (no style) — decode raster vs SVG rasterized by resvg
    should match within the consistency threshold."""
    _, c = factory
    codec = c()
    h = encode(textured_image, codec, seed=0)

    base_size = 128
    w_dec, h_dec, rgba_dec = decode(h, codec, base_size=base_size)
    svg = to_svg(h, codec, base_size=base_size)
    rgba_svg = _svg_to_rgba(svg, w_dec, h_dec)

    diff = _mean_abs_diff(rgba_dec, rgba_svg)
    assert diff < CONSISTENCY_THRESHOLD, (
        f"decode vs SVG diverged for {factory[0]}: mean abs diff = {diff:.2f} "
        f"(threshold {CONSISTENCY_THRESHOLD})"
    )


@pytest.mark.parametrize(
    "factory",
    [
        ("rect", lambda: Codec.rect(n=12)),
        ("square", lambda: Codec.square(n=12)),
    ],
)
def test_decode_vs_svg_with_corner_radius(factory, textured_image):
    """`corner_radius` should round corners equivalently in raster decode
    (SDF-based AA) and SVG (`rx`/`ry` attributes)."""
    _, c = factory
    codec = c()
    h = encode(textured_image, codec, seed=0)

    base_size = 128
    style = RenderStyle(corner_radius=8)
    w_dec, h_dec, rgba_dec = decode(h, codec, base_size=base_size, style=style)
    svg = to_svg(h, codec, base_size=base_size, style=style)
    rgba_svg = _svg_to_rgba(svg, w_dec, h_dec)

    diff = _mean_abs_diff(rgba_dec, rgba_svg)
    assert diff < CONSISTENCY_THRESHOLD, (
        f"decode vs SVG with corner_radius diverged for {factory[0]}: "
        f"mean abs diff = {diff:.2f}"
    )


@pytest.mark.parametrize(
    "factory",
    [
        ("circle", lambda: Codec.circle(n=12)),
        ("rect", lambda: Codec.rect(n=12)),
    ],
)
def test_decode_vs_svg_with_blur(factory, textured_image):
    """Blur in both raster decode (CPU sRGB convolution) and SVG
    (`<feGaussianBlur>` via resvg)."""
    _, c = factory
    codec = c()
    h = encode(textured_image, codec, seed=0)

    base_size = 128
    style = RenderStyle(blur=2.0)
    w_dec, h_dec, rgba_dec = decode(h, codec, base_size=base_size, style=style)
    svg = to_svg(h, codec, base_size=base_size, style=style)
    rgba_svg = _svg_to_rgba(svg, w_dec, h_dec)

    diff = _mean_abs_diff(rgba_dec, rgba_svg)
    # Blur amplifies the SVG-engine variance — give a looser ceiling.
    threshold = CONSISTENCY_THRESHOLD + 8.0
    assert diff < threshold, (
        f"decode vs SVG with blur diverged for {factory[0]}: "
        f"mean abs diff = {diff:.2f} (threshold {threshold})"
    )

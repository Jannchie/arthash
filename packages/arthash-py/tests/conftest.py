"""Shared pytest fixtures.

We synthesize small deterministic uint8 RGB arrays rather than checking in
PNG files — this keeps the test inputs binary-stable across platforms (PNG
encoders differ on metadata, compression, etc.).
"""

from __future__ import annotations

import numpy as np
import pytest


@pytest.fixture
def rgb_solid_red():
    """100×100 solid red, RGB uint8."""
    arr = np.zeros((100, 100, 3), dtype=np.uint8)
    arr[..., 0] = 255
    return arr


@pytest.fixture
def rgb_gradient():
    """100×60 horizontal R gradient, RGB uint8. Aspect 100/60 ≈ 1.67."""
    arr = np.zeros((60, 100, 3), dtype=np.uint8)
    arr[..., 0] = np.linspace(0, 255, 100, dtype=np.uint8)[None, :]
    arr[..., 1] = np.linspace(0, 255, 60, dtype=np.uint8)[:, None]
    arr[..., 2] = 64
    return arr


@pytest.fixture
def rgb_random_seed42():
    """96×64 pseudo-random RGB, seeded for reproducibility."""
    rng = np.random.default_rng(42)
    return (rng.random((96, 64, 3)) * 255).astype(np.uint8)

"""SearchOptions — encoder hyperparam plumbing tests.

These knobs affect encoder cost and output quality but NOT the byte format.
Verify they propagate through `encode()` and produce different outputs
when tweaked (since the search is stochastic but seed-determined).
"""

from __future__ import annotations

import time

from arthash import DEFAULT_SEARCH, Codec, SearchOptions, ShapeType, encode


def test_default_search_is_backward_compatible(rgb_random_seed42):
    """Passing search=None or DEFAULT_SEARCH should match no-arg behavior."""
    codec = Codec(shape=ShapeType.CIRCLE, n_shapes=4)
    h0 = encode(rgb_random_seed42, codec, seed=0)
    h1 = encode(rgb_random_seed42, codec, seed=0, search=None)
    h2 = encode(rgb_random_seed42, codec, seed=0, search=DEFAULT_SEARCH)
    assert h0 == h1 == h2


def test_search_changes_output(rgb_random_seed42):
    """Tweaking search params should change the result (with same seed)."""
    codec = Codec(shape=ShapeType.CIRCLE, n_shapes=4)
    h_default = encode(rgb_random_seed42, codec, seed=0)
    h_tweaked = encode(
        rgb_random_seed42, codec, seed=0,
        search=SearchOptions(n_random=100, n_topk=2, hill_climb_steps=10),
    )
    assert h_default != h_tweaked, "different search budget should yield different bytes"


def test_max_age_termination_works(rgb_random_seed42):
    """max_age mode should terminate early when no improvement happens."""
    codec = Codec(shape=ShapeType.CIRCLE, n_shapes=4)
    h = encode(
        rgb_random_seed42, codec, seed=0,
        search=SearchOptions(n_random=200, n_topk=2,
                             hill_climb_max_age=10),
    )
    assert isinstance(h, bytes)
    assert len(h) == codec.bytes_total()


def test_n_attempts_repeats_search(rgb_random_seed42):
    """n_attempts > 1 should produce at-least-as-good results (more search)."""
    codec = Codec(shape=ShapeType.CIRCLE, n_shapes=4)
    # We can't easily measure SSE from bytes alone, but at minimum the
    # encoder shouldn't crash and should produce valid bytes.
    h_single = encode(rgb_random_seed42, codec, seed=0,
                       search=SearchOptions(n_random=50, n_attempts=1,
                                            hill_climb_steps=5))
    h_quad = encode(rgb_random_seed42, codec, seed=0,
                     search=SearchOptions(n_random=50, n_attempts=4,
                                          hill_climb_steps=5))
    assert len(h_single) == len(h_quad) == codec.bytes_total()


def test_triangle_search_options(rgb_random_seed42):
    """Triangle mode also accepts SearchOptions."""
    codec = Codec(shape=ShapeType.TRIANGLE, n_shapes=3)
    h = encode(
        rgb_random_seed42, codec, seed=0,
        search=SearchOptions(n_random=20, n_topk=2, hill_climb_steps=10),
    )
    assert len(h) == codec.bytes_total()


def test_pixel_ignores_search_options(rgb_random_seed42):
    """PIXEL has no fit — search has no effect on output."""
    codec = Codec(shape=ShapeType.PIXEL, n_shapes=8)
    h_default = encode(rgb_random_seed42, codec)
    h_with_search = encode(
        rgb_random_seed42, codec,
        search=SearchOptions(n_random=1, n_topk=1, hill_climb_steps=1),
    )
    assert h_default == h_with_search


def test_larger_budget_takes_more_time(rgb_random_seed42):
    """Heavier search must be measurably slower (warmup numba first)."""
    codec = Codec(shape=ShapeType.CIRCLE, n_shapes=4)
    # Warm up numba
    encode(rgb_random_seed42, codec, seed=0)

    t0 = time.perf_counter()
    encode(rgb_random_seed42, codec, seed=0,
           search=SearchOptions(n_random=100, n_topk=2, hill_climb_steps=10))
    light_ms = (time.perf_counter() - t0) * 1000

    t0 = time.perf_counter()
    encode(rgb_random_seed42, codec, seed=0,
           search=SearchOptions(n_random=1000, n_topk=4, hill_climb_steps=100,
                                n_attempts=4))
    heavy_ms = (time.perf_counter() - t0) * 1000

    assert heavy_ms > light_ms * 2, (
        f"heavier search should be much slower; light={light_ms:.1f}ms "
        f"heavy={heavy_ms:.1f}ms"
    )

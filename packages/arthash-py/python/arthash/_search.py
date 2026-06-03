"""Search-quality knobs for CIRCLE / TRIANGLE shape modes.

Mirrors `arthash::shape::SearchOptions` (Rust). These affect encoder cost and
output quality, but NOT the byte format — same Codec + same bytes decode
identically regardless of these settings.

Two strategies:

    "primitive" — fogleman/primitive's approach. Tiny-start, Gaussian
        perturbations, m independent attempts. Produces bigger, bolder
        shapes. Default.

    "topk_uniform" — arthash's historical strategy. Uniform random pool,
        top-K hill-climb with step decay. Smaller / more numerous shapes.
"""

from __future__ import annotations

from dataclasses import asdict, dataclass


@dataclass(frozen=True)
class SearchOptions:
    """Encoder search budget. Higher budget = better fidelity, higher CPU."""

    strategy: str = "primitive"
    n_random: int = 64
    n_topk: int = 1
    hill_climb_steps: int = 40
    hill_climb_max_age: int | None = 30
    n_attempts: int = 4

    def to_native_dict(self) -> dict:
        # Note: PyO3 handles Option<u32> via None ⇒ None on the Python side.
        return asdict(self)


DEFAULT_SEARCH = SearchOptions()

"""Generate `docs/test-vectors/vectors.json` from the reference Python impl.

This is the *generator* — not a test. Run it whenever the canonical bytes
need refreshing (rare; should be tied to SPEC version bumps):

    uv run python -m tests.generate_vectors

The output is consumed by `test_vectors.py` (Python conformance test) and
by the TS / Rust SDKs (their conformance suites point at the same JSON).

Test inputs are deterministic in-memory uint8 RGB arrays — we deliberately
avoid PNGs so the test vectors stay binary-stable across platforms.
"""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any, Dict, List

import numpy as np

from arthash import Codec, ShapeType, encode
from arthash.palettes import PICO8


REPO_ROOT = Path(__file__).resolve().parents[3]
VECTORS_PATH = REPO_ROOT / "docs" / "test-vectors" / "vectors.json"


# --------------------------- input generators ---------------------------

def _solid(h: int, w: int, rgb: list[int]) -> np.ndarray:
    arr = np.zeros((h, w, 3), dtype=np.uint8)
    arr[..., 0] = rgb[0]; arr[..., 1] = rgb[1]; arr[..., 2] = rgb[2]
    return arr


def _gradient_h(h: int, w: int) -> np.ndarray:
    """Horizontal R gradient + vertical G gradient + constant B."""
    arr = np.zeros((h, w, 3), dtype=np.uint8)
    arr[..., 0] = np.linspace(0, 255, w, dtype=np.uint8)[None, :]
    arr[..., 1] = np.linspace(0, 255, h, dtype=np.uint8)[:, None]
    arr[..., 2] = 64
    return arr


def _random(h: int, w: int, seed: int) -> np.ndarray:
    rng = np.random.default_rng(seed)
    return (rng.random((h, w, 3)) * 255).astype(np.uint8)


# --------------------------- codec serialization ---------------------------

def _codec_to_dict(codec: Codec) -> Dict[str, Any]:
    """Serialize the codec's parameters for cross-language reproduction."""
    out: Dict[str, Any] = {
        "shape": codec.shape.value,
        "n_shapes": codec.n_shapes,
        "cx_bits": codec.cx_bits,
        "cy_bits": codec.cy_bits,
        "r_bits": codec.r_bits,
        "alpha_bits": codec.alpha_bits,
        "color_bits": codec.color_bits,
    }
    if codec.palette is not None:
        out["palette_hex"] = [
            f"{r:02X}{g:02X}{b:02X}" for r, g, b in codec.palette.tolist()
        ]
        out["palette_k"] = codec.palette_k
    if codec.alpha_levels is not None:
        # Default linspace is implicit; only emit when non-default
        n = 1 << codec.alpha_bits
        default = np.linspace(0.20, 0.90, n, dtype=np.float32)
        if not np.allclose(codec.alpha_levels, default):
            out["alpha_levels"] = [float(x) for x in codec.alpha_levels]
    if codec.grid_aspect is not None:
        out["grid_aspect"] = codec.grid_aspect
    return out


# --------------------------- vector specs ---------------------------

VECTOR_SPECS: List[Dict[str, Any]] = [
    # ----- DCT mode -----
    {
        "name": "dct-solid-red-100x100",
        "input": {"kind": "solid", "h": 100, "w": 100, "rgb": [255, 0, 0]},
        "codec": Codec(shape=ShapeType.DCT),
        "encode_kwargs": {"target_size": 100},
    },
    {
        "name": "dct-solid-blue-100x60",
        "input": {"kind": "solid", "h": 60, "w": 100, "rgb": [0, 0, 255]},
        "codec": Codec(shape=ShapeType.DCT),
        "encode_kwargs": {"target_size": 100},
    },
    {
        "name": "dct-gradient-100x60",
        "input": {"kind": "gradient", "h": 60, "w": 100},
        "codec": Codec(shape=ShapeType.DCT),
        "encode_kwargs": {"target_size": 100},
    },
    {
        "name": "dct-random-seed42-96x64",
        "input": {"kind": "random", "h": 96, "w": 64, "seed": 42},
        "codec": Codec(shape=ShapeType.DCT),
        "encode_kwargs": {"target_size": 100},
    },
    # ----- PIXEL mode (no fitting; pure deterministic) -----
    {
        "name": "pixel-continuous-n12-gradient-100x60",
        "input": {"kind": "gradient", "h": 60, "w": 100},
        "codec": Codec(shape=ShapeType.PIXEL, n_shapes=12),
    },
    {
        "name": "pixel-pico8-n16-gradient-100x60",
        "input": {"kind": "gradient", "h": 60, "w": 100},
        "codec": Codec(shape=ShapeType.PIXEL, n_shapes=16, palette=PICO8),
    },
    {
        "name": "pixel-pico8-n48-solid-red",
        "input": {"kind": "solid", "h": 48, "w": 48, "rgb": [255, 0, 0]},
        "codec": Codec(shape=ShapeType.PIXEL, n_shapes=48, palette=PICO8),
    },
    # ----- CIRCLE mode (seed-deterministic) -----
    {
        "name": "circle-continuous-n6-gradient-seed0",
        "input": {"kind": "gradient", "h": 60, "w": 100},
        "codec": Codec(shape=ShapeType.CIRCLE, n_shapes=6),
        "encode_kwargs": {"seed": 0},
    },
    {
        "name": "circle-pico8-n8-random-seed42",
        "input": {"kind": "random", "h": 96, "w": 64, "seed": 42},
        "codec": Codec(shape=ShapeType.CIRCLE, n_shapes=8, palette=PICO8),
        "encode_kwargs": {"seed": 42},
    },
    # ----- TRIANGLE mode -----
    {
        "name": "triangle-continuous-n4-gradient-seed0",
        "input": {"kind": "gradient", "h": 60, "w": 100},
        "codec": Codec(shape=ShapeType.TRIANGLE, n_shapes=4),
        "encode_kwargs": {"seed": 0},
    },
    {
        "name": "triangle-pico8-n6-random-seed42",
        "input": {"kind": "random", "h": 96, "w": 64, "seed": 42},
        "codec": Codec(shape=ShapeType.TRIANGLE, n_shapes=6, palette=PICO8),
        "encode_kwargs": {"seed": 42},
    },
]


def _build_input(spec: Dict[str, Any]) -> np.ndarray:
    kind = spec["kind"]
    if kind == "solid":
        return _solid(spec["h"], spec["w"], spec["rgb"])
    if kind == "gradient":
        return _gradient_h(spec["h"], spec["w"])
    if kind == "random":
        return _random(spec["h"], spec["w"], spec["seed"])
    raise ValueError(f"unknown input kind: {kind!r}")


def main() -> None:
    out_vectors = []
    for spec in VECTOR_SPECS:
        img = _build_input(spec["input"])
        kwargs = spec.get("encode_kwargs", {})
        hash_bytes = encode(img, spec["codec"], **kwargs)
        out_vectors.append({
            "name": spec["name"],
            "input": spec["input"],
            "codec": _codec_to_dict(spec["codec"]),
            "encode_kwargs": kwargs,
            "expected_hex": hash_bytes.hex(),
            "expected_bytes": len(hash_bytes),
        })
        print(f"  {spec['name']:48s} → {len(hash_bytes):3d} B  {hash_bytes.hex()}")

    payload = {
        "version": "1.0",
        "description": "Cross-language conformance vectors for arthash. "
                       "All SDKs (Python / TypeScript / Rust) must produce "
                       "`expected_hex` exactly for each (input, codec, encode_kwargs).",
        "input_kinds": {
            "solid": "Solid color fill: H×W×3 uint8 with arr[..., :] = rgb",
            "gradient": "Horizontal R gradient + vertical G gradient + constant B(=64). "
                       "arr[y, x] = (linspace(0,255,W)[x], linspace(0,255,H)[y], 64).",
            "random": "Pseudo-random uint8 RGB seeded with numpy default_rng(seed). "
                     "Pixels: (rng.random((H, W, 3)) * 255).astype(uint8). "
                     "Non-Python SDKs must replicate numpy's PCG64 sequence; if that's "
                     "impractical, skip the 'random' inputs and exercise only solid/gradient.",
        },
        "vectors": out_vectors,
    }

    VECTORS_PATH.parent.mkdir(parents=True, exist_ok=True)
    VECTORS_PATH.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
    print(f"\nwrote {len(out_vectors)} vectors → {VECTORS_PATH.relative_to(REPO_ROOT)}")


if __name__ == "__main__":
    main()

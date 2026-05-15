"""Benchmark the Python `pfhash` package on identical workloads to the Rust
example/bench.rs. Outputs NDJSON to stdout so the two streams can be merged
side-by-side in compare_bench.py.

`pfhash` is now a PyO3 binding to `pfhash-rs`, so this benchmark measures
PyO3 boundary overhead + Pillow preprocessing on top of the same Rust core
that `bench:rs` exercises directly. Use the gap as a budget for FFI cost.

Run (from repo root):

    uv run python scripts/bench_py.py

Methodology mirrors examples/bench.rs:
  * Inputs: deterministic synthetic gradient at the mode's natural
    thumbnail size (DCT at 100x100, shape modes at 48x48).
  * Warmup + timed iters; report median / p95 / min in microseconds.
"""
from __future__ import annotations

import json
import time
import statistics
import sys
from typing import Callable

import numpy as np

from pfhash import Codec, ShapeType, decode, encode


def gradient_rgb(w: int, h: int) -> np.ndarray:
    """Same gradient formula as examples/bench.rs `gradient_rgb`."""
    xs = np.arange(w, dtype=np.float32)
    ys = np.arange(h, dtype=np.float32)
    rx = np.round(xs * 255.0 / max(w - 1, 1)).astype(np.uint8)
    gy = np.round(ys * 255.0 / max(h - 1, 1)).astype(np.uint8)
    img = np.zeros((h, w, 3), dtype=np.uint8)
    img[..., 0] = np.broadcast_to(rx, (h, w))
    img[..., 1] = np.broadcast_to(gy[:, None], (h, w))
    xy = (xs[None, :] + ys[:, None]) * 0.3
    img[..., 2] = np.clip(xy, 0, 255).astype(np.uint8)
    return img


def measure(fn: Callable[[], None], warmup: int, iters: int) -> dict:
    for _ in range(warmup):
        fn()
    samples = []
    for _ in range(iters):
        t0 = time.perf_counter_ns()
        fn()
        samples.append((time.perf_counter_ns() - t0) / 1000.0)  # ns -> us
    samples.sort()
    return {
        "median_us": statistics.median(samples),
        "p95_us": samples[int(len(samples) * 0.95)],
        "min_us": samples[0],
        "iters": iters,
    }


def report(mode: str, op: str, w: int, h: int, s: dict, extra: dict | None = None) -> None:
    mpix_s = (w * h) / s["median_us"]
    out = {
        "impl": "python",
        "mode": mode,
        "op": op,
        "w": w,
        "h": h,
        "median_us": round(s["median_us"], 2),
        "p95_us": round(s["p95_us"], 2),
        "min_us": round(s["min_us"], 2),
        "iters": s["iters"],
        "mpix_per_s": round(mpix_s, 3),
    }
    if extra:
        out.update(extra)
    print(json.dumps(out), flush=True)


def main() -> None:
    # ---------- DCT ----------
    w, h = 100, 100
    img = gradient_rgb(w, h)
    codec = Codec()  # default = DCT

    hash_holder = {"bytes": b""}

    def enc_dct():
        hash_holder["bytes"] = encode(img, codec)

    s = measure(enc_dct, warmup=30, iters=200)
    report("dct", "encode", w, h, s, {"hash_bytes": len(hash_holder["bytes"])})

    hash_bytes = hash_holder["bytes"]

    def dec_dct():
        decode(hash_bytes, codec, base_size=256)

    s = measure(dec_dct, warmup=10, iters=50)
    report("dct", "decode", w, h, s)

    # Shape decode uses `aa=False` so it matches Rust's non-AA renderer.

    # ---------- Shape modes ----------
    shapes = [
        ("circle", ShapeType.CIRCLE, 12),
        ("triangle", ShapeType.TRIANGLE, 12),
        ("pixel", ShapeType.PIXEL, 12),
    ]
    for name, shape, n_shapes in shapes:
        w, h = 48, 48
        img = gradient_rgb(w, h)
        codec = Codec(shape=shape, n_shapes=n_shapes)

        # Pixel is cheap; circle/triangle are heavy (search) — fewer iters.
        if shape == ShapeType.PIXEL:
            warmup, iters = 10, 100
        else:
            warmup, iters = 3, 15

        hash_holder = {"bytes": b""}

        def enc():
            hash_holder["bytes"] = encode(img, codec)

        s = measure(enc, warmup=warmup, iters=iters)
        report(name, "encode", w, h, s, {"hash_bytes": len(hash_holder["bytes"])})

        hash_bytes = hash_holder["bytes"]

        def dec():
            decode(hash_bytes, codec, base_size=256, aa=False)

        s = measure(dec, warmup=5, iters=30)
        report(name, "decode", w, h, s)


if __name__ == "__main__":
    main()

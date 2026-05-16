"""Benchmark the raw `arthash._native` extension surface — same workload as
scripts/bench_py.py but calling the FFI directly (no PIL, no Codec dataclass)
so we can isolate FFI overhead from the Python wrapper's preprocessing.

Run (from repo root):

    uv run python packages/arthash-py/bench_binding.py > docs/benchmarks/binding.ndjson
"""
from __future__ import annotations

import json
import statistics
import time
from typing import Callable

import numpy as np
from arthash import _native as m


def gradient_rgb(w: int, h: int) -> np.ndarray:
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
        samples.append((time.perf_counter_ns() - t0) / 1000.0)
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
        "impl": "pyo3",
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
    # DCT
    w, h = 100, 100
    img = gradient_rgb(w, h)
    rgb_bytes = img.tobytes()

    hash_holder = {"b": b""}

    def enc():
        hash_holder["b"] = m.encode_rgb(rgb_bytes, w, h)

    s = measure(enc, 30, 200)
    report("dct", "encode", w, h, s, {"hash_bytes": len(hash_holder["b"])})

    hb = hash_holder["b"]

    def dec():
        m.decode(hb, base_size=256)

    s = measure(dec, 10, 50)
    report("dct", "decode", w, h, s)

    # Shape modes
    shapes = [
        ("circle", "circle", 12),
        ("triangle", "triangle", 12),
        ("pixel", "pixel", 12),
    ]
    for name, shape, n_shapes in shapes:
        w, h = 48, 48
        img = gradient_rgb(w, h)
        rgb_bytes = img.tobytes()
        codec = {"shape": shape, "n_shapes": n_shapes}

        if shape == "pixel":
            warmup, iters = 10, 100
        else:
            warmup, iters = 3, 15

        hash_holder = {"b": b""}

        def enc():
            hash_holder["b"] = m.encode_rgb(rgb_bytes, w, h, codec)

        s = measure(enc, warmup, iters)
        report(name, "encode", w, h, s, {"hash_bytes": len(hash_holder["b"])})

        hb = hash_holder["b"]

        def dec():
            m.decode(hb, codec, base_size=256)

        s = measure(dec, 5, 30)
        report(name, "decode", w, h, s)


if __name__ == "__main__":
    main()

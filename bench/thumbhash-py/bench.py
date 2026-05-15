"""Benchmark the PyPI `thumbhash` package on the same 100x100 gradient
input. Output: NDJSON to stdout.
"""
from __future__ import annotations

import json
import statistics
import time

import thumbhash


def gradient_rgba(w: int, h: int) -> list[int]:
    rgba = bytearray(w * h * 4)
    for y in range(h):
        for x in range(w):
            p = (y * w + x) * 4
            rgba[p] = round(x * 255 / max(w - 1, 1))
            rgba[p + 1] = round(y * 255 / max(h - 1, 1))
            rgba[p + 2] = min(255, int((x + y) * 0.3))
            rgba[p + 3] = 255
    return list(rgba)


def measure(fn, warmup, iters):
    for _ in range(warmup):
        fn()
    samples = []
    for _ in range(iters):
        t0 = time.perf_counter_ns()
        fn()
        samples.append((time.perf_counter_ns() - t0) / 1000.0)
    samples.sort()
    return samples[len(samples) // 2], samples[int(len(samples) * 0.95)], samples[0]


def report(mode, op, w, h, s, iters, extra=None):
    out = {
        "impl": "py-thumbhash",
        "mode": mode,
        "op": op,
        "w": w,
        "h": h,
        "median_us": round(s[0], 2),
        "p95_us": round(s[1], 2),
        "min_us": round(s[2], 2),
        "iters": iters,
        "mpix_per_s": round((w * h) / s[0], 3),
    }
    if extra:
        out.update(extra)
    print(json.dumps(out), flush=True)


def main():
    w, h = 100, 100
    rgba = gradient_rgba(w, h)
    hash_ = {"v": []}
    s = measure(lambda: hash_.__setitem__("v", thumbhash.rgba_to_thumb_hash(w, h, rgba)), 30, 200)
    report("dct", "encode", w, h, s, 200, {"hash_bytes": len(hash_["v"])})

    hb = hash_["v"]
    s = measure(lambda: thumbhash.thumb_hash_to_rgba(hb), 10, 50)
    report("dct", "decode_default", w, h, s, 50)


if __name__ == "__main__":
    main()

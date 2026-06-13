"""Encode-latency vs perceptual-quality Pareto (paper figure 2).

arthash's second axis: at comparable placeholder quality it encodes one to two
orders of magnitude faster than the only other primitive-fitting baseline
(SQIP). This script measures pure encode latency (no decode / metric) for every
method on the Kodak thumbnails, joins each method's mean LPIPS from
bench/rd_results_kodak.csv, and plots latency (log x) vs LPIPS (y).

SQIP is drawn as a reference point from the project's existing same-machine
cross-impl benchmark (docs/benchmarks/js_cross_sqip.ndjson, node) — its sharp
dependency is broken on this box, but its encode latency is already recorded and
its quality is bounded in docs/benchmarks/CROSS_IMPL.md.

Run rd_benchmark.py kodak first (for the LPIPS join). Then:
    uv run python scripts/paper/speed_benchmark.py
"""
from __future__ import annotations

import csv
import statistics
import time
from pathlib import Path

import numpy as np
from PIL import Image

import blurhash
import thash
from arthash import Codec, ShapeType, encode

ROOT = Path(__file__).resolve().parents[2]
BENCH = ROOT / "bench"
KODAK = BENCH / "kodak"
RD_CSV = BENCH / "rd_results_kodak.csv"
ENCODE_EDGE = 100
WARMUP = 2
ITERS = 9

# SQIP reference (docs/benchmarks/js_cross_sqip.ndjson, primitive-triangle,
# node, same machine). No LPIPS (sharp is broken here); plotted as a latency
# reference line. Quality bound: CROSS_IMPL.md (~1 kB SVG, +3 dB PSNR over
# arthash CIRCLE-12).
SQIP_REF = {12: 284.3, 24: 445.6, 64: 1015.4}  # median ms


def resize_long(im: Image.Image, edge: int) -> Image.Image:
    s = edge / max(im.size)
    return im.resize((max(1, round(im.width * s)), max(1, round(im.height * s))), Image.LANCZOS)


def arthash_enc(st: ShapeType, n: int):
    def run(arr: np.ndarray) -> int:
        codec = Codec() if st == ShapeType.DCT else Codec(shape=st, n_shapes=n)
        hb = encode(arr, codec) if st == ShapeType.DCT else encode(arr, codec, seed=0)
        return len(hb)
    return run


def blurhash_enc(cx: int, cy: int):
    def run(arr: np.ndarray) -> int:
        return len(blurhash.encode(arr, components_x=cx, components_y=cy).encode("ascii"))
    return run


def thumbhash_enc(arr: np.ndarray) -> int:
    rgba = np.dstack([arr, np.full(arr.shape[:2], 255, np.uint8)])
    return len(bytes(thash.rgba_to_thumb_hash_numpy(arr.shape[1], arr.shape[0], rgba)))


METHODS: dict[str, object] = {"thumbhash": thumbhash_enc, "arthash-dct": arthash_enc(ShapeType.DCT, 0)}
for cx, cy in [(3, 3), (4, 3), (5, 4), (6, 4), (8, 6), (9, 9)]:
    METHODS[f"blurhash-{cx}x{cy}"] = blurhash_enc(cx, cy)
for st, label in [(ShapeType.PIXEL, "pixel"), (ShapeType.CIRCLE, "circle"),
                  (ShapeType.TRIANGLE, "triangle"), (ShapeType.RECT, "rect"),
                  (ShapeType.SQUARE, "square")]:
    for n in (4, 8, 12, 24, 48):
        METHODS[f"arthash-{label}-{n}"] = arthash_enc(st, n)


def lpips_by_method() -> dict[str, tuple[float, float]]:
    """method -> (mean bytes, mean LPIPS) from the R-D run."""
    if not RD_CSV.exists():
        raise SystemExit(f"missing {RD_CSV}; run rd_benchmark.py kodak first")
    rows = list(csv.DictReader(RD_CSV.open()))
    out: dict[str, list[tuple[float, float]]] = {}
    for r in rows:
        out.setdefault(r["method"], []).append((float(r["bytes"]), float(r["lpips"])))
    return {m: (float(np.mean([b for b, _ in v])), float(np.mean([l for _, l in v]))) for m, v in out.items()}


def main() -> None:
    thumbs = [np.array(resize_long(Image.open(p).convert("RGB"), ENCODE_EDGE), dtype=np.uint8)
              for p in sorted(KODAK.glob("kodim*.png"))]
    if not thumbs:
        raise SystemExit("no Kodak images; run fetch_datasets.py")

    speed: dict[str, float] = {}
    for name, fn in METHODS.items():
        per_img = []
        for arr in thumbs:
            for _ in range(WARMUP):
                fn(arr)
            ts = []
            for _ in range(ITERS):
                t0 = time.perf_counter(); fn(arr); ts.append((time.perf_counter() - t0) * 1000)
            per_img.append(statistics.median(ts))
        speed[name] = float(np.median(per_img))
        print(f"{name:22s} {speed[name]:8.3f} ms", flush=True)

    quality = lpips_by_method()
    out_csv = BENCH / "speed_kodak.csv"
    with out_csv.open("w", newline="") as f:
        w = csv.writer(f); w.writerow(["method", "encode_ms", "bytes", "lpips"])
        for m in METHODS:
            b, l = quality.get(m, (float("nan"), float("nan")))
            w.writerow([m, round(speed[m], 4), round(b, 1), round(l, 4)])

    plot(speed, quality)
    print(f"\nCSV: {out_csv}")
    fastest_tri = speed.get("arthash-triangle-12", float("nan"))
    print(f"arthash-triangle-12 {fastest_tri:.2f} ms  vs  sqip triangle-12 "
          f"{SQIP_REF[12]:.0f} ms  ->  {SQIP_REF[12] / fastest_tri:.0f}x faster")


def plot(speed: dict[str, float], quality: dict[str, tuple[float, float]]) -> None:
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt

    fams: dict[str, list[tuple[float, float]]] = {}
    for m, ms in speed.items():
        if m not in quality:
            continue
        fam = m.rsplit("-", 1)[0] if (m.startswith(("arthash-", "blurhash-"))
                                      and m.rsplit("-", 1)[-1].isdigit()) else m
        fams.setdefault(fam, []).append((ms, quality[m][1]))
    fig, ax = plt.subplots(figsize=(8, 5.5))
    for fam, pts in sorted(fams.items()):
        pts.sort()
        xs = [p[0] for p in pts]; ys = [p[1] for p in pts]
        ax.plot(xs, ys, "-o" if len(pts) > 1 else "D", ms=5, label=fam)
    for n, ms in SQIP_REF.items():
        ax.axvline(ms, color="0.6", ls=":", lw=1)
    ax.text(SQIP_REF[12], ax.get_ylim()[1], " sqip-12\n (no LPIPS;\n  ~1kB SVG)",
            va="top", fontsize=7, color="0.4")
    ax.set_xscale("log")
    ax.set_xlabel("encode latency (ms, log) — lower-left is better")
    ax.set_ylabel("LPIPS ↓")
    ax.set_title("Encode latency vs perceptual quality (Kodak, 100px input)")
    ax.grid(True, alpha=0.3)
    ax.legend(fontsize=7, ncol=2)
    out = BENCH / "speed_quality_kodak.png"
    fig.tight_layout(); fig.savefig(out, dpi=140)
    print(f"plot: {out}")


if __name__ == "__main__":
    main()

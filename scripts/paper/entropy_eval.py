"""Entropy-coding headroom for arthash shape modes (offline, no Rust changes).

The current wire format spends fixed bit widths per field (e.g. CIRCLE =
cx5 cy5 r4 color16 alpha3). Those symbols are far from uniform, so a static
frequency model + arithmetic coder would shrink them. This script measures how
much, the honest way:

  1. Encode a *train* corpus, parse the LSB-first bitstream into per-field
     symbol streams, and fit a Laplace-smoothed static model per stream
     (this is the model that would ship baked into the codec, amortized to
     zero per-image cost — exactly like blurhash's fixed base83 alphabet).
  2. Encode a *test* corpus and score each image's symbols under the frozen
     model: coded_bits = Σ -log2 p_model(sym). A real range coder realises
     this within <1 byte of overhead, so it is a faithful size estimate.

Reports fixed vs entropy bytes per mode, the per-field breakdown, and the
implied byte savings (which shifts the R-D curve left).

Usage:
    uv run python scripts/entropy_eval.py [--train clic] [--test kodak]
"""
from __future__ import annotations

import argparse
import math
from collections import defaultdict
from pathlib import Path

import numpy as np
from PIL import Image

from arthash import Codec, ShapeType, decode, encode  # noqa: F401  (encode used)

ROOT = Path(__file__).resolve().parents[2]
BENCH = ROOT / "bench"
ENCODE_EDGE = 100

MODES = [
    (ShapeType.CIRCLE, "circle"),
    (ShapeType.TRIANGLE, "triangle"),
    (ShapeType.RECT, "rect"),
    (ShapeType.SQUARE, "square"),
]
SHAPE_COUNTS = [12, 32]


class BitReader:
    """LSB-first, mirrors arthash::bitio::BitReader."""

    def __init__(self, data: bytes) -> None:
        self.data = data
        self.pos = 0

    def read(self, n: int) -> int:
        v = 0
        for i in range(n):
            byte = self.data[self.pos >> 3] if (self.pos >> 3) < len(self.data) else 0
            v |= ((byte >> (self.pos & 7)) & 1) << i
            self.pos += 1
        return v

    def color565(self) -> tuple[int, int, int]:
        c = self.read(16)
        return (c >> 11) & 0x1F, (c >> 5) & 0x3F, c & 0x1F


def parse_streams(hb: bytes, mode: str, c: Codec, context: bool = False) -> dict[str, list[int]]:
    """Parse one hash into {field: [symbols]} using the known bit layout.

    With `context=True`, triangle vertices are predictively coded: the first
    vertex absolute, the other two as signed deltas (offset to a non-negative
    alphabet) — exploiting the tiny per-triangle vertex cluster that order-0
    marginals miss."""
    br = BitReader(hb)
    out: dict[str, list[int]] = defaultdict(list)
    out["aspect"].append(br.read(8))
    r, g, b = br.color565()
    out["bgR"].append(r); out["bgG"].append(g); out["bgB"].append(b)
    cxb, cyb, rb, ab = c.cx_bits, c.cy_bits, c.r_bits, c.alpha_bits
    for _ in range(c.n_shapes):
        if mode == "triangle":
            vs = [(br.read(cxb), br.read(cyb)) for _ in range(3)]
            if context:
                out["v0x"].append(vs[0][0]); out["v0y"].append(vs[0][1])
                for k in (1, 2):
                    out["dvx"].append(vs[k][0] - vs[0][0] + (1 << cxb) - 1)
                    out["dvy"].append(vs[k][1] - vs[0][1] + (1 << cyb) - 1)
            else:
                for vx, vy in vs:
                    out["vx"].append(vx); out["vy"].append(vy)
        elif mode == "rect":
            out["cx"].append(br.read(cxb)); out["cy"].append(br.read(cyb))
            out["w"].append(br.read(rb)); out["h"].append(br.read(rb))
        else:  # circle, square
            out["cx"].append(br.read(cxb)); out["cy"].append(br.read(cyb))
            out["r"].append(br.read(rb))
        r, g, b = br.color565()
        out["cR"].append(r); out["cG"].append(g); out["cB"].append(b)
        out["alpha"].append(br.read(ab))
    return out


def images_in(dataset: str) -> list[Path]:
    d = BENCH / dataset
    return sorted(p for p in d.rglob("*.png")
                  if not p.name.startswith("._") and "__MACOSX" not in p.parts)


def small_of(path: Path) -> np.ndarray:
    im = Image.open(path).convert("RGB")
    s = ENCODE_EDGE / max(im.size)
    im = im.resize((max(1, round(im.width * s)), max(1, round(im.height * s))), Image.LANCZOS)
    return np.array(im, dtype=np.uint8)


def collect(dataset: str, st: ShapeType, mode: str, n: int, context: bool = False):
    c = Codec(shape=st, n_shapes=n)
    per_image = []
    for path in images_in(dataset):
        hb = bytes(encode(small_of(path), c, seed=0))
        per_image.append((len(hb), parse_streams(hb, mode, c, context)))
    return c, per_image


def fit_model(per_image, alphabet: dict[str, int]) -> dict[str, np.ndarray]:
    """Laplace-smoothed static probabilities per field."""
    counts = {f: np.ones(k, dtype=np.float64) for f, k in alphabet.items()}
    for _, streams in per_image:
        for f, syms in streams.items():
            for s in syms:
                counts[f][s] += 1.0
    return {f: cnt / cnt.sum() for f, cnt in counts.items()}


def score(per_image, model: dict[str, np.ndarray]) -> tuple[float, float, dict[str, float]]:
    """Mean fixed bytes, mean entropy-coded bytes, per-field mean bits/image."""
    fixed_bytes, ent_bytes = [], []
    field_bits = defaultdict(list)
    for nbytes, streams in per_image:
        bits = 0.0
        local = defaultdict(float)
        for f, syms in streams.items():
            p = model[f]
            for s in syms:
                cost = -math.log2(p[s])
                bits += cost
                local[f] += cost
        fixed_bytes.append(nbytes)
        ent_bytes.append(bits / 8.0)
        for f, v in local.items():
            field_bits[f].append(v)
    return (float(np.mean(fixed_bytes)), float(np.mean(ent_bytes)),
            {f: float(np.mean(v)) for f, v in field_bits.items()})


def alphabet_for(mode: str, c: Codec, context: bool = False) -> dict[str, int]:
    a = {
        "aspect": 256,
        "bgR": 32, "bgG": 64, "bgB": 32,
        "cR": 32, "cG": 64, "cB": 32,
        "alpha": 1 << c.alpha_bits,
    }
    if mode == "triangle":
        if context:
            a["v0x"] = 1 << c.cx_bits
            a["v0y"] = 1 << c.cy_bits
            a["dvx"] = (1 << (c.cx_bits + 1)) - 1
            a["dvy"] = (1 << (c.cy_bits + 1)) - 1
        else:
            a["vx"] = 1 << c.cx_bits
            a["vy"] = 1 << c.cy_bits
    elif mode == "rect":
        a.update({"cx": 1 << c.cx_bits, "cy": 1 << c.cy_bits,
                  "w": 1 << c.r_bits, "h": 1 << c.r_bits})
    else:
        a.update({"cx": 1 << c.cx_bits, "cy": 1 << c.cy_bits, "r": 1 << c.r_bits})
    return a


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--train", default="clic")
    ap.add_argument("--test", default="kodak")
    args = ap.parse_args()

    print(f"train(model)={args.train}  test(score)={args.test}\n")
    header = (f"{'mode':9s} {'n':>3s} {'fixed_B':>8s} {'order0_B':>9s} {'save%':>6s} "
              f"{'ctx_B':>7s} {'save%':>6s}")
    print(header)
    for st, mode in MODES:
        for n in SHAPE_COUNTS:
            # order-0
            _, train_pi = collect(args.train, st, mode, n)
            ctest, test_pi = collect(args.test, st, mode, n)
            model = fit_model(train_pi, alphabet_for(mode, ctest))
            fixed, ent0, _ = score(test_pi, model)
            # context (currently only triangle differs; others identical)
            if mode == "triangle":
                _, train_c = collect(args.train, st, mode, n, context=True)
                _, test_c = collect(args.test, st, mode, n, context=True)
                model_c = fit_model(train_c, alphabet_for(mode, ctest, context=True))
                _, entc, _ = score(test_c, model_c)
            else:
                entc = ent0
            print(f"{mode:9s} {n:3d} {fixed:8.1f} {ent0:9.1f} {(1-ent0/fixed)*100:5.1f}% "
                  f"{entc:7.1f} {(1-entc/fixed)*100:5.1f}%")
    print("\norder0 = per-field static model; ctx = + triangle vertex-delta prediction")


if __name__ == "__main__":
    main()

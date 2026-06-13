"""Joint-refinement ablation for the paper.

For each shape mode and shape count, compare the greedy baseline
(refine_passes=0) against quantization-aware joint refinement (refine_passes=N)
across the full metric suite (PSNR / SSIM / LPIPS / DISTS) on a dataset.

Same protocol as rd_benchmark.py: 100px encoder input, 256px eval canvas.

Outputs bench/refine_ablation_<dataset>.csv and a stdout table.

Usage:
    uv run python scripts/refine_ablation.py [kodak|clic] [--passes N] [--limit K]
"""
from __future__ import annotations

import argparse
import csv
import math
import time
from pathlib import Path

import numpy as np
from PIL import Image

from arthash import Codec, ShapeType, SearchOptions, decode, encode

ROOT = Path(__file__).resolve().parents[2]
BENCH = ROOT / "bench"
ENCODE_EDGE = 100
EVAL_EDGE = 256

MODES = [
    (ShapeType.CIRCLE, "circle"),
    (ShapeType.TRIANGLE, "triangle"),
    (ShapeType.RECT, "rect"),
    (ShapeType.SQUARE, "square"),
]
SHAPE_COUNTS = [12, 32]


def resize_long(im: Image.Image, edge: int) -> Image.Image:
    w, h = im.size
    s = edge / max(w, h)
    return im.resize((max(1, round(w * s)), max(1, round(h * s))), Image.LANCZOS)


def psnr(a: np.ndarray, b: np.ndarray) -> float:
    mse = ((a.astype(np.float64) - b.astype(np.float64)) ** 2).mean()
    return float("inf") if mse <= 1e-12 else 20 * math.log10(255.0 / math.sqrt(mse))


class Metrics:
    def __init__(self) -> None:
        import lpips
        import torch
        from DISTS_pytorch import DISTS

        self.torch = torch
        self.lpips = lpips.LPIPS(net="alex", verbose=False).eval()
        self.dists = DISTS().eval()

    def perceptual(self, a: np.ndarray, b: np.ndarray) -> tuple[float, float]:
        t = self.torch
        with t.no_grad():
            ta = t.from_numpy(a).permute(2, 0, 1).unsqueeze(0).float() / 255.0
            tb = t.from_numpy(b).permute(2, 0, 1).unsqueeze(0).float() / 255.0
            return float(self.lpips(ta * 2 - 1, tb * 2 - 1).item()), float(self.dists(ta, tb).item())

    @staticmethod
    def ssim(a: np.ndarray, b: np.ndarray) -> float:
        from skimage.metrics import structural_similarity

        return float(structural_similarity(a, b, channel_axis=2, data_range=255))


def render(small: Image.Image, st: ShapeType, n: int, passes: int, tw: int, th: int):
    codec = Codec(shape=st, n_shapes=n)
    arr = np.array(small, dtype=np.uint8)
    t0 = time.perf_counter()
    hb = encode(arr, codec, seed=0, search=SearchOptions(refine_passes=passes))
    enc_ms = (time.perf_counter() - t0) * 1000.0
    w, h, pix = decode(hb, codec, base_size=max(tw, th), aa=True)
    img = Image.fromarray(np.asarray(pix)[..., :3], "RGB").resize((tw, th), Image.LANCZOS)
    return img, len(hb), enc_ms


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("dataset", nargs="?", default="kodak", choices=["kodak", "clic"])
    ap.add_argument("--passes", type=int, default=2)
    ap.add_argument("--limit", type=int, default=0, help="cap images (0 = all)")
    args = ap.parse_args()

    img_dir = BENCH / args.dataset
    images = sorted(p for p in img_dir.rglob("*.png")
                    if not p.name.startswith("._") and "__MACOSX" not in p.parts)
    if args.limit:
        images = images[: args.limit]
    if not images:
        raise SystemExit(f"no images in {img_dir}")

    m = Metrics()
    settings = [("base", 0), (f"refine{args.passes}", args.passes)]
    rows = []
    for i, path in enumerate(images):
        full = Image.open(path).convert("RGB")
        gt = resize_long(full, EVAL_EDGE)
        small = resize_long(full, ENCODE_EDGE)
        gt_arr = np.array(gt, dtype=np.uint8)
        for st, label in MODES:
            for n in SHAPE_COUNTS:
                for sname, passes in settings:
                    img, nbytes, enc_ms = render(small, st, n, passes, gt.width, gt.height)
                    arr = np.array(img, dtype=np.uint8)
                    lp, ds = m.perceptual(gt_arr, arr)
                    rows.append({
                        "image": path.stem, "mode": label, "n": n, "setting": sname,
                        "bytes": nbytes, "enc_ms": round(enc_ms, 2),
                        "psnr": round(psnr(gt_arr, arr), 3),
                        "ssim": round(m.ssim(gt_arr, arr), 4),
                        "lpips": round(lp, 4), "dists": round(ds, 4),
                    })
        print(f"[{args.dataset}] {i + 1}/{len(images)} {path.stem}", flush=True)

    out_csv = BENCH / f"refine_ablation_{args.dataset}.csv"
    with out_csv.open("w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=list(rows[0].keys()))
        w.writeheader()
        w.writerows(rows)

    print(f"\n=== refine ablation on {args.dataset} ({len(images)} imgs), "
          f"refine_passes={args.passes} ===")
    hdr = f"{'mode':9s} {'n':>3s} {'setting':9s} {'bytes':>6s} {'PSNR':>7s} {'SSIM':>7s} {'LPIPS':>7s} {'DISTS':>7s} {'enc_ms':>7s}"
    print(hdr)
    for st, label in MODES:
        for n in SHAPE_COUNTS:
            base = None
            for sname, _ in settings:
                sub = [r for r in rows if r["mode"] == label and r["n"] == n and r["setting"] == sname]
                agg = {k: float(np.mean([r[k] for r in sub]))
                       for k in ("bytes", "psnr", "ssim", "lpips", "dists", "enc_ms")}
                tag = ""
                if sname == "base":
                    base = agg
                elif base is not None:
                    dpsnr = agg["psnr"] - base["psnr"]
                    dlpips = (agg["lpips"] / base["lpips"] - 1) * 100
                    ddists = (agg["dists"] / base["dists"] - 1) * 100
                    tag = f"  ΔPSNR {dpsnr:+.2f}  ΔLPIPS {dlpips:+.1f}%  ΔDISTS {ddists:+.1f}%"
                print(f"{label:9s} {n:3d} {sname:9s} {agg['bytes']:6.0f} {agg['psnr']:7.2f} "
                      f"{agg['ssim']:7.4f} {agg['lpips']:7.4f} {agg['dists']:7.4f} {agg['enc_ms']:7.2f}{tag}")
    print(f"\nCSV: {out_csv}")


if __name__ == "__main__":
    main()

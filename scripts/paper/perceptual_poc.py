"""PoC: does an edge-weighted fitting objective improve LPIPS?

Pure-numpy greedy circle fitter, run twice with identical RNG (so the sampled
candidate geometries are byte-for-byte the same) under different per-pixel
weight maps. The ONLY variable is the objective:

    minimize  Σ w·(t − c)²        (w ≡ 1  ->  plain L2, arthash's objective)
                                   (w = 1 + λ·edge  ->  edge-weighted)

If the edge-weighted runs win on LPIPS at equal byte budget, the full Rust
weighted-integral implementation is worth building. This script touches no Rust.

Closed form mirrors arthash::shape::raster::ShapeSums::finalize, with every sum
weighted and `count` replaced by Σw — the two are algebraically identical.

Usage:
    uv run python scripts/paper/perceptual_poc.py [--limit K] [--n 12]
"""
from __future__ import annotations

import argparse
import math
from pathlib import Path

import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
KODAK = ROOT / "bench" / "kodak"
ENCODE_EDGE = 100
EVAL_EDGE = 256
ALPHAS = np.array([0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9])
# weight schemes to compare against the uniform L2 baseline. Each is
# (label, mode, lambda); mode picks the per-pixel weight cue.
SCHEMES = [
    ("uniform", "uniform", 0.0),
    ("edge-4", "edge", 4.0),
    ("center-4", "center", 4.0),
    ("saliency-4", "saliency", 4.0),
    ("saliency-8", "saliency", 8.0),
]


def srgb_to_linear(a: np.ndarray) -> np.ndarray:
    a = a / 255.0
    return np.where(a <= 0.04045, a / 12.92, ((a + 0.055) / 1.055) ** 2.4)


def linear_to_srgb(a: np.ndarray) -> np.ndarray:
    a = np.clip(a, 0.0, 1.0)
    s = np.where(a <= 0.0031308, a * 12.92, 1.055 * a ** (1 / 2.4) - 0.055)
    return np.clip(s * 255.0 + 0.5, 0, 255).astype(np.uint8)


def weight_map(lin: np.ndarray, mode: str, lam: float) -> np.ndarray:
    """Per-pixel weight w = 1 + lam * cue, cue in [0, 1].

    edge     — Sobel gradient magnitude (favors high-frequency boundaries).
    center   — Gaussian center prior (favors the middle of the frame).
    saliency — global color-contrast × center prior (favors the salient
               subject, an unsupervised proxy for "what the eye locks onto").
    """
    H, W, _ = lin.shape
    if mode == "uniform" or lam == 0.0:
        return np.ones((H, W), dtype=np.float64)
    if mode == "edge":
        lum = lin @ np.array([0.2126, 0.7152, 0.0722])
        gx = np.zeros_like(lum); gy = np.zeros_like(lum)
        gx[:, 1:-1] = lum[:, 2:] - lum[:, :-2]
        gy[1:-1, :] = lum[2:, :] - lum[:-2, :]
        cue = np.sqrt(gx * gx + gy * gy)
    else:
        yy, xx = np.mgrid[0:H, 0:W]
        d2 = ((yy - (H - 1) / 2) / (H / 2)) ** 2 + ((xx - (W - 1) / 2) / (W / 2)) ** 2
        center = np.exp(-d2 * 1.5)
        if mode == "center":
            cue = center
        elif mode == "saliency":
            mean = lin.reshape(-1, 3).mean(0)
            contrast = np.linalg.norm(lin - mean, axis=2)
            contrast /= (contrast.max() + 1e-9)
            cue = contrast * center
        else:
            raise ValueError(mode)
    cue = cue / (cue.max() + 1e-9)
    return 1.0 + lam * cue


def fit_circles(lin: np.ndarray, w: np.ndarray, n: int, seed: int,
                k: int = 48, climb: int = 15) -> np.ndarray:
    """Weighted greedy circle fit. Returns the rendered linear-RGB canvas."""
    H, W, _ = lin.shape
    rng = np.random.default_rng(seed)
    yy, xx = np.mgrid[0:H, 0:W]
    wsum_all = w.sum()
    bg = np.array([(w * lin[..., c]).sum() / wsum_all for c in range(3)])
    canvas = np.broadcast_to(bg, lin.shape).copy()
    r_init_max = max(2, int(max(H, W) * 0.12))
    r_max = max(H, W)

    def eval_geom(cx, cy, r):
        m = (xx - cx) ** 2 + (yy - cy) ** 2 <= r * r
        if not m.any():
            return None
        wm = w[m]
        sw = wm.sum()
        t = lin[m]; c = canvas[m]
        swt = (wm[:, None] * t).sum(0)
        swc = (wm[:, None] * c).sum(0)
        swt2 = (wm[:, None] * t * t).sum(0)
        swc2 = (wm[:, None] * c * c).sum(0)
        swtc = (wm[:, None] * t * c).sum(0)
        best = None
        for a in ALPHAS:
            color = np.clip((swt - (1 - a) * swc) / (a * sw + 1e-12), 0, 1)
            before = swt2 - 2 * swtc + swc2
            after = (swt2 - 2 * (1 - a) * swtc + (1 - a) ** 2 * swc2
                     - 2 * a * color * swt + 2 * a * (1 - a) * color * swc
                     + a * a * color * color * sw)
            d = float((after - before).sum())
            if best is None or d < best[0]:
                best = (d, a, color, m)
        return best

    for _ in range(n):
        cx = int(rng.integers(0, W)); cy = int(rng.integers(0, H))
        r = int(rng.integers(1, r_init_max + 1))
        cur = eval_geom(cx, cy, r)
        if cur is None:
            cur = (0.0, ALPHAS[0], bg.copy(), None)
        bd, ba, bc, bm = cur
        for _ in range(climb):
            which = rng.integers(0, 3)
            ncx, ncy, nr = cx, cy, r
            step = int(round(rng.normal() * max(2, max(H, W) * 0.06)))
            if which == 0:
                ncx = min(max(cx + step, 0), W - 1)
            elif which == 1:
                ncy = min(max(cy + step, 0), H - 1)
            else:
                nr = min(max(r + step, 1), r_max)
            cand = eval_geom(ncx, ncy, nr)
            if cand and cand[0] < bd:
                bd, ba, bc, bm = cand
                cx, cy, r = ncx, ncy, nr
        if bm is not None and bd < 0:
            canvas[bm] = (1 - ba) * canvas[bm] + ba * bc
    return canvas


class Lpips:
    def __init__(self) -> None:
        import lpips
        import torch
        self.torch = torch
        self.net = lpips.LPIPS(net="alex", verbose=False).eval()

    def __call__(self, a: np.ndarray, b: np.ndarray) -> float:
        t = self.torch
        with t.no_grad():
            ta = t.from_numpy(a).permute(2, 0, 1).unsqueeze(0).float() / 127.5 - 1
            tb = t.from_numpy(b).permute(2, 0, 1).unsqueeze(0).float() / 127.5 - 1
            return float(self.net(ta, tb).item())


def psnr(a, b):
    mse = ((a.astype(np.float64) - b.astype(np.float64)) ** 2).mean()
    return float("inf") if mse <= 1e-12 else 20 * math.log10(255 / math.sqrt(mse))


def resize_long(im: Image.Image, edge: int) -> Image.Image:
    s = edge / max(im.size)
    return im.resize((max(1, round(im.width * s)), max(1, round(im.height * s))), Image.LANCZOS)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--limit", type=int, default=8)
    ap.add_argument("--n", type=int, default=12)
    args = ap.parse_args()

    images = sorted(KODAK.glob("kodim*.png"))[: args.limit]
    lp = Lpips()
    agg = {label: {"lpips": [], "psnr": []} for label, _, _ in SCHEMES}
    for path in images:
        full = Image.open(path).convert("RGB")
        small = resize_long(full, ENCODE_EDGE)
        gt = resize_long(full, EVAL_EDGE)
        gt_arr = np.array(gt, dtype=np.uint8)
        lin = srgb_to_linear(np.array(small, dtype=np.float64))
        line = f"{path.stem}:"
        for label, mode, lam in SCHEMES:
            w = weight_map(lin, mode, lam)
            canvas = fit_circles(lin, w, n=args.n, seed=0)
            dec = Image.fromarray(linear_to_srgb(canvas), "RGB").resize(gt.size, Image.LANCZOS)
            arr = np.array(dec, dtype=np.uint8)
            lpv = lp(gt_arr, arr); psv = psnr(gt_arr, arr)
            agg[label]["lpips"].append(lpv); agg[label]["psnr"].append(psv)
            line += f"  {label} L{lpv:.3f}"
        print(line, flush=True)

    print(f"\n=== mean over {len(images)} imgs, n={args.n} circles ===")
    base_l = np.mean(agg["uniform"]["lpips"])
    print(f"{'scheme':>11s} {'PSNR':>7s} {'LPIPS':>7s} {'dLPIPS':>8s}")
    for label, _, _ in SCHEMES:
        l = np.mean(agg[label]["lpips"]); p = np.mean(agg[label]["psnr"])
        tag = "" if label == "uniform" else f"  {(l/base_l-1)*100:+.1f}%"
        print(f"{label:>11s} {p:7.2f} {l:7.4f}{tag}")


if __name__ == "__main__":
    main()

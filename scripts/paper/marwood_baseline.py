"""Faithful strong baseline: Marwood et al. ICIP'18 "Representing Images in
200 Bytes" (grid-vertex Delaunay triangulation + palette vertex colors +
Gouraud interpolation + entropy-coded vertices).

Reconstructed from arXiv:1809.02257 (no official code exists). Faithful to the
*stochastic* variant's representation:

  * Vertices live on a g×g grid; only their occupancy is coded — the Delaunay
    connectivity is recomputed identically at decode (so it costs 0 bytes).
  * Each vertex carries an index into a global K-color palette (6 bit/channel);
    triangles are filled by barycentric (Gouraud) interpolation of vertex colors.
  * Byte cost uses *ideal entropy* (adaptive occupancy map + palette + index
    stream) — i.e. a perfect arithmetic coder, which only helps the baseline
    and pre-empts "you crippled it". Rate is swept via the grid size g and the
    vertex budget.

We give it an error-driven greedy vertex placement plus coordinate-descent
color optimization — stronger and more reproducible than the paper's random
hill-climb, so the comparison is not against a weak baseline. Validate against
the paper's anchor with `--validate` (221px, ~200 B → ~25 dB on simple content).

Usage:
    uv run python scripts/paper/marwood_baseline.py --validate
    uv run python scripts/paper/marwood_baseline.py            # Kodak R-D vs arthash
"""
from __future__ import annotations

import argparse
import math
from pathlib import Path

import numpy as np
from PIL import Image
from scipy.spatial import Delaunay

ROOT = Path(__file__).resolve().parents[2]
BENCH = ROOT / "bench"
KODAK = BENCH / "kodak"
ENCODE_EDGE = 100
EVAL_EDGE = 256


# ----------------------------------------------------------------- model ---

def palette_quantize(small: np.ndarray, k: int) -> tuple[np.ndarray, np.ndarray]:
    """K-color palette + per-pixel index via PIL median-cut. Returns
    (palette[K,3] float 0-1, index_map[H,W])."""
    im = Image.fromarray(small, "RGB").quantize(colors=k, method=Image.MEDIANCUT, dither=Image.NONE)
    pal = np.array(im.getpalette()[: k * 3], dtype=np.float64).reshape(-1, 3)[:k] / 255.0
    idx = np.array(im, dtype=np.int64)
    return pal, idx


def gouraud(verts_grid: np.ndarray, vcolors: np.ndarray, g: int, H: int, W: int,
            tri: Delaunay | None = None) -> tuple[np.ndarray, Delaunay]:
    """Render grid vertices (each with an RGB color) by barycentric
    interpolation onto an H×W canvas. Corners are assumed present so every
    pixel lies inside the convex hull."""
    verts = verts_grid.astype(np.float64) / (g - 1) * np.array([W - 1, H - 1])
    if tri is None:
        tri = Delaunay(verts)
    yy, xx = np.mgrid[0:H, 0:W]
    pix = np.column_stack([xx.ravel(), yy.ravel()]).astype(np.float64)
    simplex = tri.find_simplex(pix)
    simplex = np.clip(simplex, 0, None)
    T = tri.transform[simplex]
    b01 = np.einsum("nij,nj->ni", T[:, :2, :], pix - T[:, 2, :])
    bary = np.column_stack([b01, 1.0 - b01.sum(1)])
    tvtx = tri.simplices[simplex]
    cols = vcolors[tvtx]
    out = (bary[:, :, None] * cols).sum(1)
    return out.reshape(H, W, 3), tri


def grid_pixel(gx: int, gy: int, g: int, H: int, W: int) -> tuple[int, int]:
    return min(round(gx / (g - 1) * (W - 1)), W - 1), min(round(gy / (g - 1) * (H - 1)), H - 1)


def entropy_bits(counts: np.ndarray) -> float:
    n = counts.sum()
    if n == 0:
        return 0.0
    p = counts[counts > 0] / n
    return float(-(p * np.log2(p)).sum() * n)


def byte_cost(vt: int, ng: int, k: int, idx_counts: np.ndarray) -> int:
    """Ideal-entropy size: adaptive occupancy map + palette + index stream."""
    p = vt / ng
    occ = ng * (-(p * math.log2(p) + (1 - p) * math.log2(1 - p))) if 0 < p < 1 else 0.0
    palette = k * 3 * 6  # 6 bit / YCoCg channel
    indices = entropy_bits(idx_counts)
    header = 16  # grid size, vertex count, palette size
    return math.ceil((header + occ + palette + indices) / 8)


# ------------------------------------------------------------------ fit ----

def fit(small: np.ndarray, g: int, budget_vertices: int, k: int,
        color_passes: int = 1):
    """Error-driven greedy vertex placement on a g×g grid + palette colors."""
    H, W, _ = small.shape
    target = small.astype(np.float64) / 255.0
    pal, idx_map = palette_quantize(small, k)

    used: set[tuple[int, int]] = set()
    verts: list[tuple[int, int]] = []
    for gx, gy in [(0, 0), (g - 1, 0), (0, g - 1), (g - 1, g - 1)]:
        used.add((gx, gy)); verts.append((gx, gy))

    def vcolors_of(vs):
        out = np.empty((len(vs), 3))
        for i, (gx, gy) in enumerate(vs):
            px, py = grid_pixel(gx, gy, g, H, W)
            out[i] = pal[idx_map[py, px]]
        return out

    tri = None
    while len(verts) < budget_vertices:
        vc = vcolors_of(verts)
        recon, tri = gouraud(np.array(verts), vc, g, H, W)
        err = ((recon - target) ** 2).sum(2)
        # pick the unused grid point with the largest accumulated error nearby
        best, best_e = None, -1.0
        gs = np.linspace(0, g - 1, g, dtype=int)
        for gy in gs:
            for gx in gs:
                if (gx, gy) in used:
                    continue
                px, py = grid_pixel(gx, gy, g, H, W)
                e = err[py, px]
                if e > best_e:
                    best_e, best = e, (gx, gy)
        if best is None:
            break
        used.add(best); verts.append(best)

    # coordinate-descent: re-pick each vertex's palette index to minimize
    # the rendered error (vertex colors interact via interpolation).
    varr = np.array(verts)
    vidx = np.array([idx_map[grid_pixel(gx, gy, g, H, W)[1], grid_pixel(gx, gy, g, H, W)[0]]
                     for gx, gy in verts])
    tri = Delaunay(varr.astype(np.float64) / (g - 1) * np.array([W - 1, H - 1]))
    for _ in range(color_passes):
        for vi in range(len(verts)):
            base = pal[vidx]
            best_e, best_c = None, vidx[vi]
            for c in range(k):
                vidx[vi] = c
                recon, _ = gouraud(varr, pal[vidx], g, H, W, tri)
                e = ((recon - target) ** 2).mean()
                if best_e is None or e < best_e:
                    best_e, best_c = e, c
            vidx[vi] = best_c

    counts = np.bincount(vidx, minlength=k).astype(np.float64)
    nbytes = byte_cost(len(verts), g * g, k, counts)
    return varr, vidx, pal, g, nbytes


def render_at(varr, vidx, pal, g, H, W) -> np.ndarray:
    recon, _ = gouraud(varr, pal[vidx], g, H, W)
    return np.clip(recon * 255 + 0.5, 0, 255).astype(np.uint8)


# --------------------------------------------------------------- metrics ---

def psnr(a, b):
    mse = ((a.astype(np.float64) - b.astype(np.float64)) ** 2).mean()
    return float("inf") if mse <= 1e-12 else 20 * math.log10(255 / math.sqrt(mse))


def resize_long(im: Image.Image, edge: int) -> Image.Image:
    s = edge / max(im.size)
    return im.resize((max(1, round(im.width * s)), max(1, round(im.height * s))), Image.LANCZOS)


# ---- rate points: (grid g, vertex budget, palette K) chosen to span bytes --
RATE_POINTS = [(16, 14, 8), (24, 28, 8), (32, 48, 12), (40, 80, 12), (48, 130, 16)]


def validate() -> None:
    """Cross-check against the paper anchor: 221px, ~200 B -> ~25 dB."""
    imgs = sorted(KODAK.glob("kodim*.png"))[:8]
    print("Marwood-baseline validation @221px (paper anchor: ~25 dB @~200 B)\n")
    print(f"{'image':9s} {'bytes':>6s} {'PSNR':>7s}")
    for p in imgs:
        full = Image.open(p).convert("RGB")
        s221 = np.array(resize_long(full, 221), dtype=np.uint8)
        H, W, _ = s221.shape
        varr, vidx, pal, g, nb = fit(s221, g=48, budget_vertices=150, k=16)
        rec = render_at(varr, vidx, pal, g, H, W)
        print(f"{p.stem:9s} {nb:6d} {psnr(s221, rec):7.2f}")


def _arthash_curve(family: str):
    """(bytes, psnr, lpips) per shape count for an arthash family, from the
    R-D run, sorted by bytes."""
    import csv
    from collections import defaultdict
    rows = list(csv.DictReader((BENCH / "rd_results_kodak.csv").open()))
    agg: dict[str, dict[str, list]] = defaultdict(lambda: defaultdict(list))
    for r in rows:
        if r["method"].startswith(family) and r["method"].rsplit("-", 1)[-1].isdigit():
            agg[r["method"]]["b"].append(float(r["bytes"]))
            agg[r["method"]]["p"].append(float(r["psnr"]))
            agg[r["method"]]["l"].append(float(r["lpips"]))
    pts = [(float(np.mean(v["b"])), float(np.mean(v["p"])), float(np.mean(v["l"])))
           for v in agg.values()]
    return sorted(pts)


def plot_vs_arthash(marwood) -> None:
    """Two panels — PSNR (Marwood wins) and LPIPS (arthash wins) vs bytes."""
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt
    mb = sorted(marwood)
    tri, cir = _arthash_curve("arthash-triangle"), _arthash_curve("arthash-circle")
    fig, (a1, a2) = plt.subplots(1, 2, figsize=(12, 4.5))
    for ax, yi, ylab in [(a1, 1, "PSNR (dB) ↑"), (a2, 2, "LPIPS ↓")]:
        ax.plot([m[0] for m in mb], [m[yi] for m in mb], "-s", color="0.3",
                label="Marwood ICIP'18 (our reimpl)")
        ax.plot([t[0] for t in tri], [t[yi] for t in tri], "-o", label="arthash triangle")
        ax.plot([c[0] for c in cir], [c[yi] for c in cir], "-o", label="arthash circle")
        ax.set_xscale("log"); ax.set_xlabel("bytes"); ax.set_ylabel(ylab); ax.grid(True, alpha=0.3)
    a1.legend(fontsize=8)
    a1.set_title("PSNR — Marwood wins (MSE-optimal)")
    a2.set_title("LPIPS — arthash wins (perceptual)")
    fig.suptitle("arthash vs Marwood ICIP'18 triangulation (Kodak)")
    fig.tight_layout()
    out = BENCH / "marwood_vs_arthash.png"
    fig.savefig(out, dpi=140)
    print(f"plot: {out}")


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--validate", action="store_true")
    ap.add_argument("--plot-only", action="store_true", help="replot from marwood_kodak.csv")
    ap.add_argument("--limit", type=int, default=24)
    args = ap.parse_args()
    if args.validate:
        validate()
        return
    if args.plot_only:
        import csv
        rows = list(csv.DictReader((BENCH / "marwood_kodak.csv").open()))
        plot_vs_arthash([(float(r["bytes"]), float(r["psnr"]), float(r["lpips"]),
                          float(r["dists"])) for r in rows])
        return

    try:
        import lpips as lpips_mod
        import torch
        from DISTS_pytorch import DISTS
        net = lpips_mod.LPIPS(net="alex", verbose=False).eval()
        dists = DISTS().eval()

        def perc(a, b):
            with torch.no_grad():
                ta = torch.from_numpy(a).permute(2, 0, 1).unsqueeze(0).float() / 255
                tb = torch.from_numpy(b).permute(2, 0, 1).unsqueeze(0).float() / 255
                return float(net(ta * 2 - 1, tb * 2 - 1).item()), float(dists(ta, tb).item())
    except ImportError:
        perc = None

    imgs = sorted(KODAK.glob("kodim*.png"))[: args.limit]
    print(f"Marwood-style triangulation baseline, Kodak ({len(imgs)} imgs), "
          f"100px input / 256px eval\n")
    print(f"{'g/Vt/K':>12s} {'bytes':>6s} {'PSNR':>7s} {'LPIPS':>7s} {'DISTS':>7s}")
    results = []
    for g, vt, k in RATE_POINTS:
        rows = []
        for p in imgs:
            full = Image.open(p).convert("RGB")
            small = np.array(resize_long(full, ENCODE_EDGE), dtype=np.uint8)
            gt = resize_long(full, EVAL_EDGE)
            gt_arr = np.array(gt, dtype=np.uint8)
            varr, vidx, pal, gg, nb = fit(small, g, vt, k)
            rec = render_at(varr, vidx, pal, gg, gt.height, gt.width)
            ps = psnr(gt_arr, rec)
            lp, ds = perc(gt_arr, rec) if perc else (float("nan"), float("nan"))
            rows.append((nb, ps, lp, ds))
        agg = [float(np.mean([r[i] for r in rows])) for i in range(4)]
        results.append(agg)
        print(f"{f'{g}/{vt}/{k}':>12s} {agg[0]:6.1f} {agg[1]:7.2f} {agg[2]:7.4f} {agg[3]:7.4f}",
              flush=True)

    import csv
    with (BENCH / "marwood_kodak.csv").open("w", newline="") as f:
        w = csv.writer(f); w.writerow(["bytes", "psnr", "lpips", "dists"])
        for r in results:
            w.writerow([round(x, 4) for x in r])
    if perc:
        plot_vs_arthash(results)


if __name__ == "__main__":
    main()

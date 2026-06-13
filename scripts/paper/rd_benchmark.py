"""Full rate-distortion benchmark for the paper: arthash (all modes, n_shapes
sweep) vs blurhash (component sweep) vs thumbhash vs tiny JPEG/WebP.

Datasets: bench/kodak (24), bench/clic (CLIC2020 professional valid, 41).
Protocol: encode from a 100px-long-edge LANCZOS thumbnail; decode/upscale to a
256px-long-edge canvas; compare against the 256px LANCZOS ground truth.
Metrics: bytes, PSNR, SSIM, LPIPS(alex), DISTS.

Outputs:
    bench/rd_results_<dataset>.csv      per-image rows
    bench/rd_curves_<dataset>.png       R-D curves (PSNR / LPIPS / DISTS vs bytes)

Usage:
    uv run python scripts/rd_benchmark.py [kodak|clic ...]
"""
from __future__ import annotations

import csv
import io
import math
import sys
from pathlib import Path

import numpy as np
from PIL import Image

import blurhash
import thash
from arthash import Codec, ShapeType, decode, encode

ROOT = Path(__file__).resolve().parents[2]
BENCH = ROOT / "bench"

ENCODE_EDGE = 100
EVAL_EDGE = 256

DATASETS = {
    "kodak": BENCH / "kodak",
    "clic": BENCH / "clic",
}


def resize_long(im: Image.Image, edge: int) -> Image.Image:
    w, h = im.size
    s = edge / max(w, h)
    return im.resize((max(1, round(w * s)), max(1, round(h * s))), Image.LANCZOS)


def psnr(a: np.ndarray, b: np.ndarray) -> float:
    mse = ((a.astype(np.float64) - b.astype(np.float64)) ** 2).mean()
    return float("inf") if mse <= 1e-12 else 20 * math.log10(255.0 / math.sqrt(mse))


class PerceptualMetrics:
    def __init__(self) -> None:
        import lpips
        import torch
        from DISTS_pytorch import DISTS

        self.torch = torch
        self.lpips = lpips.LPIPS(net="alex", verbose=False).eval()
        self.dists = DISTS().eval()

    def __call__(self, a: np.ndarray, b: np.ndarray) -> tuple[float, float]:
        t = self.torch
        with t.no_grad():
            # LPIPS expects [-1, 1]; DISTS expects [0, 1]
            ta01 = t.from_numpy(a).permute(2, 0, 1).unsqueeze(0).float() / 255.0
            tb01 = t.from_numpy(b).permute(2, 0, 1).unsqueeze(0).float() / 255.0
            lp = float(self.lpips(ta01 * 2 - 1, tb01 * 2 - 1).item())
            ds = float(self.dists(ta01, tb01).item())
        return lp, ds


def ssim_rgb(a: np.ndarray, b: np.ndarray) -> float:
    from skimage.metrics import structural_similarity

    return float(structural_similarity(a, b, channel_axis=2, data_range=255))


# ---------------------------------------------------------------- methods --

def arthash_method(shape: ShapeType, n_shapes: int):
    def run(small: Image.Image, tw: int, th: int) -> tuple[Image.Image, int]:
        codec = Codec() if shape == ShapeType.DCT else Codec(shape=shape, n_shapes=n_shapes)
        arr = np.array(small, dtype=np.uint8)
        hb = encode(arr, codec) if shape == ShapeType.DCT else encode(arr, codec, seed=0)
        w, h, pixels = decode(hb, codec, base_size=max(tw, th), aa=(shape != ShapeType.DCT))
        if shape == ShapeType.DCT:
            rgba = np.frombuffer(pixels, dtype=np.uint8).reshape(h, w, 4)
        else:
            rgba = np.asarray(pixels)
        img = Image.fromarray(rgba[..., :3], "RGB")
        return img.resize((tw, th), Image.LANCZOS), len(hb)

    return run


def thumbhash_method(small: Image.Image, tw: int, th: int) -> tuple[Image.Image, int]:
    rgba = np.array(small.convert("RGBA"), dtype=np.uint8)
    hb = bytes(thash.rgba_to_thumb_hash(small.width, small.height, rgba.flatten().tolist()))
    w, h, pix = thash.thumb_hash_to_rgba(hb)
    arr = np.frombuffer(bytes(pix), dtype=np.uint8).reshape(h, w, 4)
    img = Image.fromarray(arr[..., :3], "RGB").resize((tw, th), Image.LANCZOS)
    return img, len(hb)


def blurhash_method(cx: int, cy: int):
    def run(small: Image.Image, tw: int, th: int) -> tuple[Image.Image, int]:
        s = blurhash.encode(np.array(small), components_x=cx, components_y=cy)
        dw, dh = (32, max(1, round(32 * th / tw))) if tw >= th else (max(1, round(32 * tw / th)), 32)
        arr = np.asarray(blurhash.decode(s, dw, dh), dtype=np.uint8)
        img = Image.fromarray(arr, "RGB").resize((tw, th), Image.LANCZOS)
        return img, len(s.encode("ascii"))

    return run


def pil_codec_method(fmt: str, edge: int, quality: int):
    """Tiny JPEG/WebP baseline: shrink to `edge`, save at `quality`, decode+upscale.
    Bytes = full file size (the honest comparison: these formats carry headers)."""

    def run(small: Image.Image, tw: int, th: int) -> tuple[Image.Image, int]:
        tiny = resize_long(small, edge)
        buf = io.BytesIO()
        tiny.save(buf, fmt, quality=quality, optimize=(fmt == "JPEG"))
        data = buf.getvalue()
        img = Image.open(io.BytesIO(data)).convert("RGB").resize((tw, th), Image.LANCZOS)
        return img, len(data)

    return run


SHAPE_SWEEP = [4, 8, 12, 24, 48]
METHODS: dict[str, object] = {}
for cx, cy in [(3, 3), (4, 3), (5, 4), (6, 4), (8, 6), (9, 9)]:
    METHODS[f"blurhash-{cx}x{cy}"] = blurhash_method(cx, cy)
METHODS["thumbhash"] = thumbhash_method
METHODS["arthash-dct"] = arthash_method(ShapeType.DCT, 0)
for st, label in [(ShapeType.PIXEL, "pixel"), (ShapeType.CIRCLE, "circle"),
                  (ShapeType.TRIANGLE, "triangle"), (ShapeType.RECT, "rect")]:
    for n in SHAPE_SWEEP:
        METHODS[f"arthash-{label}-{n}"] = arthash_method(st, n)
for edge in (24, 32, 48):
    METHODS[f"jpeg-{edge}px-q10"] = pil_codec_method("JPEG", edge, 10)
    METHODS[f"webp-{edge}px-q1"] = pil_codec_method("WEBP", edge, 1)


# ----------------------------------------------------------------- runner --

def family(method: str) -> str:
    if method.startswith(("blurhash", "jpeg", "webp")):
        return method.rsplit("-", 1)[0] if method.startswith("blurhash") else method.split("-")[0]
    if method.startswith("arthash-") and method.rsplit("-", 1)[-1].isdigit():
        return method.rsplit("-", 1)[0]
    return method


def run_dataset(name: str, perceptual: PerceptualMetrics) -> None:
    img_dir = DATASETS[name]
    images = sorted(p for p in img_dir.rglob("*.png")
                    if not p.name.startswith("._") and "__MACOSX" not in p.parts)
    if not images:
        print(f"skip {name}: no images in {img_dir}")
        return
    rows = []
    for i, path in enumerate(images):
        full = Image.open(path).convert("RGB")
        gt = resize_long(full, EVAL_EDGE)
        small = resize_long(full, ENCODE_EDGE)
        gt_arr = np.array(gt, dtype=np.uint8)
        for mname, fn in METHODS.items():
            img, nbytes = fn(small, gt.width, gt.height)
            arr = np.array(img, dtype=np.uint8)
            lp, ds = perceptual(gt_arr, arr)
            rows.append({
                "image": path.stem, "method": mname, "bytes": nbytes,
                "psnr": round(psnr(gt_arr, arr), 3),
                "ssim": round(ssim_rgb(gt_arr, arr), 4),
                "lpips": round(lp, 4), "dists": round(ds, 4),
            })
        print(f"[{name}] {i + 1}/{len(images)} {path.stem}", flush=True)

    out_csv = BENCH / f"rd_results_{name}.csv"
    with out_csv.open("w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=list(rows[0].keys()))
        w.writeheader()
        w.writerows(rows)

    # summary + plot
    print(f"\n=== {name}: mean over {len(images)} images ===")
    print(f"{'method':24s} {'bytes':>7s} {'PSNR':>7s} {'SSIM':>7s} {'LPIPS':>7s} {'DISTS':>7s}")
    summary = {}
    for mname in METHODS:
        sub = [r for r in rows if r["method"] == mname]
        m = {k: float(np.mean([r[k] for r in sub])) for k in ("bytes", "psnr", "ssim", "lpips", "dists")}
        summary[mname] = m
        print(f"{mname:24s} {m['bytes']:7.1f} {m['psnr']:7.2f} {m['ssim']:7.4f} {m['lpips']:7.4f} {m['dists']:7.4f}")

    plot(name, summary)
    print(f"CSV: {out_csv}")


def plot(name: str, summary: dict[str, dict[str, float]]) -> None:
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt

    fams: dict[str, list[tuple[float, dict[str, float]]]] = {}
    for mname, m in summary.items():
        fams.setdefault(family(mname), []).append((m["bytes"], m))
    fig, axes = plt.subplots(1, 3, figsize=(16, 4.5))
    metrics = [("psnr", "PSNR (dB) ↑"), ("lpips", "LPIPS ↓"), ("dists", "DISTS ↓")]
    for ax, (key, label) in zip(axes, metrics):
        for fam, pts in sorted(fams.items()):
            pts = sorted(pts)
            xs = [p[0] for p in pts]
            ys = [p[1][key] for p in pts]
            style = "-o" if len(pts) > 1 else "D"
            ax.plot(xs, ys, style, label=fam, markersize=4)
        ax.set_xscale("log")
        ax.set_xlabel("bytes")
        ax.set_ylabel(label)
        ax.grid(True, alpha=0.3)
    axes[0].legend(fontsize=8)
    fig.suptitle(f"Rate-distortion on {name} (256px eval, 100px encoder input)")
    fig.tight_layout()
    out = BENCH / f"rd_curves_{name}.png"
    fig.savefig(out, dpi=140)
    print(f"plot: {out}")


def main() -> None:
    targets = sys.argv[1:] or list(DATASETS)
    perceptual = PerceptualMetrics()
    for name in targets:
        run_dataset(name, perceptual)


if __name__ == "__main__":
    main()

"""Fetch the evaluation corpora for the rate-distortion study.

Downloads into bench/ (git-ignored):
  * Kodak (24 images)       -> bench/kodak/kodim01..24.png
  * CLIC2020 pro valid (41) -> bench/clic/**/*.png

Idempotent: skips files already present. Run once before rd_benchmark.py /
refine_ablation.py / entropy_eval.py.

Usage:
    uv run python scripts/paper/fetch_datasets.py [--only kodak|clic]
"""
from __future__ import annotations

import argparse
import io
import urllib.request
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
BENCH = ROOT / "bench"
KODAK_URL = "https://r0k.us/graphics/kodak/kodak/kodim{:02d}.png"
CLIC_URL = "https://data.vision.ee.ethz.ch/cvl/clic/professional_valid_2020.zip"


def fetch_kodak() -> None:
    out = BENCH / "kodak"
    out.mkdir(parents=True, exist_ok=True)
    for i in range(1, 25):
        dst = out / f"kodim{i:02d}.png"
        if dst.exists():
            continue
        print(f"  kodim{i:02d}.png", flush=True)
        urllib.request.urlretrieve(KODAK_URL.format(i), dst)
    print(f"Kodak -> {out} ({len(list(out.glob('*.png')))} images)")


def fetch_clic() -> None:
    out = BENCH / "clic"
    pngs = [p for p in out.rglob("*.png") if not p.name.startswith("._")] if out.exists() else []
    if len(pngs) >= 41:
        print(f"CLIC -> {out} (already have {len(pngs)} images)")
        return
    out.mkdir(parents=True, exist_ok=True)
    print("  downloading CLIC2020 professional valid (~258 MB) ...", flush=True)
    with urllib.request.urlopen(CLIC_URL) as r:
        data = r.read()
    with zipfile.ZipFile(io.BytesIO(data)) as z:
        z.extractall(out)
    pngs = [p for p in out.rglob("*.png") if not p.name.startswith("._")]
    print(f"CLIC -> {out} ({len(pngs)} images)")


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--only", choices=["kodak", "clic"], default=None)
    args = ap.parse_args()
    if args.only in (None, "kodak"):
        fetch_kodak()
    if args.only in (None, "clic"):
        fetch_clic()


if __name__ == "__main__":
    main()

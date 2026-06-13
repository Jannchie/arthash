# Rate-distortion study (paper experiments)

Reproducible scaffolding for the sub-300-byte placeholder rate-distortion study
— the perceptual comparison of arthash shape modes against blurhash / thumbhash
/ tiny JPEG+WebP, the joint-refinement ablation, and the entropy-coding
headroom measurement. Findings are written up in [`docs/RD_STUDY.md`](../../docs/RD_STUDY.md).

Everything here is **research tooling**, not shipped library code. Datasets and
figures are git-ignored (fetched / regenerated on demand).

## Setup

```bash
# 1. Build the native extension (refine_passes lives in Rust)
cd packages/arthash-py && uv run maturin develop --release && cd ../..

# 2. Research-only Python deps (kept out of pyproject so CI's uv sync stays lean)
uv pip install lpips torch --index-url https://download.pytorch.org/whl/cpu
uv pip install scikit-image DISTS-pytorch matplotlib blurhash thumbhash-py

# 3. Fetch the corpora (-> bench/kodak, bench/clic; ~273 MB, git-ignored)
uv run python scripts/paper/fetch_datasets.py
```

## Protocol

Each method encodes from a **100 px long-edge** LANCZOS thumbnail, decodes and
upscales to a **256 px long-edge** canvas, and is scored against the 256 px
LANCZOS ground truth. Metrics: bytes, PSNR, SSIM, LPIPS(alex), DISTS.

## Scripts

| Script | What it produces |
| --- | --- |
| `fetch_datasets.py` | Downloads Kodak (24) + CLIC2020 pro-valid (41) into `bench/`. |
| `rd_benchmark.py [kodak\|clic ...]` | Full R-D sweep across all methods → `bench/rd_results_<ds>.csv` + `bench/rd_curves_<ds>.png` (PSNR / LPIPS / DISTS vs bytes). |
| `refine_ablation.py [ds] [--passes N]` | Greedy vs quantization-aware joint refinement, full metric suite → `bench/refine_ablation_<ds>.csv`. |
| `entropy_eval.py [--train ds] [--test ds]` | Entropy-coding headroom: static per-field model cross-entropy vs the fixed-width wire size. |
| `speed_benchmark.py` | Encode-latency vs LPIPS Pareto (joins `rd_results_kodak.csv`) → `bench/speed_quality_kodak.png`. Run `rd_benchmark.py kodak` first. |
| `perceptual_poc.py` | Weighted-objective ablation (edge / center / saliency) — the negative result behind RD_STUDY §2.2. |

```bash
uv run python scripts/paper/rd_benchmark.py kodak clic
uv run python scripts/paper/refine_ablation.py kodak --passes 2
uv run python scripts/paper/entropy_eval.py --train clic --test kodak
```

## Notes

* After editing any Rust under `packages/arthash-rs/`, re-run `uv run maturin
  develop --release` — `uv pip install` does **not** refresh `_native.pyd`.
* `refine_passes` is an opt-in `SearchOptions` knob (default `0`); it does not
  change the byte format, so the byte-compat regression suite stays green.
* CSVs and `rd_curves_*.png` are git-ignored — headline numbers are frozen into
  `docs/RD_STUDY.md` so the report stays self-contained.

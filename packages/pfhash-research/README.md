# @pfhash/research

R&D playground for pfhash. Everything here is exploratory — code shape isn't
stable, APIs break freely, and outputs feed into design decisions for
[`@pfhash/py`](../pfhash-py/) and the SPEC.

The Python research module (`research/`) is **gitignored** by design: it
contains a 47 MB corpus + results directory, and the scripts are kept as a
historical record rather than a packaged deliverable. Running them locally
needs the corpus to be built first.

When a research experiment stabilizes it graduates to `@pfhash/py` proper;
the experiment file stays here as the history.

## Layout

```
pfhash-research/
├── research/             Python module — gitignored
│   ├── runner.py          main encode+decode+metrics loop
│   ├── baselines.py       thash / pfhash / blurhash wrappers
│   ├── metrics.py         SSIM, MS-SSIM, ΔE2000, aspect error, composite
│   ├── compare.py         side-by-side visual grid
│   ├── ablation.py        sweep parameters across the corpus
│   ├── explore_*.py       oracle experiments (basis, color space, compander)
│   ├── bench_vs_*.py      head-to-head comparisons vs sqip / thumbhash
│   ├── corpus/            test images (gitignored, fetch_corpus.py builds)
│   └── results/           DataFrames, PNGs (gitignored)
├── primitive-bench/      Go micro-benchmark of the primitive-shape search
└── sqip-bench/           Node bench of sqip used as a reference baseline
```

## Run

```sh
# Build the corpus (downloads CC-licensed Wikimedia images + samples local
# DCIM + generates solids/synthetics):
uv run python -m research.fetch_corpus

# Run all baselines + metrics over the corpus:
uv run python -m research.runner
uv run python -m research.runner --skip blurhash_4x3   # skip the slow one

# Render a side-by-side visual grid (top 16 most-improved images):
uv run python -m research.compare --top 16 --out research/results/compare.png
```

Outputs land in `research/results/` (gitignored).

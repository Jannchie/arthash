# Paper draft — *Geometric Primitives Dominate the Perceptual Frontier of Sub-300-Byte Image Placeholders*

ICIP-style draft (IEEEtran, conference). Sourced entirely from the experiments
in [`docs/RD_STUDY.md`](../docs/RD_STUDY.md) and the tooling in
[`scripts/paper/`](../scripts/paper/).

## Build

```bash
# 1. Generate the figures (datasets + deps per scripts/paper/README.md)
uv run python scripts/paper/rd_benchmark.py kodak
uv run python scripts/paper/speed_benchmark.py
uv run python scripts/paper/marwood_baseline.py        # writes bench/marwood_vs_arthash.png

# 2. Stage them where main.tex expects (figures/ is git-ignored)
mkdir -p paper/figures
cp bench/rd_curves_kodak.png bench/speed_quality_kodak.png \
   bench/marwood_vs_arthash.png paper/figures/

# 3. Compile
cd paper && pdflatex main && pdflatex main
```

The three figures are git-ignored (regenerated from data), so the `.tex` source
is the versioned artifact; the numbers in the prose are frozen and match
`docs/RD_STUDY.md`.

## Status

Complete first draft, all sections written from real results. Open items before
submission:

- Author/affiliation (currently withheld).
- Optional corpus widening (DIV2K-valid) for variance reporting — RD_STUDY §4.
- SQIP's LPIPS point on the Marwood/quality axes (its `sharp` dep is broken
  locally; only its latency is currently plotted) — RD_STUDY §4.
- Convert `\begin{thebibliography}` to a `.bib` if the venue prefers BibTeX.

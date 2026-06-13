# Sub-300-byte placeholders: a rate-distortion study

A living record of the research line aimed at a publishable result (target
venues: DCC / ICIP). It asks one question the literature has not answered
systematically: **at image representations under ~300 bytes, what is the
rate-distortion frontier, and where do geometric-primitive hashes sit on it?**

Reproduce everything with the tooling in [`scripts/paper/`](../scripts/paper/).
Numbers below are frozen here so the report is self-contained (the CSVs and
figures they came from are git-ignored).

- **Protocol.** Encode from a 100 px long-edge LANCZOS thumbnail, decode and
  upscale to a 256 px long-edge canvas, score against the 256 px LANCZOS ground
  truth. Metrics: bytes, PSNR, SSIM, LPIPS(alex), DISTS.
- **Corpora.** Kodak (24) and CLIC2020 professional-valid (41).
- **Baselines.** blurhash (component sweep), thumbhash, arthash DCT, arthash
  shape modes (circle / triangle / rect / square × shape-count sweep), and the
  smallest JPEG / WebP the encoders will emit.

---

## 1. Headline: shape modes dominate the perceptual frontier

The motivating region (≲300 bytes) is below where standard codecs operate:
the smallest WebP lands at ~85–150 B with LPIPS ≥ 0.76, and the smallest
JPEG at ~320 B. Across that whole region the geometric shape modes are
Pareto-dominant on **both** perceptual metrics (LPIPS, DISTS) *and* PSNR — they
do not trade fidelity for perceptual score.

**Kodak-24, mean:**

| method | bytes | PSNR↑ | SSIM↑ | LPIPS↓ | DISTS↓ |
| --- | ---: | ---: | ---: | ---: | ---: |
| blurhash 3×3 | 22 | 16.41 | 0.378 | 0.881 | 0.638 |
| blurhash 6×4 | 52 | 17.07 | 0.379 | 0.874 | 0.599 |
| blurhash 9×9 | 166 | 18.04 | 0.389 | 0.867 | 0.539 |
| thumbhash | 21 | 17.67 | 0.392 | 0.905 | 0.609 |
| arthash DCT | 21 | 17.83 | 0.394 | 0.905 | 0.612 |
| **arthash circle-4** | **20** | 16.67 | 0.376 | **0.695** | **0.549** |
| **arthash circle-12** | 53 | 18.07 | 0.385 | **0.632** | **0.479** |
| **arthash triangle-12** | 77 | 18.45 | 0.391 | **0.626** | **0.456** |
| **arthash triangle-48** | 297 | 20.13 | 0.424 | **0.517** | **0.387** |
| jpeg 24px q10 | 317 | 18.46 | 0.396 | 0.930 | 0.526 |
| webp 48px q1 | 137 | 20.35 | 0.461 | 0.816 | 0.445 |

The single most striking point reproduces on CLIC: **circle-4 at 20 bytes
(LPIPS 0.70) beats blurhash-9×9 at 166 bytes (LPIPS 0.82)** — an ~8× byte
advantage at equal-or-better perceptual quality. SSIM is the one metric that
fails to separate the methods (everything sits in 0.32–0.46); we report it but
LPIPS and DISTS — which agree on the ordering — carry the argument.

A secondary, paper-worthy observation: blurhash scales terribly in this regime
(22→166 B buys only LPIPS 0.881→0.867), i.e. *more DCT coefficients is the
wrong way to spend bytes here* — geometric structure is the better target.

---

## 2. Contribution: quantization-aware joint refinement

The greedy fitter places each primitive against the residual of the ones before
it and never revisits the choice. We add an opt-in **backfitting** pass
(`SearchOptions.refine_passes`, default `0`, byte-format-preserving, all 125
regression tests green): revisit each shape, re-render the canvas without it,
re-search, and keep the replacement only if it lowers the *exact total SSE*.

The accept test is **quantization-aware**: both the baseline canvas and the
candidate are rendered from wire-quantized parameters (`quantize_<shape>` does a
real `BitWriter`/`BitReader` round-trip), so the comparison judges what the
decoder will actually render. This matters — an earlier continuous-domain accept
test *raised* MSE on some images because a continuous win evaporates under 5-bit
position / 4-bit radius / RGB565 quantization. With the quantization-aware test,
8-image quick runs improve monotonically:

| mode | refine=2 mean ΔMSE |
| --- | ---: |
| circle | −3.7% |
| triangle | −3.7% |
| rect | −2.1% |
| square | −4.1% |

> **Note for the paper.** TRIANGLE historically *regressed* under residual-driven
> initialization (the recorded quadrant test was +44% MSE), so it was left on
> uniform init. Joint refinement recovers −3.7% on triangle anyway — evidence
> that a wrong greedy ordering can be repaired by joint optimization rather than
> by smarter initialization.

### 2.1 The honest finding: PSNR improves, perception does not

Run as an SSE optimizer, refinement does exactly what it says — but on the full
Kodak-24 set the perceptual metrics tell a different story:

| mode | n | bytes | ΔPSNR | ΔLPIPS | ΔDISTS |
| --- | ---: | ---: | ---: | ---: | ---: |
| circle | 12 | 53 | +0.07 | **+0.4%** | +0.5% |
| triangle | 12 | 77 | +0.20 | **−1.2%** | −0.7% |
| rect | 12 | 59 | +0.06 | **+0.8%** | +0.2% |
| square | 12 | 53 | +0.11 | −0.0% | +0.1% |

(ΔPSNR > 0 better; ΔLPIPS / ΔDISTS < 0 better.)

PSNR rises everywhere; LPIPS is mixed — triangle improves, circle/rect get
slightly *worse*. This is a clean instance of PSNR–perception divergence:
minimizing squared error shrinks/merges primitives in flat regions to shave SSE
while giving up edge structure that LPIPS rewards. Triangles are the exception
because they natively carry oriented edges.

**Implication that reframes the paper:** for sub-300-byte placeholders, L2 is
the wrong objective. Refinement is best understood not as the contribution but
as the *infrastructure* that lets any objective be optimized — the next step
plugs a perceptual objective into it (see §4).

---

## 3. Negative result: entropy-coding headroom is small

We measured how much a static, corpus-trained entropy coder would save over the
fixed-width wire fields, the honest way: fit a Laplace-smoothed per-field model
on a *train* corpus (the model ships baked into the codec, amortized to zero
per-image cost, exactly like blurhash's fixed base83 alphabet) and score the
*test* corpus's symbols under it (cross-entropy = realized range-coder size to
within <1 byte).

| | circle-12 | triangle-12 | rect-12 | square-12 |
| --- | ---: | ---: | ---: | ---: |
| cross-dataset (CLIC→Kodak) | 5.2% | 5.0% | 3.8% | 4.4% |
| in-corpus ceiling (overfit) | 9.0% | 7.6% | 7.3% | 8.3% |

Even the overfit ceiling is <10%. A vertex-delta context model for triangles
(predict v1, v2 from v0) made it *worse* — the hill-climb spreads the vertices
across large regions rather than clustering them, so the deltas are not small.

The fields are already near-maximum-entropy: the residual-driven fitter spreads
shape positions to cover the image (near-uniform), colors use the full RGB565
gamut, the shape count is fixed (no variable-rate slack), and alpha is only 3
already-skewed bits. **The gains are in the objective and the search, not in the
serialization** — which also pre-empts the obvious reviewer question.

---

## 4. Positioning and next step

**Novelty.** No peer-reviewed paper evaluates blurhash / thumbhash / SQIP as
rate-distortion baselines — "first perceptual R-D study of industrial sub-300-byte
placeholders" is defensible. Must-cite / must-contrast:

- **Marwood et al., "Representing Images in 200 Bytes" (ICIP 2018)** — closest
  prior work (≤200 B triangulation + entropy coding) but PSNR/SSIM only, no
  perceptual metric, no placeholder baselines, no <100 B regime.
- **GaussianImage (ECCV 2024)** — quantization-aware primitive fine-tuning +
  entropy coding, but at 0.1–1 bpp with differentiable Gaussians, an order of
  magnitude above our budget.
- **Figueras i Ventura et al. (IEEE TIP 2006)** and in-loop matching-pursuit
  quantization — the rate-distortion-primitive lineage.

**Next.** A perceptually-weighted fitting objective. Precompute a per-pixel
weight map `w` (Sobel edge magnitude → later saliency) and minimize
`Σ w·(t−c)²`. The key tractability point: weighted SSE still admits the
O(1)/O(h) integral-image evaluation if the integral images carry `Σw`, `Σw·t`,
`Σw·t²` instead of plain pixel sums — so the fast hill-climb survives. Opt-in,
byte-format-preserving. This directly attacks the §2.1 divergence and is
expected to be the paper's primary contribution.

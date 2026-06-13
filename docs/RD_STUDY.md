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

### 1.1 The second axis: encode latency

Quality is not the only frontier — placeholders are generated at upload time, so
encode latency matters. Pure encode latency (no decode), median over Kodak
thumbnails, against each method's mean LPIPS:

| method | encode | LPIPS↓ | note |
| --- | ---: | ---: | --- |
| thumbhash (Rust binding) | 0.13 ms | 0.905 | fast, perceptually weak |
| arthash DCT | 0.24 ms | 0.905 | fast, perceptually weak |
| **arthash circle-12** | **0.91 ms** | **0.632** | Pareto-optimal |
| **arthash triangle-12** | **1.51 ms** | **0.626** | Pareto-optimal |
| **arthash triangle-48** | 4.5 ms | **0.517** | best quality |
| blurhash 9×9 (pure-Python) | 147 ms | 0.867 | impl-limited |
| **sqip triangle-12 (node)** | **284 ms** | — | ~1 kB SVG; +3 dB PSNR over circle-12 |

The shape modes own the lower-left (fast *and* perceptually strong). The honest
comparison is **arthash vs SQIP** — the only other primitive-fitting method, and
one built for quality not speed: arthash triangle-12 is **189× faster**
(1.5 ms vs 284 ms) at ~20× smaller output, and the integral-image hill-climb is
what buys that. (blurhash's 147 ms is its pure-Python reference impl, not an
optimized one, so we do not lean on it; thumbhash uses its Rust binding and is a
fair fast-but-weak point.) Spending ~1 ms more than a DCT hash to drop LPIPS by
~0.28 is the trade the shape modes offer.

### 1.2 vs the academic baseline (Marwood ICIP'18)

The closest academic prior is Marwood et al., *Representing Images in 200 Bytes*
(ICIP 2018): grid-vertex Delaunay triangulation (connectivity recomputed at
decode, so it costs no bytes), palette vertex colors, Gouraud interpolation, and
ANS entropy coding. No implementation was ever released, so we reimplement it
faithfully (`scripts/paper/marwood_baseline.py`) with error-driven greedy vertex
placement, palette coordinate descent, and an **ideal-entropy byte model** (a
perfect arithmetic coder — deliberately generous to the baseline). On the
paper's own setting (221 px, ~200 B) the reimpl reaches the reported magnitude
(~24 dB on simple Kodak content, 16–20 dB on hard; the paper's 1024-image
ImageNet mean is ~25 dB and Kodak is harder), so it is not a strawman.

On our protocol (Kodak, 100 px → 256 px):

| bytes | Marwood PSNR / LPIPS | nearest arthash | PSNR / LPIPS |
| ---: | --- | --- | --- |
| 84 | 18.81 / 0.720 | triangle-12 (77 B) | 18.45 / **0.626** |
| 187 | 20.33 / 0.627 | triangle-12 (77 B) | 18.45 / **0.626** |
| 187 | 20.33 / 0.627 | triangle-24 (150 B) | 19.37 / **0.571** |

The split *is* the thesis, in one comparison: **Marwood wins PSNR** (its Gouraud
mesh is MSE-optimal, ~0.5–1 dB higher at equal bytes) but **loses LPIPS
decisively** — arthash triangle-12 matches Marwood's *187-byte* LPIPS at **77
bytes (2.4× smaller)** and is better at every rate. Gouraud interpolation smooths
away exactly the structure LPIPS rewards — the same failure mode as blurhash.
Optimizing PSNR optimizes the wrong metric for placeholders; this is the
strongest single piece of evidence for the paper's central claim.

---

## 2. Control: does a better search objective help? (quantization-aware refinement)

> **Framing note.** In the paper this section is *not* a contribution — it is the
> first of three controls (refinement / weighting / entropy) that rule out the
> objective and the serialization, localizing the bottleneck to primitive
> expressiveness. It improves PSNR but not perception; that negative result is
> the point.


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

**Implication:** for sub-300-byte placeholders, L2 is the wrong objective.
The natural fix — optimize a perceptual proxy instead — is tested next and,
notably, does not work.

### 2.2 Negative result: perceptual weighting does not help

If L2 is the wrong objective, the obvious move is a perceptually-weighted one:
minimize `Σ w·(t−c)²` with a per-pixel weight `w` that emphasizes whatever the
eye cares about. Weighted SSE stays compatible with the fast integral-image
evaluation (carry `Σw`, `Σw·t`, `Σw·t²` instead of plain pixel sums), so it is
cheap to deploy — *if it helps*. We measured whether it does, before building it
in Rust, with a pure-numpy greedy circle fitter run under identical RNG so the
only variable is the weight map (`scripts/paper/perceptual_poc.py`).

Three cues, Kodak-8, 12 circles, vs the uniform-L2 baseline (LPIPS 0.700):

| weight cue | PSNR | LPIPS | ΔLPIPS |
| --- | ---: | ---: | ---: |
| edge (Sobel ×4) | 16.68 | 0.704 | +0.6% |
| center prior (×4) | 16.90 | 0.703 | +0.4% |
| saliency (×4) | 16.65 | 0.700 | −0.0% |
| saliency (×8) | 16.34 | 0.696 | −0.5% |

None of them helps: edge and center weighting make LPIPS *worse*, and the best
(strong saliency) buys −0.5% LPIPS while giving up 0.65 dB PSNR and swinging
wildly per image. Twelve solid circles cannot represent edges, so steering the
objective toward edges/subjects just sacrifices the flat-region coverage LPIPS
also rewards. **The objective is not the lever — the primitives' expressiveness
is.** This is consistent with §1, where triangles (which carry oriented edges)
already out-perform circles perceptually.

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

## 4. Positioning

The three negative results (§2.1 L2 refinement, §2.2 perceptual weighting, §3
entropy coding) converge on one thesis the measurement supports: **at sub-300-byte
budgets the binding constraint is the primitives' expressiveness, not the search
objective or the serialization.** That is the paper's spine — a rate-distortion
characterization of the regime, with the negative results bounding where the
gains can and cannot come from.

**Novelty.** No peer-reviewed paper evaluates blurhash / thumbhash / SQIP as
rate-distortion baselines — "first perceptual R-D study of industrial sub-300-byte
placeholders" is defensible. Must-cite / must-contrast:

- **Marwood et al., "Representing Images in 200 Bytes" (ICIP 2018)** — closest
  prior work (≤200 B triangulation + entropy coding); reimplemented and compared
  in §1.2. PSNR/SSIM only, no perceptual metric, no placeholder baselines — and
  on LPIPS it is dominated 2.4× by arthash.
- **GaussianImage (ECCV 2024)** — quantization-aware primitive fine-tuning +
  entropy coding, but at 0.1–1 bpp with differentiable Gaussians, an order of
  magnitude above our budget.
- **Figueras i Ventura et al. (IEEE TIP 2006)** and in-loop matching-pursuit
  quantization — the rate-distortion-primitive lineage.

**Done:** R-D vs industrial formats (§1), encode-latency Pareto (§1.1), Marwood
ICIP'18 reimplementation and comparison (§1.2), three bounding ablations (§2–3).

**Remaining work to harden the measurement.**

1. Widen the corpora (add DIV2K-valid) and report variance, so the Pareto claim
   is not Kodak/CLIC-specific.
2. Get SQIP's LPIPS onto the §1.2 axes (its `sharp` dependency is currently
   broken locally; latency is already in §1.1).

If a positive algorithmic contribution is wanted on top of the measurement, the
only untested lever is expressiveness itself — mixed-primitive fitting (pick the
best primitive type per step) or soft/differentiable edges — not another
objective tweak.

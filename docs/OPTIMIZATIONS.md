# pfhash-rs encoder optimizations

This document is the living record of performance work on the shape-mode
encoders (CIRCLE / TRIANGLE). The goal is to keep enough breadcrumbs
behind each optimization that a future maintainer can answer two
questions without re-running anything:

1. **What changed and why?** Each section explains the algorithm, the
   numerical-equivalence argument, and what the surface-level API does
   now that it didn't before.
2. **Did it actually work?** Each section includes the ablation table the
   change was committed against, so regressions in later work show up.

All measurements below are from `cargo run --release --example
bench_hillclimb` on a Windows 11 machine, median of 60 iters after 5
warmups, 48×48 inputs with `n_shapes=12` (default codec). The benchmark
harness is reproducible — see "Reproducing the data" at the bottom.

---

## Opt 1 — Row-wise integral evaluator

**Status:** always on. The original `O(bbox_area)` scanline path is no
longer reachable from `fit_*`; CIRCLE / TRIANGLE encoding goes through
`shape::integral` unconditionally. There is no flag to disable this.
**Bit-exact:** yes, on every test input we've tried.
**Module:** `shape::integral`.

### What it does

CIRCLE and TRIANGLE evaluation used to be `O(bbox_area)` per call:
`raster::eval_circle` / `eval_triangle` scan every pixel inside the shape
bbox, push it into a `ShapeSums` accumulator (15 multiplies per pixel
across the five `S_t`, `S_c`, `S_t²`, `S_c²`, `S_tc` series), then close
the form against α to get ΔSSE. At 48×48 with default search, that's
**1.7–2.2 million pixel pushes per encode** — `Accum::push` is the
encoder's hot kernel.

The integral evaluator replaces "scan every pixel" with "look up two
prefix-sum slots per row". Key observation: on every row the shape's
coverage is a single contiguous span `[xL, xR]`. So if we keep row-wise
prefix sums for the five series, each row contributes
`prefix[y][xR+1] − prefix[y][xL]` — 5 channels × 3 components = **15
f64 subtracts per row** regardless of span width. Evaluation goes from
`O(bbox_area)` to `O(bbox_height)`.

* **Circle row spans** come from `dx = isqrt(r² − dy²)`: pure integer
  math, exact.
* **Triangle row spans** come from intersecting three half-planes
  (`a·x + b ≥ 0` per edge) with the row's x-range: also integer math,
  also exact.

### Why "integral"?

The name is a slight misnomer carried over from the standard
"integral image" of Viola-Jones / SQIP. A true integral image is the
**2D** cumulative sum over both axes. Here we only need the **per-row**
prefix sum because shape coverage is row-contiguous, so the y-axis
sum collapses into an outer loop. The shorthand stuck.

### Data layout

Five `Vec<f64>` tables, each `h × (w+1) × 3`, totaling ~280 KB for a
48×48 canvas with default codec. Target series (`t`, `t²`) are built
once per `fit_*`. Canvas series (`c`, `c²`, `t·c`) are rebuilt for the
affected row range (`[ymin, ymax]`) after every `apply_*` commit —
roughly 4 rows × 432 ops × 12 commits = ~20k ops of bookkeeping per
encode, negligible against the per-eval savings.

### Numerical equivalence

The prefix-sum approach accumulates the entire row `[0, w)` left-to-right
then subtracts the unused prefix, whereas the original scanline loop
only sums inside the span. They are **algebraically** identical but not
bit-equal in f64 due to reassociation. Relative reassociation error is
`~n·ε_f64 ≈ 48 × 2⁻⁵² ≈ 10⁻¹⁴`, well below the f32 ΔSSE resolution of
`2⁻²³ ≈ 10⁻⁷`. So in practice every f32 ΔSSE matches what the original
baseline scanline would have returned.

The risk surface is **near-tie comparisons** in the hill-climb: if two
candidates differ by less than one f32 ulp, last-bit reassociation
could flip the decision and produce a divergent search trajectory. We
did not observe this in any tested workload during the soak period —
once enough confidence accrued we removed the toggle and deleted the
equivalence tests along with it. The `Integral`-based primitives in
`shape::integral` are now the only evaluator that ships.

### Performance (2026-05-16)

`baseline (µs)` is the historical scanline path; `current (µs)` is what
ships today. Both columns are 60-iter medians on the same Windows 11
machine, 48×48 inputs with `n_shapes=12`. The baseline column is frozen
— the scanline path is no longer reachable from `fit_*` since the
flag was removed; the numbers are kept as the before/after record. The
current column is reproducible (see "Reproducing the data" below).

| image / shape         | baseline (µs) | current (µs) |       Δ |
| --------------------- | ------------: | -----------: | ------: |
| gradient / circle     |          4969 |         2423 | −51.2 % |
| gradient / triangle   |          9679 |         3176 | −67.2 % |
| quadrants / circle    |          3439 |         1980 | −42.4 % |
| quadrants / triangle  |          8259 |         3008 | −63.6 % |
| noise / circle        |          1399 |         1432 |  +2.4 % |
| noise / triangle      |           857 |          995 | +16.1 % |

Eval counts and `pixels_touched` were byte-identical between the two
paths when they coexisted — the integral evaluator only changed the
per-eval cost, not the search trajectory. Hash bytes were also
byte-identical across all six (image, shape) combinations during the
soak period.

### Where the regression comes from (noise / triangle)

The noise input forces the hill-climb to give up early (no triangle
candidate improves the random canvas much, so `max_age` fires fast).
The triangles that get committed average **8.4 pixels** — at that size,
the integral path's per-row overhead (3 half-plane constraints + 15
f64 subtracts + bbox-row bookkeeping) is more work than the baseline's
"push 8 pixels into Accum, done". The bookkeeping (build + per-commit
row rebuild) is also a fixed overhead amortized over fewer pixels.

This is an adversarial synthetic input; real photographs sit in the
gradient ↔ quadrants regime. We don't add a size threshold to fall
back to the baseline path because the absolute regression (+129 µs) is
two orders of magnitude smaller than the absolute gain on realistic
inputs (~6500 µs), and the branching cost would tax every other case.

### Public API added

* `pfhash::shape::raster::ShapeSums` + `ShapeSums::finalize(α, palette)`
  — the closed-form α-dependent ΔSSE math, separated from the pixel
  scan. Public so external callers can build their own collectors.
* `pfhash::shape::raster::collect_circle_sums` and
  `collect_triangle_sums` — the original `O(bbox_area)` scanline kept
  as a public primitive for callers that don't want to build an
  `Integral` (e.g., one-shot evaluation).
* `pfhash::shape::integral::Integral::{build, update_canvas_rows}` —
  per-row prefix-sum tables.
* `pfhash::shape::integral::eval_circle_integral` /
  `eval_triangle_integral` and their `collect_*_integral` siblings —
  the `O(bbox_height)` evaluators that `fit_*` now uses unconditionally.

No JS-facing knob: the WASM glue / `@pfhash/ts` / playground inherit the
fast path automatically with no API surface.

---

## α-sweep refactor (always on)

**Status:** unconditional. No flag.
**Bit-exact:** yes — same f64 sums, same `ShapeSums::finalize` math.
**Files:** `circle::fit_primitive`, `triangle::pick_best_alpha`.

### What it does

After the hill-climb settles on a geometry, `fit_*` sweeps every
quantized α level to pick the one with the lowest ΔSSE. Originally
this was a K-call loop calling the full `eval_*` once per α:

```rust
for &a in &alpha_levels {
    let res = eval_circle(target, &canvas, ..., cx, cy, r, a, pal_ref);
    ...
}
```

But geometry is fixed inside this loop — the per-pixel sums don't
depend on α. We now collect the sums once and call the closed-form
finalize K times:

```rust
let sums = collect_circle_dispatch(target, &canvas, integral, h, w, cx, cy, r);
for &a in &alpha_levels {
    let res = sums.finalize(a, pal_ref);
    ...
}
```

### Why no flag?

We measured this in isolation first (commit history has the data: it
was previously gated behind `use_alpha_reuse`). Standalone effect is
±2 % on every workload — basically run-to-run noise:

| image / shape         | baseline (µs) | α-reuse alone (µs) |     Δ |
| --------------------- | ------------: | -----------------: | ----: |
| gradient / circle     |          5046 |               4927 | −2.4% |
| gradient / triangle   |          9759 |               9631 | −1.3% |
| quadrants / circle    |          3420 |               3427 | +0.2% |
| quadrants / triangle  |          8284 |               8228 | −0.7% |
| noise / circle        |          1366 |               1369 | +0.2% |
| noise / triangle      |           859 |                880 | +2.5% |

The ceiling is `n_shapes × K × pixels_in_final_geometry ≈ 24k / 1.7M ≈
1.4 %` of total pixel work, and we hit roughly the ceiling. A runtime
flag for a ≤2 % effect is just noise-grade configuration, so we deleted
the flag and let the refactored α-sweep be the only code path. The
public `collect_*_sums` / `ShapeSums::finalize` decomposition is
useful in its own right as primitives, independent of the in-tree call
sites.

---

## Reproducing the data

```sh
# From packages/pfhash-rs

# (1) Accurate timing — counters off so they don't perturb measurement.
cargo run --release --example bench_hillclimb -- \
    --label=run --out=../../bench/run-timed.ndjson

# (2) Eval counts / pixels touched — counters on (slightly perturbs timing,
# but timings from this run are not used).
cargo run --release --features bench-counters \
    --example bench_hillclimb -- \
    --label=run --out=../../bench/run-counts.ndjson

# (3) Cross-reference the two and print the comparison table.
python ../../bench/summarize_hillclimb.py \
    ../../bench/run-timed.ndjson \
    ../../bench/run-counts.ndjson
```

The bench runs once per (image × shape), exercising three image
classes (gradient / quadrants / xorshift noise) × two shapes
(circle / triangle). NDJSON outputs are gitignored — re-run the bench
to regenerate; the "current" column in the table above is what to
compare against.

---

## What's next

Ranked by expected impact, not yet implemented:

1. **Residual-driven random init** — sample initial centers proportional
   to the current residual image instead of uniform. Cuts `n_random` ~3×
   at equal quality. Search trajectory changes — would break test
   vectors. Estimated: 30–50 % additional speedup on realistic inputs.
2. **Coordinate line search** in hill-climb to replace single-axis
   Gaussian random walk. Estimated: ~30 % fewer steps to converge.
3. **SIMD on `Accum::push`** — packed f32x4 for the per-pixel multiplies.
   Only meaningful when opt 1 is off (otherwise the inner loop isn't
   the hot kernel). Estimated: 2–4 % on noise / small-shape workloads.
4. **Parallel random pool** — `n_random` is embarrassingly parallel.
   Native: trivial via Rayon. WASM: needs `wasm-bindgen-rayon` +
   COOP/COEP, deployment-heavy.

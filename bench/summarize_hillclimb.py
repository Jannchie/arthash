"""Join NDJSON benchmark runs and print an ablation table.

Usage:
    python bench/summarize_hillclimb.py \
        bench/hillclimb-baseline-timed.ndjson \
        bench/hillclimb-baseline-counts.ndjson \
        [bench/hillclimb-opt1-timed.ndjson ...]

Inputs are pairs: timed runs (counters=false) carry accurate `median_us`;
counts runs (counters=true) carry `eval_total` / `pixels_touched`. We join
on (label-prefix, image, shape) where label-prefix strips the `-timed` /
`-counts` suffix.
"""
import json
import sys
from collections import defaultdict


def load(path):
    rows = []
    with open(path, "r", encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            rows.append(json.loads(line))
    return rows


def base_label(lbl: str) -> str:
    for suffix in ("-timed", "-counts"):
        if lbl.endswith(suffix):
            return lbl[: -len(suffix)]
    return lbl


def main():
    if len(sys.argv) < 2:
        print(__doc__, file=sys.stderr)
        sys.exit(2)

    # Group by (label_base, image, shape).
    timed = {}
    counts = {}
    hashes = {}  # for diff verification
    for path in sys.argv[1:]:
        for r in load(path):
            key = (base_label(r["label"]), r["image"], r["shape"])
            if r.get("counters"):
                counts[key] = r
            else:
                timed[key] = r
            hashes.setdefault(key, set()).add(r["hash_hex"])

    # Collect labels seen, preserving order.
    seen_labels = []
    seen_set = set()
    for key in list(timed.keys()) + list(counts.keys()):
        lbl = key[0]
        if lbl not in seen_set:
            seen_set.add(lbl)
            seen_labels.append(lbl)

    # Find baseline (first label seen).
    baseline = seen_labels[0] if seen_labels else None

    # Print per-(image, shape) comparison, one section per pair.
    images = sorted({k[1] for k in timed.keys() | counts.keys()})
    shapes = sorted({k[2] for k in timed.keys() | counts.keys()})

    def fmt_pct(x, base):
        if base == 0 or base is None:
            return "    -"
        return f"{(x / base - 1.0) * 100:+6.1f}%"

    for img in images:
        for sh in shapes:
            rows_for = [(lbl, timed.get((lbl, img, sh)), counts.get((lbl, img, sh)))
                        for lbl in seen_labels]
            if not any(t is not None or c is not None for _, t, c in rows_for):
                continue
            print(f"\n=== {img} / {sh} ===")
            header = f"{'label':<22} {'median_us':>10} {'min_us':>10} {'mpix/s':>9} {'evals':>8} {'pixels':>12} {'px/eval':>8} {'Δvs base':>10} {'hash_match':>10}"
            print(header)
            print("-" * len(header))
            base_t = rows_for[0][1]
            for lbl, t, c in rows_for:
                median = t["median_us"] if t else None
                min_us = t["min_us"] if t else None
                mp = t["mpix_per_s"] if t else None
                ev = c["eval_total"] if c else None
                px = c["pixels_touched"] if c else None
                ape = c["avg_pixels_per_eval"] if c else None
                delta = fmt_pct(median, base_t["median_us"]) if (median and base_t) else "    -"
                hset = hashes.get((lbl, img, sh), set())
                hmatch = "yes" if hset and hset == hashes.get((baseline, img, sh), set()) else ("?" if not hset else "DIFF")
                print(
                    f"{lbl:<22} {median if median else '-':>10} {min_us if min_us else '-':>10} "
                    f"{mp if mp else '-':>9} {ev if ev is not None else '-':>8} "
                    f"{px if px is not None else '-':>12} {ape if ape is not None else '-':>8} "
                    f"{delta:>10} {hmatch:>10}"
                )


if __name__ == "__main__":
    main()

// Benchmark thumbhash (official JS) — same workload as Rust/Python/Go.
// Output: NDJSON to stdout.

import { rgbaToThumbHash, thumbHashToRGBA } from "thumbhash";

function gradient(w, h) {
  const rgba = new Uint8Array(w * h * 4);
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      const p = (y * w + x) * 4;
      rgba[p] = Math.round((x * 255) / Math.max(w - 1, 1));
      rgba[p + 1] = Math.round((y * 255) / Math.max(h - 1, 1));
      rgba[p + 2] = Math.min(255, Math.floor((x + y) * 0.3));
      rgba[p + 3] = 255;
    }
  }
  return rgba;
}

function measure(fn, warmup, iters) {
  for (let i = 0; i < warmup; i++) fn();
  // Node hrtime() resolution on Windows is good (~100ns), but for very fast
  // ops we still batch for stability.
  const batch = 50;
  const samples = [];
  for (let i = 0; i < iters; i++) {
    const t0 = process.hrtime.bigint();
    for (let j = 0; j < batch; j++) fn();
    const dt_ns = Number(process.hrtime.bigint() - t0);
    samples.push(dt_ns / batch / 1000.0);
  }
  samples.sort((a, b) => a - b);
  return {
    median_us: samples[Math.floor(samples.length / 2)],
    p95_us: samples[Math.floor(samples.length * 0.95)],
    min_us: samples[0],
    iters,
  };
}

function report(mode, op, w, h, s, extra = {}) {
  const out = {
    impl: "js",
    mode,
    op,
    w,
    h,
    median_us: Math.round(s.median_us * 100) / 100,
    p95_us: Math.round(s.p95_us * 100) / 100,
    min_us: Math.round(s.min_us * 100) / 100,
    iters: s.iters,
    mpix_per_s: Math.round(((w * h) / s.median_us) * 1000) / 1000,
    ...extra,
  };
  console.log(JSON.stringify(out));
}

function main() {
  const w = 100, h = 100;
  const rgba = gradient(w, h);
  let hash;
  let s = measure(() => { hash = rgbaToThumbHash(w, h, rgba); }, 30, 200);
  report("dct", "encode", w, h, s, { hash_bytes: hash.length });

  s = measure(() => { thumbHashToRGBA(hash); }, 10, 50);
  report("dct", "decode_default", w, h, s);
}

main();

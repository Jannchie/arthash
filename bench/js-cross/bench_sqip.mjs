// sqip (Node + Go primitive binary), serviced mode (long-running Node
// process, Go binary spawned per image). Wraps the existing
// packages/arthash-research/sqip-bench/bench.js which uses sqip@0.3 (works
// on Windows; the modern sqip 1.x has DLL conflicts via two sharp versions).
//
// This is "warm sqip" — Node + sqip module loads are paid once, per-call
// cost is just Go primitive subprocess + image I/O. Equivalent to what a
// properly designed sqip HTTP service would see per request.
//
// Output: NDJSON to stdout.

import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
const SQIP_BENCH_DIR = resolve(here, "../../packages/arthash-research/sqip-bench");
const IMG = resolve(here, "../../docs/benchmarks/visual_commons_2013_Rainbow_over_Washfold.png");
const N_SAMPLES = 20;
const WARMUP = 2;

// mode=1 in primitive = triangle (matches arthash TRIANGLE).
function runSqip(nShapes, mode, samples) {
  return new Promise((resolveP, rejectP) => {
    const paths = Array(samples + WARMUP).fill(IMG);
    const args = ["bench.js", String(mode), String(nShapes), "0", String(WARMUP), ...paths];
    const proc = spawn("node", args, { cwd: SQIP_BENCH_DIR });
    let stdout = "", stderr = "";
    proc.stdout.on("data", d => stdout += d.toString());
    proc.stderr.on("data", d => stderr += d.toString());
    proc.on("close", code => {
      if (code !== 0) {
        rejectP(new Error(`sqip bench exited ${code}: ${stderr}`));
        return;
      }
      const lines = stdout.trim().split(/\r?\n/);
      const out = lines.map(l => {
        const [ms, bytes] = l.split("\t");
        return { ms: parseFloat(ms), bytes: parseInt(bytes, 10) };
      });
      resolveP(out);
    });
  });
}

function stats(samples) {
  const ms = samples.map(s => s.ms).sort((a, b) => a - b);
  return {
    median_ms: ms[Math.floor(ms.length / 2)],
    p95_ms: ms[Math.floor(ms.length * 0.95)],
    min_ms: ms[0],
    iters: ms.length,
    svg_bytes: samples[0].bytes,
  };
}

function report(rec) {
  const r = {
    ...rec,
    median_ms: Math.round(rec.median_ms * 1000) / 1000,
    p95_ms: Math.round(rec.p95_ms * 1000) / 1000,
    min_ms: Math.round(rec.min_ms * 1000) / 1000,
  };
  console.log(JSON.stringify(r));
}

// n=64 takes ~1-2s per call so we shrink the sample count to keep the
// total runtime manageable; primitive's per-shape cost is fairly stable.
for (const n of [12, 24, 64]) {
  const samples = n >= 64
    ? await runSqip(n, 1, 8)
    : await runSqip(n, 1, N_SAMPLES);
  const s = stats(samples);
  report({ impl: "sqip-node", mode: "primitive-triangle", n_shapes: n, op: "encode", ...s });
}

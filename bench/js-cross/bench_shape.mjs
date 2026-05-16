// arthash CIRCLE / TRIANGLE (wasm) on a 100x100 real image.
// Pre-resizes the test image via sharp 0.34 (avoids sharp DLL conflicts with
// sqip — sqip is benched in bench_sqip.mjs as a separate process).
//
// Output: NDJSON to stdout.

import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";
import sharp from "sharp";
import { init, encode as ahEncode, Shape } from "arthash";

const here = dirname(fileURLToPath(import.meta.url));
const wasmPath = resolve(here, "node_modules/arthash/wasm/pkg/arthash_wasm_bg.wasm");
await init(await readFile(wasmPath));

const IMG = resolve(here, "../../docs/benchmarks/visual_commons_2013_Rainbow_over_Washfold.png");
const W = 100, H = 100;
const { data: rgba } = await sharp(IMG)
  .resize(W, H, { fit: "fill" })
  .ensureAlpha()
  .raw()
  .toBuffer({ resolveWithObject: true });
const rgb = new Uint8Array(W * H * 3);
for (let i = 0; i < W * H; i++) {
  rgb[i * 3] = rgba[i * 4];
  rgb[i * 3 + 1] = rgba[i * 4 + 1];
  rgb[i * 3 + 2] = rgba[i * 4 + 2];
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

function measure(fn, warmup, iters, batch) {
  for (let i = 0; i < warmup; i++) fn();
  const samples = [];
  for (let i = 0; i < iters; i++) {
    const t0 = process.hrtime.bigint();
    for (let j = 0; j < batch; j++) fn();
    const dt_ms = Number(process.hrtime.bigint() - t0) / 1e6;
    samples.push(dt_ms / batch);
  }
  samples.sort((a, b) => a - b);
  return {
    median_ms: samples[Math.floor(samples.length / 2)],
    p95_ms: samples[Math.floor(samples.length * 0.95)],
    min_ms: samples[0],
    iters,
  };
}

for (const n of [12, 24, 64]) {
  let hash;
  let s = measure(
    () => { hash = ahEncode(rgb, W, H, { shape: Shape.CIRCLE, nShapes: n }); },
    3, 30, 5,
  );
  report({ impl: "arthash-ts", mode: "circle", n_shapes: n, op: "encode", w: W, h: H, ...s, hash_bytes: hash.length });

  s = measure(
    () => { hash = ahEncode(rgb, W, H, { shape: Shape.TRIANGLE, nShapes: n }); },
    3, 30, 5,
  );
  report({ impl: "arthash-ts", mode: "triangle", n_shapes: n, op: "encode", w: W, h: H, ...s, hash_bytes: hash.length });
}

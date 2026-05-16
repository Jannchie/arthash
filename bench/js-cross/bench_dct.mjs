// DCT vs thumbhash, both in pure JS (arthash via wasm, thumbhash via npm).
// Workload: 100x100 RGBA gradient — matches docs/benchmarks/thumbhash_js.ndjson
// so numbers line up with the existing cross-impl table.
//
// Output: NDJSON to stdout, one record per (impl, op).

import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";
import { init, encode as ahEncode, decode as ahDecode, Shape } from "arthash";
import { rgbaToThumbHash, thumbHashToRGBA } from "thumbhash";

const here = dirname(fileURLToPath(import.meta.url));
const wasmPath = resolve(here, "node_modules/arthash/wasm/pkg/arthash_wasm_bg.wasm");
await init(await readFile(wasmPath));

const W = 100, H = 100;

function gradientRGBA(w, h) {
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

function rgbaToRgb(rgba) {
  const n = rgba.length / 4;
  const rgb = new Uint8Array(n * 3);
  for (let i = 0; i < n; i++) {
    rgb[i * 3] = rgba[i * 4];
    rgb[i * 3 + 1] = rgba[i * 4 + 1];
    rgb[i * 3 + 2] = rgba[i * 4 + 2];
  }
  return rgb;
}

function measure(fn, warmup, iters, batch = 50) {
  for (let i = 0; i < warmup; i++) fn();
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

function report(rec) {
  const r = {
    ...rec,
    median_us: Math.round(rec.median_us * 100) / 100,
    p95_us: Math.round(rec.p95_us * 100) / 100,
    min_us: Math.round(rec.min_us * 100) / 100,
  };
  console.log(JSON.stringify(r));
}

const rgba = gradientRGBA(W, H);
const rgb = rgbaToRgb(rgba);

// ---- arthash DCT ----------------------------------------------------------
let ahHash;
let s = measure(() => { ahHash = ahEncode(rgb, W, H, { shape: Shape.DCT }); }, 30, 200);
report({ impl: "arthash-ts", mode: "dct", op: "encode", w: W, h: H, ...s, hash_bytes: ahHash.length });

s = measure(() => { ahDecode(ahHash, { shape: Shape.DCT, baseSize: 32 }); }, 10, 100, 20);
report({ impl: "arthash-ts", mode: "dct", op: "decode_32", w: W, h: H, ...s, base_size: 32 });

s = measure(() => { ahDecode(ahHash, { shape: Shape.DCT, baseSize: 256 }); }, 10, 50, 10);
report({ impl: "arthash-ts", mode: "dct", op: "decode_256", w: W, h: H, ...s, base_size: 256 });

s = measure(() => { ahDecode(ahHash, { shape: Shape.DCT, baseSize: 512 }); }, 5, 30, 5);
report({ impl: "arthash-ts", mode: "dct", op: "decode_512", w: W, h: H, ...s, base_size: 512 });

// ---- thumbhash JS ---------------------------------------------------------
let thHash;
s = measure(() => { thHash = rgbaToThumbHash(W, H, rgba); }, 30, 200);
report({ impl: "thumbhash-js", mode: "dct", op: "encode", w: W, h: H, ...s, hash_bytes: thHash.length });

s = measure(() => { thumbHashToRGBA(thHash); }, 10, 100, 20);
report({ impl: "thumbhash-js", mode: "dct", op: "decode_default", w: W, h: H, ...s, base_size: "~32 (default)" });

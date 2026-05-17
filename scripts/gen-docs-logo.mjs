// Generate docs/site/public/logo.svg by running a rasterized triangle through
// arthash's RECT mode. The output rectangles ARE the logo — a meta-mark that
// shows the codec decomposing its own iconic shape.

import { readFile, writeFile } from "node:fs/promises";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

import { encode, toSvg, codec, init } from "../packages/arthash-ts/dist/index.js";

const here = dirname(fileURLToPath(import.meta.url));
const wasmBytes = await readFile(
  resolve(here, "..", "packages", "arthash-ts", "wasm", "pkg", "arthash_wasm_bg.wasm"),
);
await init(wasmBytes);

const W = 48, H = 48;                                 // encoder thumb-target size
const rgb = new Uint8Array(W * H * 3).fill(255);      // white background

// Equilateral-ish triangle, pointing up, near the visual centre.
const A = [W * 0.50, H * 0.16];
const B = [W * 0.10, H * 0.86];
const C = [W * 0.90, H * 0.86];

const edge = (a, b, p) =>
  (p[0] - a[0]) * (b[1] - a[1]) - (p[1] - a[1]) * (b[0] - a[0]);

const FILL = [0x02, 0x84, 0xc7];                      // sky-600, our brand color

for (let y = 0; y < H; y++) {
  for (let x = 0; x < W; x++) {
    const p = [x + 0.5, y + 0.5];
    const w0 = edge(B, C, p);
    const w1 = edge(C, A, p);
    const w2 = edge(A, B, p);
    if ((w0 >= 0 && w1 >= 0 && w2 >= 0) || (w0 <= 0 && w1 <= 0 && w2 <= 0)) {
      const i = (y * W + x) * 3;
      rgb[i] = FILL[0]; rgb[i + 1] = FILL[1]; rgb[i + 2] = FILL[2];
    }
  }
}

const c = codec.rect({ n: 8 });
const hash = await encode(rgb, W, H, c, { seed: 7 });
const svg = await toSvg(hash, c, { baseSize: 64, blur: 0 });

const dest = resolve(here, "..", "docs", "site", "public", "logo.svg");
await writeFile(dest, svg + "\n", "utf8");

console.log(`hash: ${hash.length} bytes  →  ${dest}`);

import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";
import { init, encode, decode, Shape } from "arthash";

const here = dirname(fileURLToPath(import.meta.url));
const wasmPath = resolve(here, "node_modules/arthash/wasm/pkg/arthash_wasm_bg.wasm");
const wasmBytes = await readFile(wasmPath);
await init(wasmBytes);

const w = 100, h = 100;
const rgb = new Uint8Array(w * h * 3);
for (let i = 0; i < rgb.length; i += 3) {
  rgb[i] = (i / 3) % 256;
  rgb[i + 1] = 128;
  rgb[i + 2] = 64;
}
const hash = encode(rgb, w, h, { shape: Shape.DCT });
console.log("DCT hash bytes:", hash.length);
const dec = decode(hash, { shape: Shape.DCT, baseSize: 256 });
console.log("decoded:", dec.w, "x", dec.h, "rgba bytes:", dec.rgba.length);

const c = encode(rgb, w, h, { shape: Shape.CIRCLE, nShapes: 12 });
console.log("CIRCLE 12 hash bytes:", c.length);
const t = encode(rgb, w, h, { shape: Shape.TRIANGLE, nShapes: 12 });
console.log("TRIANGLE 12 hash bytes:", t.length);

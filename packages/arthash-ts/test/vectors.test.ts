// Cross-language conformance + wasm round-trip tests.
//
// Unlike `render-style.test.ts` (type-level, no wasm), this suite boots the
// real wasm module in Node and exercises the encode/decode hot paths:
//
//   * DCT byte-exact conformance — replays the DCT vectors in
//     `docs/test-vectors/vectors.json` (the SAME file the Rust + Python SDKs
//     assert against) and checks the TS SDK produces identical bytes. This is
//     the cross-language guarantee: one hash, three implementations.
//   * palette encode + decode round-trip — covers the wasm copy paths the
//     other suites never run: the `Uint8Array` palette fast path in
//     `get_u8_array` and the move-not-clone `DecodeResult.intoRgba`.
//
// Only `solid` / `gradient` DCT inputs are byte-asserted; `random` needs
// numpy's PCG64 sequence (not reproduced here), exactly as the Rust harness
// skips it.

import { readFileSync } from "node:fs";
import { beforeAll, describe, expect, it } from "vitest";
import { codec, decodeSync, encodeSync, init, palette } from "../src/index.js";

interface InputSpec {
  kind: string;
  w: number;
  h: number;
  rgb?: [number, number, number];
}
interface Vector {
  name: string;
  codec: { shape: string };
  input: InputSpec;
  expected_hex: string;
}

const vectorsUrl = new URL("../../../docs/test-vectors/vectors.json", import.meta.url);
const allVectors: Vector[] = JSON.parse(readFileSync(vectorsUrl, "utf-8")).vectors;

/** Reproduce a vector's RGB input. The gradient path mirrors the Rust harness'
 *  f32 arithmetic via `Math.fround` so the synthesized pixels — and therefore
 *  the encoded bytes — match exactly. */
function buildInput(input: InputSpec): Uint8Array {
  const { w, h } = input;
  const rgb = new Uint8Array(w * h * 3);
  if (input.kind === "solid") {
    const [r, g, b] = input.rgb!;
    for (let i = 0; i < w * h; i++) {
      rgb[i * 3] = r;
      rgb[i * 3 + 1] = g;
      rgb[i * 3 + 2] = b;
    }
  } else if (input.kind === "gradient") {
    for (let y = 0; y < h; y++) {
      for (let x = 0; x < w; x++) {
        const p = (y * w + x) * 3;
        rgb[p] = Math.round(Math.fround(Math.fround(Math.fround(x) * 255) / Math.fround(w - 1)));
        rgb[p + 1] = Math.round(Math.fround(Math.fround(Math.fround(y) * 255) / Math.fround(h - 1)));
        rgb[p + 2] = 64;
      }
    }
  } else {
    throw new Error(`buildInput: unsupported kind ${input.kind}`);
  }
  return rgb;
}

function toHex(bytes: Uint8Array): string {
  let s = "";
  for (const b of bytes) s += b.toString(16).padStart(2, "0");
  return s;
}

beforeAll(async () => {
  // `--target web` pkg: feed the wasm bytes directly so no fetch / browser
  // globals are needed under Node.
  const wasmBytes = readFileSync(new URL("../wasm/pkg/arthash_wasm_bg.wasm", import.meta.url));
  await init(wasmBytes);
});

describe("DCT cross-language conformance (vectors.json)", () => {
  const dctVectors = allVectors.filter(
    (v) => v.codec.shape === "dct" && v.input.kind !== "random",
  );

  it("has DCT vectors to exercise", () => {
    expect(dctVectors.length).toBeGreaterThan(0);
  });

  for (const v of dctVectors) {
    it(`byte-exact: ${v.name}`, () => {
      const rgb = buildInput(v.input);
      const hash = encodeSync(rgb, v.input.w, v.input.h, codec.dct(), {});
      expect(toHex(hash)).toBe(v.expected_hex);
    });
  }
});

describe("palette encode + decode round-trip (wasm copy paths)", () => {
  it("passes a Uint8Array palette through and decodes an RGBA buffer", () => {
    const pal = palette.fromRgb([
      [0, 0, 0],
      [255, 255, 255],
      [220, 60, 60],
      [60, 120, 220],
    ]); // K=4 → 2-bit indices
    const c = codec.withPalette(codec.circle({ n: 12 }), pal);

    // Non-uniform input so the encoder actually places shapes.
    const w = 32;
    const h = 32;
    const rgb = new Uint8Array(w * h * 3);
    for (let y = 0; y < h; y++) {
      for (let x = 0; x < w; x++) {
        const p = (y * w + x) * 3;
        rgb[p] = (x * 8) & 0xff;
        rgb[p + 1] = (y * 8) & 0xff;
        rgb[p + 2] = 128;
      }
    }

    const hash = encodeSync(rgb, w, h, c, {});
    expect(hash.length).toBeGreaterThan(0);

    const out = decodeSync(hash, c, { baseSize: 48 });
    expect(out.w).toBeGreaterThan(0);
    expect(out.h).toBeGreaterThan(0);
    expect(out.rgba).toBeInstanceOf(Uint8Array);
    expect(out.rgba.length).toBe(out.w * out.h * 4);
  });

  it("is deterministic for the same input + seed", () => {
    const w = 24;
    const h = 24;
    const rgb = new Uint8Array(w * h * 3).map((_, i) => (i * 37) & 0xff);
    const c = codec.triangle({ n: 12 });
    const a = encodeSync(rgb, w, h, c, { seed: 7 });
    const b = encodeSync(rgb, w, h, c, { seed: 7 });
    expect(toHex(a)).toBe(toHex(b));
  });
});

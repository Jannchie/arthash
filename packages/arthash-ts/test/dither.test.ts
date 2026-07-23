// `decode(..., { dither: true })` — ordered Bayer 8×8 dithering at the
// 8-bit quantization step. Boots real wasm (same pattern as vectors.test.ts).
//
// Invariants mirrored from the Rust `tests/dither.rs` locks:
//   * default off ⇒ byte-identical output;
//   * on ⇒ each channel moves by at most 1 LSB, deterministically;
//   * sharp shape output (no blur) is left untouched.

import { readFileSync } from "node:fs";
import { beforeAll, describe, expect, it } from "vitest";
import { codec, decodeSync, encodeSync, init } from "../src/index.js";

function gradientRgb(w: number, h: number): Uint8Array {
  const rgb = new Uint8Array(w * h * 3);
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      const p = (y * w + x) * 3;
      rgb[p] = Math.round((x * 255) / (w - 1));
      rgb[p + 1] = Math.round((y * 255) / (h - 1));
      rgb[p + 2] = 64;
    }
  }
  return rgb;
}

beforeAll(async () => {
  const wasmBytes = readFileSync(new URL("../wasm/pkg/arthash_wasm_bg.wasm", import.meta.url));
  await init(wasmBytes);
});

describe("decode dither option", () => {
  const c = codec.dct();
  // Shared across tests — encode is the expensive step and the hash is
  // deterministic. Computed in beforeAll so it runs after wasm init.
  let h: Uint8Array;
  beforeAll(() => {
    h = encodeSync(gradientRgb(96, 64), 96, 64, c);
  });

  it("default off is byte-identical to explicit false", () => {
    const plain = decodeSync(h, c);
    const off = decodeSync(h, c, { dither: false });
    expect(off.rgba).toEqual(plain.rgba);
  });

  it("moves channels by at most 1 LSB and is deterministic", () => {
    const plain = decodeSync(h, c);
    const dithered = decodeSync(h, c, { dither: true });
    expect(dithered.rgba.length).toBe(plain.rgba.length);
    let changed = 0;
    for (let i = 0; i < plain.rgba.length; i++) {
      const d = Math.abs(plain.rgba[i] - dithered.rgba[i]);
      expect(d).toBeLessThanOrEqual(1);
      if (d !== 0) changed++;
    }
    expect(changed).toBeGreaterThan(0);
    const again = decodeSync(h, c, { dither: true });
    expect(again.rgba).toEqual(dithered.rgba);
  });

  it("leaves sharp shape output untouched", () => {
    const tri = codec.triangle({ n: 12 });
    const triHash = encodeSync(gradientRgb(48, 48), 48, 48, tri, { seed: 1 });
    const plain = decodeSync(triHash, tri);
    const dithered = decodeSync(triHash, tri, { dither: true });
    expect(dithered.rgba).toEqual(plain.rgba);
  });

  it("quantizes DCT to a render-time palette, dithered when enabled", () => {
    // Palette on a DCT codec (raw spec — same path the playground uses) is a
    // render-time display request; the hash bytes never contain it.
    const colors: number[] = [];
    for (let i = 0; i < 8; i++) colors.push(i * 32, 255 - i * 32, i * 17);
    const palCodec = codec.raw({
      shape: "dct",
      palette: new Uint8Array(colors),
      paletteK: 8,
    });
    const hard = decodeSync(h, palCodec);
    const dithered = decodeSync(h, palCodec, { dither: true });
    const palSet = new Set<string>();
    for (let i = 0; i < 8; i++) palSet.add(`${i * 32},${255 - i * 32},${i * 17}`);
    for (const out of [hard, dithered]) {
      for (let i = 0; i < out.rgba.length; i += 4) {
        const key = `${out.rgba[i]},${out.rgba[i + 1]},${out.rgba[i + 2]}`;
        expect(palSet.has(key), `pixel ${key} not in palette`).toBe(true);
      }
    }
    expect(hard.rgba).not.toEqual(dithered.rgba);
  });
});

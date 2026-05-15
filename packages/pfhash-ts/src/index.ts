/**
 * @pfhash/ts — TypeScript SDK for pfhash.
 *
 * Backed by the `pfhash-rs` core compiled to WebAssembly via wasm-bindgen.
 * Works in browser and Node (>= 18, native `Uint8Array` + WebAssembly).
 *
 * Quick start:
 * ```ts
 * import { encode, decode, toSvg, Shape } from "@pfhash/ts";
 *
 * const hash = encode(rgbBytes, width, height, { shape: Shape.CIRCLE, nShapes: 12 });
 * const { w, h, rgba } = decode(hash, { shape: Shape.CIRCLE, nShapes: 12 });
 * const svg = toSvg(hash, { shape: Shape.CIRCLE, nShapes: 12, blur: 12 });
 * ```
 *
 * `init()` must be called once before any other function (it loads the
 * wasm module). In bundler environments with top-level await, you can do
 * `await init()` at module scope.
 */

import initWasm, {
  encodeRgb as wasmEncodeRgb,
  decode as wasmDecode,
  toSvg as wasmToSvg,
} from "../wasm/pkg/pfhash_wasm.js";

export const Shape = {
  DCT: "dct",
  CIRCLE: "circle",
  TRIANGLE: "triangle",
  PIXEL: "pixel",
} as const;
export type Shape = (typeof Shape)[keyof typeof Shape];

/** Codec config. Hash bytes alone are not self-describing — the decoder
 *  needs the same codec used at encode time. */
export interface CodecOptions {
  shape?: Shape;
  /** Number of shapes (CIRCLE / TRIANGLE / PIXEL). Default 12. */
  nShapes?: number;
  cxBits?: number;
  cyBits?: number;
  rBits?: number;
  alphaBits?: number;
  colorBits?: 16 | 24;
  /** Optional sRGB palette, flat row-major bytes (length = 3·K, K ∈ {2,4,8,16,32,64,128,256,512,1024}).
   *  When set, the codec stores `log2(K)` bits per shape (palette index) instead of
   *  `colorBits` — shrinks the hash but requires the **same palette** at decode time.
   *  The palette is consensus knowledge and is NOT stored in the hash bytes. */
  palette?: Uint8Array;
  /** Effective palette size. Defaults to `palette.length / 3`. Useful when the buffer
   *  is over-allocated. */
  paletteK?: number;
}

export interface EncodeOptions extends CodecOptions {
  /** RNG seed for shape-mode hill-climb. Default 0. */
  seed?: number;
}

export interface DecodeOptions extends CodecOptions {
  /** Long-edge pixel target. Default 256. */
  baseSize?: number;
  /** Override the stored aspect ratio. */
  overrideAspect?: number;
}

export interface SvgRenderOptions extends DecodeOptions {
  /** Gaussian blur stdDeviation in viewBox units. `0` = no blur. */
  blur?: number;
}

export interface DecodeResult {
  w: number;
  h: number;
  /** RGBA pixels, row-major (4 bytes per pixel, length = 4·w·h). */
  rgba: Uint8Array;
}

let wasmReady: Promise<void> | null = null;

/** Initialize the wasm module. Safe to call repeatedly — only loads once.
 *
 *  `input` can be:
 *  * `undefined` — wasm-bindgen auto-locates the `.wasm` file next to the JS glue.
 *  * a `URL` / string — URL to fetch the `.wasm` from.
 *  * a `BufferSource` — pre-loaded `.wasm` bytes (useful in Node).
 *  * `WebAssembly.Module` — pre-compiled module. */
export type InitInput =
  | undefined
  | RequestInfo
  | URL
  | Response
  | BufferSource
  | WebAssembly.Module;

export function init(input?: InitInput): Promise<void> {
  if (!wasmReady) {
    wasmReady = initWasm(
      input === undefined ? undefined : { module_or_path: input },
    ).then(() => undefined);
  }
  return wasmReady;
}

function codecToObj(opts: CodecOptions | undefined): Record<string, unknown> {
  const o = opts ?? {};
  const out: Record<string, unknown> = {};
  if (o.shape !== undefined) out.shape = o.shape;
  if (o.nShapes !== undefined) out.n_shapes = o.nShapes;
  if (o.cxBits !== undefined) out.cx_bits = o.cxBits;
  if (o.cyBits !== undefined) out.cy_bits = o.cyBits;
  if (o.rBits !== undefined) out.r_bits = o.rBits;
  if (o.alphaBits !== undefined) out.alpha_bits = o.alphaBits;
  if (o.colorBits !== undefined) out.color_bits = o.colorBits;
  if (o.palette !== undefined && o.palette.length > 0) {
    // serde-wasm-bindgen accepts a plain array of numbers for Vec<u8>; pass as
    // Array to avoid TypedArray vs raw-bytes interpretation ambiguity.
    out.palette = Array.from(o.palette);
  }
  if (o.paletteK !== undefined) out.palette_k = o.paletteK;
  return out;
}

function assertReady(): void {
  if (!wasmReady) {
    throw new Error(
      "@pfhash/ts: wasm not initialized — call `await init()` before using encode/decode/toSvg.",
    );
  }
}

/** Encode RGB bytes (row-major, 3 bytes per pixel) into a pfhash. */
export function encode(
  rgb: Uint8Array,
  width: number,
  height: number,
  opts?: EncodeOptions,
): Uint8Array {
  assertReady();
  return wasmEncodeRgb(
    rgb,
    width,
    height,
    codecToObj(opts),
    BigInt(opts?.seed ?? 0),
  );
}

/** Decode a pfhash to RGBA pixels. */
export function decode(hash: Uint8Array, opts?: DecodeOptions): DecodeResult {
  assertReady();
  const r = wasmDecode(
    hash,
    codecToObj(opts),
    opts?.baseSize ?? 256,
    opts?.overrideAspect,
  );
  // Capture before `free()` — wasm-bindgen getters allocate fresh on each
  // call, so we read once and dispose.
  const out: DecodeResult = { w: r.w, h: r.h, rgba: r.rgba };
  r.free();
  return out;
}

/** Render a CIRCLE or TRIANGLE hash as a compact SVG string.
 *  Throws for DCT and PIXEL modes (no natural SVG primitive form). */
export function toSvg(hash: Uint8Array, opts?: SvgRenderOptions): string {
  assertReady();
  return wasmToSvg(
    hash,
    codecToObj(opts),
    opts?.baseSize ?? 256,
    opts?.overrideAspect,
    opts?.blur ?? 0,
  );
}

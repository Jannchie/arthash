/**
 * arthash — TypeScript SDK.
 *
 * Backed by `arthash-rs` compiled to WebAssembly via wasm-bindgen.
 * Works in browser and Node (>= 18, native `Uint8Array` + WebAssembly).
 *
 * Quick start:
 * ```ts
 * import { encode, decode, toSvg, codec, Preset, encodeImage } from "arthash";
 *
 * // Plain RGB buffer
 * const hash = await encode(rgbBytes, width, height, codec.triangle({ n: 64 }));
 * const { w, h, rgba } = await decode(hash, codec.triangle({ n: 64 }));
 *
 * // Named preset
 * const hash2 = await encode(rgbBytes, w, h, codec.preset(Preset.DetailTriangle));
 *
 * // Browser convenience: load image, resize, encode in one call
 * const hash3 = await encodeImage(imgUrl, codec.triangle({ n: 64 }));
 * ```
 *
 * The wasm module loads automatically on first call. If you want to control
 * timing (e.g. preload), call `await init()` explicitly.
 */

import initWasm, {
  encodeRgb as wasmEncodeRgb,
  encodeRgba as wasmEncodeRgba,
  decode as wasmDecode,
  toSvg as wasmToSvg,
} from "../wasm/pkg/arthash_wasm.js";

import * as palettes from "./palettes.js";
export { palettes };

// ---------------------------------------------------------------------------
// Codec — discriminated union + factory helpers
// ---------------------------------------------------------------------------

export const Shape = {
  DCT: "dct",
  CIRCLE: "circle",
  TRIANGLE: "triangle",
  PIXEL: "pixel",
  SQUARE: "square",
  RECT: "rect",
  ROTATED_RECT: "rotrect",
} as const;
export type Shape = (typeof Shape)[keyof typeof Shape];

/** sRGB palette — flat row-major bytes (length = 3·K, K ∈ {2,4,8,16,32,64,128,256,512,1024}). */
export interface Palette {
  bytes: Uint8Array;
  k?: number;
}

const VALID_PALETTE_K = new Set([2, 4, 8, 16, 32, 64, 128, 256, 512, 1024]);

/** Palette construction helpers. */
export const palette = {
  /** Build a palette from an array of `[r, g, b]` triplets. */
  fromRgb(colors: ReadonlyArray<readonly [number, number, number]>): Palette {
    if (!VALID_PALETTE_K.has(colors.length)) {
      throw new Error(
        `palette must have K ∈ {2,4,8,…,1024} colors; got ${colors.length}`,
      );
    }
    const bytes = new Uint8Array(colors.length * 3);
    for (let i = 0; i < colors.length; i++) {
      const c = colors[i]!;
      bytes[i * 3] = c[0];
      bytes[i * 3 + 1] = c[1];
      bytes[i * 3 + 2] = c[2];
    }
    return { bytes };
  },
  /** Build a palette from hex color strings (`"#aabbcc"` or `"aabbcc"`). */
  fromHex(hexes: ReadonlyArray<string>): Palette {
    const rgb: Array<readonly [number, number, number]> = hexes.map((h) => {
      const s = h.startsWith("#") ? h.slice(1) : h;
      if (s.length !== 6) {
        throw new Error(`palette: expected 6-char hex, got ${h!}`);
      }
      return [
        parseInt(s.slice(0, 2), 16),
        parseInt(s.slice(2, 4), 16),
        parseInt(s.slice(4, 6), 16),
      ] as const;
    });
    return palette.fromRgb(rgb);
  },
} as const;

/** Color encoding for shape / PIXEL modes. */
export type ColorMode =
  | { type: "rgb565" }
  | { type: "rgb888" }
  | { type: "palette"; palette: Palette };

interface CodecBase {
  color?: ColorMode;
}

/** Codec — byte-format contract. Construct via `codec.dct()`, `codec.triangle(...)`,
 *  etc. The same Codec value MUST be passed to both encode and decode. */
export type Codec =
  | { kind: "dct" }
  | ({ kind: "circle"; n: number } & CodecBase)
  | ({ kind: "triangle"; n: number } & CodecBase)
  | ({ kind: "square"; n: number } & CodecBase)
  | ({ kind: "rect"; n: number } & CodecBase)
  | ({ kind: "rotrect"; n: number; thetaBits?: number } & CodecBase)
  | ({ kind: "pixel"; n: number; gridAspect?: number } & CodecBase)
  | { kind: "raw"; spec: RawCodecSpec };

/** Low-level codec spec — every SPEC field exposed. Use via `codec.raw(...)`
 *  for advanced controls (overriding bit widths, custom alpha levels). */
export interface RawCodecSpec {
  shape: Shape;
  nShapes?: number;
  cxBits?: number;
  cyBits?: number;
  rBits?: number;
  alphaBits?: number;
  colorBits?: 16 | 24;
  thetaBits?: number;
  palette?: Uint8Array;
  paletteK?: number;
  gridAspect?: number;
}

/** Named codec recipes. Use `codec.preset(Preset.DetailTriangle)`.
 *
 *  Roughly ordered by byte budget: `TinyDct` (~21 B) → `Placeholder*` (~50–80
 *  B) → `Medium*` (~150 B) → `Detail*` (~270–400 B). Actual byte counts vary
 *  ±1 B with image aspect. */
export const Preset = {
  TinyDct: "tiny_dct",
  PlaceholderTriangle: "placeholder_triangle",
  PlaceholderCircle: "placeholder_circle",
  PlaceholderPixel: "placeholder_pixel",
  MediumTriangle: "medium_triangle",
  DetailTriangle: "detail_triangle",
  DetailCircle: "detail_circle",
} as const;
export type Preset = (typeof Preset)[keyof typeof Preset];

/** Factory namespace — `codec.dct()`, `codec.triangle({ n: 64 })`, … */
export const codec = {
  dct(): Codec {
    return { kind: "dct" };
  },
  circle(opts: { n?: number; color?: ColorMode } = {}): Codec {
    return { kind: "circle", n: opts.n ?? 12, color: opts.color };
  },
  triangle(opts: { n?: number; color?: ColorMode } = {}): Codec {
    return { kind: "triangle", n: opts.n ?? 12, color: opts.color };
  },
  square(opts: { n?: number; color?: ColorMode } = {}): Codec {
    return { kind: "square", n: opts.n ?? 12, color: opts.color };
  },
  rect(opts: { n?: number; color?: ColorMode } = {}): Codec {
    return { kind: "rect", n: opts.n ?? 12, color: opts.color };
  },
  rotatedRect(opts: {
    n?: number;
    thetaBits?: number;
    color?: ColorMode;
  } = {}): Codec {
    return {
      kind: "rotrect",
      n: opts.n ?? 12,
      thetaBits: opts.thetaBits,
      color: opts.color,
    };
  },
  pixel(opts: {
    n?: number;
    gridAspect?: number;
    color?: ColorMode;
  } = {}): Codec {
    return {
      kind: "pixel",
      n: opts.n ?? 12,
      gridAspect: opts.gridAspect,
      color: opts.color,
    };
  },
  preset(p: Preset): Codec {
    switch (p) {
      case Preset.TinyDct: return codec.dct();
      case Preset.PlaceholderTriangle: return codec.triangle({ n: 12 });
      case Preset.PlaceholderCircle: return codec.circle({ n: 12 });
      case Preset.PlaceholderPixel: return codec.pixel({ n: 16 });
      case Preset.MediumTriangle: return codec.triangle({ n: 24 });
      case Preset.DetailTriangle: return codec.triangle({ n: 64 });
      case Preset.DetailCircle: return codec.circle({ n: 64 });
    }
  },
  /** Convenience: switch a codec's color mode to palette indexing. */
  withPalette(c: Codec, palette: Palette): Codec {
    if (c.kind === "dct" || c.kind === "raw") return c;
    return { ...c, color: { type: "palette", palette } };
  },
  /** Low-level escape hatch — set any SPEC field. For conformance tests and
   *  advanced controls. Normal users should prefer the named factories. */
  raw(spec: RawCodecSpec): Codec {
    return { kind: "raw", spec };
  },
  /** True if this codec stores per-shape palette indices instead of full color. */
  isPaletteMode(c: Codec): boolean {
    if (c.kind === "raw") return c.spec.palette !== undefined;
    if (c.kind === "dct") return false;
    return c.color?.type === "palette";
  },
  /** Total byte length of a hash produced by this codec (header + body, rounded
   *  up to the byte). Matches Python's `Codec.bytes_total()` and Rust's
   *  `Codec::bytes_total()`. */
  bytesTotal(c: Codec): number {
    const cfg = codecToSpecFields(c);
    return computeBytesTotal(cfg);
  },
} as const;

interface SpecFields {
  shape: Shape;
  nShapes: number;
  cxBits: number;
  cyBits: number;
  rBits: number;
  alphaBits: number;
  colorBits: number;
  thetaBits: number;
  paletteK: number;
  hasPalette: boolean;
}

const DEFAULT_SPEC: SpecFields = {
  shape: Shape.DCT,
  nShapes: 12,
  cxBits: 5,
  cyBits: 5,
  rBits: 4,
  alphaBits: 3,
  colorBits: 16,
  thetaBits: 5,
  paletteK: 0,
  hasPalette: false,
};

function codecToSpecFields(c: Codec): SpecFields {
  if (c.kind === "raw") {
    const s = c.spec;
    const paletteLen = s.palette?.length ?? 0;
    const k = s.paletteK ?? (paletteLen > 0 ? paletteLen / 3 : 0);
    return {
      ...DEFAULT_SPEC,
      shape: s.shape,
      nShapes: s.nShapes ?? DEFAULT_SPEC.nShapes,
      cxBits: s.cxBits ?? DEFAULT_SPEC.cxBits,
      cyBits: s.cyBits ?? DEFAULT_SPEC.cyBits,
      rBits: s.rBits ?? DEFAULT_SPEC.rBits,
      alphaBits: s.alphaBits ?? DEFAULT_SPEC.alphaBits,
      colorBits: s.colorBits ?? DEFAULT_SPEC.colorBits,
      thetaBits: s.thetaBits ?? DEFAULT_SPEC.thetaBits,
      hasPalette: paletteLen > 0,
      paletteK: k,
    };
  }
  if (c.kind === "dct") return { ...DEFAULT_SPEC, shape: Shape.DCT };

  const out: SpecFields = { ...DEFAULT_SPEC, shape: c.kind, nShapes: c.n };
  if (c.kind === "rotrect" && c.thetaBits !== undefined) {
    out.thetaBits = c.thetaBits;
  }
  const color = c.color;
  if (color && color.type === "rgb888") {
    out.colorBits = 24;
  } else if (color && color.type === "palette") {
    out.hasPalette = true;
    out.paletteK = color.palette.k ?? color.palette.bytes.length / 3;
  }
  return out;
}

function colorFieldBits(s: SpecFields): number {
  if (s.hasPalette) return Math.log2(Math.max(2, s.paletteK)) | 0;
  return s.colorBits;
}

function perShapeBits(s: SpecFields): number {
  const cx = s.cxBits, cy = s.cyBits, r = s.rBits;
  const col = colorFieldBits(s), a = s.alphaBits;
  switch (s.shape) {
    case Shape.CIRCLE:
    case Shape.SQUARE: return cx + cy + r + col + a;
    case Shape.RECT: return cx + cy + 2 * r + col + a;
    case Shape.ROTATED_RECT: return cx + cy + 2 * r + s.thetaBits + col + a;
    case Shape.TRIANGLE: return 3 * (cx + cy) + col + a;
    case Shape.PIXEL: return col;
    case Shape.DCT: return 0;
  }
}

function computeBytesTotal(s: SpecFields): number {
  if (s.shape === Shape.DCT) {
    // SPEC §3 — header 40 bits + 4·(28 + 16) = 216 bits / 8 = 27 B (no alpha)
    return Math.ceil((40 + 4 * (28 + 16)) / 8);
  }
  const headerBits = s.shape === Shape.PIXEL ? 8 : 8 + colorFieldBits(s);
  return Math.ceil((headerBits + s.nShapes * perShapeBits(s)) / 8);
}

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/** Hill-climb search budget for shape modes — affects encoder cost / quality
 *  but NOT byte format. */
export interface SearchOptions {
  strategy?: "primitive" | "topk_uniform";
  nRandom?: number;
  nTopk?: number;
  hillClimbSteps?: number;
  hillClimbMaxAge?: number | null;
  nAttempts?: number;
}

export interface EncodeOptions {
  /** RNG seed for shape-mode hill-climb. Default 0. */
  seed?: number;
  /** Override the encoder's tuned search defaults. */
  search?: SearchOptions;
}

export interface DecodeOptions {
  /** Long-edge pixel target. Default 256. */
  baseSize?: number;
  /** Override the stored aspect ratio. */
  overrideAspect?: number;
  /** SHAPE-mode supersample factor. 1 = off, 2 = 4 samples, 4 = 16 samples.
   *  Ignored by DCT / PIXEL. */
  aa?: number;
  /** PIXEL only — `"nearest"` (default) or `"bilinear"`. */
  pixelSmooth?: "nearest" | "bilinear";
}

export interface SvgRenderOptions {
  baseSize?: number;
  overrideAspect?: number;
  /** Gaussian blur stdDeviation in viewBox units. `0` = no blur. */
  blur?: number;
}

export interface DecodeResult {
  w: number;
  h: number;
  /** RGBA pixels, row-major (4 bytes per pixel, length = 4·w·h). */
  rgba: Uint8Array;
}

// ---------------------------------------------------------------------------
// Wasm lifecycle — auto-init on first use
// ---------------------------------------------------------------------------

export type InitInput =
  | undefined
  | RequestInfo
  | URL
  | Response
  | BufferSource
  | WebAssembly.Module;

let wasmReady: Promise<void> | null = null;

/** Force the wasm module to load now. Optional — encode/decode/toSvg
 *  auto-init on first call. Safe to call repeatedly. */
export function init(input?: InitInput): Promise<void> {
  if (!wasmReady) {
    wasmReady = initWasm(
      input === undefined ? undefined : { module_or_path: input },
    ).then(() => undefined);
  }
  return wasmReady;
}

async function ready(): Promise<void> {
  if (!wasmReady) {
    wasmReady = initWasm().then(() => undefined);
  }
  await wasmReady;
}

// ---------------------------------------------------------------------------
// FFI plumbing
// ---------------------------------------------------------------------------

function codecToFfi(c: Codec): Record<string, unknown> {
  if (c.kind === "raw") {
    const s = c.spec;
    const out: Record<string, unknown> = { shape: s.shape };
    if (s.nShapes !== undefined) out.n_shapes = s.nShapes;
    if (s.cxBits !== undefined) out.cx_bits = s.cxBits;
    if (s.cyBits !== undefined) out.cy_bits = s.cyBits;
    if (s.rBits !== undefined) out.r_bits = s.rBits;
    if (s.alphaBits !== undefined) out.alpha_bits = s.alphaBits;
    if (s.colorBits !== undefined) out.color_bits = s.colorBits;
    if (s.thetaBits !== undefined) out.theta_bits = s.thetaBits;
    if (s.palette !== undefined && s.palette.length > 0) {
      out.palette = Array.from(s.palette);
    }
    if (s.paletteK !== undefined) out.palette_k = s.paletteK;
    if (s.gridAspect !== undefined) out.grid_aspect = s.gridAspect;
    return out;
  }

  const out: Record<string, unknown> = { shape: c.kind };
  if (c.kind === "dct") return out;
  out.n_shapes = c.n;
  if (c.kind === "rotrect" && c.thetaBits !== undefined) {
    out.theta_bits = c.thetaBits;
  }
  if (c.kind === "pixel" && c.gridAspect !== undefined) {
    out.grid_aspect = c.gridAspect;
  }
  const color = c.color;
  if (color === undefined || color.type === "rgb565") {
    out.color_bits = 16;
  } else if (color.type === "rgb888") {
    out.color_bits = 24;
  } else {
    out.color_bits = 16;
    const pal = color.palette;
    out.palette = Array.from(pal.bytes);
    if (pal.k !== undefined) out.palette_k = pal.k;
  }
  return out;
}

function searchToFfi(s: SearchOptions | undefined): unknown {
  if (!s) return undefined;
  const out: Record<string, unknown> = {};
  if (s.strategy !== undefined) out.strategy = s.strategy;
  if (s.nRandom !== undefined) out.n_random = s.nRandom;
  if (s.nTopk !== undefined) out.n_topk = s.nTopk;
  if (s.hillClimbSteps !== undefined) out.hill_climb_steps = s.hillClimbSteps;
  if (s.hillClimbMaxAge !== undefined) {
    out.hill_climb_max_age = s.hillClimbMaxAge;
  }
  if (s.nAttempts !== undefined) out.n_attempts = s.nAttempts;
  return out;
}

// ---------------------------------------------------------------------------
// Encode / Decode / SVG — all async (auto-init)
// ---------------------------------------------------------------------------

function assertReady(): void {
  if (!wasmReady) {
    throw new Error(
      "arthash: wasm not initialized — `await init()` first, or use the " +
        "async `encode()` / `decode()` which auto-initialize.",
    );
  }
}

/** Encode RGB bytes into a placeholder hash. */
export async function encode(
  rgb: Uint8Array,
  width: number,
  height: number,
  c: Codec,
  opts: EncodeOptions = {},
): Promise<Uint8Array> {
  await ready();
  return wasmEncodeRgb(
    rgb,
    width,
    height,
    codecToFfi(c),
    BigInt(opts.seed ?? 0),
    searchToFfi(opts.search),
  );
}

/** Synchronous encode — requires `await init()` to have completed first.
 *  Useful when you want to call from a tight hot loop with a known-ready
 *  wasm module. */
export function encodeSync(
  rgb: Uint8Array,
  width: number,
  height: number,
  c: Codec,
  opts: EncodeOptions = {},
): Uint8Array {
  assertReady();
  return wasmEncodeRgb(
    rgb,
    width,
    height,
    codecToFfi(c),
    BigInt(opts.seed ?? 0),
    searchToFfi(opts.search),
  );
}

/** Encode RGBA bytes. Shape codecs composite the alpha over white internally. */
export async function encodeRgba(
  rgba: Uint8Array,
  width: number,
  height: number,
  c: Codec,
  opts: EncodeOptions = {},
): Promise<Uint8Array> {
  await ready();
  return wasmEncodeRgba(
    rgba,
    width,
    height,
    codecToFfi(c),
    BigInt(opts.seed ?? 0),
    searchToFfi(opts.search),
  );
}

/** Decode a placeholder hash to RGBA pixels. */
export async function decode(
  hash: Uint8Array,
  c: Codec,
  opts: DecodeOptions = {},
): Promise<DecodeResult> {
  await ready();
  return decodeSync(hash, c, opts);
}

/** Synchronous decode — requires `await init()` to have completed first. */
export function decodeSync(
  hash: Uint8Array,
  c: Codec,
  opts: DecodeOptions = {},
): DecodeResult {
  assertReady();
  const r = wasmDecode(
    hash,
    codecToFfi(c),
    opts.baseSize ?? 256,
    opts.overrideAspect,
    opts.aa,
    opts.pixelSmooth,
  );
  const out: DecodeResult = { w: r.w, h: r.h, rgba: r.rgba };
  r.free();
  return out;
}

/** Render a shape-mode hash as a compact SVG string. */
export async function toSvg(
  hash: Uint8Array,
  c: Codec,
  opts: SvgRenderOptions = {},
): Promise<string> {
  await ready();
  return toSvgSync(hash, c, opts);
}

/** Synchronous SVG render — requires `await init()` first. */
export function toSvgSync(
  hash: Uint8Array,
  c: Codec,
  opts: SvgRenderOptions = {},
): string {
  assertReady();
  return wasmToSvg(
    hash,
    codecToFfi(c),
    opts.baseSize ?? 256,
    opts.overrideAspect,
    opts.blur ?? 0,
  );
}

// ---------------------------------------------------------------------------
// Browser convenience: image → hash in one call
// ---------------------------------------------------------------------------

/** Encoder thumbnail long-edge for a codec. */
function thumbTarget(c: Codec): number {
  return c.kind === "dct" ? 100 : 48;
}

/** Browser-only: load an image source (URL string, Blob, HTMLImageElement,
 *  ImageBitmap), resize to the codec's thumbnail target, encode in one call.
 *  Requires a DOM canvas — for Node, use `encode` / `encodeRgba` directly. */
export async function encodeImage(
  source: string | Blob | HTMLImageElement | ImageBitmap,
  c: Codec,
  opts: EncodeOptions = {},
): Promise<Uint8Array> {
  await ready();
  const { rgb, w, h } = await imageToThumbRgb(source, thumbTarget(c));
  return encode(rgb, w, h, c, opts);
}

async function imageToThumbRgb(
  source: string | Blob | HTMLImageElement | ImageBitmap,
  target: number,
): Promise<{ rgb: Uint8Array; w: number; h: number }> {
  if (typeof document === "undefined") {
    throw new Error(
      "arthash.encodeImage requires a browser DOM. In Node, decode the " +
        "image yourself and call encode(rgb, w, h, codec).",
    );
  }

  let bitmap: ImageBitmap;
  if (typeof source === "string") {
    const blob = await (await fetch(source)).blob();
    bitmap = await createImageBitmap(blob);
  } else if (source instanceof Blob) {
    bitmap = await createImageBitmap(source);
  } else if ((source as ImageBitmap).width !== undefined && (source as ImageBitmap).close) {
    bitmap = source as ImageBitmap;
  } else {
    bitmap = await createImageBitmap(source as HTMLImageElement);
  }

  const sw = bitmap.width;
  const sh = bitmap.height;
  let w: number, h: number;
  const longest = Math.max(sw, sh);
  if (longest <= target) {
    w = sw;
    h = sh;
  } else if (sw >= sh) {
    w = target;
    h = Math.max(1, Math.round((target * sh) / sw));
  } else {
    h = target;
    w = Math.max(1, Math.round((target * sw) / sh));
  }

  const canvas = document.createElement("canvas");
  canvas.width = w;
  canvas.height = h;
  const ctx = canvas.getContext("2d", { willReadFrequently: true });
  if (!ctx) throw new Error("arthash.encodeImage: 2D canvas context unavailable");
  ctx.imageSmoothingEnabled = true;
  ctx.imageSmoothingQuality = "high";
  ctx.drawImage(bitmap, 0, 0, w, h);
  const { data } = ctx.getImageData(0, 0, w, h);
  const rgb = new Uint8Array(w * h * 3);
  for (let i = 0, j = 0; i < data.length; i += 4, j += 3) {
    rgb[j] = data[i]!;
    rgb[j + 1] = data[i + 1]!;
    rgb[j + 2] = data[i + 2]!;
  }
  return { rgb, w, h };
}

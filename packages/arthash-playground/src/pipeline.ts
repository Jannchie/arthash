import {
  encodeSync as pfEncode,
  decodeSync as pfDecode,
  toSvgSync as pfToSvg,
  codec as makeCodec,
  Shape,
  type Shape as ShapeType,
  type Codec,
  type EncodeOptions,
  type DecodeOptions,
  type RawCodecSpec,
  type SearchOptions,
} from "arthash";

/** Subset of `SearchOptions` we surface on the playground toolbar. `null` /
 *  omitted values fall back to the codec's per-mode tuned defaults. */
export interface SearchConfig {
  strategy: "primitive" | "topk_uniform";
  nRandom: number;
  hillClimbMaxAge: number;
  nAttempts: number;
}

export const DEFAULT_SEARCH: SearchConfig = {
  strategy: "primitive",
  nRandom: 64,
  hillClimbMaxAge: 30,
  nAttempts: 4,
};

export const DCT_THUMB = 100;
export const SHAPE_THUMB = 48;

export function thumbTarget(shape: ShapeType): number {
  return shape === Shape.DCT ? DCT_THUMB : SHAPE_THUMB;
}

/** Derive position / radius bit widths from the render-target long edge.
 *  Calibrated to match the canonical default (base=256 → cx=cy=5, r=4) at
 *  roughly 8-pixel precision per quantization step. */
export function coordBitsForBase(base: number): { cxBits: number; cyBits: number; rBits: number } {
  const cx = Math.max(2, Math.min(8, Math.round(Math.log2(Math.max(8, base) / 8))));
  return {
    cxBits: cx,
    cyBits: cx,
    rBits: Math.max(1, Math.min(6, cx - 1)),
  };
}

// Color handling — a single user-facing `colorMode` keyed list of presets:
//
//   * "auto-N" (N ∈ {2,4,8})    — procedural palette at log2 = N bits/shape.
//                                  K ≤ 8 ⇒ grayscale ramp; K=16 ⇒ 8 grays +
//                                  8 saturated HSL hues; K ≥ 32 ⇒ uniform RGB
//                                  cube with bits biased toward G.
//   * "rgb-565" (16 bit), "rgb-888" (24 bit) — codec continuous-color modes.
//   * themed palettes (GB DMG, CGA, PICO-8, …) — fixed hand-curated colors.
//
// Themed palettes are normal palette-mode entries that happen to specify K
// concrete colors; the codec doesn't know or care that they're themed.

export interface Palette {
  colors: ReadonlyArray<readonly [number, number, number]>;
}

function level(i: number, n: number): number {
  return n === 1 ? 128 : Math.round((i / (n - 1)) * 255);
}

function grayscalePalette(k: number): Palette {
  const colors = Array.from({ length: k }, (_, i) => {
    const v = level(i, k);
    return [v, v, v] as const;
  });
  return { colors };
}

/** HSL → sRGB. h ∈ [0, 360), s,l ∈ [0, 1]. */
function hslToRgb(h: number, s: number, l: number): readonly [number, number, number] {
  const c = (1 - Math.abs(2 * l - 1)) * s;
  const x = c * (1 - Math.abs(((h / 60) % 2) - 1));
  const m = l - c / 2;
  let r = 0, g = 0, b = 0;
  if (h < 60)       { r = c; g = x; b = 0; }
  else if (h < 120) { r = x; g = c; b = 0; }
  else if (h < 180) { r = 0; g = c; b = x; }
  else if (h < 240) { r = 0; g = x; b = c; }
  else if (h < 300) { r = x; g = 0; b = c; }
  else              { r = c; g = 0; b = x; }
  return [
    Math.round((r + m) * 255),
    Math.round((g + m) * 255),
    Math.round((b + m) * 255),
  ];
}

/** RGB-cube bit splits for K ≥ 32. Sum = log2(K). G gets the extra bit. */
const RGB_CUBE_SPLITS: Record<number, readonly [number, number, number]> = {
  32:   [2, 2, 1],
  64:   [2, 2, 2],
  128:  [2, 3, 2],
  256:  [3, 3, 2],
  512:  [3, 3, 3],
  1024: [3, 4, 3],
};

function rgbCubePalette(k: number): Palette {
  const split = RGB_CUBE_SPLITS[k];
  if (!split) throw new Error(`no auto palette defined for K=${k}`);
  const [rb, gb, bb] = split;
  const rN = 1 << rb, gN = 1 << gb, bN = 1 << bb;
  const colors: Array<readonly [number, number, number]> = [];
  for (let r = 0; r < rN; r++) {
    for (let g = 0; g < gN; g++) {
      for (let b = 0; b < bN; b++) {
        colors.push([level(r, rN), level(g, gN), level(b, bN)] as const);
      }
    }
  }
  return { colors };
}

/** Auto-generated palette for the given bit depth. */
export function autoPalette(depth: number): Palette {
  const k = 1 << depth;
  if (k <= 8) return grayscalePalette(k);
  if (k === 16) {
    // 8 grays + 8 saturated hues — much friendlier for natural images than a
    // 2×4×2 RGB cube, where G is way over-represented.
    const colors: Array<readonly [number, number, number]> = [];
    for (let i = 0; i < 8; i++) {
      const v = level(i, 8);
      colors.push([v, v, v] as const);
    }
    for (let i = 0; i < 8; i++) {
      colors.push(hslToRgb((i / 8) * 360, 0.72, 0.5));
    }
    return { colors };
  }
  return rgbCubePalette(k);
}

// ---- themed palettes -----------------------------------------------------

export interface ColorOption {
  id: string;
  label: string;
  /** Palette-mode preset: bytes + K. */
  palette?: Palette;
  /** Continuous-color preset: 16 or 24. */
  colorBits?: 16 | 24;
}

const THEMED_PALETTES: ReadonlyArray<{ id: string; label: string; palette: Palette }> = [
  {
    id: "mono",
    label: "Mono · B/W",
    palette: { colors: [[0x00, 0x00, 0x00], [0xff, 0xff, 0xff]] },
  },
  {
    id: "gb-pocket",
    label: "GB Pocket · 4 gray",
    palette: { colors: [
      [0x00, 0x00, 0x00], [0x55, 0x55, 0x55], [0xaa, 0xaa, 0xaa], [0xff, 0xff, 0xff],
    ] },
  },
  {
    id: "gb-dmg",
    label: "GB DMG · 4 green",
    palette: { colors: [
      [0x08, 0x18, 0x20], [0x34, 0x68, 0x56], [0x88, 0xc0, 0x70], [0xe0, 0xf8, 0xd0],
    ] },
  },
  {
    id: "gba-warm",
    label: "GBA · 4 warm",
    palette: { colors: [
      [0x2a, 0x1b, 0x2a], [0x6f, 0x52, 0x57], [0xb9, 0x98, 0x82], [0xf3, 0xe5, 0xc0],
    ] },
  },
  {
    id: "cga",
    label: "CGA · 4",
    palette: { colors: [
      [0x00, 0x00, 0x00], [0x55, 0xff, 0xff], [0xff, 0x55, 0xff], [0xff, 0xff, 0xff],
    ] },
  },
  {
    id: "pico8",
    label: "PICO-8 · 16",
    palette: { colors: [
      [0x00, 0x00, 0x00], [0x1d, 0x2b, 0x53], [0x7e, 0x25, 0x53], [0x00, 0x87, 0x51],
      [0xab, 0x52, 0x36], [0x5f, 0x57, 0x4f], [0xc2, 0xc3, 0xc7], [0xff, 0xf1, 0xe8],
      [0xff, 0x00, 0x4d], [0xff, 0xa3, 0x00], [0xff, 0xec, 0x27], [0x00, 0xe4, 0x36],
      [0x29, 0xad, 0xff], [0x83, 0x76, 0x9c], [0xff, 0x77, 0xa8], [0xff, 0xcc, 0xaa],
    ] },
  },
  {
    id: "resurrect-64",
    label: "Resurrect · 64",
    // Kerrie Lake's general-purpose 64-color palette
    // (https://lospec.com/palette-list/resurrect-64). Wide hue/value coverage
    // for portraits, landscapes, and UI pixel art.
    palette: { colors: [
      [0x2e, 0x22, 0x2f], [0x3e, 0x35, 0x46], [0x62, 0x55, 0x65], [0x96, 0x6c, 0x6c],
      [0xab, 0x94, 0x7a], [0x69, 0x4f, 0x62], [0x7f, 0x70, 0x8a], [0x9b, 0xab, 0xb2],
      [0xc7, 0xdc, 0xd0], [0xff, 0xff, 0xff], [0x6e, 0x27, 0x27], [0xb3, 0x38, 0x31],
      [0xea, 0x4f, 0x36], [0xf5, 0x7d, 0x4a], [0xae, 0x23, 0x34], [0xe8, 0x3b, 0x3b],
      [0xfb, 0x6b, 0x1d], [0xf7, 0x96, 0x17], [0xf9, 0xc2, 0x2b], [0x7a, 0x30, 0x45],
      [0x9e, 0x45, 0x39], [0xcd, 0x68, 0x3d], [0xe6, 0x90, 0x4e], [0xfb, 0xb9, 0x54],
      [0x4c, 0x3e, 0x24], [0x67, 0x66, 0x33], [0xa2, 0xa9, 0x47], [0xd5, 0xe0, 0x4b],
      [0xfb, 0xff, 0x86], [0x16, 0x5a, 0x4c], [0x23, 0x90, 0x63], [0x1e, 0xbc, 0x73],
      [0x91, 0xdb, 0x69], [0xcd, 0xdf, 0x6c], [0x31, 0x36, 0x38], [0x37, 0x4e, 0x4a],
      [0x54, 0x7e, 0x64], [0x92, 0xa9, 0x84], [0xb2, 0xba, 0x90], [0x0b, 0x5e, 0x65],
      [0x0b, 0x8a, 0x8f], [0x0e, 0xaf, 0x9b], [0x30, 0xe1, 0xb9], [0x8f, 0xf8, 0xe2],
      [0x32, 0x33, 0x53], [0x48, 0x4a, 0x77], [0x4d, 0x65, 0xb4], [0x4d, 0x9b, 0xe6],
      [0x8f, 0xd3, 0xff], [0x45, 0x29, 0x3f], [0x6b, 0x3e, 0x75], [0x90, 0x5e, 0xa9],
      [0xa8, 0x84, 0xf3], [0xea, 0xad, 0xed], [0x75, 0x3c, 0x54], [0xa2, 0x4b, 0x6f],
      [0xcf, 0x65, 0x7f], [0xed, 0x80, 0x99], [0x83, 0x1c, 0x5d], [0xc3, 0x24, 0x54],
      [0xf0, 0x4f, 0x78], [0xf6, 0x81, 0x81], [0xfc, 0xa7, 0x90], [0xfd, 0xcb, 0xb0],
    ] },
  },
  {
    id: "pc98",
    label: "PC-98 · 16",
    // 16-color PC-98 visual-novel palette: deep purple shadows, neon magenta /
    // cyan accents, warm cream highlights — the dusk-lit, dithered look of
    // late-'80s / early-'90s NEC PC-9801 illustration games.
    palette: { colors: [
      [0x00, 0x00, 0x00], [0x22, 0x14, 0x33], [0x44, 0x2a, 0x5a], [0x6e, 0x3d, 0x8c],
      [0xa8, 0x55, 0xc0], [0xe0, 0x88, 0xd8], [0xf6, 0xb8, 0xd8], [0xd8, 0x3a, 0x7a],
      [0x3a, 0x4e, 0xa8], [0x4a, 0x90, 0xd8], [0x6a, 0xd0, 0xe0], [0x58, 0xb8, 0x80],
      [0xc8, 0x88, 0x58], [0xf0, 0xc8, 0x88], [0xf8, 0xe8, 0xb8], [0xf8, 0xf0, 0xe0],
    ] },
  },
  {
    id: "oil-6",
    label: "Oil · 6",
    // GrafxKid's 6-color "Oil" palette (https://lospec.com/palette-list/oil-6):
    // a moody, low-saturation ramp from cream to deep ink — popular for
    // atmospheric pixel art and 1-bit-style scenes with a tinted dusk feel.
    palette: { colors: [
      [0xfb, 0xf5, 0xef], [0xf2, 0xd3, 0xab], [0xc6, 0x9f, 0xa5],
      [0x8b, 0x6d, 0x9c], [0x49, 0x4d, 0x7e], [0x27, 0x27, 0x44],
    ] },
  },
  {
    id: "endesga-32",
    label: "Endesga · 32",
    // ENDESGA's 32-color palette (https://lospec.com/palette-list/endesga-32):
    // a high-contrast general-purpose set tuned for vivid game art — bold
    // primaries, clean skin tones, and crisp blues/cyans.
    palette: { colors: [
      [0xbe, 0x4a, 0x2f], [0xd7, 0x76, 0x43], [0xea, 0xd4, 0xaa], [0xe4, 0xa6, 0x72],
      [0xb8, 0x6f, 0x50], [0x73, 0x3e, 0x39], [0x3e, 0x27, 0x31], [0xa2, 0x26, 0x33],
      [0xe4, 0x3b, 0x44], [0xf7, 0x76, 0x22], [0xfe, 0xae, 0x34], [0xfe, 0xe7, 0x61],
      [0x63, 0xc7, 0x4d], [0x3e, 0x89, 0x48], [0x26, 0x5c, 0x42], [0x19, 0x3c, 0x3e],
      [0x12, 0x4e, 0x89], [0x00, 0x99, 0xdb], [0x2c, 0xe8, 0xf5], [0xff, 0xff, 0xff],
      [0xc0, 0xcb, 0xdc], [0x8b, 0x9b, 0xb4], [0x5a, 0x69, 0x88], [0x3a, 0x44, 0x66],
      [0x26, 0x2b, 0x44], [0x18, 0x14, 0x25], [0xff, 0x00, 0x44], [0x68, 0x38, 0x6c],
      [0xb5, 0x50, 0x88], [0xf6, 0x75, 0x7a], [0xe8, 0xb7, 0x96], [0xc2, 0x85, 0x69],
    ] },
  },
  {
    id: "lospec500",
    label: "Lospec500 · 42",
    // Lospec500 commemorative palette
    // (https://lospec.com/palette-list/lospec500), a 42-color community
    // palette covering full hue/value range. The "500" is the Lospec milestone
    // it commemorates, not the color count.
    palette: { colors: [
      [0x10, 0x12, 0x1c], [0x2c, 0x1e, 0x31], [0x6b, 0x26, 0x43], [0xac, 0x28, 0x47],
      [0xec, 0x27, 0x3f], [0x94, 0x49, 0x3a], [0xde, 0x5d, 0x3a], [0xe9, 0x85, 0x37],
      [0xf3, 0xa8, 0x33], [0x4d, 0x35, 0x33], [0x6e, 0x4c, 0x30], [0xa2, 0x6d, 0x3f],
      [0xce, 0x92, 0x48], [0xda, 0xb1, 0x63], [0xe8, 0xd2, 0x82], [0xf7, 0xf3, 0xb7],
      [0x1e, 0x40, 0x44], [0x00, 0x65, 0x54], [0x26, 0x85, 0x4c], [0x5a, 0xb5, 0x52],
      [0x9d, 0xe6, 0x4e], [0x00, 0x8b, 0x8b], [0x62, 0xa4, 0x77], [0xa6, 0xcb, 0x96],
      [0xd3, 0xee, 0xd3], [0x3e, 0x3b, 0x65], [0x38, 0x59, 0xb3], [0x33, 0x88, 0xde],
      [0x36, 0xc5, 0xf4], [0x6d, 0xea, 0xd6], [0x5e, 0x5b, 0x8c], [0x8c, 0x78, 0xa5],
      [0xb0, 0xa7, 0xb8], [0xde, 0xce, 0xed], [0x9a, 0x4d, 0x76], [0xc8, 0x78, 0xaf],
      [0xcc, 0x99, 0xff], [0xfa, 0x6e, 0x79], [0xff, 0xa2, 0xac], [0xff, 0xd1, 0xd5],
      [0xf6, 0xe8, 0xe0], [0xff, 0xff, 0xff],
    ] },
  },
];

/** Unified list shown in the color dropdown. Auto bit-depths first, then
 *  continuous modes, then themed palettes. */
export const COLOR_OPTIONS: ReadonlyArray<ColorOption> = [
  { id: "auto-2", label: "Auto · 2 bit · 4 gray",   palette: autoPalette(2) },
  { id: "auto-4", label: "Auto · 4 bit · 16 mixed", palette: autoPalette(4) },
  { id: "auto-8", label: "Auto · 8 bit · 256 cube", palette: autoPalette(8) },
  { id: "rgb-565", label: "16 bit · RGB-565", colorBits: 16 },
  { id: "rgb-888", label: "24 bit · RGB-888", colorBits: 24 },
  ...THEMED_PALETTES.map((t) => ({ id: t.id, label: t.label, palette: t.palette })),
];

export function getColorOption(id: string): ColorOption {
  return COLOR_OPTIONS.find((o) => o.id === id) ?? COLOR_OPTIONS[3]; // default rgb-565
}

/** Flatten a Palette into the sRGB bytes the codec expects (3·K, row-major). */
export function paletteToBytes(palette: Palette): Uint8Array {
  const out = new Uint8Array(palette.colors.length * 3);
  for (let i = 0; i < palette.colors.length; i++) {
    const c = palette.colors[i];
    out[i * 3] = c[0];
    out[i * 3 + 1] = c[1];
    out[i * 3 + 2] = c[2];
  }
  return out;
}

export function supportsSvg(shape: ShapeType): boolean {
  return (
    shape === Shape.CIRCLE
    || shape === Shape.TRIANGLE
    || shape === Shape.SQUARE
    || shape === Shape.RECT
    || shape === Shape.ROTATED_RECT
    || shape === Shape.PIXEL
  );
}

/** Pick (gw, gh) that targets ~`nTarget` cells while making each cell as
 *  square as possible for the given image aspect. The actual cell count
 *  (gw·gh) may drift slightly from nTarget because gw·gh = nTarget often
 *  has no factorization that yields square cells (e.g. 16:9 with n=12).
 *
 *  Returns [gw, gh] — both encoder & decoder will agree on this grid because
 *  the codec's own `pixel_grid` rederives the same pair from n_shapes=gw·gh
 *  for an aspect-matched n. */
export function squareCellGrid(nTarget: number, aspect: number): [number, number] {
  const idealGw = Math.sqrt(nTarget * aspect);
  const idealGh = Math.sqrt(nTarget / aspect);
  let best: [number, number] = [
    Math.max(1, Math.round(idealGw)),
    Math.max(1, Math.round(idealGh)),
  ];
  let bestScore = Infinity;
  for (const gw of [Math.floor(idealGw), Math.ceil(idealGw)]) {
    for (const gh of [Math.floor(idealGh), Math.ceil(idealGh)]) {
      if (gw < 1 || gh < 1) continue;
      const cellAspect = (aspect * gh) / gw;
      const aspectErr = Math.abs(Math.log(Math.max(1e-9, cellAspect)));
      const nDrift = Math.abs(gw * gh - nTarget) / Math.max(1, nTarget);
      // Squareness dominates; n drift mildly penalized.
      const score = aspectErr * 5 + nDrift;
      if (score < bestScore) { bestScore = score; best = [gw, gh]; }
    }
  }
  return best;
}

export function fitLongEdge(w: number, h: number, target: number) {
  if (w <= 0 || h <= 0) return { w: Math.max(1, w), h: Math.max(1, h) };
  if (Math.max(w, h) <= target) return { w, h };
  return w >= h
    ? { w: target, h: Math.max(1, Math.round((target * h) / w)) }
    : { w: Math.max(1, Math.round((target * w) / h)), h: target };
}

function rgbaToRgb(rgba: Uint8ClampedArray): Uint8Array {
  const out = new Uint8Array((rgba.length / 4) * 3);
  for (let i = 0, j = 0; i < rgba.length; i += 4, j += 3) {
    out[j] = rgba[i];
    out[j + 1] = rgba[i + 1];
    out[j + 2] = rgba[i + 2];
  }
  return out;
}

export async function loadImage(src: string, crossOrigin = true): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    if (crossOrigin) img.crossOrigin = "anonymous";
    img.onload = () => resolve(img);
    img.onerror = () => reject(new Error(`failed to load ${src}`));
    img.src = src;
  });
}

export function imageToThumbRgb(img: HTMLImageElement, target: number): { rgb: Uint8Array; w: number; h: number } {
  const { w, h } = fitLongEdge(img.naturalWidth, img.naturalHeight, target);
  const canvas = document.createElement("canvas");
  canvas.width = w;
  canvas.height = h;
  const ctx = canvas.getContext("2d", { willReadFrequently: true });
  if (!ctx) throw new Error("2D context unavailable");
  ctx.imageSmoothingEnabled = true;
  ctx.imageSmoothingQuality = "high";
  ctx.drawImage(img, 0, 0, w, h);
  const { data } = ctx.getImageData(0, 0, w, h);
  return { rgb: rgbaToRgb(data), w, h };
}

export interface RunResult {
  hash: Uint8Array;
  /** Raster decode. Undefined when the caller opted into SVG-only mode for a
   *  shape whose SVG path doesn't depend on rendered pixels — we skip the
   *  raster decode entirely so the work isn't thrown away. */
  decoded?: { w: number; h: number; rgba: Uint8Array };
  svg: string | null;
  encodeMs: number;
  decodeMs: number;
}

export interface RunOpts {
  shape: ShapeType;
  nShapes?: number;
  baseSize: number;
  seed: number;
  blur: number;
  /** Corner radius for rect / square / rotrect (viewBox units). Silently
   *  ignored for other shapes. */
  cornerRadius?: number;
  /** Override (rare) — advanced panel knob. */
  alphaBits?: number;
  /** ID of an entry in COLOR_OPTIONS. */
  colorId?: string;
  /** Hint that the caller will display SVG output. When true and the shape
   *  has a hash-driven SVG renderer (everything `supportsSvg` returns true
   *  for), the raster decode is skipped — saves an O(base²) pass per tile in
   *  Gallery / Compare. */
  useSvg?: boolean;
  /** Hill-climb search knobs. Ignored by DCT (which has no hill-climb). */
  search?: SearchConfig;
}

/** True if `cornerRadius` is honored for the given shape (rect-family). */
export function supportsCornerRadius(shape: ShapeType): boolean {
  return (
    shape === Shape.RECT
    || shape === Shape.SQUARE
    || shape === Shape.ROTATED_RECT
  );
}

/** Build a codec from playground RunOpts using the raw spec escape hatch —
 *  we need fine-grained bit-width control that the named factories don't
 *  expose (alpha_bits slider, base-size-derived cx/cy/r). */
function buildCodec(opts: RunOpts): Codec {
  const auto = coordBitsForBase(opts.baseSize);
  const spec: RawCodecSpec = {
    shape: opts.shape,
    nShapes: opts.nShapes,
    cxBits: auto.cxBits,
    cyBits: auto.cyBits,
    rBits: auto.rBits,
    alphaBits: opts.alphaBits,
  };
  const option = getColorOption(opts.colorId ?? "rgb-565");
  if (option.palette) {
    spec.palette = paletteToBytes(option.palette);
    spec.paletteK = option.palette.colors.length;
  } else if (option.colorBits !== undefined) {
    spec.colorBits = option.colorBits;
  }
  return makeCodec.raw(spec);
}

export function runPipeline(img: HTMLImageElement, opts: RunOpts): RunResult {
  const target = thumbTarget(opts.shape);
  const { rgb, w, h } = imageToThumbRgb(img, target);

  // PIXEL: pre-derive the grid so cells come out square. Override n_shapes
  // to gw·gh — the codec's own pixel_grid will redrive the same (gw, gh)
  // because the ratio matches the image aspect.
  const optsEff = { ...opts };
  if (opts.shape === Shape.PIXEL) {
    const srcAspect = img.naturalWidth / Math.max(1, img.naturalHeight);
    const [gw, gh] = squareCellGrid(opts.nShapes ?? 12, srcAspect);
    optsEff.nShapes = gw * gh;
  }

  const codec = buildCodec(optsEff);
  const encOpts: EncodeOptions = { seed: opts.seed };
  if (opts.search && opts.shape !== Shape.DCT) {
    const s: SearchOptions = {
      strategy: opts.search.strategy,
      nRandom: opts.search.nRandom,
      hillClimbMaxAge: opts.search.hillClimbMaxAge,
      nAttempts: opts.search.nAttempts,
    };
    encOpts.search = s;
  }

  const t0 = performance.now();
  const hash = pfEncode(rgb, w, h, codec, encOpts);
  const encodeMs = performance.now() - t0;

  // Style is applied uniformly across decode + to_svg — same `(hash, codec,
  // style)` produces visually matched output on both paths. cornerRadius is
  // silently ignored for non-rect-family shapes; the wasm core does this in
  // the shape-specific decode_render path.
  const style = {
    blur: opts.blur,
    cornerRadius: opts.cornerRadius ?? 0,
  };
  const decodeArgs: DecodeOptions = { baseSize: opts.baseSize, style };

  const wantsSvg = opts.useSvg === true && supportsSvg(opts.shape);

  let decoded: { w: number; h: number; rgba: Uint8Array } | undefined;
  let decodeMs = 0;
  if (!wantsSvg) {
    const t1 = performance.now();
    decoded = pfDecode(hash, codec, decodeArgs);
    decodeMs = performance.now() - t1;
  }

  let svg: string | null = null;
  if (supportsSvg(opts.shape)) {
    const tSvg0 = performance.now();
    try {
      svg = pfToSvg(hash, codec, { baseSize: opts.baseSize, style });
      if (svg && !/preserveAspectRatio=/.test(svg)) {
        svg = svg.replace(/<svg\b/, '<svg preserveAspectRatio="none"');
      }
    } catch {
      svg = null;
    }
    if (svg) decodeMs += performance.now() - tSvg0;
  }

  if (wantsSvg && svg === null) {
    const t1 = performance.now();
    decoded = pfDecode(hash, codec, decodeArgs);
    decodeMs = performance.now() - t1;
  }

  return { hash, decoded, svg, encodeMs, decodeMs };
}

// Serialize encode work across tiles. Without this, every tile's rAF callback
// queues up in the same frame, the synchronous WASM encodes run back-to-back
// without yielding, and the browser only paints once at the end — so progress
// jumps from ~1/N straight to N/N. Chaining each call behind one rAF gives the
// browser a frame to paint between encodes, which makes the progress smooth.
let encodeChain: Promise<unknown> = Promise.resolve();
export function awaitEncodeSlot(): Promise<void> {
  const slot = encodeChain.then(
    () => new Promise<void>((resolve) => requestAnimationFrame(() => resolve())),
  );
  encodeChain = slot.catch(() => undefined);
  return slot;
}

export function fmtMs(ms: number): string {
  if (!ms || !Number.isFinite(ms)) return "—";
  return ms >= 100 ? `${ms.toFixed(0)} ms` : ms >= 10 ? `${ms.toFixed(1)} ms` : `${ms.toFixed(2)} ms`;
}

export function toHex(bytes: Uint8Array): string {
  return Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
}

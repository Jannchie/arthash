import {
  encode as pfEncode,
  decode as pfDecode,
  toSvg as pfToSvg,
  Shape,
  type Shape as ShapeType,
  type EncodeOptions,
  type DecodeOptions,
} from "@pfhash/ts";

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

function hex2(v: number) { return v.toString(16).padStart(2, "0"); }

/** Render PIXEL mode as a mosaic SVG. Each cell becomes one <rect>; Y edges
 *  fall on .5 so they antialias into a single pixel row, giving the soft
 *  seams the reference layout has. Outer cells extend a full cell beyond the
 *  viewport so blur (if any) fades naturally at the edges. */
function pixelToSvg(
  decoded: { w: number; h: number; rgba: Uint8Array },
  gw: number,
  gh: number,
  blur: number,
): string {
  const W = decoded.w, H = decoded.h;
  // Cell size in viewport units. Integer math + .5 Y offsets match the
  // reference's `M-40-39.5h76v76h-76z` style.
  const cellW = W / gw;
  const cellH = H / gh;
  const parts: string[] = [
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${W} ${H}" preserveAspectRatio="none">`,
  ];
  if (blur > 0) {
    parts.push(
      `<defs><filter id="b" x="-20%" y="-20%" width="140%" height="140%"><feGaussianBlur stdDeviation="${blur}"/></filter></defs>`,
      `<g filter="url(#b)">`,
    );
  }
  for (let gy = 0; gy < gh; gy++) {
    const isFirstY = gy === 0;
    const isLastY = gy === gh - 1;
    const yTop = (isFirstY ? -cellH : (gy * cellH)) + 0.5;
    const yEnd = (isLastY ? (H + cellH) : ((gy + 1) * cellH)) + 0.5;
    const h = yEnd - yTop;
    for (let gx = 0; gx < gw; gx++) {
      const isFirstX = gx === 0;
      const isLastX = gx === gw - 1;
      const xLeft = isFirstX ? -cellW : (gx * cellW);
      const xEnd = isLastX ? (W + cellW) : ((gx + 1) * cellW);
      const w = xEnd - xLeft;
      // Sample the decoded RGBA at the cell center.
      const sx = Math.min(W - 1, Math.max(0, Math.floor((gx + 0.5) * W / gw)));
      const sy = Math.min(H - 1, Math.max(0, Math.floor((gy + 0.5) * H / gh)));
      const i = (sy * W + sx) * 4;
      const color = `#${hex2(decoded.rgba[i])}${hex2(decoded.rgba[i + 1])}${hex2(decoded.rgba[i + 2])}`;
      parts.push(
        `<rect x="${xLeft}" y="${yTop}" width="${w}" height="${h}" fill="${color}"/>`,
      );
    }
  }
  if (blur > 0) parts.push(`</g>`);
  parts.push(`</svg>`);
  return parts.join("");
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
  decoded: { w: number; h: number; rgba: Uint8Array };
  svg: string | null;
  encodeMs: number;
  decodeMs: number;
}

export interface RunOpts extends EncodeOptions, DecodeOptions {
  shape: ShapeType;
  baseSize: number;
  seed: number;
  blur: number;
  /** ID of an entry in COLOR_OPTIONS. */
  colorId?: string;
}

function codecFields(opts: RunOpts) {
  // Only emit fields the caller actually set so SDK defaults still apply.
  const out: Record<string, unknown> = { shape: opts.shape };
  if (opts.nShapes !== undefined) out.nShapes = opts.nShapes;

  // Position / radius bit widths are derived from base — base is the single
  // "precision budget" knob, and smaller base shrinks the hash.
  const auto = coordBitsForBase(opts.baseSize);
  out.cxBits = opts.cxBits ?? auto.cxBits;
  out.cyBits = opts.cyBits ?? auto.cyBits;
  out.rBits = opts.rBits ?? auto.rBits;
  if (opts.alphaBits !== undefined) out.alphaBits = opts.alphaBits;

  const option = getColorOption(opts.colorId ?? "rgb-565");
  if (option.palette) {
    out.palette = paletteToBytes(option.palette);
    out.paletteK = option.palette.colors.length;
  } else if (option.colorBits !== undefined) {
    out.colorBits = option.colorBits;
  }
  return out;
}

export function runPipeline(img: HTMLImageElement, opts: RunOpts): RunResult {
  const target = thumbTarget(opts.shape);
  const { rgb, w, h } = imageToThumbRgb(img, target);

  // PIXEL: pre-derive the grid so cells come out square. Override n_shapes
  // to gw·gh — the codec's own pixel_grid will redrive the same (gw, gh)
  // because the ratio matches the image aspect.
  let pixelGW = 0, pixelGH = 0;
  const optsEff = { ...opts };
  if (opts.shape === Shape.PIXEL) {
    const srcAspect = img.naturalWidth / Math.max(1, img.naturalHeight);
    const [gw, gh] = squareCellGrid(opts.nShapes ?? 12, srcAspect);
    pixelGW = gw;
    pixelGH = gh;
    optsEff.nShapes = gw * gh;
  }

  const codec = codecFields(optsEff);

  const t0 = performance.now();
  const hash = pfEncode(rgb, w, h, { ...codec, seed: opts.seed } as EncodeOptions);
  const encodeMs = performance.now() - t0;

  const decodeArgs: DecodeOptions = {
    ...codec,
    baseSize: opts.baseSize,
  };
  if (opts.overrideAspect !== undefined && opts.overrideAspect > 0) {
    decodeArgs.overrideAspect = opts.overrideAspect;
  }

  const t1 = performance.now();
  const decoded = pfDecode(hash, decodeArgs);
  const decodeMs = performance.now() - t1;

  let svg: string | null = null;
  if (supportsSvg(opts.shape)) {
    if (opts.shape === Shape.PIXEL) {
      // Use the same (gw, gh) we forced into the codec above so the SVG
      // mosaic exactly matches the encoded grid.
      svg = pixelToSvg(decoded, pixelGW, pixelGH, opts.blur);
    } else {
      try {
        svg = pfToSvg(hash, { ...decodeArgs, blur: opts.blur });
        // Force the SVG to stretch into the tile rather than letterbox — the
        // decoded viewBox's aspect can differ from the tile's CSS aspect by a
        // pixel of rounding (most visible at small `base`), which otherwise
        // leaves a thin white bar.
        if (svg && !/preserveAspectRatio=/.test(svg)) {
          svg = svg.replace(/<svg\b/, '<svg preserveAspectRatio="none"');
        }
      } catch {
        svg = null;
      }
    }
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

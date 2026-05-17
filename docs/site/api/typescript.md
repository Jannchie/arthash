# TypeScript API

Package: [`arthash`](https://www.npmjs.com/package/arthash) on npm.
Backed by `arthash-rs` compiled to WebAssembly via wasm-bindgen.

```ts
import {
  encode, decode, toSvg,
  encodeSync, decodeSync, toSvgSync,
  encodeImage, init,
  codec, palette, palettes,
  Preset, Shape,
} from "arthash";
```

## Lifecycle

### `init(input?)`

Force the wasm module to load now. Optional — `encode` / `decode` / `toSvg`
auto-initialise on first call. Safe to call repeatedly.

```ts
init(input?: InitInput): Promise<void>

type InitInput =
  | undefined        // default: fetch wasm from the package URL
  | RequestInfo
  | URL
  | Response
  | BufferSource
  | WebAssembly.Module;
```

Use the custom input when bundling rules force you to load the `.wasm` from a
non-default path.

## Encoding

### `encode(rgb, width, height, codec, opts?)`

Encode RGB bytes (row-major, 3 bytes per pixel) into a hash. Async — auto-inits
the wasm module if needed.

```ts
encode(
  rgb: Uint8Array,
  width: number,
  height: number,
  c: Codec,
  opts?: EncodeOptions,
): Promise<Uint8Array>
```

### `encodeRgba(rgba, width, height, codec, opts?)`

Same as `encode` but takes RGBA. Shape codecs composite alpha over white
internally.

### `encodeSync(rgb, width, height, codec, opts?)`

Synchronous variant. Requires `await init()` to have completed first.

### `encodeImage(source, codec, opts?)` (browser only)

Load + resize + encode in one call.

```ts
encodeImage(
  source: string | Blob | HTMLImageElement | ImageBitmap,
  c: Codec,
  opts?: EncodeOptions,
): Promise<Uint8Array>
```

The image is resized so the long edge matches the codec's thumbnail target
(48 px for shape modes, ≤ 100 px for DCT), then RGB is extracted via a 2D
canvas. Throws in environments without `document` (use `sharp` / `jimp` in
Node).

### `EncodeOptions`

```ts
interface EncodeOptions {
  seed?: number;             // RNG seed for hill-climb; default 0
  search?: SearchOptions;    // override search budget (encoder cost, not byte format)
}

interface SearchOptions {
  strategy?: "primitive" | "topk_uniform";
  nRandom?: number;
  nTopk?: number;
  hillClimbSteps?: number;
  hillClimbMaxAge?: number | null;
  nAttempts?: number;
}
```

`SearchOptions` affects only encoder cost and quality — the resulting hash is
byte-format-identical regardless of search budget.

## Decoding

### `decode(hash, codec, opts?)`

```ts
decode(
  hash: Uint8Array,
  c: Codec,
  opts?: DecodeOptions,
): Promise<DecodeResult>

interface DecodeOptions {
  baseSize?: number;                    // long-edge target in px; default 256
  overrideAspect?: number;              // override stored aspect ratio
  aa?: number;                          // shape supersample (1 / 2 / 4)
  pixelSmooth?: "nearest" | "bilinear"; // PIXEL only; default "nearest"
}

interface DecodeResult {
  w: number;
  h: number;
  rgba: Uint8Array;                     // row-major, length = 4·w·h
}
```

### `decodeSync(hash, codec, opts?)`

Synchronous variant. Requires `await init()` first.

## SVG render

### `toSvg(hash, codec, opts?)`

```ts
toSvg(
  hash: Uint8Array,
  c: Codec,
  opts?: SvgRenderOptions,
): Promise<string>

interface SvgRenderOptions {
  baseSize?: number;            // long-edge in px (viewBox units); default 256
  overrideAspect?: number;
  blur?: number;                // Gaussian stdDeviation in viewBox units; 0 = off
}
```

Only supports `CIRCLE` / `TRIANGLE` / `SQUARE` / `RECT` / `ROTATED_RECT`. DCT
and PIXEL have no SVG primitive form and throw.

### `toSvgSync(hash, codec, opts?)`

Synchronous variant. Requires `await init()` first.

## Codec

### `Codec` (discriminated union)

```ts
type Codec =
  | { kind: "dct" }
  | ({ kind: "circle"; n: number } & CodecBase)
  | ({ kind: "triangle"; n: number } & CodecBase)
  | ({ kind: "square"; n: number } & CodecBase)
  | ({ kind: "rect"; n: number } & CodecBase)
  | ({ kind: "rotrect"; n: number; thetaBits?: number } & CodecBase)
  | ({ kind: "pixel"; n: number; gridAspect?: number } & CodecBase)
  | { kind: "raw"; spec: RawCodecSpec };

interface CodecBase {
  color?: ColorMode;
}

type ColorMode =
  | { type: "rgb565" }
  | { type: "rgb888" }
  | { type: "palette"; palette: Palette };
```

### `codec` (factory namespace)

| Function                         | Returns                                              |
| -------------------------------- | ---------------------------------------------------- |
| `codec.dct()`                    | `{ kind: "dct" }`                                    |
| `codec.circle({ n? })`           | `{ kind: "circle", n }`                              |
| `codec.triangle({ n? })`         | `{ kind: "triangle", n }`                            |
| `codec.square({ n? })`           | `{ kind: "square", n }`                              |
| `codec.rect({ n? })`             | `{ kind: "rect", n }`                                |
| `codec.rotatedRect({ n?, thetaBits? })` | `{ kind: "rotrect", n, thetaBits }`           |
| `codec.pixel({ n?, gridAspect? })` | `{ kind: "pixel", n, gridAspect }`                 |
| `codec.preset(p)`                | preset → factory return                              |
| `codec.withPalette(c, palette)`  | clone of `c` with `color = { type: "palette", ... }` |
| `codec.raw(spec)`                | low-level escape hatch                               |
| `codec.isPaletteMode(c)`         | `boolean`                                            |
| `codec.bytesTotal(c)`            | `number` — total hash bytes for this codec           |

### `Preset`

```ts
enum Preset {
  TinyDct, PlaceholderTriangle, PlaceholderCircle, PlaceholderPixel,
  MediumTriangle, MediumCircle, MediumPixel,
  DetailTriangle, DetailCircle, DetailPixel,
}
```

### `RawCodecSpec`

Low-level codec spec — every SPEC field exposed. Use via `codec.raw(...)` for
conformance tests and advanced controls.

```ts
interface RawCodecSpec {
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
```

## Palette

```ts
interface Palette {
  bytes: Uint8Array;       // flat row-major sRGB, length = 3·K
  k?: number;
}
```

### `palette` (factory)

| Function                  | Description                                        |
| ------------------------- | -------------------------------------------------- |
| `palette.fromRgb(colors)` | Build from `[[r,g,b], ...]` triplets (K power of 2) |
| `palette.fromHex(hexes)`  | Build from `"#rrggbb"` strings                     |

### `palettes`

Bundled constants — `palettes.PICO8`, `palettes.GAMEBOY`, etc. See [Palettes](../guide/palettes).

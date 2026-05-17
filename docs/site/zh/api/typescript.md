# TypeScript API

npm 包：[`arthash`](https://www.npmjs.com/package/arthash)。底层是通过 wasm-bindgen 编译的 `arthash-rs`。

```ts
import {
  encode, decode, toSvg,
  encodeSync, decodeSync, toSvgSync,
  encodeImage, init,
  codec, palette, palettes,
  Preset, Shape,
} from "arthash";
```

## 生命周期

### `init(input?)`

强制立即加载 wasm 模块。可选——`encode` / `decode` / `toSvg` 在首次调用时会自动初始化。重复调用安全。

```ts
init(input?: InitInput): Promise<void>

type InitInput =
  | undefined        // 默认：从 package 的 URL fetch wasm
  | RequestInfo
  | URL
  | Response
  | BufferSource
  | WebAssembly.Module;
```

打包规则要求从非默认路径加载 `.wasm` 时，用 `input` 参数手动指定。

## 编码

### `encode(rgb, width, height, codec, opts?)`

把 RGB 字节（row-major，每像素 3 字节）编成 hash。异步——必要时自动初始化 wasm。

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

同上，但输入是 RGBA。shape codec 会在内部把 alpha 合成到白色背景上。

### `encodeSync(rgb, width, height, codec, opts?)`

同步版本。需要先 `await init()`。

### `encodeImage(source, codec, opts?)`（仅浏览器）

加载 + 缩放 + 编码一步完成。

```ts
encodeImage(
  source: string | Blob | HTMLImageElement | ImageBitmap,
  c: Codec,
  opts?: EncodeOptions,
): Promise<Uint8Array>
```

图片会被缩放到 codec 的缩略图目标长边尺寸（shape 模式 48 px，DCT ≤ 100 px），再用 2D canvas 抽 RGB。在没有 `document` 的环境会抛错（Node 请用 `sharp` / `jimp`）。

### `EncodeOptions`

```ts
interface EncodeOptions {
  seed?: number;             // hill-climb 的 RNG seed，默认 0
  search?: SearchOptions;    // 覆盖搜索预算（影响编码成本，不影响字节格式）
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

`SearchOptions` 只影响编码成本与质量——无论预算如何，最终 hash 的字节格式相同。

## 解码

### `decode(hash, codec, opts?)`

```ts
decode(
  hash: Uint8Array,
  c: Codec,
  opts?: DecodeOptions,
): Promise<DecodeResult>

interface DecodeOptions {
  baseSize?: number;                    // 长边目标 px，默认 256
  overrideAspect?: number;              // 覆盖存储的宽高比
  aa?: number;                          // shape 模式超采样（1 / 2 / 4）
  pixelSmooth?: "nearest" | "bilinear"; // 仅 PIXEL；默认 "nearest"
}

interface DecodeResult {
  w: number;
  h: number;
  rgba: Uint8Array;                     // row-major，长度 = 4·w·h
}
```

### `decodeSync(hash, codec, opts?)`

同步版本。需要先 `await init()`。

## SVG 渲染

### `toSvg(hash, codec, opts?)`

```ts
toSvg(
  hash: Uint8Array,
  c: Codec,
  opts?: SvgRenderOptions,
): Promise<string>

interface SvgRenderOptions {
  baseSize?: number;            // 长边 px（viewBox 单位）；默认 256
  overrideAspect?: number;
  blur?: number;                // 高斯 stdDeviation，单位 viewBox；0 = 关
}
```

只支持 `CIRCLE` / `TRIANGLE` / `SQUARE` / `RECT` / `ROTATED_RECT`。DCT 和 PIXEL 没有 SVG 原语形式，调用会抛错。

### `toSvgSync(hash, codec, opts?)`

同步版本。需要先 `await init()`。

## Codec

### `Codec`（区分联合）

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

### `codec`（工厂命名空间）

| 函数                                  | 返回                                                 |
| ------------------------------------- | ---------------------------------------------------- |
| `codec.dct()`                         | `{ kind: "dct" }`                                    |
| `codec.circle({ n? })`                | `{ kind: "circle", n }`                              |
| `codec.triangle({ n? })`              | `{ kind: "triangle", n }`                            |
| `codec.square({ n? })`                | `{ kind: "square", n }`                              |
| `codec.rect({ n? })`                  | `{ kind: "rect", n }`                                |
| `codec.rotatedRect({ n?, thetaBits? })` | `{ kind: "rotrect", n, thetaBits }`              |
| `codec.pixel({ n?, gridAspect? })`    | `{ kind: "pixel", n, gridAspect }`                   |
| `codec.preset(p)`                     | 预设 → 工厂返回值                                    |
| `codec.withPalette(c, palette)`       | 把 `c` 的颜色模式切到 palette                        |
| `codec.raw(spec)`                     | 低层入口                                             |
| `codec.isPaletteMode(c)`              | `boolean`                                            |
| `codec.bytesTotal(c)`                 | `number` —— 此 codec 编出的 hash 总字节数            |

### `Preset`

```ts
enum Preset {
  TinyDct, PlaceholderTriangle, PlaceholderCircle, PlaceholderPixel,
  MediumTriangle, MediumCircle, MediumPixel,
  DetailTriangle, DetailCircle, DetailPixel,
}
```

### `RawCodecSpec`

低层 codec 规格——SPEC 每个字段都暴露。通过 `codec.raw(...)` 使用，适合一致性测试和高级控制。

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
  bytes: Uint8Array;       // 扁平 row-major sRGB，长度 = 3·K
  k?: number;
}
```

### `palette`（工厂）

| 函数                      | 描述                                              |
| ------------------------- | ------------------------------------------------- |
| `palette.fromRgb(colors)` | 从 `[[r,g,b], ...]` 构造（K 必须为 2 的幂）        |
| `palette.fromHex(hexes)`  | 从 `"#rrggbb"` 字符串构造                          |

### `palettes`

内置常量——`palettes.PICO8`、`palettes.GAMEBOY` 等。详见 [调色板](../guide/palettes)。

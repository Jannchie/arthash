# TypeScript API

npm 包：[`arthash`](https://www.npmjs.com/package/arthash)。底层是通过 wasm-bindgen 编译的 `arthash-rs`。

```ts
import {
  encode, decode, toSvg,
  encodeSync, decodeSync, toSvgSync,
  toImageData, toImageBitmap,
  encodeImage, init,
  codec, palette, palettes,
  Preset, Shape,
  type RenderStyle,
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
decode<C extends Codec>(
  hash: Uint8Array,
  c: C,
  opts?: DecodeOptions & { style?: RenderStyle<C> },
): Promise<DecodeResult>

interface DecodeOptions {
  baseSize?: number;                    // 长边目标 px，默认 256
  overrideAspect?: number;              // 覆盖存储的宽高比
  aa?: number;                          // shape 模式超采样（1 / 2 / 4）
  pixelSmooth?: "nearest" | "bilinear"; // 仅 PIXEL；默认 "nearest"
  style?: RenderStyle;                  // 视觉样式（见下）
}

interface DecodeResult {
  w: number;
  h: number;
  rgba: Uint8Array;                     // row-major，长度 = 4·w·h
}
```

### `decodeSync(hash, codec, opts?)`

同步版本。需要先 `await init()`。

### `toImageData(hash, codec, opts?)`

仅浏览器。返回可直接喂给 `ctx.putImageData(...)` 的 `ImageData`。参数同
`decode`。Node 下抛错——请用 `decode()` 配合自己的图像库（sharp /
@napi-rs/canvas / jimp）。

```ts
toImageData<C extends Codec>(
  hash: Uint8Array,
  c: C,
  opts?: DecodeOptions & { style?: RenderStyle<C> },
): Promise<ImageData>
```

### `toImageBitmap(hash, codec, opts?)`

仅浏览器。返回适合 GPU 上传的 `ImageBitmap`（`drawImage`、
`texSubImage2D`、worker `postMessage` transfer list）。参数同 `decode`。

```ts
toImageBitmap<C extends Codec>(
  hash: Uint8Array,
  c: C,
  opts?: DecodeOptions & { style?: RenderStyle<C> },
): Promise<ImageBitmap>
```

## SVG 渲染

### `toSvg(hash, codec, opts?)`

```ts
toSvg<C extends Codec>(
  hash: Uint8Array,
  c: C,
  opts?: SvgRenderOptions & { style?: RenderStyle<C> },
): Promise<string>

interface SvgRenderOptions {
  baseSize?: number;            // 长边 px（viewBox 单位）；默认 256
  overrideAspect?: number;
  style?: RenderStyle;          // 视觉样式（见下）
  /** @deprecated 0.3.0 起弃用——请改用 `style.blur`。1.0 移除。 */
  blur?: number;
}
```

只支持 `CIRCLE` / `TRIANGLE` / `SQUARE` / `RECT` / `ROTATED_RECT`。DCT 和 PIXEL 没有 SVG 原语形式，调用会抛错。

### `toSvgSync(hash, codec, opts?)`

同步版本。需要先 `await init()`。

## RenderStyle

```ts
type RenderStyle<C extends Codec = Codec> = C extends {
  kind: "rect" | "square" | "rotrect";
} ? {
  blur?: number;          // 高斯 stdDeviation（viewBox 单位），0 = 锐利
  cornerRadius?: number;  // 圆角半径（viewBox 单位），仅 rect 家族
} : {
  blur?: number;
  cornerRadius?: never;   // 非 rect 家族 codec 上设置 = 编译期错误
};
```

`RenderStyle` 独立于 codec 的字节格式——同一 `(hash, codec)` 配不同 `style`
产生视觉不同但字节不变的输出。默认值（两个字段都为 0）走零成本快路径。

`<C>` 泛型在类型层强制 `cornerRadius` 只能用于 rect / square / rotrect
codec——传给 circle / triangle / pixel / dct 是编译期错误。PIXEL 故意排除
（瓦片网格在圆角下会出现可见缝隙）。

两个字段在 `decode` / `toSvg` / `toImageData` / `toImageBitmap` 之间应用
方式一致——`(hash, codec, style)` 在 raster 和 SVG 路径上视觉对齐。

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
// 尺寸轴：small (n=12, pixel n=16) / medium (n=24) / large (n=64)
const Preset = {
  Dct,
  SmallTriangle, SmallCircle, SmallPixel, SmallRect, SmallSquare,
  MediumTriangle, MediumCircle, MediumPixel, MediumRect, MediumSquare,
  LargeTriangle, LargeCircle, LargePixel, LargeRect, LargeSquare,

  // 0.3 之前的别名（JSDoc @deprecated 标注），为 source 兼容保留。
  TinyDct, PlaceholderTriangle, PlaceholderCircle, PlaceholderPixel,
  DetailTriangle, DetailCircle, DetailPixel,
};
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

# TypeScript API

npm パッケージ：[`arthash`](https://www.npmjs.com/package/arthash)。wasm-bindgen 経由でコンパイルされた `arthash-rs` がバックエンド。

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

## ライフサイクル

### `init(input?)`

wasm モジュールを今すぐロードします。オプション—`encode` / `decode` / `toSvg` は初回呼び出し時に自動初期化されます。繰り返し呼び出しても安全です。

```ts
init(input?: InitInput): Promise<void>

type InitInput =
  | undefined        // デフォルト：パッケージ URL から wasm を fetch
  | RequestInfo
  | URL
  | Response
  | BufferSource
  | WebAssembly.Module;
```

バンドラーが非デフォルトパスから `.wasm` をロードさせる必要がある場合、カスタム input を使ってください。

## エンコード

### `encode(rgb, width, height, codec, opts?)`

RGB バイト（row-major、ピクセル 3 バイト）をハッシュにエンコード。非同期—必要なら wasm を自動初期化します。

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

`encode` と同じですが RGBA を受け取ります。shape codec は内部で alpha を白に合成します。

### `encodeSync(rgb, width, height, codec, opts?)`

同期版。先に `await init()` 完了が必要。

### `encodeImage(source, codec, opts?)`（ブラウザ専用）

ロード + リサイズ + エンコードを一度に。

```ts
encodeImage(
  source: string | Blob | HTMLImageElement | ImageBitmap,
  c: Codec,
  opts?: EncodeOptions,
): Promise<Uint8Array>
```

画像は codec のサムネイルターゲット（shape モードは 48 px 長辺、DCT は ≤ 100 px）にリサイズされ、2D canvas で RGB が抽出されます。`document` のない環境では throw します（Node では `sharp` / `jimp` を使ってください）。

### `EncodeOptions`

```ts
interface EncodeOptions {
  seed?: number;             // hill-climb の RNG seed、デフォルト 0
  search?: SearchOptions;    // 検索予算の上書き（エンコードコストに影響、バイト形式には影響なし）
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

`SearchOptions` はエンコードコストと品質にのみ影響します—予算に関わらず結果ハッシュのバイト形式は同一です。

## デコード

### `decode(hash, codec, opts?)`

```ts
decode<C extends Codec>(
  hash: Uint8Array,
  c: C,
  opts?: DecodeOptions & { style?: RenderStyle<C> },
): Promise<DecodeResult>

interface DecodeOptions {
  baseSize?: number;                    // 長辺の目標 px、デフォルト 256
  overrideAspect?: number;              // 格納されているアスペクト比を上書き
  aa?: number;                          // shape スーパーサンプル（1 / 2 / 4）
  pixelSmooth?: "nearest" | "bilinear"; // PIXEL のみ、デフォルト "nearest"
  style?: RenderStyle;                  // 視覚スタイル（後述）
}

interface DecodeResult {
  w: number;
  h: number;
  rgba: Uint8Array;                     // row-major、長さ = 4·w·h
}
```

### `decodeSync(hash, codec, opts?)`

同期版。先に `await init()` 完了が必要。

### `toImageData(hash, codec, opts?)`

ブラウザ専用。`ctx.putImageData(...)` にそのまま渡せる `ImageData` を返
します。オプションは `decode` と同一。Node では throw します—`decode()` と
お好みの画像ライブラリ（sharp / @napi-rs/canvas / jimp）を使ってください。

```ts
toImageData<C extends Codec>(
  hash: Uint8Array,
  c: C,
  opts?: DecodeOptions & { style?: RenderStyle<C> },
): Promise<ImageData>
```

### `toImageBitmap(hash, codec, opts?)`

ブラウザ専用。GPU アップロード（`drawImage`、`texSubImage2D`、worker
`postMessage` の transfer list）に適した `ImageBitmap` を返します。
オプションは `decode` と同一。

```ts
toImageBitmap<C extends Codec>(
  hash: Uint8Array,
  c: C,
  opts?: DecodeOptions & { style?: RenderStyle<C> },
): Promise<ImageBitmap>
```

## SVG レンダリング

### `toSvg(hash, codec, opts?)`

```ts
toSvg<C extends Codec>(
  hash: Uint8Array,
  c: C,
  opts?: SvgRenderOptions & { style?: RenderStyle<C> },
): Promise<string>

interface SvgRenderOptions {
  baseSize?: number;            // 長辺 px（viewBox 単位）、デフォルト 256
  overrideAspect?: number;
  style?: RenderStyle;          // 視覚スタイル（後述）
  /** @deprecated 0.3.0 以降—`style.blur` を使ってください。1.0 で削除。 */
  blur?: number;
}
```

`CIRCLE` / `TRIANGLE` / `SQUARE` / `RECT` / `ROTATED_RECT` のみサポート。DCT と PIXEL は SVG プリミティブ表現がないため throw します。

### `toSvgSync(hash, codec, opts?)`

同期版。先に `await init()` 完了が必要。

## RenderStyle

```ts
type RenderStyle<C extends Codec = Codec> = C extends {
  kind: "rect" | "square" | "rotrect";
} ? {
  blur?: number;          // ガウスの stdDeviation（viewBox 単位）、0 = シャープ
  cornerRadius?: number;  // 角丸半径（viewBox 単位）、rect ファミリーのみ
} : {
  blur?: number;
  cornerRadius?: never;   // 非 rect ファミリーで設定するとコンパイルエラー
};
```

`RenderStyle` は codec のバイト形式と独立しています—同じ `(hash, codec)`
に異なる `style` を渡すと、ハッシュバイトを変えずに視覚的に異なる出力が
得られます。デフォルト（両フィールド 0）はゼロコストの fast path。

`<C>` ジェネリクスにより、`cornerRadius` は rect / square / rotrect codec
だけで使えることが型レベルで強制されます—circle / triangle / pixel / dct
codec に渡すとコンパイルエラー。PIXEL は意図的に除外（タイルグリッドに
角丸を適用すると隣接セル間に縫い目が見える）。

両フィールドは `decode` / `toSvg` / `toImageData` / `toImageBitmap` で
同じ意味で適用され、`(hash, codec, style)` は raster と SVG の経路で
視覚的に揃った出力を生みます。

## Codec

### `Codec`（判別共用体）

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

### `codec`（ファクトリ名前空間）

| 関数                                  | 戻り値                                                |
| ------------------------------------- | ----------------------------------------------------- |
| `codec.dct()`                         | `{ kind: "dct" }`                                     |
| `codec.circle({ n? })`                | `{ kind: "circle", n }`                               |
| `codec.triangle({ n? })`              | `{ kind: "triangle", n }`                             |
| `codec.square({ n? })`                | `{ kind: "square", n }`                               |
| `codec.rect({ n? })`                  | `{ kind: "rect", n }`                                 |
| `codec.rotatedRect({ n?, thetaBits? })` | `{ kind: "rotrect", n, thetaBits }`                 |
| `codec.pixel({ n?, gridAspect? })`    | `{ kind: "pixel", n, gridAspect }`                    |
| `codec.preset(p)`                     | プリセット → ファクトリ戻り値                          |
| `codec.withPalette(c, palette)`       | `c` の色モードをパレットに切り替えたクローン           |
| `codec.raw(spec)`                     | 低レベル入口                                           |
| `codec.isPaletteMode(c)`              | `boolean`                                             |
| `codec.bytesTotal(c)`                 | `number` — この codec のハッシュ総バイト数             |

### `Preset`

```ts
// サイズ軸：small (n=12, pixel n=16) / medium (n=24) / large (n=64)
const Preset = {
  Dct,
  SmallTriangle, SmallCircle, SmallPixel, SmallRect, SmallSquare,
  MediumTriangle, MediumCircle, MediumPixel, MediumRect, MediumSquare,
  LargeTriangle, LargeCircle, LargePixel, LargeRect, LargeSquare,

  // 0.3 以前の非推奨エイリアス（JSDoc @deprecated）—ソース互換のため保持。
  TinyDct, PlaceholderTriangle, PlaceholderCircle, PlaceholderPixel,
  DetailTriangle, DetailCircle, DetailPixel,
};
```

### `RawCodecSpec`

低レベル codec 仕様—SPEC の全フィールドを公開します。`codec.raw(...)` 経由で使用、適合性テストと高度な制御向け。

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
  bytes: Uint8Array;       // フラット row-major sRGB、長さ = 3·K
  k?: number;
}
```

### `palette`（ファクトリ）

| 関数                      | 説明                                                  |
| ------------------------- | ----------------------------------------------------- |
| `palette.fromRgb(colors)` | `[[r,g,b], ...]` 三組から構築（K は 2 の冪）           |
| `palette.fromHex(hexes)`  | `"#rrggbb"` 文字列から構築                            |

### `palettes`

同梱定数—`palettes.PICO8`、`palettes.GAMEBOY` など。詳細は [パレット](../guide/palettes) を参照。

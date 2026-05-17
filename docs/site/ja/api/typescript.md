# TypeScript API

npm パッケージ：[`arthash`](https://www.npmjs.com/package/arthash)。wasm-bindgen 経由でコンパイルされた `arthash-rs` がバックエンド。

```ts
import {
  encode, decode, toSvg,
  encodeSync, decodeSync, toSvgSync,
  encodeImage, init,
  codec, palette, palettes,
  Preset, Shape,
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
decode(
  hash: Uint8Array,
  c: Codec,
  opts?: DecodeOptions,
): Promise<DecodeResult>

interface DecodeOptions {
  baseSize?: number;                    // 長辺の目標 px、デフォルト 256
  overrideAspect?: number;              // 格納されているアスペクト比を上書き
  aa?: number;                          // shape スーパーサンプル（1 / 2 / 4）
  pixelSmooth?: "nearest" | "bilinear"; // PIXEL のみ、デフォルト "nearest"
}

interface DecodeResult {
  w: number;
  h: number;
  rgba: Uint8Array;                     // row-major、長さ = 4·w·h
}
```

### `decodeSync(hash, codec, opts?)`

同期版。先に `await init()` 完了が必要。

## SVG レンダリング

### `toSvg(hash, codec, opts?)`

```ts
toSvg(
  hash: Uint8Array,
  c: Codec,
  opts?: SvgRenderOptions,
): Promise<string>

interface SvgRenderOptions {
  baseSize?: number;            // 長辺 px（viewBox 単位）、デフォルト 256
  overrideAspect?: number;
  blur?: number;                // ガウスの stdDeviation（viewBox 単位）、0 = オフ
}
```

`CIRCLE` / `TRIANGLE` / `SQUARE` / `RECT` / `ROTATED_RECT` のみサポート。DCT と PIXEL は SVG プリミティブ表現がないため throw します。

### `toSvgSync(hash, codec, opts?)`

同期版。先に `await init()` 完了が必要。

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
enum Preset {
  TinyDct, PlaceholderTriangle, PlaceholderCircle, PlaceholderPixel,
  MediumTriangle, MediumCircle, MediumPixel,
  DetailTriangle, DetailCircle, DetailPixel,
}
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

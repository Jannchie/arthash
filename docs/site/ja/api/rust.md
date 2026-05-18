# Rust API

crate：[`arthash`](https://crates.io/crates/arthash)。正規実装—Python と TypeScript バインディングは同じコードを呼び出します。

```rust
use arthash::{
    Codec, Preset,
    encode_rgb, encode_rgba, decode, to_svg,
    EncodeOptions, DecodeOptions, SvgOptions,
    RenderStyle,
    SearchOptions, SearchStrategy,
    Palette,
};
```

## トップレベル関数

### `encode_rgb(rgb, width, height, codec, opts) -> Vec<u8>`

```rust
pub fn encode_rgb(
    rgb: &[u8],
    width: u32,
    height: u32,
    codec: &Codec,
    opts: EncodeOptions,
) -> Vec<u8>;
```

row-major RGB、ピクセル 3 バイト；長さは `3 * width * height` と一致する必要があります。

### `encode_rgba(rgba, width, height, codec, opts) -> Vec<u8>`

RGBA 入力に対する `encode_rgb`。shape モードは内部で alpha を白に合成します。

### `decode(hash, codec, opts) -> DecodeResult`

```rust
pub struct DecodeResult {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,     // row-major、ピクセル 4 バイト
}

pub fn decode(
    hash: &[u8],
    codec: &Codec,
    opts: DecodeOptions,
) -> DecodeResult;
```

### `to_svg(hash, codec, opts) -> Result<String, SvgError>`

```rust
pub fn to_svg(
    hash: &[u8],
    codec: &Codec,
    opts: SvgOptions,
) -> Result<String, SvgError>;
```

shape モード（`CIRCLE`、`TRIANGLE`、`SQUARE`、`RECT`、`ROTATED_RECT`）のみサポート。`DCT` は `Err(SvgError::UnsupportedShape(_))` を返します。

## Options

```rust
pub struct EncodeOptions {
    pub seed: u64,                       // デフォルト 0
    pub search: Option<SearchOptions>,   // 検索予算を上書き
}

pub struct DecodeOptions {
    pub base_size: u32,                  // デフォルト 256
    pub override_aspect: Option<f32>,
    pub aa: u32,                         // shape スーパーサンプル（1 / 2 / 4）
    pub pixel_smooth: PixelSmooth,
    pub style: RenderStyle,              // 視覚スタイル（後述）
}

pub struct SvgOptions {
    pub base_size: u32,                  // デフォルト 256
    pub override_aspect: Option<f32>,
    pub style: RenderStyle,              // 視覚スタイル
    #[deprecated(since = "0.3.0", note = "use style.blur; removed in 1.0")]
    pub blur: f32,                       // style.blur を使ってください
}

pub enum PixelSmooth { Nearest, Bilinear }
```

すべての struct は `Default` を実装。

## `RenderStyle`

```rust
#[derive(Clone, Copy, Debug, Default)]
pub struct RenderStyle {
    pub blur: f32,          // ガウスの stdDeviation（viewBox 単位）
    pub corner_radius: f32, // rect / square / rotrect のみ
}
```

codec のバイト形式から独立—同じ `(hash, codec)` に異なる `style` を渡す
と、ハッシュバイトを変えずに視覚的に異なる出力が得られます。デフォルト
（`RenderStyle::default()`）はゼロコストの fast path、pre-0.3.0 と出力が
バイト一致。

非 rect ファミリーの codec（circle / triangle / pixel / DCT）に対する
`corner_radius` は静かに無視されます—TS SDK はコンパイル時に条件型で捕捉、
Python は `UserWarning` を発出、Rust は data-in / data-out の慣例に従って
静かに無視する経路を取ります。

## Codec

```rust
pub enum Codec {
    Dct,
    Circle    { n: u32, color: ColorMode },
    Triangle  { n: u32, color: ColorMode },
    Square    { n: u32, color: ColorMode },
    Rect      { n: u32, color: ColorMode },
    RotRect   { n: u32, theta_bits: u32, color: ColorMode },
    Pixel     { n: u32, grid_aspect: Option<f32>, color: ColorMode },
    Raw       { spec: RawCodecSpec },
}

pub enum ColorMode {
    Rgb565,
    Rgb888,
    Palette(Palette),
}
```

### ファクトリメソッド

```rust
Codec::dct()
Codec::circle(n)
Codec::triangle(n)
Codec::square(n)
Codec::rect(n)
Codec::rotated_rect(n)
Codec::pixel(n)
Codec::raw(spec)
```

すべての shape ファクトリは `ColorMode::Rgb565` がデフォルト。パレットに切り替えるには `with_palette` を使用：

```rust
let c = Codec::triangle(24).with_palette(Palette::from_hex(&[...]).unwrap());
```

### メソッド

| メソッド                        | 説明                                       |
| ------------------------------- | ------------------------------------------ |
| `codec.bytes_total()`           | ハッシュ総バイト数                         |
| `codec.is_palette_mode()`       | `bool`                                     |
| `codec.with_palette(p)`         | パレットカラーモードに切り替えたクローン    |

## Preset

```rust
pub enum Preset {
    // サイズ軸：small (n=12, pixel n=16) / medium (n=24) / large (n=64)
    Dct,
    SmallTriangle, SmallCircle, SmallPixel, SmallRect, SmallSquare,
    MediumTriangle, MediumCircle, MediumPixel, MediumRect, MediumSquare,
    LargeTriangle, LargeCircle, LargePixel, LargeRect, LargeSquare,

    // 0.3 以前の非推奨エイリアス—ソース互換のため保持。
    TinyDct,
    PlaceholderTriangle, PlaceholderCircle, PlaceholderPixel,
    DetailTriangle, DetailCircle, DetailPixel,
}

impl Preset {
    pub fn codec(self) -> Codec;
    pub fn all() -> &'static [Preset];
    pub fn name(self) -> &'static str;
    pub fn from_name(s: &str) -> Option<Self>;
}
```

## Palette

```rust
pub struct Palette {
    pub bytes: Vec<u8>,    // フラット row-major sRGB
    pub k: u32,            // = bytes.len() / 3、[2, 1024] の 2 の冪
}

impl Palette {
    pub fn from_rgb(colors: &[[u8; 3]]) -> Result<Self, PaletteError>;
    pub fn from_hex(hexes: &[&str]) -> Result<Self, PaletteError>;
}
```

## 検索オプション

エンコードコスト / 品質にのみ影響—ハッシュのバイト形式は同一です。

```rust
pub struct SearchOptions {
    pub strategy: SearchStrategy,
    pub n_random: u32,
    pub n_topk: u32,
    pub hill_climb_steps: u32,
    pub hill_climb_max_age: Option<u32>,
    pub n_attempts: u32,
}

pub enum SearchStrategy { Primitive, TopkUniform }
```

## 例

```rust
use arthash::{Codec, Preset, encode_rgb, decode, EncodeOptions, DecodeOptions};

fn main() {
    let (w, h, rgb) = load_rgb("photo.jpg");

    // 最小—DCT、~21 B
    let codec = Codec::dct();
    let hash = encode_rgb(&rgb, w, h, &codec, EncodeOptions::default());
    println!("{} bytes", hash.len());

    // 名前付きプリセット
    let codec = Preset::LargeTriangle.codec();
    let hash = encode_rgb(&rgb, w, h, &codec, EncodeOptions::default());
    let out = decode(&hash, &codec, DecodeOptions { base_size: 512, ..Default::default() });
    write_png("placeholder.png", &out.rgba, out.width, out.height);
}
```

## SPEC 適合性

バイト形式は [`docs/SPEC.md`](https://github.com/Jannchie/arthash/blob/main/docs/SPEC.md) に固定。Rust crate は参照実装です：適合性ベクターは `packages/arthash-rs/tests/vectors.rs` にあり、Python と TypeScript のテストスイートも `docs/test-vectors/` に対して検証します。

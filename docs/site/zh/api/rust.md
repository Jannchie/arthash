# Rust API

crate：[`arthash`](https://crates.io/crates/arthash)。标准实现——Python 和 TypeScript 绑定都调用同一份代码。

```rust
use arthash::{
    Codec, Preset,
    encode_rgb, encode_rgba, decode, to_svg,
    EncodeOptions, DecodeOptions, SvgRenderOptions,
    SearchOptions, SearchStrategy,
    Palette,
};
```

## 顶层函数

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

row-major RGB，每像素 3 字节；长度必须等于 `3 * width * height`。

### `encode_rgba(rgba, width, height, codec, opts) -> Vec<u8>`

同上，输入是 RGBA。shape 模式会在内部把 alpha 合成到白色背景。

### `decode(hash, codec, opts) -> DecodeResult`

```rust
pub struct DecodeResult {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,     // row-major，每像素 4 字节
}

pub fn decode(
    hash: &[u8],
    codec: &Codec,
    opts: DecodeOptions,
) -> DecodeResult;
```

### `to_svg(hash, codec, opts) -> String`

```rust
pub fn to_svg(
    hash: &[u8],
    codec: &Codec,
    opts: SvgRenderOptions,
) -> String;
```

只支持 shape 模式（`CIRCLE`、`TRIANGLE`、`SQUARE`、`RECT`、`ROTATED_RECT`），DCT / PIXEL 返回 `Err`。

## Options

```rust
pub struct EncodeOptions {
    pub seed: u64,                       // 默认 0
    pub search: Option<SearchOptions>,   // 覆盖搜索预算
}

pub struct DecodeOptions {
    pub base_size: u32,                  // 默认 256
    pub override_aspect: Option<f32>,
    pub aa: Option<u32>,                 // shape 超采样（1 / 2 / 4）
    pub pixel_smooth: Option<PixelSmooth>,
}

pub struct SvgRenderOptions {
    pub base_size: u32,                  // 默认 256
    pub override_aspect: Option<f32>,
    pub blur: f32,                       // 高斯 stdDeviation；0 = 关
}

pub enum PixelSmooth { Nearest, Bilinear }
```

所有 struct 都实现了 `Default`。

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

### 工厂方法

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

所有 shape 工厂默认 `ColorMode::Rgb565`，要切换调色板请用 `with_palette`：

```rust
let c = Codec::triangle(24).with_palette(Palette::from_hex(&[...]).unwrap());
```

### 方法

| 方法                            | 描述                                        |
| ------------------------------- | ------------------------------------------- |
| `codec.bytes_total()`           | hash 总字节数                               |
| `codec.is_palette_mode()`       | `bool`                                      |
| `codec.with_palette(p)`         | 返回切到调色板颜色模式的副本                |

## Preset

```rust
pub enum Preset {
    // 尺寸轴：small (n=12, pixel n=16) / medium (n=24) / large (n=64)
    Dct,
    SmallTriangle, SmallCircle, SmallPixel, SmallRect, SmallSquare,
    MediumTriangle, MediumCircle, MediumPixel, MediumRect, MediumSquare,
    LargeTriangle, LargeCircle, LargePixel, LargeRect, LargeSquare,

    // 0.3 之前的别名——为 source 兼容保留。
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
    pub bytes: Vec<u8>,    // 扁平 row-major sRGB
    pub k: u32,            // = bytes.len() / 3，必须是 [2, 1024] 中 2 的幂
}

impl Palette {
    pub fn from_rgb(colors: &[[u8; 3]]) -> Result<Self, PaletteError>;
    pub fn from_hex(hexes: &[&str]) -> Result<Self, PaletteError>;
}
```

## 搜索选项

只影响编码成本 / 质量——hash 字节格式相同。

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

## 示例

```rust
use arthash::{Codec, Preset, encode_rgb, decode, EncodeOptions, DecodeOptions};

fn main() {
    let (w, h, rgb) = load_rgb("photo.jpg");

    // 极致小——DCT，~21 B
    let codec = Codec::dct();
    let hash = encode_rgb(&rgb, w, h, &codec, EncodeOptions::default());
    println!("{} bytes", hash.len());

    // 命名预设
    let codec = Preset::LargeTriangle.codec();
    let hash = encode_rgb(&rgb, w, h, &codec, EncodeOptions::default());
    let out = decode(&hash, &codec, DecodeOptions { base_size: 512, ..Default::default() });
    write_png("placeholder.png", &out.rgba, out.width, out.height);
}
```

## SPEC 一致性

字节格式定在 [`docs/SPEC.md`](https://github.com/Jannchie/arthash/blob/main/docs/SPEC.md)。Rust crate 是参考实现：一致性向量在 `packages/arthash-rs/tests/vectors.rs`，Python 与 TypeScript 测试套件也会对照 `docs/test-vectors/` 验证。

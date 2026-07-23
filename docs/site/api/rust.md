# Rust API

Crate: [`arthash`](https://crates.io/crates/arthash). The canonical
implementation — Python and TypeScript bindings call the same code.

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

## Top-level functions

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

Row-major RGB, 3 bytes per pixel; length must equal `3 * width * height`.

### `encode_rgba(rgba, width, height, codec, opts) -> Vec<u8>`

As above for RGBA input. Shape modes composite alpha over white internally.

### `decode(hash, codec, opts) -> DecodeResult`

```rust
pub struct DecodeResult {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,     // row-major, 4 bytes per pixel
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

Only supports shape modes (`CIRCLE`, `TRIANGLE`, `SQUARE`, `RECT`,
`ROTATED_RECT`). Returns `Err(SvgError::UnsupportedShape(_))` for `DCT`.

## Options

```rust
pub struct EncodeOptions {
    pub seed: u64,                       // default 0
    pub search: Option<SearchOptions>,   // override search budget
}

pub struct DecodeOptions {
    pub base_size: u32,                  // default 256
    pub override_aspect: Option<f32>,
    pub aa: u32,                         // shape supersample (1 / 2 / 4)
    pub pixel_smooth: PixelSmooth,
    pub style: RenderStyle,              // visual styling (see below)
    pub dither: bool,                    // Bayer 8×8 dither at 8-bit quantization
                                         // (DCT / blurred output); on a DCT codec
                                         // with a render-time palette (Codec::Raw),
                                         // dithers the palette quantization.
                                         // Default false
    pub dither_scale: u32,               // palette-dither dot pitch in output px;
                                         // 0 (default) = auto (base_size/128)
}

pub struct SvgOptions {
    pub base_size: u32,                  // default 256
    pub override_aspect: Option<f32>,
    pub style: RenderStyle,              // visual styling
    #[deprecated(since = "0.3.0", note = "use style.blur; removed in 1.0")]
    pub blur: f32,                       // use style.blur instead
}

pub enum PixelSmooth { Nearest, Bilinear }
```

All structs implement `Default`.

## `RenderStyle`

```rust
#[derive(Clone, Copy, Debug, Default)]
pub struct RenderStyle {
    pub blur: f32,          // Gaussian stdDeviation in viewBox units
    pub corner_radius: f32, // rect / square / rotrect only
}
```

Independent of the codec byte format — same `(hash, codec)` with different
`style` produces visually distinct output without changing the hash bytes.
Default (`RenderStyle::default()`) takes the zero-cost fast path and is
byte-identical to pre-0.3.0 output.

`corner_radius` is silently ignored on non-rect-family codecs (circle /
triangle / pixel / DCT) — the TS SDK catches this at compile time via
conditional types; Python emits a `UserWarning`; Rust takes the silent
path matching idiomatic data-in / data-out behavior.

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

### Factory methods

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

All shape factories default to `ColorMode::Rgb565`. Wrap in `with_palette` to
switch:

```rust
let c = Codec::triangle(24).with_palette(Palette::from_hex(&[...]).unwrap());
```

### Methods

| Method                          | Description                              |
| ------------------------------- | ---------------------------------------- |
| `codec.bytes_total()`           | Total hash bytes                         |
| `codec.is_palette_mode()`       | `bool`                                   |
| `codec.with_palette(p)`         | Clone with palette colour                |

## Preset

```rust
pub enum Preset {
    // Size axis: small (n=12, pixel n=16) / medium (n=24) / large (n=64)
    Dct,
    SmallTriangle, SmallCircle, SmallPixel, SmallRect, SmallSquare,
    MediumTriangle, MediumCircle, MediumPixel, MediumRect, MediumSquare,
    LargeTriangle, LargeCircle, LargePixel, LargeRect, LargeSquare,

    // Deprecated pre-0.3 aliases — kept for source compatibility.
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
    pub bytes: Vec<u8>,    // flat row-major sRGB
    pub k: u32,            // = bytes.len() / 3, must be a power of 2 in [2, 1024]
}

impl Palette {
    pub fn from_rgb(colors: &[[u8; 3]]) -> Result<Self, PaletteError>;
    pub fn from_hex(hexes: &[&str]) -> Result<Self, PaletteError>;
}
```

## Search options

Affect encoder cost / quality only — the resulting hash is byte-format
identical.

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

## Example

```rust
use arthash::{Codec, Preset, encode_rgb, decode, EncodeOptions, DecodeOptions};

fn main() {
    let (w, h, rgb) = load_rgb("photo.jpg");

    // Smallest — DCT, ~21 B
    let codec = Codec::dct();
    let hash = encode_rgb(&rgb, w, h, &codec, EncodeOptions::default());
    println!("{} bytes", hash.len());

    // Named preset
    let codec = Preset::LargeTriangle.codec();
    let hash = encode_rgb(&rgb, w, h, &codec, EncodeOptions::default());
    let out = decode(&hash, &codec, DecodeOptions { base_size: 512, ..Default::default() });
    write_png("placeholder.png", &out.rgba, out.width, out.height);
}
```

## SPEC conformance

The byte format is pinned in
[`docs/SPEC.md`](https://github.com/Jannchie/arthash/blob/main/docs/SPEC.md).
The Rust crate is the reference implementation: a conformance vector lives in
`packages/arthash-rs/tests/vectors.rs` and is also checked from the Python and
TypeScript test suites against `docs/test-vectors/`.

//! `arthash` CLI — encode images to placeholder hashes; decode hashes back to
//! preview PNG or compact SVG.
//!
//! Hash bytes alone are not self-describing — `decode`/`svg` need the same
//! `--shape` and `-n` flags that were passed to `encode`. The defaults here
//! match `Codec::default()` (DCT, 12 shapes).

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use image::imageops::FilterType;
use image::{DynamicImage, ImageBuffer, ImageFormat, ImageReader, Rgb, RgbImage};
use arthash::{
    decode as pf_decode, encode_rgb, to_svg as pf_to_svg, Codec, DecodeOptions, EncodeOptions,
    Preset, SvgOptions,
};
use std::fs;
use std::io::{self, Cursor, Read, Write};
use std::path::{Path, PathBuf};

/// DCT mode target long-edge (SPEC §3 — encoder works at ≤ 100 px).
const DCT_TARGET: u32 = 100;
/// Shape-mode encoder thumbnail long-edge (mirrors `arthash::shape::THUMB`).
const SHAPE_TARGET: u32 = 48;

#[derive(Parser)]
#[command(name = "arthash", version, about = "Encode images to placeholder hashes and back.")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Encode an image file (PNG/JPEG/WebP) into a arthash.
    Encode {
        /// Input image path.
        input: PathBuf,
        /// Output path. `-` or omitted ⇒ stdout.
        #[arg(short = 'o', long)]
        output: Option<PathBuf>,
        #[command(flatten)]
        codec: CodecArgs,
        /// Output encoding.
        #[arg(long, value_enum, default_value_t = HashFormat::Raw)]
        format: HashFormat,
        /// RNG seed (shape modes only).
        #[arg(long, default_value_t = 0)]
        seed: u64,
    },
    /// Decode a arthash into a preview PNG.
    Decode {
        /// Path to hash bytes. `-` ⇒ stdin.
        input: PathBuf,
        /// Output PNG path.
        #[arg(short = 'o', long)]
        output: PathBuf,
        #[command(flatten)]
        codec: CodecArgs,
        /// Input encoding.
        #[arg(long, value_enum, default_value_t = HashFormat::Raw)]
        format: HashFormat,
        /// Long-edge pixel target for the rendered preview.
        #[arg(long, default_value_t = 256)]
        size: u32,
    },
    /// Encode an image and immediately render the placeholder thumbnail.
    /// Skips writing the hash to disk — useful for previews.
    Thumb {
        /// Input image path.
        input: PathBuf,
        /// Output path. Omitted ⇒ stdout. Extension picks the format
        /// when `--format auto` (the default).
        #[arg(short = 'o', long)]
        output: Option<PathBuf>,
        /// Output format. `auto` ⇒ infer from `-o` extension, else PNG.
        #[arg(long, value_enum, default_value_t = ThumbFormat::Auto)]
        format: ThumbFormat,
        #[command(flatten)]
        codec: CodecArgs,
        /// Long-edge pixel target for the rendered preview.
        #[arg(long, default_value_t = 256)]
        size: u32,
        /// Gaussian blur stdDeviation (SVG output only; `0` = none).
        #[arg(long, default_value_t = 0.0)]
        blur: f32,
        /// RNG seed (shape modes only).
        #[arg(long, default_value_t = 0)]
        seed: u64,
    },
    /// Render a arthash as an SVG (CIRCLE / TRIANGLE only).
    Svg {
        /// Path to hash bytes. `-` ⇒ stdin.
        input: PathBuf,
        /// Output path. `-` or omitted ⇒ stdout.
        #[arg(short = 'o', long)]
        output: Option<PathBuf>,
        #[command(flatten)]
        codec: CodecArgs,
        /// Input encoding.
        #[arg(long, value_enum, default_value_t = HashFormat::Raw)]
        format: HashFormat,
        /// Long-edge pixel value used in the SVG viewBox.
        #[arg(long, default_value_t = 256)]
        size: u32,
        /// Gaussian blur stdDeviation in viewBox units (`0` = none).
        #[arg(long, default_value_t = 0.0)]
        blur: f32,
    },
}

#[derive(clap::Args, Clone)]
struct CodecArgs {
    /// Named preset (e.g. `detail-triangle`, `placeholder-circle`).
    /// Overrides `--shape` / `--n-shapes` when set.
    #[arg(long, value_enum, conflicts_with_all = ["shape", "n_shapes"])]
    preset: Option<PresetArg>,
    /// Shape mode. Ignored when `--preset` is given.
    #[arg(long, value_enum, default_value_t = ShapeArg::Dct)]
    shape: ShapeArg,
    /// Number of shapes (all shape modes; ignored for DCT).
    #[arg(short = 'n', long = "n-shapes", default_value_t = 12)]
    n_shapes: u32,
}

impl CodecArgs {
    fn to_codec(&self) -> Codec {
        if let Some(p) = self.preset {
            return p.to_preset().codec();
        }
        match self.shape {
            ShapeArg::Dct => Codec::dct(),
            ShapeArg::Circle => Codec::circle(self.n_shapes),
            ShapeArg::Triangle => Codec::triangle(self.n_shapes),
            ShapeArg::Square => Codec::square(self.n_shapes),
            ShapeArg::Rect => Codec::rect(self.n_shapes),
            ShapeArg::RotatedRect => Codec::rotated_rect(self.n_shapes),
            ShapeArg::Pixel => Codec::pixel(self.n_shapes),
        }
    }

    fn is_dct(&self) -> bool {
        matches!(self.to_codec(), Codec::Dct)
    }

    fn supports_svg(&self) -> bool {
        !matches!(self.to_codec(), Codec::Dct | Codec::Pixel { .. })
    }
}

#[derive(ValueEnum, Clone, Copy)]
enum ShapeArg {
    Dct,
    Circle,
    Triangle,
    Square,
    Rect,
    #[value(name = "rotrect", alias = "rotated-rect")]
    RotatedRect,
    Pixel,
}

#[derive(ValueEnum, Clone, Copy)]
enum PresetArg {
    #[value(name = "tiny-dct")]
    TinyDct,
    #[value(name = "placeholder-triangle")]
    PlaceholderTriangle,
    #[value(name = "placeholder-circle")]
    PlaceholderCircle,
    #[value(name = "placeholder-pixel")]
    PlaceholderPixel,
    #[value(name = "medium-triangle")]
    MediumTriangle,
    #[value(name = "medium-circle")]
    MediumCircle,
    #[value(name = "medium-pixel")]
    MediumPixel,
    #[value(name = "detail-triangle")]
    DetailTriangle,
    #[value(name = "detail-circle")]
    DetailCircle,
    #[value(name = "detail-pixel")]
    DetailPixel,
}

impl PresetArg {
    fn to_preset(self) -> Preset {
        match self {
            PresetArg::TinyDct => Preset::TinyDct,
            PresetArg::PlaceholderTriangle => Preset::PlaceholderTriangle,
            PresetArg::PlaceholderCircle => Preset::PlaceholderCircle,
            PresetArg::PlaceholderPixel => Preset::PlaceholderPixel,
            PresetArg::MediumTriangle => Preset::MediumTriangle,
            PresetArg::MediumCircle => Preset::MediumCircle,
            PresetArg::MediumPixel => Preset::MediumPixel,
            PresetArg::DetailTriangle => Preset::DetailTriangle,
            PresetArg::DetailCircle => Preset::DetailCircle,
            PresetArg::DetailPixel => Preset::DetailPixel,
        }
    }
}

#[derive(ValueEnum, Clone, Copy)]
enum HashFormat {
    Raw,
    Hex,
}

#[derive(ValueEnum, Clone, Copy, PartialEq, Eq)]
enum ThumbFormat {
    Auto,
    Png,
    Jpeg,
    Webp,
    Svg,
}

impl ThumbFormat {
    /// Resolve `Auto` against the output path (or default to PNG).
    fn resolve(self, output: Option<&Path>) -> ThumbFormat {
        if self != ThumbFormat::Auto {
            return self;
        }
        output
            .and_then(|p| p.extension())
            .and_then(|e| e.to_str())
            .and_then(|e| match e.to_ascii_lowercase().as_str() {
                "png" => Some(ThumbFormat::Png),
                "jpg" | "jpeg" => Some(ThumbFormat::Jpeg),
                "webp" => Some(ThumbFormat::Webp),
                "svg" => Some(ThumbFormat::Svg),
                _ => None,
            })
            .unwrap_or(ThumbFormat::Png)
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Encode { input, output, codec, format, seed } => {
            cmd_encode(&input, output.as_deref(), &codec, format, seed)
        }
        Cmd::Decode { input, output, codec, format, size } => {
            cmd_decode(&input, &output, &codec, format, size)
        }
        Cmd::Svg { input, output, codec, format, size, blur } => {
            cmd_svg(&input, output.as_deref(), &codec, format, size, blur)
        }
        Cmd::Thumb { input, output, format, codec, size, blur, seed } => {
            cmd_thumb(&input, output.as_deref(), format, &codec, size, blur, seed)
        }
    }
}

fn cmd_encode(
    input: &std::path::Path,
    output: Option<&std::path::Path>,
    codec_args: &CodecArgs,
    format: HashFormat,
    seed: u64,
) -> Result<()> {
    let codec = codec_args.to_codec();
    let target = if codec_args.is_dct() { DCT_TARGET } else { SHAPE_TARGET };
    let (rgb, w, h) =
        load_and_resize(input, target).with_context(|| format!("loading {}", input.display()))?;

    let opts = EncodeOptions { seed, ..Default::default() };
    let bytes = encode_rgb(&rgb, w, h, &codec, opts);

    write_hash(output, &bytes, format)
}

fn cmd_decode(
    input: &std::path::Path,
    output: &std::path::Path,
    codec_args: &CodecArgs,
    format: HashFormat,
    size: u32,
) -> Result<()> {
    let codec = codec_args.to_codec();
    let hash = read_hash(input, format)?;

    let out = pf_decode(
        &hash,
        &codec,
        DecodeOptions { base_size: size, ..Default::default() },
    );

    // RGBA → RGB for PNG output (alpha is always 255 in current decode output).
    let mut rgb = Vec::with_capacity((out.width * out.height * 3) as usize);
    for px in out.rgba.chunks_exact(4) {
        rgb.extend_from_slice(&px[..3]);
    }
    let img: RgbImage = ImageBuffer::<Rgb<u8>, _>::from_raw(out.width, out.height, rgb)
        .context("decoded buffer size mismatch")?;
    img.save(output).with_context(|| format!("writing {}", output.display()))?;
    Ok(())
}

fn cmd_svg(
    input: &std::path::Path,
    output: Option<&std::path::Path>,
    codec_args: &CodecArgs,
    format: HashFormat,
    size: u32,
    blur: f32,
) -> Result<()> {
    let codec = codec_args.to_codec();
    let hash = read_hash(input, format)?;
    let svg = pf_to_svg(
        &hash,
        &codec,
        SvgOptions { base_size: size, blur, ..Default::default() },
    )
    .map_err(|e| anyhow::anyhow!("svg: {e:?}"))?;

    match output {
        None => {
            io::stdout().write_all(svg.as_bytes())?;
        }
        Some(p) => fs::write(p, svg).with_context(|| format!("writing {}", p.display()))?,
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn cmd_thumb(
    input: &Path,
    output: Option<&Path>,
    format: ThumbFormat,
    codec_args: &CodecArgs,
    size: u32,
    blur: f32,
    seed: u64,
) -> Result<()> {
    let codec = codec_args.to_codec();
    let resolved = format.resolve(output);

    // Reject SVG up-front for unsupported shapes (matches `to_svg`'s contract).
    if resolved == ThumbFormat::Svg && !codec_args.supports_svg() {
        bail!("svg output is not supported for DCT or PIXEL codecs");
    }

    let target = if codec_args.is_dct() { DCT_TARGET } else { SHAPE_TARGET };
    let (rgb, w, h) =
        load_and_resize(input, target).with_context(|| format!("loading {}", input.display()))?;

    let hash = encode_rgb(&rgb, w, h, &codec, EncodeOptions { seed, ..Default::default() });

    if resolved == ThumbFormat::Svg {
        let svg = pf_to_svg(
            &hash,
            &codec,
            SvgOptions { base_size: size, blur, ..Default::default() },
        )
        .map_err(|e| anyhow::anyhow!("svg: {e:?}"))?;
        match output {
            None => io::stdout().write_all(svg.as_bytes())?,
            Some(p) => fs::write(p, svg).with_context(|| format!("writing {}", p.display()))?,
        }
        return Ok(());
    }

    let out = pf_decode(
        &hash,
        &codec,
        DecodeOptions { base_size: size, ..Default::default() },
    );
    let mut rgb_out = Vec::with_capacity((out.width * out.height * 3) as usize);
    for px in out.rgba.chunks_exact(4) {
        rgb_out.extend_from_slice(&px[..3]);
    }
    let img: RgbImage = ImageBuffer::<Rgb<u8>, _>::from_raw(out.width, out.height, rgb_out)
        .context("decoded buffer size mismatch")?;
    let dyn_img = DynamicImage::ImageRgb8(img);

    let img_format = match resolved {
        ThumbFormat::Png => ImageFormat::Png,
        ThumbFormat::Jpeg => ImageFormat::Jpeg,
        ThumbFormat::Webp => ImageFormat::WebP,
        ThumbFormat::Svg | ThumbFormat::Auto => unreachable!(),
    };

    let mut buf: Vec<u8> = Vec::new();
    dyn_img
        .write_to(&mut Cursor::new(&mut buf), img_format)
        .context("encoding output image")?;
    match output {
        None => io::stdout().write_all(&buf)?,
        Some(p) => fs::write(p, &buf).with_context(|| format!("writing {}", p.display()))?,
    }
    Ok(())
}

/// Decode an image file, resize so its long edge equals `target`, return
/// row-major flat RGB at the resized resolution.
fn load_and_resize(path: &std::path::Path, target: u32) -> Result<(Vec<u8>, u32, u32)> {
    let img = ImageReader::open(path)?
        .with_guessed_format()?
        .decode()
        .context("decoding image")?;

    let (w0, h0) = (img.width(), img.height());
    let (w, h) = fit_long_edge(w0, h0, target);
    let resized = if (w, h) == (w0, h0) {
        img
    } else {
        img.resize_exact(w, h, FilterType::Lanczos3)
    };
    let rgb = resized.to_rgb8();
    Ok((rgb.into_raw(), w, h))
}

fn fit_long_edge(w: u32, h: u32, target: u32) -> (u32, u32) {
    if w == 0 || h == 0 {
        return (w.max(1), h.max(1));
    }
    if w.max(h) <= target {
        return (w, h);
    }
    if w >= h {
        let new_h = ((target as u64 * h as u64) / w as u64).max(1) as u32;
        (target, new_h)
    } else {
        let new_w = ((target as u64 * w as u64) / h as u64).max(1) as u32;
        (new_w, target)
    }
}

fn read_hash(path: &std::path::Path, format: HashFormat) -> Result<Vec<u8>> {
    let raw = if path == std::path::Path::new("-") {
        let mut buf = Vec::new();
        io::stdin().read_to_end(&mut buf)?;
        buf
    } else {
        fs::read(path).with_context(|| format!("reading {}", path.display()))?
    };
    match format {
        HashFormat::Raw => Ok(raw),
        HashFormat::Hex => decode_hex(std::str::from_utf8(&raw).context("hex input not UTF-8")?),
    }
}

fn write_hash(
    output: Option<&std::path::Path>,
    bytes: &[u8],
    format: HashFormat,
) -> Result<()> {
    let payload: Vec<u8> = match format {
        HashFormat::Raw => bytes.to_vec(),
        HashFormat::Hex => encode_hex(bytes).into_bytes(),
    };
    match output {
        None => io::stdout().write_all(&payload)?,
        Some(p) if p == std::path::Path::new("-") => io::stdout().write_all(&payload)?,
        Some(p) => fs::write(p, &payload).with_context(|| format!("writing {}", p.display()))?,
    }
    Ok(())
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0xf) as usize] as char);
    }
    s
}

fn decode_hex(s: &str) -> Result<Vec<u8>> {
    let cleaned: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    if cleaned.len() % 2 != 0 {
        bail!("hex input has odd length");
    }
    let mut out = Vec::with_capacity(cleaned.len() / 2);
    let bytes = cleaned.as_bytes();
    for i in (0..bytes.len()).step_by(2) {
        let hi = hex_nibble(bytes[i])?;
        let lo = hex_nibble(bytes[i + 1])?;
        out.push((hi << 4) | lo);
    }
    Ok(out)
}

fn hex_nibble(b: u8) -> Result<u8> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        b'A'..=b'F' => Ok(b - b'A' + 10),
        _ => bail!("non-hex character: {:?}", b as char),
    }
}

//! Codec — byte-format contract shared between encoder and decoder. SPEC §2.
//!
//! Public API is the [`Codec`] enum. Construct one via the factory methods:
//!
//! ```ignore
//! let c = Codec::dct();
//! let c = Codec::triangle(64);
//! let c = Codec::triangle(64).with_palette(Palette::new(my_rgb).unwrap());
//! let c = Codec::pixel(16).with_color(ColorMode::Rgb888);
//! ```
//!
//! Two codecs are byte-compatible iff they decode to the same internal
//! [`CodecConfig`] (i.e. same variant, same `n`, same color/palette, same
//! bit widths). Hash bytes are not self-describing; the codec is consensus
//! knowledge that both sides must hold.

use crate::colorspace::srgb_u8_to_linear;

/// Shape tag used by [`CodecConfig`]. Exposed for FFI bindings + conformance
/// tests that work at the SPEC level; normal users address shapes via the
/// [`Codec`] enum variants directly.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[doc(hidden)]
pub enum ShapeType {
    Dct,
    Circle,
    Triangle,
    Pixel,
    Square,
    Rect,
    RotatedRect,
}

impl ShapeType {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "dct" => Some(Self::Dct),
            "circle" => Some(Self::Circle),
            "triangle" => Some(Self::Triangle),
            "pixel" => Some(Self::Pixel),
            "square" => Some(Self::Square),
            "rect" => Some(Self::Rect),
            "rotrect" | "rotated_rect" => Some(Self::RotatedRect),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dct => "dct",
            Self::Circle => "circle",
            Self::Triangle => "triangle",
            Self::Pixel => "pixel",
            Self::Square => "square",
            Self::Rect => "rect",
            Self::RotatedRect => "rotrect",
        }
    }
}

/// Powers-of-two K allowed for palette mode (SPEC §4.4).
pub const VALID_PALETTE_K: &[usize] = &[2, 4, 8, 16, 32, 64, 128, 256, 512, 1024];

// ---------------------------------------------------------------------------
// Public API: Palette, ColorMode, Codec, Preset
// ---------------------------------------------------------------------------

/// Codec construction error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodecError {
    /// Palette byte slice length is not a multiple of 3.
    PaletteLenNotMultipleOf3(usize),
    /// Palette has fewer than 2 colors or its effective K is not one of
    /// [`VALID_PALETTE_K`].
    PaletteKInvalid(usize),
    /// `palette_k` exceeds the number of colors in the palette.
    PaletteKOverflow { k: usize, len: usize },
}

impl std::fmt::Display for CodecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PaletteLenNotMultipleOf3(n) => {
                write!(f, "palette byte length ({n}) is not a multiple of 3")
            }
            Self::PaletteKInvalid(k) => {
                write!(f, "palette K={k} must be a power of 2 in {VALID_PALETTE_K:?}")
            }
            Self::PaletteKOverflow { k, len } => {
                write!(f, "palette_k={k} > palette length={len}")
            }
        }
    }
}

impl std::error::Error for CodecError {}

/// An sRGB palette of `K ∈ {2, 4, 8, … 1024}` colors. Shape codecs that carry
/// a palette store `log₂K` bits per shape instead of the full color field —
/// the palette itself is consensus knowledge, not stored in the hash.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Palette {
    bytes: Vec<u8>,
    k: usize,
}

impl Palette {
    /// Build a palette from flat row-major sRGB bytes (`length = 3·K`).
    /// `K` is inferred as `length / 3` and must be in [`VALID_PALETTE_K`].
    pub fn new(srgb_bytes: impl Into<Vec<u8>>) -> Result<Self, CodecError> {
        let bytes = srgb_bytes.into();
        if bytes.len() % 3 != 0 {
            return Err(CodecError::PaletteLenNotMultipleOf3(bytes.len()));
        }
        let k = bytes.len() / 3;
        if !VALID_PALETTE_K.contains(&k) {
            return Err(CodecError::PaletteKInvalid(k));
        }
        Ok(Self { bytes, k })
    }

    /// Build from a slice of `(r, g, b)` triplets.
    pub fn from_rgb(colors: &[[u8; 3]]) -> Result<Self, CodecError> {
        let mut bytes = Vec::with_capacity(colors.len() * 3);
        for c in colors {
            bytes.extend_from_slice(c);
        }
        Self::new(bytes)
    }

    /// Take only the first `k` colors of an over-allocated palette buffer.
    /// `k` must be in [`VALID_PALETTE_K`] and `≤ self.len()`.
    pub fn with_k(mut self, k: usize) -> Result<Self, CodecError> {
        let len = self.bytes.len() / 3;
        if k > len {
            return Err(CodecError::PaletteKOverflow { k, len });
        }
        if !VALID_PALETTE_K.contains(&k) {
            return Err(CodecError::PaletteKInvalid(k));
        }
        self.k = k;
        Ok(self)
    }

    /// Number of colors in this palette.
    pub fn len(&self) -> usize {
        self.k
    }

    pub fn is_empty(&self) -> bool {
        self.k == 0
    }

    /// Bits per palette index (`log₂K`).
    pub fn bits(&self) -> u32 {
        (self.k as u32).trailing_zeros()
    }

    /// Raw sRGB bytes of the active `K` colors (length `3·K`).
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.k * 3]
    }
}

/// Color encoding for shape / PIXEL modes.
///
/// * [`ColorMode::Rgb565`] — 16 bits per color (default).
/// * [`ColorMode::Rgb888`] — 24 bits per color (more fidelity, larger hash).
/// * [`ColorMode::Palette`] — `log₂K` bits per palette index. The palette is
///   consensus knowledge; it is not stored in the hash.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum ColorMode {
    Rgb565,
    Rgb888,
    Palette(Palette),
}

impl Default for ColorMode {
    fn default() -> Self {
        ColorMode::Rgb565
    }
}

impl ColorMode {
    pub(crate) fn color_bits(&self) -> u32 {
        match self {
            ColorMode::Rgb565 => 16,
            ColorMode::Rgb888 => 24,
            // Palette mode stores indices, but `color_bits` retains the
            // continuous fallback width so non-palette consumers can still
            // ask. Codec dispatch uses `is_palette()` to branch first.
            ColorMode::Palette(_) => 16,
        }
    }

    pub(crate) fn palette(&self) -> Option<&Palette> {
        match self {
            ColorMode::Palette(p) => Some(p),
            _ => None,
        }
    }
}

/// Named presets — battle-tested codec recipes you can drop in without
/// understanding the byte format. Same value used at encode + decode time.
///
/// Two axes:
/// * **size** — `Small*` (n=12, pixel n=16, ~50–80 B) → `Medium*` (n=24,
///   ~100–150 B) → `Large*` (n=64, ~270–400 B).
/// * **shape** — `Triangle` / `Circle` / `Pixel` / `Rect` / `Square`.
///
/// Plus [`Preset::Dct`], the single frequency-domain placeholder (~21 B,
/// outside the size axis).
///
/// Actual byte counts vary ±1 B with image aspect. The pre-0.3 names
/// (`TinyDct` / `Placeholder*` / `Detail*`) are kept as deprecated aliases
/// for source compatibility and will be removed in 1.0.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[allow(deprecated)]
pub enum Preset {
    // ----- Active variants ------------------------------------------------

    /// V4 thumbhash-style hash, ~21 B. Blurry frequency-domain placeholder.
    Dct,

    /// 12-triangle mosaic, ~77 B. Quick SVG placeholder.
    SmallTriangle,
    /// 12-circle mosaic, ~53 B. SQIP-style overlapping circles.
    SmallCircle,
    /// 16-cell PIXEL mosaic, ~33 B. Lo-fi mosaic look.
    SmallPixel,
    /// 12-rectangle mosaic. Axis-aligned rectangles.
    SmallRect,
    /// 12-square mosaic. Axis-aligned squares.
    SmallSquare,

    /// 24-triangle mosaic, ~150 B. Middle ground between Small and Large.
    MediumTriangle,
    /// 24-circle mosaic, ~102 B. Middle ground with circular brush feel.
    MediumCircle,
    /// 24-cell PIXEL mosaic, ~49 B. Medium lo-fi mosaic.
    MediumPixel,
    /// 24-rectangle mosaic. Middle ground axis-aligned rectangles.
    MediumRect,
    /// 24-square mosaic. Middle ground axis-aligned squares.
    MediumSquare,

    /// 64-triangle mosaic, ~395 B. Detail level — playground default.
    LargeTriangle,
    /// 64-circle mosaic, ~267 B. Detail level with circular brush feel.
    LargeCircle,
    /// 64-cell PIXEL mosaic, ~129 B. Detail-level lo-fi mosaic.
    LargePixel,
    /// 64-rectangle mosaic. Detail-level axis-aligned rectangles.
    LargeRect,
    /// 64-square mosaic. Detail-level axis-aligned squares.
    LargeSquare,

    // ----- Deprecated pre-0.3 aliases -------------------------------------

    /// Deprecated alias for [`Preset::Dct`].
    #[deprecated(since = "0.3.0", note = "renamed to `Preset::Dct`")]
    TinyDct,
    /// Deprecated alias for [`Preset::SmallTriangle`].
    #[deprecated(since = "0.3.0", note = "renamed to `Preset::SmallTriangle`")]
    PlaceholderTriangle,
    /// Deprecated alias for [`Preset::SmallCircle`].
    #[deprecated(since = "0.3.0", note = "renamed to `Preset::SmallCircle`")]
    PlaceholderCircle,
    /// Deprecated alias for [`Preset::SmallPixel`].
    #[deprecated(since = "0.3.0", note = "renamed to `Preset::SmallPixel`")]
    PlaceholderPixel,
    /// Deprecated alias for [`Preset::LargeTriangle`].
    #[deprecated(since = "0.3.0", note = "renamed to `Preset::LargeTriangle`")]
    DetailTriangle,
    /// Deprecated alias for [`Preset::LargeCircle`].
    #[deprecated(since = "0.3.0", note = "renamed to `Preset::LargeCircle`")]
    DetailCircle,
    /// Deprecated alias for [`Preset::LargePixel`].
    #[deprecated(since = "0.3.0", note = "renamed to `Preset::LargePixel`")]
    DetailPixel,
}

#[allow(deprecated)]
impl Preset {
    pub fn codec(self) -> Codec {
        match self {
            Preset::Dct | Preset::TinyDct => Codec::dct(),
            Preset::SmallTriangle | Preset::PlaceholderTriangle => Codec::triangle(12),
            Preset::SmallCircle | Preset::PlaceholderCircle => Codec::circle(12),
            Preset::SmallPixel | Preset::PlaceholderPixel => Codec::pixel(16),
            Preset::SmallRect => Codec::rect(12),
            Preset::SmallSquare => Codec::square(12),
            Preset::MediumTriangle => Codec::triangle(24),
            Preset::MediumCircle => Codec::circle(24),
            Preset::MediumPixel => Codec::pixel(24),
            Preset::MediumRect => Codec::rect(24),
            Preset::MediumSquare => Codec::square(24),
            Preset::LargeTriangle | Preset::DetailTriangle => Codec::triangle(64),
            Preset::LargeCircle | Preset::DetailCircle => Codec::circle(64),
            Preset::LargePixel | Preset::DetailPixel => Codec::pixel(64),
            Preset::LargeRect => Codec::rect(64),
            Preset::LargeSquare => Codec::square(64),
        }
    }

    /// All named presets in declaration order — active variants first, then
    /// deprecated aliases. Used by [`Preset::from_name`] for round-trip parsing.
    pub fn all() -> &'static [Preset] {
        &[
            // Active
            Preset::Dct,
            Preset::SmallTriangle,
            Preset::SmallCircle,
            Preset::SmallPixel,
            Preset::SmallRect,
            Preset::SmallSquare,
            Preset::MediumTriangle,
            Preset::MediumCircle,
            Preset::MediumPixel,
            Preset::MediumRect,
            Preset::MediumSquare,
            Preset::LargeTriangle,
            Preset::LargeCircle,
            Preset::LargePixel,
            Preset::LargeRect,
            Preset::LargeSquare,
            // Deprecated aliases — kept so old serialized names round-trip.
            Preset::TinyDct,
            Preset::PlaceholderTriangle,
            Preset::PlaceholderCircle,
            Preset::PlaceholderPixel,
            Preset::DetailTriangle,
            Preset::DetailCircle,
            Preset::DetailPixel,
        ]
    }

    /// Stable name for serialization / CLI flags / config files. Each variant
    /// (including deprecated ones) returns its own name so round-trip via
    /// [`Preset::from_name`] is stable.
    pub fn name(self) -> &'static str {
        match self {
            Preset::Dct => "dct",
            Preset::SmallTriangle => "small_triangle",
            Preset::SmallCircle => "small_circle",
            Preset::SmallPixel => "small_pixel",
            Preset::SmallRect => "small_rect",
            Preset::SmallSquare => "small_square",
            Preset::MediumTriangle => "medium_triangle",
            Preset::MediumCircle => "medium_circle",
            Preset::MediumPixel => "medium_pixel",
            Preset::MediumRect => "medium_rect",
            Preset::MediumSquare => "medium_square",
            Preset::LargeTriangle => "large_triangle",
            Preset::LargeCircle => "large_circle",
            Preset::LargePixel => "large_pixel",
            Preset::LargeRect => "large_rect",
            Preset::LargeSquare => "large_square",
            Preset::TinyDct => "tiny_dct",
            Preset::PlaceholderTriangle => "placeholder_triangle",
            Preset::PlaceholderCircle => "placeholder_circle",
            Preset::PlaceholderPixel => "placeholder_pixel",
            Preset::DetailTriangle => "detail_triangle",
            Preset::DetailCircle => "detail_circle",
            Preset::DetailPixel => "detail_pixel",
        }
    }

    /// Parse from the stable [`name`](Self::name) string. Old pre-0.3 names
    /// (`tiny_dct` / `placeholder_*` / `detail_*`) still parse but return the
    /// deprecated variant.
    pub fn from_name(s: &str) -> Option<Self> {
        Preset::all().iter().copied().find(|p| p.name() == s)
    }
}

impl From<Preset> for Codec {
    fn from(p: Preset) -> Self {
        p.codec()
    }
}

/// Codec — the byte-format contract.
///
/// Construct via factory methods — `Codec::dct()`, `Codec::triangle(n)`, etc.
/// Override defaults with the `with_*` builders. The same `Codec` value MUST
/// be passed to both encode and decode; the hash bytes alone are not
/// self-describing.
///
/// Two codecs that compare equal (`==`) decode each other's hashes byte-for-byte.
/// For a looser check (e.g. ignoring quirks like `alpha_levels`), use
/// [`Codec::is_byte_compatible_with`].
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case", tag = "kind"))]
pub enum Codec {
    /// V4 thumbhash-style frequency-domain placeholder (~21 B).
    Dct,
    /// SQIP-style overlapping circles.
    Circle { n: u32, color: ColorMode },
    /// Primitive-style triangle mosaic.
    Triangle { n: u32, color: ColorMode },
    /// Axis-aligned squares.
    Square { n: u32, color: ColorMode },
    /// Axis-aligned rectangles.
    Rect { n: u32, color: ColorMode },
    /// Rotated rectangles. `theta_bits` tunes the angle quantization step
    /// (5 bits ⇒ ~5.6°/step, the default).
    RotatedRect {
        n: u32,
        theta_bits: u32,
        color: ColorMode,
    },
    /// Retro pixel mosaic. `grid_aspect` lets the caller pin the grid shape
    /// (else it's derived from image aspect at encode time).
    Pixel {
        n: u32,
        color: ColorMode,
        grid_aspect: Option<f32>,
    },
    /// Escape hatch for conformance tests / FFI bindings that need to drive
    /// every SPEC field directly. Use the factory methods or builders for
    /// normal usage.
    #[doc(hidden)]
    Raw(CodecConfig),
}

impl Default for Codec {
    fn default() -> Self {
        Codec::Dct
    }
}

impl Codec {
    // ----- factory methods ------------------------------------------------

    pub const fn dct() -> Self {
        Codec::Dct
    }

    pub fn circle(n: u32) -> Self {
        Codec::Circle { n, color: ColorMode::Rgb565 }
    }

    pub fn triangle(n: u32) -> Self {
        Codec::Triangle { n, color: ColorMode::Rgb565 }
    }

    pub fn square(n: u32) -> Self {
        Codec::Square { n, color: ColorMode::Rgb565 }
    }

    pub fn rect(n: u32) -> Self {
        Codec::Rect { n, color: ColorMode::Rgb565 }
    }

    pub fn rotated_rect(n: u32) -> Self {
        Codec::RotatedRect { n, theta_bits: 5, color: ColorMode::Rgb565 }
    }

    pub fn pixel(n: u32) -> Self {
        Codec::Pixel { n, color: ColorMode::Rgb565, grid_aspect: None }
    }

    // ----- builders -------------------------------------------------------

    /// Replace the color mode. No-op for [`Codec::Dct`] / [`Codec::Raw`]
    /// (DCT has its own internal color treatment; Raw codecs are already
    /// fully specified).
    pub fn with_color(mut self, new_color: ColorMode) -> Self {
        match &mut self {
            Codec::Dct | Codec::Raw(_) => {}
            Codec::Circle { color, .. }
            | Codec::Triangle { color, .. }
            | Codec::Square { color, .. }
            | Codec::Rect { color, .. }
            | Codec::RotatedRect { color, .. }
            | Codec::Pixel { color, .. } => *color = new_color,
        }
        self
    }

    /// Convenience: switch the color mode to palette indexing.
    pub fn with_palette(self, palette: Palette) -> Self {
        self.with_color(ColorMode::Palette(palette))
    }

    /// `RotatedRect` only — override `theta_bits` (default 5).
    pub fn with_theta_bits(mut self, bits: u32) -> Self {
        if let Codec::RotatedRect { theta_bits, .. } = &mut self {
            *theta_bits = bits;
        }
        self
    }

    /// `Pixel` only — pin the grid aspect.
    pub fn with_grid_aspect(mut self, aspect: f32) -> Self {
        if let Codec::Pixel { grid_aspect, .. } = &mut self {
            *grid_aspect = Some(aspect);
        }
        self
    }

    /// Read the color mode out (None for DCT / Raw).
    pub fn color(&self) -> Option<&ColorMode> {
        match self {
            Codec::Dct | Codec::Raw(_) => None,
            Codec::Circle { color, .. }
            | Codec::Triangle { color, .. }
            | Codec::Square { color, .. }
            | Codec::Rect { color, .. }
            | Codec::RotatedRect { color, .. }
            | Codec::Pixel { color, .. } => Some(color),
        }
    }

    /// Number of shapes / cells in this codec (None for DCT).
    pub fn n(&self) -> Option<u32> {
        match self {
            Codec::Dct => None,
            Codec::Circle { n, .. }
            | Codec::Triangle { n, .. }
            | Codec::Square { n, .. }
            | Codec::Rect { n, .. }
            | Codec::RotatedRect { n, .. }
            | Codec::Pixel { n, .. } => Some(*n),
            Codec::Raw(cfg) => match cfg.shape {
                ShapeType::Dct => None,
                _ => Some(cfg.n_shapes),
            },
        }
    }

    /// Internal lowering — collapse the enum into the SPEC field layout that
    /// encoder/decoder dispatchers consume. Bit widths follow the historical
    /// defaults (`cx=cy=5, r=4, alpha=3`); they are not exposed in the public
    /// API because shape-encoder cost grows superlinearly with coord precision
    /// and the defaults work for every realistic placeholder size.
    pub(crate) fn to_config(&self) -> CodecConfig {
        let mut cfg = CodecConfig::default();
        match self {
            Codec::Dct => {
                cfg.shape = ShapeType::Dct;
            }
            Codec::Circle { n, color } => {
                cfg.shape = ShapeType::Circle;
                cfg.n_shapes = *n;
                apply_color(&mut cfg, color);
            }
            Codec::Triangle { n, color } => {
                cfg.shape = ShapeType::Triangle;
                cfg.n_shapes = *n;
                apply_color(&mut cfg, color);
            }
            Codec::Square { n, color } => {
                cfg.shape = ShapeType::Square;
                cfg.n_shapes = *n;
                apply_color(&mut cfg, color);
            }
            Codec::Rect { n, color } => {
                cfg.shape = ShapeType::Rect;
                cfg.n_shapes = *n;
                apply_color(&mut cfg, color);
            }
            Codec::RotatedRect { n, theta_bits, color } => {
                cfg.shape = ShapeType::RotatedRect;
                cfg.n_shapes = *n;
                cfg.theta_bits = *theta_bits;
                apply_color(&mut cfg, color);
            }
            Codec::Pixel { n, color, grid_aspect } => {
                cfg.shape = ShapeType::Pixel;
                cfg.n_shapes = *n;
                cfg.grid_aspect = *grid_aspect;
                apply_color(&mut cfg, color);
            }
            Codec::Raw(raw) => return raw.clone(),
        }
        cfg
    }

    /// Header + per-shape bit counts → total hash byte length.
    pub fn bytes_total(&self) -> usize {
        let cfg = self.to_config();
        cfg.bytes_total(false)
    }

    /// Returns true if hashes produced by `self` will decode correctly under
    /// `other`. Looser than `==`: only the SPEC fields that actually drive the
    /// byte layout are compared (shape, n, all bit widths, palette bytes,
    /// alpha-levels effect). Stylistic differences like overall codec variant
    /// don't matter — `Codec::triangle(64)` is byte-compatible with the
    /// equivalent `Codec::Raw(...)`.
    pub fn is_byte_compatible_with(&self, other: &Codec) -> bool {
        let a = self.to_config();
        let b = other.to_config();
        a.shape == b.shape
            && a.n_shapes == b.n_shapes
            && a.cx_bits == b.cx_bits
            && a.cy_bits == b.cy_bits
            && a.r_bits == b.r_bits
            && a.alpha_bits == b.alpha_bits
            && a.color_bits == b.color_bits
            && a.theta_bits == b.theta_bits
            && a.effective_palette_k() == b.effective_palette_k()
            && palette_active_bytes(&a) == palette_active_bytes(&b)
    }
}

fn palette_active_bytes(cfg: &CodecConfig) -> Option<&[u8]> {
    let pal = cfg.palette.as_ref()?;
    let k = cfg.effective_palette_k().unwrap_or(pal.len() / 3);
    Some(&pal[..k * 3])
}

fn apply_color(cfg: &mut CodecConfig, color: &ColorMode) {
    match color {
        ColorMode::Rgb565 => {
            cfg.color_bits = 16;
            cfg.palette = None;
            cfg.palette_k = None;
        }
        ColorMode::Rgb888 => {
            cfg.color_bits = 24;
            cfg.palette = None;
            cfg.palette_k = None;
        }
        ColorMode::Palette(pal) => {
            cfg.color_bits = 16;
            cfg.palette = Some(pal.bytes.clone());
            cfg.palette_k = Some(pal.k);
        }
    }
}

// ---------------------------------------------------------------------------
// Internal field-layout struct
// ---------------------------------------------------------------------------

/// Low-level SPEC field layout. Most users should construct codecs via the
/// [`Codec`] factory methods; this struct is exposed primarily for FFI
/// bindings and conformance tests that need to drive every SPEC field
/// directly. Wrap one in [`Codec::Raw`] to pass it through `encode_*` /
/// `decode` / `to_svg`.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[doc(hidden)]
pub struct CodecConfig {
    pub shape: ShapeType,
    pub n_shapes: u32,
    pub cx_bits: u32,
    pub cy_bits: u32,
    /// CIRCLE: radius bits. SQUARE: side bits. RECT/ROTATED_RECT: per-axis
    /// extent bits (width and height each get this many).
    pub r_bits: u32,
    pub alpha_bits: u32,
    pub color_bits: u32, // 16 = RGB-565, 24 = RGB-888
    pub theta_bits: u32,

    pub palette: Option<Vec<u8>>,
    pub palette_k: Option<usize>,
    pub alpha_levels: Option<Vec<f32>>,
    pub grid_aspect: Option<f32>,
}

impl Default for CodecConfig {
    fn default() -> Self {
        Self {
            shape: ShapeType::Dct,
            n_shapes: 12,
            cx_bits: 5,
            cy_bits: 5,
            r_bits: 4,
            alpha_bits: 3,
            color_bits: 16,
            theta_bits: 5,
            palette: None,
            palette_k: None,
            alpha_levels: None,
            grid_aspect: None,
        }
    }
}

impl CodecConfig {
    pub(crate) fn is_palette_mode(&self) -> bool {
        self.palette.is_some()
    }

    pub(crate) fn effective_palette_k(&self) -> Option<usize> {
        let pal = self.palette.as_ref()?;
        Some(self.palette_k.unwrap_or(pal.len() / 3))
    }

    pub(crate) fn palette_bits(&self) -> u32 {
        match self.effective_palette_k() {
            Some(k) if k >= 2 => (k as u32).trailing_zeros(),
            _ => 0,
        }
    }

    pub(crate) fn color_field_bits(&self) -> u32 {
        if self.is_palette_mode() {
            self.palette_bits()
        } else {
            self.color_bits
        }
    }

    pub(crate) fn palette_linear(&self) -> Option<Vec<f32>> {
        let pal = self.palette.as_ref()?;
        let k = self.effective_palette_k().unwrap_or(pal.len() / 3);
        let active = &pal[..k * 3];
        Some(active.iter().map(|&c| srgb_u8_to_linear(c)).collect())
    }

    pub(crate) fn alpha_levels_owned(&self) -> Vec<f32> {
        if let Some(levels) = &self.alpha_levels {
            return levels.clone();
        }
        let n = 1usize << self.alpha_bits;
        if n == 1 {
            return vec![0.20];
        }
        (0..n)
            .map(|i| 0.20 + (0.90 - 0.20) * (i as f32) / ((n - 1) as f32))
            .collect()
    }

    /// SPEC: per-shape body bit count.
    pub(crate) fn per_shape_bits(&self) -> u32 {
        let cx = self.cx_bits;
        let cy = self.cy_bits;
        let r = self.r_bits;
        let col = self.color_field_bits();
        let a = self.alpha_bits;
        match self.shape {
            ShapeType::Circle | ShapeType::Square => cx + cy + r + col + a,
            ShapeType::Rect => cx + cy + 2 * r + col + a,
            ShapeType::RotatedRect => cx + cy + 2 * r + self.theta_bits + col + a,
            ShapeType::Triangle => 3 * (cx + cy) + col + a,
            ShapeType::Pixel => col,
            ShapeType::Dct => 0,
        }
    }

    pub(crate) fn header_bits(&self) -> u32 {
        match self.shape {
            ShapeType::Dct => 40,
            ShapeType::Pixel => 8,
            _ => 8 + self.color_field_bits(),
        }
    }

    /// Total hash byte count. `has_alpha` is DCT-specific (alpha channel
    /// adds a fixed quantized block per SPEC §3.5).
    pub(crate) fn bytes_total(&self, has_alpha: bool) -> usize {
        if matches!(self.shape, ShapeType::Dct) {
            let header = if has_alpha { 48 } else { 40 };
            let n_l_max = 28;
            return (header + 4 * (n_l_max + 16 + if has_alpha { 24 } else { 0 }) + 7) / 8;
        }
        let bits = self.header_bits() + self.n_shapes * self.per_shape_bits();
        bits.div_ceil(8) as usize
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_codec_is_dct() {
        let c = Codec::default();
        assert!(matches!(c, Codec::Dct));
    }

    #[test]
    fn triangle_factory_sets_n_and_default_color() {
        let c = Codec::triangle(24);
        match c {
            Codec::Triangle { n, color } => {
                assert_eq!(n, 24);
                assert!(matches!(color, ColorMode::Rgb565));
            }
            _ => panic!("expected Triangle"),
        }
    }

    #[test]
    fn with_palette_swaps_color() {
        let pal = Palette::from_rgb(&[[0, 0, 0]; 16]).unwrap();
        let c = Codec::triangle(12).with_palette(pal);
        match c {
            Codec::Triangle { color, .. } => {
                assert!(matches!(color, ColorMode::Palette(_)));
            }
            _ => panic!("expected Triangle"),
        }
    }

    #[test]
    fn palette_new_rejects_bad_length() {
        assert!(matches!(
            Palette::new(vec![0u8; 7]),
            Err(CodecError::PaletteLenNotMultipleOf3(7))
        ));
    }

    #[test]
    fn palette_new_rejects_non_pow2_k() {
        assert!(matches!(
            Palette::new(vec![0u8; 3 * 6]),
            Err(CodecError::PaletteKInvalid(6))
        ));
    }

    #[test]
    fn palette_with_k_truncates() {
        let pal = Palette::new(vec![0u8; 3 * 16]).unwrap();
        let pal8 = pal.with_k(8).unwrap();
        assert_eq!(pal8.len(), 8);
        assert_eq!(pal8.bits(), 3);
    }

    #[test]
    fn config_per_shape_bits_circle() {
        let cfg = Codec::circle(12).to_config();
        assert_eq!(cfg.per_shape_bits(), 33); // 5+5+4+16+3
    }

    #[test]
    fn config_per_shape_bits_triangle() {
        let cfg = Codec::triangle(12).to_config();
        assert_eq!(cfg.per_shape_bits(), 49); // 3*(5+5)+16+3
    }

    #[test]
    fn config_per_shape_bits_pixel() {
        let cfg = Codec::pixel(12).to_config();
        assert_eq!(cfg.per_shape_bits(), 16); // color only
    }

    #[test]
    fn rotated_rect_default_theta_bits_5() {
        let c = Codec::rotated_rect(8);
        match c {
            Codec::RotatedRect { theta_bits, .. } => assert_eq!(theta_bits, 5),
            _ => panic!(),
        }
    }

    #[test]
    fn with_theta_bits_only_affects_rotated_rect() {
        let c = Codec::rotated_rect(8).with_theta_bits(7);
        match c {
            Codec::RotatedRect { theta_bits, .. } => assert_eq!(theta_bits, 7),
            _ => panic!(),
        }
        // No-op on other variants:
        let c2 = Codec::circle(4).with_theta_bits(7);
        assert!(matches!(c2, Codec::Circle { .. }));
    }

    #[test]
    fn preset_to_codec() {
        let c: Codec = Preset::LargeTriangle.into();
        match c {
            Codec::Triangle { n, .. } => assert_eq!(n, 64),
            _ => panic!(),
        }
        // New rect/square presets reach the right factory.
        match Codec::from(Preset::LargeRect) {
            Codec::Rect { n, .. } => assert_eq!(n, 64),
            _ => panic!(),
        }
        match Codec::from(Preset::SmallSquare) {
            Codec::Square { n, .. } => assert_eq!(n, 12),
            _ => panic!(),
        }
    }

    /// Deprecated aliases produce byte-identical codecs to their replacements.
    #[test]
    #[allow(deprecated)]
    fn preset_deprecated_aliases_equivalent() {
        assert_eq!(Preset::TinyDct.codec(), Preset::Dct.codec());
        assert_eq!(Preset::PlaceholderTriangle.codec(), Preset::SmallTriangle.codec());
        assert_eq!(Preset::PlaceholderCircle.codec(), Preset::SmallCircle.codec());
        assert_eq!(Preset::PlaceholderPixel.codec(), Preset::SmallPixel.codec());
        assert_eq!(Preset::DetailTriangle.codec(), Preset::LargeTriangle.codec());
        assert_eq!(Preset::DetailCircle.codec(), Preset::LargeCircle.codec());
        assert_eq!(Preset::DetailPixel.codec(), Preset::LargePixel.codec());
    }

    #[test]
    fn codec_eq_works() {
        assert_eq!(Codec::triangle(64), Codec::triangle(64));
        assert_ne!(Codec::triangle(64), Codec::triangle(12));
        assert_ne!(Codec::triangle(64), Codec::circle(64));
    }

    #[test]
    fn factory_byte_compatible_with_raw() {
        let factory = Codec::triangle(64);
        let raw = Codec::Raw(factory.to_config());
        assert!(factory.is_byte_compatible_with(&raw));
        assert!(raw.is_byte_compatible_with(&factory));
        // Same byte total too.
        assert_eq!(factory.bytes_total(), raw.bytes_total());
    }

    #[test]
    fn preset_round_trips_via_name() {
        for &p in Preset::all() {
            assert_eq!(Preset::from_name(p.name()), Some(p));
        }
        assert_eq!(Preset::from_name("not-a-preset"), None);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_round_trip_triangle() {
        let c = Codec::triangle(64);
        let s = serde_json::to_string(&c).unwrap();
        let back: Codec = serde_json::from_str(&s).unwrap();
        assert_eq!(c, back);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_round_trip_palette_codec() {
        let pal = Palette::from_rgb(&[[0, 0, 0]; 16]).unwrap();
        let c = Codec::circle(8).with_palette(pal);
        let s = serde_json::to_string(&c).unwrap();
        let back: Codec = serde_json::from_str(&s).unwrap();
        assert_eq!(c, back);
    }

    #[cfg(feature = "serde")]
    #[test]
    #[allow(deprecated)]
    fn serde_preset_kebab_case() {
        // New canonical names round-trip.
        let s = serde_json::to_string(&Preset::LargeTriangle).unwrap();
        assert_eq!(s, "\"large_triangle\"");
        let back: Preset = serde_json::from_str(&s).unwrap();
        assert_eq!(back, Preset::LargeTriangle);

        // Deprecated names still parse — old serialized data must still load.
        let back_old: Preset = serde_json::from_str("\"detail_triangle\"").unwrap();
        assert_eq!(back_old, Preset::DetailTriangle);
        // And serialize back to their own name (round-trip stability).
        let s_old = serde_json::to_string(&Preset::DetailTriangle).unwrap();
        assert_eq!(s_old, "\"detail_triangle\"");

        // New rect/square presets serialize as expected.
        let s_rect = serde_json::to_string(&Preset::MediumRect).unwrap();
        assert_eq!(s_rect, "\"medium_rect\"");
    }
}

//! Codec + ShapeType. SPEC §2.
//!
//! Two codecs are byte-compatible iff `shape`, `n_shapes`, every bit width,
//! and `palette[..palette_k]` agree. Hash bytes contain only image-dependent
//! data; the Codec itself is consensus knowledge, not stored in the hash.

use crate::colorspace::srgb_u8_to_linear;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ShapeType {
    Dct,         // V4 thumbhash-style (~21 B)
    Circle,      // SQIP-style overlapping circles
    Triangle,    // Primitive-style triangle mosaic
    Pixel,       // Retro-palette pixel mosaic
    Square,      // Axis-aligned square (cx, cy, s)
    Rect,        // Axis-aligned rectangle (cx, cy, w, h)
    RotatedRect, // Rotated rectangle (cx, cy, w, h, theta)
}

impl ShapeType {
    #[allow(clippy::should_implement_trait)]
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
}

/// Powers-of-two K allowed for palette mode (SPEC §4.4).
pub const VALID_PALETTE_K: &[usize] = &[2, 4, 8, 16, 32, 64, 128, 256, 512, 1024];

#[derive(Clone, Debug)]
pub struct Codec {
    pub shape: ShapeType,
    pub n_shapes: u32,
    pub cx_bits: u32,
    pub cy_bits: u32,
    /// CIRCLE: radius bits. SQUARE: side bits. RECT/ROTATED_RECT: per-axis
    /// extent bits (width and height each get this many bits).
    pub r_bits: u32,
    pub alpha_bits: u32,
    pub color_bits: u32, // 16 = RGB-565, 24 = RGB-888
    /// ROTATED_RECT only: bits for theta ∈ [0, π). Unused by other modes.
    pub theta_bits: u32,

    /// (K, 3) uint8 sRGB palette, row-major flat. `None` ⇒ continuous color.
    pub palette: Option<Vec<u8>>,
    /// Effective K. Defaults to `palette.len() / 3` when palette is set.
    pub palette_k: Option<usize>,
    /// Discrete alpha set. Length MUST equal `1 << alpha_bits`. `None` ⇒
    /// `linspace(0.20, 0.90, 1 << alpha_bits)` (SPEC §4.5).
    pub alpha_levels: Option<Vec<f32>>,
    /// PIXEL only. `None` ⇒ derive from image aspect (SPEC §5.7).
    pub grid_aspect: Option<f32>,
}

impl Default for Codec {
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

impl Codec {
    pub fn is_palette_mode(&self) -> bool {
        self.palette.is_some()
    }

    pub fn effective_palette_k(&self) -> Option<usize> {
        let pal = self.palette.as_ref()?;
        Some(self.palette_k.unwrap_or(pal.len() / 3))
    }

    pub fn palette_bits(&self) -> u32 {
        match self.effective_palette_k() {
            Some(k) if k >= 2 => (k as u32).trailing_zeros(),
            _ => 0,
        }
    }

    pub fn color_field_bits(&self) -> u32 {
        if self.is_palette_mode() {
            self.palette_bits()
        } else {
            self.color_bits
        }
    }

    /// Linear-RGB palette of effective length K × 3 (row-major flat).
    pub fn palette_linear(&self) -> Option<Vec<f32>> {
        let pal = self.palette.as_ref()?;
        let k = self.effective_palette_k().unwrap_or(pal.len() / 3);
        let active = &pal[..k * 3];
        Some(active.iter().map(|&c| srgb_u8_to_linear(c)).collect())
    }

    /// Default alpha levels per SPEC §4.5: linspace(0.20, 0.90, 1 << alpha_bits).
    pub fn alpha_levels_owned(&self) -> Vec<f32> {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_codec_is_dct() {
        let c = Codec::default();
        assert_eq!(c.shape, ShapeType::Dct);
        assert_eq!(c.n_shapes, 12);
    }

    #[test]
    fn palette_bits() {
        let mut c = Codec::default();
        c.palette = Some(vec![0u8; 16 * 3]);
        assert_eq!(c.effective_palette_k(), Some(16));
        assert_eq!(c.palette_bits(), 4);
    }

    #[test]
    fn alpha_levels_default_3bit() {
        let c = Codec { alpha_bits: 3, ..Codec::default() };
        let levels = c.alpha_levels_owned();
        assert_eq!(levels.len(), 8);
        assert!((levels[0] - 0.20).abs() < 1e-6);
        assert!((levels[7] - 0.90).abs() < 1e-6);
    }
}

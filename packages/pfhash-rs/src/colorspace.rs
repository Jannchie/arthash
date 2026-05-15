//! sRGB ↔ linear RGB. SPEC §4.1.

/// Forward transform on uint8 input → f32 linear in [0, 1].
pub fn srgb_u8_to_linear(c: u8) -> f32 {
    let s = c as f32 / 255.0;
    if s <= 0.04045 {
        s / 12.92
    } else {
        ((s + 0.055) / 1.055).powf(2.4)
    }
}

/// Element-wise forward transform on an RGB u8 slice → flat f32 linear vec.
pub fn srgb_u8_slice_to_linear(rgb_u8: &[u8]) -> Vec<f32> {
    rgb_u8.iter().map(|&c| srgb_u8_to_linear(c)).collect()
}

/// Inverse transform on f32 linear → u8 sRGB.
pub fn linear_to_srgb_u8(lin: f32) -> u8 {
    let c = lin.clamp(0.0, 1.0);
    let s = if c <= 0.0031308 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (s * 255.0 + 0.5).clamp(0.0, 255.0) as u8
}
